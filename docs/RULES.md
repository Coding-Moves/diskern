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
| `patterns`    | Substrings matched against the normalized path (lowercased, `/`-separated). |
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

## Contributing rules

Growing this database (per-platform, per-app) *is* the product work, and
rules PRs are very welcome. Guidelines:

1. Be conservative: when in doubt, use `review`, not `safe`.
2. Patterns should be specific enough not to match user data
   (e.g. `/target/debug`, not `/target`).
3. Write the `description` for end users: what it is, why it's safe (or
   not), what happens after removal.
4. Add a test in [`rules.rs`](../crates/diskern-core/src/rules.rs) if the
   rule protects something critical.
