---
name: tbdflow
description: Drive Trunk-Based Development with the tbdflow CLI from Claude Code. Use when the user wants to commit, branch, sync with trunk, complete/merge work, undo a commit, check for conflicts (radar), log intent breadcrumbs, generate a changelog, bootstrap a new repo, or otherwise manage their git workflow. tbdflow is the only interface for these git workflow actions — do not fall back to raw git.
---

# tbdflow (Claude Code)

`tbdflow` is a CLI that enforces Trunk-Based Development: small, safe, conventional
commits straight to a healthy trunk (`main`). This skill is tuned for **non-interactive,
machine-readable** use by Claude Code.

## Golden rule for agents

**Always pass `--non-interactive --toon`** (global flags, before the subcommand):

```bash
tbdflow --non-interactive --toon <command> [args]
```

- `--non-interactive` disables every wizard/prompt. Missing required input becomes a
  clear error (like `gh`) instead of hanging on a TTY prompt.
- `--toon` emits one machine-readable [TOON](https://github.com/toon-format/toon) document
  on stdout — parse `result`, `ok`, `warnings`, and `error` from it. Human prose is suppressed.
- Add `--verbose` to also capture the exact git/gh command `trace[]`.

Example TOON result:

```
command: commit
ok: true
warnings[1]: DoD checklist deferred (--non-interactive); unchecked items added as TODO footer
result:
  subject: "fix(login): resolve timeout"
  type: fix
  branch: main
  sha: 7d2a007
  signed: true
  pushed: true
```

If `ok: false`, read `error` and fix the inputs — never fall back to raw `git`.

## Preflight (run once per session)

```bash
tbdflow --toon doctor
```

Checks git, the GitHub CLI (`gh` installed + authenticated), GPG signing, and config.
If `tbdflow` itself is missing: `cargo install tbdflow`, or download a release binary from
https://github.com/cladam/tbdflow/releases. If `healthy: false`, surface the failing
`checks[].detail` to the user before proceeding.

## Commands

| Intent | Command |
|--------|---------|
| Commit to trunk | `tbdflow --non-interactive --toon commit -t <type> [-s <scope>] -m "<msg>" [--issue KEY-123] [-b] [--body "..."]` |
| Start a branch | `tbdflow --non-interactive --toon branch -t <type> -n <slug> [--issue KEY-123]` |
| Merge a branch back | `tbdflow --non-interactive --toon complete -t <type> -n <slug>` |
| Sync with trunk | `tbdflow --non-interactive --toon sync` |
| Status | `tbdflow --toon status` |
| Conflict radar | `tbdflow --toon radar` |
| Undo a trunk commit | `tbdflow --non-interactive --toon undo <sha> [--no-push]` |
| Log a breadcrumb | `tbdflow --toon note "<why>"` (alias `+`) |
| Changelog | `tbdflow --toon changelog --unreleased` |
| New repo (bootstrap) | `tbdflow --non-interactive --toon init [--create-remote owner/name --private --push]` |

Pre-commit habit: run `tbdflow --non-interactive --toon sync` before `commit`.

## Conventional commit rules (the linter enforces these — generate valid input)

Header: `type(scope)!: subject`

- **type** ∈ `feat fix chore docs refactor test build ci perf revert style`. Never invent one. Default `chore` if nothing behaves differently.
- **subject** ≤ **72 chars**, lowercase first letter, no trailing period, imperative ("add", not "added").
- **scope** optional, lowercase, no spaces.
- **body** lines ≤ **80 chars**, separated from the subject by a blank line (use `--body`).
- **issue key** `--issue` must match `^[A-Z]+-\d+$` (e.g. `PROJ-123`).
- Breaking change: pass `-b`; describe with `--breaking-description "..."`.
- Branch name (`-n`): lowercase, hyphen-separated, no spaces. Slugify titles (`"Fix login bug"` → `fix/login-bug`).

Staging is automatic — never run `git add`. Accumulated `note` breadcrumbs are folded into the next commit body; drop 1–2 before any non-trivial commit to record the *why*.

## GPG signing

Commits and tags are **signed automatically** when a signing key is configured
(`user.signingkey` or `commit.gpgsign=true`), reusing the user's `gpg-agent`. If signing
blocks the agent (no cached passphrase / no agent), pass `--no-sign` for that call and tell
the user. Check signing status with `tbdflow --toon doctor`.

## New repositories and unborn branches

`tbdflow` works from an empty/unborn state:

- `tbdflow --non-interactive init` → `git init`, names the trunk `main`, writes
  `.tbdflow.yml` + `.dod.yml`, and makes the initial commit. Add `--remote <url>` to link an
  existing remote, or `--create-remote owner/name [--private] [--push]` to create one via `gh`.
- The **first `commit`** on an unborn repo skips the pre-pull and sets the upstream on push.
- `branch` requires at least one commit — make the initial commit first.

## When NOT to use tbdflow

Long-lived branches, history rewrites, interactive rebases, or merging without explicit
user intent. If something can't be done via `tbdflow`, explain the limitation — do not run
raw `git` for the workflow actions above.
