# Dependency automation

Three pieces keep the Rust dependency tree honest. They overlap on
purpose: Dependabot moves versions forward, the audit workflow notices
when a version we are already on turns out to be vulnerable, and the
auto-fix workflow turns that finding into a PR.

| Piece | File | Cadence | Output |
| --- | --- | --- | --- |
| Audit | [audit.yml](../.github/workflows/audit.yml) | Mondays 06:00 UTC, every push to `main`/`master`, PRs touching `Cargo.lock` | A labelled tracking issue, or an inline PR annotation |
| Auto-fix | [auto-fix.yml](../.github/workflows/auto-fix.yml) | Mondays 07:00 UTC, and after a failed Audit run on `main`/`master` | A PR on `automation/cargo-update` |
| Dependabot | [dependabot.yml](../.github/dependabot.yml) | Weekly | One grouped PR for minor/patch, one PR per major |

## Audit

`cargo audit` reads `Cargo.lock` against the [RUSTSEC advisory
database](https://rustsec.org/) and reports two things: vulnerabilities,
and warnings (unmaintained, unsound, or yanked crates). The workflow
counts both.

On a pull request it runs `rustsec/audit-check`, which annotates the
advisory inline on the changed lockfile.

Everywhere else it owns a single issue titled **RUSTSEC advisories in
Cargo.lock**, labelled `security` and `automated-issue`:

- Advisories found, no open issue → files one listing every advisory id,
  the crate and version that pulled it in, and the advisory title.
- Advisories found, issue already open → rewrites the body in place.
  A four-month-old advisory stays one thread, not seventeen.
- Nothing found → closes the issue with a comment.

The job then fails, which is what auto-fix watches for.

Both labels are created by the workflow if the repository doesn't have
them yet, so this needs no manual setup.

## Auto-fix

Runs `cargo audit fix` (raises a version requirement when clearing the
advisory needs a semver-incompatible bump — this subcommand is
experimental, so it is best-effort) and then `cargo update` (refreshes
the lockfile inside the existing ranges).

If nothing changed, it stops. If something changed it runs the same
gates as CI — `cargo fmt --check`, `cargo clippy -D warnings`,
`cargo test` — and only opens the PR when they pass. That check matters
more than it looks; see the token caveat below.

The PR body carries the manifest diff and the `cargo audit` output as it
stands *after* the update, so you can see at a glance whether the bump
actually cleared the advisory or only part of it.

It always reuses the branch `automation/cargo-update`, so a second run
updates the open PR rather than stacking a new one, and it always works
from the default branch — a pull request whose lockfile trips the audit
is the PR author's to fix, so audit failures from PRs are ignored here.

## Repository settings you have to enable

None of this works until two settings are on. Both are one-time.

**1. Let Actions create pull requests.**
Settings → Actions → General → *Workflow permissions*:

- Select **Read and write permissions**.
- Tick **Allow GitHub Actions to create and approve pull requests**.

Without the tick, the auto-fix run fails at the `create-pull-request`
step with `GitHub Actions is not permitted to create or approve pull
requests`. The per-workflow `permissions:` blocks cannot grant this —
it is an account/repository switch that caps what any token in the repo
is allowed to do.

If the repository belongs to an organization, the same switch exists at
the org level (Organization settings → Actions → General) and the org
setting wins. Turning it on for the repository alone is not enough if
the org has it off.

**2. Turn on Dependabot security updates.**
Settings → Code security → enable **Dependency graph**, **Dependabot
alerts**, and **Dependabot security updates**.

Version updates come from [dependabot.yml](../.github/dependabot.yml).
Security updates do not — they are driven by that setting, they ignore
the `groups` config, and they open one PR per advisory.

## The GITHUB_TOKEN caveat

A PR opened using `GITHUB_TOKEN` does not trigger other workflows.
GitHub does this deliberately, to stop a workflow from triggering itself
in a loop. The practical effect: the auto-fix PR arrives with no CI
checks on it.

Two ways to live with that:

- **As shipped.** Auto-fix runs fmt, clippy and tests itself before
  opening the PR, so the code is checked even though the PR shows no
  check runs. Closing and reopening the PR by hand also kicks CI off.
- **With a PAT.** Create a fine-grained personal access token with
  *Contents: read and write* and *Pull requests: read and write* on this
  repository, save it as the repository secret `AUTOMATION_TOKEN`, and
  swap the `token:` input in
  [auto-fix.yml](../.github/workflows/auto-fix.yml):

  ```yaml
  token: ${{ secrets.AUTOMATION_TOKEN }}
  ```

  PRs opened with a PAT do trigger workflows, so CI runs normally. The
  cost is a token to rotate.

## Assignee

The PR and Dependabot config both assign `Muawiya-contact`, the
[CODEOWNER](../.github/CODEOWNERS). `github.repository_owner` resolves
to the `Coding-Moves` organization, and an organization cannot be an
assignee — so the username is written out rather than templated. Change
it in both files if ownership moves.

## Running it by hand

Both workflows have `workflow_dispatch`: Actions → pick the workflow →
**Run workflow**. Locally:

```
cargo install cargo-audit --features=fix --locked
cargo audit
cargo audit fix
cargo update
```
