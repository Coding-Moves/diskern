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

    // rename() fails across filesystems; fall back to copy+remove.
    if std::fs::rename(file, &dest).is_err() {
        std::fs::copy(file, &dest).map_err(|e| io_err(file, e))?;
        std::fs::remove_file(file).map_err(|e| io_err(file, e))?;
    }

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
    std::fs::rename(&record.quarantined_to, &record.original)
        .map_err(|e| io_err(&record.quarantined_to, e))
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

    #[test]
    fn refuses_protected() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("critical.sys");
        std::fs::write(&f, b"x").unwrap();
        assert!(quarantine(&f, Verdict::Protected, dir.path()).is_err());
        assert!(f.exists()); // untouched
    }
}
