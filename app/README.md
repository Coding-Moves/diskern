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
| `quarantine_finding` | Moves one exact report finding to quarantine; the backend owns the verdict and rejects stale or superseded report state |
| `list_quarantine` | Everything currently quarantined, read from the manifest |
| `restore_quarantined` | Puts one file back where it came from |
| `purge_quarantine` | Empties quarantine for good — the only deletion in the app |

Everything except `quarantine_finding` and `purge_quarantine` is
read-only. Quarantine is manifest-backed, so what was moved in one
session is still restorable in the next; the Quarantine panel renders
before any scan has been run for exactly that reason. A completed scan is the
user-review snapshot for quarantine: starting or cancelling another scan
invalidates that snapshot, and a new scan is required before acting again.

## Quarantine history

Quarantine history shows 50 records per page, including while the section is
collapsed. First, Previous, Next, and Last controls keep every record reachable
without mounting the full history. Restoring a record refreshes the manifest;
if the current page disappears, the panel returns to the last populated page.
Page controls and competing actions pause while a restore or purge is running.

The section count and purge confirmation cover the entire quarantine history.
Purge deletes every file listed in the history, including files on other pages. Pagination
only limits mounted UI rows; the backend still returns the complete manifest.

## Frontend tests

Run `npm test` with Node 22.22.2+, 24.15.0+, or a newer supported LTS release.
The quarantine interaction tests use jsdom and Vite's JSX loader with mocked
Tauri commands; they do not restore or delete real files. A 10,000-record
fixture checks all 200 pages, verifies a maximum of 50 mounted rows, and covers
later-page restore, page adjustment, pending/failed actions, and full-history
purge confirmation. `npm run build` checks the production frontend bundle.

## Updater

Release builds auto-update via Tauri's updater plugin, verified against
the public key in [`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json).
See [docs/RELEASING.md](../docs/RELEASING.md) for the signing setup.
