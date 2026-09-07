use serde_json::json;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

fn run_scan(root: &std::path::Path, rules: Option<&std::path::Path>) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_diskern"));
    command.arg("scan").arg(root).arg("--top").arg("0");
    if let Some(rules) = rules {
        command.arg("--rules").arg(rules);
    }
    command.output().expect("diskern should start")
}

fn write_rules(path: &std::path::Path, pattern: &str) {
    let rules = json!({
        "version": 1,
        "rules": [{
            "id": "test-rule",
            "patterns": [pattern],
            "category": "browser_cache",
            "verdict": "safe",
            "description": "Rule loaded from the test file."
        }]
    });
    fs::write(path, serde_json::to_vec(&rules).unwrap()).unwrap();
}

#[test]
fn scan_without_rules_uses_embedded_database() {
    let root = tempdir().unwrap();
    let cache = root.path().join(".cache/google-chrome/Default/Cache");
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join("entry"), b"cached").unwrap();

    let output = run_scan(root.path(), None);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("matched rule chrome-cache"), "{stdout}");
}

#[test]
fn scan_accepts_and_uses_external_rules_file() {
    let root = tempdir().unwrap();
    fs::write(root.path().join("sample.custom"), b"custom").unwrap();
    let rules = root.path().join("rules.json");
    write_rules(&rules, "**/*.custom");

    let output = run_scan(root.path(), Some(&rules));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Rules: external database"), "{stdout}");
    assert!(stdout.contains("matched rule test-rule"), "{stdout}");
    assert!(
        stdout.contains("Rule loaded from the test file."),
        "{stdout}"
    );
}

#[test]
fn missing_rules_file_fails_clearly() {
    let root = tempdir().unwrap();
    let missing = root.path().join("missing.json");
    let output = run_scan(root.path(), Some(&missing));

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("could not read rules file"), "{stderr}");
    assert!(stderr.contains("missing.json"), "{stderr}");
}

#[test]
fn malformed_rules_file_fails_clearly() {
    let root = tempdir().unwrap();
    let rules = root.path().join("malformed.json");
    fs::write(&rules, b"{ not json }").unwrap();
    let output = run_scan(root.path(), Some(&rules));

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("could not parse rules file"), "{stderr}");
    assert!(stderr.contains("malformed.json"), "{stderr}");
}

#[test]
fn invalid_glob_in_rules_file_fails_before_scan() {
    let root = tempdir().unwrap();
    let rules = root.path().join("invalid-glob.json");
    write_rules(&rules, "[");

    let output = run_scan(root.path(), Some(&rules));

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("could not validate rules file"), "{stderr}");
    assert!(stderr.contains("invalid glob pattern"), "{stderr}");
    assert!(stderr.contains("test-rule"), "{stderr}");
}

#[test]
fn external_rules_cannot_shadow_embedded_protected_rules() {
    let root = tempdir().unwrap();
    let protected_path = root.path().join("windows/installer/setup.msi");
    fs::create_dir_all(protected_path.parent().unwrap()).unwrap();
    fs::write(&protected_path, b"installer").unwrap();
    let rules = root.path().join("rules.json");
    write_rules(&rules, "**/*.msi");

    let output = run_scan(root.path(), Some(&rules));

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("matched rule windows-installer-cache"),
        "{stdout}"
    );
    assert!(!stdout.contains("matched rule test-rule"), "{stdout}");
    assert!(stdout.contains("Protected — do not touch"), "{stdout}");
}
