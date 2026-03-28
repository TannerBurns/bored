//! Pure git operation helpers (no Database or Tauri dependencies).
//! Used by `next_steps` and other modules for push, PR, diff, and commit operations.

use std::process::Command;

/// Result of a git push operation
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushResult {
    pub success: bool,
    pub message: String,
    pub branch: String,
}

/// Result of creating a pull request
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestResult {
    pub success: bool,
    pub url: Option<String>,
    pub message: String,
}

pub fn get_default_branch(working_dir: &str) -> Result<String, String> {
    let output = Command::new("git")
        .args(["symbolic-ref", "refs/remotes/origin/HEAD"])
        .current_dir(working_dir)
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let branch = String::from_utf8_lossy(&output.stdout)
                .trim()
                .strip_prefix("refs/remotes/origin/")
                .unwrap_or("main")
                .to_string();
            return Ok(format!("origin/{}", branch));
        }
    }

    let check_main = Command::new("git")
        .args(["rev-parse", "--verify", "origin/main"])
        .current_dir(working_dir)
        .output();

    if let Ok(output) = check_main {
        if output.status.success() {
            return Ok("origin/main".to_string());
        }
    }

    Ok("origin/master".to_string())
}

/// Extract a conventional commit type from the branch name prefix.
/// Falls back to "chore" when the prefix isn't a recognized commitizen type.
pub(super) fn infer_commit_type_from_branch(branch: &str) -> &'static str {
    let prefix = branch.split('/').next().unwrap_or("");
    match prefix {
        "feat" => "feat",
        "fix" => "fix",
        "docs" => "docs",
        "style" => "style",
        "refactor" => "refactor",
        "perf" => "perf",
        "test" => "test",
        "build" => "build",
        "ci" => "ci",
        "chore" => "chore",
        "revert" => "revert",
        _ => "chore",
    }
}

/// Get the current branch name for a working directory.
/// Returns `None` if HEAD is detached or the git command fails.
pub fn get_current_branch(working_dir: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(working_dir)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if branch.is_empty() || branch == "HEAD" {
        return None;
    }
    Some(branch)
}

/// Returns `true` if the branch name is a protected default branch (main, master).
pub fn is_protected_branch(branch: &str) -> bool {
    matches!(branch, "main" | "master")
}

/// Verify the working directory is NOT on a protected branch.
/// Returns `Ok(())` if safe to commit, or `Err` with a descriptive message.
pub fn assert_not_on_protected_branch(working_dir: &str) -> Result<(), String> {
    if let Some(branch) = get_current_branch(working_dir) {
        if is_protected_branch(&branch) {
            return Err(format!(
                "REFUSED: attempted commit on protected branch '{}' in {}. \
                 Commits must target a feature branch, not the default branch.",
                branch, working_dir
            ));
        }
    }
    Ok(())
}

/// Check whether there are uncommitted changes (staged or unstaged) in the working directory.
pub fn has_uncommitted_changes(working_dir: &str) -> bool {
    Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(working_dir)
        .output()
        .map(|o| {
            o.status.success()
                && !String::from_utf8_lossy(&o.stdout).trim().is_empty()
        })
        .unwrap_or(false)
}

/// Stage all changes and commit them. Returns Ok(()) on success or an error message.
/// Refuses to commit if the working directory is on a protected branch (main/master).
pub fn commit_all_changes(working_dir: &str, message: &str) -> Result<(), String> {
    assert_not_on_protected_branch(working_dir)?;

    let add_output = Command::new("git")
        .args(["add", "-A"])
        .current_dir(working_dir)
        .output()
        .map_err(|e| format!("Failed to run git add: {}", e))?;

    if !add_output.status.success() {
        let stderr = String::from_utf8_lossy(&add_output.stderr);
        return Err(format!("git add -A failed: {}", stderr));
    }

    let commit_output = Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(working_dir)
        .output()
        .map_err(|e| format!("Failed to run git commit: {}", e))?;

    if !commit_output.status.success() {
        let stderr = String::from_utf8_lossy(&commit_output.stderr);
        return Err(format!("git commit failed: {}", stderr));
    }

    Ok(())
}

/// Check whether there are local commits not yet pushed to `origin/<branch>`.
/// Returns `true` when `origin/<branch>` doesn't exist (never pushed) or when
/// there are commits ahead of the remote tracking ref.
pub(super) fn check_has_unpushed(working_dir: &str, branch: &str) -> bool {
    let remote_ref = format!("origin/{}", branch);
    let output = Command::new("git")
        .args(["rev-list", "--count", &format!("{}..{}", remote_ref, branch)])
        .current_dir(working_dir)
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let count: usize = String::from_utf8_lossy(&o.stdout)
                .trim()
                .parse()
                .unwrap_or(0);
            count > 0
        }
        _ => true,
    }
}

/// Push a single working directory's branch to origin.
/// Refuses to operate if `branch` is a protected branch name or the working
/// directory is currently checked out on a protected branch.
pub(super) fn push_single_branch(working_dir: &str, branch: &str, ticket_title: &str) -> PushResult {
    if is_protected_branch(branch) {
        return PushResult {
            success: false,
            message: format!(
                "REFUSED: will not push protected branch '{}'. Commits must target a feature branch.",
                branch
            ),
            branch: branch.to_string(),
        };
    }
    if let Err(e) = assert_not_on_protected_branch(working_dir) {
        return PushResult {
            success: false,
            message: e,
            branch: branch.to_string(),
        };
    }

    if has_uncommitted_changes(working_dir) {
        let commit_type = infer_commit_type_from_branch(branch);
        let commit_msg = format!("{}: {}", commit_type, ticket_title);
        if let Err(e) = commit_all_changes(working_dir, &commit_msg) {
            return PushResult {
                success: false,
                message: format!("Failed to commit uncommitted changes: {}", e),
                branch: branch.to_string(),
            };
        }
    }

    let output = Command::new("git")
        .args(["push", "-u", "origin", branch])
        .current_dir(working_dir)
        .output();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            if output.status.success() {
                PushResult {
                    success: true,
                    message: if stderr.is_empty() { stdout } else { format!("{}{}", stdout, stderr) },
                    branch: branch.to_string(),
                }
            } else {
                PushResult {
                    success: false,
                    message: format!("{}{}", stdout, stderr),
                    branch: branch.to_string(),
                }
            }
        }
        Err(e) => PushResult {
            success: false,
            message: format!("Failed to run git push: {}", e),
            branch: branch.to_string(),
        },
    }
}

/// Create a PR for a single working directory.
/// Refuses to operate if `branch` is a protected branch name or the working
/// directory is currently checked out on a protected branch.
pub(super) fn create_pr_for_project(
    working_dir: &str,
    branch: &str,
    ticket_title: &str,
    pr_title: &str,
    pr_body: &str,
) -> PullRequestResult {
    if is_protected_branch(branch) {
        return PullRequestResult {
            success: false,
            url: None,
            message: format!(
                "REFUSED: will not create PR from protected branch '{}'. Commits must target a feature branch.",
                branch
            ),
        };
    }
    if let Err(e) = assert_not_on_protected_branch(working_dir) {
        return PullRequestResult {
            success: false,
            url: None,
            message: e,
        };
    }

    if has_uncommitted_changes(working_dir) {
        let commit_type = infer_commit_type_from_branch(branch);
        let commit_msg = format!("{}: {}", commit_type, ticket_title);
        if let Err(e) = commit_all_changes(working_dir, &commit_msg) {
            return PullRequestResult {
                success: false,
                url: None,
                message: format!("Failed to commit uncommitted changes: {}", e),
            };
        }
    }

    let push_output = Command::new("git")
        .args(["push", "-u", "origin", branch])
        .current_dir(working_dir)
        .output();

    match push_output {
        Ok(output) if !output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return PullRequestResult {
                success: false,
                url: None,
                message: format!("Failed to push branch to origin: {}{}", stdout, stderr),
            };
        }
        Err(e) => {
            return PullRequestResult {
                success: false,
                url: None,
                message: format!("Failed to run git push: {}", e),
            };
        }
        _ => {}
    }

    let output = Command::new("gh")
        .args(["pr", "create", "--title", pr_title, "--body", pr_body, "--head", branch])
        .current_dir(working_dir)
        .output();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if output.status.success() {
                let url = stdout.trim().to_string();
                PullRequestResult {
                    success: true,
                    url: if url.is_empty() { None } else { Some(url) },
                    message: "Pull request created successfully".to_string(),
                }
            } else {
                PullRequestResult {
                    success: false,
                    url: None,
                    message: format!("{}{}", stdout, stderr),
                }
            }
        }
        Err(e) => PullRequestResult {
            success: false,
            url: None,
            message: format!("Failed to run gh pr create: {}", e),
        },
    }
}

/// Get branch diff for a single working directory against its default branch.
pub(super) fn get_single_project_diff(working_dir: &str, branch: &str) -> Result<(String, usize), String> {
    let default_branch = get_default_branch(working_dir)?;

    let diff_output = Command::new("git")
        .args(["diff", &format!("{}...{}", default_branch, branch)])
        .current_dir(working_dir)
        .output()
        .map_err(|e| format!("Failed to run git diff: {}", e))?;

    if !diff_output.status.success() {
        let stderr = String::from_utf8_lossy(&diff_output.stderr);
        return Err(format!("git diff failed (exit {}): {}", diff_output.status, stderr.trim()));
    }

    let diff = String::from_utf8_lossy(&diff_output.stdout).to_string();

    let stat_output = Command::new("git")
        .args(["diff", "--stat", &format!("{}...{}", default_branch, branch)])
        .current_dir(working_dir)
        .output()
        .map_err(|e| format!("Failed to run git diff --stat: {}", e))?;

    if !stat_output.status.success() {
        let stderr = String::from_utf8_lossy(&stat_output.stderr);
        return Err(format!("git diff --stat failed (exit {}): {}", stat_output.status, stderr.trim()));
    }

    let stat = String::from_utf8_lossy(&stat_output.stdout).to_string();
    let files_changed = stat.lines().count().saturating_sub(1);

    Ok((diff, files_changed))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a temporary git repo with an initial commit (on the default branch).
    fn init_temp_repo() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().to_str().unwrap().to_string();

        Command::new("git")
            .args(["init"])
            .current_dir(&path)
            .output()
            .expect("git init");
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&path)
            .output()
            .expect("git config email");
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&path)
            .output()
            .expect("git config name");

        std::fs::write(dir.path().join("README.md"), "# init").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&path)
            .output()
            .expect("git add");
        Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(&path)
            .output()
            .expect("git commit");

        (dir, path)
    }

    /// Switch to a feature branch so tests can call commit_all_changes
    /// without hitting the protected-branch guard.
    fn checkout_feature_branch(path: &str) {
        Command::new("git")
            .args(["checkout", "-b", "feat/test-work"])
            .current_dir(path)
            .output()
            .expect("checkout feature branch");
    }

    /// Create a temp repo with a bare origin so `get_default_branch` resolves.
    fn init_temp_repo_with_remote() -> (tempfile::TempDir, String, tempfile::TempDir) {
        let (dir, path) = init_temp_repo();

        let bare_dir = tempfile::tempdir().expect("bare dir");
        let bare_path = bare_dir.path().to_str().unwrap();

        Command::new("git")
            .args(["clone", "--bare", &path, bare_path])
            .output()
            .expect("git clone --bare");
        Command::new("git")
            .args(["remote", "add", "origin", bare_path])
            .current_dir(&path)
            .output()
            .expect("git remote add");

        let branch_output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&path)
            .output()
            .expect("git branch");
        let branch = String::from_utf8_lossy(&branch_output.stdout).trim().to_string();

        Command::new("git")
            .args(["push", "-u", "origin", &branch])
            .current_dir(&path)
            .output()
            .expect("git push");

        (dir, path, bare_dir)
    }

    // --- has_uncommitted_changes ---

    #[test]
    fn uncommitted_changes_clean_repo_returns_false() {
        let (_dir, path) = init_temp_repo();
        assert!(!has_uncommitted_changes(&path));
    }

    #[test]
    fn uncommitted_changes_modified_file_returns_true() {
        let (dir, path) = init_temp_repo();
        std::fs::write(dir.path().join("README.md"), "# modified").unwrap();
        assert!(has_uncommitted_changes(&path));
    }

    #[test]
    fn uncommitted_changes_staged_file_returns_true() {
        let (dir, path) = init_temp_repo();
        std::fs::write(dir.path().join("README.md"), "# staged").unwrap();
        Command::new("git")
            .args(["add", "README.md"])
            .current_dir(&path)
            .output()
            .expect("git add");
        assert!(has_uncommitted_changes(&path));
    }

    #[test]
    fn uncommitted_changes_untracked_file_returns_true() {
        let (dir, path) = init_temp_repo();
        std::fs::write(dir.path().join("new_file.txt"), "hello").unwrap();
        assert!(has_uncommitted_changes(&path));
    }

    #[test]
    fn uncommitted_changes_invalid_dir_returns_false() {
        assert!(!has_uncommitted_changes("/nonexistent/path/that/does/not/exist"));
    }

    // --- commit_all_changes ---

    #[test]
    fn commit_all_changes_happy_path() {
        let (dir, path) = init_temp_repo();
        checkout_feature_branch(&path);
        std::fs::write(dir.path().join("README.md"), "# changed").unwrap();

        let result = commit_all_changes(&path, "test commit");
        assert!(result.is_ok());
        assert!(!has_uncommitted_changes(&path));
    }

    #[test]
    fn commit_all_changes_includes_untracked_files() {
        let (dir, path) = init_temp_repo();
        checkout_feature_branch(&path);
        std::fs::write(dir.path().join("brand_new.txt"), "new content").unwrap();

        let result = commit_all_changes(&path, "add new file");
        assert!(result.is_ok());
        assert!(!has_uncommitted_changes(&path));

        let log = Command::new("git")
            .args(["log", "--oneline", "--name-only", "-1"])
            .current_dir(&path)
            .output()
            .expect("git log");
        let output = String::from_utf8_lossy(&log.stdout);
        assert!(output.contains("brand_new.txt"));
    }

    #[test]
    fn commit_all_changes_uses_provided_message() {
        let (dir, path) = init_temp_repo();
        checkout_feature_branch(&path);
        std::fs::write(dir.path().join("README.md"), "# updated").unwrap();

        commit_all_changes(&path, "feat: my custom message").unwrap();

        let log = Command::new("git")
            .args(["log", "-1", "--format=%s"])
            .current_dir(&path)
            .output()
            .expect("git log");
        let msg = String::from_utf8_lossy(&log.stdout).trim().to_string();
        assert_eq!(msg, "feat: my custom message");
    }

    #[test]
    fn commit_all_changes_invalid_dir_returns_err() {
        let result = commit_all_changes("/nonexistent/path/that/does/not/exist", "msg");
        assert!(result.is_err());
    }

    #[test]
    fn commit_all_changes_refuses_on_protected_branch() {
        let (dir, path) = init_temp_repo();
        std::fs::write(dir.path().join("README.md"), "# changed").unwrap();

        let result = commit_all_changes(&path, "should be refused");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("REFUSED"), "error should mention REFUSED: {}", err);
        assert!(err.contains("protected branch"), "error should mention protected branch: {}", err);
        assert!(has_uncommitted_changes(&path), "changes should still be uncommitted");
    }

    // --- check_has_unpushed ---

    #[test]
    fn check_has_unpushed_no_remote_returns_true() {
        let (_dir, path) = init_temp_repo();
        assert!(check_has_unpushed(&path, "main"));
    }

    #[test]
    fn check_has_unpushed_invalid_dir_returns_true() {
        assert!(check_has_unpushed("/nonexistent/path/that/does/not/exist", "main"));
    }

    #[test]
    fn check_has_unpushed_with_local_remote_up_to_date() {
        let (_dir, path, _bare) = init_temp_repo_with_remote();

        let branch_output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&path)
            .output()
            .expect("git branch");
        let branch = String::from_utf8_lossy(&branch_output.stdout).trim().to_string();

        assert!(!check_has_unpushed(&path, &branch));
    }

    #[test]
    fn check_has_unpushed_with_local_commit_ahead() {
        let (dir, path, _bare) = init_temp_repo_with_remote();

        Command::new("git")
            .args(["checkout", "-b", "feat/unpushed-test"])
            .current_dir(&path)
            .output()
            .expect("checkout feature branch");

        std::fs::write(dir.path().join("new.txt"), "local only").unwrap();
        commit_all_changes(&path, "local commit").unwrap();

        assert!(check_has_unpushed(&path, "feat/unpushed-test"));
    }

    // --- infer_commit_type_from_branch ---

    #[test]
    fn infer_commit_type_recognizes_all_conventional_types() {
        assert_eq!(infer_commit_type_from_branch("feat/add-feature"), "feat");
        assert_eq!(infer_commit_type_from_branch("fix/login-bug"), "fix");
        assert_eq!(infer_commit_type_from_branch("docs/update-readme"), "docs");
        assert_eq!(infer_commit_type_from_branch("style/formatting"), "style");
        assert_eq!(infer_commit_type_from_branch("refactor/auth-service"), "refactor");
        assert_eq!(infer_commit_type_from_branch("perf/query-opt"), "perf");
        assert_eq!(infer_commit_type_from_branch("test/add-tests"), "test");
        assert_eq!(infer_commit_type_from_branch("build/webpack"), "build");
        assert_eq!(infer_commit_type_from_branch("ci/github-actions"), "ci");
        assert_eq!(infer_commit_type_from_branch("chore/deps"), "chore");
        assert_eq!(infer_commit_type_from_branch("revert/bad-merge"), "revert");
    }

    #[test]
    fn infer_commit_type_falls_back_to_chore() {
        assert_eq!(infer_commit_type_from_branch("ticket/abc123/something"), "chore");
        assert_eq!(infer_commit_type_from_branch("feature/not-a-type"), "chore");
        assert_eq!(infer_commit_type_from_branch("hotfix/urgent"), "chore");
    }

    #[test]
    fn infer_commit_type_no_slash_returns_chore() {
        assert_eq!(infer_commit_type_from_branch("main"), "chore");
        assert_eq!(infer_commit_type_from_branch("develop"), "chore");
    }

    #[test]
    fn infer_commit_type_nested_path() {
        assert_eq!(infer_commit_type_from_branch("feat/JIRA-123/add-oauth"), "feat");
        assert_eq!(infer_commit_type_from_branch("fix/abc12345/user-login-error"), "fix");
    }

    #[test]
    fn infer_commit_type_empty_string() {
        assert_eq!(infer_commit_type_from_branch(""), "chore");
    }

    // --- push_single_branch ---

    #[test]
    fn push_single_branch_to_bare_remote() {
        let (dir, path, _bare) = init_temp_repo_with_remote();

        Command::new("git")
            .args(["checkout", "-b", "feat/push-test"])
            .current_dir(&path)
            .output()
            .expect("checkout feature branch");

        std::fs::write(dir.path().join("new.txt"), "content").unwrap();
        commit_all_changes(&path, "add new file").unwrap();

        let result = push_single_branch(&path, "feat/push-test", "My ticket");
        assert!(result.success, "push should succeed: {}", result.message);
        assert_eq!(result.branch, "feat/push-test");
    }

    #[test]
    fn push_single_branch_commits_uncommitted_changes() {
        let (dir, path, _bare) = init_temp_repo_with_remote();

        Command::new("git")
            .args(["checkout", "-b", "feat/auto-commit-test"])
            .current_dir(&path)
            .output()
            .expect("checkout feature branch");

        std::fs::write(dir.path().join("uncommitted.txt"), "data").unwrap();
        assert!(has_uncommitted_changes(&path));

        let _result = push_single_branch(&path, "feat/auto-commit-test", "My ticket");
        assert!(!has_uncommitted_changes(&path));
    }

    #[test]
    fn push_single_branch_refuses_protected_branch_name() {
        let result = push_single_branch("/some/path", "main", "title");
        assert!(!result.success);
        assert!(result.message.contains("REFUSED"), "should mention REFUSED: {}", result.message);

        let result2 = push_single_branch("/some/path", "master", "title");
        assert!(!result2.success);
        assert!(result2.message.contains("REFUSED"), "should mention REFUSED: {}", result2.message);
    }

    #[test]
    fn push_single_branch_refuses_protected_working_dir() {
        let (_dir, path) = init_temp_repo();
        let result = push_single_branch(&path, "feat/something", "title");
        assert!(!result.success);
        assert!(result.message.contains("REFUSED"), "should mention REFUSED: {}", result.message);
    }

    #[test]
    fn push_single_branch_invalid_dir_fails() {
        let result = push_single_branch("/nonexistent/path", "feat/test", "title");
        assert!(!result.success);
    }

    // --- get_single_project_diff ---

    #[test]
    fn get_single_project_diff_no_changes_returns_empty() {
        let (_dir, path, _bare) = init_temp_repo_with_remote();

        let branch_output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&path)
            .output()
            .expect("git branch");
        let branch = String::from_utf8_lossy(&branch_output.stdout).trim().to_string();

        let (diff, files_changed) = get_single_project_diff(&path, &branch).unwrap();
        assert!(diff.trim().is_empty());
        assert_eq!(files_changed, 0);
    }

    #[test]
    fn get_single_project_diff_with_changes() {
        let (dir, path, _bare) = init_temp_repo_with_remote();

        Command::new("git")
            .args(["checkout", "-b", "feat/diff-test"])
            .current_dir(&path)
            .output()
            .expect("checkout branch");

        std::fs::write(dir.path().join("feature.txt"), "new feature code").unwrap();
        commit_all_changes(&path, "feat: add feature").unwrap();

        let (diff, files_changed) = get_single_project_diff(&path, "feat/diff-test").unwrap();
        assert!(!diff.trim().is_empty());
        assert!(diff.contains("feature.txt"));
        assert_eq!(files_changed, 1);
    }

    // --- get_current_branch ---

    #[test]
    fn get_current_branch_returns_branch_name() {
        let (_dir, path) = init_temp_repo();
        let branch = get_current_branch(&path);
        assert!(branch.is_some());
        let name = branch.unwrap();
        assert!(name == "main" || name == "master", "expected main or master, got {}", name);
    }

    #[test]
    fn get_current_branch_on_feature_branch() {
        let (_dir, path) = init_temp_repo();
        checkout_feature_branch(&path);
        let branch = get_current_branch(&path);
        assert_eq!(branch, Some("feat/test-work".to_string()));
    }

    #[test]
    fn get_current_branch_returns_none_for_invalid_dir() {
        let branch = get_current_branch("/nonexistent/path/that/does/not/exist");
        assert!(branch.is_none());
    }

    // --- is_protected_branch ---

    #[test]
    fn is_protected_branch_recognizes_main_and_master() {
        assert!(is_protected_branch("main"));
        assert!(is_protected_branch("master"));
    }

    #[test]
    fn is_protected_branch_allows_feature_branches() {
        assert!(!is_protected_branch("feat/add-feature"));
        assert!(!is_protected_branch("fix/bug"));
        assert!(!is_protected_branch("agent-work/abc/123"));
        assert!(!is_protected_branch("develop"));
        assert!(!is_protected_branch(""));
    }

    // --- assert_not_on_protected_branch ---

    #[test]
    fn assert_not_on_protected_branch_blocks_main() {
        let (_dir, path) = init_temp_repo();
        let result = assert_not_on_protected_branch(&path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("REFUSED"));
        assert!(err.contains("protected branch"));
    }

    #[test]
    fn assert_not_on_protected_branch_allows_feature_branch() {
        let (_dir, path) = init_temp_repo();
        checkout_feature_branch(&path);
        let result = assert_not_on_protected_branch(&path);
        assert!(result.is_ok());
    }

    #[test]
    fn assert_not_on_protected_branch_allows_invalid_dir() {
        let result = assert_not_on_protected_branch("/nonexistent/path");
        assert!(result.is_ok(), "should pass when branch cannot be determined");
    }
}
