use std::fs::{self, File};
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
    let permissions = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => {
            let permissions = metadata.permissions();
            if permissions.readonly() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "destination report is read-only",
                ));
            }
            // Opening without truncation preserves the original export's OS
            // write-permission checks, even when the parent allows replacement.
            File::options().write(true).open(path)?;
            Some(permissions)
        }
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "destination must be a regular file; use a file path instead of a symbolic link, directory, or special file",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };

    // A bare filename has an empty parent. Keep staging beside the destination
    // so publication stays on the same filesystem; never create parent folders.
    let directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    write(temporary.as_file_mut())?;
    // Keep staged bytes private until the complete report has been written.
    if let Some(permissions) = permissions {
        temporary.as_file().set_permissions(permissions)?;
    }
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
    fn replacement_preserves_existing_report_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let path = dir.path().join("report.json");
        fs::write(&path, b"old report").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();

        write_json(&path, "{}").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"{}\n");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn read_only_reports_are_preserved() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("report.json");
        fs::write(&path, b"old report").unwrap();
        let original_permissions = fs::metadata(&path).unwrap().permissions();
        let mut read_only = original_permissions.clone();
        read_only.set_readonly(true);
        fs::set_permissions(&path, read_only).unwrap();

        let result = write_json(&path, "{}");
        let contents = fs::read(&path).unwrap();
        let entries = fs::read_dir(dir.path()).unwrap().count();
        // Restore permissions before assertions so Windows can remove the fixture.
        fs::set_permissions(&path, original_permissions).unwrap();

        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(contents, b"old report");
        assert_eq!(entries, 1);
    }

    #[cfg(unix)]
    #[test]
    fn publication_preserves_symlinks_and_their_targets() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let target = dir.path().join("original.json");
        fs::write(&target, b"original report").unwrap();
        let path = dir.path().join("report.json");
        symlink(&target, &path).unwrap();

        let error = write_json(&path, "{}").unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(fs::read(&target).unwrap(), b"original report");
        assert_eq!(fs::read_link(&path).unwrap(), target);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn publication_preserves_special_files() {
        use std::os::unix::fs::FileTypeExt;
        use std::os::unix::net::UnixListener;

        let dir = tempdir().unwrap();
        let path = dir.path().join("report.sock");
        let _listener = UnixListener::bind(&path).unwrap();

        let error = write_json(&path, "{}").unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(fs::symlink_metadata(&path).unwrap().file_type().is_socket());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn new_reports_have_private_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let path = dir.path().join("report.json");
        write_json(&path, "{}").unwrap();

        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn replacement_failure_cleans_up_and_preserves_destination() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("report.json");
        let existing = path.join("keep.txt");

        // Make the destination invalid after preflight, so this exercises a
        // persist failure and cleanup rather than only destination validation.
        let error = write_atomically(&path, |file| {
            file.write_all(b"{}")?;
            fs::create_dir(&path)?;
            fs::write(&existing, b"keep")
        })
        .unwrap_err();

        assert!(!error.to_string().is_empty());
        assert_eq!(fs::read(&existing).unwrap(), b"keep");
        // Cleanup must happen even while the returned error is still alive.
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        assert_eq!(fs::read_dir(&path).unwrap().count(), 1);
    }
}
