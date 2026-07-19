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
5. Replace `YOUR_GITHUB_USERNAME` in `tauri.conf.json`, `Cargo.toml`.

## Every release

```sh
# bump "version" in app/src-tauri/tauri.conf.json and Cargo.toml [workspace.package]
git tag v0.1.0
git push --tags
```

The Release workflow builds Windows + Linux, signs the updater artifacts,
uploads them to a draft GitHub release, and generates `latest.json`.
Review the draft, publish it — running apps pick up the update from
`releases/latest/download/latest.json` on next launch.

## Code signing roadmap

- Windows: apply to SignPath Foundation (free for OSS) once v0.x has real
  releases and users. Until then, unsigned builds will show SmartScreen
  warnings — document this in release notes.
- macOS: deferred until an Apple Developer account ($99/yr) is justified.
- Store channels (winget/Flathub): use a separate build with the updater
  disabled; the store manages updates.
