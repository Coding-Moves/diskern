# diskern-cli

The Diskern engine from the terminal. Installs a single `diskern` binary.

## Usage

```sh
# Read-only scan: find duplicates, caches, and reclaimable space
diskern scan ~/Downloads

# Full JSON report (for scripting / piping into jq)
diskern scan ~/Downloads --json
```

Scanning is always read-only — the CLI never modifies, moves, or deletes
anything.

## Develop

```sh
cargo run -p diskern-cli -- scan <dir>
```

No GUI dependencies required; this crate only depends on
[`diskern-core`](../diskern-core) and clap.
