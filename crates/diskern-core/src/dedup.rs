//! Duplicate detection: size-first staging, then parallel BLAKE3.
//!
//! 1. Group by size — unique sizes cannot be duplicates, skip them.
//! 2. Hash the survivors in parallel (rayon + blake3).
//! 3. Group by hash — groups of 2+ are duplicate sets.

use crate::FileEntry;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateSet {
    pub hash: String,
    pub size: u64,
    pub paths: Vec<std::path::PathBuf>,
    /// Bytes reclaimable = size * (copies - 1).
    pub wasted: u64,
}

/// Find duplicate sets. Mutates entries in-place to record computed hashes.
pub fn find_duplicates(entries: &mut [FileEntry]) -> Vec<DuplicateSet> {
    static NEVER: AtomicBool = AtomicBool::new(false);
    find_duplicates_cancellable(entries, &NEVER)
        .expect("a run that cannot be cancelled cannot stop early")
}

/// [`find_duplicates`], abandoned as soon as `cancelled` is set. `None` means
/// it stopped early and the result is incomplete.
///
/// Hashing is where a scan of a real disk spends most of its time, so a
/// cancellation that can't reach stage 2 isn't a cancellation — the walk
/// finishing just means the user waits for the expensive half with no way
/// out.
pub fn find_duplicates_cancellable(
    entries: &mut [FileEntry],
    cancelled: &AtomicBool,
) -> Option<Vec<DuplicateSet>> {
    find_duplicates_filtered(entries, |_| true, cancelled)
}

/// [`find_duplicates_cancellable`], restricted to the entries `eligible`
/// accepts.
///
/// Duplicate sets are an *offer*: "you are storing this three times, keep
/// one". An entry the user is never allowed to act on doesn't belong in
/// that offer, and hashing it is work spent to produce a number nobody can
/// use. `eligible` is where the caller says which entries those are; it is
/// called exactly once per entry, before any hashing.
pub fn find_duplicates_filtered<F>(
    entries: &mut [FileEntry],
    eligible: F,
    cancelled: &AtomicBool,
) -> Option<Vec<DuplicateSet>>
where
    F: Fn(&FileEntry) -> bool,
{
    let keep: Vec<bool> = entries.iter().map(eligible).collect();

    // Stage 1: bucket by size.
    let mut by_size: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        if keep[i] {
            by_size.entry(e.size).or_default().push(i);
        }
    }
    let candidates: Vec<usize> = by_size
        .into_values()
        .filter(|v| v.len() > 1)
        .flatten()
        .collect();

    // Stage 2: hash candidates in parallel. The check is inside the closure,
    // not around the whole stage: rayon still visits every remaining item,
    // but each one returns immediately instead of hashing, so an in-flight
    // scan drains in about the time the longest single file takes.
    let hashes: Vec<(usize, Option<String>)> = candidates
        .par_iter()
        .map(|&i| {
            if cancelled.load(Ordering::Relaxed) {
                return (i, None);
            }
            (i, hash_file(&entries[i].path))
        })
        .collect();
    if cancelled.load(Ordering::Relaxed) {
        return None;
    }
    for (i, h) in hashes {
        entries[i].hash = h;
    }

    // Stage 3: bucket by hash. Only stage-1 survivors carry one, but the
    // eligibility check is repeated here so a caller reusing entries that
    // already hold hashes from an earlier run can't smuggle them back in.
    let mut by_hash: HashMap<&str, Vec<&FileEntry>> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        match &e.hash {
            Some(h) if keep[i] => by_hash.entry(h).or_default().push(e),
            _ => continue,
        }
    }

    let mut sets: Vec<DuplicateSet> = by_hash
        .into_iter()
        .filter(|(_, v)| v.len() > 1)
        .map(|(hash, group)| {
            let size = group[0].size;
            DuplicateSet {
                hash: hash.to_string(),
                size,
                wasted: size * (group.len() as u64 - 1),
                paths: group.iter().map(|e| e.path.clone()).collect(),
            }
        })
        .collect();

    sets.sort_by_key(|s| std::cmp::Reverse(s.wasted)); // biggest wins first
    Some(sets)
}

fn hash_file(path: &std::path::Path) -> Option<String> {
    // blake3 memory-maps large files internally via update_mmap_rayon.
    let mut hasher = blake3::Hasher::new();
    hasher.update_mmap_rayon(path).ok()?;
    Some(hasher.finalize().to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{scan, ScanOptions, ScanProgress};
    use std::sync::Arc;

    #[test]
    fn detects_identical_content() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a"), b"same-bytes").unwrap();
        std::fs::write(dir.path().join("b"), b"same-bytes").unwrap();
        std::fs::write(dir.path().join("c"), b"different!").unwrap();

        let opts = ScanOptions {
            roots: vec![dir.path().to_path_buf()],
            ..Default::default()
        };
        let mut entries = scan(&opts, Arc::new(ScanProgress::default())).unwrap();
        let sets = find_duplicates(&mut entries);

        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].paths.len(), 2);
        assert_eq!(sets[0].wasted, 10);
    }

    #[test]
    fn ineligible_entries_form_no_sets() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a"), b"same-bytes").unwrap();
        std::fs::write(dir.path().join("b"), b"same-bytes").unwrap();
        std::fs::write(dir.path().join("keep-a"), b"other-byte").unwrap();
        std::fs::write(dir.path().join("keep-b"), b"other-byte").unwrap();

        let opts = ScanOptions {
            roots: vec![dir.path().to_path_buf()],
            ..Default::default()
        };
        let mut entries = scan(&opts, Arc::new(ScanProgress::default())).unwrap();
        let never = AtomicBool::new(false);
        let sets = find_duplicates_filtered(
            &mut entries,
            |e| e.path.file_name().is_some_and(|n| n.to_string_lossy().starts_with("keep")),
            &never,
        )
        .unwrap();

        assert_eq!(sets.len(), 1);
        assert!(sets[0].paths.iter().all(|p| p
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("keep"))));
        // Excluded entries are never hashed, so they cost nothing either.
        for entry in &entries {
            if entry.path.file_name().is_some_and(|n| n == "a") {
                assert!(entry.hash.is_none());
            }
        }
    }

    #[test]
    fn stops_when_cancelled() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a"), b"same-bytes").unwrap();
        std::fs::write(dir.path().join("b"), b"same-bytes").unwrap();

        let opts = ScanOptions {
            roots: vec![dir.path().to_path_buf()],
            ..Default::default()
        };
        let mut entries = scan(&opts, Arc::new(ScanProgress::default())).unwrap();

        let cancelled = AtomicBool::new(true);
        assert!(find_duplicates_cancellable(&mut entries, &cancelled).is_none());
    }
}
