# Architecture

Diskern is a Cargo workspace with one engine and two frontends, plus a
marketing site.

```text
┌─────────────┐   ┌──────────────────┐
│ diskern-cli │   │ app/ (Tauri v2)  │   two thin frontends
└──────┬──────┘   └────────┬─────────┘
       │                   │
       └───────┬───────────┘
               ▼
      ┌─────────────────┐
      │  diskern-core   │   the engine: all logic lives here
      └─────────────────┘
```

## Where things live

| Path                  | What it is                                              |
| --------------------- | ------------------------------------------------------- |
| `crates/diskern-core` | Engine: scanner, dedup, rules, risk, graph, quarantine  |
| `crates/diskern-cli`  | `diskern` binary — engine from the terminal             |
| `app/`                | Tauri v2 desktop app (React frontend, Rust backend)     |
| `site/`               | Landing page, deployed to GitHub Pages                  |
| `docs/`               | Project docs — [running locally](DEVELOPMENT.md), [rules](RULES.md), [releasing](RELEASING.md) |
| `.github/workflows/`  | CI, release builds, Pages deploy                        |

## A scan, end to end

One function orchestrates almost all of it:
[`report::build_with`](../crates/diskern-core/src/report.rs). Reading it
alongside this section is the fastest way into the engine.

```text
scanner ──► graph ──► rules + risk ──► dedup ──► report
```

**1. Walk.** [`scanner::scan`](../crates/diskern-core/src/scanner.rs)
walks the roots in parallel with `jwalk`, skipping excluded directories,
and returns a `FileEntry` per file: path, size, modified and accessed
times, whether it is a symlink. Metadata only — nothing is read or
hashed here, and nothing is written ever.

**2. Graph.**
[`graph::ImpactGraph::from_entries`](../crates/diskern-core/src/graph.rs)
makes one pass looking for two things: directories holding a project
marker (`Cargo.toml`, `package.json`, `pyproject.toml`), and directories
that are dependency stores (`target`, `node_modules`, a virtualenv). It
links each project to the store it owns, so the engine can later answer
"how many live projects reference this?".

**3. Classify.** [`rules::RulesDb::classify`](../crates/diskern-core/src/rules.rs)
matches the normalized path against the rule globs — first match wins,
which is why `protected` rules are listed first — yielding a `Category`
and a base `Verdict`. [`risk::downgrade`](../crates/diskern-core/src/risk.rs)
then applies the graph's answer. Evidence can only make a verdict *more*
cautious, never less, and `Protected` is final.

**4. Dedup.** [`dedup::find_duplicates_filtered`](../crates/diskern-core/src/dedup.rs)
buckets by size, BLAKE3-hashes only the files whose sizes collide (which
skips most of a real disk), then buckets by hash. It runs after
classification so that entries nothing will act on can sit it out — they
have no place in an offer to keep one copy and drop the rest, and
hashing them is the most expensive way to produce an unusable number.

**5. Report.** [`risk::assess`](../crates/diskern-core/src/risk.rs) adds
an informational score and per-file evidence, and each entry becomes a
`Finding` carrying its category, verdict, reclaimable bytes and the
`reasons` that justify them. The headline total counts findings in full
and adds only the duplicate copies nothing has counted yet.

Acting on a finding is a separate call:
[`actions::quarantine`](../crates/diskern-core/src/actions.rs) is the
only function in the crate that writes, it refuses `Risky` and
`Protected`, and it records every move in a manifest so a restore
survives the process exiting.

## Adding a feature

Work out which layer owns it before writing anything — the answer is
usually further down than it first looks.

**Does a rule cover it?** Teaching Diskern that some directory is a cache
is data, not code: add it to
[`rules/base.json`](../crates/diskern-core/rules/base.json) with a test.
No Rust, no new code paths, and it reaches both frontends at once.

**Does it change a verdict?** Then it belongs in `rules`, `risk` or
`graph`, and the constraint in [design decisions](#design-decisions)
applies: evidence may only make a verdict more cautious. Add the evidence
as a `reason` too — a verdict the user can't see the basis for is the
thing Diskern exists not to ship.

**Does it change what a scan finds or reports?** `scanner`, `dedup` and
`report`. Watch the cancellation flag: anything that loops over every
entry has to check it, or a Cancel arriving during that stage does
nothing.

**Only then, the frontends.** Add a Tauri command in
[`commands.rs`](../app/src-tauri/src/commands.rs) and register it in
[`lib.rs`](../app/src-tauri/src/lib.rs), or a flag in
[`main.rs`](../crates/diskern-cli/src/main.rs). Both should be thin
enough that the interesting part of your change is already tested in the
engine before either sees it.

## Design decisions

**Frontends are thin.** The CLI and the app both call the same engine
functions; neither contains scanning or classification logic. Any new
capability goes into `diskern-core` first.

**Safety is layered, not sprinkled.** Read-only scanning, quarantine
instead of deletion, and deterministic rule-based verdicts are enforced in
the engine (see the [core README](../crates/diskern-core/README.md)), so
no frontend can accidentally weaken them.

**AI is narration-only.** The optional `ai` feature explains findings in
plain language; it can never change a verdict. This keeps the engine fully
auditable and offline-capable.
