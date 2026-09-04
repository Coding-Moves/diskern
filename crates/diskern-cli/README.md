# diskern-cli

The Diskern engine from the terminal. Installs a single `diskern` binary.

## Usage

```sh
# Read-only scan: find duplicates, caches, and reclaimable space
diskern scan ~/Downloads

# Only what the rules engine considers safe to remove
diskern scan ~/Downloads --verdict safe

# Every finding, not just the first few per category
diskern scan ~ --top 0

# Full JSON report (for scripting / piping into jq)
diskern scan ~/Downloads --json
```

### Output

Findings are grouped by verdict (safest first), then by category within
each verdict, with the rule that matched printed underneath as the
evidence:

```text
Scanned 84,213 files.
Reclaimable: 12.4 GB across 1,208 findings and 46 duplicate sets.

Safe to remove — 802 findings · 6.1 GB
  Browser cache · 641 · 4.8 GB
      412.0 MB  /home/u/.cache/google-chrome/Default/Cache/data_2
                matched rule chrome-cache: Chrome browser cache. …
      …     …  … 636 more
```

| Flag        | Default | Effect                                                  |
| ----------- | ------- | ------------------------------------------------------- |
| `--top N`   | `5`     | Findings shown per category; `0` shows every one.       |
| `--verdict` | all     | `safe`, `review`, `risky` or `protected`. Duplicate sets have no verdict, so they are omitted when this is set. |
| `--json`    | off     | Full report as JSON; the flags above don't apply.        |

Scanning is always read-only — the CLI never modifies, moves, or deletes
anything.

## Develop

```sh
cargo run -p diskern-cli -- scan <dir>
```

No GUI dependencies required; this crate only depends on
[`diskern-core`](../diskern-core) and clap.
