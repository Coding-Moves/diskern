# The rules database

Safety verdicts in Diskern come from a deterministic, auditable rules
database — never from a model or a network call. Rules are **data, not
code**: shipped as JSON, versioned, and embedded into the binary at build
time from [`crates/diskern-core/rules/base.json`](../crates/diskern-core/rules/base.json).

## Rule format

```json
{
  "id": "chrome-cache",
  "patterns": ["/google/chrome/user data/default/cache", "/.cache/google-chrome"],
  "category": "browser_cache",
  "verdict": "safe",
  "description": "Chrome browser cache. Fully regenerated on next use."
}
```

| Field         | Meaning                                                              |
| ------------- | -------------------------------------------------------------------- |
| `id`          | Stable, kebab-case, unique. Shown to users as evidence.              |
| `patterns`    | Globs matched against the normalized path (lowercased, `/`-separated). |
| `category`    | What the file *is* — `browser_cache`, `build_artifact`, `log`, …     |
| `verdict`     | Base safety verdict (see below).                                     |
| `description` | Plain-language explanation shown to the user.                        |

## Verdicts

- `safe` — regenerable, removal has no effect on apps or data
- `review` — probably reclaimable, but the user should look first
  (e.g. `target/`, `node_modules/`)
- `risky` — removal likely breaks something
- `protected` — system-critical; never offered for removal. **Final:**
  nothing can downgrade a protected verdict.

The risk module may make a verdict *more* cautious based on local
evidence (e.g. recently-accessed files), never less.

## Matching semantics

- **First match wins** — order in the file is priority order, which is
  why `protected` rules are listed first.
- Paths that match no rule get `unknown` / `review` — never `safe`.
- Patterns are globs, matched against the whole normalized path:
  - `*` matches within one path component and stops at `/`
  - `**` spans components
  - a pattern that starts with `/` is anchored at the filesystem root

That anchoring is what keeps a rule inside the directory it names.
`/tmp/**` is the root's scratch directory; it does not reach
`/home/user/tmp/tax-return.pdf`. `**/node_modules/**` still matches at
any depth, because that is what the rule means.

Write patterns to end in `/**` when the rule is about a directory, and
as `**/*.ext` when it is about an extension. A directory pattern without
the trailing `/**` matches the directory entry itself — and the scanner
only ever classifies files, so it would match nothing.

A pattern that isn't a valid glob is dropped with a warning rather than
taken down the scan, so **a typo makes a rule match nothing**. For a
`protected` rule that fails open, which is why
`every_shipped_pattern_is_a_valid_glob` in
[`rules.rs`](../crates/diskern-core/src/rules.rs) exists.

**Rule order is a safety property.** `installer-packages` matches
`**/*.msi` anywhere on the disk — including `C:\Windows\Installer`, the
cache Windows needs to uninstall, repair or patch installed software, and
the `Package Cache` folders that serve the same purpose for .NET and
Visual Studio. `review` is an *actionable* verdict in the app, so without
`windows-installer-cache` listed above `installer-packages`, the UI would
offer to quarantine them.

When adding a broad rule:

1. Put it at the bottom, below the `protected` entries.
2. Work out what else its pattern reaches. An extension matches in system
   directories too, not just in `Downloads`.
3. If it can shadow a `protected` rule, add a test. `rules.rs` has one
   pinning exactly these cases.

Prefer leaving something `unknown` over classifying it wrongly:
`report::build` drops unknown entries, so they never reach the user as an
actionable row.

## Contributing rules

Growing this database (per-platform, per-app) *is* the product work, and
rules PRs are very welcome. Guidelines:

1. Be conservative: when in doubt, use `review`, not `safe`.
2. Patterns should be specific enough not to match user data
   (e.g. `**/target/debug/**`, not `**/target/**`).
3. Write the `description` for end users: what it is, why it's safe (or
   not), what happens after removal.
4. Add a test in [`rules.rs`](../crates/diskern-core/src/rules.rs) if the
   rule protects something critical.
