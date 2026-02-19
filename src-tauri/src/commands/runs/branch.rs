use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::{Emitter, Window};

use crate::agents::prompt::{
    generate_branch_name_generation_prompt, parse_branch_name_from_output,
};
use crate::agents::worktree::{
    self, create_worktree, create_worktree_with_existing_branch, WorktreeConfig, WorktreeInfo,
};
use crate::agents::{run_agent_via_provider, AgentProvider, AgentRunConfig};
use crate::db::models::{CreateRun, RunStatus};
use crate::db::{Database, Ticket};

/// Context for setting up a worktree and branch for an agent run.
pub(super) struct WorktreeBranchSetup<'a> {
    pub ticket: &'a Ticket,
    pub run_id: &'a str,
    pub repo_path: &'a str,
    pub agent_id: &'a str,
    pub provider: Arc<dyn AgentProvider>,
    pub db: &'a Arc<Database>,
    pub window: &'a Window,
    pub branch_gen_model: Option<String>,
    pub agent_config: &'a std::collections::HashMap<String, serde_json::Value>,
}

/// Generate a branch name using AI via a quick agent call
///
/// This runs a quick Claude/Cursor agent call to generate a meaningful branch name
/// based on the ticket's title and description.
pub(super) async fn generate_ai_branch_name(
    ticket: &Ticket,
    repo_path: &std::path::Path,
    agent_id: &str,
    provider: Arc<dyn AgentProvider>,
    db: Arc<Database>,
    model: Option<String>,
    agent_config: &std::collections::HashMap<String, serde_json::Value>,
) -> Option<String> {
    let prompt = generate_branch_name_generation_prompt(ticket);
    let run_id = uuid::Uuid::new_v4().to_string();

    tracing::info!(
        "Generating AI branch name for ticket {} via quick agent call",
        ticket.id
    );

    // Create a temporary sub-run for the branch generation stage
    let sub_run = db.create_run(&CreateRun {
        ticket_id: ticket.id.clone(),
        agent_type: agent_id.to_string(),
        repo_path: repo_path.to_string_lossy().to_string(),
        parent_run_id: None,
        stage: Some("branch-gen".to_string()),
        ..Default::default()
    });

    if let Err(e) = &sub_run {
        tracing::warn!("Failed to create branch-gen sub-run: {}", e);
    }

    let model_for_cost = model.clone().unwrap_or_default();

    tracing::info!(
        "Branch-gen config: agent={}, model={:?}, repo={}",
        agent_id,
        &model_for_cost,
        repo_path.display(),
    );

    let config = AgentRunConfig {
        agent_id: agent_id.to_string(),
        ticket_id: ticket.id.clone(),
        run_id: run_id.clone(),
        repo_path: repo_path.to_path_buf(),
        prompt: prompt.clone(),
        timeout_secs: Some(60),
        api_url: String::new(),
        api_token: String::new(),
        model,
        agent_config: agent_config.clone(),
    };

    let start_time = std::time::Instant::now();

    let provider_for_extract = provider.clone();
    let result = tokio::task::spawn_blocking(move || {
        run_agent_via_provider(&*provider, &config, None)
    })
    .await;

    match result {
        Ok(Ok(agent_result)) => {
            let duration_secs = start_time.elapsed().as_secs_f64();
            let stdout = agent_result.captured_stdout.as_deref().unwrap_or("");
            let stdout_len = stdout.len();

            tracing::info!(
                "Branch-gen agent finished: exit_code={:?}, status={:?}, stdout_len={}, duration={:.1}s",
                agent_result.exit_code,
                agent_result.status,
                stdout_len,
                duration_secs,
            );

            let cost_data = crate::agents::provider::extract_cost_with_overrides(
                &*provider_for_extract,
                stdout,
                &model_for_cost,
                agent_config,
                duration_secs,
            );
            if let Ok(ref sr) = sub_run {
                let mut metadata = serde_json::json!({
                    "duration_secs": duration_secs,
                    "exit_code": agent_result.exit_code,
                    "stdout_len": stdout_len,
                });
                if let Some(ref cost) = cost_data {
                    metadata["cost"] = serde_json::to_value(cost).unwrap_or_default();
                }
                let _ = db.set_run_metadata(&sr.id, &metadata);
            }

            if stdout.is_empty() {
                tracing::warn!(
                    "Branch-gen produced no stdout (exit_code={:?})",
                    agent_result.exit_code,
                );
            } else {
                let text_content = provider_for_extract.extract_text(stdout);
                let preview: String = text_content.chars().take(500).collect();
                tracing::info!("Branch-gen extracted text ({} chars): {}", text_content.len(), preview);

                if let Some(branch_name) = parse_branch_name_from_output(&text_content) {
                    tracing::info!("AI generated branch name: {}", branch_name);
                    if let Ok(ref sr) = sub_run {
                        let _ = db.update_run_status(&sr.id, RunStatus::Finished, Some(0), None);
                    }
                    return Some(branch_name);
                }

                let raw_preview: String = stdout.chars().take(1000).collect();
                tracing::warn!(
                    "Could not parse branch_name JSON from extracted text. \
                     Raw stdout preview: {}",
                    raw_preview,
                );
            }
        }
        Ok(Err(e)) => {
            tracing::error!("Branch-gen agent spawn/execution failed: {}", e);
            if let Ok(ref sr) = sub_run {
                let _ = db.set_run_metadata(&sr.id, &serde_json::json!({
                    "error": format!("{}", e),
                    "error_kind": "spawn_failed",
                }));
            }
        }
        Err(e) => {
            tracing::error!("Branch-gen tokio task panicked or was cancelled: {}", e);
            if let Ok(ref sr) = sub_run {
                let _ = db.set_run_metadata(&sr.id, &serde_json::json!({
                    "error": format!("{}", e),
                    "error_kind": "task_join_failed",
                }));
            }
        }
    }

    if let Ok(ref sr) = sub_run {
        let _ = db.update_run_status(
            &sr.id,
            RunStatus::Error,
            Some(1),
            Some("Failed to generate branch name"),
        );
    }

    None
}

/// Resolve the branch name and create a git worktree for isolated agent execution.
///
/// - First runs (no branch): generates an AI branch name, stores it on the ticket, then creates a worktree.
/// - Subsequent runs: reuses the existing branch via worktree.
///
/// Returns `(Option<WorktreeInfo>, branch_name)`.
pub(super) async fn setup_worktree_and_branch(
    ctx: WorktreeBranchSetup<'_>,
) -> Result<(Option<WorktreeInfo>, String), String> {
    let WorktreeBranchSetup {
        ticket, run_id, repo_path, agent_id,
        provider, db, window, branch_gen_model,
        agent_config,
    } = ctx;

    let ticket_id = &ticket.id;
    let repo_path_buf = std::path::PathBuf::from(repo_path);

    let branch_to_use = if let Some(ref existing_branch) = ticket.branch_name {
        tracing::info!("Ticket {} already has branch: {}", ticket_id, existing_branch);
        existing_branch.clone()
    } else {
        tracing::info!("Ticket {} has no branch yet, generating AI branch name...", ticket_id);

        let ai_branch = generate_ai_branch_name(
            ticket, &repo_path_buf, agent_id,
            provider.clone(), db.clone(),
            branch_gen_model, agent_config,
        ).await;

        let branch = if let Some(name) = ai_branch {
            tracing::info!("AI generated branch name: {}", name);
            name
        } else {
            let fallback = worktree::generate_branch_name(&ticket.id, &ticket.title);
            tracing::warn!("AI branch generation failed, using fallback: {}", fallback);
            fallback
        };

        if let Err(e) = db.set_ticket_branch(ticket_id, &branch) {
            let _ = db.unlock_ticket(ticket_id);
            return Err(format!(
                "Failed to store branch name on ticket: {}. Aborting run to prevent inconsistent state.", e
            ));
        }
        tracing::info!("Stored branch name '{}' on ticket {}", branch, ticket_id);
        let _ = window.emit(
            "ticket-branch-updated",
            serde_json::json!({ "ticketId": ticket_id, "branchName": branch }),
        );

        branch
    };

    tracing::info!("Creating worktree for ticket {} with branch: {}", ticket_id, branch_to_use);

    let worktree = if ticket.branch_name.is_some() {
        match create_worktree_with_existing_branch(&repo_path_buf, &branch_to_use, run_id, None) {
            Ok(info) => {
                tracing::info!(
                    "Created worktree for run {} at {} using existing branch {}",
                    run_id, info.path.display(), info.branch_name
                );
                Some(info)
            }
            Err(e) => {
                tracing::warn!("Failed to create worktree with existing branch, falling back to main repo: {}", e);
                None
            }
        }
    } else {
        match create_worktree(&WorktreeConfig {
            repo_path: repo_path_buf.clone(),
            branch_name: branch_to_use.clone(),
            run_id: run_id.to_string(),
            base_dir: None,
            base_branch: None,
        }) {
            Ok(info) => {
                tracing::info!(
                    "Created new worktree for run {} at {} with new branch {}",
                    run_id, info.path.display(), info.branch_name
                );
                Some(info)
            }
            Err(e) => {
                tracing::warn!("Failed to create new worktree, falling back to main repo: {}", e);
                None
            }
        }
    };

    Ok((worktree, branch_to_use))
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
