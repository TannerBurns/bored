//! Tauri commands for post-completion next steps (push, PR, diff, open in editor)

use std::process::Command;
use std::sync::Arc;
use tauri::State;

use crate::db::Database;

pub use super::diff_parser::{DiffHunk, DiffLine, FileDiff};
use super::diff_parser::parse_unified_diff;

pub use super::git_helpers::{
    PullRequestResult, PushResult,
    commit_all_changes, get_default_branch, has_uncommitted_changes,
};
use super::git_helpers::{
    check_has_unpushed, create_pr_for_project, get_single_project_diff, push_single_branch,
};

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
    pub has_unpushed: bool,
    pub has_uncommitted: bool,
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

    let working_dir = resolve_working_dir_for_project(&project_path, &branch)?;
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
/// Returns the worktree path if one exists for this branch, otherwise the project path.
pub fn resolve_working_dir_for_project(project_path: &str, branch: &str) -> Result<String, String> {
    let worktree_output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(project_path)
        .output()
        .map_err(|e| format!("Failed to list worktrees: {}", e))?;

    let worktree_list = String::from_utf8_lossy(&worktree_output.stdout);
    let mut current_worktree = String::new();

    for line in worktree_list.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            current_worktree = path.to_string();
        }
        if let Some(branch_ref) = line.strip_prefix("branch ") {
            let wt_branch = branch_ref.strip_prefix("refs/heads/").unwrap_or(branch_ref);
            if wt_branch == branch {
                if std::path::Path::new(&current_worktree).exists() {
                    return Ok(current_worktree);
                }
                tracing::warn!(
                    "Worktree for branch {} listed at {} but directory missing, pruning stale reference",
                    branch,
                    current_worktree
                );
                let _ = Command::new("git")
                    .args(["worktree", "prune"])
                    .current_dir(project_path)
                    .output();
                break;
            }
        }
    }

    Ok(project_path.to_string())
}

/// Resolve working dir + branch for a specific project within a ticket, or fall
/// back to `get_ticket_working_dir` when no project_id is given.
fn resolve_ticket_project_dir(
    db: &Database,
    ticket_id: &str,
    project_id: Option<&str>,
) -> Result<(String, String), String> {
    if let Some(pid) = project_id {
        let ticket = db.get_ticket(ticket_id).map_err(|e| e.to_string())?;
        let branch = ticket
            .branch_name
            .ok_or_else(|| "Ticket has no branch name".to_string())?;
        let project = db
            .get_project(pid)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Project not found: {}", pid))?;
        let working_dir = resolve_working_dir_for_project(&project.path, &branch)?;
        Ok((working_dir, branch))
    } else {
        get_ticket_working_dir(db, ticket_id)
    }
}

#[tauri::command]
pub async fn push_branch(
    ticket_id: String,
    project_id: Option<String>,
    db: State<'_, Arc<Database>>,
) -> Result<PushResult, String> {
    if project_id.is_none() {
        let ticket = db.get_ticket(&ticket_id).map_err(|e| e.to_string())?;
        if ticket.workspace_id.is_some() {
            let dirs = get_ticket_working_dirs(&db, &ticket_id)?;
            if dirs.len() > 1 {
                let mut any_success = false;
                let mut messages = Vec::new();
                let mut branch_name = String::new();

                for (_, project_name, working_dir, branch) in &dirs {
                    branch_name = branch.clone();
                    let result = push_single_branch(working_dir, branch, &ticket.title);
                    if result.success {
                        any_success = true;
                        messages.push(format!("[{}] pushed successfully", project_name));
                    } else {
                        messages.push(format!("[{}] {}", project_name, result.message));
                    }
                }

                return Ok(PushResult {
                    success: any_success,
                    message: messages.join("\n"),
                    branch: branch_name,
                });
            }
        }
    }

    let (working_dir, branch) = resolve_ticket_project_dir(&db, &ticket_id, project_id.as_deref())?;
    let ticket = db.get_ticket(&ticket_id).map_err(|e| e.to_string())?;
    Ok(push_single_branch(&working_dir, &branch, &ticket.title))
}

#[tauri::command]
pub async fn create_pull_request(
    ticket_id: String,
    project_id: Option<String>,
    title: Option<String>,
    body: Option<String>,
    db: State<'_, Arc<Database>>,
) -> Result<PullRequestResult, String> {
    let ticket = db.get_ticket(&ticket_id).map_err(|e| e.to_string())?;

    if project_id.is_none() && ticket.workspace_id.is_some() {
        let dirs = get_ticket_working_dirs(&db, &ticket_id)?;
        if dirs.len() > 1 {
            let mut urls = Vec::new();
            let mut messages = Vec::new();
            let mut all_success = true;

            for (_, project_name, working_dir, branch) in &dirs {
                let default_branch = get_default_branch(working_dir)
                    .unwrap_or_else(|_| "origin/main".to_string());

                let has_changes = Command::new("git")
                    .args(["diff", "--stat", &format!("{}...{}", default_branch, branch)])
                    .current_dir(working_dir)
                    .output()
                    .map(|o| {
                        o.status.success()
                            && o.stdout.len() > 1
                    })
                    .unwrap_or(false);

                if !has_changes {
                    messages.push(format!("[{}] no changes, skipped", project_name));
                    continue;
                }

                let pr_title = title.clone().unwrap_or_else(|| ticket.title.clone());
                let pr_body = body.clone().unwrap_or_else(|| {
                    format!(
                        "{}\n\n---\n*Created from branch `{}` (project: {})*",
                        ticket.description_md, branch, project_name
                    )
                });

                let result = create_pr_for_project(
                    working_dir,
                    branch,
                    &ticket.title,
                    &pr_title,
                    &pr_body,
                );

                if result.success {
                    if let Some(ref url) = result.url {
                        urls.push(url.clone());
                    }
                    messages.push(format!("[{}] PR created successfully", project_name));
                } else {
                    all_success = false;
                    messages.push(format!("[{}] {}", project_name, result.message));
                }
            }

            return Ok(PullRequestResult {
                success: all_success && !urls.is_empty(),
                url: if urls.is_empty() { None } else { Some(urls.join("\n")) },
                message: messages.join("\n"),
            });
        }
    }

    let (working_dir, branch) = resolve_ticket_project_dir(&db, &ticket_id, project_id.as_deref())?;

    let pr_title = title.unwrap_or_else(|| ticket.title.clone());
    let pr_body = body.unwrap_or_else(|| {
        format!("{}\n\n---\n*Created from branch `{}`*", ticket.description_md, branch)
    });

    Ok(create_pr_for_project(
        &working_dir,
        &branch,
        &ticket.title,
        &pr_title,
        &pr_body,
    ))
}

/// Get branch diff for a ticket (sync, for use from other commands).
pub fn get_branch_diff_sync(db: &Database, ticket_id: &str) -> Result<BranchDiff, String> {
    let ticket = db.get_ticket(ticket_id).map_err(|e| e.to_string())?;

    if ticket.workspace_id.is_some() {
        let dirs = get_ticket_working_dirs(db, ticket_id)?;
        if dirs.len() > 1 {
            let mut combined_diff = String::new();
            let mut total_files_changed = 0usize;
            let branch = dirs[0].3.clone();

            for (_, project_name, working_dir, branch) in &dirs {
                match get_single_project_diff(working_dir, branch) {
                    Ok((diff, files_changed)) => {
                        if !diff.trim().is_empty() {
                            combined_diff.push_str(&format!("\n### Project: {}\n\n", project_name));
                            combined_diff.push_str(&diff);
                        }
                        total_files_changed += files_changed;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to get diff for project {}: {}", project_name, e);
                    }
                }
            }

            return Ok(BranchDiff {
                diff: combined_diff,
                files_changed: total_files_changed,
                branch,
            });
        }
    }

    let (working_dir, branch) = get_ticket_working_dir(db, ticket_id)?;
    let (diff, files_changed) = get_single_project_diff(&working_dir, &branch)?;

    Ok(BranchDiff {
        diff,
        files_changed,
        branch,
    })
}

/// Get structured per-file diffs for a ticket's branch.
pub fn get_branch_diff_files_sync(db: &Database, ticket_id: &str) -> Result<Vec<FileDiff>, String> {
    let ticket = db.get_ticket(ticket_id).map_err(|e| e.to_string())?;

    if ticket.workspace_id.is_some() {
        let dirs = get_ticket_working_dirs(db, ticket_id)?;
        if dirs.len() > 1 {
            let mut all_files = Vec::new();
            for (_, project_name, working_dir, branch) in &dirs {
                match get_single_project_diff(working_dir, branch) {
                    Ok((diff, _)) => {
                        let mut files = parse_unified_diff(&diff);
                        for file in &mut files {
                            file.path = format!("[{}] {}", project_name, file.path);
                        }
                        all_files.extend(files);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to get diff for project {}: {}", project_name, e);
                    }
                }
            }
            return Ok(all_files);
        }
    }

    let BranchDiff { diff, .. } = get_branch_diff_sync(db, ticket_id)?;
    Ok(parse_unified_diff(&diff))
}

#[tauri::command]
pub async fn get_branch_diff_files(
    ticket_id: String,
    project_id: Option<String>,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<FileDiff>, String> {
    if let Some(ref pid) = project_id {
        let ticket = db.get_ticket(&ticket_id).map_err(|e| e.to_string())?;
        let branch = ticket
            .branch_name
            .ok_or_else(|| "Ticket has no branch name".to_string())?;
        let project = db
            .get_project(pid)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Project not found: {}", pid))?;
        let working_dir = resolve_working_dir_for_project(&project.path, &branch)?;
        let default_branch = get_default_branch(&working_dir)?;

        let diff_output = Command::new("git")
            .args(["diff", &format!("{}...{}", default_branch, branch)])
            .current_dir(&working_dir)
            .output()
            .map_err(|e| format!("Failed to run git diff: {}", e))?;

        if !diff_output.status.success() {
            let stderr = String::from_utf8_lossy(&diff_output.stderr);
            return Err(format!("git diff failed: {}", stderr.trim()));
        }

        let diff = String::from_utf8_lossy(&diff_output.stdout).to_string();
        Ok(parse_unified_diff(&diff))
    } else {
        get_branch_diff_files_sync(&db, &ticket_id)
    }
}

#[tauri::command]
pub async fn get_branch_diff(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<BranchDiff, String> {
    get_branch_diff_sync(&db, &ticket_id)
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

        let has_unpushed = check_has_unpushed(working_dir, branch);
        let has_uncommitted = has_uncommitted_changes(working_dir);

        results.push(ProjectBranchStatus {
            project_id: project_id.clone(),
            project_name: project_name.clone(),
            branch: branch.clone(),
            working_dir: working_dir.clone(),
            has_changes,
            has_unpushed,
            has_uncommitted,
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

    // --- resolve_working_dir_for_project ---

    #[test]
    fn resolve_working_dir_no_worktree_returns_project_path() {
        let (_dir, path) = init_temp_repo();
        let result = resolve_working_dir_for_project(&path, "feat/some-branch").unwrap();
        assert_eq!(result, path);
    }

    #[test]
    fn resolve_working_dir_with_worktree_returns_worktree_path() {
        let (_dir, path) = init_temp_repo();

        Command::new("git")
            .args(["branch", "feat/wt-test"])
            .current_dir(&path)
            .output()
            .expect("create branch");

        let wt_dir = tempfile::tempdir().expect("wt temp dir");
        let wt_path = wt_dir.path().to_str().unwrap().to_string();
        std::fs::remove_dir(&wt_path).ok();

        Command::new("git")
            .args(["worktree", "add", &wt_path, "feat/wt-test"])
            .current_dir(&path)
            .output()
            .expect("git worktree add");

        let result = resolve_working_dir_for_project(&path, "feat/wt-test").unwrap();
        let canon = |p: &str| std::fs::canonicalize(p).unwrap_or_else(|_| std::path::PathBuf::from(p));
        assert_eq!(canon(&result), canon(&wt_path));
    }

    #[test]
    fn resolve_working_dir_stale_worktree_falls_back_to_project() {
        let (_dir, path) = init_temp_repo();

        Command::new("git")
            .args(["branch", "feat/stale"])
            .current_dir(&path)
            .output()
            .expect("create branch");

        let wt_dir = tempfile::tempdir().expect("wt temp dir");
        let wt_path = wt_dir.path().to_str().unwrap().to_string();
        std::fs::remove_dir(&wt_path).ok();

        Command::new("git")
            .args(["worktree", "add", &wt_path, "feat/stale"])
            .current_dir(&path)
            .output()
            .expect("git worktree add");

        std::fs::remove_dir_all(&wt_path).expect("remove wt dir");

        let result = resolve_working_dir_for_project(&path, "feat/stale").unwrap();
        assert_eq!(result, path, "should fall back to project path when worktree dir is missing");
    }

    #[test]
    fn resolve_working_dir_invalid_dir_returns_err() {
        let result = resolve_working_dir_for_project("/nonexistent/path/xyz", "main");
        assert!(result.is_err());
    }
}
