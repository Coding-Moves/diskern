//! The ONLY module allowed to modify the filesystem — and it only moves.
//!
//! Design rule: Diskern never hard-deletes. Files are moved to a
//! quarantine directory with a manifest recording original locations, so
//! every action is reversible until the user explicitly purges quarantine.

use crate::{GenomeError, Result, Verdict};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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

    // Flatten path into a unique quarantine filename.
    let stamp = now_epoch();
    let flat = file.to_string_lossy().replace(['/', '\\', ':'], "_");
    let dest = quarantine_dir.join(format!("{stamp}_{flat}"));

    move_file(file, &dest)?;

    Ok(QuarantineRecord {
        original: file.to_path_buf(),
        quarantined_to: dest,
        at_epoch: stamp,
    })
}

/// Restore a quarantined file to its original location.
pub fn restore(record: &QuarantineRecord) -> Result<()> {
    if let Some(parent) = record.original.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
    }
    move_file(&record.quarantined_to, &record.original)
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
}
