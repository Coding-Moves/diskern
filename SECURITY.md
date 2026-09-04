# Security Policy

## Reporting a vulnerability

Please **do not** open a public issue for security problems.

Instead, use GitHub's private vulnerability reporting:
**Security → Report a vulnerability** on the
[repository page](https://github.com/Coding-Moves/diskern/security).

You should receive an acknowledgment within a few days. Please include
steps to reproduce and the affected version/commit.

## Scope

Diskern touches user files, so we treat the following as security issues:

- Anything that causes a scan (which must be read-only) to modify files.
- Anything that bypasses quarantine and hard-deletes data.
- Path traversal or symlink tricks that let a rule act outside scanned roots.
- The updater accepting an improperly signed release.

## Dependency advisories

Advisories in our dependencies are a different case from the above: they
are already public in the [RUSTSEC database](https://rustsec.org/) by the
time we see them, so there is nothing to disclose privately. A workflow
audits `Cargo.lock` weekly and on every push to `main`, files a public
tracking issue labelled `security` when it finds something, and opens a
dependency-update PR to clear it. See
[docs/DEPENDENCY-AUTOMATION.md](docs/DEPENDENCY-AUTOMATION.md).

## Supported versions

Only the latest release receives security fixes.
