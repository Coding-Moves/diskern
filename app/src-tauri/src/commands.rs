use diskern_core::{
    actions, report, report::ReportStage, rules::RulesDb, scanner, FileIdentity, Finding,
    GenomeError,
};
use serde::Serialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tauri::{Emitter, State, Window};

/// Backend-owned scan/report authority.
///
/// A completed report is a review snapshot. Starting or cancelling a scan
/// invalidates the previous snapshot, and only the scan that owns the current
/// generation may publish. The mutex protects short state transitions only;
/// filesystem scanning and quarantine I/O happen outside it.
#[derive(Default)]
pub struct ScanAuthority(Mutex<AuthorityState>);

#[derive(Default)]
struct AuthorityState {
    next_generation: u64,
    active: Option<ActiveGeneration>,
    cancelled_generation: Option<u64>,
    report: Option<CompletedReport>,
    epoch: Arc<AtomicU64>,
}

struct ActiveGeneration {
    generation: u64,
    progress: Arc<scanner::ScanProgress>,
}

struct CompletedReport {
    generation: u64,
    report: Arc<report::Report>,
}

struct ActionLease {
    generation: u64,
    report: Arc<report::Report>,
    epoch: Arc<AtomicU64>,
}

#[derive(Debug, Eq, PartialEq)]
enum PublishOutcome {
    Published,
    Cancelled,
    Superseded,
}

impl ActionLease {
    fn is_current(&self) -> bool {
        self.epoch.load(Ordering::Acquire) == self.generation
    }
}

impl ScanAuthority {
    fn slot(&self) -> MutexGuard<'_, AuthorityState> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn begin(&self, progress: Arc<scanner::ScanProgress>) -> u64 {
        let mut state = self.slot();
        if let Some(previous) = state.active.take() {
            previous.progress.cancel();
        }
        state.next_generation = state.next_generation.wrapping_add(1);
        let generation = state.next_generation;
        state.epoch.store(generation, Ordering::Release);
        state.cancelled_generation = None;
        state.report = None;
        state.active = Some(ActiveGeneration {
            generation,
            progress,
        });
        generation
    }

    fn publish(&self, generation: u64, report: report::Report) -> PublishOutcome {
        let mut state = self.slot();
        if state
            .active
            .as_ref()
            .is_some_and(|active| active.generation == generation)
            && state.epoch.load(Ordering::Acquire) == generation
        {
            state.report = Some(CompletedReport {
                generation,
                report: Arc::new(report),
            });
            state.active = None;
            state.cancelled_generation = None;
            PublishOutcome::Published
        } else if state.cancelled_generation == Some(generation) {
            state.cancelled_generation = None;
            PublishOutcome::Cancelled
        } else {
            PublishOutcome::Superseded
        }
    }

    fn abandon(&self, generation: u64) {
        let mut state = self.slot();
        if state
            .active
            .as_ref()
            .is_some_and(|active| active.generation == generation)
        {
            state.active = None;
            state.report = None;
        }
        if state.cancelled_generation == Some(generation) {
            state.cancelled_generation = None;
        }
    }

    fn finish_active(&self, generation: u64) {
        let mut state = self.slot();
        if state
            .active
            .as_ref()
            .is_some_and(|active| active.generation == generation)
        {
            state.active = None;
        }
    }

    fn cancel(&self) -> bool {
        let mut state = self.slot();
        let Some(active) = state.active.take() else {
            return false;
        };
        state.cancelled_generation = Some(active.generation);
        active.progress.cancel();
        state
            .epoch
            .store(active.generation.wrapping_add(1), Ordering::Release);
        state.report = None;
        true
    }

    fn lease(&self) -> Option<ActionLease> {
        let state = self.slot();
        if state.active.is_some() {
            return None;
        }
        let completed = state.report.as_ref()?;
        Some(ActionLease {
            generation: completed.generation,
            report: completed.report.clone(),
            epoch: state.epoch.clone(),
        })
    }
}

#[derive(Clone, Copy)]
enum ScanPhase {
    WalkingFiles,
    BuildingImpactGraph,
    ClassifyingFiles,
    CheckingDuplicates,
    PreparingFindings,
}

impl ScanPhase {
    fn label(self) -> &'static str {
        match self {
            ScanPhase::WalkingFiles => "Walking files",
            ScanPhase::BuildingImpactGraph => "Building project impact graph",
            ScanPhase::ClassifyingFiles => "Classifying files",
            ScanPhase::CheckingDuplicates => "Checking duplicate candidates",
            ScanPhase::PreparingFindings => "Preparing findings",
        }
    }
}

fn set_scan_phase(phase: &Mutex<ScanPhase>, next: ScanPhase) {
    *phase
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = next;
}

fn phase_for_report_stage(stage: ReportStage) -> ScanPhase {
    match stage {
        ReportStage::BuildingImpactGraph => ScanPhase::BuildingImpactGraph,
        ReportStage::ClassifyingEntries => ScanPhase::ClassifyingFiles,
        ReportStage::FindingDuplicates => ScanPhase::CheckingDuplicates,
        ReportStage::PreparingFindings => ScanPhase::PreparingFindings,
    }
}

#[derive(Clone, Serialize)]
struct ScanProgressPayload {
    files_seen: u64,
    bytes_seen: u64,
    phase: &'static str,
}

#[derive(Clone, Serialize)]
struct ScanPreviewPayload {
    findings: Vec<Finding>,
    files_scanned: u64,
    total_reclaimable: u64,
}

/// Everything a running scan owns outside itself: the ticker thread that
/// emits `scan-progress`, and this scan's entry in the shared
/// authority slot. Both are released on drop.
///
/// They used to be released by statements after the `.await`, which only
/// run if control reaches them. A `?` between the two — there was one —
/// returned on a panicking blocking task and left the ticker emitting
/// every 150ms for the rest of the process, behind whatever the UI showed
/// next. Dropping the command's future, which is how a Tauri command is
/// cancelled, skipped the cleanup entirely. A guard cannot be skipped.
struct ScanRun<'a> {
    stop: Arc<std::sync::atomic::AtomicBool>,
    ticker: Option<std::thread::JoinHandle<()>>,
    state: &'a ScanAuthority,
    generation: u64,
}

impl<'a> ScanRun<'a> {
    fn start(
        window: &Window,
        state: &'a ScanAuthority,
        generation: u64,
        progress: Arc<scanner::ScanProgress>,
        phase: Arc<Mutex<ScanPhase>>,
    ) -> Self {
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));

        // A plain OS thread keeps this independent of whatever async
        // runtime Tauri is using internally.
        let ticker = {
            let progress = progress.clone();
            let phase = phase.clone();
            let stop = stop.clone();
            let window = window.clone();
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let phase = *phase
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    let payload = ScanProgressPayload {
                        files_seen: progress.files_seen.load(Ordering::Relaxed),
                        bytes_seen: progress.bytes_seen.load(Ordering::Relaxed),
                        phase: phase.label(),
                    };
                    let _ = window.emit("scan-progress", payload);
                    std::thread::sleep(Duration::from_millis(150));
                }
            })
        };

        Self {
            stop,
            ticker: Some(ticker),
            state,
            generation,
        }
    }
}

impl Drop for ScanRun<'_> {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(ticker) = self.ticker.take() {
            let _ = ticker.join();
        }

        // Clear only if the slot still belongs to this generation. A newer
        // scan may already own it.
        self.state.finish_active(self.generation);
    }
}

/// Read-only scan. Safe to expose; touches nothing.
///
/// Emits a `scan-progress` event roughly every 150ms while running, so the
/// UI can show a live "N files found" counter instead of a frozen button.
/// This is a live count, not a percentage — the total file count isn't
/// known until the walk finishes, so a true percentage would be fake.
///
/// Returns `None` when the scan was cancelled. Cancelling is a thing the
/// user did on purpose, so it travels as a successful outcome with no
/// report, not as an `Err` the UI would have to pattern-match on a string.
#[tauri::command]
pub async fn start_scan(
    window: Window,
    state: State<'_, Arc<ScanAuthority>>,
    roots: Vec<PathBuf>,
) -> Result<Option<report::Report>, String> {
    let progress = Arc::new(scanner::ScanProgress::default());
    let authority = state.inner().clone();
    let generation = authority.begin(progress.clone());
    let phase = Arc::new(Mutex::new(ScanPhase::WalkingFiles));

    // Everything this scan has to undo, undone on the way out however the
    // way out happens.
    let run = ScanRun::start(
        &window,
        &authority,
        generation,
        progress.clone(),
        phase.clone(),
    );

    let progress_for_scan = progress.clone();
    let phase_for_scan = phase.clone();
    let window_for_preview = window.clone();
    let joined = tauri::async_runtime::spawn_blocking(move || {
        let opts = scanner::ScanOptions {
            roots,
            ..Default::default()
        };
        let rules = RulesDb::embedded();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let mut counted_identities: HashSet<FileIdentity> = HashSet::new();
        let mut preview_findings = Vec::new();
        let mut preview_total_reclaimable = 0;
        match scanner::scan_with(&opts, progress_for_scan.clone(), |entry| {
            if let Some(finding) =
                report::provisional_finding(entry, &rules, &mut counted_identities, now)
            {
                preview_total_reclaimable += finding.reclaimable;
                preview_findings.push(finding);

                // A scan can find many files per second. Emit in modest
                // batches so the frontend gets early rows without making
                // every single filesystem entry a cross-thread UI event.
                if preview_findings.len() % 25 == 0 {
                    let _ = window_for_preview.emit(
                        "scan-preview",
                        ScanPreviewPayload {
                            findings: preview_findings.clone(),
                            files_scanned: progress_for_scan.files_seen.load(Ordering::Relaxed),
                            total_reclaimable: preview_total_reclaimable,
                        },
                    );
                }
            }
        }) {
            // build_cancellable also returns None when cancelled — the walk
            // is only the first half, and dedup hashing is where a cancel
            // most needs to land.
            Ok(entries) => {
                let _ = window_for_preview.emit(
                    "scan-preview",
                    ScanPreviewPayload {
                        findings: preview_findings,
                        files_scanned: progress_for_scan.files_seen.load(Ordering::Relaxed),
                        total_reclaimable: preview_total_reclaimable,
                    },
                );
                Ok(report::build_with_progress(
                    entries,
                    &rules,
                    &report::ReportOptions::default(),
                    &progress_for_scan.cancelled,
                    |stage| set_scan_phase(&phase_for_scan, phase_for_report_stage(stage)),
                ))
            }
            // The user asked for this. `None` means "cancelled", which the
            // frontend renders as an outcome rather than a red error box.
            Err(GenomeError::Cancelled) => Ok(None),
            Err(e) => Err::<_, String>(e.to_string()),
        }
    })
    .await;

    // One final snapshot so the UI's last-seen count matches the real total.
    let phase = *phase
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _ = window.emit(
        "scan-progress",
        ScanProgressPayload {
            files_seen: progress.files_seen.load(Ordering::Relaxed),
            bytes_seen: progress.bytes_seen.load(Ordering::Relaxed),
            phase: phase.label(),
        },
    );

    let joined = joined.map_err(|e| e.to_string())?;
    let result = match joined {
        Ok(Some(report)) => match authority.publish(generation, report.clone()) {
            PublishOutcome::Published => Ok(Some(report)),
            PublishOutcome::Cancelled => Ok(None),
            PublishOutcome::Superseded => Err("scan was superseded by a newer scan".to_string()),
        },
        Ok(None) => {
            authority.abandon(generation);
            Ok(None)
        }
        Err(error) => {
            authority.abandon(generation);
            Err(error)
        }
    };
    drop(run);
    result
}

/// Stop the scan that is currently running.
///
/// Returns whether there was one to stop. A cancel that arrives after the
/// walk has already finished is a no-op, not an error — the UI can race
/// this against the scan completing and doesn't need to care who won.
///
/// Read-only, like `start_scan`: setting the flag makes the walk return
/// early. Nothing is written, moved, or deleted.
#[tauri::command]
pub fn cancel_scan(state: State<'_, Arc<ScanAuthority>>) -> bool {
    state.cancel()
}

/// The mutating commands below all take `quarantine_dir` from the frontend,
/// which resolves it to `<app local data>/Quarantine`. Nothing here trusts a
/// verdict the frontend claims: `quarantine_finding` takes only a path lookup
/// key and obtains the finding from the current backend report lease. The
/// report-bound core action then applies the graph-aware verdict and fresh
/// static-rule tightening before moving anything.
#[tauri::command]
pub async fn quarantine_finding(
    state: State<'_, Arc<ScanAuthority>>,
    path: PathBuf,
    quarantine_dir: PathBuf,
) -> Result<actions::QuarantineRecord, String> {
    let authority = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        quarantine_from_authoritative(&authority, &path, &quarantine_dir)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn quarantine_from_authoritative(
    authority: &ScanAuthority,
    path: &std::path::Path,
    quarantine_dir: &std::path::Path,
) -> Result<actions::QuarantineRecord, String> {
    let lease = authority.lease().ok_or_else(|| {
        "no current completed scan report; refusing to quarantine — scan again".to_string()
    })?;
    if !lease.is_current() {
        return Err("scan report was superseded; refusing to quarantine — scan again".into());
    }
    actions::quarantine_finding_with_authorization(
        path,
        &lease.report,
        &RulesDb::embedded(),
        quarantine_dir,
        || lease.is_current(),
    )
    .map_err(|e| e.to_string())
}

/// Everything still in quarantine, read from the manifest on disk.
///
/// Read-only. This is what makes quarantine reversible across restarts:
/// before the manifest existed, the record of where a file came from lived
/// only in the value `quarantine_finding` returned, and the UI dropped it.
#[tauri::command]
pub async fn list_quarantine(
    quarantine_dir: PathBuf,
) -> Result<Vec<actions::QuarantineRecord>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        actions::list(&quarantine_dir).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Put one quarantined file back where it came from.
///
/// Addressed by its path *in quarantine*, which is the unique one — the
/// same original can be quarantined, restored and quarantined again. The
/// manifest is consulted first, so a path the frontend invented reaches
/// no file.
#[tauri::command]
pub async fn restore_quarantined(
    quarantine_dir: PathBuf,
    quarantined_to: PathBuf,
) -> Result<actions::QuarantineRecord, String> {
    tauri::async_runtime::spawn_blocking(move || {
        actions::restore_from_manifest(&quarantine_dir, &quarantined_to).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Empty quarantine for good.
///
/// The one command in Diskern that deletes anything, and it deletes only
/// the files the manifest lists — the ones this app moved there. The UI
/// confirms first; this is past the point of no return.
#[tauri::command]
pub async fn purge_quarantine(quarantine_dir: PathBuf) -> Result<actions::PurgeSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        actions::purge(&quarantine_dir).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use diskern_core::{Category, FileEntry, Finding, Verdict};
    use std::path::Path;

    fn empty_report() -> report::Report {
        report::Report {
            findings: vec![],
            duplicate_sets: vec![],
            total_reclaimable: 0,
            files_scanned: 0,
        }
    }

    fn report_for(path: &Path, verdict: Verdict) -> report::Report {
        let metadata = std::fs::symlink_metadata(path).unwrap();
        let epoch = |time: std::time::SystemTime| {
            time.duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
        };
        report::Report {
            findings: vec![Finding {
                entry: FileEntry {
                    path: path.to_path_buf(),
                    size: metadata.len(),
                    modified: metadata.modified().ok().map(epoch),
                    accessed: metadata.accessed().ok().map(epoch),
                    is_symlink: metadata.file_type().is_symlink(),
                    identity: None,
                    hash: None,
                },
                category: Category::TempFile,
                verdict,
                risk_score: 0.5,
                reasons: vec!["backend test".into()],
                reclaimable: metadata.len(),
            }],
            duplicate_sets: vec![],
            total_reclaimable: metadata.len(),
            files_scanned: 1,
        }
    }

    #[test]
    fn report_stages_map_to_user_visible_scan_phases() {
        assert_eq!(
            phase_for_report_stage(ReportStage::BuildingImpactGraph).label(),
            "Building project impact graph"
        );
        assert_eq!(
            phase_for_report_stage(ReportStage::ClassifyingEntries).label(),
            "Classifying files"
        );
        assert_eq!(
            phase_for_report_stage(ReportStage::FindingDuplicates).label(),
            "Checking duplicate candidates"
        );
        assert_eq!(
            phase_for_report_stage(ReportStage::PreparingFindings).label(),
            "Preparing findings"
        );
    }

    #[test]
    fn no_completed_report_fails_closed() {
        let authority = ScanAuthority::default();
        let error = quarantine_from_authoritative(
            &authority,
            Path::new("/not-in-a-report"),
            Path::new("/not-created"),
        )
        .unwrap_err();
        assert!(error.contains("no current completed scan report"));
    }

    #[test]
    fn only_the_current_generation_can_publish() {
        let authority = ScanAuthority::default();
        let first_progress = Arc::new(scanner::ScanProgress::default());
        let first = authority.begin(first_progress.clone());
        let second_progress = Arc::new(scanner::ScanProgress::default());
        let second = authority.begin(second_progress);

        assert!(first_progress.cancelled.load(Ordering::Acquire));
        assert_eq!(
            authority.publish(first, empty_report()),
            PublishOutcome::Superseded
        );
        assert_eq!(
            authority.publish(second, empty_report()),
            PublishOutcome::Published
        );
        assert!(authority.lease().is_some());
    }

    #[test]
    fn cancellation_completion_race_returns_the_cancellation_outcome() {
        let authority = ScanAuthority::default();
        let progress = Arc::new(scanner::ScanProgress::default());
        let generation = authority.begin(progress.clone());

        // The walk may finish and return a report after cancel() has revoked
        // the active slot but before it observes the cancellation flag.
        assert!(authority.cancel());
        assert_eq!(
            authority.publish(generation, empty_report()),
            PublishOutcome::Cancelled
        );
        assert!(progress.cancelled.load(Ordering::Acquire));
        assert!(authority.lease().is_none());
    }

    #[test]
    fn a_new_generation_makes_an_old_completion_a_supersede() {
        let authority = ScanAuthority::default();
        let first = authority.begin(Arc::new(scanner::ScanProgress::default()));
        assert!(authority.cancel());
        let second = authority.begin(Arc::new(scanner::ScanProgress::default()));

        // The cancellation marker is generation-scoped. Once a new scan
        // begins, a late completion from the old one is a real supersede.
        assert_eq!(
            authority.publish(first, empty_report()),
            PublishOutcome::Superseded
        );
        assert_eq!(
            authority.publish(second, empty_report()),
            PublishOutcome::Published
        );
    }

    #[test]
    fn cancellation_invalidates_the_report_authority() {
        let authority = ScanAuthority::default();
        let progress = Arc::new(scanner::ScanProgress::default());
        authority.begin(progress.clone());
        assert!(authority.cancel());
        assert!(progress.cancelled.load(Ordering::Acquire));
        assert!(authority.lease().is_none());
        assert!(!authority.cancel());
    }

    #[test]
    fn an_invalidated_action_lease_cannot_move_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("cache.bin");
        let quarantine_dir = dir.path().join("quarantine");
        std::fs::write(&target, b"cache").unwrap();
        let authority = ScanAuthority::default();
        let generation = authority.begin(Arc::new(scanner::ScanProgress::default()));
        assert_eq!(
            authority.publish(generation, report_for(&target, Verdict::Review)),
            PublishOutcome::Published
        );
        let lease = authority.lease().unwrap();

        authority.begin(Arc::new(scanner::ScanProgress::default()));
        let result = actions::quarantine_finding_with_authorization(
            &target,
            &lease.report,
            &RulesDb::embedded(),
            &quarantine_dir,
            || lease.is_current(),
        );
        assert!(result.is_err());
        assert!(target.exists());
        assert!(!quarantine_dir.exists());
    }

    #[test]
    fn risky_report_is_refused_without_a_frontend_verdict() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("dependency.bin");
        let quarantine_dir = dir.path().join("quarantine");
        std::fs::write(&target, b"keep").unwrap();
        let authority = ScanAuthority::default();
        let generation = authority.begin(Arc::new(scanner::ScanProgress::default()));
        assert_eq!(
            authority.publish(generation, report_for(&target, Verdict::Risky)),
            PublishOutcome::Published
        );

        let result = quarantine_from_authoritative(&authority, &target, &quarantine_dir);
        assert!(result.is_err());
        assert!(target.exists());
    }
}
