# Diskern

[![CI](https://github.com/Coding-Moves/diskern/actions/workflows/ci.yml/badge.svg)](https://github.com/Coding-Moves/diskern/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](README.md#license)

**Understand your disk before you clean it.**

Diskern scans your computer, understands what every file is *for*, and
safely frees up space — explaining exactly what's safe to remove and what
would break if you did.

## Principles (non-negotiable)

1. **Read-only by default.** Scanning never modifies anything.
2. **Never hard-deletes.** Actions move files to a reviewable quarantine;
   purging quarantine is a separate, explicit user step.
3. **Rules decide, AI narrates.** Safety verdicts come from a deterministic,
   auditable rules database and local evidence — never from a model or a
   network call. The optional AI layer only explains, in plain language,
   what the engine already found.
4. **Evidence over confidence.** Every finding shows *why* ("matched rule
   chrome-cache", "referenced by 3 projects"), not just a percentage.

## Layout

```
crates/diskern-core   engine: scanner, dedup (BLAKE3), rules, risk, graph, quarantine
crates/diskern-cli    `diskern scan <dir>` — the engine from the terminal
app/                  Tauri v2 desktop app (React frontend)
site/                 landing page (GitHub Pages)
docs/                 architecture and release docs
```

Each section has its own README:
[core](crates/diskern-core/README.md) ·
[cli](crates/diskern-cli/README.md) ·
[app](app/README.md) ·
[site](site/README.md)

## Development

```sh
# Engine + CLI (no GUI deps needed)
cargo test -p diskern-core
cargo run -p diskern-cli -- scan ~/Downloads

# Desktop app (needs Tauri v2 prerequisites: https://v2.tauri.app/start/prerequisites/)
cd app && npm install && npm run tauri dev
```

## Releasing

See [docs/RELEASING.md](docs/RELEASING.md). Short version: set the two
updater-key secrets once, then `git tag vX.Y.Z && git push --tags`.

## Contributing

Contributions are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md) for
setup and guidelines, and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
for how the pieces fit together.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in this work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions. 


