use crate::config::{Config, ReviewLabelsConfig, ReviewStrategy};
use crate::git::{self, RunOpts};
use anyhow::{Context, Result};
use colored::Colorize;
use glob::Pattern;
use serde_json::Value;

fn short_hash(hash: &str) -> &str {
    &hash[..7.min(hash.len())]
}

/// Returns true if any review rule patterns match the files changed in this commit.
pub fn should_auto_trigger_review(
    config: &Config,
    commit_hash: &str,
    opts: RunOpts,
) -> Result<bool> {
    if !config.review.enabled || config.review.rules.is_empty() {
        return Ok(false);
    }

    let touched_files = git::get_changed_files(commit_hash, opts)?;

    for rule in &config.review.rules {
        if let Ok(pattern) = Pattern::new(&rule.pattern) {
            if touched_files.iter().any(|f| pattern.matches(f)) {
                if opts.verbose {
                    crate::say!(
                        "{} Auto-trigger: files match rule pattern '{}'",
                        "[REVIEW]".magenta(),
                        rule.pattern
                    );
                }
                return Ok(true);
            }
        }
    }

    Ok(false)
}

pub fn trigger_review(
    config: &Config,
    reviewers_override: Option<&[String]>,
    commit_hash: &str,
    message: &str,
    author: &str,
    opts: RunOpts,
) -> Result<()> {
    if !config.review.enabled {
        if opts.verbose {
            crate::say!("{}", "Review system is disabled in config.".dimmed());
        }
        return Ok(());
    }

    // Identify which rules apply based on touched files
    let touched_files = git::get_changed_files(commit_hash, opts)?;
    let mut applicable_reviewers: Vec<String> = Vec::new();
    let mut is_targeted = false;

    for rule in &config.review.rules {
        if let Ok(pattern) = Pattern::new(&rule.pattern) {
            let matched = touched_files.iter().any(|f| pattern.matches(f));
            if matched {
                if opts.verbose {
                    crate::say!(
                        "{} File match for rule: {}",
                        "[RULE]".magenta(),
                        rule.pattern.dimmed()
                    );
                }
                is_targeted = true;
                if let Some(rule_reviewers) = &rule.reviewers {
                    applicable_reviewers.extend(rule_reviewers.clone());
                }
            }
        }
    }

    let mut final_reviewers = if let Some(ovr) = reviewers_override {
        ovr.to_vec()
    } else if !applicable_reviewers.is_empty() {
        applicable_reviewers
    } else {
        config.review.default_reviewers.clone()
    };

    final_reviewers.sort();
    final_reviewers.dedup();

    crate::say!("{}", "--- Triggering Non-blocking Review ---".blue());
    if is_targeted {
        crate::say!("{} Review triggered by targeted file rules.", ">>".yellow());
    }

    let short = short_hash(commit_hash);
    crate::say!(
        "{} {} ({})",
        "Review requested for:".green(),
        message.bold(),
        short.dimmed()
    );
    crate::say!("   Author: {}", author);
    if !final_reviewers.is_empty() {
        crate::say!("   Reviewers: {}", final_reviewers.join(", "));
    }

    if opts.dry_run {
        crate::say!("{}", "[DRY RUN] Would create review request".yellow());
        return Ok(());
    }

    match &config.review.strategy {
        ReviewStrategy::GithubIssue => {
            create_github_issue(
                &config.review.labels,
                &final_reviewers,
                commit_hash,
                message,
                author,
                opts,
            )?;
        }
        ReviewStrategy::GithubWorkflow => {
            trigger_github_workflow(config, commit_hash, message, author, &final_reviewers, opts)?;
        }
        ReviewStrategy::LogOnly => {
            crate::say!(
                "{}",
                "Review logged (no external system integration)".dimmed()
            );
        }
    }

    Ok(())
}

fn trigger_github_workflow(
    config: &Config,
    commit_hash: &str,
    message: &str,
    author: &str,
    reviewers: &[String],
    opts: RunOpts,
) -> Result<()> {
    if !is_gh_cli_available() {
        crate::say!(
            "{}",
            "Warning: GitHub CLI (gh) not found. Install it to trigger workflows.".yellow()
        );
        crate::say!(
            "{}",
            "Install: https://cli.github.com/ or 'brew install gh'".dimmed()
        );
        return Ok(());
    }

    let workflow_name = config
        .review
        .workflow
        .as_deref()
        .unwrap_or("nbr-review.yml");

    let short = short_hash(commit_hash);

    if opts.verbose {
        crate::say!(
            "{} Triggering workflow '{}' for commit {}",
            "[INFO]".cyan(),
            workflow_name,
            short
        );
    }

    // Build workflow inputs as JSON
    let reviewers_json = reviewers.join(",");

    let output = crate::gh::output(
        &[
            "workflow",
            "run",
            workflow_name,
            "-f",
            &format!("commit_sha={}", commit_hash),
            "-f",
            &format!("commit_message={}", message),
            "-f",
            &format!("author={}", author),
            "-f",
            &format!("reviewers={}", reviewers_json),
        ],
        opts,
    )
    .context("Failed to trigger GitHub workflow")?;

    if output.status.success() {
        crate::say!(
            "{}",
            format!(
                "Workflow '{}' triggered for commit {}",
                workflow_name, short
            )
            .green()
        );
        crate::say!(
            "{}",
            "   Server-side review management is now active.".dimmed()
        );
        crate::say!(
            "{}",
            "   Check GitHub Actions for issue creation and status updates.".dimmed()
        );
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("could not find any workflows") {
            crate::say!(
                "{}",
                format!(
                    "Warning: Workflow '{}' not found in repository.",
                    workflow_name
                )
                .yellow()
            );
            crate::say!(
                "{}",
                "   Create the workflow file at .github/workflows/ to enable server-side reviews."
                    .dimmed()
            );
            crate::say!(
                "{}",
                "   Falling back to client-side issue creation...".dimmed()
            );
            // Fallback to client-side issue creation
            create_github_issue(
                &config.review.labels,
                reviewers,
                commit_hash,
                message,
                author,
                opts,
            )?;
        } else {
            crate::say!(
                "{}",
                format!("Warning: Failed to trigger workflow: {}", stderr.trim()).yellow()
            );
        }
    }

    Ok(())
}

fn create_github_issue(
    labels: &ReviewLabelsConfig,
    reviewers: &[String],
    commit_hash: &str,
    message: &str,
    author: &str,
    opts: RunOpts,
) -> Result<()> {
    let short = short_hash(commit_hash);

    // Check if gh CLI is available
    if !is_gh_cli_available() {
        crate::say!(
            "{}",
            "Warning: GitHub CLI (gh) not found. Install it to enable GitHub issue creation."
                .yellow()
        );
        crate::say!(
            "{}",
            "Install: https://cli.github.com/ or 'brew install gh'".dimmed()
        );
        return Ok(());
    }

    // Ensure all review labels exist (create if missing)
    ensure_review_labels_exist(labels, opts);

    // Get the repository URL for commit links
    let repo_url = git::get_remote_url(opts).unwrap_or_default();
    let commit_url = if repo_url.is_empty() {
        format!("`{}`", commit_hash)
    } else {
        format!("[`{}`]({}/commit/{})", short, repo_url, commit_hash)
    };

    let title = format!("[Review] {} ({})", message, short);
    let body = format!(
        "## Non-blocking Review Request\n\n\
        **Commit:** {}\n\
        **Author:** {}\n\
        **Message:** {}\n\n\
        ---\n\n\
        > In Trunk-Based Development, this code is already in the trunk.\n\
        > Your goal is **Course Correction** and **Knowledge Sharing**, not gatekeeping.\n\n\
        ### What to Look For\n\n\
        | Focus | Question |\n\
        |-------|----------|\n\
        | **Design & Intent** | Does the implementation align with our architectural patterns? |\n\
        | **Logic & Edge Cases** | Are there logical flaws or unhappy paths that tests might miss? |\n\
        | **Readability** | Are names descriptive? (Code as Documentation) |\n\
        | **Simplification** | Can this be done with less code or lower complexity? |\n\n\
        ### How to Comment\n\n\
        - **Questions > Commands**: _\"Could we use the existing helper here?\"_ instead of _\"Change this.\"_\n\
        - **Praise**: If you see something clever or clean, say so! NBR boosts team morale.\n\
        - **Nitpicking**: Label minor style issues as `(nit)` so the author knows they're optional.\n\n\
        ### Concerns\n\n\
        _No concerns raised yet._\n\n\
        ---\n\n\
        To approve via CLI:\n\
        ```\n\
        tbdflow review --approve {}\n\
        ```\n\n\
        To raise a concern:\n\
        ```\n\
        tbdflow review --concern {} -m \"Your concern here\"\n\
        ```",
        commit_url, author, message, short, short
    );

    let mut args = vec!["issue", "create", "--title", &title, "--body", &body];

    // Add the pending label
    if label_exists(&labels.pending, opts) {
        args.push("--label");
        args.push(&labels.pending);
    }

    // Add assignees if configured
    let assignees: Vec<&str> = reviewers.iter().map(String::as_str).collect();
    let assignees_str = assignees.join(",");
    if !assignees.is_empty() {
        args.push("--assignee");
        args.push(&assignees_str);
    }

    let output = crate::gh::output(&args, opts).context("Failed to execute 'gh' CLI")?;

    if output.status.success() {
        let issue_url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        crate::say!("{} {}", "Review issue created:".green(), issue_url);
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        crate::say!(
            "{}",
            format!("Warning: Failed to create GitHub issue: {}", stderr).yellow()
        );
    }

    Ok(())
}

fn label_exists(label_name: &str, opts: RunOpts) -> bool {
    let (ok, stdout) = crate::gh::run_lenient(
        &["label", "list", "--search", label_name, "--json", "name"],
        opts,
    );
    ok && stdout.contains(&format!("\"name\":\"{}\"", label_name))
}

fn ensure_label_exists(label_name: &str, description: &str, color: &str, opts: RunOpts) {
    if label_exists(label_name, opts) {
        return;
    }

    let (ok, _) = crate::gh::run_lenient(
        &[
            "label",
            "create",
            label_name,
            "--description",
            description,
            "--color",
            color,
        ],
        opts,
    );
    if ok && opts.verbose {
        crate::say!("{} Created '{}' label", "[INFO]".cyan(), label_name);
    }
    // Otherwise continue silently: label creation may fail due to permissions;
    // the issue is still created, just without the label.
}

fn ensure_review_labels_exist(labels: &ReviewLabelsConfig, opts: RunOpts) {
    ensure_label_exists(
        &labels.pending,
        "Review pending - awaiting attention",
        "FBCA04", // Yellow
        opts,
    );
    ensure_label_exists(
        &labels.concern,
        "Review concern raised - needs attention",
        "D93F0B", // Red-orange
        opts,
    );
    ensure_label_exists(
        &labels.accepted,
        "Review accepted/approved",
        "0E8A16", // Green
        opts,
    );
    ensure_label_exists(
        &labels.dismissed,
        "Review dismissed - won't fix",
        "6A737D", // Gray
        opts,
    );
}

fn is_gh_cli_available() -> bool {
    crate::gh::available()
}

pub fn handle_review_trigger(
    config: &Config,
    reviewers_override: Option<Vec<String>>,
    commit_sha: Option<&str>,
    opts: RunOpts,
) -> Result<()> {
    if !config.review.enabled {
        crate::say!(
            "{}",
            "Review system is not enabled. Add the following to your .tbdflow.yml:".yellow()
        );
        crate::say!("\n  review:");
        crate::say!("    enabled: true");
        crate::say!("    strategy: github-issue");
        crate::say!("    default_reviewers:");
        crate::say!("      - teammate-username\n");
        return Ok(());
    }

    let commit_hash = match commit_sha {
        Some(sha) if !sha.is_empty() => {
            // Resolve the provided SHA to a full hash
            let full = git::resolve_commit_hash(sha, opts)?;
            if opts.verbose {
                crate::say!(
                    "{} Triggering review for commit {}",
                    "[REVIEW]".magenta(),
                    short_hash(&full)
                );
            }
            full
        }
        _ => git::get_head_commit_hash(opts)?,
    };
    let message = git::get_commit_message(&commit_hash, opts)?;
    let author = git::get_user_name(opts)?;

    trigger_review(
        config,
        reviewers_override.as_deref(),
        &commit_hash,
        &message,
        &author,
        opts,
    )
}

pub fn handle_review_digest(config: &Config, since: &str, opts: RunOpts) -> Result<()> {
    crate::say!(
        "{}",
        format!("--- Trunk Evolution Digest (Since {}) ---", since).blue()
    );

    let log = git::get_log_since(since, opts)?;

    if log.is_empty() {
        crate::say!(
            "{}",
            "No new commits found in the specified time range.".yellow()
        );
        return Ok(());
    }

    crate::say!("\n{}", "COMMITS FOR REVIEW".cyan().bold());
    crate::say!("{}", "─".repeat(50).cyan());

    for line in log.lines() {
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(3, '|').collect();
        if parts.len() >= 2 {
            let hash = short_hash(parts[0]);
            let author = parts.get(1).unwrap_or(&"unknown");
            let message = parts.get(2).unwrap_or(&"");
            crate::say!(
                "  {} {} {}",
                hash.yellow(),
                format!("({})", author).dimmed(),
                message
            );
        }
    }

    crate::say!("{}", "─".repeat(50).cyan());

    if !config.review.default_reviewers.is_empty() {
        crate::say!(
            "\n{}",
            format!(
                "Default reviewers: {}",
                config.review.default_reviewers.join(", ")
            )
            .dimmed()
        );
    }

    crate::say!("\n{}", "Next steps:".bold());
    crate::say!("   • Review commits above and discuss with the team");
    crate::say!("   • Run 'tbdflow review --approve <hash>' to mark as reviewed");
    crate::say!("   • Run 'tbdflow review --trigger' to create review issues\n");

    Ok(())
}

pub fn handle_review_approve(config: &Config, commit_hash: &str, opts: RunOpts) -> Result<()> {
    let short = short_hash(commit_hash);

    crate::say!("{}", format!("--- Approving Commit {} ---", short).blue());

    if opts.dry_run {
        crate::say!("{}", "[DRY RUN] Would mark commit as approved".yellow());
        return Ok(());
    }

    match &config.review.strategy {
        ReviewStrategy::GithubIssue => {
            close_github_review_issue(&config.review.labels, short, opts)?;
        }
        ReviewStrategy::GithubWorkflow => {
            // For workflow strategy, close the issue which will trigger
            // the server-side Action to update commit status
            close_github_review_issue(&config.review.labels, short, opts)?;
            crate::say!(
                "{}",
                "   Server-side workflow will update commit status.".dimmed()
            );
        }
        ReviewStrategy::LogOnly => {
            crate::say!("{}", format!("Commit {} marked as approved", short).green());
        }
    }

    Ok(())
}

pub fn handle_review_concern(
    config: &Config,
    commit_hash: &str,
    message: &str,
    opts: RunOpts,
) -> Result<()> {
    let short = short_hash(commit_hash);

    crate::say!(
        "{}",
        format!("--- Raising Concern on Commit {} ---", short).blue()
    );

    if opts.dry_run {
        crate::say!("{}", "[DRY RUN] Would raise concern on commit".yellow());
        return Ok(());
    }

    match &config.review.strategy {
        ReviewStrategy::GithubIssue | ReviewStrategy::GithubWorkflow => {
            raise_github_concern(config, commit_hash, message, opts)?;
        }
        ReviewStrategy::LogOnly => {
            crate::say!("{}", format!("CONCERN on {}: {}", short, message).yellow());
        }
    }

    Ok(())
}

pub fn handle_review_dismiss(
    config: &Config,
    commit_hash: &str,
    message: &str,
    opts: RunOpts,
) -> Result<()> {
    let short = short_hash(commit_hash);

    crate::say!(
        "{}",
        format!("--- Dismissing Review for Commit {} ---", short).blue()
    );

    if opts.dry_run {
        crate::say!("{}", "[DRY RUN] Would dismiss review".yellow());
        return Ok(());
    }

    match &config.review.strategy {
        ReviewStrategy::GithubIssue | ReviewStrategy::GithubWorkflow => {
            dismiss_github_review_issue(&config.review.labels, short, message, opts)?;
        }
        ReviewStrategy::LogOnly => {
            crate::say!(
                "{}",
                format!("Review for {} dismissed: {}", short, message).dimmed()
            );
        }
    }

    Ok(())
}

fn raise_github_concern(
    config: &Config,
    commit_hash: &str,
    message: &str,
    opts: RunOpts,
) -> Result<()> {
    let short = short_hash(commit_hash);
    let labels = &config.review.labels;

    if !is_gh_cli_available() {
        crate::say!(
            "{}",
            "Warning: GitHub CLI (gh) not found. Cannot raise concern.".yellow()
        );
        return Ok(());
    }

    // Search for the review issue
    let search_query = format!("[Review] in:title {} in:title is:open", short);

    if opts.verbose {
        crate::say!("{} Searching for review issue...", "[INFO]".cyan());
    }

    let (ok, json_output) = crate::gh::run_lenient(
        &[
            "issue",
            "list",
            "--search",
            &search_query,
            "--json",
            "number,body",
            "--limit",
            "1",
        ],
        opts,
    );

    if !ok {
        crate::say!(
            "{}",
            format!("Warning: Could not find review issue for {}", short).yellow()
        );
        return Ok(());
    }

    if let Some(issue_num) = extract_issue_number(&json_output) {
        let issue_num_str = issue_num.to_string();

        // Update labels: remove pending, add concern
        if opts.verbose {
            crate::say!(
                "{} Updating labels on issue #{}",
                "[INFO]".cyan(),
                issue_num
            );
        }

        let _ = crate::gh::run_lenient(
            &[
                "issue",
                "edit",
                &issue_num_str,
                "--remove-label",
                &labels.pending,
            ],
            opts,
        );

        let _ = crate::gh::run_lenient(
            &[
                "issue",
                "edit",
                &issue_num_str,
                "--add-label",
                &labels.concern,
            ],
            opts,
        );

        // Add a comment with the concern
        let comment = format!("**Concern Raised**\n\n{}", message);

        let _ = crate::gh::run_lenient(
            &["issue", "comment", &issue_num_str, "--body", &comment],
            opts,
        );

        // Append checklist item to the issue body
        append_concern_checklist_item(&issue_num_str, message, opts)?;

        // Set commit status based on config
        set_commit_status(config, commit_hash, message, opts)?;

        crate::say!(
            "{}",
            format!(
                "Concern raised on issue #{} for commit {} (label: {})",
                issue_num, short, labels.concern
            )
            .yellow()
        );
    } else {
        crate::say!(
            "{}",
            format!("Warning: No open review issue found for commit {}", short).yellow()
        );
        crate::say!("   Run 'tbdflow review --trigger' first to create the review issue.");
    }

    Ok(())
}

fn append_concern_checklist_item(
    issue_num: &str,
    concern_message: &str,
    opts: RunOpts,
) -> Result<()> {
    // Get current issue body
    let (ok, json_output) =
        crate::gh::run_lenient(&["issue", "view", issue_num, "--json", "body"], opts);
    if !ok {
        return Ok(());
    }

    // Extract the body content
    let current_body = extract_body_from_json(&json_output).unwrap_or_default();

    // Replace the "No concerns raised yet" placeholder or append to concerns section
    let new_body = if current_body.contains("_No concerns raised yet._") {
        current_body.replace(
            "_No concerns raised yet._",
            &format!("- [ ] {}", concern_message),
        )
    } else if current_body.contains("### Concerns") {
        // Find the concerns section and append the new item
        let concerns_marker = "### Concerns\n\n";
        if let Some(pos) = current_body.find(concerns_marker) {
            let insert_pos = pos + concerns_marker.len();
            let (before, after) = current_body.split_at(insert_pos);
            format!("{}- [ ] {}\n{}", before, concern_message, after)
        } else {
            current_body
        }
    } else {
        current_body
    };

    if opts.verbose {
        crate::say!(
            "{} Updating issue body with concern checklist item",
            "[INFO]".cyan()
        );
    }

    let _ = crate::gh::run_lenient(&["issue", "edit", issue_num, "--body", &new_body], opts);

    Ok(())
}

fn extract_body_from_json(json: &str) -> Option<String> {
    let parsed: Value = serde_json::from_str(json).ok()?;
    parsed["body"].as_str().map(|s| s.to_string())
}

fn set_commit_status(
    config: &Config,
    commit_hash: &str,
    message: &str,
    opts: RunOpts,
) -> Result<()> {
    if !is_gh_cli_available() {
        return Ok(());
    }

    let (state, description) = if config.review.concern_blocks_status {
        ("failure", format!("Audit Concern: {}", message))
    } else {
        (
            "pending",
            format!("Awaiting fix-forward for concern: {}", message),
        )
    };

    // Get repo owner/name
    let Some((owner, name)) = crate::gh::repo_owner_name(opts) else {
        return Ok(());
    };

    if opts.verbose {
        crate::say!(
            "{} Setting commit status to '{}' for {}",
            "[INFO]".cyan(),
            state,
            short_hash(commit_hash)
        );
    }

    let api_path = format!("repos/{}/{}/statuses/{}", owner, name, commit_hash);

    let _ = crate::gh::run_lenient(
        &[
            "api",
            &api_path,
            "-f",
            &format!("state={}", state),
            "-f",
            "context=peer-review",
            "-f",
            &format!("description={}", description),
        ],
        opts,
    );

    Ok(())
}

fn dismiss_github_review_issue(
    labels: &ReviewLabelsConfig,
    short_hash: &str,
    message: &str,
    opts: RunOpts,
) -> Result<()> {
    if !is_gh_cli_available() {
        crate::say!(
            "{}",
            "Warning: GitHub CLI (gh) not found. Cannot dismiss review.".yellow()
        );
        return Ok(());
    }

    // Search for the review issue
    let search_query = format!("[Review] in:title {} in:title is:open", short_hash);

    if opts.verbose {
        crate::say!("{} Searching for review issue...", "[INFO]".cyan());
    }

    let (ok, json_output) = crate::gh::run_lenient(
        &[
            "issue",
            "list",
            "--search",
            &search_query,
            "--json",
            "number",
            "--limit",
            "1",
        ],
        opts,
    );

    if ok {
        if let Some(issue_num) = extract_issue_number(&json_output) {
            let issue_num_str = issue_num.to_string();

            // Update labels: remove pending/concern, add dismissed
            if opts.verbose {
                crate::say!(
                    "{} Updating labels on issue #{}",
                    "[INFO]".cyan(),
                    issue_num
                );
            }

            let _ = crate::gh::run_lenient(
                &[
                    "issue",
                    "edit",
                    &issue_num_str,
                    "--remove-label",
                    &labels.pending,
                ],
                opts,
            );

            let _ = crate::gh::run_lenient(
                &[
                    "issue",
                    "edit",
                    &issue_num_str,
                    "--remove-label",
                    &labels.concern,
                ],
                opts,
            );

            let _ = crate::gh::run_lenient(
                &[
                    "issue",
                    "edit",
                    &issue_num_str,
                    "--add-label",
                    &labels.dismissed,
                ],
                opts,
            );

            // Close with a comment
            let comment = format!(
                "**Dismissed** via `tbdflow review --dismiss`\n\nReason: {}",
                message
            );

            let (close_ok, _) = crate::gh::run_lenient(
                &["issue", "close", &issue_num_str, "--comment", &comment],
                opts,
            );

            if close_ok {
                crate::say!(
                    "{}",
                    format!(
                        "Review for commit {} dismissed and issue #{} closed (label: {})",
                        short_hash, issue_num, labels.dismissed
                    )
                    .dimmed()
                );
            } else {
                crate::say!(
                    "{}",
                    "Review dismissed (issue close failed)".to_string().yellow()
                );
            }
        } else {
            crate::say!(
                "{}",
                format!(
                    "Review for {} dismissed (no open review issue found)",
                    short_hash
                )
                .dimmed()
            );
        }
    } else {
        crate::say!(
            "{}",
            format!("Review for {} dismissed", short_hash).dimmed()
        );
    }

    Ok(())
}

fn close_github_review_issue(
    labels: &ReviewLabelsConfig,
    short_hash: &str,
    opts: RunOpts,
) -> Result<()> {
    if !is_gh_cli_available() {
        crate::say!(
            "{}",
            "Warning: GitHub CLI (gh) not found. Marking as approved locally only.".yellow()
        );
        crate::say!("{}", format!("Commit {} approved", short_hash).green());
        return Ok(());
    }

    // Search for the review issue
    let search_query = format!("[Review] in:title {} in:title is:open", short_hash);

    if opts.verbose {
        crate::say!("{} Searching for review issue...", "[INFO]".cyan());
    }

    let (ok, json_output) = crate::gh::run_lenient(
        &[
            "issue",
            "list",
            "--search",
            &search_query,
            "--json",
            "number",
            "--limit",
            "1",
        ],
        opts,
    );

    if ok {
        // Simple JSON parsing for issue number
        if let Some(issue_num) = extract_issue_number(&json_output) {
            let issue_num_str = issue_num.to_string();

            // Remove pending/concern labels and add accepted label
            if opts.verbose {
                crate::say!(
                    "{} Updating labels on issue #{}",
                    "[INFO]".cyan(),
                    issue_num
                );
            }

            let _ = crate::gh::run_lenient(
                &[
                    "issue",
                    "edit",
                    &issue_num_str,
                    "--remove-label",
                    &labels.pending,
                ],
                opts,
            );

            let _ = crate::gh::run_lenient(
                &[
                    "issue",
                    "edit",
                    &issue_num_str,
                    "--remove-label",
                    &labels.concern,
                ],
                opts,
            );

            let _ = crate::gh::run_lenient(
                &[
                    "issue",
                    "edit",
                    &issue_num_str,
                    "--add-label",
                    &labels.accepted,
                ],
                opts,
            );

            if opts.verbose {
                crate::say!("{} Closing issue #{}", "[INFO]".cyan(), issue_num);
            }

            let (close_ok, _) = crate::gh::run_lenient(
                &[
                    "issue",
                    "close",
                    &issue_num_str,
                    "--comment",
                    "Approved via `tbdflow review --approve`",
                ],
                opts,
            );

            if close_ok {
                crate::say!(
                    "{}",
                    format!(
                        "Commit {} approved and review issue #{} closed (label: {})",
                        short_hash, issue_num, labels.accepted
                    )
                    .green()
                );
            } else {
                crate::say!(
                    "{}",
                    format!("Commit {} approved (issue close failed)", short_hash).yellow()
                );
            }
        } else {
            crate::say!(
                "{}",
                format!(
                    "Commit {} approved (no open review issue found)",
                    short_hash
                )
                .green()
            );
        }
    } else {
        crate::say!("{}", format!("Commit {} approved", short_hash).green());
    }

    Ok(())
}

fn extract_issue_number(json: &str) -> Option<i64> {
    let parsed: Value = serde_json::from_str(json).ok()?;
    parsed.as_array()?.first()?["number"].as_i64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_hash_returns_first_seven_chars() {
        assert_eq!(short_hash("abc1234567890"), "abc1234");
    }

    #[test]
    fn short_hash_handles_exact_seven_chars() {
        assert_eq!(short_hash("abc1234"), "abc1234");
    }

    #[test]
    fn short_hash_handles_short_input() {
        assert_eq!(short_hash("abc"), "abc");
    }

    #[test]
    fn short_hash_handles_empty_input() {
        assert_eq!(short_hash(""), "");
    }

    #[test]
    fn extract_issue_number_parses_valid_json() {
        let json = r#"[{"number":123}]"#;
        assert_eq!(extract_issue_number(json), Some(123));
    }

    #[test]
    fn extract_issue_number_parses_larger_number() {
        let json = r#"[{"number":98765}]"#;
        assert_eq!(extract_issue_number(json), Some(98765));
    }

    #[test]
    fn extract_issue_number_returns_none_for_empty_array() {
        let json = r#"[]"#;
        assert_eq!(extract_issue_number(json), None);
    }

    #[test]
    fn extract_issue_number_returns_none_for_invalid_json() {
        let json = r#"not json"#;
        assert_eq!(extract_issue_number(json), None);
    }

    #[test]
    fn extract_issue_number_handles_whitespace() {
        let json = r#"[{"number": 42}]"#;
        assert_eq!(extract_issue_number(json), Some(42));
    }
}
