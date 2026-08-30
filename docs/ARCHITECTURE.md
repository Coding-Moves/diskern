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
| `docs/`               | Project docs (this file, [RELEASING.md](RELEASING.md))  |
| `.github/workflows/`  | CI, release builds, Pages deploy                        |

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
