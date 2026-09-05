# Diskern desktop app

Tauri v2 desktop app: React frontend ([`src/`](src)) over the Rust engine
([`src-tauri/`](src-tauri)).

## Develop

Needs the [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/)
(Rust toolchain + platform WebView deps) — see
[docs/DEVELOPMENT.md](../docs/DEVELOPMENT.md) for the per-platform list
and the errors you get without them. Then:

```sh
npm install
npm run tauri dev
```

`npm run build` compiles the frontend alone, without touching Rust,
which is enough to check a JSX change.

## How it talks to the engine

The frontend never touches the filesystem itself. It invokes Tauri
commands defined in [`src-tauri/src/commands.rs`](src-tauri/src/commands.rs),
which call into [`diskern-core`](../crates/diskern-core). During a scan
the backend emits `scan-progress` events (~every 150ms) so the UI can show
a live file counter.

| Command | Does |
| --- | --- |
| `start_scan` | Read-only scan; returns a report, or `null` if cancelled |
| `cancel_scan` | Stops the scan in flight |
| `quarantine_finding` | Moves one file to quarantine, re-classifying server-side first |
| `list_quarantine` | Everything currently quarantined, read from the manifest |
| `restore_quarantined` | Puts one file back where it came from |
| `purge_quarantine` | Empties quarantine for good — the only deletion in the app |

Everything except `quarantine_finding` and `purge_quarantine` is
read-only. Quarantine is manifest-backed, so what was moved in one
session is still restorable in the next; the Quarantine panel renders
before any scan has been run for exactly that reason.

## Updater

Release builds auto-update via Tauri's updater plugin, verified against
the public key in [`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json).
See [docs/RELEASING.md](../docs/RELEASING.md) for the signing setup.
