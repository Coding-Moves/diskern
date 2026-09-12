# Releasing

## One-time setup

1. Generate the updater keypair (choose a password, remember it):
   ```sh
   cd app && npm run tauri signer generate -- -w ~/.tauri/diskern.key
   ```
2. Put the **public key** into `app/src-tauri/tauri.conf.json` under
   `plugins.updater.pubkey`.
3. Add GitHub Actions secrets:
   - `TAURI_SIGNING_PRIVATE_KEY` — contents of `~/.tauri/diskern.key`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — the password you chose
4. **Back up the private key + password somewhere safe.** If lost,
   existing installs can never update again.

The repository and updater endpoint already point to `Coding-Moves/diskern`.
For later releases, keep using the existing signing key so installed copies
can verify updates.

## Every release

### 1. Prepare the version and changelog

Choose the next version and update all of these together:

| File | What to update |
| --- | --- |
| `Cargo.toml` | `[workspace.package].version` |
| `Cargo.lock` | Versions of `diskern-core`, `diskern-cli`, and `diskern-app` |
| `app/src-tauri/tauri.conf.json` | `version` |
| `app/package.json` | `version` |
| `app/package-lock.json` | Top-level and root package versions |
| `CHANGELOG.md` | Move completed changes from `Unreleased` into a dated release entry |
| `README.md` | Public download links for the versioned release artifacts |
| `docs/releases/vX.Y.Z.md` | Draft release notes, download links and contributor credits |

After editing the version fields, let Cargo and npm refresh their lockfiles:

```sh
cargo check -p diskern-core -p diskern-cli
(cd app && npm install --package-lock-only --ignore-scripts)
git diff -- Cargo.toml Cargo.lock app/package.json app/package-lock.json \
  app/src-tauri/tauri.conf.json CHANGELOG.md README.md docs/releases/
```

Check that the diff contains the intended version updates. Keep unrelated
dependency upgrades in a separate change.

### 2. Check the release candidate

Use the setup in [DEVELOPMENT.md](DEVELOPMENT.md), including the platform
WebView dependencies for the desktop checks. From the repository root:

```sh
cargo fmt --all --check
cargo clippy --locked -p diskern-core -p diskern-cli --all-targets --all-features -- -D warnings
cargo test --locked -p diskern-core -p diskern-cli --all-features
(cd app && npm ci && npm run build)
cargo clippy --locked -p diskern-app --all-targets -- -D warnings
cargo test --locked -p diskern-app
(cd site && npm ci && npm run lint && npm test && npm run build)
```

Merge the version changes through a PR, then wait for CI and the dependency
audit to pass on the exact `main` commit you intend to release. Resolve any
release blockers before tagging; a green run from an older commit is not
enough.

### 3. Push only the intended tag

Update your clean checkout to the checked commit. The version below is an
example: replace it with the version you prepared in step 1.

```sh
git switch main
git pull --ff-only origin main
git status --short
git rev-parse HEAD
# Confirm this is the commit whose CI and audit passed.
release_version=0.3.0
git tag "v$release_version"
git push origin "v$release_version"
```

### 4. Review the draft before publishing

The Release workflow builds Windows + Linux, signs the updater artifacts,
uploads them to a draft GitHub release, and generates `latest.json`.

- Confirm both build jobs succeeded and the draft's tag, version, and release
  notes match the intended release.
- Check that Windows and Linux installers, their updater signatures, and
  `latest.json` are attached. The manifest must name the new version and
  point to that release's artifacts for both platforms.
- Test installing and launching the builds on Windows and Linux. Using
  disposable fixtures, check scan, cancel, quarantine, restore after restart,
  and explicit purge. Verify the signed update path from the previous release
  with a test endpoint serving the candidate manifest and artifacts.
- Publish the draft once those checks pass. Confirm the public updater
  manifest and website downloads now show the new version. Existing apps
  check `releases/latest/download/latest.json` on launch.

## Code signing roadmap

- Windows: apply to SignPath Foundation (free for OSS) once v0.x has real
  releases and users. Until then, unsigned builds will show SmartScreen
  warnings — document this in release notes.
- macOS: deferred until an Apple Developer account ($99/yr) is justified.
- Store channels (winget/Flathub): use a separate build with the updater
  disabled; the store manages updates.
