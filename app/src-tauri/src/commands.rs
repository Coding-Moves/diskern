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
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));

    // Ticker thread: emits progress snapshots on an interval until `stop`
    // is set once the scan below finishes. A plain OS thread keeps this
    // independent of whatever async runtime Tauri is using internally.
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

    let progress_for_scan = progress.clone();
    // Note the missing `?`: the join result is unwrapped *after* the cleanup
    // below. Returning early here would leave the ticker running and this
    // scan still listed as in-flight, so cancelling would target a scan that
    // had already ended.
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

    stop.store(true, Ordering::Relaxed);
    let _ = ticker.join();
    // Whatever happened, this scan is no longer in flight.
    *state.slot() = None;

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

/// The ONLY mutating command. Re-classifies server-side before acting —
/// the frontend's claimed verdict is never trusted.
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
