# Contributing to Diskern

Thanks for your interest in improving Diskern! Contributions of all kinds
are welcome — bug reports, docs, rules for the safety database, and code.

## Ground rules

Diskern's [safety principles](README.md#principles-non-negotiable) are
non-negotiable. PRs that make scanning mutate state, hard-delete files, or
let a model/network call decide a safety verdict will not be accepted.

## Getting started

```sh
# Engine + CLI — no GUI dependencies needed
cargo test -p diskern-core
cargo run -p diskern-cli -- scan ~/Downloads

# Desktop app (needs Tauri v2 prerequisites)
cd app && npm install && npm run tauri dev
```

See each section's README for details:
[`crates/diskern-core`](crates/diskern-core/README.md) ·
[`crates/diskern-cli`](crates/diskern-cli/README.md) ·
[`app`](app/README.md) · [`site`](site/README.md)

## Before you open a PR

1. `cargo fmt --all` — CI enforces formatting.
2. `cargo clippy --workspace` — fix new warnings.
3. `cargo test --workspace` — all tests green.
4. Keep commits small and focused; one logical change per commit.

CI runs the same checks on every PR, plus a spell checker
([typos](https://github.com/crate-ci/typos), config in `_typos.toml`),
a markdown link checker, and lint/build of the site and app frontends.
A weekly audit workflow additionally scans dependencies for RustSec
advisories and docs for dead external links.

## Adding safety rules

Rules live in [`crates/diskern-core/rules/base.json`](crates/diskern-core/rules/base.json)
and are the main way to contribute — see [docs/RULES.md](docs/RULES.md)
for the format, verdict levels, and guidelines. Prefer conservative
verdicts: when in doubt, use `review` rather than `safe`.

## Reporting bugs

Open an issue with your OS, the command or app action you ran, and what
you expected vs. what happened. For security issues, see
[SECURITY.md](SECURITY.md) — please don't open a public issue.
