# Changelog

All notable changes to Diskern are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Relicensed from MIT OR Apache-2.0 to MIT only

### Added

- Dual MIT OR Apache-2.0 licensing with in-repo license texts
- Community docs: contributing guide, code of conduct, security policy
- Per-section READMEs and an architecture overview

## [0.1.0]

Initial development version.

- `diskern-core`: parallel scanner, BLAKE3 dedup, deterministic rules
  database, evidence-based risk scoring, quarantine-only actions
- `diskern-cli`: `diskern scan <dir>` with summary and `--json` output
- Desktop app: Tauri v2 + React with live scan progress and signed
  auto-updates
