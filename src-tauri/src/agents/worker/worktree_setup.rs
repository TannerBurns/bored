//! Worktree setup logic for worker operations.

use std::collections::HashMap;
use std::path::PathBuf;
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
    pub api_url: &'a str,
    pub api_token: &'a str,
    pub provider: Arc<dyn AgentProvider>,
    pub agent_config: HashMap<String, serde_json::Value>,
}

/// Result of worktree setup
pub enum WorktreeSetupResult {
    Success(WorktreeInfo),
    Failed(String),
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
            error_handling::handle_worktree_failure(WorktreeFailureContext {
                db: ctx.db.clone(),
                app_handle: ctx.app_handle.clone(),
                ticket: ctx.ticket,
                repo_path: &ctx.repo_path,
                error: &e,
                api_url: ctx.api_url,
                api_token: ctx.api_token,
                provider: ctx.provider.clone(),
                agent_config: ctx.agent_config.clone(),
                worker_id: ctx.worker_id,
            })
            .await;
            WorktreeSetupResult::Failed(format!("Failed to create worktree: {}", e))
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
            error_handling::handle_worktree_failure(WorktreeFailureContext {
                db: ctx.db.clone(),
                app_handle: ctx.app_handle.clone(),
                ticket: ctx.ticket,
                repo_path: &ctx.repo_path,
                error: &e,
                api_url: ctx.api_url,
                api_token: ctx.api_token,
                provider: ctx.provider.clone(),
                agent_config: ctx.agent_config.clone(),
                worker_id: ctx.worker_id,
            })
            .await;
            WorktreeSetupResult::Failed(format!("Failed to create worktree: {}", e))
        }
    }
}
