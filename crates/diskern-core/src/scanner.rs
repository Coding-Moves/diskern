//! Parallel, read-only filesystem walk.
//!
//! Strategy: walk with `jwalk` (parallel, ordered), collect metadata only.
//! Hashing is NOT done here — see [`crate::dedup`], which hashes only files
//! whose sizes collide. On a typical disk that skips >95% of hash work.

use crate::{FileEntry, FileIdentity, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io;
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
pub fn scan(opts: &ScanOptions, progress: Arc<ScanProgress>) -> Result<Vec<FileEntry>> {
    scan_with(opts, progress, |_| {})
}

/// [`scan`] plus a callback for each collected file entry.
///
/// The callback runs after the progress counters move and before the entry
/// is stored in the final scan list. It is for provisional UI feedback only:
/// the complete report still comes from the returned `Vec<FileEntry>` so the
/// impact graph and duplicate detector can make the final safety decision.
pub fn scan_with<F>(
    opts: &ScanOptions,
    progress: Arc<ScanProgress>,
    mut on_entry: F,
) -> Result<Vec<FileEntry>>
where
    F: FnMut(&FileEntry),
{
    let mut out = Vec::new();
    // Normalized once for the whole scan: the exclude list never changes, and
    // both the root check below and every directory the walk opens use it.
    let excludes: Vec<String> = opts.excludes.iter().map(|e| normalize_exclude(e)).collect();
    let roots = planned_roots(&opts.roots)?;
    let root_set: Arc<HashSet<PathBuf>> = Arc::new(roots.iter().cloned().collect());

    for root in roots {
        if progress.cancelled.load(Ordering::Relaxed) {
            return Err(crate::GenomeError::Cancelled);
        }
        // Say so, rather than walking a root whose every entry the exclude
        // list will drop. `/run/user/<uid>` holds real caches and sits under
        // the `/run` exclude, so this is reachable — and an empty report is
        // indistinguishable from a clean disk, which is the answer issue #103
        // and #82 are both about not giving.
        if let Some(exclude) = excluded_by(&root, &excludes) {
            return Err(crate::GenomeError::ExcludedRoot {
                path: root,
                exclude: exclude.to_string(),
            });
        }
        walk_root(
            &root,
            &excludes,
            opts,
            &root_set,
            &progress,
            &mut out,
            &mut on_entry,
        )?;
    }
    Ok(out)
}

fn planned_roots(roots: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut planned = Vec::new();
    let mut seen = HashSet::new();

    for root in roots {
        let root = absolute_root(root)?;
        if seen.insert(root.clone()) {
            planned.push(root);
        }
    }

    Ok(planned)
}

/// Which exclude, if any, contains `path`.
fn excluded_by<'a>(path: &Path, excludes: &'a [String]) -> Option<&'a str> {
    let path = path.to_string_lossy();
    excludes
        .iter()
        .find(|ex| is_within(&path, ex))
        .map(String::as_str)
}

/// Issue #103. The rules are written against absolute paths, and the ones
/// anchored at the filesystem root — `/tmp/**`, `/var/log/**` — are anchored
/// on purpose: that is what keeps `/tmp` out of `~/tmp`. `jwalk` builds every
/// entry's path from the root as it was handed in, so `diskern scan tmp` from
/// `/var` produced `tmp/systemd-private/x`, which no anchored pattern can
/// match. The walk found the files and the rules could not tell where they
/// were, so the scan reported nothing to clean.
///
/// `absolute`, not `canonicalize`: it touches no filesystem, works on a path
/// that does not exist, and leaves symlinks alone. `canonicalize` would
/// rewrite `/var/tmp` to `/private/var/tmp` on macOS, scanning somewhere
/// other than what was asked for.
///
/// It resolves a leading `.` but leaves `..` in place, since `a/../b` is only
/// `b` when `a` isn't a symlink. A root spelled with `..` therefore still
/// misses anchored rules; making it absolute is what the rules need, and
/// guessing past a symlink is not.
fn absolute_root(root: &Path) -> Result<PathBuf> {
    std::path::absolute(root).map_err(|source| crate::GenomeError::Io {
        path: root.to_path_buf(),
        source,
    })
}

fn walk_root<F>(
    root: &Path,
    excludes: &[String],
    opts: &ScanOptions,
    roots: &Arc<HashSet<PathBuf>>,
    progress: &ScanProgress,
    out: &mut Vec<FileEntry>,
    on_entry: &mut F,
) -> Result<()>
where
    F: FnMut(&FileEntry),
{
    // `process_read_dir` runs on every directory the walk opens, so the list
    // arrives already normalized rather than being rebuilt here.
    let excludes = excludes.to_vec();
    let current_root = root.to_path_buf();
    let roots = Arc::clone(roots);

    let walker = jwalk::WalkDir::new(root)
        .follow_links(opts.follow_symlinks)
        .skip_hidden(false)
        .process_read_dir(move |_depth, _path, _state, children| {
            children.retain(|entry| {
                entry
                    .as_ref()
                    .map(|e| {
                        let path = e.path();
                        !is_excluded(&path, &excludes)
                            && (path == current_root || !roots.contains(&path))
                    })
                    .unwrap_or(true)
            });
        });

    for entry in walker {
        if progress.cancelled.load(Ordering::Relaxed) {
            return Err(crate::GenomeError::Cancelled);
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) if err.depth() == 0 => return Err(root_walk_error(root, err)),
            Err(_) => continue, // descendant permission errors: skip, don't die
        };
        if let Some(err) = root_read_error(root, &entry) {
            return Err(err);
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(meta) => meta,
            Err(err) if entry.depth() == 0 => {
                return Err(crate::GenomeError::Io {
                    path: entry.path(),
                    source: err.into(),
                });
            }
            Err(_) => continue,
        };
        let size = meta.len();

        progress.files_seen.fetch_add(1, Ordering::Relaxed);
        progress.bytes_seen.fetch_add(size, Ordering::Relaxed);

        let entry = FileEntry {
            path: entry.path(),
            size,
            modified: meta.modified().ok().and_then(to_epoch),
            accessed: meta.accessed().ok().and_then(to_epoch),
            is_symlink: entry.path_is_symlink(),
            identity: file_identity(&entry.path(), &meta),
            hash: None,
        };
        on_entry(&entry);
        out.push(entry);
    }
    Ok(())
}

#[cfg(unix)]
fn file_identity(_path: &Path, meta: &std::fs::Metadata) -> Option<FileIdentity> {
    use std::os::unix::fs::MetadataExt;

    (meta.nlink() > 1).then_some(FileIdentity {
        device: meta.dev(),
        file: meta.ino(),
    })
}

#[cfg(windows)]
fn file_identity(path: &Path, _meta: &std::fs::Metadata) -> Option<FileIdentity> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };

    let file = std::fs::File::open(path).ok()?;
    let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) };
    if ok == 0 {
        return None;
    }

    let info = unsafe { info.assume_init() };
    if info.nNumberOfLinks <= 1 {
        return None;
    }

    Some(FileIdentity {
        device: info.dwVolumeSerialNumber.into(),
        file: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    })
}

#[cfg(not(any(unix, windows)))]
fn file_identity(_path: &Path, _meta: &std::fs::Metadata) -> Option<FileIdentity> {
    None
}

fn root_read_error(root: &Path, entry: &jwalk::DirEntry<((), ())>) -> Option<crate::GenomeError> {
    if entry.depth() != 0 {
        return None;
    }

    let error = entry.read_children.as_ref()?.error()?;
    Some(crate::GenomeError::Io {
        path: error.path().unwrap_or(root).to_path_buf(),
        source: borrowed_walk_error(error),
    })
}

fn root_walk_error(root: &Path, error: jwalk::Error) -> crate::GenomeError {
    let path = error.path().unwrap_or(root).to_path_buf();
    crate::GenomeError::Io {
        path,
        source: error.into(),
    }
}

fn borrowed_walk_error(error: &jwalk::Error) -> io::Error {
    let kind = error
        .io_error()
        .map_or(io::ErrorKind::Other, |err| err.kind());
    io::Error::new(kind, error.to_string())
}

/// Lowercased, `/`-separated, no trailing separator. An exclude written
/// `C:\\Windows\\WinSxS` has to match a root the user typed as
/// `c:\\windows\\winsxs`.
///
/// ASCII case folding, not Unicode: these are directory names like
/// `Windows` and `System`, and it lets [`is_excluded`] fold the path a
/// character at a time instead of building a lowercased copy of it.
fn normalize_exclude(exclude: &str) -> String {
    let normalized = exclude.replace('\\', "/").to_ascii_lowercase();
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
///
/// This runs for every child of every directory the walk opens, so it
/// normalizes nothing: it folds the path as it compares and stops at the
/// first character that differs — which, for a path not under an exclude,
/// is almost always the first or second. Building a normalized copy per
/// entry cost two allocations each and measured 2.4x slower than the
/// plain `starts_with` it replaced.
fn is_excluded(path: &Path, excludes: &[String]) -> bool {
    if excludes.is_empty() {
        return false;
    }
    let path = path.to_string_lossy();
    excludes.iter().any(|ex| is_within(&path, ex))
}

/// Is `path`, folded as it goes, the already-normalized `exclude` itself
/// or something inside it?
fn is_within(path: &str, exclude: &str) -> bool {
    let mut chars = path.chars();
    for expected in exclude.chars() {
        match chars.next() {
            Some(c) if fold(c) == expected => {}
            _ => return false,
        }
    }
    // Ends exactly there, or carries on at a component boundary — which a
    // trailing separator on the path itself also satisfies.
    match chars.next() {
        None => true,
        Some(sep) => sep == '/' || sep == '\\',
    }
}

fn fold(c: char) -> char {
    match c {
        '\\' => '/',
        c => c.to_ascii_lowercase(),
    }
}

fn to_epoch(t: std::time::SystemTime) -> Option<i64> {
    t.duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

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
    fn nothing_is_excluded_when_there_are_no_excludes() {
        assert!(!is_excluded(Path::new("/proc/1/maps"), &[]));
    }

    #[test]
    fn a_trailing_separator_on_the_path_still_matches() {
        let excludes = [normalize_exclude("/run")];
        assert!(is_excluded(Path::new("/run/"), &excludes));
    }

    #[test]
    fn an_exclude_longer_than_the_path_does_not_match() {
        let excludes = [normalize_exclude("/proc/self/fd")];
        assert!(!is_excluded(Path::new("/proc"), &excludes));
    }

    #[test]
    fn a_trailing_separator_on_an_exclude_changes_nothing() {
        let excludes = [normalize_exclude("/proc/")];
        assert!(is_excluded(Path::new("/proc/1/maps"), &excludes));
        assert!(!is_excluded(Path::new("/process-data/x"), &excludes));
    }

    /// The folding in `is_within` against a directory that really exists.
    ///
    /// The filesystem's own case rules have no bearing on this, on any
    /// platform: `is_excluded` compares two strings and never opens a
    /// file. Drop the ASCII folding and this fails on Windows, macOS and
    /// Linux alike — which is what makes it worth running on all three.
    #[test]
    fn an_exclude_matches_a_real_directory_whatever_its_case() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("Cache");
        std::fs::create_dir(&cache).unwrap();
        std::fs::write(cache.join("blob.bin"), b"cached").unwrap();
        std::fs::write(dir.path().join("keep.txt"), b"keep").unwrap();

        let opts = ScanOptions {
            roots: vec![dir.path().to_path_buf()],
            // Written lowercase; the directory on disk is not.
            excludes: vec![cache.to_string_lossy().to_lowercase()],
            ..Default::default()
        };
        let entries = scan(&opts, Arc::new(ScanProgress::default())).unwrap();

        assert_eq!(entries.len(), 1, "{entries:#?}");
        assert_eq!(
            entries[0].path.file_name().unwrap().to_string_lossy(),
            "keep.txt"
        );
    }

    /// Issue #103. Anchored rules only match absolute paths, so a relative
    /// root has to be resolved before the walk, not after.
    #[test]
    fn a_relative_root_is_made_absolute() {
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(absolute_root(Path::new("tmp")).unwrap(), cwd.join("tmp"));
        assert_eq!(absolute_root(Path::new(".")).unwrap(), cwd);
    }

    /// An absolute root is already what the rules expect and must survive
    /// untouched — in particular `/var/tmp` must not become the symlink
    /// target `/private/var/tmp` that `canonicalize` would produce on macOS.
    #[cfg(unix)]
    #[test]
    fn an_absolute_root_is_left_alone() {
        let root = Path::new("/var/tmp");
        assert_eq!(absolute_root(root).unwrap(), root);
    }

    /// The same guarantee on Windows, where it needs a different path to
    /// state. `/var/tmp` is not absolute there — it is rooted but has no
    /// drive, so `absolute` resolves it against the current one, which is
    /// the right answer and not the one this test is about.
    #[cfg(windows)]
    #[test]
    fn an_absolute_root_is_left_alone() {
        let root = Path::new(r"C:\Users\example\AppData\Local\Temp");
        assert_eq!(absolute_root(root).unwrap(), root);
    }

    /// A root the exclude list covers is an error, not an empty scan. The
    /// walk would drop every entry and the report would look like a clean
    /// disk, which is the answer #103 exists to stop giving.
    #[test]
    fn a_root_inside_an_exclude_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.bin"), b"x").unwrap();

        let err = scan(
            &ScanOptions {
                roots: vec![dir.path().to_path_buf()],
                excludes: vec![dir.path().to_string_lossy().into_owned()],
                ..Default::default()
            },
            Arc::new(ScanProgress::default()),
        )
        .unwrap_err();

        assert!(
            matches!(err, crate::GenomeError::ExcludedRoot { .. }),
            "{err:?}"
        );
        assert!(err.to_string().contains("excluded directory"), "{err}");
    }

    #[test]
    fn repeated_roots_are_scanned_once() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("one.bin"), b"one").unwrap();
        let progress = Arc::new(ScanProgress::default());

        let entries = scan(
            &ScanOptions {
                roots: vec![dir.path().to_path_buf(), dir.path().to_path_buf()],
                ..Default::default()
            },
            Arc::clone(&progress),
        )
        .unwrap();

        assert_eq!(entries.len(), 1, "{entries:#?}");
        assert_eq!(progress.files_seen.load(Ordering::Relaxed), 1);
        assert_eq!(progress.bytes_seen.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn nested_roots_do_not_count_the_child_tree_twice() {
        let dir = tempfile::tempdir().unwrap();
        let child = dir.path().join("child");
        std::fs::create_dir(&child).unwrap();
        std::fs::write(dir.path().join("root.bin"), b"root").unwrap();
        std::fs::write(child.join("child.bin"), b"child").unwrap();

        for roots in [
            vec![dir.path().to_path_buf(), child.clone()],
            vec![child.clone(), dir.path().to_path_buf()],
        ] {
            let progress = Arc::new(ScanProgress::default());
            let entries = scan(
                &ScanOptions {
                    roots,
                    ..Default::default()
                },
                Arc::clone(&progress),
            )
            .unwrap();

            assert_eq!(entries.len(), 2, "{entries:#?}");
            assert_eq!(progress.files_seen.load(Ordering::Relaxed), 2);
            assert_eq!(progress.bytes_seen.load(Ordering::Relaxed), 9);
        }
    }

    #[test]
    fn an_explicit_child_root_still_reports_its_own_exclude_error() {
        let dir = tempfile::tempdir().unwrap();
        let child = dir.path().join("child");
        std::fs::create_dir(&child).unwrap();
        std::fs::write(child.join("one.bin"), b"one").unwrap();

        let err = scan(
            &ScanOptions {
                roots: vec![dir.path().to_path_buf(), child.clone()],
                excludes: vec![child.to_string_lossy().into_owned()],
                ..Default::default()
            },
            Arc::new(ScanProgress::default()),
        )
        .unwrap_err();

        match err {
            crate::GenomeError::ExcludedRoot { path, .. } => assert_eq!(path, child),
            other => panic!("expected excluded child root, got {other:?}"),
        }
    }

    #[test]
    fn a_missing_root_is_an_error_from_the_engine() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");

        let err = scan(
            &ScanOptions {
                roots: vec![missing.clone()],
                ..Default::default()
            },
            Arc::new(ScanProgress::default()),
        )
        .unwrap_err();

        let text = err.to_string();
        assert!(text.contains(&missing.display().to_string()), "{text}");
    }

    #[test]
    fn an_invalid_child_root_is_an_error_from_the_engine() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file");
        std::fs::write(&file, b"not a directory").unwrap();
        let invalid_root = file.join("child");

        let err = scan(
            &ScanOptions {
                roots: vec![invalid_root.clone()],
                ..Default::default()
            },
            Arc::new(ScanProgress::default()),
        )
        .unwrap_err();

        let text = err.to_string();
        assert!(text.contains(&invalid_root.display().to_string()), "{text}");
    }

    #[test]
    fn an_empty_readable_root_still_succeeds() {
        let dir = tempfile::tempdir().unwrap();

        let entries = scan(
            &ScanOptions {
                roots: vec![dir.path().to_path_buf()],
                ..Default::default()
            },
            Arc::new(ScanProgress::default()),
        )
        .unwrap();

        assert!(entries.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_explicit_root_is_an_error() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let locked = dir.path().join("locked");
        std::fs::create_dir(&locked).unwrap();
        let original_mode = std::fs::metadata(&locked).unwrap().permissions().mode();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();

        if std::fs::read_dir(&locked).is_ok() {
            std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(original_mode))
                .unwrap();
            return;
        }

        let err = scan(
            &ScanOptions {
                roots: vec![locked.clone()],
                ..Default::default()
            },
            Arc::new(ScanProgress::default()),
        )
        .unwrap_err();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(original_mode)).unwrap();

        let text = err.to_string();
        assert!(text.contains(&locked.display().to_string()), "{text}");
    }

    /// The check is about containment, not a shared prefix: `/runtime-data`
    /// is not inside `/run`, the same property `is_within` is written for.
    #[test]
    fn a_root_merely_sharing_a_prefix_with_an_exclude_is_scanned() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("runtime-data");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("a.bin"), b"x").unwrap();

        let entries = scan(
            &ScanOptions {
                roots: vec![root],
                excludes: vec![dir.path().join("run").to_string_lossy().into_owned()],
                ..Default::default()
            },
            Arc::new(ScanProgress::default()),
        )
        .unwrap();

        assert_eq!(entries.len(), 1);
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
