//! Parallel, read-only filesystem walk.
//!
//! Strategy: walk with `jwalk` (parallel, ordered), collect metadata only.
//! Hashing is NOT done here — see [`crate::dedup`], which hashes only files
//! whose sizes collide. On a typical disk that skips >95% of hash work.

use crate::{FileEntry, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanOptions {
    pub roots: Vec<PathBuf>,
    /// Glob-style excludes (e.g. "/proc", "C:\\Windows\\WinSxS").
    pub excludes: Vec<String>,
    pub follow_symlinks: bool, // default false — symlink loops are real
    pub min_file_size: u64,    // skip tiny files for dedup purposes
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            roots: vec![],
            excludes: default_excludes(),
            follow_symlinks: false,
            min_file_size: 1,
        }
    }
}

/// Paths we never scan. Grows per-platform; keep in sync with rules db.
fn default_excludes() -> Vec<String> {
    let mut v = vec![];
    #[cfg(target_os = "linux")]
    v.extend(["/proc", "/sys", "/dev", "/run"].map(String::from));
    #[cfg(target_os = "windows")]
    v.extend(["C:\\Windows\\WinSxS", "C:\\Windows\\System32"].map(String::from));
    #[cfg(target_os = "macos")]
    v.extend(["/System", "/private/var/db"].map(String::from));
    v
}

/// Live progress, shared with the UI via atomics (cheap to poll).
#[derive(Debug, Default)]
pub struct ScanProgress {
    pub files_seen: AtomicU64,
    pub bytes_seen: AtomicU64,
    pub cancelled: AtomicBool,
}

impl ScanProgress {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

/// Walk the roots and return every file entry. Read-only.
///
/// TODO(next): stream entries through a channel instead of collecting,
/// so the UI can render results while the scan runs.
pub fn scan(opts: &ScanOptions, progress: Arc<ScanProgress>) -> Result<Vec<FileEntry>> {
    let mut out = Vec::new();

    for root in &opts.roots {
        if progress.cancelled.load(Ordering::Relaxed) {
            return Err(crate::GenomeError::Cancelled);
        }
        walk_root(root, opts, &progress, &mut out)?;
    }
    Ok(out)
}

fn walk_root(
    root: &Path,
    opts: &ScanOptions,
    progress: &ScanProgress,
    out: &mut Vec<FileEntry>,
) -> Result<()> {
    let excludes = opts.excludes.clone();

    let walker = jwalk::WalkDir::new(root)
        .follow_links(opts.follow_symlinks)
        .skip_hidden(false)
        .process_read_dir(move |_depth, _path, _state, children| {
            children.retain(|entry| {
                entry
                    .as_ref()
                    .map(|e| !is_excluded(&e.path(), &excludes))
                    .unwrap_or(true)
            });
        });

    for entry in walker {
        if progress.cancelled.load(Ordering::Relaxed) {
            return Err(crate::GenomeError::Cancelled);
        }
        let Ok(entry) = entry else { continue }; // permission errors: skip, don't die
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let size = meta.len();
        if size < opts.min_file_size {
            continue;
        }

        progress.files_seen.fetch_add(1, Ordering::Relaxed);
        progress.bytes_seen.fetch_add(size, Ordering::Relaxed);

        out.push(FileEntry {
            path: entry.path(),
            size,
            modified: meta.modified().ok().and_then(to_epoch),
            accessed: meta.accessed().ok().and_then(to_epoch),
            is_symlink: entry.path_is_symlink(),
            hash: None,
        });
    }
    Ok(())
}

fn is_excluded(path: &Path, excludes: &[String]) -> bool {
    let p = path.to_string_lossy();
    excludes.iter().any(|ex| p.starts_with(ex.as_str()))
}

fn to_epoch(t: std::time::SystemTime) -> Option<i64> {
    t.duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_a_temp_tree() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"hello").unwrap();
        std::fs::write(dir.path().join("b.txt"), b"world!").unwrap();

        let opts = ScanOptions {
            roots: vec![dir.path().to_path_buf()],
            ..Default::default()
        };
        let entries = scan(&opts, Arc::new(ScanProgress::default())).unwrap();
        assert_eq!(entries.len(), 2);
    }
}
