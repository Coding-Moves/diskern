//! The ONLY module allowed to modify the filesystem — and it only moves.
//!
//! Design rule: Diskern never hard-deletes. Files are moved to a
//! quarantine directory with a manifest recording original locations, so
//! every action is reversible until the user explicitly purges quarantine.
//!
//! The manifest is what makes "reversible" outlive the process. It is a
//! JSON Lines file in the quarantine directory: one [`QuarantineRecord`]
//! per line, appended as files arrive, rewritten when one leaves. The
//! quarantine filenames cannot stand in for it — flattening a path maps
//! `/` and `_` onto the same character, so `1700000000_home_user_.cache_x`
//! has no single original it could be read back to.
//!
//! [`purge`] is the one operation here that deletes, and it deletes only
//! what the manifest says this crate put there.

use crate::{GenomeError, Result, Verdict};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// Quarantine's record of itself, inside the quarantine directory.
pub const MANIFEST_NAME: &str = "manifest.jsonl";

/// Longest flattened name we will build. Filenames are capped at 255
/// bytes on every filesystem Diskern targets, and a deep path flattens
/// past that easily; the tail is kept because that is the half that
/// distinguishes two files.
const MAX_FLAT_LEN: usize = 180;

/// Serializes every manifest access in this process.
///
/// `restore_from_manifest` and `purge` are read-modify-write: they read
/// the whole manifest and later rewrite it from what they read. A record
/// appended in between was erased by that rewrite, leaving the file in
/// quarantine with nothing recording where it belongs — invisible to
/// `list`, unrestorable, and not even removed by a later `purge`.
///
/// The app makes that reachable: `quarantine_finding`, `restore_quarantined`
/// and `purge_quarantine` each run on their own blocking task, and nothing
/// in the UI stops a user quarantining one row while another row's restore
/// is still in flight.
///
/// One lock for the process rather than one per directory: these
/// operations are rare, short, and a second quarantine directory in one
/// process isn't a thing that happens.
static MANIFEST: Mutex<()> = Mutex::new(());

/// A poisoned lock means an earlier holder panicked mid-operation. The
/// manifest on disk is either the old one or the new one — `write_manifest`
/// renames a complete file into place — so there is no torn state to
/// protect, and refusing to unlock would break quarantine for the rest of
/// the session.
fn manifest_lock() -> MutexGuard<'static, ()> {
    MANIFEST
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantineRecord {
    pub original: PathBuf,
    pub quarantined_to: PathBuf,
    pub at_epoch: i64,
}

/// Move a file into quarantine. Refuses Protected/Risky verdicts —
/// callers can't bypass safety by calling this directly.
pub fn quarantine(
    file: &Path,
    verdict: Verdict,
    quarantine_dir: &Path,
) -> Result<QuarantineRecord> {
    match verdict {
        Verdict::Protected | Verdict::Risky => {
            return Err(GenomeError::Rules(format!(
                "refusing to quarantine {} with verdict {verdict:?}",
                file.display()
            )));
        }
        Verdict::Safe | Verdict::Review => {}
    }

    std::fs::create_dir_all(quarantine_dir).map_err(|e| io_err(quarantine_dir, e))?;

    let stamp = now_epoch();
    let record = QuarantineRecord {
        original: file.to_path_buf(),
        quarantined_to: unique_dest(quarantine_dir, stamp, file),
        at_epoch: stamp,
    };

    // Encode before moving anything. `serde` refuses a `PathBuf` that
    // isn't valid UTF-8, and Linux and macOS both allow filenames that
    // aren't — a Latin-1 name out of an old archive is enough. Moving
    // first and discovering that afterwards left the file in quarantine
    // with no record of where it came from, under a flattened name that
    // cannot be read back: the exact loss this manifest exists to stop.
    let line = encode(&record)?;

    move_file(file, &record.quarantined_to)?;

    // The move happened; the record must follow it or the move must not
    // stand. Anything else strands the file.
    let _guard = manifest_lock();
    if let Err(e) = append_line(quarantine_dir, &line) {
        // Best effort, and the only sensible order: the original was
        // sitting here a moment ago, so putting it back is the outcome
        // closest to nothing having happened.
        let _ = move_file(&record.quarantined_to, &record.original);
        return Err(e);
    }
    Ok(record)
}

/// A quarantine filename that no existing file already owns.
///
/// Flattening is lossy on purpose (it only has to be readable; the
/// manifest is the source of truth), which means two different originals
/// can flatten to the same name inside the same second. Without this the
/// second `rename` would silently overwrite the first — quarantine losing
/// a file is the one failure this module cannot have.
fn unique_dest(quarantine_dir: &Path, stamp: i64, file: &Path) -> PathBuf {
    let mut flat = file.to_string_lossy().replace(['/', '\\', ':'], "_");
    if flat.len() > MAX_FLAT_LEN {
        // Byte-slice on a char boundary; a split mid-codepoint would panic.
        let cut = flat
            .char_indices()
            .map(|(i, _)| i)
            .find(|&i| flat.len() - i <= MAX_FLAT_LEN)
            .unwrap_or(0);
        flat = flat.split_off(cut);
    }

    let base = format!("{stamp}_{flat}");
    let mut candidate = quarantine_dir.join(&base);
    let mut n = 1u32;
    while candidate.exists() {
        candidate = quarantine_dir.join(format!("{base}.{n}"));
        n += 1;
    }
    candidate
}

/// Restore a quarantined file to its original location.
///
/// Refuses when something is already there. Both halves of `move_file`
/// replace an existing destination without asking, and the files most
/// likely to be quarantined are the ones most likely to come back: a
/// browser cache is `safe` precisely because the browser rebuilds it, so
/// "quarantine the cache, keep browsing, change your mind" ends with the
/// undo destroying the newer file. Overwriting a file the user did not
/// name is exactly what this module promises never to do, so an occupied
/// destination is an error the caller shows rather than a decision this
/// function makes.
///
/// Leaves the manifest alone — see [`restore_from_manifest`] for the
/// version that also stops listing the file as quarantined.
pub fn restore(record: &QuarantineRecord) -> Result<()> {
    // symlink_metadata, not exists(): a broken symlink is something in
    // the way too, and exists() follows the link and reports false.
    if std::fs::symlink_metadata(&record.original).is_ok() {
        return Err(GenomeError::Rules(format!(
            "{} already exists; move or remove it and restore again — \
             refusing to overwrite it with the quarantined copy",
            record.original.display()
        )));
    }

    if let Some(parent) = record.original.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
    }
    move_file(&record.quarantined_to, &record.original)
}

/// Everything the manifest says is currently in quarantine, oldest first.
///
/// A quarantine directory with no manifest is empty, not broken: the app
/// resolves the path before anything has been quarantined into it.
pub fn list(quarantine_dir: &Path) -> Result<Vec<QuarantineRecord>> {
    let _guard = manifest_lock();
    read_manifest(quarantine_dir)
}

/// [`list`] without taking the lock — for callers that already hold it.
/// `Mutex` is not reentrant, so they must not go through `list`.
fn read_manifest(quarantine_dir: &Path) -> Result<Vec<QuarantineRecord>> {
    let path = manifest_path(quarantine_dir);
    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(io_err(&path, e)),
    };

    Ok(contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| match serde_json::from_str::<QuarantineRecord>(line) {
            Ok(record) => Some(record),
            // One torn line — a half-written append after a power cut —
            // must not hide every other file from the restore list.
            Err(e) => {
                tracing::warn!(manifest = %path.display(), "skipping unreadable manifest line: {e}");
                None
            }
        })
        .collect())
}

/// Restore the file quarantined *to* `quarantined_to` and stop listing it.
///
/// Addressed by quarantine path rather than by original path: the
/// quarantine path is the unique one. The same original can be quarantined
/// again after being restored, and both attempts are real records.
pub fn restore_from_manifest(
    quarantine_dir: &Path,
    quarantined_to: &Path,
) -> Result<QuarantineRecord> {
    // Held across the read, the move and the rewrite: anything appended
    // between the read and the rewrite would be erased by it.
    let _guard = manifest_lock();
    let records = read_manifest(quarantine_dir)?;
    let record = records
        .iter()
        .find(|r| r.quarantined_to == quarantined_to)
        .cloned()
        .ok_or_else(|| {
            GenomeError::Rules(format!(
                "{} is not listed in the quarantine manifest",
                quarantined_to.display()
            ))
        })?;

    restore(&record)?;

    // Rewrite only after the move succeeded. Dropping the line first would
    // strand the file in quarantine with nothing recording where it came
    // from — the failure this manifest exists to prevent.
    let remaining: Vec<QuarantineRecord> = records
        .into_iter()
        .filter(|r| r.quarantined_to != quarantined_to)
        .collect();
    write_manifest(quarantine_dir, &remaining)?;
    Ok(record)
}

/// What a [`purge`] actually did.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PurgeSummary {
    pub files_removed: usize,
    pub bytes_removed: u64,
    /// Files the manifest listed that could not be removed, with the
    /// reason. Reported rather than raised: one locked file shouldn't
    /// abort the purge of everything else.
    pub failed: Vec<String>,
}

/// Empty quarantine for good. This is the only deletion in Diskern, and
/// it deletes only the files the manifest says this crate moved here —
/// never whatever else happens to be sitting in the directory.
pub fn purge(quarantine_dir: &Path) -> Result<PurgeSummary> {
    let _guard = manifest_lock();
    let records = read_manifest(quarantine_dir)?;
    let mut summary = PurgeSummary::default();
    let mut kept = Vec::new();

    for record in records {
        let size = std::fs::metadata(&record.quarantined_to)
            .map(|m| m.len())
            .unwrap_or(0);
        match std::fs::remove_file(&record.quarantined_to) {
            Ok(()) => {
                summary.files_removed += 1;
                summary.bytes_removed += size;
            }
            // Already gone: the manifest was stale, and the goal (not
            // there any more) is met. Anything else stays listed so the
            // user can still restore it.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                summary.files_removed += 1;
            }
            Err(e) => {
                summary
                    .failed
                    .push(format!("{}: {e}", record.quarantined_to.display()));
                kept.push(record);
            }
        }
    }

    write_manifest(quarantine_dir, &kept)?;
    Ok(summary)
}

/// Where the manifest lives for a given quarantine directory.
pub fn manifest_path(quarantine_dir: &Path) -> PathBuf {
    quarantine_dir.join(MANIFEST_NAME)
}

/// One manifest line. Fails on a path that is not valid UTF-8, which is
/// why every caller encodes before it moves anything.
fn encode(record: &QuarantineRecord) -> Result<String> {
    serde_json::to_string(record).map_err(|e| {
        GenomeError::Rules(format!(
            "cannot record {} in the quarantine manifest, so it will not be moved: {e}",
            record.original.display()
        ))
    })
}

fn append_line(quarantine_dir: &Path, line: &str) -> Result<()> {
    let path = manifest_path(quarantine_dir);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| io_err(&path, e))?;
    writeln!(file, "{line}").map_err(|e| io_err(&path, e))
}

/// Rewrite the whole manifest. Via a temporary file in the same directory
/// so a crash mid-write leaves the old manifest intact rather than a
/// truncated one — the manifest is the only copy of where these files
/// belong.
fn write_manifest(quarantine_dir: &Path, records: &[QuarantineRecord]) -> Result<()> {
    let path = manifest_path(quarantine_dir);
    let tmp = path.with_extension("jsonl.tmp");

    let mut body = String::new();
    for record in records {
        body.push_str(&encode(record)?);
        body.push('\n');
    }

    std::fs::write(&tmp, body).map_err(|e| io_err(&tmp, e))?;
    std::fs::rename(&tmp, &path).map_err(|e| io_err(&path, e))
}

/// Move a file, in the one direction this crate is allowed to move things.
///
/// `rename` is the fast path and fails with `EXDEV` when the two paths are
/// on different filesystems — which is the normal case here, not an edge
/// one: quarantine lives in the app's local data directory, and the files
/// being quarantined come from wherever the user pointed the scan, which
/// may well be a second drive or a `/home` on its own partition.
///
/// Both directions go through this. The quarantine path used to handle the
/// fallback and the restore path didn't, so the exact case the move
/// survived was the case the undo failed on.
fn move_file(from: &Path, to: &Path) -> Result<()> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    copy_then_remove(from, to)
}

/// The fallback half of [`move_file`], separated so it can be tested
/// without two filesystems to hand.
///
/// The remove comes last on purpose: if it fails, the file still exists in
/// both places, which is recoverable. Removing first and failing to copy
/// would not be.
fn copy_then_remove(from: &Path, to: &Path) -> Result<()> {
    std::fs::copy(from, to).map_err(|e| io_err(from, e))?;
    std::fs::remove_file(from).map_err(|e| io_err(from, e))
}

fn io_err(path: &Path, source: std::io::Error) -> GenomeError {
    GenomeError::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarantine_and_restore_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let q = dir.path().join("quarantine");
        let f = dir.path().join("victim.txt");
        std::fs::write(&f, b"data").unwrap();

        let rec = quarantine(&f, Verdict::Safe, &q).unwrap();
        assert!(!f.exists());
        assert!(rec.quarantined_to.exists());

        restore(&rec).unwrap();
        assert!(f.exists());
    }

    /// Issue #42. `quarantine` handled the cross-filesystem case and
    /// `restore` didn't, so quarantining a file from another mount worked
    /// and undoing it returned EXDEV. Provoking a real EXDEV needs two
    /// mounts, so this exercises the fallback both directions now share.
    #[test]
    fn the_cross_filesystem_fallback_moves_the_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("from.txt");
        let to = dir.path().join("to.txt");
        std::fs::write(&from, b"data").unwrap();

        copy_then_remove(&from, &to).unwrap();

        assert!(!from.exists());
        assert_eq!(std::fs::read(&to).unwrap(), b"data");
    }

    #[test]
    fn restore_recreates_a_directory_that_was_removed_meanwhile() {
        let dir = tempfile::tempdir().unwrap();
        let q = dir.path().join("quarantine");
        let nested = dir.path().join("a/b");
        std::fs::create_dir_all(&nested).unwrap();
        let f = nested.join("victim.txt");
        std::fs::write(&f, b"data").unwrap();

        let rec = quarantine(&f, Verdict::Safe, &q).unwrap();
        std::fs::remove_dir_all(dir.path().join("a")).unwrap();

        restore(&rec).unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"data");
    }

    #[test]
    fn refuses_protected() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("critical.sys");
        std::fs::write(&f, b"x").unwrap();
        assert!(quarantine(&f, Verdict::Protected, dir.path()).is_err());
        assert!(f.exists()); // untouched
    }

    /// Issue #43. The module promised every action stays reversible until
    /// the user purges, and the only record of where a file came from was
    /// the value `quarantine` returned — which the app dropped on the
    /// floor. Reversibility has to survive the process exiting.
    #[test]
    fn a_record_outlives_the_process_that_made_it() {
        let dir = tempfile::tempdir().unwrap();
        let q = dir.path().join("quarantine");
        let f = dir.path().join("victim.txt");
        std::fs::write(&f, b"data").unwrap();

        // Return value deliberately ignored, the way the app used to.
        let _ = quarantine(&f, Verdict::Safe, &q).unwrap();

        // Nothing carried over in memory: read it back off the disk.
        let listed = list(&q).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].original, f);

        restore_from_manifest(&q, &listed[0].quarantined_to).unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"data");
        assert!(list(&q).unwrap().is_empty());
    }

    /// Flattening maps `/` and `_` onto the same character, so two
    /// different originals can produce the same name inside one second.
    /// Before `unique_dest` the second rename overwrote the first.
    #[test]
    fn two_files_that_flatten_alike_do_not_overwrite_each_other() {
        let dir = tempfile::tempdir().unwrap();
        let q = dir.path().join("quarantine");
        let a = dir.path().join("a_b");
        let b_dir = dir.path().join("a");
        std::fs::create_dir(&b_dir).unwrap();
        let b = b_dir.join("b");
        std::fs::write(&a, b"first").unwrap();
        std::fs::write(&b, b"second").unwrap();

        let ra = quarantine(&a, Verdict::Safe, &q).unwrap();
        let rb = quarantine(&b, Verdict::Safe, &q).unwrap();

        assert_ne!(ra.quarantined_to, rb.quarantined_to);
        assert_eq!(std::fs::read(&ra.quarantined_to).unwrap(), b"first");
        assert_eq!(std::fs::read(&rb.quarantined_to).unwrap(), b"second");
        assert_eq!(list(&q).unwrap().len(), 2);
    }

    #[test]
    fn an_empty_quarantine_lists_nothing_rather_than_failing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(list(&dir.path().join("never-used")).unwrap().is_empty());
    }

    /// A half-written line after a power cut must not hide the records
    /// either side of it.
    #[test]
    fn a_torn_manifest_line_is_skipped_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let q = dir.path().join("quarantine");
        let f = dir.path().join("victim.txt");
        std::fs::write(&f, b"data").unwrap();
        quarantine(&f, Verdict::Safe, &q).unwrap();

        let manifest = manifest_path(&q);
        let good = std::fs::read_to_string(&manifest).unwrap();
        // A line cut off mid-value, the way an interrupted append leaves it.
        std::fs::write(
            &manifest,
            format!("{{\"original\": \"/home/u/half-writ\n{good}"),
        )
        .unwrap();

        assert_eq!(list(&q).unwrap().len(), 1);
    }

    #[test]
    fn purge_removes_the_files_it_listed_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let q = dir.path().join("quarantine");
        let f = dir.path().join("victim.txt");
        std::fs::write(&f, b"data").unwrap();
        let rec = quarantine(&f, Verdict::Safe, &q).unwrap();

        // Something Diskern did not put here.
        let stranger = q.join("not-ours.txt");
        std::fs::write(&stranger, b"leave me alone").unwrap();

        let summary = purge(&q).unwrap();
        assert_eq!(summary.files_removed, 1);
        assert_eq!(summary.bytes_removed, 4);
        assert!(summary.failed.is_empty());

        assert!(!rec.quarantined_to.exists());
        assert!(stranger.exists());
        assert!(list(&q).unwrap().is_empty());
    }

    #[test]
    fn restoring_something_not_in_the_manifest_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let q = dir.path().join("quarantine");
        std::fs::create_dir_all(&q).unwrap();
        assert!(restore_from_manifest(&q, &q.join("invented")).is_err());
    }

    /// A path serde cannot encode must stop the move, not follow it.
    ///
    /// Linux and macOS both allow filenames that aren't valid UTF-8, and a
    /// disk scanner meets them. Encoding after the move left the file in
    /// quarantine, absent from the manifest, under a flattened name with
    /// no way back — while the caller was told the operation failed.
    #[cfg(unix)]
    #[test]
    fn a_path_that_cannot_be_recorded_is_not_moved() {
        use std::os::unix::ffi::OsStrExt;

        let dir = tempfile::tempdir().unwrap();
        let q = dir.path().join("quarantine");
        // "café.dmg" in Latin-1: a valid filename, invalid UTF-8.
        let victim = dir.path().join(std::ffi::OsStr::from_bytes(b"caf\xe9.dmg"));
        std::fs::write(&victim, b"payload").unwrap();

        let err = quarantine(&victim, Verdict::Review, &q).unwrap_err();
        assert!(
            err.to_string().contains("will not be moved"),
            "unexpected error: {err}"
        );

        // The file is exactly where it was, and quarantine is untouched.
        assert_eq!(std::fs::read(&victim).unwrap(), b"payload");
        assert!(list(&q).unwrap().is_empty());
        let strays = std::fs::read_dir(&q).map(|d| d.count()).unwrap_or(0);
        assert_eq!(strays, 0, "quarantine should hold no orphan");
    }

    /// Restoring must not destroy a file that came back on its own.
    ///
    /// The caches Diskern calls `safe` are the ones applications rebuild,
    /// so this is the ordinary sequence, not a contrived one: quarantine
    /// the cache, carry on using the app, then change your mind.
    #[test]
    fn restore_refuses_to_overwrite_something_already_there() {
        let dir = tempfile::tempdir().unwrap();
        let q = dir.path().join("quarantine");
        let original = dir.path().join("cache.dat");
        std::fs::write(&original, b"old-copy").unwrap();

        let rec = quarantine(&original, Verdict::Safe, &q).unwrap();
        std::fs::write(&original, b"regenerated-by-the-app").unwrap();

        let err = restore(&rec).unwrap_err();
        assert!(err.to_string().contains("already exists"), "{err}");

        // Neither copy was destroyed: the new one is still in place and
        // the quarantined one is still restorable once it is moved aside.
        assert_eq!(std::fs::read(&original).unwrap(), b"regenerated-by-the-app");
        assert_eq!(std::fs::read(&rec.quarantined_to).unwrap(), b"old-copy");
        assert_eq!(list(&q).unwrap().len(), 1);

        std::fs::remove_file(&original).unwrap();
        restore(&rec).unwrap();
        assert_eq!(std::fs::read(&original).unwrap(), b"old-copy");
    }

    /// Nothing may end up in the quarantine directory without a manifest
    /// line naming it.
    ///
    /// `restore_from_manifest` and `purge` rewrite the manifest from what
    /// they read, so a record appended in between used to be erased —
    /// stranding a file that `list` could not see, `restore` could not
    /// reach and `purge` would not remove. Two threads quarantining while
    /// a third restores reproduces it without the lock.
    #[test]
    fn concurrent_quarantines_and_restores_strand_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let q = dir.path().join("quarantine");
        let victims: Vec<PathBuf> = (0..40)
            .map(|i| {
                let f = dir.path().join(format!("victim-{i}.dat"));
                std::fs::write(&f, format!("{i}")).unwrap();
                f
            })
            .collect();

        std::thread::scope(|scope| {
            for chunk in victims.chunks(20) {
                let q = &q;
                scope.spawn(move || {
                    for victim in chunk {
                        quarantine(victim, Verdict::Safe, q).unwrap();
                    }
                });
            }
            // Restores racing the appends, on whatever is listed so far.
            scope.spawn(|| {
                for _ in 0..40 {
                    if let Some(record) = list(&q).unwrap().first() {
                        // Losing the race with another restore is fine;
                        // stranding a file is not.
                        let _ = restore_from_manifest(&q, &record.quarantined_to);
                    }
                }
            });
        });

        let listed: std::collections::HashSet<PathBuf> = list(&q)
            .unwrap()
            .into_iter()
            .map(|r| r.quarantined_to)
            .collect();
        let on_disk: Vec<PathBuf> = std::fs::read_dir(&q)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.file_name().is_some_and(|n| n != MANIFEST_NAME))
            .collect();

        for path in &on_disk {
            assert!(
                listed.contains(path),
                "{} is in quarantine with no manifest record",
                path.display()
            );
        }
        assert_eq!(listed.len(), on_disk.len());
    }
}
