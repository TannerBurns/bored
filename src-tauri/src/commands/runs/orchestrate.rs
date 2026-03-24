//! Background workflow execution logic.
//!
//! Extracted from `start_agent_run` to keep the command handler focused on
//! setup/validation while this module owns the long-running async workflow.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{Emitter, Manager, Window};

use crate::agents::orchestrator::{OrchestratorConfig, WorkflowOrchestrator};
use crate::agents::spawner::CancelHandle;
use crate::agents::worktree::{self, WorktreeInfo};
use crate::agents::AgentProvider;
use crate::db::git_stats::collect_git_stats_for_ticket;
use crate::db::models::{RunStatus, Task};
use crate::db::{AuthorType, CreateComment, Database, Ticket};

use super::branch::start_heartbeat;
use super::{AgentCompleteEvent, AgentErrorEvent, StageConfig};

/// All context needed by the background workflow task.
///
/// Collected in `start_agent_run` before spawning, then moved into the
/// async task to avoid capturing dozens of individual variables.
pub(super) struct WorkflowTaskContext {
    pub db: Arc<Database>,
    pub window: Window,
    pub run_id: String,
    pub ticket_id: String,
    pub ticket: Ticket,
    pub worktree_info: WorktreeInfo,
    pub main_repo_path: PathBuf,
    pub branch_name: String,
    pub agent_id: String,
    pub provider: Arc<dyn AgentProvider>,
    pub cancel_handles: Arc<Mutex<HashMap<String, CancelHandle>>>,
    pub agent_config: HashMap<String, serde_json::Value>,
    pub workflow_settings: Arc<Mutex<crate::commands::workflow_settings::PerAgentSettings>>,
    pub stage_configs: HashMap<String, StageConfig>,
    pub code_review_max_iterations: usize,
    pub stage_timeout_secs: u64,
    pub stage_max_retries: u32,
    pub workflow_mode_override: Option<String>,
    pub workspace_file: Option<PathBuf>,
    pub workspace_paths: Vec<PathBuf>,
}

/// Execute the multi-stage workflow in the background.
///
/// Handles heartbeat, orchestrator creation, task lifecycle (start/complete/fail),
/// result handling, ticket unlock, and worktree cleanup.
pub(super) async fn execute_workflow_task(ctx: WorkflowTaskContext) {
    let WorkflowTaskContext {
        db,
        window,
        run_id,
        ticket_id,
        ticket,
        worktree_info,
        main_repo_path,
        branch_name,
        agent_id,
        provider,
        cancel_handles,
        agent_config,
        workflow_settings,
        stage_configs,
        code_review_max_iterations,
        stage_timeout_secs,
        stage_max_retries,
        workflow_mode_override,
        workspace_file,
        workspace_paths,
    } = ctx;

    if let Err(e) = db.update_run_status(&run_id, RunStatus::Running, None, None) {
        tracing::error!("Failed to update run status: {}", e);
    }

    let running_flag = Arc::new(AtomicBool::new(true));
    let heartbeat_handle = start_heartbeat(
        db.clone(), ticket_id.clone(), run_id.clone(), running_flag.clone(),
    );

    let cancel_handles_for_cleanup = cancel_handles.clone();
    let orchestrator_working_path = worktree_info.path.clone();
    let is_temp_branch = worktree_info.is_temp_branch;
    let target_branch = worktree_info.target_branch.clone();

    let worktree_branch = Some(branch_name);
    let branch_already_created = true;

    tracing::debug!(
        "Orchestrator config: worktree_branch={:?}, branch_already_created={}, is_temp_branch={}",
        worktree_branch, branch_already_created, is_temp_branch
    );

    let task = db.get_next_pending_task(&ticket.id).ok().flatten();

    // Must succeed before continuing — complete_task/fail_task require 'in_progress' status
    if let Some(ref t) = task {
        if let Err(e) = db.start_task(&t.id, &run_id) {
            tracing::error!(
                "Failed to mark task {} as in_progress: {}. Aborting run to prevent stuck task.",
                t.id, e
            );
            let _ = db.update_run_status(
                &run_id, RunStatus::Error, None,
                Some(&format!("Failed to start task: {}", e)),
            );
            let _ = db.unlock_ticket(&ticket_id);
            let _ = safety_commit_and_record(&db, &worktree_info.path, &run_id, None, None, Some(&worktree_info.branch_name));
            let _ = worktree::remove_worktree(&worktree_info.path, &main_repo_path);
            let _ = window.emit("agent-error", &AgentErrorEvent {
                run_id: run_id.clone(),
                error: format!("Failed to start task: {}", e),
            });
            return;
        }
    }

    let resume_from_stage = ticket.paused_at_stage.clone();
    let previous_run_id = ticket.paused_run_id.clone();
    if let Some(ref stage) = resume_from_stage {
        tracing::info!(
            "Resuming ticket {} from stage '{}' (previous run: {:?})",
            ticket.id, stage, previous_run_id
        );
    }

    if ticket.paused_run_id.is_some() {
        if let Err(e) = db.clear_ticket_pause(&ticket.id) {
            tracing::warn!("Failed to clear ticket pause state: {}", e);
        }
    }

    let orchestrator = WorkflowOrchestrator::new(OrchestratorConfig {
        db: db.clone(),
        window: Some(window.clone()),
        app_handle: Some(window.app_handle().clone()),
        parent_run_id: run_id.clone(),
        ticket: ticket.clone(),
        task: task.clone(),
        repo_path: orchestrator_working_path,
        workspace_file,
        workspace_paths,
        agent_id,
        provider,
        cancel_handles,
        worktree_branch,
        branch_already_created,
        is_temp_branch,
        target_branch,
        agent_config,
        resume_from_stage,
        previous_run_id,
        workflow_settings,
        stage_configs,
        code_review_max_iterations,
        stage_timeout_secs,
        stage_max_retries,
        workflow_mode_override,
    });

    tracing::info!("Starting workflow for run {}", run_id);
    let start_time = std::time::Instant::now();
    let result = orchestrator.execute().await;
    let duration_secs = start_time.elapsed().as_secs_f64();
    tracing::info!("Workflow for run {} completed in {:.1}s (ok={})", run_id, duration_secs, result.is_ok());

    running_flag.store(false, Ordering::SeqCst);
    heartbeat_handle.abort();

    {
        let mut handles = cancel_handles_for_cleanup
            .lock()
            .expect("cancel handles mutex poisoned");
        handles.remove(&run_id);
    }

    handle_workflow_result(&db, &window, &run_id, &task, result, duration_secs);

    if let Err(e) = db.unlock_ticket(&ticket_id) {
        tracing::error!("Failed to unlock ticket {}: {}", ticket_id, e);
    }

    let safety_commit_info = safety_commit_and_record(
        &db,
        &worktree_info.path,
        &run_id,
        worktree_info.target_branch.as_deref(),
        worktree_info.target_branch.as_ref().map(|_| worktree_info.branch_name.as_str()),
        Some(&worktree_info.branch_name),
    );

    // Merge detour branch back into target if this was a detour worktree
    let mut detour_merged = false;
    if let (Some(ref target), Some(ref fork_point)) =
        (&worktree_info.target_branch, &worktree_info.detour_fork_point)
    {
        match worktree::merge_detour_into_target(
            &main_repo_path,
            &worktree_info.branch_name,
            target,
            fork_point,
        ) {
            Ok(worktree::DetourMergeResult::Merged { ref new_head }) => {
                tracing::info!(
                    "Merged detour {} into {} (HEAD: {})",
                    worktree_info.branch_name,
                    target,
                    new_head
                );
                detour_merged = true;
            }
            Ok(worktree::DetourMergeResult::MergedWorkingTreeDirty { ref new_head }) => {
                tracing::info!(
                    "Merged detour {} into {} via update-ref (HEAD: {}, working tree not updated — dirty)",
                    worktree_info.branch_name,
                    target,
                    new_head
                );
                detour_merged = true;
                post_detour_dirty_worktree_comment(&db, &ticket_id, target);
            }
            Ok(worktree::DetourMergeResult::MergedWorkingTreeStale { ref new_head }) => {
                tracing::info!(
                    "Merged detour {} into {} via update-ref (HEAD: {}, working tree stale — ff-merge failed)",
                    worktree_info.branch_name,
                    target,
                    new_head
                );
                detour_merged = true;
                post_detour_stale_worktree_comment(&db, &ticket_id, target);
            }
            Ok(worktree::DetourMergeResult::NothingToMerge) => {
                tracing::info!(
                    "Detour {} had no new commits",
                    worktree_info.branch_name
                );
                detour_merged = true;
            }
            Ok(worktree::DetourMergeResult::Diverged { .. }) => {
                tracing::warn!(
                    "Target {} diverged from detour {}; leaving detour for manual merge",
                    target,
                    worktree_info.branch_name
                );
                post_detour_recovery_comment(
                    &db, &ticket_id, &worktree_info.branch_name, target,
                    "The target branch has diverged since the agent started working.",
                );
            }
            Err(e) => {
                tracing::error!(
                    "Failed to merge detour {}: {}",
                    worktree_info.branch_name,
                    e
                );
                post_detour_recovery_comment(
                    &db, &ticket_id, &worktree_info.branch_name, target,
                    &format!("Merge failed: {}", e),
                );
            }
        }
    }

    // Record detour merge result in run metadata (only when merge was actually attempted)
    if let (Some(ref target_branch), Some(_)) =
        (&worktree_info.target_branch, &worktree_info.detour_fork_point)
    {
        if let Ok(existing) = db.get_run(&run_id) {
            let mut meta = existing.metadata.unwrap_or_else(|| serde_json::json!({}));
            if let Some(sc) = meta.get_mut("safety_commit") {
                sc["merged_to_target"] = serde_json::json!(detour_merged);
            } else {
                let mut sc = serde_json::json!({
                    "merged_to_target": detour_merged,
                    "target_branch": target_branch,
                    "detour_branch": &worktree_info.branch_name,
                    "branch": &worktree_info.branch_name,
                });
                if let Some((ref hash, ref created_at)) = safety_commit_info {
                    sc["commit_hash"] = serde_json::json!(hash);
                    sc["created_at"] = serde_json::json!(created_at);
                }
                meta["safety_commit"] = sc;
            }
            let _ = db.set_run_metadata(&run_id, &meta);
        }
    }

    // Collect git stats after detour merge so the ticket branch reflects all changes
    if let Some(ref branch) = ticket.branch_name {
        let repo_dir = main_repo_path.to_string_lossy().to_string();
        match crate::commands::next_steps::get_default_branch(&repo_dir) {
            Ok(default_branch) => {
                let stats = collect_git_stats_for_ticket(&repo_dir, branch, &default_branch);
                if let Err(e) = db.upsert_git_stats(&ticket_id, &stats) {
                    tracing::warn!("Failed to upsert git stats for ticket {}: {}", ticket_id, e);
                }
            }
            Err(e) => {
                tracing::debug!("Could not determine default branch for git stats: {}", e);
            }
        }
    }

    if let Err(e) = worktree::remove_worktree(&worktree_info.path, &main_repo_path) {
        tracing::error!("Failed to remove worktree {}: {}", worktree_info.path.display(), e);
    }

    // Only delete the detour branch if the merge succeeded; preserve it for manual merge otherwise
    if detour_merged {
        worktree::delete_branch(&main_repo_path, &worktree_info.branch_name);
    }
}

/// Handle the orchestrator result -- update DB status, manage tasks, emit events.
fn handle_workflow_result(
    db: &Database,
    window: &Window,
    run_id: &str,
    task: &Option<Task>,
    result: Result<(), String>,
    duration_secs: f64,
) {
    match result {
        Ok(()) => {
            if let Err(e) = db.update_run_status(
                run_id, RunStatus::Finished, Some(0),
                Some("Multi-stage workflow completed successfully"),
            ) {
                tracing::error!("Failed to update run {} status to Finished: {}", run_id, e);
            }

            if let Some(ref t) = task {
                if let Err(e) = db.complete_task(&t.id) {
                    tracing::warn!("Failed to mark task {} as completed: {}", t.id, e);
                }
            }

            let event = AgentCompleteEvent {
                run_id: run_id.to_string(),
                status: "finished".to_string(),
                exit_code: Some(0),
                duration_secs,
            };
            if let Err(e) = window.emit("agent-complete", &event) {
                tracing::error!("Failed to emit agent-complete event: {}", e);
            }
        }
        Err(ref e) if e.starts_with("Plan requires user clarification:") => {
            tracing::info!(
                "Run {} stopped for user clarification in {:.1}s",
                run_id, duration_secs
            );
            if let Err(db_err) = db.update_run_status(
                run_id, RunStatus::Finished, Some(0),
                Some("Waiting for user clarification"),
            ) {
                tracing::error!("Failed to update run {} status to Finished: {}", run_id, db_err);
            }

            if let Some(ref t) = task {
                if let Err(fail_err) = db.fail_task(&t.id) {
                    tracing::warn!("Failed to mark task {} as failed: {}", t.id, fail_err);
                }
            }

            let event = AgentCompleteEvent {
                run_id: run_id.to_string(),
                status: "finished".to_string(),
                exit_code: Some(0),
                duration_secs,
            };
            if let Err(emit_err) = window.emit("agent-complete", &event) {
                tracing::error!("Failed to emit agent-complete event: {}", emit_err);
            }
        }
        Err(ref e) if e.starts_with("Task deleted by auto-clarification:") => {
            tracing::info!(
                "Run {} finished after auto-clarification task deletion in {:.1}s",
                run_id, duration_secs
            );
            if let Err(db_err) = db.update_run_status(
                run_id, RunStatus::Finished, Some(0),
                Some("Task removed by auto-clarification"),
            ) {
                tracing::error!("Failed to update run {} status to Finished: {}", run_id, db_err);
            }

            let event = AgentCompleteEvent {
                run_id: run_id.to_string(),
                status: "finished".to_string(),
                exit_code: Some(0),
                duration_secs,
            };
            if let Err(emit_err) = window.emit("agent-complete", &event) {
                tracing::error!("Failed to emit agent-complete event: {}", emit_err);
            }
        }
        Err(e) => {
            let was_cancelled_or_paused = e.contains("cancelled")
                || e.contains("Cancelled")
                || e.contains("paused")
                || e.contains("Paused");

            if !was_cancelled_or_paused {
                tracing::error!("Workflow failed for run {}: {}", run_id, e);
                if let Err(db_err) = db.update_run_status(
                    run_id, RunStatus::Error, None,
                    Some(&format!("Multi-stage workflow failed: {}", e)),
                ) {
                    tracing::error!("Failed to update run {} status to Error: {}", run_id, db_err);
                }

                if let Some(ref t) = task {
                    if let Err(fail_err) = db.fail_task(&t.id) {
                        tracing::warn!("Failed to mark task {} as failed: {}", t.id, fail_err);
                    }
                }
            }

            let event = AgentErrorEvent {
                run_id: run_id.to_string(),
                error: e,
            };
            if let Err(emit_err) = window.emit("agent-error", &event) {
                tracing::error!("Failed to emit agent-error event: {}", emit_err);
            }
        }
    }
}

/// Attempt a safety commit in the worktree and record it in run metadata.
/// When detour context is provided, the metadata includes the branch names
/// so the UI can show context-aware messaging.
fn safety_commit_and_record(
    db: &Database,
    worktree_path: &std::path::Path,
    run_id: &str,
    target_branch: Option<&str>,
    detour_branch: Option<&str>,
    branch_name: Option<&str>,
) -> Option<(String, String)> {
    match worktree::safety_commit_if_needed(worktree_path, run_id) {
        Ok(Some(commit_hash)) => {
            let created_at = chrono::Utc::now().to_rfc3339();
            tracing::warn!(
                "Safety commit created for run {}: {} (agent did not commit all changes)",
                run_id,
                commit_hash
            );
            match db.get_run(run_id) {
                Ok(existing) => {
                    let mut meta = existing.metadata.unwrap_or_else(|| serde_json::json!({}));
                    let mut sc = serde_json::json!({
                        "commit_hash": commit_hash,
                        "created_at": &created_at,
                    });
                    if let Some(tb) = target_branch {
                        sc["target_branch"] = serde_json::json!(tb);
                    }
                    if let Some(db_name) = detour_branch {
                        sc["detour_branch"] = serde_json::json!(db_name);
                    }
                    if let Some(bn) = branch_name {
                        sc["branch"] = serde_json::json!(bn);
                    }
                    meta["safety_commit"] = sc;
                    if let Err(e) = db.set_run_metadata(run_id, &meta) {
                        tracing::error!("Failed to store safety commit metadata for run {}: {}", run_id, e);
                    }
                }
                Err(e) => {
                    tracing::warn!("Safety commit succeeded but failed to record metadata for run {}: {}", run_id, e);
                }
            }
            Some((commit_hash, created_at))
        }
        Ok(None) => None,
        Err(e) => {
            tracing::error!("Safety commit failed for run {}: {}", run_id, e);
            None
        }
    }
}

/// Post a system comment on the ticket notifying the user that the detour
/// branch could not be merged automatically and providing recovery steps.
fn post_detour_recovery_comment(
    db: &Database,
    ticket_id: &str,
    detour_branch: &str,
    target_branch: &str,
    reason: &str,
) {
    let body = format!(
        "## Detour Branch Needs Manual Merge\n\n\
         {reason}\n\n\
         The agent's work is preserved on branch `{detour_branch}`.\n\n\
         ### Recovery steps\n\
         ```bash\n\
         git checkout {target_branch}\n\
         git merge {detour_branch}\n\
         # resolve any conflicts, then:\n\
         git branch -d {detour_branch}\n\
         ```"
    );

    if let Err(e) = db.create_comment(&CreateComment {
        ticket_id: ticket_id.to_string(),
        author_type: AuthorType::System,
        body_md: body,
        metadata: Some(serde_json::json!({ "type": "detour-merge-failed" })),
    }) {
        tracing::error!(
            "Failed to post detour recovery comment for ticket {}: {}",
            ticket_id, e
        );
    }
}

/// Post a system comment when the detour merged via update-ref but the user's
/// working tree wasn't updated because they have uncommitted changes.
fn post_detour_dirty_worktree_comment(db: &Database, ticket_id: &str, target_branch: &str) {
    let body = format!(
        "## Working Tree Out of Sync\n\n\
         The agent's work has been merged into `{target_branch}`, but your working tree \
         was not updated because you have uncommitted changes.\n\n\
         To see the agent's changes, run:\n\
         ```bash\n\
         git stash           # save your changes\n\
         git reset --hard HEAD\n\
         git stash pop        # re-apply your changes\n\
         ```"
    );

    if let Err(e) = db.create_comment(&CreateComment {
        ticket_id: ticket_id.to_string(),
        author_type: AuthorType::System,
        body_md: body,
        metadata: Some(serde_json::json!({ "type": "detour-working-tree-dirty" })),
    }) {
        tracing::error!(
            "Failed to post dirty-worktree comment for ticket {}: {}",
            ticket_id, e
        );
    }
}

/// Post a system comment when the detour merged via update-ref but the working
/// tree wasn't updated because merge --ff-only failed (the tree itself is clean).
fn post_detour_stale_worktree_comment(db: &Database, ticket_id: &str, target_branch: &str) {
    let body = format!(
        "## Working Tree Out of Sync\n\n\
         The agent's work has been merged into `{target_branch}`, but your working tree \
         could not be updated automatically.\n\n\
         To see the agent's changes, run:\n\
         ```bash\n\
         git reset --hard HEAD\n\
         ```"
    );

    if let Err(e) = db.create_comment(&CreateComment {
        ticket_id: ticket_id.to_string(),
        author_type: AuthorType::System,
        body_md: body,
        metadata: Some(serde_json::json!({ "type": "detour-working-tree-stale" })),
    }) {
        tracing::error!(
            "Failed to post stale-worktree comment for ticket {}: {}",
            ticket_id, e
        );
    }
}
