//! Centralised access to the GitHub CLI (`gh`).
//!
//! Previously `gh` was shelled out ad-hoc and `is_gh_cli_available` was
//! duplicated across modules. This module is the single place that knows how to
//! find `gh`, check auth, run a command, and parse common results. Every
//! invocation is traced through [`crate::report`] when verbose, so it shows up in
//! the TOON `trace[]` alongside git commands.
//!
//! `gh` already behaves well in non-TTY contexts (it skips the pager, strips
//! color, and errors out instead of prompting), so no extra flags are needed.

use crate::git::RunOpts;
use crate::report;
use anyhow::{Context, Result};
use serde_json::Value;
use std::process::{Command, Stdio};

/// True if the `gh` binary is on the PATH.
pub fn available() -> bool {
    Command::new("gh")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// True if `gh auth status` reports an authenticated account.
pub fn auth_ok(opts: RunOpts) -> bool {
    if !available() {
        return false;
    }
    if opts.verbose {
        report::trace("gh", &["auth", "status"]);
    }
    Command::new("gh")
        .args(["auth", "status"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Run a `gh` command and return trimmed stdout. Errors carry stderr.
pub fn run(args: &[&str], opts: RunOpts) -> Result<String> {
    if opts.verbose {
        report::trace("gh", args);
    }
    let output = Command::new("gh")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("Failed to execute 'gh {}'", args.join(" ")))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(anyhow::anyhow!("gh {} failed: {}", args.join(" "), stderr))
    }
}

/// Run a `gh` command, returning success and trimmed stdout without erroring on
/// a non-zero exit (useful for best-effort label/status calls).
pub fn run_lenient(args: &[&str], opts: RunOpts) -> (bool, String) {
    if opts.verbose {
        report::trace("gh", args);
    }
    match Command::new("gh").args(args).output() {
        Ok(o) => (
            o.status.success(),
            String::from_utf8_lossy(&o.stdout).trim().to_string(),
        ),
        Err(_) => (false, String::new()),
    }
}

/// Run a `gh` command and return the raw process output (status + stdout +
/// stderr). Use when the caller needs to inspect stderr, e.g. to distinguish
/// "workflow not found" from other failures.
pub fn output(args: &[&str], opts: RunOpts) -> std::io::Result<std::process::Output> {
    if opts.verbose {
        report::trace("gh", args);
    }
    Command::new("gh").args(args).output()
}

/// Resolve the current repository's `(owner, name)` via `gh repo view`.
pub fn repo_owner_name(opts: RunOpts) -> Option<(String, String)> {
    let json = run(&["repo", "view", "--json", "owner,name"], opts).ok()?;
    let parsed: Value = serde_json::from_str(&json).ok()?;
    let owner = parsed["owner"]["login"].as_str()?.to_string();
    let name = parsed["name"].as_str()?.to_string();
    Some((owner, name))
}

/// Create a GitHub repository for an existing local repo and wire up `origin`.
/// Mirrors `gh repo create <name> --source=. --remote=origin [--public|--private] [--push]`.
pub fn create_repo(
    name: &str,
    private: bool,
    push: bool,
    opts: RunOpts,
) -> Result<String> {
    let visibility = if private { "--private" } else { "--public" };
    let mut args = vec![
        "repo",
        "create",
        name,
        "--source=.",
        "--remote=origin",
        visibility,
    ];
    if push {
        args.push("--push");
    }
    run(&args, opts)
}
