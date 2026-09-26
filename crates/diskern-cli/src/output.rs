use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

pub(super) fn write_json(path: &Path, rendered: &str) -> io::Result<()> {
    write_atomically(path, |file| {
        file.write_all(rendered.as_bytes())?;
        file.write_all(b"\n")
    })
}

fn write_atomically(
    path: &Path,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    // A bare filename has an empty parent. Keep staging beside the destination
    // so publication stays on the same filesystem; never create parent folders.
    let directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    write(temporary.as_file_mut())?;
    temporary.as_file().sync_all()?;

    // persist replaces existing files on all supported platforms. Converting
    // its error also drops the retained temporary file, cleaning up immediately.
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn fail_after_partial_write(file: &mut File) -> io::Result<()> {
        file.write_all(b"{\"files_scanned\":")?;
        Err(io::Error::other("injected write failure"))
    }

    #[test]
    fn partial_write_failure_preserves_existing_report_and_cleans_up() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("report.json");
        let previous = b"{\"files_scanned\":42}\n";
        fs::write(&path, previous).unwrap();

        let error = write_atomically(&path, fail_after_partial_write).unwrap_err();

        assert_eq!(error.to_string(), "injected write failure");
        assert_eq!(fs::read(&path).unwrap(), previous);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn partial_write_failure_leaves_no_new_report_or_temporary_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("report.json");

        let error = write_atomically(&path, fail_after_partial_write).unwrap_err();

        assert_eq!(error.to_string(), "injected write failure");
        assert!(!path.exists());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn publication_replaces_a_symlink_with_a_private_report() {
        use std::os::unix::fs::{symlink, PermissionsExt};

        let dir = tempdir().unwrap();
        let target = dir.path().join("original.json");
        fs::write(&target, b"original report").unwrap();
        let path = dir.path().join("report.json");
        symlink(&target, &path).unwrap();

        write_json(&path, "{}").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"{}\n");
        assert_eq!(fs::read(&target).unwrap(), b"original report");
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(metadata.is_file());
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn replacement_failure_cleans_up_and_preserves_destination() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("report.json");
        fs::create_dir(&path).unwrap();
        let existing = path.join("keep.txt");
        fs::write(&existing, b"keep").unwrap();

        let error = write_json(&path, "{}").unwrap_err();

        assert!(!error.to_string().is_empty());
        assert_eq!(fs::read(&existing).unwrap(), b"keep");
        // Cleanup must happen even while the returned error is still alive.
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        assert_eq!(fs::read_dir(&path).unwrap().count(), 1);
    }
}
