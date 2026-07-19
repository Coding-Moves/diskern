//! # diskern-core
//!
//! The Diskern engine. Everything here is:
//! - **Read-only by default.** Nothing in this crate deletes, moves, or
//!   modifies user files except through [`actions`], which only ever
//!   quarantines (moves to a review folder) — never hard-deletes.
//! - **Deterministic.** Safety classification comes from [`rules`] +
//!   [`risk`], never from a network call or a model. The optional `ai`
//!   feature adds *narration only*.
//! - **UI-agnostic.** Consumed by the CLI, the Tauri app, and tests alike.
//!
//! ## Pipeline
//!
//! ```text
//! scanner ──► index ──► dedup ──► graph ──► rules + risk ──► report
//!                                                   │
//!                                        (optional) ai narration
//! ```

pub mod scanner;
pub mod dedup;
pub mod rules;
pub mod risk;
pub mod graph;
pub mod actions;
pub mod report;

#[cfg(feature = "ai")]
pub mod ai;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A single filesystem entry discovered by the scanner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: PathBuf,
    pub size: u64,
    /// Seconds since epoch; None if the platform couldn't report it.
    pub modified: Option<i64>,
    pub accessed: Option<i64>,
    pub is_symlink: bool,
    /// BLAKE3 hash — only computed for size-collision candidates.
    pub hash: Option<String>,
}

/// What kind of thing the rules engine believes this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    BrowserCache,
    BuildArtifact,   // target/, node_modules/, __pycache__/ ...
    PackageManagerCache,
    TempFile,
    Log,
    Installer,
    DuplicateFile,
    EmptyDirectory,
    SystemCritical,  // driver stores, OS components — never touch
    Unknown,
}

/// Deterministic risk verdict. This is the ONLY thing allowed to gate an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Regenerable, unreferenced, well-known category. Quarantine freely.
    Safe,
    /// Probably fine, but show the user the evidence first.
    Review,
    /// Referenced by something, or unknown provenance. Warn loudly.
    Risky,
    /// System-critical. The UI must not even offer an action.
    Protected,
}

/// One finding = one row the user sees.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub entry: FileEntry,
    pub category: Category,
    pub verdict: Verdict,
    /// 0.0..=1.0 — Bayesian-ish score from `risk`; informational, never the gate.
    pub risk_score: f64,
    /// Human-readable, evidence-based reasons ("matched rule chrome-cache",
    /// "referenced by 3 lockfiles"). The AI layer may rephrase these; it may
    /// not invent new ones.
    pub reasons: Vec<String>,
    /// Bytes reclaimable if acted on.
    pub reclaimable: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum GenomeError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("scan cancelled")]
    Cancelled,
    #[error("rules database error: {0}")]
    Rules(String),
}

pub type Result<T> = std::result::Result<T, GenomeError>;
