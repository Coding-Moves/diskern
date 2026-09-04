# Changelog

All notable changes to Diskern are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Licensed under MIT

### Added

- Automated RUSTSEC auditing: a weekly `cargo audit` that files and closes
  a labelled tracking issue, and an auto-fix workflow that opens a
  dependency-update PR when an advisory lands
- Community docs: contributing guide, code of conduct, security policy
- Per-section READMEs and an architecture overview

## [0.1.0]

Initial development version.

- `diskern-core`: parallel scanner, BLAKE3 dedup, deterministic rules
  database, evidence-based risk scoring, quarantine-only actions
- `diskern-cli`: `diskern scan <dir>` with summary and `--json` output
- Desktop app: Tauri v2 + React with live scan progress and signed
  auto-updates
