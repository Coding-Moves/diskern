//! Assembles scanner + dedup + rules + risk into Findings — the single
//! structure both the CLI and the Tauri UI render.

use crate::{dedup, risk, rules::RulesDb, Category, FileEntry, Finding, Verdict};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub duplicate_sets: Vec<dedup::DuplicateSet>,
    pub total_reclaimable: u64,
    pub files_scanned: u64,
}

pub fn build(mut entries: Vec<FileEntry>, rules: &RulesDb) -> Report {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let duplicate_sets = dedup::find_duplicates(&mut entries);
    let files_scanned = entries.len() as u64;

    let mut findings = Vec::new();
    for entry in entries {
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
            reclaimable: if verdict == Verdict::Protected { 0 } else { entry.size },
            entry,
            category,
            verdict,
            risk_score: assessment.score,
            reasons,
        });
    }

    let total_reclaimable = findings.iter().map(|f| f.reclaimable).sum::<u64>()
        + duplicate_sets.iter().map(|d| d.wasted).sum::<u64>();

    findings.sort_by(|a, b| b.reclaimable.cmp(&a.reclaimable));

    Report {
        findings,
        duplicate_sets,
        total_reclaimable,
        files_scanned,
    }
}
