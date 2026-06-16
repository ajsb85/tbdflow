use clap::{CommandFactory, Parser};
use colored::Colorize;
use std::io;
use std::io::Write;
use tbdflow::cli::Commands;
use tbdflow::cli::TaskAction;
use tbdflow::commit::CommitParams;
use tbdflow::git::get_current_branch;
use tbdflow::git::RunOpts;
use tbdflow::toon::Toon;
use tbdflow::{
    branch, changelog, cli, commands, commit, config, git, intent, radar, recover, report, review,
    runtime, say, wizard,
};

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    // Resolve agent-mode defaults: explicit flag > TBDFLOW_* env > CLAUDECODE/CI
    // auto-detect > built-in. Lets Claude Code/CI run tbdflow without repeating
    // --non-interactive/--toon on every call (set them in .claude/settings.json).
    let toon = runtime::toon(cli.toon);
    let opts = RunOpts::with_flags(
        cli.verbose,
        cli.dry_run,
        toon,
        runtime::non_interactive(cli.non_interactive),
        runtime::no_sign(cli.no_sign),
    );
    report::init(toon, cli.command.name());

    let result = run(cli, opts);

    match &result {
        Ok(()) => report::flush(true, None, None),
        Err(e) => report::flush(false, Some(format!("{:#}", e)), error_code(e)),
    }
    result
}

/// Error returned when an interactive wizard would be needed but the user passed
/// `--non-interactive`. Mirrors the style of the `gh` CLI.
fn non_interactive_error(needs: &str) -> anyhow::Error {
    anyhow::anyhow!(
        "{} when running with --non-interactive (interactive wizard disabled)",
        needs
    )
}

/// Read a text argument from a file path, or from stdin when the path is `-`.
fn read_text_arg(path: &str) -> anyhow::Result<String> {
    use anyhow::Context;
    if path == "-" {
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
            .context("Failed to read from stdin")?;
        Ok(buf)
    } else {
        std::fs::read_to_string(path).with_context(|| format!("Failed to read file '{}'", path))
    }
}

/// Map an error to a stable, machine-readable code for the TOON `code` field so
/// agents can branch on the failure type without parsing prose. Centralised so
/// the codes stay consistent. Returns `None` for unclassified errors.
fn error_code(err: &anyhow::Error) -> Option<&'static str> {
    use tbdflow::git::GitError;
    if let Some(git_err) = err.downcast_ref::<GitError>() {
        return Some(match git_err {
            GitError::Git(_) => "git_failed",
            GitError::DirectoryNotClean(_) => "dirty_worktree",
            GitError::InvalidBranchType(_) => "invalid_branch_type",
            GitError::BranchNotFound(_) => "branch_not_found",
            GitError::TagAlreadyExists(_) => "tag_exists",
            GitError::CannotCompleteMainBranch => "cannot_complete_main",
            GitError::NotOnMainBranch(_) => "not_on_main",
            GitError::NotAGitRepository(_) => "not_a_repo",
        });
    }
    // Our own anyhow!-constructed errors: classify by a stable marker substring.
    let msg = err.to_string();
    let code = if msg.contains("not a git repository") {
        "not_a_repo"
    } else if msg.contains("--non-interactive") {
        "missing_args"
    } else if msg.contains("no commits yet") {
        "unborn_no_commits"
    } else if msg.contains("trunk CI is failing") {
        "ci_failing"
    } else {
        return None;
    };
    Some(code)
}

fn run(cli: cli::Cli, opts: RunOpts) -> anyhow::Result<()> {
    if !matches!(
        cli.command,
        Commands::Init { .. }
            | Commands::Update
            | Commands::Completion { .. }
            | Commands::GenerateManPage
            | Commands::Doctor
    ) && git::is_git_repository(opts).is_err()
    {
        say!(
            "{}",
            "Error: Not a git repository (or any of the parent directories).".red()
        );
        say!("Hint: Run 'tbdflow init' to initialise a new repository here.");
        return Err(anyhow::anyhow!(
            "not a git repository; run 'tbdflow init' first"
        ));
    }

    let config = config::load_tbdflow_config()?;

    match cli.command {
        Commands::Init {
            remote,
            create_remote,
            private,
            push,
        } => {
            commands::handle_init_command(opts, remote, create_remote, private, push)?;
        }
        Commands::Info { edit } => {
            commands::handle_info(opts, edit)?;
        }
        Commands::Doctor => {
            commands::handle_doctor(opts, &config)?;
        }
        Commands::Config { get_dod } => {
            if get_dod {
                if let Ok(dod_config) = config::load_dod_config() {
                    let mut items = Vec::new();
                    for item in &dod_config.checklist {
                        say!("{}", item);
                        items.push(Toon::str(item.clone()));
                    }
                    report::result(Toon::obj(vec![("dod", Toon::Arr(items))]));
                }
            }
        }
        Commands::HeadSha => {
            let sha = git::get_head_commit_hash(opts)?;
            let short = sha[..std::cmp::min(7, sha.len())].to_string();
            say!("{}", short);
            report::result(Toon::obj(vec![("sha", Toon::str(short))]));
        }
        Commands::Update => {
            commands::handle_update_command()?;
        }
        Commands::Commit {
            r#type,
            scope,
            message,
            message_file,
            body,
            body_file,
            breaking,
            breaking_description,
            tag,
            no_verify,
            issue,
            include_projects,
        } => {
            // Resolve subject/body from files or stdin when requested. Files
            // avoid shell-escaping pain for multi-line bodies. (clap already
            // guarantees --message/--message-file and --body/--body-file are
            // mutually exclusive.)
            let message = match message_file {
                Some(path) => Some(read_text_arg(&path)?.trim_end().to_string()),
                None => message,
            };
            let body = match body_file {
                Some(path) => Some(read_text_arg(&path)?.trim_end_matches('\n').to_string()),
                None => body,
            };

            let params = match (r#type, message) {
                (Some(t), Some(m)) => CommitParams {
                    r#type: t,
                    scope,
                    message: m,
                    body,
                    breaking,
                    breaking_description,
                    tag,
                    issue,
                    include_projects,
                    no_verify,
                },
                _ => {
                    if opts.non_interactive {
                        return Err(non_interactive_error("--type and --message are required"));
                    }
                    let w = wizard::run_commit_wizard(&config)?;
                    CommitParams {
                        r#type: w.r#type,
                        scope: w.scope,
                        message: w.message,
                        body: w.body,
                        breaking: w.breaking,
                        breaking_description: w.breaking_description,
                        tag: w.tag,
                        issue: w.issue,
                        include_projects,
                        no_verify,
                    }
                }
            };

            commit::handle_commit(opts, &config, params)?;
        }
        Commands::Branch {
            r#type,
            name,
            issue,
            from_commit,
        } => {
            if r#type.is_none() || name.is_none() {
                if opts.non_interactive {
                    return Err(non_interactive_error("--type and --name are required"));
                }
                let wizard_result = wizard::run_branch_wizard(&config)?;
                branch::handle_branch(
                    Some(wizard_result.branch_type),
                    &config,
                    Some(wizard_result.name),
                    wizard_result.issue,
                    wizard_result.from_commit,
                    opts,
                )?;
            } else {
                branch::handle_branch(r#type, &config, name, issue, from_commit, opts)?;
            }
        }
        Commands::Complete { r#type, name } => match (r#type, name) {
            (Some(t), Some(n)) => {
                branch::handle_complete(t, n, &config, opts)?;
            }
            _ => {
                if opts.non_interactive {
                    return Err(non_interactive_error("--type and --name are required"));
                }
                let wizard_result = wizard::run_complete_wizard(&config)?;
                branch::handle_complete(
                    wizard_result.branch_type,
                    wizard_result.name,
                    &config,
                    opts,
                )?;
            }
        },
        Commands::Sync => {
            commands::handle_sync(opts, &config)?;
        }
        Commands::Radar => {
            radar::handle_radar(opts, &config)?;
        }
        Commands::Status => {
            say!("--- Checking status ---");
            let status_output = git::get_scoped_status(&config, opts)?;

            if status_output.is_empty() {
                say!("{}", "Working directory is clean.".green());
            } else {
                say!("{}", status_output.yellow());
            }
            report::result(Toon::obj(vec![
                ("clean", Toon::Bool(status_output.is_empty())),
                ("status", Toon::str(status_output)),
            ]));
        }
        Commands::Context => {
            commands::handle_context(opts, &config)?;
        }
        Commands::CurrentBranch => {
            say!("{}", "--- Current branch ---".to_string().blue());
            let branch_name = get_current_branch(opts)?;
            say!("{}", format!("Current branch is: {}", branch_name).green());
            report::result(Toon::obj(vec![("branch", Toon::str(branch_name))]));
        }
        Commands::CheckBranches => {
            commands::handle_check_branches(opts, &config)?;
        }
        Commands::GenerateManPage => {
            // Raw generator output; bypasses TOON/human routing intentionally.
            let mut cmd = cli::Cli::command();
            let mut buffer: Vec<u8> = Default::default();
            let man = clap_mangen::Man::new(cmd.clone());
            man.render(&mut buffer)?;
            writeln!(buffer, "\n--------------------------------------------------------------------------------\n")?;
            for sub in cmd.get_subcommands_mut() {
                commands::render_manpage_section(sub, &mut buffer)?;
            }
            io::stdout().write_all(&buffer)?;
        }
        Commands::Completion { shell } => {
            // Raw generator output; bypasses TOON/human routing intentionally.
            let mut cmd = cli::Cli::command();
            let bin_name = cmd.get_name().to_string();
            clap_complete::generate(shell, &mut cmd, bin_name, &mut io::stdout());
        }
        Commands::Changelog {
            from,
            to,
            unreleased,
        } => {
            let changelog = if from.is_none() && to.is_none() && !unreleased {
                if opts.non_interactive {
                    return Err(non_interactive_error(
                        "--unreleased or --from/--to are required",
                    ));
                }
                let wizard_result = wizard::run_changelog_wizard()?;
                changelog::handle_changelog(
                    opts,
                    &config,
                    wizard_result.from,
                    wizard_result.to,
                    wizard_result.unreleased,
                )?
            } else {
                changelog::handle_changelog(opts, &config, from, to, unreleased)?
            };

            if changelog.is_empty() {
                say!(
                    "{}",
                    "No conventional commits found in the specified range.".yellow()
                );
            } else {
                say!("{}", changelog);
            }
            report::result(Toon::obj(vec![("changelog", Toon::str(changelog))]));
        }
        Commands::Undo { sha, no_push } => {
            commands::handle_undo(&sha, no_push, opts, &config)?;
        }
        Commands::Note { message, show } => {
            let git_root = std::path::PathBuf::from(git::get_git_root(opts)?);
            let current_branch = get_current_branch(opts)?;
            if show {
                intent::show_intent_log(&git_root, Some(&current_branch))?;
            } else if let Some(msg) = message {
                // Capture WIP state alongside the note
                let snapshot_hash = git::stash_create(opts)?;
                intent::add_note_with_snapshot(
                    &git_root,
                    &msg,
                    &current_branch,
                    snapshot_hash.clone(),
                )?;
                say!("{}", format!("Note recorded: \"{}\"", msg).green());
                let mut fields = vec![("note".to_string(), Toon::str(msg))];
                if let Some(hash) = snapshot_hash {
                    let short = hash[..std::cmp::min(10, hash.len())].to_string();
                    say!("{}", format!("WIP snapshot: {}", short).dimmed());
                    fields.push(("snapshot".to_string(), Toon::str(short)));
                }
                report::result(Toon::Obj(fields));
            } else {
                intent::show_intent_log(&git_root, Some(&current_branch))?;
            }
        }
        Commands::Task(action) => {
            let git_root = std::path::PathBuf::from(git::get_git_root(opts)?);
            let current_branch = get_current_branch(opts)?;
            match action {
                TaskAction::Start { description } => {
                    intent::start_task(&git_root, &description, &current_branch)?;
                    say!("{}", format!("Task started: \"{}\"", description).green());
                    say!(
                        "{}",
                        "Use 'tbdflow +' or 'tbdflow note' to log your thoughts as you work."
                            .dimmed()
                    );
                    report::result(Toon::obj(vec![
                        ("task", Toon::str(description)),
                        ("started", Toon::Bool(true)),
                    ]));
                }
                TaskAction::Show => {
                    intent::show_intent_log(&git_root, Some(&current_branch))?;
                }
                TaskAction::Clear => {
                    intent::cleanup_intent_log(&git_root)?;
                    say!("{}", "Intent log cleared.".green());
                    report::result(Toon::obj(vec![("cleared", Toon::Bool(true))]));
                }
            }
        }
        Commands::Recover { selector, list } => {
            let git_root = std::path::PathBuf::from(git::get_git_root(opts)?);
            let current_branch = get_current_branch(opts)?;
            if list || selector.is_none() {
                recover::handle_recover_list(&git_root, &current_branch)?;
            } else if let Some(sel) = selector {
                recover::handle_recover_apply(&git_root, &sel, opts)?;
            }
        }
        Commands::Review {
            sha,
            trigger,
            digest,
            approve,
            concern,
            dismiss,
            message,
            since,
            reviewers,
        } => {
            if let Some(commit_hash) = approve {
                review::handle_review_approve(&config, &commit_hash, opts)?;
            } else if let Some(commit_hash) = concern {
                let msg = message.ok_or_else(|| {
                    anyhow::anyhow!("--message is required when raising a concern")
                })?;
                review::handle_review_concern(&config, &commit_hash, &msg, opts)?;
            } else if let Some(commit_hash) = dismiss {
                let msg = message.ok_or_else(|| {
                    anyhow::anyhow!("--message is required when dismissing a review")
                })?;
                review::handle_review_dismiss(&config, &commit_hash, &msg, opts)?;
            } else if digest {
                review::handle_review_digest(&config, &since, opts)?;
            } else if let Some(commit_sha) = sha {
                review::handle_review_trigger(&config, reviewers, Some(commit_sha.as_str()), opts)?;
            } else if trigger {
                review::handle_review_trigger(&config, reviewers, None, opts)?;
            } else {
                review::handle_review_digest(&config, &since, opts)?;
            }
        }
    }

    Ok(())
}
