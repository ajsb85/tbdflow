<div align="center">
  <p align="center">
    <img src="assets/tbdflow-logo.png" alt="tbdflow logo" width="200"/>
  </p>

  <p align="center">
    <i><b>Keep your code flowing</b></i><br/>
  </p>

[![Crates.io](https://img.shields.io/crates/v/tbdflow.svg)](https://crates.io/crates/tbdflow)
[![Downloads](https://img.shields.io/crates/d/tbdflow.svg)](https://crates.io/crates/tbdflow)

</div>

## The problem

Many teams say they practise Trunk-Based Development but in day-to-day reality things deviate:

- **Commit messages become inconsistent.** Everyone formats them a little differently.
- **Branches that were meant to live for hours** stick around for days.
- **Merging back to main** turns into a manual sequence people half-remember.
- **Two people change the same file** and nobody notices until a push fails.
- **The Definition of Done exists,** but it lives in a document no one looks at during the work.

None of this breaks the build immediately. But over time, the trunk stops feeling safe to work in.

## The solution

`tbdflow` is a small CLI that **codifies your team's Trunk-Based workflow** so the safe path is always the easiest path.

```bash
cargo install tbdflow
```

It handles the ceremony (pulling, rebasing, linting, pushing) so you can stay focused on the work.

![A terminal running the command tbdflow](docs/commit-demo.gif "A demo of tbdflow running commit-to-main commands")

## What it does

| Pain point                     | How tbdflow helps                                                                                |
|--------------------------------|--------------------------------------------------------------------------------------------------|
| Inconsistent commits           | `tbdflow commit` enforces Conventional Commits with built-in linting                             |
| Long-lived branches            | `tbdflow branch` + `tbdflow complete` with stale-branch warnings                                 |
| "Did I pull before pushing?"   | `tbdflow sync` + auto-rebase before every commit to main                                         |
| Pulling a broken trunk         | `tbdflow sync` pre-flight CI check warns before pulling a red build                              |
| Merge conflicts you didn't see | `tbdflow radar` shows trunk health, file churn hotspots, and who else is touching the same files |
| "Why was this done?"           | `tbdflow task` + `tbdflow note` captures intent before it's lost                                 |
| "What if I lose my work?"      | **WIP Guard** auto-snapshots your working directory during notes, syncs, and radar scans         |

## Philosophy

* **Main is where the work happens.** `tbdflow commit` is your daily driver: pull, commit, push, done. Small and
  frequent beats large and delayed.
* **Branches are short-lived guests.** They're supported, but they should check out quickly.
* **Cleanup shouldn't be your job.** Completed branches get merged, tagged (for releases), and deleted automatically.
* **Commit messages should tell a story.** [Conventional Commits](https://www.conventionalcommits.org/) keep the
  history readable for humans and machines alike.
* **Collaboration should be visible.** `tbdflow radar` shows trunk health, churn hotspots, and file overlaps; turning
  silent conflicts into early conversations.

### Why not just use Git?

You absolutely should. `tbdflow` isn't a replacement. You'll still reach for raw `git` when rebasing, cherry-picking,
or bisecting.

Think of it as a **workflow assistant** that wraps the repeatable parts of your day:

1. **Everyone does it the same way.**
   Commits, branches, and releases follow the same steps every time. No more "how did you format that commit again?"

2. **Less to keep in your head.**
   You don't need to remember `pull --rebase` then commit then push then tag then delete branch. The CLI does.

3. **The TBD path is the easy path.**
   For 80% of your day, `tbdflow` keeps you in the flow. For the other 20%, Git is right there.

### Installation

#### Download a prebuilt binary (no Rust required)

Each [GitHub release](https://github.com/ajsb85/tbdflow/releases) ships a binary named
`tbdflow-<arch>-<os>` (e.g. `tbdflow-x86_64-linux`):

```bash
curl -fsSL "https://github.com/ajsb85/tbdflow/releases/latest/download/tbdflow-$(uname -m)-$(uname -s | tr '[:upper:]' '[:lower:]')" -o tbdflow
chmod +x tbdflow
sudo install -m 0755 tbdflow /usr/local/bin/tbdflow
```

#### Installing from crates.io

Requires [Rust and Cargo](https://www.rust-lang.org/tools/install). Note: crates.io tracks the
upstream crate — for this fork's latest features use a release binary or build from source.

```bash
cargo install tbdflow
```

To update to the latest version:

```bash
tbdflow update
```

#### Building from source

Or build it yourself (requires Rust and Cargo):

```bash
git clone https://github.com/ajsb85/tbdflow.git
cd tbdflow
sudo cargo install --path . --root /usr/local
```

#### Shell completions and man page

After installing, generate and install completions and the man page (see
[Advanced Usage](#shell-completion) for per-shell paths):

```bash
tbdflow generate-completion bash | sudo tee /usr/share/bash-completion/completions/tbdflow >/dev/null
tbdflow generate-man-page | sudo tee /usr/local/share/man/man1/tbdflow.1 >/dev/null && sudo mandb -q
```

### Monorepo Support

If you work in a monorepo, `tbdflow` understands that not every commit should touch every directory.

When you run `tbdflow commit`, `tbdflow sync` or `tbdflow status` from the repo root, only root-level files are
affected. Project subdirectories are left alone. Run the same commands from inside a project directory and they
automatically scope to that directory. (Run `tbdflow init` in each subdirectory to set this up.)

This is configured in your root `.tbdflow.yml` file:

```
# in .tbdflow.yml
monorepo:
enabled: true
  # A list of all directories that are self-contained projects.
  # These will be excluded from root-level commits and status checks.
  project_dirs:
    - "frontend"
    - "backend-api"
    - "infra"
```

For an overview and to inspect your current configuration, you can run `tbdflow info`.

#### Handling Cross-Cutting Changes

For "vertical slice" changes that intentionally touch multiple project directories, you can use the `--include-projects`
flag.
This flag overrides the default safety mechanism and stages all changes from all directories, allowing you to create a
single, cross-cutting commit.

### Interactive Wizard Mode

To make `tbdflow` even more user-friendly, the core commands (`branch`, `commit`, `complete`, `changelog`) now feature
an interactive "wizard" mode.

If you run one of these commands without providing the required flags, `tbdflow` will automatically launch a
step-by-step guide.
This is perfect for new users who are still learning the workflow, or for complex commits where you want to be sure
you've covered all the options.

For power users, the original flag-based interface is still available for a faster, scripted experience.

### Configuration

`tbdflow` is configurable via two optional files in the root of your repository. To get started quickly, run
`tbdflow init` to generate default versions of these files.

`.tbdflow.yml`
This file controls the core workflow of the tool. You can customise:

- The name of your main branch (e.g. main, trunk).
- Allowed branch types and their prefixes (e.g feat/, chore/)
- A strategy for handling issue references ("branch-name" or "commit-scope")
- The threshold for stale branch warnings.
- Automatic tagging formats.
- Commit message linting rules.

> **Note:** `main_branch_name` configures which branch is your trunk (typically `main` or `master`).
> tbdflow assumes this branch accepts direct commits. For protected branches, use short-lived feature branches with
`tbdflow branch`.

`.dod.yml`
This file controls the interactive Definition of Done checklist for the commit command.

### Features

#### The Definition of Done (DoD) Check

Most teams have a Definition of Done. Most of the time, it lives in a wiki nobody opens mid-task.

If you add a `.dod.yml` to your repo, `tbdflow commit` will surface the checklist right when it matters, before you
push. It's optional, non-blocking, and stays out of your way when you don't need it.

**Example** `.dod.yml`:

```
# .dod.yml in your project root
checklist:
  - "All relevant automated tests pass successfully."
  - "New features or fixes are covered by new tests."
  - "Security implications of this change have been considered."
  - "Relevant documentation (code comments, READMEs) is updated."
```

If you skip items, `tbdflow` offers to add a TODO list to the commit footer so the incomplete work is tracked in
Git history, not lost in a chat thread.

#### Commit Message Linting

Your `.tbdflow.yml` can include linting rules that catch issues before the commit happens: subject too long, wrong
type, missing scope. Quick feedback, no surprises in the log later.

**Default linting rules:**

```yaml
lint:
  conventional_commit_type:
    enabled: true
    allowed_types:
      - build
      - chore
      - ci
      - docs
      - feat
      - fix
      - perf
      - refactor
      - revert
      - style
      - test
  issue_key_missing:
    enabled: false
    pattern: ^[A-Z]+-\d+$
  scope:
    enabled: true
    enforce_lowercase: true
  subject_line_rules:
    max_length: 72          # hard limit (commit rejected)
    recommended_length: 50  # soft limit (warning only) — the "50/72 rule"
    enforce_lowercase: true
    no_period: true
  body_line_rules:
    max_line_length: 80          # hard limit (commit rejected)
    recommended_line_length: 72  # soft limit (warning only)
    leading_blank: true
```

##### The 50/72 rule

Beyond the hard limits, `tbdflow` follows the widely-used **50/72 rule** and emits a
*non-blocking warning* (shown in human output and in the TOON `warnings[]`) when:

- the **subject line** exceeds the recommended **50** characters (it stays readable in
  compact views like `git log --oneline`; 72 remains the hard cap), or
- a **body line** exceeds the recommended **72** characters (so it won't wrap awkwardly in
  an 80-column terminal after Git's indentation).

These are guidance, not gates — the commit still goes through. Lengths are measured in
characters, not bytes.

#### Signed commits and tags

If a signing key is configured (`user.signingkey`, or `commit.gpgsign = true`), `tbdflow`
automatically GPG-signs commits and annotated tags, reusing your existing `gpg-agent` and
git config (SSH signing works too). It sets `GPG_TTY` so signing works in non-TTY contexts.
Pass `--no-sign` to skip signing for a single invocation, or set `commit.gpgsign = false`
to opt out entirely. `tbdflow doctor` reports your signing status and verifies the latest
tag's signature.

#### Intent Log

You tried three approaches before settling on the final one. By the time you commit, the first two are gone. From
your memory and from the diff. A week later, a reviewer suggests one of the approaches you already rejected.

The Intent Log fixes this. While you work, you drop one-line breadcrumbs. At commit time, they're woven into the
message body automatically. Zero context-switching, full context for whoever reads the commit next.

**Start a task (optional):**

```bash
tbdflow task start "Refactor auth logic"
```

**Leave notes as you work:**

```bash
tbdflow note "tried factory pattern, felt too verbose"
tbdflow + "switching to a simple trait implementation"
tbdflow n "trait approach is cleaner, keeping it"
```

The `note` command has two shorthand aliases: `+` and `n`.

**Notes are consumed at commit time:**

When you run `tbdflow commit`, the notes are appended to the commit body automatically:

```text
feat(auth): implement trait-based auth logic

Intent Log:
- tried factory pattern, felt too verbose
- switching to a simple trait implementation
- trait approach is cleaner, keeping it
```

**Other task commands:**

```bash
tbdflow task show    # Show the current task and notes
tbdflow task clear   # Discard the current intent log
```

**Branch awareness:**

The intent log tracks which branch it belongs to. If you switch branches, tbdflow warns you about the stale log so
notes from one task don't leak into another commit.

**File:** Notes are stored locally in `.tbdflow-intent.json` (git-ignored, never committed). The file is deleted
automatically after a successful push to trunk or after `tbdflow complete`.

#### WIP Guard (Continuous Safety)

In TBD, your work-in-progress lives locally until it's ready for trunk. WIP Guard makes sure that
work is never lost by automatically capturing immutable snapshots of your working directory at key moments.

**How it works:**

Under the hood, `tbdflow` uses `git stash create` to generate a commit object representing your current working tree.
Unlike a regular stash, these snapshots don't touch the stash reflog, they can't interfere with your manual stashes,
and they stay in the Git object store until garbage collection (typically 14–30 days).

**When snapshots are captured:**

| Command         | What happens                                                                    |
|-----------------|---------------------------------------------------------------------------------|
| `tbdflow +`     | A snapshot is linked to each breadcrumb note                                    |
| `tbdflow sync`  | A pre-sync snapshot is captured before rebasing                                 |
| `tbdflow radar` | A background snapshot is taken if the working directory is dirty (every 30 min) |
| `tbdflow undo`  | A safety snapshot is captured before the destructive checkout + revert sequence |

**Anti-collision pre-flight:**

Before `sync` or `undo`, tbdflow checks whether a rebase, merge, or cherry-pick is already in progress. If one is,
the command halts with a clear message instead of creating a "Git ghost" state.

**Recovery:**

```bash
# List all available snapshots
tbdflow recover --list

# Restore a snapshot by index
tbdflow recover 1

# Restore a snapshot by hash
tbdflow recover a7b8c9d0
```

Snapshots are applied with `git stash apply` (not `pop`), so they remain available for repeated recovery.

**Lifecycle:** Snapshots are preserved in the intent log until the work is committed to trunk. Feature branch
commits keep the snapshots intact. Once work reaches main, the intent log is cleared. The commit itself is now
the safety net.

---

## Working with AI agents (Claude Code)

`tbdflow` is built to be driven by AI coding agents, not just humans. Two global flags make
it safe and efficient to call programmatically:

- `--non-interactive` — never prompts. Missing required input becomes a clear error (like
  `gh`), and interactive wizards/checklists are disabled.
- `--toon` — emits one machine-readable [TOON](https://github.com/toon-format/toon) document
  instead of human prose. Combine with `--verbose` to capture the underlying git/gh command
  `trace[]`.

```bash
tbdflow --non-interactive --toon commit -t fix -s login -m "resolve timeout"
```

```text
command: commit
ok: true
result:
  subject: "fix(login): resolve timeout"
  type: fix
  branch: main
  sha: 7d2a007
  signed: true
  pushed: true
```

On failure the document carries `ok: false`, a human `error`, and a **stable `code`**
(`missing_args`, `dirty_worktree`, `ci_failing`, `not_a_repo`, `unborn_no_commits`, …) so an
agent can branch on the code instead of parsing prose.

### Zero-config agent mode

You don't have to repeat the flags. tbdflow resolves each global flag with this precedence:

```
explicit CLI flag  >  TBDFLOW_* env var  >  CLAUDECODE/CI auto-detect  >  built-in default
```

- **Auto-detect:** non-interactive mode turns on automatically when `CLAUDECODE` (set by
  Claude Code) or `CI` is present — so agents never hang on a prompt, no flags required.
- **Env vars:** set `TBDFLOW_NON_INTERACTIVE`, `TBDFLOW_TOON`, or `TBDFLOW_NO_SIGN` once and
  every call inherits them. This repo ships them in `.claude/settings.json`, so inside Claude
  Code a bare `tbdflow <command>` already runs non-interactive + TOON.
- **Verify:** `tbdflow doctor` reports the resolved `defaults` and where non-interactive came
  from (`defaults.non_interactive_source`).

```jsonc
// .claude/settings.json
{
  "env": { "TBDFLOW_NON_INTERACTIVE": "1", "TBDFLOW_TOON": "1" }
}
```

### Situational awareness in one call

`tbdflow context` collapses status + sync-state + radar + branch info into a single TOON
document — fewer round-trips for an agent:

```bash
tbdflow --toon context
```

returns `branch, clean, unborn, ahead, behind, trunk_ci, stale[]{branch,days},
overlaps[]{branch,author,file,kind}, recent`.

### Preflight

`tbdflow doctor` checks the environment — git, the GitHub CLI (`gh` installed +
authenticated), GPG signing (and the latest tag's signature), and your configuration:

```bash
tbdflow doctor          # human report
tbdflow --toon doctor   # machine-readable
```

### File-based commit messages

To avoid shell-escaping multi-line bodies, read the subject/body from a file (or stdin
with `-`):

```bash
tbdflow commit -t feat --message-file subject.txt --body-file body.txt
printf 'line one\nline two' | tbdflow commit -t docs -m "update notes" --body-file -
```

### Bundled Claude Code integration

This repo ships a ready-to-use [Claude Code](https://claude.com/claude-code) setup under
`.claude/`:

- **Skill** (`.claude/skills/tbdflow/`) — teaches the agent the full workflow, the
  conventional-commit rules, and the per-command TOON result schemas.
- **Slash commands** (`.claude/commands/`) — `/ship` (commit to trunk), `/catchup`
  (sync + context), `/radar` (overlap scan).
- **Guard hook** (`.claude/hooks/guard-git.sh` + `.claude/settings.json`) — a `PreToolUse`
  hook that redirects raw `git commit|push|merge|rebase` to tbdflow (override with a trailing
  `# raw-git-ok`).

Generic, editor-agnostic agent docs live at the repo root: `SKILL.md` and `AGENT.md`.

---

## Global options

| Flag              | Description                                                                 | Required |
|-------------------|-----------------------------------------------------------------------------|----------|
| --verbose         | Prints the underlying Git/gh commands as they are executed.                 | No       |
| --dry-run         | Simulate the command without making any changes.                            | No       |
| --toon            | Emit machine-readable TOON output instead of human prose (great for agents).| No       |
| --non-interactive | Never prompt; missing input becomes an error and wizards are disabled.      | No       |
| --no-sign         | Skip GPG signing for this commit/tag even if a signing key is configured.   | No       |

## Commands

### 1. `commit`

This is the primary command for daily work.

Commits staged changes using a Conventional Commits message. This command is context-aware:

* **On `main`:** It runs the full TBD workflow: pulls the latest changes with rebase, commits, and pushes.
* **On any other branch:** It simply commits and pushes, allowing you to save work-in-progress.

**Usage:**

```bash
tbdflow commit [options]
```

**Options:**

| Flag | Option                 | Description                                              | Required |
|------|------------------------|----------------------------------------------------------|----------|
| -t   | --type                 | The type of commit (e.g., feat, fix, chore).             | Yes      |
| -s   | --scope                | The scope of the changes (e.g., api, ui).                | No       |
| -m   | --message              | The descriptive commit message (subject line).           | Yes      |
|      | --message-file         | Read the subject from a file (`-` for stdin).            | No       |
|      | --body                 | Optional multi-line body for the commit message.         | No       |
|      | --body-file            | Read the body from a file (`-` for stdin).              | No       |
| -b   | --breaking             | Mark the commit as a breaking change.                    | No       |
|      | --breaking-description | Provide a description for the 'BREAKING CHANGE:' footer. | No       |
|      | --tag                  | Optionally add and push an annotated tag to this commit. | No       |
|      | --issue                | Optionally add an issue reference to the footer.         | No       |
|      | --no-verify            | Bypass the interactive DoD checklist.                    | No       |

**Example:**

```bash
# A new feature
tbdflow commit -t feat -s auth -m "add password reset endpoint"

# A bug fix with a breaking change
tbdflow commit -t fix -m "correct user permission logic" -b
tbdflow commit -t refactor -m "rename internal API" --breaking --breaking-description "The `getUser` function has been renamed to `fetchUser`."

# A bug fix with a new tag
tbdflow commit -t fix -m "correct user permission logic" --tag "v1.1.1"
```

### 2. `branch`

Creates and pushes a new, short-lived branch from the latest version of `main`. This is the primary command for starting
new work that isn't a direct commit to `main`.

**Usage:**

```bash
tbdflow branch --type <type> --name <name> [--issue <issue-id>] [--from_commit <commit hash>]
```

**Options (release):**

| Flag              | Description                                                                     | Required |
|-------------------|---------------------------------------------------------------------------------|----------|
| -t, --type        | The type of branch (e.g. feat, fix, chore). See .tbdflow.yml for allowed types. | Yes      |
| -n, --name        | A short, desriptive name for the branch.                                        | Yes      |
| --issue           | Optional issue reference to include in the branch name or commit scope.         | No       |
| -f, --from_commit | Optional commit hash on `main` to branch from.                                  | No       |

**Examples:**

```bash
# Create a simple feature branch named "feat/new-dashboard"
tbdflow branch -t feat -n "new-dashboard"

# Create a fix branch with an issue reference in the name
# (This will be named "fix/PROJ-123-login-bug" by default)
tbdflow branch -t fix -n "login-bug" --issue "PROJ-123"

# Create a release branch from a specific commit
tbdflow branch -t release -v "2.1.0" -f "39b68b5"
```

### 3. `complete`

Merges a short-lived branch back into main, then deletes the local and remote copies of the branch.

**Automatic Tagging:**

* When completing a release branch, a tag (e.g. v2.1.0) is automatically created and pushed.

**Usage:**

```bash
tbdflow complete --type <branch-type> --name <branch-name>
```

**Options:**

| Flag | Option | Description                                      | Required |
|------|--------|--------------------------------------------------|----------|
| -t   | --type | The type of branch: feature, release, or hotfix. | Yes      |
| -n   | --name | The name or version of the branch to complete.   | Yes      |

**Examples:**

```bash
# Complete a feature branch
tbdflow complete -t feat -n "user-profile-page"

# Complete a release branch (this will be tagged v2.1.0)
tbdflow complete -t release -n "2.1.0"
```

### 4. `changelog`

Generates a changelog in Markdown format from your repository's Conventional Commit history. See `tbdflow` repo for a
CHANGELOG.md generated by this command.

**Usage:**

```bash
tbdflow changelog [options]
```

**Options:**

| Option       | Description                                                               |
|--------------|---------------------------------------------------------------------------|
| --unreleased | Generate a changelog for all commits since the last tag.                  |
| --from       | Generate a changelog for commits from a specific tag.                     |
| --to         | Generate a changelog for commits up to a specific tag (defaults to HEAD). |

**Examples:**

```bash
# Generate a changelog for a new version
tbdflow changelog --from v0.12.0 --to v0.13.0

# See what will be in the next release
tbdflow changelog --unreleased
```

### 5. `review`

Manages non-blocking post-commit reviews for trunk-based development. In TBD, code is committed to trunk first and
reviewed asynchronously, this command facilitates that workflow by creating GitHub issues for review tracking.

**Philosophy:**

In Trunk-Based Development, reviews are for **course correction** and **knowledge sharing**, not gatekeeping.
Code is already in trunk; reviewers focus on Intent, Impact, and Insight.

**Usage:**

```bash
tbdflow review [sha] [options]
```

**Options:**

| Option                | Description                                                            |
|-----------------------|------------------------------------------------------------------------|
| \<sha\>               | Trigger a review for a specific commit (positional argument).          |
| --trigger             | Create a review request for the current HEAD commit.                   |
| --digest              | Generate a digest of commits needing review.                           |
| --approve \<hash\>    | Mark a commit as approved (closes issue with `review-accepted`).       |
| --concern \<hash\>    | Raise a concern on a commit (keeps issue open, adds `review-concern`). |
| --dismiss \<hash\>    | Dismiss a review (closes issue with `review-dismissed`).               |
| -m, --message         | Message for concern or dismiss (required with --concern/--dismiss).    |
| --since \<time\>      | Time range for digest (default: "1 day ago").                          |
| --reviewers \<users\> | Override default reviewers (comma-separated GitHub usernames).         |

**Examples:**

```bash
# Create a review issue for a specific commit
tbdflow review abc1234

# Create a review issue for the latest commit (HEAD)
tbdflow review --trigger

# See commits from the last 3 days that may need review
tbdflow review --digest --since "3 days ago"

# Mark a commit as reviewed (closes the associated GitHub issue)
tbdflow review --approve abc1234

# Raise a concern on a commit (keeps issue open, notifies author)
tbdflow review --concern abc1234 -m "Potential thread safety issue"

# Dismiss a review without fixing (closes issue)
tbdflow review --dismiss abc1234 -m "Won't fix, out of scope"
```

#### Review Labels (Nuanced Statuses)

`tbdflow` uses configurable labels to track review status throughout the lifecycle:

| Label              | Description                                     | Issue State |
|--------------------|-------------------------------------------------|-------------|
| `review-pending`   | Review awaiting attention (default on creation) | Open        |
| `review-concern`   | Concern raised - needs attention from author    | Open        |
| `review-accepted`  | Review approved                                 | Closed      |
| `review-dismissed` | Review dismissed (won't fix)                    | Closed      |

**Concern Workflow:**

When you raise a concern with `--concern`:

1. The issue label changes from `review-pending` to `review-concern`
2. A comment is added to the issue with the concern message
3. A checklist item is appended to the issue body: `- [ ] <concern>`
4. (Optional) A commit status is set based on `concern_blocks_status` config

This is **always non-blocking**, concerns are informational and encourage fix-forward patterns.

**Configuration:**

Enable the review system in your `.tbdflow.yml`:

```yaml
review:
  enabled: true
  strategy: github-issue  # or "github-workflow" or "log-only"
  default_reviewers:
    - teammate-username
    - another-reviewer

  # Optional: Customise label names (defaults shown)
  labels:
    pending: "review-pending"
    concern: "review-concern"
    accepted: "review-accepted"
    dismissed: "review-dismissed"

  # Optional: Set commit status to 'failure' when concern is raised
  # If false (default), status is 'pending' with description
  concern_blocks_status: false
```

**Commit Status Behaviour:**

When `concern_blocks_status` is configured:

| Setting           | Status State | Description                                   |
|-------------------|--------------|-----------------------------------------------|
| `false` (default) | `pending`    | "Awaiting fix-forward for concern: [message]" |
| `true`            | `failure`    | "Audit Concern: [message]"                    |

#### Targeted Review Rules

For teams that need specific reviewers for certain files or directories, you can configure **review rules** with glob
patterns. When rules are configured, reviews are **automatically triggered** after a commit if any changed files match
a rule pattern. The appropriate reviewers are assigned based on the matching rules.

This allows:

- **Opt-in by Default**: Without rules, `tbdflow review --trigger` is manual
- **Auto-trigger with Rules**: When rules are configured and files match, reviews are triggered automatically after
  commit
- **Smart Routing**: Database changes go to the DB expert, infrastructure changes go to DevOps, etc.

```yaml
review:
  enabled: true
  strategy: github-issue
  default_reviewers:
    - cladam

  rules:
    # Database changes get reviewed by the DB expert
    - pattern: "migrations/**"
      reviewers: [ "db-expert" ]

    # Targeted review for infrastructure changes
    - pattern: "infra/*.tf"
      reviewers: [ "devops-lead" ]

    # Targeted review for critical security modules
    - pattern: "src/auth/**"
      reviewers: [ "security-officer" ]
```

**Rule Options:**

| Field       | Description                                                              | Required |
|-------------|--------------------------------------------------------------------------|----------|
| `pattern`   | Glob pattern for files that trigger this rule (e.g., `src/auth/**`)      | Yes      |
| `reviewers` | List of reviewers specifically for these files (uses default if not set) | No       |

**Strategies:**

| Strategy          | Description                                            | Best For                             |
|-------------------|--------------------------------------------------------|--------------------------------------|
| `github-issue`    | CLI creates GitHub issues directly                     | Small teams, simple setup            |
| `github-workflow` | CLI triggers GitHub Actions for server-side management | Regulated environments, audit trails |
| `log-only`        | Local logging only, no external integration            | Offline or air-gapped environments   |

> **Note:** Both `github-issue` and `github-workflow` strategies require the [GitHub CLI (
`gh`)](https://cli.github.com/)
> to be installed and authenticated.

#### Server-Side Reviews with GitHub Actions

For teams that need **commit status gates**, **full audit trails**, or **multi-reviewer orchestration**, use the
`github-workflow` strategy. This triggers a GitHub Actions workflow that:

1. Creates review issues (even if someone bypasses the CLI)
2. Sets commit statuses (`pending` → `success`) for deploy gating
3. Handles multi-reviewer consensus automatically

To set up:

1. Copy `.github/workflows/nbr-review.yml.example` to `.github/workflows/nbr-review.yml`
2. Configure your `.tbdflow.yml`:

```yaml
review:
  enabled: true
  strategy: github-workflow
  workflow: nbr-review.yml
  default_reviewers:
    - teammate-username
```

3. Run `tbdflow review --trigger` and the workflow handles the rest

### 6. `task` and `note`

Think of these as your development scratch pad. Start a task, jot down what you're trying and why, and let the
commit pick it all up when you're ready.

**Usage:**

```bash
tbdflow task start <description>   # Start a named task
tbdflow task show                  # Show current task and notes
tbdflow task clear                 # Discard the intent log

tbdflow note <message>             # Log a note
tbdflow + <message>                # Shorthand alias
tbdflow n <message>                # Shorthand alias
```

**Options (`note`):**

| Flag   | Description                                          | Required |
|--------|------------------------------------------------------|----------|
| --show | Show the current intent log instead of adding a note | No       |

**Examples:**

```bash
# Start a task and leave breadcrumbs
tbdflow task start "Refactor auth module"
tbdflow + "tried decorator pattern, too much boilerplate"
tbdflow + "simple middleware chain works better"

# View what you've captured
tbdflow task show

# Notes are automatically included when you commit
tbdflow commit -t refactor -s auth -m "simplify auth middleware"
# The commit body will contain:
#   Intent Log:
#   - tried decorator pattern, felt too verbose
#   - simple middleware chain works better
```

### 7. `recover`

Lists and restores WIP snapshots captured by the WIP Guard.

**Usage:**

```bash
tbdflow recover --list             # Show available snapshots
tbdflow recover <index>            # Restore by index
tbdflow recover <hash>             # Restore by commit hash
```

**Example output:**

```
Available WIP snapshots:
  #     Type       Timestamp              Note                                     Hash
  ------------------------------------------------------------------------------------------
  1     intent     2026-04-18T14:15:00     trying trait-based approach              a7b8c9d0e1
  2     intent     2026-04-18T14:42:00     added error variants                     f2e3d4c5b6
  3     pre-sync   2026-04-18T15:01:00     Pre-sync safety snapshot                 e1f2g3h4i5
```

> Snapshots are branch-aware. If you switch branches,
> tbdflow warns you before applying a snapshot from a different context.

### 8. `radar`

Orient yourself before you start typing. `tbdflow radar` is the **situational-awareness dashboard** for
Trunk-Based Development. Run it first thing in the morning, or any time you sit back down at the keyboard.

It answers three questions at a glance:

1. **Is the trunk healthy?** (Trunk Status)
2. **Where is work concentrating?** (Hotspots / Churn)
3. **Is anyone touching the same files as me?** (Overlap Scan)

**Usage:**

```bash
tbdflow radar
```

**Example output:**

```text
--- Trunk Status ---
main is Green (Last integrated 12m ago)

--- Hotspots (Last 3 days) ---
  src/auth/logic.rs (14 changes)
  src/db/schema.sql (8 changes)

--- Scanning for overlapping work ---
Fetching latest from origin...
No local changes detected. Nothing to scan.
```

#### Trunk Status (The Heartbeat)

Shows the CI status of the trunk and how long ago the last commit landed. If CI checks are enabled (`ci_check.enabled:
true`), you'll see Green / Red / Pending; otherwise it shows Unknown.

> **Goal:** Provide a pulse for the repository. If main is red, fix-forward or wait and talk to a peer.

#### Hotspots (The Churn)

Lists the top files with the most changes on the trunk in the last 72 hours.

> **Goal:** Prevent "Blind Collisions." If a file is being thrashed, you should pair or wait before editing it.

#### Overlap Scan

Compares your uncommitted local changes against every active remote branch to surface file (or line) overlaps before
you push.

**Detection Levels** (configurable in `.tbdflow.yml`):

| Level  | What it checks                        | Speed        |
|--------|---------------------------------------|--------------|
| `file` | Same files touched (default)          | ~5ms/branch  |
| `line` | Overlapping line ranges in same files | ~50ms/branch |

**Example overlap output:**

```text
OVERLAP DETECTED with 1 active branch(es):

  feat/API-42-user-auth (by @alicia, 2 commits ahead)
  ├── src/auth/handler.rs [!!] LINE OVERLAP
  └── src/auth/middleware.rs [!] SAME FILE

  3 other active branch(es) have no overlap with your changes.

Hint: Coordinate with the overlapping author(s) before pushing.
```

**Integration:**

Radar is also integrated into other commands:

* **`tbdflow sync`** automatically shows a one-liner warning if overlap is detected.
* **`tbdflow commit`** optionally warns or prompts for confirmation before committing (configurable).

**Configuration:**

```yaml
radar:
  enabled: true
  level: file          # file | line
  on_sync: true        # Show warnings during tbdflow sync
  on_commit: warn      # off | warn | confirm
  ignore_patterns: # Files to exclude from overlap detection
    - "*.lock"
    - "*-lock.*"
    - "CHANGELOG.md"
```

### 9. Pre-flight CI check

When enabled, `tbdflow sync` checks the CI status of the trunk (via the `gh` CLI) **before** pulling.
If the trunk is red or pending, you get a prompt instead of blindly pulling a broken build.

**Configuration:**

```yaml
ci_check:
  enabled: true   # default: false
```

**Behaviour:**

| Trunk CI status | What happens                                            |
|-----------------|---------------------------------------------------------|
| Green           | Silent proceed, prints a brief confirmation             |
| Failed          | Warns and prompts: "Continue with sync? (y/N)"          |
| Pending         | Informs and prompts: "Pull anyway? (y/N)"               |
| Unknown         | Proceeds silently (e.g. `gh` not installed, no CI runs) |

> Requires the [GitHub CLI](https://cli.github.com/) (`gh`) to be installed and authenticated.

### 10. Utility commands

Not part of the core workflow, but handy for checking on things:

**Examples:**

```bash
# Does a pull, shows latest changes to main branch, and warns about stale branches.
# If ci_check is enabled, checks trunk CI status first.
tbdflow sync

# One-shot situational awareness (branch, clean, ahead/behind, CI, stale, overlaps)
tbdflow context

# Environment preflight: git, gh auth, GPG signing, and config
tbdflow doctor

# Bootstrap a brand-new repository (optionally create the GitHub remote via gh)
tbdflow init
tbdflow --non-interactive init --create-remote owner/name --private --push

# Inspect your current configuration
tbdflow info

# Checks the status of the working dir
tbdflow status

# Shows the current branch name
tbdflow current-branch

# Explicitly checks for local branches older than one day.
tbdflow check-branches

# Checks for a new version of tbdflow and updates it if available.
tbdflow update
```

#### `undo`

In TBD, the rule is simple: if the trunk breaks, fix it or revert it immediately. `tbdflow undo` is a smart wrapper
around `git revert` that syncs with the remote, verifies the commit is on the trunk, cleanly reverts it, and pushes,
all in one command.

**Usage:**

```bash
tbdflow undo <sha> [options]
```

**Options:**

| Flag      | Description                                       | Required |
|-----------|---------------------------------------------------|----------|
| --no-push | Create the revert commit locally without pushing. | No       |

**Examples:**

```bash
# Revert a specific commit on the trunk
tbdflow undo abc1234

# Revert locally without pushing (e.g. to inspect the result first)
tbdflow undo abc1234 --no-push

# Preview what would happen without making changes
tbdflow --dry-run undo abc1234
```

### 11. Advanced Usage

#### Shell Completion

Add tab-completion to your shell:

For Zsh (`~/.zshrc`):

```bash
eval "$(tbdflow generate-completion zsh)"
```

For Bash (`~/.bashrc`):

```bash
eval "$(tbdflow generate-completion bash)"
```

For Fish (`~/.config/fish/config.fish`):

```bash
tbdflow generate-completion fish | source
```

#### Man Page

```bash
tbdflow generate-man-page > tbdflow.1 && man tbdflow.1
```

## IDE support

`tbdflow` comes with IDE support for:

- [IntelliJ](https://github.com/cladam/tbdflow/tree/main/plugins/intellij)
- [VS Code](https://github.com/hekonsek/tbdflow-vscode-extension)

## Contributing

First off, thank you for considering contributing to `tbdflow`! ❤️

Please feel free to open an issue or submit a pull request.
