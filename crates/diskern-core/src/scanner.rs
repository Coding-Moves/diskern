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
    /// Directories never walked, given as paths rather than globs
    /// (e.g. "/proc", "C:\\Windows\\WinSxS"). Matched on whole path
    /// components after the same normalization the rules database uses,
    /// so case and separator style don't have to line up with the root.
    pub excludes: Vec<String>,
    pub follow_symlinks: bool, // default false — symlink loops are real
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            roots: vec![],
            excludes: default_excludes(),
            follow_symlinks: false,
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
    // Normalized once, not per directory: `process_read_dir` runs on every
    // directory the walk opens, and the exclude list never changes.
    let excludes: Vec<String> = opts.excludes.iter().map(|e| normalize_exclude(e)).collect();

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

/// Same shape the rules database matches in: lowercased, `/`-separated,
/// no trailing separator. An exclude written `C:\\Windows\\WinSxS` has to
/// match a root the user typed as `c:\\windows\\winsxs`, and
/// `rules::classify` already normalizes for exactly that reason.
fn normalize_exclude(exclude: &str) -> String {
    let normalized = exclude.replace('\\', "/").to_lowercase();
    let trimmed = normalized.trim_end_matches('/');
    // "/" itself trims to empty; keep it as the root rather than a prefix
    // that matches every path.
    if trimmed.is_empty() {
        normalized
    } else {
        trimmed.to_string()
    }
}

/// True when `path` *is* an excluded directory or sits inside one.
///
/// Compared on whole path components. A raw `starts_with` on the string
/// made `/run` exclude `/runtime-data` as well, because "/run" is a prefix
/// of "/runtime-data" in characters but not in directories.
fn is_excluded(path: &Path, excludes: &[String]) -> bool {
    let p = crate::rules::normalize(path);
    let p = p.trim_end_matches('/');
    excludes.iter().any(|ex| {
        p == ex
            || p.strip_prefix(ex.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
    })
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
    fn excludes_match_whole_components_not_characters() {
        let excludes = ["/run".to_string()];
        assert!(is_excluded(Path::new("/run"), &excludes));
        assert!(is_excluded(Path::new("/run/user/1000/x"), &excludes));
        // The bug: a character-prefix compare skipped this too.
        assert!(!is_excluded(Path::new("/runtime-data/x"), &excludes));
        assert!(!is_excluded(Path::new("/runner"), &excludes));
    }

    #[test]
    fn excludes_survive_a_differently_cased_or_separated_root() {
        let excludes = [normalize_exclude("C:\\Windows\\WinSxS")];
        assert!(is_excluded(
            Path::new("c:/windows/winsxs/component/x.dll"),
            &excludes
        ));
        assert!(is_excluded(
            Path::new("C:\\Windows\\WinSxS\\x.dll"),
            &excludes
        ));
        assert!(!is_excluded(
            Path::new("c:/windows/winsxs-backup/x.dll"),
            &excludes
        ));
    }

    #[test]
    fn a_trailing_separator_on_an_exclude_changes_nothing() {
        let excludes = [normalize_exclude("/proc/")];
        assert!(is_excluded(Path::new("/proc/1/maps"), &excludes));
        assert!(!is_excluded(Path::new("/process-data/x"), &excludes));
    }

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
