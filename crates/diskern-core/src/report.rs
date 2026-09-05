//! Assembles scanner + dedup + rules + risk into Findings — the single
//! structure both the CLI and the Tauri UI render.

use crate::{dedup, risk, rules::RulesDb, Category, FileEntry, Finding, Verdict};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};

/// Knobs for [`build_with`] that are about the report, not the walk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportOptions {
    /// Files strictly smaller than this take no part in duplicate
    /// detection. This lived on `ScanOptions` as `min_file_size` and was
    /// applied inside the walk, which meant raising it to speed dedup up
    /// also hid every small file from the rules engine, the risk model,
    /// the report and `files_scanned` — none of which the name promised.
    pub dedup_min_size: u64,
}

impl Default for ReportOptions {
    fn default() -> Self {
        // 1, not 0: a zero-byte file is identical to every other zero-byte
        // file, so a default of 0 makes one enormous duplicate set worth
        // nothing. They still reach the report as findings.
        Self { dedup_min_size: 1 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub duplicate_sets: Vec<dedup::DuplicateSet>,
    pub total_reclaimable: u64,
    pub files_scanned: u64,
}

pub fn build(entries: Vec<FileEntry>, rules: &RulesDb) -> Report {
    static NEVER: AtomicBool = AtomicBool::new(false);
    build_cancellable(entries, rules, &NEVER)
        .expect("a run that cannot be cancelled cannot stop early")
}

/// [`build`], abandoned as soon as `cancelled` is set. `None` means it
/// stopped early, so there is no report to show — not an error, just the
/// user's answer arriving before ours.
pub fn build_cancellable(
    entries: Vec<FileEntry>,
    rules: &RulesDb,
    cancelled: &AtomicBool,
) -> Option<Report> {
    build_with(entries, rules, &ReportOptions::default(), cancelled)
}

/// [`build_cancellable`] with the report knobs spelled out.
pub fn build_with(
    mut entries: Vec<FileEntry>,
    rules: &RulesDb,
    opts: &ReportOptions,
    cancelled: &AtomicBool,
) -> Option<Report> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let duplicate_sets = dedup::find_duplicates_filtered(
        &mut entries,
        |e| e.size >= opts.dedup_min_size,
        cancelled,
    )?;
    let files_scanned = entries.len() as u64;

    let mut findings = Vec::new();
    for entry in entries {
        // Classification is cheap per entry, but a home directory is
        // millions of them — cheap times millions is still a wait.
        if cancelled.load(Ordering::Relaxed) {
            return None;
        }
        let (category, verdict, rule) = rules.classify(&entry.path);

        // Unknown + unremarkable files aren't findings; don't drown the user.
        if category == Category::Unknown {
            continue;
        }

        let assessment = risk::assess(&entry, verdict, now);
        let mut reasons: Vec<String> = rule
            .map(|r| vec![format!("matched rule {}: {}", r.id, r.description)])
            .unwrap_or_default();
        reasons.extend(assessment.reasons);

        findings.push(Finding {
            reclaimable: if verdict == Verdict::Protected {
                0
            } else {
                entry.size
            },
            entry,
            category,
            verdict,
            risk_score: assessment.score,
            reasons,
        });
    }

    let total_reclaimable = findings.iter().map(|f| f.reclaimable).sum::<u64>()
        + duplicate_sets.iter().map(|d| d.wasted).sum::<u64>();

    findings.sort_by_key(|f| std::cmp::Reverse(f.reclaimable));

    Some(Report {
        findings,
        duplicate_sets,
        total_reclaimable,
        files_scanned,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{scan, ScanOptions, ScanProgress};
    use std::sync::Arc;

    fn scan_dir(dir: &std::path::Path) -> Vec<FileEntry> {
        let opts = ScanOptions {
            roots: vec![dir.to_path_buf()],
            ..Default::default()
        };
        scan(&opts, Arc::new(ScanProgress::default())).unwrap()
    }

    /// Issue #47. `min_file_size` was applied inside the walk, so a file
    /// under it never reached the rules engine, the risk model, the report
    /// or `files_scanned` — the default of 1 quietly dropped every
    /// zero-byte file. The knob is about dedup and now only affects dedup.
    #[test]
    fn the_dedup_minimum_does_not_hide_files_from_the_report() {
        let dir = tempfile::tempdir().unwrap();
        let tmp = dir.path().join("tmp");
        std::fs::create_dir(&tmp).unwrap();
        std::fs::write(tmp.join("empty.log"), b"").unwrap();
        std::fs::write(tmp.join("small-a"), b"tiny").unwrap();
        std::fs::write(tmp.join("small-b"), b"tiny").unwrap();

        let rules = RulesDb::new(
            1,
            vec![crate::rules::Rule {
                id: "test-temp".into(),
                patterns: vec!["**/tmp/**".into()],
                category: Category::TempFile,
                verdict: Verdict::Review,
                description: "test".into(),
            }],
        );

        let entries = scan_dir(dir.path());
        let never = AtomicBool::new(false);

        // Every file is scanned and classified, whatever the dedup floor.
        let big_floor = ReportOptions {
            dedup_min_size: 1_000_000,
        };
        let report = build_with(entries.clone(), &rules, &big_floor, &never).unwrap();
        assert_eq!(report.files_scanned, 3);
        assert_eq!(report.findings.len(), 3);
        // ...but nothing is small enough to be staged for dedup.
        assert!(report.duplicate_sets.is_empty());

        // With the default floor the two identical files do pair up.
        let report = build_with(entries, &rules, &ReportOptions::default(), &never).unwrap();
        assert_eq!(report.files_scanned, 3);
        assert_eq!(report.duplicate_sets.len(), 1);
    }
}
