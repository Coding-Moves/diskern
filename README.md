# Diskern

[![CI](https://github.com/Coding-Moves/diskern/actions/workflows/ci.yml/badge.svg)](https://github.com/Coding-Moves/diskern/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

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

Project docs live in [docs/](docs/README.md) — architecture, the
[rules database](docs/RULES.md), an [FAQ](docs/FAQ.md), and release
instructions. Each section has its own README:
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

[MIT](LICENSE). Unless you explicitly state otherwise, any contribution
you intentionally submit for inclusion in this work shall be licensed as
MIT, without any additional terms or conditions. 


