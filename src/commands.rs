use crate::git::RunOpts;
use crate::toon::Toon;
use crate::{config, gh, git, intent, radar, report};
use anyhow::Result;
use clap::Command as Commands;
use colored::*;
use dialoguer::{theme::ColorfulTheme, Confirm, Input};
use std::env;
use std::fs;
use std::path::PathBuf;

pub fn handle_update_command() -> Result<(), anyhow::Error> {
    crate::say!("{}", "--- Checking for updates ---".blue());
    let status = self_update::backends::github::Update::configure()
        .repo_owner("cladam")
        .repo_name("tbdflow")
        .bin_name("tbdflow")
        .show_download_progress(true)
        .current_version(self_update::cargo_crate_version!())
        .build()?
        .update()?;

    crate::say!("Update status: `{}`!", status.version());
    if status.updated() {
        crate::say!("{}", "Successfully updated tbdflow!".green());
    } else {
        crate::say!("{}", "tbdflow is already up to date.".green());
    }
    Ok(())
}

/// A single environment check produced by `tbdflow doctor`.
struct Check {
    name: String,
    ok: bool,
    detail: String,
}

fn bin_available(bin: &str) -> bool {
    std::process::Command::new(bin)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Verifies the environment tbdflow relies on: git, the GitHub CLI (installed +
/// authenticated), GPG signing, and the loaded configuration.
pub fn handle_doctor(opts: RunOpts, config: &config::Config) -> Result<()> {
    crate::say!("{}", "--- tbdflow doctor ---".blue());
    let mut checks: Vec<Check> = Vec::new();

    // git
    let git_ok = bin_available("git");
    checks.push(Check {
        name: "git".into(),
        ok: git_ok,
        detail: if git_ok {
            "installed".into()
        } else {
            "not found on PATH".into()
        },
    });

    let in_repo = git::is_git_repository(opts).is_ok();
    checks.push(Check {
        name: "git-repo".into(),
        ok: in_repo,
        detail: if in_repo {
            "inside a work tree".into()
        } else {
            "not a git repository (run 'tbdflow init')".into()
        },
    });

    if in_repo {
        let unborn = git::is_unborn_head(opts);
        checks.push(Check {
            // Informational: an unborn repo is a valid starting state, not a fault.
            name: "commits".into(),
            ok: true,
            detail: if unborn {
                "unborn branch (no commits yet) — run 'tbdflow commit' to start".into()
            } else {
                "has history".into()
            },
        });
    }

    // gh CLI
    let gh_ok = gh::available();
    checks.push(Check {
        name: "gh".into(),
        ok: gh_ok,
        detail: if gh_ok {
            "installed".into()
        } else {
            "not found (review/CI features limited)".into()
        },
    });
    if gh_ok {
        let auth = gh::auth_ok(opts);
        checks.push(Check {
            name: "gh-auth".into(),
            ok: auth,
            detail: if auth {
                "authenticated".into()
            } else {
                "not authenticated (run 'gh auth login')".into()
            },
        });
    }

    // gpg signing
    let gpg_ok = bin_available("gpg");
    let key = git::signing_key(opts);
    match (&key, gpg_ok) {
        (Some(k), true) => {
            let secret_present = std::process::Command::new("gpg")
                .args(["--list-secret-keys", k])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            checks.push(Check {
                name: "signing".into(),
                ok: secret_present,
                detail: if secret_present {
                    format!("commits sign with key {}", k)
                } else {
                    format!("key {} configured but no secret key available", k)
                },
            });
        }
        (Some(k), false) => checks.push(Check {
            name: "signing".into(),
            ok: false,
            detail: format!("key {} configured but gpg not installed", k),
        }),
        (None, _) => checks.push(Check {
            name: "signing".into(),
            ok: true,
            detail: "no signing key configured (commits unsigned)".into(),
        }),
    }

    // signed tags: verify the latest tag's signature (verification only needs
    // the public key, so it never prompts).
    if in_repo {
        if let Ok(tag) = git::get_latest_tag(opts) {
            let verified = git::verify_tag(&tag, opts);
            let key_configured = key.is_some();
            let (ok, detail) = if verified {
                (true, format!("latest tag '{}' has a valid signature", tag))
            } else if key_configured {
                (
                    false,
                    format!(
                        "latest tag '{}' is unsigned/unverifiable despite a signing key",
                        tag
                    ),
                )
            } else {
                (
                    true,
                    format!("latest tag '{}' is unsigned (no signing key configured)", tag),
                )
            };
            checks.push(Check {
                name: "signed-tags".into(),
                ok,
                detail,
            });
        }
    }

    // config
    checks.push(Check {
        name: "config".into(),
        ok: true,
        detail: format!(
            "main branch '{}', linting {}",
            config.main_branch_name,
            if config.lint.is_some() {
                "enabled"
            } else {
                "disabled"
            }
        ),
    });

    // Human output
    for c in &checks {
        let mark = if c.ok { "ok".green() } else { "FAIL".red() };
        crate::say!("  [{}] {}: {}", mark, c.name.bold(), c.detail.dimmed());
    }

    // Resolved agent-mode defaults (so an agent can confirm it's configured).
    let ni_source = crate::runtime::non_interactive_source();
    crate::say!(
        "\n{} non-interactive={} toon={} no-sign={}{}",
        "Agent defaults:".bold(),
        opts.non_interactive,
        opts.toon,
        opts.no_sign,
        match ni_source {
            Some(s) => format!(" (non-interactive via {})", s),
            None => String::new(),
        }
    );

    let all_ok = checks.iter().all(|c| c.ok);
    if all_ok {
        crate::say!("{}", "\nEnvironment looks healthy.".green());
    } else {
        crate::say!("{}", "\nSome checks need attention (see above).".yellow());
    }

    report::result(Toon::obj(vec![
        ("healthy", Toon::Bool(all_ok)),
        (
            "defaults",
            Toon::obj(vec![
                ("non_interactive", Toon::Bool(opts.non_interactive)),
                ("toon", Toon::Bool(opts.toon)),
                ("no_sign", Toon::Bool(opts.no_sign)),
                (
                    "non_interactive_source",
                    match ni_source {
                        Some(s) => Toon::str(s),
                        None => Toon::Null,
                    },
                ),
            ]),
        ),
        (
            "checks",
            Toon::Arr(
                checks
                    .into_iter()
                    .map(|c| {
                        Toon::obj(vec![
                            ("name", Toon::str(c.name)),
                            ("ok", Toon::Bool(c.ok)),
                            ("detail", Toon::str(c.detail)),
                        ])
                    })
                    .collect(),
            ),
        ),
    ]));

    Ok(())
}

/// One-shot situational awareness for agents: collapses status, sync-state,
/// stale-branch, trunk-CI, and radar-overlap lookups into a single TOON result.
pub fn handle_context(opts: RunOpts, config: &config::Config) -> Result<()> {
    crate::say!("{}", "--- Context ---".blue());

    let branch = git::get_current_branch(opts).unwrap_or_default();
    let unborn = git::is_unborn_head(opts);
    let upstream = git::has_upstream(opts);
    let status_output = git::get_scoped_status(config, opts).unwrap_or_default();
    let clean = status_output.is_empty();
    let (ahead, behind) = git::ahead_behind(opts).unwrap_or((0, 0));

    let stale = if unborn {
        Vec::new()
    } else {
        git::get_stale_branches(opts, &branch, config.stale_branch_threshold_days)
            .unwrap_or_default()
    };

    // Trunk CI (only meaningful when enabled and a remote exists).
    let trunk = radar::get_trunk_status(config, opts);
    let ci = match trunk.ci {
        git::CiStatus::Green => "green",
        git::CiStatus::Failed => "failed",
        git::CiStatus::Pending => "pending",
        git::CiStatus::Unknown(_) => "unknown",
    };

    // Radar overlaps (best-effort; needs a remote and a dirty tree).
    let overlaps = if config.radar.enabled && git::has_origin_remote(opts) {
        radar::scan(config, opts).ok()
    } else {
        None
    };

    // Human summary
    crate::say!(
        "Branch {} ({}), {}{} | CI: {}",
        branch.bold(),
        if clean { "clean".green() } else { "dirty".yellow() },
        format!("ahead {} ", ahead),
        format!("behind {}", behind),
        ci
    );
    if !stale.is_empty() {
        crate::say!("{}", format!("{} stale branch(es)", stale.len()).yellow());
    }

    // Structured result
    let mut fields = vec![
        ("branch".to_string(), Toon::str(branch)),
        ("clean".to_string(), Toon::Bool(clean)),
        ("unborn".to_string(), Toon::Bool(unborn)),
        ("upstream".to_string(), Toon::Bool(upstream)),
        ("ahead".to_string(), Toon::Int(ahead as i64)),
        ("behind".to_string(), Toon::Int(behind as i64)),
        ("trunk_ci".to_string(), Toon::str(ci)),
        (
            "stale".to_string(),
            Toon::Arr(
                stale
                    .into_iter()
                    .map(|(b, days)| {
                        Toon::obj(vec![
                            ("branch", Toon::str(b)),
                            ("days", Toon::Int(days)),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("overlaps".to_string(), Toon::Arr(overlap_rows(overlaps))),
    ];
    if let Ok(recent) = git::log_graph(opts) {
        fields.push(("recent".to_string(), Toon::str(recent)));
    }
    report::result(Toon::Obj(fields));
    Ok(())
}

/// Flatten radar overlaps into tabular rows `{branch,author,file,kind}`.
fn overlap_rows(result: Option<radar::RadarResult>) -> Vec<Toon> {
    let Some(result) = result else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for o in &result.overlaps {
        for f in &o.overlapping_files {
            let kind = match f.overlap_kind {
                radar::OverlapKind::SameFile => "same-file",
                radar::OverlapKind::LineOverlap { .. } => "line-overlap",
            };
            rows.push(Toon::obj(vec![
                ("branch", Toon::str(o.branch_name.clone())),
                ("author", Toon::str(o.author.clone())),
                ("file", Toon::str(f.file_path.clone())),
                ("kind", Toon::str(kind)),
            ]));
        }
    }
    rows
}

/// The default `.dod.yml` checklist written by `init`.
fn default_dod_yaml() -> &'static str {
    "checklist:\n  - \"Code is clean, readable, and adheres to team coding standards.\"\n  - \"All relevant automated tests (unit, integration) pass successfully.\"\n  - \"New features or bug fixes are covered by appropriate new tests.\"\n  - \"Security implications of this change have been considered.\"\n  - \"Relevant documentation (code comments, READMEs, etc.) is updated.\"\n"
}

/// `--dry-run` for init: print the plan and make NO changes (no git init, no
/// file writes, no gh calls). The repo/state queries it uses run for real, so
/// the plan reflects the actual on-disk state.
fn report_init_plan(
    opts: RunOpts,
    is_repo: bool,
    remote: &Option<String>,
    create_remote: &Option<String>,
    private: bool,
    push: bool,
) -> Result<()> {
    crate::say!(
        "{}",
        "[DRY RUN] No changes will be made. Planned actions:".yellow()
    );

    let base = if is_repo {
        std::path::PathBuf::from(git::get_git_root(opts)?)
    } else {
        env::current_dir()?
    };

    let mut steps: Vec<String> = Vec::new();
    if !is_repo {
        steps.push("git init (trunk: 'main')".to_string());
    }
    if !base.join(".tbdflow.yml").exists() {
        steps.push("write .tbdflow.yml".to_string());
    }
    if !base.join(".dod.yml").exists() {
        steps.push("write .dod.yml".to_string());
    }
    if !is_repo || git::is_unborn_head(opts) {
        steps.push("create initial commit".to_string());
    }
    if let Some(slug) = create_remote {
        steps.push(format!(
            "gh repo create {} ({})",
            slug,
            if private { "private" } else { "public" }
        ));
        if push {
            steps.push("push initial commit to the new remote".to_string());
        }
    } else if let Some(url) = remote {
        steps.push(format!("git remote add origin {} + push", url));
    }

    if steps.is_empty() {
        crate::say!("  - (nothing to do — already initialised)");
    }
    for s in &steps {
        crate::say!("  - {}", s);
    }
    report::result(Toon::obj(vec![
        ("dry_run", Toon::Bool(true)),
        ("is_repo", Toon::Bool(is_repo)),
        (
            "plan",
            Toon::Arr(steps.into_iter().map(Toon::Str).collect()),
        ),
    ]));
    Ok(())
}

pub fn handle_init_command(
    opts: RunOpts,
    remote: Option<String>,
    create_remote: Option<String>,
    private: bool,
    push: bool,
) -> Result<()> {
    crate::say!("--- Initialising tbdflow configuration ---");

    // Real read-only check (runs even under --dry-run).
    let is_repo = git::is_git_repository(opts).is_ok();

    // --dry-run is side-effect-free: report the plan and stop. (Previously the
    // file writes here ran during dry-run and corrupted the subsequent real run.)
    if opts.dry_run {
        return report_init_plan(opts, is_repo, &remote, &create_remote, private, push);
    }

    // 1. Ensure a git repository (auto-init under --non-interactive).
    let mut repo_initialised = false;
    if !is_repo {
        let current_dir = env::current_dir()?.to_string_lossy().to_string();
        let do_init = opts.non_interactive
            || Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt(format!(
                    "Currently not in a git repository ({}). Would you like to initialise one?",
                    current_dir
                ))
                .interact()?;
        if !do_init {
            crate::say!("Aborted. Please run `tbdflow init` from within a git repository.");
            return Ok(());
        }
        git::init_git_repository(opts)?;
        // Name the trunk 'main' from the start (safe on an unborn branch).
        let _ = git::set_head_branch("main", opts);
        repo_initialised = true;
        crate::say!("{}", "New git repository initialised.".green());
    }

    // 2. Write config files (only the ones that are absent).
    let git_root = git::get_git_root(opts)?;
    let current_dir = env::current_dir()?;
    let mut files_created = false;

    if current_dir.as_path() != std::path::Path::new(&git_root) {
        // In a subdirectory: create a project-specific config.
        let project_config_path = current_dir.join(".tbdflow.yml");
        if !project_config_path.exists() {
            let project_config = config::Config {
                project_root: Some(".".to_string()),
                ..Default::default()
            };
            fs::write(&project_config_path, yaml_serde::to_string(&project_config)?)?;
            files_created = true;
            crate::say!(
                "{}",
                "Created project-specific .tbdflow.yml in current directory.".green()
            );
        } else {
            crate::say!(
                "{}",
                ".tbdflow.yml already exists in this directory. Skipping.".yellow()
            );
        }
    } else {
        let tbdflow_path = std::path::Path::new(&git_root).join(".tbdflow.yml");
        if !tbdflow_path.exists() {
            fs::write(&tbdflow_path, yaml_serde::to_string(&config::Config::default())?)?;
            files_created = true;
            crate::say!(
                "{}",
                "Created default .tbdflow.yml configuration file.".green()
            );
        } else {
            crate::say!("{}", ".tbdflow.yml already exists. Skipping.".yellow());
        }
    }

    let dod_path = std::path::Path::new(&git_root).join(".dod.yml");
    if !dod_path.exists() {
        fs::write(&dod_path, default_dod_yaml())?;
        files_created = true;
        crate::say!("{}", "Created default .dod.yml checklist file.".green());
    } else {
        crate::say!("{}", ".dod.yml already exists. Skipping.".yellow());
    }

    // 3. Initial commit — make one whenever the repo is unborn (this self-heals
    //    a repo that has config files but no commits, e.g. after an aborted
    //    setup), or when we just created config files in an existing repo.
    let unborn = git::is_unborn_head(opts);
    let mut made_commit = false;
    if files_created || unborn {
        crate::say!("\n{}", "Creating initial commit...".blue());
        git::add_all(opts)?;
        if git::has_staged_changes(opts)? {
            git::commit("chore: initialise tbdflow configuration", opts)?;
            made_commit = true;
            crate::say!("{}", "Initial commit created.".green());
        } else if unborn {
            report::warn(
                "repository is unborn with nothing to commit — add files, then run 'tbdflow commit'",
            );
        }
    }

    // 4. Remote linking.
    let mut remote_linked = false;
    let mut remote_target: Option<String> = None;

    if let Some(slug) = create_remote {
        if !gh::available() {
            return Err(anyhow::anyhow!(
                "--create-remote needs the GitHub CLI (gh); install it from https://cli.github.com/"
            ));
        }
        if git::is_unborn_head(opts) {
            return Err(anyhow::anyhow!(
                "cannot create a remote for an unborn repository; commit something first"
            ));
        }
        crate::say!(
            "{}",
            format!("Creating GitHub repository '{}'...", slug).blue()
        );
        let out = gh::create_repo(&slug, private, push, opts)?;
        remote_linked = true;
        remote_target = Some(slug.clone());
        crate::say!("{}", format!("Created and linked {}", out).green());
    } else if let Some(remote_url) = remote {
        link_remote(&remote_url, opts)?;
        remote_linked = true;
        remote_target = Some(remote_url);
    } else if made_commit && !opts.non_interactive {
        // Interactive fallback: offer to link a remote.
        if Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(
                "\nDo you want to link a remote repository and push the initial commit now?",
            )
            .interact()?
        {
            let remote_url: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Please enter the remote repository URL (e.g. from GitHub)")
                .interact_text()?;
            if !remote_url.is_empty() {
                link_remote(&remote_url, opts)?;
                remote_linked = true;
                remote_target = Some(remote_url);
            } else {
                crate::say!("{}", "No URL provided. Skipping remote setup.".yellow());
            }
        }
    }

    let mut result = vec![
        ("initialised".to_string(), Toon::Bool(repo_initialised)),
        ("config_created".to_string(), Toon::Bool(files_created)),
        ("committed".to_string(), Toon::Bool(made_commit)),
        ("remote_linked".to_string(), Toon::Bool(remote_linked)),
    ];
    if let Some(target) = remote_target {
        result.push(("remote".to_string(), Toon::str(target)));
    }
    report::result(Toon::Obj(result));
    Ok(())
}

/// Link an existing remote URL as `origin`, reconcile with any remote trunk, and
/// push the initial commit with upstream tracking.
fn link_remote(remote_url: &str, opts: RunOpts) -> Result<()> {
    git::add_remote("origin", remote_url, opts)?;
    git::fetch_origin(opts)?;

    if git::remote_branch_exists("main", opts).is_ok() {
        crate::say!(
            "{}",
            "Remote 'main' branch found. Reconciling histories...".yellow()
        );
        git::rebase_onto_main("main", opts)?;
    }

    git::push_set_upstream("main", opts)?;
    crate::say!(
        "{}",
        "Successfully linked remote and pushed initial commit.".green()
    );
    Ok(())
}

pub fn handle_info(opts: RunOpts, edit: bool) -> Result<()> {
    let git_root = git::get_git_root(RunOpts::new(false, false))?;
    let root_config_path = PathBuf::from(&git_root).join(".tbdflow.yml");

    if edit {
        let editor = env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
        std::process::Command::new(&editor)
            .arg(&root_config_path)
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to open editor: {}", e))?;
        return Ok(());
    }

    crate::say!("{}", "--- tbdflow Configuration ---".blue());

    let root_config: config::Config = if root_config_path.exists() {
        let yaml_str = fs::read_to_string(&root_config_path)?;
        yaml_serde::from_str(&yaml_str)?
    } else {
        config::Config::default()
    };

    let final_config = config::load_tbdflow_config()?;

    print_mode_and_settings(&root_config, &root_config_path, &final_config)?;
    print_review_config(&final_config.review);
    print_radar_config(&final_config.radar);
    print_ci_config(&final_config.ci_check);
    print_git_info(opts)?;

    // Structured result for --toon consumers.
    report::result(Toon::obj(vec![
        (
            "main_branch",
            Toon::str(final_config.main_branch_name.clone()),
        ),
        (
            "issue_strategy",
            Toon::str(format!("{:?}", final_config.issue_handling.strategy)),
        ),
        ("lint", Toon::Bool(final_config.lint.is_some())),
        ("review", Toon::Bool(final_config.review.enabled)),
        ("radar", Toon::Bool(final_config.radar.enabled)),
        ("ci_check", Toon::Bool(final_config.ci_check.enabled)),
        (
            "monorepo",
            Toon::Bool(final_config.monorepo.enabled),
        ),
        (
            "remote",
            match git::get_remote_url(opts) {
                Ok(url) => Toon::str(url),
                Err(_) => Toon::Null,
            },
        ),
        (
            "current_branch",
            Toon::str(git::get_current_branch(opts).unwrap_or_default()),
        ),
    ]));

    Ok(())
}

fn print_mode_and_settings(
    root_config: &config::Config,
    root_config_path: &PathBuf,
    final_config: &config::Config,
) -> Result<()> {
    if let Some(project_root) = config::find_project_root()? {
        let project_config_path = project_root.join(".tbdflow.yml");
        if project_config_path.exists() {
            crate::say!("Mode: {} (Project)", "Monorepo".to_string().bold());
            crate::say!("Project Root: {}", project_root.to_string_lossy());
            crate::say!(
                "Loaded project-specific config from: {}",
                project_config_path.to_string_lossy()
            );

            let project_yaml_str = fs::read_to_string(&project_config_path)?;
            let project_config: config::Config = yaml_serde::from_str(&project_yaml_str)?;

            crate::say!("\n{}", "--- Settings ---".bold());

            let main_branch_source =
                if project_config.main_branch_name != root_config.main_branch_name {
                    "(overridden by project)".yellow()
                } else {
                    "(inherited from root)".dimmed()
                };
            crate::say!(
                "Main Branch: {} {}",
                project_config.main_branch_name, main_branch_source
            );

            let issue_strategy_source =
                if project_config.issue_handling.strategy != root_config.issue_handling.strategy {
                    "(overridden by project)".yellow()
                } else {
                    "(inherited from root)".dimmed()
                };
            crate::say!(
                "Issue Handling Strategy: {:?} {}",
                format!("{:?}", project_config.issue_handling.strategy).cyan(),
                issue_strategy_source
            );
        }
    } else {
        if root_config.monorepo.enabled && !root_config.monorepo.project_dirs.is_empty() {
            crate::say!("Mode: {} (Root)", "Monorepo".to_string().bold());
            crate::say!(
                "Loaded root config from: {}",
                root_config_path.to_string_lossy()
            );
            crate::say!("Project Directories:");
            for dir in &root_config.monorepo.project_dirs {
                crate::say!("- {}", dir.cyan());
            }
        } else {
            crate::say!("Mode: {}", "Standalone".bold());
            if root_config_path.exists() {
                crate::say!("Loaded config from: {}", root_config_path.to_string_lossy());
            }
        }

        crate::say!("\n{}", "--- Settings ---".bold());
        crate::say!(
            "Main Branch: {}",
            root_config.main_branch_name.to_string().cyan()
        );
        crate::say!(
            "Issue Handling Strategy: {}",
            format!("{:?}", root_config.issue_handling.strategy).cyan(),
        );
    }

    crate::say!(
        "Stale Branch Threshold: {} days",
        format!("{}", final_config.stale_branch_threshold_days).cyan()
    );

    let lint_status = if final_config.lint.is_some() {
        "Enabled".green()
    } else {
        "Disabled".red()
    };
    crate::say!("Commit Linting: {}", lint_status);

    Ok(())
}

fn print_review_config(review: &config::ReviewConfig) {
    crate::say!("\n{}", "--- Review ---".bold());
    if review.enabled {
        crate::say!("Review: {}", "Enabled".green());
        crate::say!("Strategy: {}", format!("{:?}", review.strategy).cyan());
        if !review.default_reviewers.is_empty() {
            crate::say!(
                "Default Reviewers: {}",
                review.default_reviewers.join(", ").cyan()
            );
        }
        if let Some(ref workflow) = review.workflow {
            crate::say!("Workflow: {}", workflow.cyan());
        }
        if !review.rules.is_empty() {
            crate::say!(
                "Targeted Rules: {}",
                format!("{}", review.rules.len()).cyan()
            );
        }
        crate::say!(
            "Concern Blocks Status: {}",
            if review.concern_blocks_status {
                "Yes".yellow()
            } else {
                "No".dimmed()
            }
        );
    } else {
        crate::say!("Review: {}", "Disabled".red());
    }
}

fn print_radar_config(radar: &config::RadarConfig) {
    crate::say!("\n{}", "--- Radar ---".bold());
    if radar.enabled {
        crate::say!("Radar: {}", "Enabled".green());
        crate::say!("Detection Level: {}", format!("{:?}", radar.level).cyan());
        crate::say!(
            "On Sync: {}",
            if radar.on_sync {
                "Yes".green()
            } else {
                "No".dimmed()
            }
        );
        crate::say!("On Commit: {}", format!("{:?}", radar.on_commit).cyan());
        if !radar.ignore_patterns.is_empty() {
            crate::say!(
                "Ignore Patterns: {}",
                radar.ignore_patterns.join(", ").dimmed()
            );
        }
    } else {
        crate::say!("Radar: {}", "Disabled".red());
    }
}

fn print_ci_config(ci_check: &config::CiCheckConfig) {
    crate::say!("\n{}", "--- CI Check ---".bold());
    if ci_check.enabled {
        crate::say!("CI Check on Sync: {}", "Enabled".green());
    } else {
        crate::say!("CI Check on Sync: {}", "Disabled".red());
    }
}

fn print_git_info(opts: RunOpts) -> Result<()> {
    crate::say!("\n{}", "--- Git Info ---".bold());
    if let Ok(remote_url) = git::get_remote_url(opts) {
        crate::say!("Remote 'origin' URL: {}", remote_url.to_string().cyan());
    } else {
        crate::say!("Remote 'origin' URL: Not found.");
    }

    let current_branch = git::get_current_branch(opts)?;
    crate::say!("Current branch: {}", current_branch.to_string().cyan());

    if let Ok(latest_tag) = git::get_latest_tag(opts) {
        crate::say!("Latest tag: {}", latest_tag.to_string().cyan());
    } else {
        crate::say!("Latest tag: Not found.");
    }

    Ok(())
}

pub fn handle_sync(opts: RunOpts, config: &config::Config) -> Result<()> {
    crate::say!(
        "{}",
        "--- Syncing with remote and showing status ---"
            .to_string()
            .blue()
    );
    let current_branch = git::get_current_branch(opts)?;

    // Anti-collision pre-flight: abort if a git operation is already in progress
    if let Some(msg) = git::check_git_operation_in_progress(opts)? {
        crate::say!(
            "{}",
            format!("Error: {} Please resolve it before using tbdflow.", msg).red()
        );
        return Err(anyhow::anyhow!("{}", msg));
    }

    if let Ok(Some(hash)) = git::stash_create(opts) {
        let git_root = std::path::PathBuf::from(git::get_git_root(opts)?);
        intent::record_safety_snapshot(
            &git_root,
            &hash,
            &current_branch,
            "Pre-sync safety snapshot",
        )?;
        if opts.verbose {
            crate::say!(
                "{}",
                format!(
                    "Pre-sync snapshot captured: {}",
                    &hash[..std::cmp::min(10, hash.len())]
                )
                .dimmed()
            );
        }
    }

    // Check trunk CI status before pulling to avoid importing a broken build
    if config.ci_check.enabled {
        let ci_status = git::check_ci_status(&config.main_branch_name, opts);
        match ci_status {
            git::CiStatus::Green => {
                crate::say!("{}", "Pre-flight CI check: trunk is green.".green());
            }
            git::CiStatus::Failed => {
                crate::say!(
                    "\n{}",
                    "The trunk is currently failing CI. Pulling now might break your local build."
                        .bold()
                        .yellow()
                );
                if opts.non_interactive {
                    // Safe default for agents: do not import a broken trunk.
                    return Err(anyhow::anyhow!(
                        "trunk CI is failing; sync aborted. Rerun without --non-interactive to override."
                    ));
                }
                let should_continue = Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt("Continue with sync?")
                    .default(false)
                    .interact()?;
                if !should_continue {
                    crate::say!("{}", "Sync aborted.".yellow());
                    return Ok(());
                }
            }
            git::CiStatus::Pending => {
                crate::say!("\n{}", "⏳ Trunk CI is still running.".bold().yellow());
                if opts.non_interactive {
                    // Non-blocking for agents: warn and proceed.
                    report::warn("trunk CI is still running; pulling anyway (--non-interactive)");
                } else {
                    let should_continue = Confirm::with_theme(&ColorfulTheme::default())
                        .with_prompt("Pull anyway?")
                        .default(false)
                        .interact()?;
                    if !should_continue {
                        crate::say!("{}", "Sync aborted.".yellow());
                        return Ok(());
                    }
                }
            }
            git::CiStatus::Unknown(reason) => {
                if opts.verbose {
                    crate::say!(
                        "{} {}",
                        "Pre-flight CI check skipped:".dimmed(),
                        reason.dimmed()
                    );
                }
                // Proceed silently — no CI info available is not a blocker
            }
        }
    }

    if current_branch == config.main_branch_name {
        crate::say!("On main branch, pulling latest changes...");
        git::pull_latest_with_rebase(opts)?;
    } else {
        crate::say!(
            "On feature branch '{}', rebasing onto latest '{}'...",
            current_branch, config.main_branch_name
        );
        git::fetch_origin(opts)?;
        git::rebase_onto_main(&config.main_branch_name, opts)?;
    }

    crate::say!("\n{}", "Current status:".bold());

    let status_output = git::get_scoped_status(config, opts)?;

    if status_output.is_empty() {
        crate::say!("{}", "Working directory is clean.".green());
    } else {
        crate::say!("{}", status_output.yellow());
    }

    let log_output = git::log_graph(opts)?;
    crate::say!("\n{}", "Recent activity:".bold());
    crate::say!("{}", log_output.cyan());

    // Radar: quick overlap scan
    let overlap = radar::quick_scan_for_sync(config, opts).ok().flatten();
    if let Some(ref radar_summary) = overlap {
        crate::say!("\n{}", radar_summary.yellow());
    }

    check_and_warn_for_stale_branches(opts, &current_branch, config)?;

    report::result(Toon::obj(vec![
        ("branch", Toon::str(current_branch)),
        ("clean", Toon::Bool(status_output.is_empty())),
        ("overlap", Toon::Bool(overlap.is_some())),
    ]));
    Ok(())
}

pub fn handle_check_branches(opts: RunOpts, config: &config::Config) -> Result<()> {
    crate::say!(
        "{}",
        "--- Checking current branch and stale branches ---"
            .to_string()
            .blue()
    );

    let current_branch = git::get_current_branch(opts)?;
    if current_branch != config.main_branch_name {
        return Err(git::GitError::NotOnMainBranch(current_branch).into());
    }
    check_and_warn_for_stale_branches(opts, &current_branch, config)?;
    Ok(())
}

pub fn check_and_warn_for_stale_branches(
    opts: RunOpts,
    current_branch: &str,
    config: &config::Config,
) -> Result<()> {
    let stale_branches =
        git::get_stale_branches(opts, current_branch, config.stale_branch_threshold_days)?;
    if !stale_branches.is_empty() {
        crate::say!(
            "\n{}",
            "Warning: The following branches may be stale:"
                .bold()
                .yellow()
        );
        for (branch, days) in stale_branches {
            crate::say!(
                "{}",
                format!("  - {} (last commit {} days ago)", branch, days).yellow()
            );
        }
    }
    Ok(())
}

pub fn get_branch_prefix_or_error<'a>(
    branch_types: &'a std::collections::HashMap<String, String>,
    r#type: &str,
) -> Result<&'a String> {
    branch_types.get(r#type).ok_or_else(|| {
        let allowed_types = branch_types
            .keys()
            .map(|s| s.as_str())
            .collect::<Vec<&str>>()
            .join(", ");
        anyhow::anyhow!(
            "Invalid branch type '{}'. Allowed types are: {}",
            r#type,
            allowed_types
        )
    })
}

pub fn handle_undo(sha: &str, no_push: bool, opts: RunOpts, config: &config::Config) -> Result<()> {
    crate::say!(
        "{}",
        "--- Undo: The Panic Button ---".to_string().bold().red()
    );

    // Anti-collision pre-flight
    if let Some(msg) = git::check_git_operation_in_progress(opts)? {
        crate::say!(
            "{}",
            format!("Error: {} Please resolve it before using tbdflow.", msg).red()
        );
        return Err(anyhow::anyhow!("{}", msg));
    }

    // WIP Guard: snapshot before the destructive checkout + fast-forward
    if let Ok(Some(hash)) = git::stash_create(opts) {
        let git_root = std::path::PathBuf::from(git::get_git_root(opts)?);
        let current_branch = git::get_current_branch(opts)?;
        intent::record_safety_snapshot(
            &git_root,
            &hash,
            &current_branch,
            "Pre-undo safety snapshot",
        )?;
        if opts.verbose {
            crate::say!(
                "{}",
                format!(
                    "Pre-undo snapshot captured: {}",
                    &hash[..std::cmp::min(10, hash.len())]
                )
                .dimmed()
            );
        }
    }

    let main_branch = &config.main_branch_name;

    if !git::commit_exists(sha, opts)? {
        crate::say!(
            "{}",
            format!("Error: Commit '{}' does not exist in this repository.", sha).red()
        );
        return Err(anyhow::anyhow!("Commit not found: {}", sha));
    }

    let subject = git::get_commit_subject(sha, opts)?;
    crate::say!(
        "{}",
        format!("Commit to revert: {} ({})", sha, subject).yellow()
    );

    git::is_working_directory_clean(opts)?;

    // Sync with remote (fast-forward only to preserve commit SHAs)
    crate::say!("Syncing with remote before reverting...");
    git::checkout_main(opts, main_branch)?;
    git::pull_fast_forward_only(opts)?;

    if !git::is_ancestor_of(sha, main_branch, opts)? {
        crate::say!(
            "{}",
            format!(
                "Error: Commit '{}' is not on the '{}' branch. Undo only works on trunk commits.",
                sha, main_branch
            )
            .red()
        );
        return Err(anyhow::anyhow!(
            "Commit '{}' is not on '{}'.",
            sha,
            main_branch
        ));
    }

    crate::say!("{}", format!("Reverting commit {}...", sha).blue());
    git::revert_commit(sha, opts)?;

    if no_push {
        crate::say!(
            "{}",
            "Revert commit created locally (--no-push). Remember to push when ready.".yellow()
        );
    } else {
        crate::say!("Pushing revert to remote...");
        git::push(opts)?;
        crate::say!(
            "\n{}",
            format!(
                "Success! Commit '{}' has been reverted on '{}'.",
                sha, main_branch
            )
            .green()
        );
    }

    let log_output = git::log_graph(opts)?;
    crate::say!("\n{}", "Recent activity:".bold());
    crate::say!("{}", log_output.cyan());

    crate::say!(
        "\n{}",
        "Hint: The reverted changes are still in your git history. You can re-apply them later."
            .dimmed()
    );

    report::result(Toon::obj(vec![
        ("reverted", Toon::str(sha.to_string())),
        ("pushed", Toon::Bool(!no_push)),
    ]));

    Ok(())
}

/// Generate a flattened man page for tbdflow to stdout, users can pipe this to a file.
pub fn render_manpage_section(cmd: &Commands, buffer: &mut Vec<u8>) -> Result<(), anyhow::Error> {
    let man = clap_mangen::Man::new(cmd.clone());
    // Render the command's sections into the buffer
    man.render_name_section(buffer)?;
    man.render_synopsis_section(buffer)?;
    man.render_description_section(buffer)?;
    man.render_options_section(buffer)?;

    // Only add a SUBCOMMANDS header if there are subcommands
    if cmd.has_subcommands() {
        use std::io::Write;
        writeln!(buffer, "\nSUBCOMMANDS\n")?;
        let mut cmd_mut = cmd.clone();
        for sub in cmd_mut.get_subcommands_mut() {
            render_manpage_section(sub, buffer)?;
        }
    }

    Ok(())
}
