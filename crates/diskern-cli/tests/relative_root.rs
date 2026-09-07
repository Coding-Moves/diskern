//! Issue #103. A relative scan root produced entries no anchored rule could
//! match, so the scan reported nothing to clean for a directory full of
//! matches. Driving the real binary with a working directory is the only
//! honest way to pin this: changing the current directory inside a test would
//! race every other test in the same binary.

use serde_json::json;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

/// A rule anchored at the filesystem root, like the shipped `/tmp/**` and
/// `/var/log/**`. Windows paths normalize to `c:/...`, so a `/`-anchored
/// pattern cannot match there and neither can the bug.
#[cfg(unix)]
fn write_anchored_rules(path: &std::path::Path) {
    let rules = json!({
        "version": 1,
        "rules": [{
            "id": "anchored-marker",
            "patterns": ["/**/*.marker"],
            "category": "temp_file",
            "verdict": "review",
            "description": "Anchored rule for the relative-root regression."
        }]
    });
    fs::write(path, serde_json::to_vec(&rules).unwrap()).unwrap();
}

#[cfg(unix)]
#[test]
fn a_relative_root_still_reaches_anchored_rules() {
    let root = tempdir().unwrap();
    fs::write(root.path().join("scratch.marker"), b"temp").unwrap();
    let rules = root.path().join("rules.json");
    write_anchored_rules(&rules);

    // "." as the root, resolved against the child's working directory.
    let output = Command::new(env!("CARGO_BIN_EXE_diskern"))
        .current_dir(root.path())
        .args(["scan", ".", "--rules", "rules.json", "--top", "0"])
        .output()
        .expect("diskern should start");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("matched rule anchored-marker"), "{stdout}");
    // The printed path is the one the manifest would record on quarantine.
    assert!(stdout.contains("/scratch.marker"), "{stdout}");
}

#[cfg(unix)]
#[test]
fn an_absolute_root_reaches_the_same_rule() {
    let root = tempdir().unwrap();
    fs::write(root.path().join("scratch.marker"), b"temp").unwrap();
    let rules = root.path().join("rules.json");
    write_anchored_rules(&rules);

    let output = Command::new(env!("CARGO_BIN_EXE_diskern"))
        .args([
            "scan",
            &root.path().to_string_lossy(),
            "--rules",
            &rules.to_string_lossy(),
            "--top",
            "0",
        ])
        .output()
        .expect("diskern should start");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("matched rule anchored-marker"), "{stdout}");
}
