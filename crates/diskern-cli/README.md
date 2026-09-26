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

# Skip large directories while scanning their parent
diskern scan ~ --exclude ~/Videos --exclude ~/VMs

# Full JSON report (for scripting / piping into jq)
diskern scan ~/Downloads --json

# Write the JSON report to a file instead of stdout
diskern scan ~/Downloads --json --output report.json

# Add deterministic narration over the finished report
diskern scan ~/Downloads --explain
```

### Output

Findings are grouped by verdict (safest first), then by category within
each verdict, with every reason printed underneath as the evidence — the
rule that matched, and anything that changed the verdict from the rule's
own:

```text
Scanned 84,213 files.
Reclaimable: 12.4 GB across 1,208 findings and 46 duplicate sets.

Safe to remove — 802 findings · 6.1 GB
  Browser cache · 641 · 4.8 GB
      412.0 MB  /home/u/.cache/google-chrome/Default/Cache/data_2
                matched rule chrome-cache: Chrome browser cache. …
                not accessed in 210 days
      …     …  … 636 more

Risky — not recommended — 3 findings · 0 B
  Build artifacts · 3 · 0 B
           0 B  /home/u/work/api/node_modules/.package-lock.json
                matched rule node-modules: Node.js dependencies. …
                referenced by 3 projects
```

`referenced by 3 projects` is the impact graph: three live projects have
that `node_modules` on their dependency path, so the verdict drops from
`review` to `risky` and its bytes stop counting as reclaimable —
nothing will offer to move it.

| Flag        | Default | Effect                                                  |
| ----------- | ------- | ------------------------------------------------------- |
| `--top N`   | `5`     | Findings shown per category; `0` shows every one.       |
| `--exclude <dir>` | platform defaults | Skip a directory while scanning; repeat the flag to skip more. User excludes are added to the built-in protected excludes. |
| `--verdict` | all     | `safe`, `review`, `risky` or `protected`. Duplicate sets have no verdict, so they are omitted when this is set. |
| `--explain` | off | Print deterministic narration over the finished report. |
| `--json`    | off     | Full report as JSON; the flags above don't apply.        |
| `--output <file>` | stdout | Write the JSON report to a file instead of printing it; requires `--json`. |
| `--rules <file>` | embedded | Load and validate an external rules database; embedded protected rules remain authoritative. |

Scanning never modifies the files it inspects. `--json --output <file>`
explicitly writes the completed report to the chosen destination, leaving
stdout empty and printing a confirmation to stderr. Its parent directory
must already exist.

File exports are written to a unique temporary file in that directory and
synced before replacing the destination. A failed write or replacement
preserves an existing report; a failed export to a new path leaves no
partial report there. Temporary files are removed on ordinary errors.
Replacement publishes a new file at the named path, rather than modifying
an existing inode or following a destination symlink. On Unix, the new
file has private permissions. An abrupt process termination may leave a
temporary file behind; full power-loss durability is not guaranteed.

## Develop

```sh
cargo run -p diskern-cli -- scan <dir>
```

No GUI dependencies required; this crate only depends on
[`diskern-core`](../diskern-core) and clap.
