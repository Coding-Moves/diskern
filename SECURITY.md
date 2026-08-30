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

## Supported versions

Only the latest release receives security fixes.
