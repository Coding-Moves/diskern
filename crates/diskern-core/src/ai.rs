//! Optional AI narration layer. NARRATES facts, never decides.
//!
//! Contract: the provider receives structured findings the engine already
//! computed and returns prose. It cannot change verdicts, scores, or
//! trigger actions. The app must work fully with `AiProvider = None`.
//!
//! Planned implementations (both BYO/free for users):
//! - `AnthropicProvider` / `OpenAiProvider`: user pastes their own API key.
//! - `OllamaProvider`: local models via http://localhost:11434.

use crate::{human_bytes, Finding};

pub trait AiProvider: Send + Sync {
    /// Turn a set of findings into a short, plain-language explanation.
    /// Implementations should instruct the model to ONLY restate the
    /// provided evidence — never to speculate about unlisted files.
    fn narrate(&self, findings: &[Finding]) -> Result<String, AiError>;
}

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("provider not configured")]
    NotConfigured,
    #[error("request failed: {0}")]
    Request(String),
}

/// Always-available fallback: deterministic template narration.
pub struct TemplateNarrator;

impl AiProvider for TemplateNarrator {
    fn narrate(&self, findings: &[Finding]) -> Result<String, AiError> {
        let total: u64 = findings.iter().map(|f| f.reclaimable).sum();
        Ok(format!(
            "Found {} items totalling {} reclaimable. Top reasons: {}",
            findings.len(),
            human_bytes(total),
            findings
                .iter()
                .take(3)
                .flat_map(|f| f.reasons.first().cloned())
                .collect::<Vec<_>>()
                .join("; ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Category, FileEntry, Verdict};
    use std::path::PathBuf;

    fn sample_finding(path: &str, reclaimable: u64, reason: &str) -> Finding {
        Finding {
            entry: FileEntry {
                path: PathBuf::from(path),
                size: reclaimable,
                modified: None,
                accessed: None,
                is_symlink: false,
                identity: None,
                hash: None,
            },
            category: Category::BrowserCache,
            verdict: Verdict::Safe,
            risk_score: 0.1,
            reasons: vec![reason.to_string()],
            reclaimable,
        }
    }

    #[test]
    fn template_narrator_narrates_facts_and_formats_bytes() {
        let findings = vec![
            sample_finding("/cache/a", 50_000_000, "chrome cache expired"),
            sample_finding("/cache/b", 30_000_000, "firefox cache expired"),
            sample_finding("/cache/c", 20_000_000, "safari cache expired"),
            sample_finding("/cache/d", 10_000_000, "edge cache expired"),
        ];

        let narrator = TemplateNarrator;
        let narrative = narrator.narrate(&findings).unwrap();

        assert_eq!(
            narrative,
            "Found 4 items totalling 110.0 MB reclaimable. Top reasons: chrome cache expired; firefox cache expired; safari cache expired"
        );
    }

    #[test]
    fn template_narrator_empty_findings() {
        let narrator = TemplateNarrator;
        let narrative = narrator.narrate(&[]).unwrap();
        assert_eq!(
            narrative,
            "Found 0 items totalling 0 B reclaimable. Top reasons: "
        );
    }
}
