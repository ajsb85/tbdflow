---
name: tbdflow
description: Drive Trunk-Based Development with the tbdflow CLI from Claude Code. Use when the user wants to commit, branch, sync with trunk, complete/merge work, undo a commit, check for conflicts (radar), log intent breadcrumbs, generate a changelog, bootstrap a new repo, or otherwise manage their git workflow. tbdflow is the only interface for these git workflow actions — do not fall back to raw git.
---

# tbdflow (Claude Code)

`tbdflow` is a CLI that enforces Trunk-Based Development: small, safe, conventional
commits straight to a healthy trunk (`main`). This skill is tuned for **non-interactive,
machine-readable** use by Claude Code.

## Golden rule for agents

Run tbdflow in **agent mode**: non-interactive (never prompts) + TOON (machine-readable).

**In this repo it's already configured** — `.claude/settings.json` exports
`TBDFLOW_NON_INTERACTIVE=1` and `TBDFLOW_TOON=1`, and tbdflow auto-enables non-interactive
when `CLAUDECODE` or `CI` is set. So a bare `tbdflow <command>` already runs in agent mode
here. Confirm with `tbdflow doctor` (`defaults.non_interactive_source`).

**Elsewhere (no env configured), pass the flags explicitly:**

```bash
tbdflow --non-interactive --toon <command> [args]
```

- `--non-interactive` disables every wizard/prompt. Missing required input becomes a
  clear error (like `gh`) instead of hanging on a TTY prompt.
- `--toon` emits one machine-readable [TOON](https://github.com/toon-format/toon) document
  on stdout — parse `result`, `ok`, `warnings`, and `error` from it. Human prose is suppressed.
- Add `--verbose` to also capture the exact git/gh command `trace[]`.

Resolution precedence: **explicit flag > `TBDFLOW_*` env > `CLAUDECODE`/`CI` detect > default**
(env vars: `TBDFLOW_NON_INTERACTIVE`, `TBDFLOW_TOON`, `TBDFLOW_NO_SIGN`).

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

If `ok: false`, read the stable **`code`** field (not the prose) and branch on it —
never fall back to raw `git`. Codes include: `missing_args`, `dirty_worktree`,
`ci_failing`, `not_a_repo`, `unborn_no_commits`, `branch_not_found`, `tag_exists`,
`not_on_main`, `cannot_complete_main`, `git_failed`.

## Situational awareness in one call

Prefer **`tbdflow --toon context`** over running `status` + `sync` + `radar` separately —
it returns branch, `clean`, `unborn`, `ahead`/`behind`, `trunk_ci`, `stale[]`,
radar `overlaps[]{branch,author,file,kind}`, and recent commits in a single round-trip.

## Preflight (run once per session)

```bash
tbdflow --toon doctor
```

Checks git, the GitHub CLI (`gh` installed + authenticated), GPG signing, and config.
If `tbdflow` itself is missing, install a release binary (no Rust needed):

```bash
curl -fsSL "https://github.com/ajsb85/tbdflow/releases/latest/download/tbdflow-$(uname -m)-$(uname -s | tr '[:upper:]' '[:lower:]')" -o tbdflow && chmod +x tbdflow && sudo install -m 0755 tbdflow /usr/local/bin/tbdflow
```

or `cargo install tbdflow` (crates.io tracks upstream; use the release binary for this fork's
features). If `healthy: false`, surface the failing `checks[].detail` to the user first.

## Commands

| Intent | Command |
|--------|---------|
| Situational awareness | `tbdflow --toon context` |
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
- **subject** — aim for ≤ **50 chars** (the "50/72 rule"; tbdflow warns past 50), **72 hard max**;
  lowercase first letter, no trailing period, imperative ("add", not "added"). The length
  counts the full `type(scope): subject` line.
- **scope** optional, lowercase, no spaces.
- **body** lines ≤ **72 chars** recommended (warned past 72), **80 hard max**, separated from
  the subject by a blank line (use `--body` or `--body-file`).
- **issue key** `--issue` must match `^[A-Z]+-\d+$` (e.g. `PROJ-123`).
- Breaking change: pass `-b`; describe with `--breaking-description "..."`.
- Branch name (`-n`): lowercase, hyphen-separated, no spaces. Slugify titles (`"Fix login bug"` → `fix/login-bug`).

Staging is automatic — never run `git add`. Accumulated `note` breadcrumbs are folded into the next commit body; drop 1–2 before any non-trivial commit to record the *why*.

For multi-line bodies, avoid shell-escaping: write the text to a file and pass
`--body-file <path>` (or `--body-file -` to read stdin). `--message-file` does the same
for the subject. These conflict with `-m`/`--body`.

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

## TOON `result` schemas

Field names you can rely on per command (all under `result:` in `--toon` mode):

- **context** — `branch, clean, unborn, upstream, ahead, behind, trunk_ci,
  stale[]{branch,days}, overlaps[]{branch,author,file,kind}, recent`
- **commit** — `subject, type, branch, sha, signed, pushed` (+ `tag` if tagged)
- **branch** — `branch, created`
- **complete** — `branch, merged, deleted`
- **sync** — `branch, clean, overlap`
- **status** — `clean, status`
- **radar** — `enabled, branches_scanned, local_files, overlaps[]{branch,author,file,kind}`
- **doctor** — `healthy, checks[]{name,ok,detail}`
- **info** — `main_branch, issue_strategy, lint, review, radar, ci_check, monorepo, remote, current_branch`
- **init** — `initialised, config_created, remote_linked` (+ `remote`)
- **undo** — `reverted, pushed`

On failure the document has `ok: false`, a human `error`, and a stable `code` (see above).

## Slash commands (this repo)

`/ship` (commit to trunk), `/catchup` (sync + context), `/radar` (overlap scan) wrap the
right tbdflow invocations. A `PreToolUse` guard hook blocks raw `git commit|push|merge|rebase`
and redirects to tbdflow (override with a trailing `# raw-git-ok`).

## When NOT to use tbdflow

Long-lived branches, history rewrites, interactive rebases, or merging without explicit
user intent. If something can't be done via `tbdflow`, explain the limitation — do not run
raw `git` for the workflow actions above.
