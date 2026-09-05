# Changelog

All notable changes to Diskern are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Cancel a running scan from the desktop app
- `diskern scan` prints the findings themselves — grouped by verdict and
  category, with `--top` to cap each group and `--verdict` to filter
- Rules for logs and installers, so both categories are reachable, with the
  Windows Installer cache explicitly protected from them
- CI type-checks the Tauri crate whenever the app or the engine changes,
  instead of leaving app breakage to surface at release time
- Automated RUSTSEC auditing: a weekly `cargo audit` that files and closes
  a labelled tracking issue, and an auto-fix workflow that opens a
  dependency-update PR when an advisory lands
- Community docs: contributing guide, code of conduct, security policy
- Per-section READMEs and an architecture overview
- Quarantine keeps a manifest, so a quarantined file can be restored
  after the app is closed. The desktop app gained a Quarantine panel with
  per-file restore and an explicit purge
- The impact graph reaches the report: a dependency store a live project
  depends on is now marked more cautiously than an abandoned one, with
  "referenced by N projects" as the evidence

### Changed

- Licensed under MIT
- Rule patterns are globs rather than substrings, so a rule stays inside
  the directory it names. The Firefox rule covers `cache2` rather than
  the whole profile, and `/tmp` no longer matches `~/tmp`
- Excludes are matched on whole path components after normalization, so
  `/run` stops excluding `/runtime-data` and a differently-cased Windows
  root still matches
- `min_file_size` moved from `ScanOptions` to `ReportOptions` as
  `dedup_min_size`: it is a dedup knob and was hiding small files from
  the whole report
- The reclaimable headline no longer counts the same bytes twice, and no
  longer counts bytes on files the app refuses to act on
- `cargo audit` ignores seventeen unfixable transitive advisories from
  Tauri's Linux GTK3 stack, listed with a reason each in
  `.cargo/audit.toml`

### Fixed

- Restoring a quarantined file across filesystems no longer fails with
  `EXDEV`
- Restore refuses when something is already at the original path, rather
  than overwriting it — quarantine a cache, keep using the app, change
  your mind, and the file the app rebuilt survives
- A file whose path can't be recorded in the manifest is not moved at
  all, instead of being moved and then losing its record
- The scan progress ticker thread stops from a guard, so a panicking or
  cancelled scan can't leave it emitting for the rest of the process
- Two files that flatten to the same quarantine name no longer overwrite
  each other

## [0.1.0]

Initial development version.

- `diskern-core`: parallel scanner, BLAKE3 dedup, deterministic rules
  database, evidence-based risk scoring, quarantine-only actions
- `diskern-cli`: `diskern scan <dir>` with summary and `--json` output
- Desktop app: Tauri v2 + React with live scan progress and signed
  auto-updates
