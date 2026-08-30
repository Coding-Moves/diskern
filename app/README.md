# Diskern desktop app

Tauri v2 desktop app: React frontend ([`src/`](src)) over the Rust engine
([`src-tauri/`](src-tauri)).

## Develop

Needs the [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/)
(Rust toolchain + platform WebView deps), then:

```sh
npm install
npm run tauri dev
```

## How it talks to the engine

The frontend never touches the filesystem itself. It invokes Tauri
commands defined in [`src-tauri/src/commands.rs`](src-tauri/src/commands.rs),
which call into [`diskern-core`](../crates/diskern-core). During a scan the
backend emits `scan-progress` events (~every 150ms) so the UI can show a
live file counter.

## Updater

Release builds auto-update via Tauri's updater plugin, verified against
the public key in [`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json).
See [docs/RELEASING.md](../docs/RELEASING.md) for the signing setup.
