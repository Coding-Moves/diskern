use diskern_core::{actions, report, rules::RulesDb, scanner, Verdict};
use std::path::PathBuf;
use std::sync::Arc;

/// Read-only scan. Safe to expose; touches nothing.
#[tauri::command]
pub async fn start_scan(roots: Vec<PathBuf>) -> Result<report::Report, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let opts = scanner::ScanOptions { roots, ..Default::default() };
        let progress = Arc::new(scanner::ScanProgress::default());
        let entries = scanner::scan(&opts, progress).map_err(|e| e.to_string())?;
        Ok(report::build(entries, &RulesDb::embedded()))
    })
    .await
    .map_err(|e| e.to_string())?
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
            return Err(format!("{} is classified {verdict:?}; action refused", path.display()));
        }
        actions::quarantine(&path, verdict, &quarantine_dir).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
