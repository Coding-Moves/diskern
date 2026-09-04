use diskern_core::{actions, report, rules::RulesDb, scanner, Verdict};
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
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
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
#[tauri::command]
pub async fn start_scan(
    window: Window,
    state: State<'_, ActiveScan>,
    roots: Vec<PathBuf>,
) -> Result<report::Report, String> {
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
        let entries = scanner::scan(&opts, progress_for_scan).map_err(|e| e.to_string())?;
        Ok::<_, String>(report::build(entries, &RulesDb::embedded()))
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
