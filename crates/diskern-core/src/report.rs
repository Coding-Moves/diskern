//! Assembles scanner + dedup + rules + risk into Findings — the single
//! structure both the CLI and the Tauri UI render.

use crate::{dedup, risk, rules::RulesDb, Category, FileEntry, Finding, Verdict};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
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

    // Classify before dedup, not after. A protected file has no business
    // in a duplicate set — the set is an offer to keep one copy and drop
    // the rest, and dropping a driver store copy is not on offer — and
    // hashing it is time spent producing a number nobody can act on.
    //
    // Classification is cheap per entry, but a home directory is millions
    // of them, so it happens once and the answer is kept.
    let mut verdicts: Vec<(Category, Verdict, Option<&crate::rules::Rule>)> =
        Vec::with_capacity(entries.len());
    for entry in &entries {
        if cancelled.load(Ordering::Relaxed) {
            return None;
        }
        verdicts.push(rules.classify(&entry.path));
    }

    let duplicate_sets = dedup::find_duplicates_filtered(
        &mut entries,
        |i, e| e.size >= opts.dedup_min_size && verdicts[i].1 != Verdict::Protected,
        cancelled,
    )?;
    let files_scanned = entries.len() as u64;

    let mut findings = Vec::new();
    for (entry, (category, verdict, rule)) in entries.into_iter().zip(verdicts) {
        if cancelled.load(Ordering::Relaxed) {
            return None;
        }

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

    let total_reclaimable = total_reclaimable(&findings, &duplicate_sets);

    findings.sort_by_key(|f| std::cmp::Reverse(f.reclaimable));

    Some(Report {
        findings,
        duplicate_sets,
        total_reclaimable,
        files_scanned,
    })
}

/// The two halves of the report overlap, so they can't just be added.
///
/// A finding offers the file's own bytes. A duplicate set offers the
/// copies beyond the first. A file is often both: two identical 1 GB
/// installers under `/tmp` are two findings at 1 GB each *and* a duplicate
/// set with 1 GB wasted, and summing those said 3 GB when 2 GB is
/// everything there is.
///
/// So findings are counted in full, and a duplicate set adds only the
/// redundant copies nobody has counted yet.
fn total_reclaimable(findings: &[Finding], duplicate_sets: &[dedup::DuplicateSet]) -> u64 {
    let counted: HashSet<&std::path::Path> = findings
        .iter()
        .filter(|f| f.reclaimable > 0)
        .map(|f| f.entry.path.as_path())
        .collect();

    let from_findings: u64 = findings.iter().map(|f| f.reclaimable).sum();
    let from_duplicates: u64 = duplicate_sets
        .iter()
        .map(|set| {
            // One copy always stays; that is what makes the rest redundant.
            let redundant = set.paths.len().saturating_sub(1);
            let already = set
                .paths
                .iter()
                .filter(|p| counted.contains(p.as_path()))
                .count();
            set.size * redundant.saturating_sub(already) as u64
        })
        .sum();

    from_findings + from_duplicates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{scan, ScanOptions, ScanProgress};
    use std::sync::Arc;

    fn temp_rules() -> RulesDb {
        RulesDb::new(
            1,
            vec![
                crate::rules::Rule {
                    id: "test-protected".into(),
                    patterns: vec!["**/dk-sys/**".into()],
                    category: Category::SystemCritical,
                    verdict: Verdict::Protected,
                    description: "test".into(),
                },
                crate::rules::Rule {
                    id: "test-temp".into(),
                    patterns: vec!["**/dk-scratch/**".into()],
                    category: Category::TempFile,
                    verdict: Verdict::Review,
                    description: "test".into(),
                },
            ],
        )
    }

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
        let tmp = dir.path().join("dk-scratch");
        std::fs::create_dir(&tmp).unwrap();
        std::fs::write(tmp.join("empty.log"), b"").unwrap();
        std::fs::write(tmp.join("small-a"), b"tiny").unwrap();
        std::fs::write(tmp.join("small-b"), b"tiny").unwrap();

        let rules = temp_rules();
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

    /// Issue #44. Two identical files under `/tmp` are two findings *and*
    /// one duplicate set. Adding both halves counted the same bytes twice:
    /// 4 + 4 + 4 = 12 where 8 is everything on the disk.
    #[test]
    fn duplicated_findings_are_not_counted_twice() {
        let dir = tempfile::tempdir().unwrap();
        let tmp = dir.path().join("dk-scratch");
        std::fs::create_dir(&tmp).unwrap();
        std::fs::write(tmp.join("a.iso"), b"same").unwrap();
        std::fs::write(tmp.join("b.iso"), b"same").unwrap();

        let never = AtomicBool::new(false);
        let report = build_with(
            scan_dir(dir.path()),
            &temp_rules(),
            &ReportOptions::default(),
            &never,
        )
        .unwrap();

        assert_eq!(report.findings.len(), 2);
        assert_eq!(report.duplicate_sets.len(), 1);
        assert_eq!(report.duplicate_sets[0].wasted, 4);
        // Both copies are already offered as findings, so the duplicate
        // set adds nothing on top of them.
        assert_eq!(report.total_reclaimable, 8);
    }

    /// A duplicate whose copies are not findings still contributes — the
    /// fix must not swing the other way and undercount.
    #[test]
    fn duplicates_outside_the_findings_still_count() {
        let dir = tempfile::tempdir().unwrap();
        let docs = dir.path().join("docs");
        std::fs::create_dir(&docs).unwrap();
        std::fs::write(docs.join("a.txt"), b"same").unwrap();
        std::fs::write(docs.join("b.txt"), b"same").unwrap();

        let never = AtomicBool::new(false);
        let report = build_with(
            scan_dir(dir.path()),
            &temp_rules(),
            &ReportOptions::default(),
            &never,
        )
        .unwrap();

        assert!(report.findings.is_empty()); // unknown category, dropped
        assert_eq!(report.duplicate_sets.len(), 1);
        assert_eq!(report.total_reclaimable, 4); // one redundant copy
    }

    /// Half in, half out: one copy is an actionable finding, the other is
    /// unclassified user data. Acting on everything frees one copy's worth.
    #[test]
    fn a_duplicate_shared_with_a_finding_counts_once() {
        let dir = tempfile::tempdir().unwrap();
        let tmp = dir.path().join("dk-scratch");
        let docs = dir.path().join("docs");
        std::fs::create_dir(&tmp).unwrap();
        std::fs::create_dir(&docs).unwrap();
        std::fs::write(tmp.join("a.iso"), b"same").unwrap();
        std::fs::write(docs.join("keep.iso"), b"same").unwrap();

        let never = AtomicBool::new(false);
        let report = build_with(
            scan_dir(dir.path()),
            &temp_rules(),
            &ReportOptions::default(),
            &never,
        )
        .unwrap();

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.duplicate_sets.len(), 1);
        assert_eq!(report.total_reclaimable, 4);
    }

    /// The other half of #44: `find_duplicates` ran over every entry,
    /// including protected ones, so system files contributed `wasted`
    /// bytes to a total the user is never allowed to act on.
    #[test]
    fn protected_files_form_no_duplicate_sets() {
        let dir = tempfile::tempdir().unwrap();
        let sys = dir.path().join("dk-sys");
        std::fs::create_dir(&sys).unwrap();
        std::fs::write(sys.join("a.dll"), b"same").unwrap();
        std::fs::write(sys.join("b.dll"), b"same").unwrap();

        let never = AtomicBool::new(false);
        let report = build_with(
            scan_dir(dir.path()),
            &temp_rules(),
            &ReportOptions::default(),
            &never,
        )
        .unwrap();

        assert_eq!(report.findings.len(), 2);
        assert!(report.findings.iter().all(|f| f.verdict == Verdict::Protected));
        assert!(report.duplicate_sets.is_empty());
        assert_eq!(report.total_reclaimable, 0);
    }
}
