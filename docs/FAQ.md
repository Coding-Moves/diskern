# FAQ

**Can Diskern delete my files?**
Not directly. Scanning is read-only, and cleanup actions only move files
to a quarantine folder you can review and restore from. Emptying
quarantine is a separate, explicit step.

**Does it send my data anywhere?**
No. Scanning, classification, and risk scoring are fully local. The
optional AI narration layer is off by default and only ever *explains*
findings — it never decides them (see the
[principles](../README.md#principles-non-negotiable)).

**Why did Diskern mark something "review" instead of "safe"?**
Verdicts come from the [rules database](RULES.md) plus local evidence.
Anything not positively identified as regenerable defaults to `review` —
Diskern would rather ask than guess.

**Why is a file I use daily listed at all?**
The report shows what was *found*, not what to delete. Recently-accessed
files get a more cautious score precisely so you notice them.

**CLI or desktop app?**
Same engine either way. The CLI (`diskern scan <dir>`) suits scripting
and remote machines; the app adds visual review and quarantine handling.

**Which platforms are supported?**
The engine and CLI build anywhere Rust does. The desktop app targets
Windows, macOS, and Linux via Tauri v2.
