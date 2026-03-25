use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::agents::worker::branching;
use crate::agents::worktree::{
    create_worktree, create_worktree_with_existing_branch, WorktreeConfig, WorktreeError,
    WorktreeInfo,
};
use crate::db::{Database, Ticket};

/// Context for setting up a worktree and branch for an agent run.
pub(super) struct WorktreeBranchSetup<'a> {
    pub ticket: &'a Ticket,
    pub run_id: &'a str,
    pub repo_path: &'a str,
    pub db: &'a Arc<Database>,
}

/// Create a git worktree for isolated agent execution.
///
/// Uses the same approach as the worker path:
/// - First runs (no branch): creates a worktree with a temporary branch name
///   (`agent-work/{ticket_id}/{run_id}`). The orchestrator's branch-gen stage
///   will later rename it to an AI-generated name.
/// - Subsequent runs (existing branch): creates a worktree using the existing branch.
///
/// Returns `(WorktreeInfo, branch_name)`. Fails with the original `WorktreeError`
/// so callers can run diagnostics on the structured error.
pub(super) async fn setup_worktree_and_branch(
    ctx: WorktreeBranchSetup<'_>,
) -> Result<(WorktreeInfo, String), WorktreeError> {
    let WorktreeBranchSetup {
        ticket, run_id, repo_path, db,
    } = ctx;

    let ticket_id = &ticket.id;
    let repo_path_buf = std::path::PathBuf::from(repo_path);

    if let Some(ref existing_branch) = ticket.branch_name {
        tracing::info!(
            "Ticket {} already has branch: {}, creating worktree",
            ticket_id, existing_branch
        );

        let worktree = create_worktree_with_existing_branch(
            &repo_path_buf, existing_branch, run_id, None,
        )
        .inspect(|info| {
            tracing::info!(
                "Created worktree for run {} at {} using branch {} (target: {:?})",
                run_id, info.path.display(), info.branch_name, info.target_branch
            );
        })
        .inspect_err(|e| {
            tracing::error!(
                "Failed to create worktree for existing branch '{}': {}. \
                 Aborting run to prevent working directly on the main repo.",
                existing_branch, e
            );
        })?;

        // On detour branches, worktree.branch_name != existing_branch.
        let actual_branch = worktree.branch_name.clone();
        Ok((worktree, actual_branch))
    } else {
        let temp_branch = format!(
            "agent-work/{}/{}",
            &ticket_id[..8.min(ticket_id.len())],
            &run_id[..8.min(run_id.len())]
        );

        let base_branch =
            branching::get_base_branch_for_ticket(db, ticket, "direct-run");

        tracing::info!(
            "Ticket {} has no branch yet, creating worktree with temp branch: {}{}",
            ticket_id,
            temp_branch,
            base_branch
                .as_ref()
                .map_or(String::new(), |b| format!(" (based on {})", b))
        );

        let mut worktree = create_worktree(&WorktreeConfig {
            repo_path: repo_path_buf.clone(),
            branch_name: temp_branch.clone(),
            run_id: run_id.to_string(),
            base_dir: None,
            base_branch,
        })
        .inspect_err(|e| {
            tracing::error!(
                "Failed to create worktree with temp branch '{}': {}. \
                 Aborting run to prevent working directly on the main repo.",
                temp_branch, e
            );
        })?;

        tracing::info!(
            "Created new worktree for run {} at {} with temp branch {}",
            run_id, worktree.path.display(), worktree.branch_name
        );

        worktree.is_temp_branch = true;

        Ok((worktree, temp_branch))
    }
}

/// Start a heartbeat task to extend the lock periodically
///
/// Returns a task handle that can be aborted when the run completes.
pub(super) fn start_heartbeat(
    db: Arc<Database>,
    ticket_id: String,
    run_id: String,
    running: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<()> {
    const HEARTBEAT_INTERVAL_SECS: u64 = 60;
    const LOCK_DURATION_MINS: i64 = 30;

    tokio::spawn(async move {
        let mut ticker =
            tokio::time::interval(std::time::Duration::from_secs(HEARTBEAT_INTERVAL_SECS));

        loop {
            ticker.tick().await;

            if !running.load(Ordering::SeqCst) {
                tracing::debug!("Heartbeat stopping - run {} is no longer running", run_id);
                break;
            }

            let new_expires = chrono::Utc::now() + chrono::Duration::minutes(LOCK_DURATION_MINS);

            if let Err(e) = db.extend_lock(&ticket_id, &run_id, new_expires) {
                tracing::error!("Heartbeat failed for ticket {}: {}", ticket_id, e);
                break;
            }

            tracing::debug!(
                "Heartbeat: extended lock for ticket {} until {}",
                ticket_id,
                new_expires
            );
        }
    })
}
