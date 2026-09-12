use std::process::Command;
use tempfile::tempdir;

fn run_scan(root: &std::path::Path) -> std::process::Output {
    run_scan_roots([root])
}

fn run_scan_roots<'a>(
    roots: impl IntoIterator<Item = &'a std::path::Path>,
) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_diskern"));
    command.arg("scan");
    for root in roots {
        command.arg(root);
    }
    command.output().expect("diskern should start")
}

fn run_scan_with_excludes<'a>(
    root: &std::path::Path,
    excludes: impl IntoIterator<Item = &'a std::path::Path>,
) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_diskern"));
    command.arg("scan").arg(root);
    for exclude in excludes {
        command.arg("--exclude").arg(exclude);
    }
    command.output().expect("diskern should start")
}

#[test]
fn missing_scan_root_fails_and_names_the_root() {
    let temp = tempdir().unwrap();
    let missing = temp.path().join("missing");

    let output = run_scan(&missing);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&missing.display().to_string()),
        "expected stderr to name {missing:?}, got: {stderr}"
    );
}

#[test]
fn invalid_child_scan_root_fails_and_names_the_root() {
    let temp = tempdir().unwrap();
    let file = temp.path().join("file");
    std::fs::write(&file, b"not a directory").unwrap();
    let invalid_root = file.join("child");

    let output = run_scan(&invalid_root);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&invalid_root.display().to_string()),
        "expected stderr to name {invalid_root:?}, got: {stderr}"
    );
}

#[test]
fn nested_scan_roots_are_counted_once_in_the_summary() {
    let temp = tempdir().unwrap();
    let child = temp.path().join("child");
    std::fs::create_dir(&child).unwrap();
    std::fs::write(temp.path().join("root.bin"), b"root").unwrap();
    std::fs::write(child.join("child.bin"), b"child").unwrap();

    let output = run_scan_roots([temp.path(), child.as_path()]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Scanned 2 files."),
        "expected nested roots to count two files once, got: {stdout}"
    );
}

#[test]
fn scan_excludes_cli_directories() {
    let temp = tempdir().unwrap();
    let skipped = temp.path().join("skip-me");
    std::fs::create_dir(&skipped).unwrap();
    std::fs::write(temp.path().join("keep.bin"), b"keep").unwrap();
    std::fs::write(skipped.join("skip.bin"), b"skip").unwrap();

    let output = run_scan_with_excludes(temp.path(), [skipped.as_path()]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Scanned 1 files."),
        "expected excluded directory to be skipped, got: {stdout}"
    );
}
