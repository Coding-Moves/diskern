//! Evidence-based risk scoring. Local, deterministic, explainable.
//!
//! The score is informational; the Verdict is the gate. Risk evidence can
//! only make a verdict MORE cautious, never less.

use crate::{FileEntry, Verdict};

/// Days since last access, if the platform reports atime.
fn days_since_access(entry: &FileEntry, now: i64) -> Option<i64> {
    entry.accessed.map(|a| (now - a) / 86_400)
}

pub struct RiskAssessment {
    pub score: f64, // 0.0 = definitely keep, 1.0 = definitely removable
    pub reasons: Vec<String>,
}

/// Naive first-pass heuristic. TODO: Bayesian combination once we have
/// reference-graph evidence (crate::graph) feeding in.
pub fn assess(entry: &FileEntry, base_verdict: Verdict, now: i64) -> RiskAssessment {
    let mut score: f64 = match base_verdict {
        Verdict::Safe => 0.9,
        Verdict::Review => 0.5,
        Verdict::Risky => 0.2,
        Verdict::Protected => 0.0,
    };
    let mut reasons = vec![];

    if let Some(days) = days_since_access(entry, now) {
        if days > 180 {
            score = (score + 0.05).min(1.0);
            reasons.push(format!("not accessed in {days} days"));
        } else if days < 7 {
            score = (score - 0.2).max(0.0);
            reasons.push(format!("accessed {days} day(s) ago — recently in use"));
        }
    }

    RiskAssessment { score, reasons }
}

/// Verdicts can only be downgraded (made more cautious) by evidence.
pub fn downgrade(verdict: Verdict, referenced_by: usize) -> Verdict {
    match (verdict, referenced_by) {
        (Verdict::Protected, _) => Verdict::Protected,
        (v, 0) => v,
        (Verdict::Safe, _) => Verdict::Review,
        (Verdict::Review, _) => Verdict::Risky,
        (v, _) => v,
    }
}
