//! Optional AI narration layer. NARRATES facts, never decides.
//!
//! Contract: the provider receives structured findings the engine already
//! computed and returns prose. It cannot change verdicts, scores, or
//! trigger actions. The app must work fully with `AiProvider = None`.
//!
//! Planned implementations (both BYO/free for users):
//! - `AnthropicProvider` / `OpenAiProvider`: user pastes their own API key.
//! - `OllamaProvider`: local models via http://localhost:11434.

use crate::Finding;

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
            "Found {} items totalling {:.1} GB reclaimable. Top reasons: {}",
            findings.len(),
            total as f64 / 1e9,
            findings
                .iter()
                .take(3)
                .flat_map(|f| f.reasons.first().cloned())
                .collect::<Vec<_>>()
                .join("; ")
        ))
    }
}
