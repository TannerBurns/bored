//! Tauri commands for post-completion next steps (push, PR, diff, open in editor)

use std::process::Command;
use std::sync::Arc;
use tauri::State;

use crate::db::Database;

pub use super::diff_parser::{DiffHunk, DiffLine, FileDiff};
use super::diff_parser::parse_unified_diff;

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

/// Result of getting a branch diff
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchDiff {
    pub diff: String,
    pub files_changed: usize,
    pub branch: String,
}

/// Per-project branch status for workspace tickets
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBranchStatus {
    pub project_id: String,
    pub project_name: String,
    pub branch: String,
    pub working_dir: String,
    pub has_changes: bool,
    pub files_changed: usize,
    pub additions: usize,
    pub deletions: usize,
}

/// Per-project file diffs
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileDiffs {
    pub project_id: String,
    pub project_name: String,
    pub files: Vec<FileDiff>,
}

/// Per-project push result
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPushResult {
    pub project_id: String,
    pub project_name: String,
    pub success: bool,
    pub message: String,
    pub branch: String,
}

/// Workspace-wide push results
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePushResult {
    pub results: Vec<ProjectPushResult>,
}

/// Find the working directory for a ticket (worktree or project path).
/// Returns (working_dir, branch).
pub fn get_ticket_working_dir(db: &Database, ticket_id: &str) -> Result<(String, String), String> {
    let ticket = db.get_ticket(ticket_id).map_err(|e| e.to_string())?;
    let branch = ticket
        .branch_name
        .ok_or_else(|| "Ticket has no branch name".to_string())?;

    // Try to find a project path for this ticket
    let project_path = if let Some(ref project_id) = ticket.project_id {
        let project = db
            .get_project(project_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Project not found: {}", project_id))?;
        project.path
    } else if let Some(ref workspace_id) = ticket.workspace_id {
        let projects = db
            .get_workspace_projects(workspace_id)
            .map_err(|e| e.to_string())?;
        let first = projects
            .first()
            .ok_or_else(|| "Workspace has no projects".to_string())?;
        first.path.clone()
    } else {
        return Err("Ticket has no associated project or workspace".to_string());
    };

    // Check if there's a worktree for this branch
    let worktree_output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&project_path)
        .output()
        .map_err(|e| format!("Failed to list worktrees: {}", e))?;

    let worktree_list = String::from_utf8_lossy(&worktree_output.stdout);
    let mut working_dir = project_path.clone();

    // Parse worktree list to find one matching our branch
    let mut current_worktree = String::new();
    for line in worktree_list.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            current_worktree = path.to_string();
        }
        if let Some(branch_ref) = line.strip_prefix("branch ") {
            let wt_branch = branch_ref
                .strip_prefix("refs/heads/")
                .unwrap_or(branch_ref);
            if wt_branch == branch {
                working_dir = current_worktree.clone();
                break;
            }
        }
    }

    Ok((working_dir, branch))
}

/// Get working directories for all projects in a workspace ticket.
/// For single-project tickets, returns a one-element vec.
/// Returns vec of (project_id, project_name, working_dir, branch).
pub fn get_ticket_working_dirs(
    db: &Database,
    ticket_id: &str,
) -> Result<Vec<(String, String, String, String)>, String> {
    let ticket = db.get_ticket(ticket_id).map_err(|e| e.to_string())?;
    let branch = ticket
        .branch_name
        .ok_or_else(|| "Ticket has no branch name".to_string())?;

    if let Some(ref workspace_id) = ticket.workspace_id {
        let projects = db
            .get_workspace_projects(workspace_id)
            .map_err(|e| e.to_string())?;

        let mut results = Vec::new();
        for project in &projects {
            let working_dir = resolve_working_dir_for_project(&project.path, &branch)?;
            results.push((
                project.id.clone(),
                project.name.clone(),
                working_dir,
                branch.clone(),
            ));
        }
        Ok(results)
    } else if let Some(ref project_id) = ticket.project_id {
        let project = db
            .get_project(project_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Project not found: {}", project_id))?;
        let working_dir = resolve_working_dir_for_project(&project.path, &branch)?;
        Ok(vec![(
            project.id.clone(),
            project.name.clone(),
            working_dir,
            branch,
        )])
    } else {
        Err("Ticket has no associated project or workspace".to_string())
    }
}

/// Resolve the working directory for a specific project and branch.
fn resolve_working_dir_for_project(project_path: &str, branch: &str) -> Result<String, String> {
    let worktree_output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(project_path)
        .output()
        .map_err(|e| format!("Failed to list worktrees: {}", e))?;

    let worktree_list = String::from_utf8_lossy(&worktree_output.stdout);
    let mut working_dir = project_path.to_string();
    let mut current_worktree = String::new();

    for line in worktree_list.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            current_worktree = path.to_string();
        }
        if let Some(branch_ref) = line.strip_prefix("branch ") {
            let wt_branch = branch_ref.strip_prefix("refs/heads/").unwrap_or(branch_ref);
            if wt_branch == branch {
                working_dir = current_worktree.clone();
                break;
            }
        }
    }

    Ok(working_dir)
}

/// Extract a conventional commit type from the branch name prefix.
/// Falls back to "chore" when the prefix isn't a recognized commitizen type.
fn infer_commit_type_from_branch(branch: &str) -> &'static str {
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

#[tauri::command]
pub async fn push_branch(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<PushResult, String> {
    let (working_dir, branch) = get_ticket_working_dir(&db, &ticket_id)?;

    if has_uncommitted_changes(&working_dir) {
        let ticket = db.get_ticket(&ticket_id).map_err(|e| e.to_string())?;
        let commit_type = infer_commit_type_from_branch(&branch);
        let commit_msg = format!("{}: {}", commit_type, ticket.title);
        if let Err(e) = commit_all_changes(&working_dir, &commit_msg) {
            return Ok(PushResult {
                success: false,
                message: format!("Failed to commit uncommitted changes: {}", e),
                branch,
            });
        }
    }

    let output = Command::new("git")
        .args(["push", "-u", "origin", &branch])
        .current_dir(&working_dir)
        .output()
        .map_err(|e| format!("Failed to run git push: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(PushResult {
            success: true,
            message: if stderr.is_empty() {
                stdout
            } else {
                // git push often writes to stderr even on success
                format!("{}{}", stdout, stderr)
            },
            branch,
        })
    } else {
        Ok(PushResult {
            success: false,
            message: format!("{}{}", stdout, stderr),
            branch,
        })
    }
}

/// Check whether there are uncommitted changes (staged or unstaged) in the working directory.
fn has_uncommitted_changes(working_dir: &str) -> bool {
    // `git status --porcelain` outputs one line per changed file; empty = clean
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
fn commit_all_changes(working_dir: &str, message: &str) -> Result<(), String> {
    // Stage everything (new, modified, deleted)
    let add_output = Command::new("git")
        .args(["add", "-A"])
        .current_dir(working_dir)
        .output()
        .map_err(|e| format!("Failed to run git add: {}", e))?;

    if !add_output.status.success() {
        let stderr = String::from_utf8_lossy(&add_output.stderr);
        return Err(format!("git add -A failed: {}", stderr));
    }

    // Commit
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

#[tauri::command]
pub async fn create_pull_request(
    ticket_id: String,
    title: Option<String>,
    body: Option<String>,
    db: State<'_, Arc<Database>>,
) -> Result<PullRequestResult, String> {
    let (working_dir, branch) = get_ticket_working_dir(&db, &ticket_id)?;
    let ticket = db.get_ticket(&ticket_id).map_err(|e| e.to_string())?;

    if has_uncommitted_changes(&working_dir) {
        let commit_type = infer_commit_type_from_branch(&branch);
        let commit_msg = format!("{}: {}", commit_type, ticket.title);
        if let Err(e) = commit_all_changes(&working_dir, &commit_msg) {
            return Ok(PullRequestResult {
                success: false,
                url: None,
                message: format!("Failed to commit uncommitted changes: {}", e),
            });
        }
    }

    // Always push to ensure the remote is up-to-date with local commits
    let push_output = Command::new("git")
        .args(["push", "-u", "origin", &branch])
        .current_dir(&working_dir)
        .output()
        .map_err(|e| format!("Failed to run git push: {}", e))?;

    if !push_output.status.success() {
        let stdout = String::from_utf8_lossy(&push_output.stdout);
        let stderr = String::from_utf8_lossy(&push_output.stderr);
        return Ok(PullRequestResult {
            success: false,
            url: None,
            message: format!("Failed to push branch to origin: {}{}", stdout, stderr),
        });
    }

    let pr_title = title.unwrap_or_else(|| ticket.title.clone());
    let pr_body = body.unwrap_or_else(|| {
        let mut body_parts = vec![ticket.description_md.clone()];
        body_parts.push(format!("\n---\n*Created from branch `{}`*", branch));
        body_parts.join("\n")
    });

    let output = Command::new("gh")
        .args([
            "pr",
            "create",
            "--title",
            &pr_title,
            "--body",
            &pr_body,
            "--head",
            &branch,
        ])
        .current_dir(&working_dir)
        .output()
        .map_err(|e| format!("Failed to run gh pr create: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        let url = stdout.trim().to_string();
        Ok(PullRequestResult {
            success: true,
            url: if url.is_empty() { None } else { Some(url) },
            message: "Pull request created successfully".to_string(),
        })
    } else {
        Ok(PullRequestResult {
            success: false,
            url: None,
            message: format!("{}{}", stdout, stderr),
        })
    }
}

/// Get branch diff for a ticket (sync, for use from other commands).
pub fn get_branch_diff_sync(db: &Database, ticket_id: &str) -> Result<BranchDiff, String> {
    let (working_dir, branch) = get_ticket_working_dir(db, ticket_id)?;
    let default_branch = get_default_branch(&working_dir)?;

    let diff_output = Command::new("git")
        .args(["diff", &format!("{}...{}", default_branch, branch)])
        .current_dir(&working_dir)
        .output()
        .map_err(|e| format!("Failed to run git diff: {}", e))?;

    if !diff_output.status.success() {
        let stderr = String::from_utf8_lossy(&diff_output.stderr);
        return Err(format!("git diff failed (exit {}): {}", diff_output.status, stderr.trim()));
    }

    let diff = String::from_utf8_lossy(&diff_output.stdout).to_string();

    let stat_output = Command::new("git")
        .args([
            "diff",
            "--stat",
            &format!("{}...{}", default_branch, branch),
        ])
        .current_dir(&working_dir)
        .output()
        .map_err(|e| format!("Failed to run git diff --stat: {}", e))?;

    if !stat_output.status.success() {
        let stderr = String::from_utf8_lossy(&stat_output.stderr);
        return Err(format!("git diff --stat failed (exit {}): {}", stat_output.status, stderr.trim()));
    }

    let stat = String::from_utf8_lossy(&stat_output.stdout).to_string();
    let files_changed = stat.lines().count().saturating_sub(1);

    Ok(BranchDiff {
        diff,
        files_changed,
        branch,
    })
}

/// Get structured per-file diffs for a ticket's branch.
pub fn get_branch_diff_files_sync(db: &Database, ticket_id: &str) -> Result<Vec<FileDiff>, String> {
    let BranchDiff { diff, .. } = get_branch_diff_sync(db, ticket_id)?;
    Ok(parse_unified_diff(&diff))
}

#[tauri::command]
pub async fn get_branch_diff_files(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<FileDiff>, String> {
    get_branch_diff_files_sync(&db, &ticket_id)
}

#[tauri::command]
pub async fn get_branch_diff(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<BranchDiff, String> {
    get_branch_diff_sync(&db, &ticket_id)
}

pub fn get_default_branch(working_dir: &str) -> Result<String, String> {
    // Try to determine the default branch
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

    // Fallback: check if origin/main exists, otherwise try origin/master
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

#[tauri::command]
pub async fn get_workspace_branch_status(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<ProjectBranchStatus>, String> {
    let dirs = get_ticket_working_dirs(&db, &ticket_id)?;
    let mut results = Vec::new();

    for (project_id, project_name, working_dir, branch) in &dirs {
        let default_branch = get_default_branch(working_dir)
            .unwrap_or_else(|_| "origin/main".to_string());

        let stat_output = Command::new("git")
            .args(["diff", "--stat", &format!("{}...{}", default_branch, branch)])
            .current_dir(working_dir)
            .output();

        let (has_changes, files_changed, additions, deletions) = if let Ok(output) = stat_output {
            if output.status.success() {
                let stat = String::from_utf8_lossy(&output.stdout).to_string();
                let fc = stat.lines().count().saturating_sub(1);

                let numstat = Command::new("git")
                    .args(["diff", "--numstat", &format!("{}...{}", default_branch, branch)])
                    .current_dir(working_dir)
                    .output();

                let (adds, dels) = if let Ok(ns) = numstat {
                    let text = String::from_utf8_lossy(&ns.stdout);
                    text.lines().fold((0usize, 0usize), |(a, d), line| {
                        let parts: Vec<&str> = line.split('\t').collect();
                        let add: usize = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
                        let del: usize = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                        (a + add, d + del)
                    })
                } else {
                    (0, 0)
                };

                (fc > 0, fc, adds, dels)
            } else {
                (false, 0, 0, 0)
            }
        } else {
            (false, 0, 0, 0)
        };

        results.push(ProjectBranchStatus {
            project_id: project_id.clone(),
            project_name: project_name.clone(),
            branch: branch.clone(),
            working_dir: working_dir.clone(),
            has_changes,
            files_changed,
            additions,
            deletions,
        });
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- test helpers ---

    /// Create a temporary git repo with an initial commit.
    /// Returns the TempDir (keeps it alive) and its path as a String.
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

        // Create an initial commit so HEAD exists
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
        std::fs::write(dir.path().join("README.md"), "# changed").unwrap();

        let result = commit_all_changes(&path, "test commit");
        assert!(result.is_ok());
        // Working tree should be clean after commit
        assert!(!has_uncommitted_changes(&path));
    }

    #[test]
    fn commit_all_changes_includes_untracked_files() {
        let (dir, path) = init_temp_repo();
        std::fs::write(dir.path().join("brand_new.txt"), "new content").unwrap();

        let result = commit_all_changes(&path, "add new file");
        assert!(result.is_ok());
        assert!(!has_uncommitted_changes(&path));

        // Verify the file is tracked by checking git log
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
}
