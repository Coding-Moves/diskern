//! Issue #73. Every other test in the workspace sits next to the module it
//! covers; this file drives the compiled `diskern` binary the way a user
//! runs it, so the behaviour only the binary owns — argument errors, exit
//! codes, the verdict ordering of the printed report, and the `--json`
//! contract other tools consume — is pinned end to end.
//!
//! The sibling files use the same harness (`CARGO_BIN_EXE_diskern` plus a
//! `tempdir` fixture); these tests add what none of them reach.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::tempdir;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_diskern"))
        .args(args)
        .output()
        .expect("diskern should start")
}

fn scan(root: &Path, flags: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_diskern"));
    command.arg("scan").arg(root);
    for flag in flags {
        command.arg(flag);
    }
    command.output().expect("diskern should start")
}

/// Byte offset of `needle` in `haystack` — ordering assertions are only as
/// readable as the panic they produce, so a miss dumps the whole report.
fn at(haystack: &str, needle: &str) -> usize {
    haystack
        .find(needle)
        .unwrap_or_else(|| panic!("expected {needle:?} in:\n{haystack}"))
}

/// One file per verdict, so the ordering assertions see all four headings.
/// Every path leans on a `**/`-anchored embedded rule, which classifies the
/// same wherever the platform puts a tempdir. Under `/tmp` (Linux) and
/// `AppData/Local/Temp` (Windows) the catch-all `temp-dirs` rule also claims
/// `package.json` — that only adds a second review finding and changes none
/// of the checks; on macOS it drops out of the report entirely.
fn write_verdict_fixture(root: &Path) {
    let cache = root.join(".cache/google-chrome/Default/Cache");
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join("data_0"), vec![0u8; 4096]).unwrap(); // safe

    let downloads = root.join("downloads");
    fs::create_dir_all(&downloads).unwrap();
    fs::write(downloads.join("setup.dmg"), vec![0u8; 2048]).unwrap(); // review

    // A smaller review finding in a second category, so category ordering
    // inside the verdict is exercised too.
    let target = root.join("proj/target/debug");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("app.o"), vec![0u8; 256]).unwrap(); // review

    // A node_modules a live project references is downgraded to risky.
    let store = root.join("live/node_modules/react");
    fs::create_dir_all(&store).unwrap();
    fs::write(root.join("live/package.json"), b"{}").unwrap();
    fs::write(store.join("index.js"), vec![0u8; 512]).unwrap(); // risky

    let installer_cache = root.join("windows/installer");
    fs::create_dir_all(&installer_cache).unwrap();
    fs::write(installer_cache.join("cached.msi"), vec![0u8; 1024]).unwrap(); // protected
}

#[test]
fn scan_prints_findings_grouped_by_verdict_safest_first() {
    let root = tempdir().unwrap();
    write_verdict_fixture(root.path());

    let output = scan(root.path(), &["--top", "0"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let safe = at(&stdout, "Safe to remove");
    let review = at(&stdout, "Review first");
    let risky = at(&stdout, "Risky — not recommended");
    let protected = at(&stdout, "Protected — do not touch");
    assert!(
        safe < review && review < risky && risky < protected,
        "verdict headings out of order:\n{stdout}"
    );

    // Each fixture file lands under the heading its verdict earned — the
    // regression this file exists for is a finding printed under the wrong
    // one.
    let cache_row = at(&stdout, "data_0");
    assert!(
        safe < cache_row && cache_row < review,
        "safe finding outside its section:\n{stdout}"
    );

    let review_section = &stdout[review..risky];
    assert!(
        at(review_section, "Installers") < at(review_section, "Build artifacts"),
        "categories not ordered by reclaimable subtotal:\n{review_section}"
    );
    assert!(
        at(review_section, "setup.dmg") > at(review_section, "Installers"),
        "review finding outside its section:\n{review_section}"
    );

    let risky_section = &stdout[risky..protected];
    assert!(
        at(risky_section, "index.js") < at(risky_section, "referenced by 1 project"),
        "risky finding missing its evidence:\n{risky_section}"
    );

    let protected_section = &stdout[protected..];
    assert!(
        protected_section.contains("cached.msi"),
        "protected finding outside its section:\n{protected_section}"
    );
}

#[test]
fn scan_json_emits_a_parseable_report_with_the_promised_fields() {
    let root = tempdir().unwrap();
    write_verdict_fixture(root.path());

    let output = scan(root.path(), &["--json"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // The whole of stdout must be one JSON document — tools reading this
    // output break on a stray log line, not just on a missing field.
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be a single JSON document");

    assert_eq!(report["files_scanned"].as_u64(), Some(6));
    assert!(report["total_reclaimable"].is_u64());
    assert!(report["duplicate_sets"].is_array());

    let findings = report["findings"]
        .as_array()
        .expect("findings should be an array");

    // Findings arrive sorted by reclaimable bytes, largest first.
    let reclaimables: Vec<u64> = findings
        .iter()
        .map(|f| f["reclaimable"].as_u64().expect("reclaimable is u64"))
        .collect();
    assert!(
        reclaimables.windows(2).all(|w| w[0] >= w[1]),
        "findings not sorted by reclaimable: {reclaimables:?}"
    );

    // Each fixture file surfaces with the verdict and category its rule
    // promises. `package.json` is deliberately absent from the table: whether
    // a tempdir-catch-all rule claims it depends on where the platform puts
    // tempdirs, so it is a finding on Linux and Windows and dropped on macOS.
    let expected: [(&str, &str, &str); 5] = [
        (
            ".cache/google-chrome/Default/Cache/data_0",
            "safe",
            "browser_cache",
        ),
        ("downloads/setup.dmg", "review", "installer"),
        ("proj/target/debug/app.o", "review", "build_artifact"),
        (
            "live/node_modules/react/index.js",
            "risky",
            "build_artifact",
        ),
        (
            "windows/installer/cached.msi",
            "protected",
            "system_critical",
        ),
    ];
    for (suffix, verdict, category) in expected {
        let finding = findings
            .iter()
            .find(|f| {
                Path::new(f["entry"]["path"].as_str().unwrap_or_default())
                    .ends_with(Path::new(suffix))
            })
            .unwrap_or_else(|| panic!("no finding for {suffix}: {findings:?}"));
        assert_eq!(finding["verdict"], verdict, "wrong verdict for {suffix}");
        assert_eq!(finding["category"], category, "wrong category for {suffix}");
        assert!(
            finding["reasons"].is_array(),
            "finding for {suffix} carries no reasons array"
        );
    }

    // One finding checked end to end: path, size, verdict, and the rule
    // evidence a user — or a tool — reads to trust the verdict.
    let cache = findings
        .iter()
        .find(|f| {
            Path::new(f["entry"]["path"].as_str().unwrap_or_default())
                .ends_with(Path::new(".cache/google-chrome/Default/Cache/data_0"))
        })
        .expect("a finding for the chrome cache fixture");
    assert_eq!(cache["entry"]["size"].as_u64(), Some(4096));
    assert_eq!(cache["reclaimable"].as_u64(), Some(4096));
    assert!(
        cache["reasons"]
            .as_array()
            .expect("reasons should be an array")
            .iter()
            .any(|r| r
                .as_str()
                .unwrap_or_default()
                .starts_with("matched rule chrome-cache")),
        "finding carries no rule evidence: {cache}"
    );
}

#[test]
fn argument_errors_exit_2_and_write_no_report() {
    // clap owns these failures: the run never reaches the engine, so the
    // contract is a usage error on stderr, nothing on stdout, exit 2.
    let cases: [&[&str]; 4] = [
        &[],                                  // no subcommand
        &["scan"],                            // missing required roots
        &["scan", ".", "--verdict", "bogus"], // value outside the enum
        &["defragment"],                      // unknown subcommand
    ];
    for args in cases {
        let output = run(args);
        assert_eq!(
            output.status.code(),
            Some(2),
            "args {args:?} should exit 2, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.stdout.is_empty(),
            "args {args:?} wrote a report to stdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(!output.stderr.is_empty(), "args {args:?} failed silently");
    }
}

#[test]
fn a_scan_failure_exits_1() {
    // Distinct from clap's 2: the invocation was valid, the run failed.
    let root = tempdir().unwrap();
    let missing = root.path().join("missing");

    let output = scan(&missing, &[]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
