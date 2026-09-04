# Changelog

All notable changes to Diskern are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Cancel a running scan from the desktop app
- `diskern scan` prints the findings themselves — grouped by verdict and
  category, with `--top` to cap each group and `--verdict` to filter
- Rules for logs and installers, so both categories are reachable
- CI type-checks the Tauri crate whenever the app or the engine changes,
  instead of leaving app breakage to surface at release time
- Automated RUSTSEC auditing: a weekly `cargo audit` that files and closes
  a labelled tracking issue, and an auto-fix workflow that opens a
  dependency-update PR when an advisory lands
- Community docs: contributing guide, code of conduct, security policy
- Per-section READMEs and an architecture overview

### Changed

- Licensed under MIT

## [0.1.0]

Initial development version.

- `diskern-core`: parallel scanner, BLAKE3 dedup, deterministic rules
  database, evidence-based risk scoring, quarantine-only actions
- `diskern-cli`: `diskern scan <dir>` with summary and `--json` output
- Desktop app: Tauri v2 + React with live scan progress and signed
  auto-updates
