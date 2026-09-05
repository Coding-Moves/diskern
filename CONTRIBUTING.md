# Contributing to Diskern

Thanks for your interest in improving Diskern! Contributions of all kinds
are welcome — bug reports, docs, rules for the safety database, and code.

**New here?** [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) gets the project
running on your machine, including what to install and what the build
errors mean. This file is about what to work on and how to land it.

## Ground rules

Diskern's [safety principles](README.md#principles-non-negotiable) are
non-negotiable. PRs that make scanning mutate state, hard-delete files,
or let a model or network call decide a safety verdict will not be
accepted.

Two consequences worth stating plainly, because they come up:

- **A wrong `safe` verdict is a bug of a different class.** `safe` and
  `review` are both *actionable* in the app — the UI offers to quarantine
  either. A rule that matches more than it means doesn't produce a
  cosmetic error, it offers to move someone's data. When in doubt use
  `review`, and when still in doubt leave it `unknown`: `report::build`
  drops unknown entries, so they never reach the user as an actionable
  row.
- **The engine decides, the frontends display.** The CLI and the app both
  call the same functions in `diskern-core` and neither contains
  scanning or classification logic. A new capability goes into the engine
  first, with tests, and the frontends render what it returns.

## Good first contributions

**Rules for the safety database.** This is the most useful thing you can
do and the easiest to start on. Diskern only knows what its rules tell
it, and the shipped set is deliberately tiny — it covers Chrome,
Firefox, pip, Rust and Node build output, and a handful of system
directories. Everything else on your disk classifies as `unknown` and is
dropped from the report.

If you know where an application on your platform keeps its regenerable
cache, that is a rule nobody else can write as well as you. Format,
verdict levels and the safety reasoning are in
[docs/RULES.md](docs/RULES.md); the rules themselves are JSON in
[`crates/diskern-core/rules/base.json`](crates/diskern-core/rules/base.json),
and adding one needs no Rust.

**Issues labelled `good first issue`** are scoped to a single file with
the reasoning already worked out.

**Documentation.** If something here or in `docs/` was wrong or missing
when you followed it, that is a bug report worth filing even if you
don't fix it.

## The loop

1. Fork, and branch from `main`. Branch names follow the commit prefixes
   below: `fix/…`, `feat/…`, `docs/…`, `ci/…`.
2. Make the change, with a test that fails without it.
3. Run [what CI runs](docs/DEVELOPMENT.md#5-running-the-tests).
4. Open the PR.

### Commits

`type(scope): summary in the imperative`, where type is `feat`, `fix`,
`docs`, `test`, `refactor`, `perf`, `style`, `chore` or `ci`, and scope
is usually the module or crate (`rules`, `scanner`, `app`, `cli`).

One logical change per commit. The body matters more than the summary:
say what was wrong and why the fix is the right shape, not what the diff
already shows. `git log` in this repo is the reference — the useful
messages explain a decision someone would otherwise have to re-derive.

### Pull requests

The [template](.github/PULL_REQUEST_TEMPLATE.md) asks for what and why.
Include the reasoning you'd want if you were reviewing it cold: what
breaks without the change, what you considered and rejected, and
anything you're unsure about — an explicit "I'm not certain this is the
right layer" gets a better review than silence.

If your change alters a verdict, say so in the description. Verdicts are
what the app acts on, so a rule or risk change is a behaviour change
even when the diff looks like data.

Before you open it:

1. `cargo fmt --all`
2. `cargo clippy -p diskern-core -p diskern-cli --all-targets -- -D warnings`
3. `cargo test -p diskern-core -p diskern-cli`
4. Update [CHANGELOG.md](CHANGELOG.md) under `Unreleased` if the change
   is user-visible.

`--workspace` instead of `-p …` also builds the desktop app, which needs
the [platform webview
dependencies](docs/DEVELOPMENT.md#1-what-you-need). CI compiles the app
separately, so scoping to the two crates locally is fine.

## Where things go

| You want to… | Goes in |
| --- | --- |
| Teach Diskern about a new cache or artifact directory | [`rules/base.json`](crates/diskern-core/rules/base.json) |
| Change how a verdict is decided | `crates/diskern-core/src/{rules,risk,graph}.rs` |
| Change what a scan finds or how it reports | `crates/diskern-core/src/{scanner,dedup,report}.rs` |
| Change what quarantine does | `crates/diskern-core/src/actions.rs` — the only module that writes |
| Add a terminal flag or change CLI output | `crates/diskern-cli/src/main.rs` |
| Change the app's UI | `app/src/` |
| Expose engine behaviour to the app | `app/src-tauri/src/commands.rs`, then `app/src/` |

[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) walks a scan through the
engine stage by stage if you want the fuller picture first.

## Reporting bugs

Open an issue with your OS, the command or app action you ran, and what
you expected versus what happened. For a wrong verdict, the path that
was misclassified and what it actually is are the two things that make
it fixable — a `diskern scan <dir> --json` excerpt is ideal.

For security issues, see [SECURITY.md](SECURITY.md) — please don't open
a public issue.
