use std::fs::write;
use std::process::Command;
use tempfile::{tempdir, TempDir};

/// An `assert_cmd` invocation of the tbdflow binary with agent-mode env vars
/// stripped, so tests are hermetic regardless of whether they run inside Claude
/// Code / CI (which would otherwise auto-enable non-interactive/TOON).
#[allow(dead_code)]
pub fn tbdflow_cmd() -> assert_cmd::Command {
    let mut cmd = assert_cmd::Command::cargo_bin("tbdflow").unwrap();
    cmd.env_remove("CLAUDECODE")
        .env_remove("CI")
        .env_remove("TBDFLOW_TOON")
        .env_remove("TBDFLOW_NON_INTERACTIVE")
        .env_remove("TBDFLOW_NO_SIGN");
    cmd
}

/// Sets up a temporary Git repository for testing purposes.
pub fn setup_temp_git_repo() -> (TempDir, TempDir, std::path::PathBuf) {
    let dir = tempdir().expect("create temp dir");
    let repo_path = dir.path().to_path_buf();

    // Create a bare repo to act as 'origin'
    let bare_dir = tempdir().expect("create bare repo");
    let bare_repo_path = bare_dir.path().to_path_buf();
    Command::new("git")
        .arg("init")
        .arg("--bare")
        .current_dir(&bare_repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .arg("init")
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "init.defaultBranch")
        .env("GIT_CONFIG_VALUE_0", "main")
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(&["config", "user.email", "test@example.com"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(&["config", "user.name", "Test"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(&["config", "push.autoSetupRemote", "true"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    // Keep tests deterministic and offline: never GPG-sign in the test repo,
    // regardless of the developer's global git signing config.
    Command::new("git")
        .args(&["config", "commit.gpgsign", "false"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(&["config", "tag.gpgsign", "false"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    let file_path = repo_path.join("README.md");
    write(&file_path, "test").unwrap();
    Command::new("git")
        .args(&["add", "."])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(&["commit", "-m", "init"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    // Add local bare repo as remote
    Command::new("git")
        .args(&["remote", "add", "origin", bare_repo_path.to_str().unwrap()])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    // Push main to origin to set up tracking
    Command::new("git")
        .args(&["push", "-u", "origin", "main"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    let status = Command::new("git")
        .args(&["status", "--porcelain"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    assert!(
        status.stdout.is_empty(),
        "Repo not clean after setup: {:?}",
        String::from_utf8_lossy(&status.stdout)
    );

    (dir, bare_dir, repo_path)
}
