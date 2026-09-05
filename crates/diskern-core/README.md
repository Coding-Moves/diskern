# diskern-core

The Diskern engine: a pure Rust library with no UI and no network access
(except the optional `ai` feature). Consumed by the CLI, the desktop app,
and tests alike.

## Guarantees

- **Read-only by default.** Nothing here modifies user files except the
  [`actions`](src/actions.rs) module, which only quarantines — never
  hard-deletes.
- **Deterministic verdicts.** Safety classification comes from the rules
  database + risk model, never from a model or a network call.

## Pipeline

```text
scanner ──► graph ──► rules + risk ──► dedup ──► report
                                                   │
                                        (optional) ai narration
```

Dedup runs *after* classification: a duplicate set is an offer to keep
one copy and drop the rest, so entries nothing will act on have no place
in one — and hashing them is the most expensive way to produce a number
nobody can use. [`report::build_with`](src/report.rs) is the whole
sequence in one function.

## Modules

| Module                       | Purpose                                                |
| ---------------------------- | ------------------------------------------------------ |
| [`scanner`](src/scanner.rs)  | Parallel filesystem walk (jwalk) with live progress    |
| [`dedup`](src/dedup.rs)      | Duplicate detection — BLAKE3, size-collision gated     |
| [`graph`](src/graph.rs)      | Reference graph — which projects depend on which stores |
| [`rules`](src/rules.rs)      | Deterministic rules DB ([`rules/base.json`](rules/base.json)) |
| [`risk`](src/risk.rs)        | Turns rule matches + evidence into a verdict           |
| [`report`](src/report.rs)    | Aggregates findings into a serializable report         |
| [`actions`](src/actions.rs)  | Quarantine + manifest (move, restore, purge) — the only mutator |
| [`ai`](src/ai.rs)            | Optional narration layer (`--features ai`)             |

## Develop

```sh
cargo test -p diskern-core
```
