//! Worktree setup logic for worker operations.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::agents::provider::AgentProvider;
use crate::db::{Database, Ticket};

use super::super::worktree::{self, WorktreeConfig, WorktreeInfo};
use super::branching;
use super::error_handling::{self, WorktreeFailureContext};

/// Context for worktree creation
pub struct WorktreeSetupContext<'a> {
    pub db: Arc<Database>,
    pub ticket: &'a Ticket,
    pub run_id: &'a str,
    pub repo_path: PathBuf,
    pub worker_id: &'a str,
    pub app_handle: Option<tauri::AppHandle>,
    pub provider: Arc<dyn AgentProvider>,
    pub agent_config: HashMap<String, serde_json::Value>,
    pub diagnostic_model: Option<String>,
}

/// Result of worktree setup
pub enum WorktreeSetupResult {
    Success(WorktreeInfo),
    Failed {
        message: String,
        /// Whether the ticket was successfully moved to the Blocked column.
        ticket_blocked: bool,
    },
}

/// Create a worktree for the ticket, handling both existing branch and new branch cases.
pub async fn create_worktree_for_ticket(ctx: WorktreeSetupContext<'_>) -> WorktreeSetupResult {
    if let Some(ref existing_branch) = ctx.ticket.branch_name {
        create_worktree_with_existing_branch(&ctx, existing_branch).await
    } else {
        create_worktree_with_new_branch(&ctx).await
    }
}

/// Create a worktree using an existing branch
async fn create_worktree_with_existing_branch(
    ctx: &WorktreeSetupContext<'_>,
    existing_branch: &str,
) -> WorktreeSetupResult {
    tracing::info!(
        "Worker {} found existing branch for ticket {}: {}, creating worktree",
        ctx.worker_id,
        ctx.ticket.id,
        existing_branch
    );

    match worktree::create_worktree_with_existing_branch(
        &ctx.repo_path,
        existing_branch,
        ctx.run_id,
        None,
    ) {
        Ok(info) => {
            tracing::info!(
                "Worker {} created worktree at {} using branch {}",
                ctx.worker_id,
                info.path.display(),
                info.branch_name
            );
            WorktreeSetupResult::Success(info)
        }
        Err(e) => {
            tracing::error!(
                "Worker {} failed to create worktree for ticket {}: {}",
                ctx.worker_id,
                ctx.ticket.id,
                e
            );
            let ticket_blocked = error_handling::handle_worktree_failure(WorktreeFailureContext {
                db: ctx.db.clone(),
                app_handle: ctx.app_handle.clone(),
                ticket: ctx.ticket,
                repo_path: &ctx.repo_path,
                error: &e,
                provider: ctx.provider.clone(),
                agent_config: ctx.agent_config.clone(),
                worker_id: ctx.worker_id,
                diagnostic_model: ctx.diagnostic_model.clone(),
            })
            .await;
            WorktreeSetupResult::Failed {
                message: format!("Failed to create worktree: {}", e),
                ticket_blocked,
            }
        }
    }
}

/// Create a worktree with a new temporary branch
async fn create_worktree_with_new_branch(ctx: &WorktreeSetupContext<'_>) -> WorktreeSetupResult {
    let temp_branch = format!(
        "agent-work/{}/{}",
        &ctx.ticket.id[..8.min(ctx.ticket.id.len())],
        &ctx.run_id[..8.min(ctx.run_id.len())]
    );

    // For epic child tickets, check if previous sibling has a branch
    let base_branch = branching::get_base_branch_for_ticket(&ctx.db, ctx.ticket, ctx.worker_id);

    tracing::info!(
        "Worker {} ticket {} has no branch yet, creating worktree with temp branch: {}{}",
        ctx.worker_id,
        ctx.ticket.id,
        temp_branch,
        base_branch
            .as_ref()
            .map_or(String::new(), |b| format!(" (based on {})", b))
    );

    match worktree::create_worktree(&WorktreeConfig {
        repo_path: ctx.repo_path.clone(),
        branch_name: temp_branch.clone(),
        run_id: ctx.run_id.to_string(),
        base_dir: None,
        base_branch,
    }) {
        Ok(mut info) => {
            tracing::info!(
                "Worker {} created worktree at {} with temp branch {}",
                ctx.worker_id,
                info.path.display(),
                info.branch_name
            );
            info.is_temp_branch = true;
            WorktreeSetupResult::Success(info)
        }
        Err(e) => {
            tracing::error!(
                "Worker {} failed to create worktree for ticket {}: {}",
                ctx.worker_id,
                ctx.ticket.id,
                e
            );
            let ticket_blocked = error_handling::handle_worktree_failure(WorktreeFailureContext {
                db: ctx.db.clone(),
                app_handle: ctx.app_handle.clone(),
                ticket: ctx.ticket,
                repo_path: &ctx.repo_path,
                error: &e,
                provider: ctx.provider.clone(),
                agent_config: ctx.agent_config.clone(),
                worker_id: ctx.worker_id,
                diagnostic_model: ctx.diagnostic_model.clone(),
            })
            .await;
            WorktreeSetupResult::Failed {
                message: format!("Failed to create worktree: {}", e),
                ticket_blocked,
            }
        }
    }
}

/// Result of workspace worktree setup (multiple worktrees + .code-workspace file)
pub struct WorkspaceWorktreeSet {
    pub worktrees: Vec<WorktreeInfo>,
    pub workspace_file: PathBuf,
}

/// Create worktrees for all projects in a workspace, using the same branch name.
/// Generates a .code-workspace file pointing to all worktrees.
/// On partial failure, cleans up already-created worktrees.
pub async fn create_worktrees_for_workspace(
    db: Arc<Database>,
    workspace_id: &str,
    ticket: &Ticket,
    run_id: &str,
    worker_id: &str,
    app_handle: Option<tauri::AppHandle>,
    provider: Arc<dyn AgentProvider>,
    agent_config: HashMap<String, serde_json::Value>,
    diagnostic_model: Option<String>,
) -> Result<WorkspaceWorktreeSet, String> {
    let projects = db
        .get_workspace_projects(workspace_id)
        .map_err(|e| format!("Failed to get workspace projects: {}", e))?;

    if projects.is_empty() {
        return Err("Workspace has no projects".to_string());
    }

    let mut created_worktrees: Vec<WorktreeInfo> = Vec::new();

    for project in &projects {
        let repo_path = PathBuf::from(&project.path);
        let ctx = WorktreeSetupContext {
            db: db.clone(),
            ticket,
            run_id,
            repo_path,
            worker_id,
            app_handle: app_handle.clone(),
            provider: provider.clone(),
            agent_config: agent_config.clone(),
            diagnostic_model: diagnostic_model.clone(),
        };

        match create_worktree_for_ticket(ctx).await {
            WorktreeSetupResult::Success(info) => {
                created_worktrees.push(info);
            }
            WorktreeSetupResult::Failed { message, .. } => {
                for wt in &created_worktrees {
                    let _ = worktree::remove_worktree(&wt.path, &wt.repo_path);
                }
                return Err(format!(
                    "Failed to create worktree for project '{}': {}",
                    project.name, message
                ));
            }
        }
    }

    let workspace_dir = created_worktrees[0]
        .path
        .parent()
        .unwrap_or_else(|| Path::new("/tmp"))
        .to_path_buf();
    let workspace_file = workspace_dir.join(format!("{}.code-workspace", run_id));

    let folders: Vec<serde_json::Value> = created_worktrees
        .iter()
        .zip(projects.iter())
        .map(|(wt, proj)| {
            serde_json::json!({
                "path": wt.path.to_string_lossy(),
                "name": proj.name
            })
        })
        .collect();

    let workspace_content = serde_json::json!({
        "folders": folders
    });

    let json = serde_json::to_string_pretty(&workspace_content)
        .map_err(|e| format!("Failed to serialize .code-workspace file: {}", e))?;
    std::fs::write(&workspace_file, json)
        .map_err(|e| format!("Failed to write .code-workspace file: {}", e))?;

    Ok(WorkspaceWorktreeSet {
        worktrees: created_worktrees,
        workspace_file,
    })
}
