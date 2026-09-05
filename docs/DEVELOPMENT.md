# Running Diskern locally

Everything you need to get the engine, the CLI, the desktop app and the
site running on your own machine, and what to do when one of them
doesn't.

If you only want to change the [rules database](RULES.md) — which is the
most useful contribution there is — you need **step 1 and step 2 only**.
The rules are JSON, the tests are pure functions, and none of it needs a
GUI toolchain.

## 1. What you need

| For | You need |
| --- | --- |
| Engine + CLI | A Rust toolchain (stable, edition 2021) and a C linker |
| Desktop app | The above, plus Node 20+ and your platform's WebView deps |
| Site | Node 20+ |

Rust comes from [rustup](https://rustup.rs). Everything in the workspace
builds on current stable; there is no pinned toolchain file, and CI
tracks `stable`.

**A C linker is not optional.** Rust needs one to link every binary,
including build scripts and proc macros, so a machine with `rustc` but no
linker cannot even run `cargo check`. The error is `linker 'cc' not
found`.

```sh
# Debian / Ubuntu
sudo apt-get install build-essential
# Fedora
sudo dnf install gcc
# macOS — the Command Line Tools, which also carry `cc`
xcode-select --install
# Windows: install the "Desktop development with C++" workload for MSVC,
# or use the GNU toolchain via rustup.
```

For the **desktop app**, Tauri v2 needs your platform's webview
development files. macOS (WKWebView) and Windows (WebView2) ship theirs
with the OS; Linux does not:

```sh
# Debian / Ubuntu — the same line CI and the release build use
sudo apt-get install libwebkit2gtk-4.1-dev libappindicator3-dev \
  librsvg2-dev patchelf
```

Other distributions are covered by the
[Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/).

## 2. The engine and the CLI

No GUI dependencies. This is the fast loop, and the one to stay in while
you work on the engine:

```sh
cargo test -p diskern-core -p diskern-cli
```

Then point it at something real. Scanning is read-only — it never
modifies, moves or deletes anything, so this is safe to run anywhere:

```sh
cargo run -p diskern-cli -- scan ~/Downloads
```

Useful while developing: `--top 0` shows every finding rather than five
per category, `--verdict risky` narrows to one verdict, and `--json`
gives the whole report for piping into `jq`. See the
[CLI README](../crates/diskern-cli/README.md).

## 3. The desktop app

```sh
cd app
npm install
npm run tauri dev
```

The first build compiles the whole Tauri dependency tree and takes a
while; later ones are incremental. `npm run tauri dev` starts Vite and
the Rust backend together and reloads the frontend on save — a change to
`src-tauri/` restarts the backend, which is slower.

To check only that the frontend compiles, `npm run build` skips Rust
entirely.

## 4. The site

```sh
cd site
npm install
npm run dev     # local preview
npm run lint    # oxlint, same as CI
```

## 5. Running the tests

```sh
# What CI runs, and what your change has to pass
cargo test -p diskern-core -p diskern-cli
cargo fmt --check
cargo clippy -p diskern-core -p diskern-cli --all-targets -- -D warnings
```

`cargo test --workspace` also builds `diskern-app`, which needs the
webview dependencies from step 1. Without them it fails in a `*-sys`
build script — that is a missing system package, not a broken test.
Scope to the two crates instead.

Tests live beside the code they cover, in `#[cfg(test)] mod tests` at the
bottom of each module. `crates/diskern-core/src/report.rs` has the
fullest examples: they build a small tree in a `tempfile::tempdir()`,
scan it, and assert on the report.

Two things worth knowing before you write one:

- **Don't rely on the embedded rules in a report test.** A temp directory
  lives under `/tmp` on Linux and `%LOCALAPPDATA%\Temp` on Windows, both
  of which the shipped rules match — so every fixture file classifies as
  a temp file and your assertions quietly measure the wrong thing. Build
  a small `RulesDb::new(..)` with only the rules the test is about;
  `temp_rules()` in `report.rs` is the pattern.
- **Compare paths, not strings containing paths.** `Path::join` and `==`
  work on every platform; `contains("a/b")` finds nothing on Windows.

## 6. What CI runs

Every pull request gets:

| Check | What it does |
| --- | --- |
| `lint` | `cargo fmt --check` and clippy with `--all-targets` (tests included) |
| `test (ubuntu / windows / macos)` | The engine and CLI suite on all three platforms |
| `app-tauri (ubuntu / windows / macos)` | Compiles the Tauri shell; only on changes under `crates/`, `app/` or the workflow |
| `app-frontend`, `site` | Vite builds, plus oxlint for the site |
| `typos` | Spell-checks code and docs — config in [`_typos.toml`](../_typos.toml) |
| `doc-links` | Relative links between markdown files resolve |

The engine tests run on three platforms because the engine is
platform-specific in places: the rules database describes Windows and
macOS paths, `scanner::default_excludes` has a `cfg` arm per platform,
and quarantine has to flatten drive letters that only exist on Windows.

A weekly [audit workflow](DEPENDENCY-AUTOMATION.md) additionally checks
dependencies against RustSec advisories and docs for dead external links.

## 7. When it doesn't build

| Symptom | Cause |
| --- | --- |
| `linker 'cc' not found` | No C toolchain — see step 1. |
| `error: the 'cargo' binary ... is not applicable` | A partial rustup toolchain. `rustup toolchain uninstall stable && rustup toolchain install stable`. |
| ``The system library `glib-2.0` ... was not found`` | Building the app without the Linux webview packages from step 1. It names `glib-2.0` rather than webkitgtk because `glib-sys` is the first `*-sys` crate to fail; installing the four packages fixes all of them. Scope to `-p diskern-core -p diskern-cli` if you didn't mean to build the app. |
| `The pkg-config command could not be found` | Same cause, one step earlier: `pkg-config` itself is missing. |
| `failed to resolve ../dist` from `tauri-build` | The frontend hasn't been built. `npm run build` in `app/` first, or use `npm run tauri dev`, which does it for you. |
| Tests pass locally, fail on Windows CI | Almost always a path assumption — see the two notes in step 5. |

### Building without a local toolchain

If you can't install a C compiler on the machine you're working on —
no root, a locked-down box — the engine and CLI build fine in a
container:

```sh
podman run --rm -v "$PWD":/work:z -w /work docker.io/library/rust:1-slim \
  cargo test -p diskern-core -p diskern-cli
```

Add `-v cargo-cache:/usr/local/cargo/registry` to keep the dependency
cache between runs. The image ships neither `clippy` nor `rustfmt`, so
add `rustup component add clippy rustfmt &&` before the `cargo` call when
you need them, and building the *app* this way additionally needs the
webview packages installed into the image.

Rootless Podman maps the container's root to your own user, so `target/`
comes out owned by you. Rootful Docker does not — it leaves a
root-owned `target/` in your checkout. Mount it elsewhere if that is
what you have:
`-v "$PWD/../diskern-target":/work/target`.
