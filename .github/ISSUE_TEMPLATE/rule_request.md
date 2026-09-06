---
name: Rules database entry
about: Tell us about a cache or artifact directory Diskern doesn't know yet
labels: rules, enhancement
---

<!-- Diskern only knows what its rules tell it. If you know where an app
     on your platform keeps its regenerable files, that's a rule nobody
     else can write as well as you — and it needs no Rust.
     Format and reasoning: docs/RULES.md -->

**Which application, and on which OS?**

<!-- e.g. "VS Code on macOS", "Docker on Linux" -->


**Where does it keep the files?**

<!-- The full path, with the user-specific parts marked:
     ~/Library/Application Support/Code/CachedData
     C:\Users\<user>\AppData\Local\<app>\Cache -->


**What's in there, and what happens after it's removed?**

<!-- What regenerates it, and what the user loses in the meantime —
     "rebuilt on next launch, first launch is slower" is exactly right. -->


**Proposed verdict**

<!-- safe = regenerable, removal affects nothing
     review = probably reclaimable, but the user should look first
     Prefer review when unsure. A wrong `safe` offers to move real data. -->

- [ ] `safe`
- [ ] `review`
- [ ] not sure

**Is there anything nearby that must NOT match?**

<!-- The trap this database exists to avoid: a Firefox profile holds
     cache2/ next to logins.json and cookies.sqlite, so the rule has to
     name the cache directory, not the profile. Anything similar here? -->


**Would you like to open the PR yourself?**

<!-- Happy either way. The rules are JSON in
     crates/diskern-core/rules/base.json and adding one is a few lines. -->
