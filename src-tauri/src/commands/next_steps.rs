//! Tauri commands for post-completion next steps (push, PR, diff, open in editor)

use std::process::Command;
use std::sync::Arc;
use tauri::State;

use crate::db::Database;

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

/// Find the working directory for a ticket (worktree or project path)
fn get_ticket_working_dir(db: &Database, ticket_id: &str) -> Result<(String, String), String> {
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
    } else {
        return Err("Ticket has no associated project".to_string());
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

#[tauri::command]
pub async fn push_branch(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<PushResult, String> {
    let (working_dir, branch) = get_ticket_working_dir(&db, &ticket_id)?;

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

#[tauri::command]
pub async fn create_pull_request(
    ticket_id: String,
    title: Option<String>,
    body: Option<String>,
    db: State<'_, Arc<Database>>,
) -> Result<PullRequestResult, String> {
    let (working_dir, branch) = get_ticket_working_dir(&db, &ticket_id)?;

    let ticket = db.get_ticket(&ticket_id).map_err(|e| e.to_string())?;

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

#[tauri::command]
pub async fn get_branch_diff(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<BranchDiff, String> {
    let (working_dir, branch) = get_ticket_working_dir(&db, &ticket_id)?;

    // Get the default branch (main or master)
    let default_branch = get_default_branch(&working_dir)?;

    // Get the diff
    let diff_output = Command::new("git")
        .args(["diff", &format!("{}...{}", default_branch, branch)])
        .current_dir(&working_dir)
        .output()
        .map_err(|e| format!("Failed to run git diff: {}", e))?;

    let diff = String::from_utf8_lossy(&diff_output.stdout).to_string();

    // Count files changed
    let stat_output = Command::new("git")
        .args([
            "diff",
            "--stat",
            &format!("{}...{}", default_branch, branch),
        ])
        .current_dir(&working_dir)
        .output()
        .map_err(|e| format!("Failed to run git diff --stat: {}", e))?;

    let stat = String::from_utf8_lossy(&stat_output.stdout).to_string();
    let files_changed = stat.lines().count().saturating_sub(1); // Last line is summary

    Ok(BranchDiff {
        diff,
        files_changed,
        branch,
    })
}

#[tauri::command]
pub async fn open_in_editor(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    let (working_dir, _branch) = get_ticket_working_dir(&db, &ticket_id)?;

    Command::new("cursor")
        .arg(&working_dir)
        .spawn()
        .map_err(|e| format!("Failed to open Cursor: {}", e))?;

    Ok(())
}

fn get_default_branch(working_dir: &str) -> Result<String, String> {
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
