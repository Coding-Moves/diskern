use std::fs;
use std::process::Command;
use tempfile::tempdir;

fn run_scan(root: &std::path::Path) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_diskern"));
    command.arg("scan").arg(root).arg("--top").arg("0");
    command.output().expect("diskern should start")
}

#[test]
fn scan_with_single_duplicate_set_prints_singular_summary() {
    let root = tempdir().unwrap();
    // Two identical files with non-zero size form exactly 1 duplicate set.
    fs::write(root.path().join("first.txt"), b"duplicate payload here").unwrap();
    fs::write(root.path().join("second.txt"), b"duplicate payload here").unwrap();

    let output = run_scan(root.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("1 duplicate set."),
        "expected '1 duplicate set.', got: {stdout}"
    );
    assert!(
        stdout.contains("Duplicate files — 1 set ·"),
        "expected 'Duplicate files — 1 set ·', got: {stdout}"
    );
}

#[test]
fn scan_summary_says_file_for_one_and_files_for_two() {
    let root = tempdir().unwrap();
    fs::write(root.path().join("only.bin"), b"data").unwrap();

    let output = run_scan(root.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Scanned 1 file."),
        "expected 'Scanned 1 file.', got: {stdout}"
    );

    // A second, different file keeps the count at two and the plural intact.
    fs::write(root.path().join("second.bin"), b"more data").unwrap();
    let output = run_scan(root.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Scanned 2 files."),
        "expected 'Scanned 2 files.', got: {stdout}"
    );
}

#[test]
fn scan_with_single_finding_prints_singular_summary() {
    let root = tempdir().unwrap();
    let cache = root.path().join(".cache/google-chrome/Default/Cache");
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join("entry"), b"cached-single-entry").unwrap();

    let output = run_scan(root.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("across 1 finding and 0 duplicate sets."),
        "expected 'across 1 finding and 0 duplicate sets.', got: {stdout}"
    );
    assert!(
        stdout.contains("1 finding ·"),
        "expected '1 finding ·', got: {stdout}"
    );
}
