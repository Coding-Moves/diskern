use diskern_core::{actions, report, rules::RulesDb, scanner, GenomeError, Verdict};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tauri::{Emitter, State, Window};

/// Handle on the scan that is running right now, if any.
///
/// The engine has always supported cancellation — `ScanProgress` carries the
/// flag and the walk checks it on every entry — but `start_scan` created its
/// `ScanProgress` as a local, so nothing outside that one call could ever
/// reach it. This is the shared slot that makes it reachable.
#[derive(Default)]
pub struct ActiveScan(Mutex<Option<Arc<scanner::ScanProgress>>>);

impl ActiveScan {
    /// A poisoned lock means some earlier holder panicked while swapping an
    /// `Option`. There is no invariant here worth protecting, and refusing
    /// to unlock would leave cancellation permanently broken for the rest of
    /// the session — so recover the value instead.
    fn slot(&self) -> MutexGuard<'_, Option<Arc<scanner::ScanProgress>>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[derive(Clone, Serialize)]
struct ScanProgressPayload {
    files_seen: u64,
    bytes_seen: u64,
}

/// Everything a running scan owns outside itself: the ticker thread that
/// emits `scan-progress`, and this scan's entry in the shared
/// [`ActiveScan`] slot. Both are released on drop.
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
    state: &'a ActiveScan,
    progress: Arc<scanner::ScanProgress>,
}

impl<'a> ScanRun<'a> {
    fn start(
        window: &Window,
        state: &'a ActiveScan,
        progress: Arc<scanner::ScanProgress>,
    ) -> Self {
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));

        // A plain OS thread keeps this independent of whatever async
        // runtime Tauri is using internally.
        let ticker = {
            let progress = progress.clone();
            let stop = stop.clone();
            let window = window.clone();
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let payload = ScanProgressPayload {
                        files_seen: progress.files_seen.load(Ordering::Relaxed),
                        bytes_seen: progress.bytes_seen.load(Ordering::Relaxed),
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
            progress,
        }
    }
}

impl Drop for ScanRun<'_> {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(ticker) = self.ticker.take() {
            let _ = ticker.join();
        }

        // Clear only if the slot still holds *this* scan. An unconditional
        // `= None` would let a short scan that started second erase a
        // longer one's handle when it finished first, leaving the survivor
        // with a Cancel button wired to nothing.
        let mut slot = self.state.slot();
        if slot
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &self.progress))
        {
            *slot = None;
        }
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
    state: State<'_, ActiveScan>,
    roots: Vec<PathBuf>,
) -> Result<Option<report::Report>, String> {
    let progress = Arc::new(scanner::ScanProgress::default());
    // Publish the handle before the walk starts. The guard is a temporary
    // here on purpose — holding it across the await below would make this
    // future non-Send.
    *state.slot() = Some(progress.clone());

    // Everything this scan has to undo, undone on the way out however the
    // way out happens.
    let run = ScanRun::start(&window, &state, progress.clone());

    let progress_for_scan = progress.clone();
    let joined = tauri::async_runtime::spawn_blocking(move || {
        let opts = scanner::ScanOptions {
            roots,
            ..Default::default()
        };
        match scanner::scan(&opts, progress_for_scan.clone()) {
            // build_cancellable also returns None when cancelled — the walk
            // is only the first half, and dedup hashing is where a cancel
            // most needs to land.
            Ok(entries) => Ok(report::build_cancellable(
                entries,
                &RulesDb::embedded(),
                &progress_for_scan.cancelled,
            )),
            // The user asked for this. `None` means "cancelled", which the
            // frontend renders as an outcome rather than a red error box.
            Err(GenomeError::Cancelled) => Ok(None),
            Err(e) => Err::<_, String>(e.to_string()),
        }
    })
    .await;

    drop(run);

    // One final snapshot so the UI's last-seen count matches the real total.
    let _ = window.emit(
        "scan-progress",
        ScanProgressPayload {
            files_seen: progress.files_seen.load(Ordering::Relaxed),
            bytes_seen: progress.bytes_seen.load(Ordering::Relaxed),
        },
    );

    joined.map_err(|e| e.to_string())?
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
pub fn cancel_scan(state: State<'_, ActiveScan>) -> bool {
    match state.slot().as_ref() {
        Some(progress) => {
            progress.cancel();
            true
        }
        None => false,
    }
}

/// The mutating commands below all take `quarantine_dir` from the
/// frontend, which resolves it to `<app local data>/Quarantine`. Nothing
/// here trusts a verdict the frontend claims — `quarantine_finding`
/// re-classifies, and the restore/purge commands only ever touch paths
/// the manifest in that directory says Diskern put there itself.
///
/// Re-classifies server-side before acting — the frontend's claimed
/// verdict is never trusted.
#[tauri::command]
pub async fn quarantine_finding(
    path: PathBuf,
    quarantine_dir: PathBuf,
) -> Result<actions::QuarantineRecord, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (_, verdict, _) = RulesDb::embedded().classify(&path);
        if matches!(verdict, Verdict::Protected | Verdict::Risky) {
            return Err(format!(
                "{} is classified {verdict:?}; action refused",
                path.display()
            ));
        }
        actions::quarantine(&path, verdict, &quarantine_dir).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
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
