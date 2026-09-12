//! Assembles scanner + dedup + rules + risk into Findings — the single
//! structure both the CLI and the Tauri UI render.

use crate::{
    dedup, graph, risk, rules::RulesDb, Category, FileEntry, FileIdentity, Finding, Verdict,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportStage {
    BuildingImpactGraph,
    ClassifyingEntries,
    FindingDuplicates,
    PreparingFindings,
}

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
    entries: Vec<FileEntry>,
    rules: &RulesDb,
    opts: &ReportOptions,
    cancelled: &AtomicBool,
) -> Option<Report> {
    build_with_progress(entries, rules, opts, cancelled, |_| {})
}

/// [`build_with`] plus coarse stage callbacks for UI progress.
///
/// This is intentionally phase-level, not partial findings. The impact graph
/// must see every entry before it can make verdicts more cautious, and dedup
/// must see every actionable same-size file before it can say which copies are
/// redundant. A future streaming design can emit provisional findings, but it
/// needs an explicit "not actionable yet" contract rather than pretending the
/// final report exists before these stages finish.
pub fn build_with_progress<F>(
    mut entries: Vec<FileEntry>,
    rules: &RulesDb,
    opts: &ReportOptions,
    cancelled: &AtomicBool,
    mut progress: F,
) -> Option<Report>
where
    F: FnMut(ReportStage),
{
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // The graph stage. Project roots and the dependency stores they point
    // at, worked out from the same entries the rest of the pipeline sees,
    // so a `node_modules` three live projects depend on can be told apart
    // from an abandoned one.
    progress(ReportStage::BuildingImpactGraph);
    let impact = graph::ImpactGraph::from_entries_cancellable(&entries, cancelled)?;

    // Classify before dedup, not after. A file nothing will act on has no
    // business in a duplicate set — the set is an offer to keep one copy
    // and drop the rest, and dropping a driver store copy is not on offer
    // — and hashing it is time spent producing a number nobody can use.
    //
    // Classification is cheap per entry, but a home directory is millions
    // of them, so it happens once and the answer is kept.
    progress(ReportStage::ClassifyingEntries);
    let mut verdicts: Vec<Classified> = Vec::with_capacity(entries.len());
    for entry in &entries {
        if cancelled.load(Ordering::Relaxed) {
            return None;
        }
        let (category, base, rule) = rules.classify(&entry.path);
        // Evidence can only make a verdict more cautious, never less.
        let referenced_by = impact.referencing_projects(&entry.path);
        let verdict = risk::downgrade(base, referenced_by);
        verdicts.push(Classified {
            category,
            verdict,
            rule,
            referenced_by,
        });
    }

    progress(ReportStage::FindingDuplicates);
    let duplicate_sets = dedup::find_duplicates_filtered(
        &mut entries,
        |i, e| e.size >= opts.dedup_min_size && is_actionable(verdicts[i].verdict),
        cancelled,
    )?;
    let files_scanned = entries.len() as u64;

    progress(ReportStage::PreparingFindings);
    let mut findings = Vec::new();
    let mut counted_identities = HashSet::new();
    for (entry, class) in entries.into_iter().zip(verdicts) {
        if cancelled.load(Ordering::Relaxed) {
            return None;
        }

        // Unknown + unremarkable files aren't findings; don't drown the user.
        if class.category == Category::Unknown {
            continue;
        }

        let assessment = risk::assess(&entry, class.verdict, now);
        let mut reasons: Vec<String> = class
            .rule
            .map(|r| vec![format!("matched rule {}: {}", r.id, r.description)])
            .unwrap_or_default();
        if class.referenced_by > 0 {
            reasons.push(format!(
                "referenced by {} project{}",
                class.referenced_by,
                if class.referenced_by == 1 { "" } else { "s" }
            ));
        }
        reasons.extend(assessment.reasons);

        let reclaimable = finding_reclaimable(&entry, class.verdict, &mut counted_identities);

        findings.push(Finding {
            reclaimable,
            entry,
            category: class.category,
            verdict: class.verdict,
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

/// Build the provisional row the UI may show while the filesystem walk is
/// still running.
///
/// This deliberately uses only information available during the walk: the
/// matching rule and the file metadata. It does not consult the impact graph
/// or duplicate detector, so callers must present this as a preview and must
/// not make it actionable. The final report can make a verdict stricter or
/// adjust reclaimable bytes once full-disk context exists.
pub fn provisional_finding(
    entry: &FileEntry,
    rules: &RulesDb,
    counted_identities: &mut HashSet<FileIdentity>,
    now: i64,
) -> Option<Finding> {
    let (category, verdict, rule) = rules.classify(&entry.path);
    if category == Category::Unknown {
        return None;
    }

    let assessment = risk::assess(entry, verdict, now);
    let mut reasons: Vec<String> = rule
        .map(|r| vec![format!("matched rule {}: {}", r.id, r.description)])
        .unwrap_or_default();
    reasons.push("preview while scanning; final safety check still running".into());
    reasons.extend(assessment.reasons);

    Some(Finding {
        reclaimable: finding_reclaimable(entry, verdict, counted_identities),
        entry: entry.clone(),
        category,
        verdict,
        risk_score: assessment.score,
        reasons,
    })
}

/// Whether anything will ever offer to move this file.
///
/// `actions::quarantine` refuses Protected and Risky, and the UI renders
/// no action for either. One definition, used in both places it matters:
/// what counts towards the reclaimable headline, and what takes part in
/// dedup. Splitting them let Risky bytes back into the total through the
/// duplicate half after they had been taken out of the findings half.
fn is_actionable(verdict: Verdict) -> bool {
    match verdict {
        Verdict::Safe | Verdict::Review => true,
        Verdict::Risky | Verdict::Protected => false,
    }
}

fn finding_reclaimable(
    entry: &FileEntry,
    verdict: Verdict,
    counted_identities: &mut HashSet<FileIdentity>,
) -> u64 {
    // Bytes nothing will ever offer to move are not reclaimable.
    if !is_actionable(verdict) {
        return 0;
    }

    // Moving one hard-link name does not free the file's blocks. Count the
    // shared bytes once across all findings that point at the same file.
    if entry
        .identity
        .is_some_and(|identity| !counted_identities.insert(identity))
    {
        return 0;
    }

    entry.size
}

/// What the pipeline worked out about one entry before findings are built.
struct Classified<'a> {
    category: Category,
    /// After [`risk::downgrade`], not the rule's base verdict.
    verdict: Verdict,
    rule: Option<&'a crate::rules::Rule>,
    referenced_by: usize,
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

    #[test]
    fn build_reports_coarse_progress_stages() {
        let cancelled = AtomicBool::new(false);
        let entries = vec![FileEntry {
            path: std::path::PathBuf::from("/tmp/dk-scratch/cache.bin"),
            size: 4,
            modified: None,
            accessed: None,
            is_symlink: false,
            identity: None,
            hash: None,
        }];
        let mut stages = Vec::new();

        let report = build_with_progress(
            entries,
            &temp_rules(),
            &ReportOptions::default(),
            &cancelled,
            |stage| stages.push(stage),
        )
        .unwrap();

        assert_eq!(report.files_scanned, 1);
        assert_eq!(
            stages,
            vec![
                ReportStage::BuildingImpactGraph,
                ReportStage::ClassifyingEntries,
                ReportStage::FindingDuplicates,
                ReportStage::PreparingFindings,
            ]
        );
    }

    #[test]
    fn provisional_findings_use_rule_evidence_without_final_graph_context() {
        let mut counted = HashSet::new();
        let entry = FileEntry {
            path: std::path::PathBuf::from("/tmp/dk-scratch/cache.bin"),
            size: 4,
            modified: None,
            accessed: None,
            is_symlink: false,
            identity: None,
            hash: None,
        };

        let finding = provisional_finding(&entry, &temp_rules(), &mut counted, 0).unwrap();

        assert_eq!(finding.category, Category::TempFile);
        assert_eq!(finding.verdict, Verdict::Review);
        assert_eq!(finding.reclaimable, 4);
        assert!(finding
            .reasons
            .iter()
            .any(|reason| reason.contains("preview while scanning")));
    }

    #[test]
    fn provisional_findings_skip_unknown_files() {
        let mut counted = HashSet::new();
        let entry = FileEntry {
            path: std::path::PathBuf::from("/home/me/photo.jpg"),
            size: 4,
            modified: None,
            accessed: None,
            is_symlink: false,
            identity: None,
            hash: None,
        };

        assert!(provisional_finding(&entry, &temp_rules(), &mut counted, 0).is_none());
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

    /// Issue #66. Two hard-linked names point at the same file blocks, so
    /// moving one name does not free another copy's worth of space.
    #[test]
    fn hard_linked_findings_count_their_bytes_once() {
        let dir = tempfile::tempdir().unwrap();
        let tmp = dir.path().join("dk-scratch");
        std::fs::create_dir(&tmp).unwrap();
        let a = tmp.join("a.iso");
        let b = tmp.join("b.iso");
        std::fs::write(&a, b"same").unwrap();
        std::fs::hard_link(&a, &b).unwrap();

        let never = AtomicBool::new(false);
        let report = build_with(
            scan_dir(dir.path()),
            &temp_rules(),
            &ReportOptions::default(),
            &never,
        )
        .unwrap();

        assert_eq!(report.findings.len(), 2);
        assert!(report.duplicate_sets.is_empty());
        assert_eq!(report.total_reclaimable, 4);
    }

    /// A hard-linked pair plus a separate copy has two real copies on disk,
    /// not three. Count the shared finding bytes once and let dedup account
    /// for the separate copy without adding the same bytes again.
    #[test]
    fn hard_linked_findings_shared_with_a_real_copy_count_once() {
        let dir = tempfile::tempdir().unwrap();
        let tmp = dir.path().join("dk-scratch");
        std::fs::create_dir(&tmp).unwrap();
        let a = tmp.join("a.iso");
        let b = tmp.join("b.iso");
        let c = tmp.join("c.iso");
        std::fs::write(&a, b"same").unwrap();
        std::fs::hard_link(&a, &b).unwrap();
        std::fs::write(&c, b"same").unwrap();

        let never = AtomicBool::new(false);
        let report = build_with(
            scan_dir(dir.path()),
            &temp_rules(),
            &ReportOptions::default(),
            &never,
        )
        .unwrap();

        assert_eq!(report.findings.len(), 3);
        assert_eq!(report.duplicate_sets.len(), 1);
        assert_eq!(report.duplicate_sets[0].wasted, 4);
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
    /// including ones nothing will act on, so their bytes contributed
    /// `wasted` to a total the user can never do anything with.
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
        assert!(report
            .findings
            .iter()
            .all(|f| f.verdict == Verdict::Protected));
        assert!(report.duplicate_sets.is_empty());
        assert_eq!(report.total_reclaimable, 0);
    }

    /// Issue #48. The graph stage was in the pipeline diagram and not in
    /// the pipeline: `risk::downgrade` had no callers anywhere, so a
    /// `node_modules` three live projects depend on got the same verdict
    /// and the same reasons as an abandoned one.
    #[test]
    fn a_referenced_store_is_more_cautious_than_an_abandoned_one() {
        let dir = tempfile::tempdir().unwrap();
        let live = dir.path().join("live");
        let dead = dir.path().join("dead");
        std::fs::create_dir_all(live.join("node_modules/react")).unwrap();
        std::fs::create_dir_all(dead.join("node_modules/react")).unwrap();
        std::fs::write(live.join("package.json"), b"{}").unwrap();
        std::fs::write(live.join("node_modules/react/index.js"), b"live").unwrap();
        std::fs::write(dead.join("node_modules/react/index.js"), b"dead").unwrap();

        let rules = RulesDb::new(
            1,
            vec![crate::rules::Rule {
                id: "test-node-modules".into(),
                patterns: vec!["**/node_modules/**".into()],
                category: Category::BuildArtifact,
                verdict: Verdict::Review,
                description: "test".into(),
            }],
        );

        let never = AtomicBool::new(false);
        let report = build_with(
            scan_dir(dir.path()),
            &rules,
            &ReportOptions::default(),
            &never,
        )
        .unwrap();

        // Compared as paths, not as substrings of one: Windows separates
        // with `\`, so "live/node_modules" matches nothing there.
        let find = |root: &std::path::Path| {
            let wanted = root.join("node_modules").join("react").join("index.js");
            report
                .findings
                .iter()
                .find(|f| f.entry.path == wanted)
                .unwrap_or_else(|| panic!("no finding for {}", wanted.display()))
                .clone()
        };

        let referenced = find(&live);
        assert_eq!(referenced.verdict, Verdict::Risky);
        assert!(referenced
            .reasons
            .iter()
            .any(|r| r == "referenced by 1 project"));
        // Nothing offers to move a risky file, so its bytes are not on
        // offer either.
        assert_eq!(referenced.reclaimable, 0);

        let abandoned = find(&dead);
        assert_eq!(abandoned.verdict, Verdict::Review);
        assert!(!abandoned
            .reasons
            .iter()
            .any(|r| r.starts_with("referenced by")));
        assert_eq!(abandoned.reclaimable, 4);
    }

    /// The graph stage is a full pass over the entries in front of every
    /// other cancellation check, so it has to answer a cancel itself —
    /// otherwise it just moves the dead spot the last round removed.
    #[test]
    fn a_cancel_during_the_graph_stage_stops_the_report() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"x").unwrap();

        let cancelled = AtomicBool::new(true);
        assert!(build_with(
            scan_dir(dir.path()),
            &temp_rules(),
            &ReportOptions::default(),
            &cancelled,
        )
        .is_none());
    }

    /// Risky bytes were taken out of the findings half of the headline and
    /// then walked back in through the duplicate half: `total_reclaimable`
    /// treats a zero-reclaimable finding as "nobody counted this yet", so
    /// a duplicate set of Risky copies added its full `wasted`.
    #[test]
    fn risky_duplicates_do_not_return_to_the_headline() {
        let dir = tempfile::tempdir().unwrap();
        for project in ["live1", "live2"] {
            let root = dir.path().join(project);
            std::fs::create_dir_all(root.join("node_modules/react")).unwrap();
            std::fs::write(root.join("package.json"), b"{}").unwrap();
            std::fs::write(root.join("node_modules/react/index.js"), b"identical").unwrap();
        }

        let rules = RulesDb::new(
            1,
            vec![crate::rules::Rule {
                id: "test-node-modules".into(),
                patterns: vec!["**/node_modules/**".into()],
                category: Category::BuildArtifact,
                verdict: Verdict::Review,
                description: "test".into(),
            }],
        );

        let never = AtomicBool::new(false);
        let report = build_with(
            scan_dir(dir.path()),
            &rules,
            &ReportOptions::default(),
            &never,
        )
        .unwrap();

        let risky: Vec<&Finding> = report
            .findings
            .iter()
            .filter(|f| f.verdict == Verdict::Risky)
            .collect();
        assert_eq!(risky.len(), 2, "both node_modules copies are referenced");
        assert!(risky.iter().all(|f| f.reclaimable == 0));

        // Nothing the app refuses to act on may reach a duplicate set...
        for set in &report.duplicate_sets {
            for path in &set.paths {
                assert!(
                    !path.to_string_lossy().contains("node_modules"),
                    "{} is risky and should not be offered as a duplicate",
                    path.display()
                );
            }
        }
        // ...so the headline is the one duplicate pair the user really
        // can act on — the two identical `package.json` files, which are
        // unclassified and so not findings, but are still two copies of
        // the same two bytes. Before the fix the risky pair added its own
        // 9 bytes on top, promising space the app refuses to free.
        assert_eq!(report.duplicate_sets.len(), 1);
        assert_eq!(report.total_reclaimable, 2);
    }
}
