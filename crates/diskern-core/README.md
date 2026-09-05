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
scanner ──► index ──► dedup ──► graph ──► rules + risk ──► report
                                                  │
                                       (optional) ai narration
```

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
