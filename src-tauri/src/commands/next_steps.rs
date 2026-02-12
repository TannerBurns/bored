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

/// Per-file diff for the file-by-file diff viewer
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub path: String,
    /// "modified", "added", "deleted", "renamed"
    pub status: String,
    pub additions: usize,
    pub deletions: usize,
    pub hunks: Vec<DiffHunk>,
}

/// A hunk in a unified diff (e.g. @@ -1,5 +1,7 @@)
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

/// A single line in a hunk
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    /// "add", "delete", "context"
    pub line_type: String,
    pub content: String,
    pub old_line_num: Option<usize>,
    pub new_line_num: Option<usize>,
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

/// Get branch diff for a ticket (sync, for use from other commands).
pub fn get_branch_diff_sync(db: &Database, ticket_id: &str) -> Result<BranchDiff, String> {
    let (working_dir, branch) = get_ticket_working_dir(db, ticket_id)?;
    let default_branch = get_default_branch(&working_dir)?;

    let diff_output = Command::new("git")
        .args(["diff", &format!("{}...{}", default_branch, branch)])
        .current_dir(&working_dir)
        .output()
        .map_err(|e| format!("Failed to run git diff: {}", e))?;

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

    let stat = String::from_utf8_lossy(&stat_output.stdout).to_string();
    let files_changed = stat.lines().count().saturating_sub(1);

    Ok(BranchDiff {
        diff,
        files_changed,
        branch,
    })
}

/// Parse unified diff output into per-file structured diffs.
fn parse_unified_diff(diff: &str) -> Vec<FileDiff> {
    let mut files = Vec::new();
    let diff_splits = diff.split("\ndiff --git ");
    for (i, block) in diff_splits.enumerate() {
        let block = block.trim_start();
        let block = if i == 0 && !block.starts_with("diff --git ") {
            continue;
        } else if i == 0 {
            block.strip_prefix("diff --git ").unwrap_or(block)
        } else {
            block
        };
        let mut lines = block.lines();
        let first = match lines.next() {
            Some(l) => l,
            None => continue,
        };
        // First line: "a/path b/path" or "a/path b/path\n"
        let path = first
            .strip_prefix("a/")
            .and_then(|s| s.split(" b/").next())
            .unwrap_or(first)
            .to_string();
        if path.is_empty() {
            continue;
        }
        let (mut additions, mut deletions) = (0usize, 0usize);
        let mut hunks = Vec::new();
        let mut in_header = true;
        let mut current_hunk_header = String::new();
        let mut current_hunk_lines: Vec<DiffLine> = Vec::new();
        let mut old_line = 0usize;
        let mut new_line = 0usize;

        for line in lines {
            if line.starts_with("@@ ") {
                if !current_hunk_header.is_empty() {
                    hunks.push(DiffHunk {
                        header: current_hunk_header.clone(),
                        lines: current_hunk_lines.clone(),
                    });
                }
                current_hunk_header = line.to_string();
                current_hunk_lines.clear();
                if let Some(rest) = line.strip_prefix("@@ ") {
                    if let Some(rest) = rest.strip_suffix(" @@") {
                        let parts: Vec<&str> = rest.split(' ').collect();
                        if let Some(old_part) = parts.first() {
                            old_line = old_part.split(',').next().and_then(|s| s.parse().ok()).unwrap_or(1);
                        }
                        if let Some(new_part) = parts.get(1) {
                            new_line = new_part.split(',').next().and_then(|s| s.parse().ok()).unwrap_or(1);
                        }
                    }
                }
                in_header = false;
                continue;
            }
            if in_header {
                if line.starts_with("new file mode") {
                    additions += 0;
                }
                continue;
            }
            let (line_type, content) = if let Some(rest) = line.get(1..) {
                match line.chars().next() {
                    Some('+') => {
                        additions += 1;
                        (Some("add"), rest.to_string())
                    }
                    Some('-') => {
                        deletions += 1;
                        (Some("delete"), rest.to_string())
                    }
                    Some(' ') => (Some("context"), rest.to_string()),
                    _ => (None, line.to_string()),
                }
            } else {
                (Some("context"), String::new())
            };
            if let Some(lt) = line_type {
                let (old_num, new_num) = match lt {
                    "add" => (None, Some(new_line)),
                    "delete" => (Some(old_line), None),
                    _ => (Some(old_line), Some(new_line)),
                };
                current_hunk_lines.push(DiffLine {
                    line_type: lt.to_string(),
                    content,
                    old_line_num: old_num,
                    new_line_num: new_num,
                });
                match lt {
                    "add" => new_line = new_line.saturating_add(1),
                    "delete" => old_line = old_line.saturating_add(1),
                    _ => {
                        old_line = old_line.saturating_add(1);
                        new_line = new_line.saturating_add(1);
                    }
                }
            }
        }
        if !current_hunk_header.is_empty() {
            hunks.push(DiffHunk {
                header: current_hunk_header,
                lines: current_hunk_lines,
            });
        }
        let status = if additions > 0 && deletions == 0 && hunks.iter().all(|h| h.lines.iter().all(|l| l.line_type != "delete")) {
            "added"
        } else if deletions > 0 && additions == 0 && hunks.iter().all(|h| h.lines.iter().all(|l| l.line_type != "add")) {
            "deleted"
        } else {
            "modified"
        };
        files.push(FileDiff {
            path,
            status: status.to_string(),
            additions,
            deletions,
            hunks,
        });
    }
    files
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
