use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State, Window};

use crate::agents::{self, cursor, AgentKind, AgentRunConfig, extract_text_from_stream_json};
use crate::agents::spawner::{CancelHandle, run_agent_with_capture};
use crate::agents::orchestrator::{WorkflowOrchestrator, OrchestratorConfig};
use crate::agents::prompt::{generate_branch_name_generation_prompt, parse_branch_name_from_output};
use crate::db::models::{AgentRun, AgentType, CreateRun, RunStatus};
use crate::db::Database;

/// Shared state for tracking running agents
pub struct RunningAgents {
    pub handles: Arc<Mutex<HashMap<String, CancelHandle>>>,
}

impl RunningAgents {
    pub fn new() -> Self {
        Self {
            handles: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for RunningAgents {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the hook script path from app data directory
fn get_hook_script_path(app: &AppHandle) -> Option<String> {
    app.path_resolver()
        .app_data_dir()
        .map(|dir| dir.join("scripts").join("cursor-hook.js"))
        .map(|p| p.to_string_lossy().to_string())
}

/// Update project hooks with run-specific configuration (run_id, api_url, api_token)
/// This ensures the hook script has access to the current run context
fn update_project_hooks_for_run(
    repo_path: &std::path::Path,
    hook_script_path: &str,
    api_url: &str,
    api_token: &str,
    run_id: &str,
    agent_kind: AgentKind,
) -> Result<(), String> {
    tracing::debug!(
        "Updating project hooks: run_id={}, api_url={}, token_prefix={}...",
        run_id,
        api_url,
        &api_token.chars().take(8).collect::<String>()
    );
    
    match agent_kind {
        AgentKind::Cursor => {
            cursor::install_hooks_with_run_id(
                repo_path,
                hook_script_path,
                Some(api_url),
                Some(api_token),
                Some(run_id),
            )
            .map_err(|e| format!("Failed to update Cursor hooks.json: {}", e))
        }
        AgentKind::Claude => {
            agents::claude::install_local_hooks_with_run_id(
                repo_path,
                hook_script_path,
                Some(api_url),
                Some(api_token),
                Some(run_id),
            )
            .map_err(|e| format!("Failed to update Claude settings.local.json: {}", e))
        }
    }
}

/// Generate a branch name using AI via a quick agent call
/// 
/// This runs a quick Claude/Cursor agent call to generate a meaningful branch name
/// based on the ticket's title and description.
async fn generate_ai_branch_name(
    ticket: &crate::db::Ticket,
    repo_path: &std::path::Path,
    agent_kind: AgentKind,
    db: Arc<Database>,
    _window: Option<&Window>,
) -> Option<String> {
    let prompt = generate_branch_name_generation_prompt(ticket);
    let run_id = uuid::Uuid::new_v4().to_string();
    
    tracing::info!("Generating AI branch name for ticket {} via quick agent call", ticket.id);
    
    // Create a temporary sub-run for the branch generation stage
    let sub_run = db.create_run(&CreateRun {
        ticket_id: ticket.id.clone(),
        agent_type: match agent_kind {
            AgentKind::Cursor => AgentType::Cursor,
            AgentKind::Claude => AgentType::Claude,
        },
        repo_path: repo_path.to_string_lossy().to_string(),
        parent_run_id: None,
        stage: Some("branch-gen".to_string()),
    });
    
    if let Err(e) = &sub_run {
        tracing::warn!("Failed to create branch-gen sub-run: {}", e);
    }
    
    // Use agent-appropriate model for branch generation
    // Cursor doesn't recognize Claude model names, so only set model for Claude agent
    let model = match agent_kind {
        AgentKind::Claude => Some("claude-opus-4-5".to_string()),
        AgentKind::Cursor => None, // Let Cursor use its default model
    };
    
    let config = AgentRunConfig {
        kind: agent_kind,
        ticket_id: ticket.id.clone(),
        run_id: run_id.clone(),
        repo_path: repo_path.to_path_buf(),
        prompt: prompt.clone(),
        timeout_secs: Some(60), // Short timeout for branch generation
        api_url: String::new(), // Not needed for branch generation
        api_token: String::new(), // Not needed for branch generation
        model,
    };
    
    // Run synchronously in a blocking task
    let result = tokio::task::spawn_blocking(move || {
        run_agent_with_capture(config, None, None)
    }).await;
    
    match result {
        Ok(Ok(agent_result)) => {
            if let Some(ref stdout) = agent_result.captured_stdout {
                // Extract text from stream-json format if needed
                let text_content = extract_text_from_stream_json(stdout)
                    .unwrap_or_else(|| stdout.clone());
                
                tracing::debug!("Branch-gen output (extracted): {}", text_content);
                
                if let Some(branch_name) = parse_branch_name_from_output(&text_content) {
                    tracing::info!("AI generated branch name: {}", branch_name);
                    
                    // Update sub-run status
                    if let Ok(ref sr) = sub_run {
                        let _ = db.update_run_status(&sr.id, RunStatus::Finished, Some(0), None);
                    }
                    
                    return Some(branch_name);
                }
            }
            tracing::warn!("Could not parse branch name from AI output");
        }
        Ok(Err(e)) => {
            tracing::warn!("Branch generation agent failed: {:?}", e);
        }
        Err(e) => {
            tracing::warn!("Branch generation task failed: {}", e);
        }
    }
    
    // Update sub-run status on failure
    if let Ok(ref sr) = sub_run {
        let _ = db.update_run_status(&sr.id, RunStatus::Error, Some(1), Some("Failed to generate branch name"));
    }
    
    None
}

/// Start a heartbeat task to extend the lock periodically
/// 
/// Returns a task handle that can be aborted when the run completes.
fn start_heartbeat(
    db: Arc<Database>,
    ticket_id: String,
    run_id: String,
    running: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<()> {
    const HEARTBEAT_INTERVAL_SECS: u64 = 60;
    const LOCK_DURATION_MINS: i64 = 30;
    
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(HEARTBEAT_INTERVAL_SECS));
        
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
            
            tracing::debug!("Heartbeat: extended lock for ticket {} until {}", ticket_id, new_expires);
        }
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRunInput {
    pub ticket_id: String,
    pub agent_type: String,
    pub repo_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentLogEvent {
    pub run_id: String,
    pub stream: String,
    pub content: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCompleteEvent {
    pub run_id: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub duration_secs: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentErrorEvent {
    pub run_id: String,
    pub error: String,
}

#[tauri::command]
pub async fn start_agent_run(
    window: Window,
    ticket_id: String,
    agent_type: String,
    repo_path: String,
    db: State<'_, Arc<Database>>,
    running_agents: State<'_, RunningAgents>,
) -> Result<String, String> {
    tracing::info!("=== START_AGENT_RUN CALLED ===");
    tracing::info!("Agent type: {}, Ticket ID: {}, Repo path: {}", agent_type, ticket_id, repo_path);

    let agent_kind = match agent_type.as_str() {
        "cursor" => AgentKind::Cursor,
        "claude" => AgentKind::Claude,
        _ => return Err(format!("Invalid agent type: {}", agent_type)),
    };
    let db_agent_type = match agent_kind {
        AgentKind::Cursor => AgentType::Cursor,
        AgentKind::Claude => AgentType::Claude,
    };

    let ticket = db
        .get_ticket(&ticket_id)
        .map_err(|e| format!("Failed to get ticket: {}", e))?;

    let run = db
        .create_run(&CreateRun {
            ticket_id: ticket_id.clone(),
            agent_type: db_agent_type,
            repo_path: repo_path.clone(),
            parent_run_id: None,
            stage: None,
        })
        .map_err(|e| format!("Failed to create run: {}", e))?;

    let run_id = run.id.clone();
    
    // Lock the ticket with a 30-minute expiration (same as worker default)
    // This ensures the cleanup service can release stale locks
    let lock_expires_at = chrono::Utc::now() + chrono::Duration::minutes(30);
    db.lock_ticket(&ticket_id, &run_id, lock_expires_at)
        .map_err(|e| format!("Failed to lock ticket: {}", e))?;
    tracing::info!("Locked ticket {} with run {} until {}", ticket_id, run_id, lock_expires_at);

    // Get API credentials early - needed for orchestration
    let api_url = std::env::var("AGENT_KANBAN_API_URL")
        .unwrap_or_else(|_| format!("http://127.0.0.1:{}", 
            std::env::var("AGENT_KANBAN_API_PORT").unwrap_or_else(|_| "7432".to_string())
        ));
    let api_token = std::env::var("AGENT_KANBAN_API_TOKEN")
        .unwrap_or_else(|_| "default-token".to_string());
    
    // Create a git worktree for isolated agent execution
    // - First runs (no branch): generate AI branch name first, then create worktree
    // - Subsequent runs: use worktree with existing branch
    let (worktree_info, branch_name) = {
        use agents::worktree::{create_worktree_with_existing_branch, create_worktree, WorktreeConfig, generate_branch_name};
        
        let repo_path_buf = std::path::PathBuf::from(&repo_path);
        
        // Determine the branch to use
        let branch_to_use = if let Some(ref existing_branch) = ticket.branch_name {
            tracing::info!("Ticket {} already has branch: {}", ticket_id, existing_branch);
            existing_branch.clone()
        } else {
            // First run - generate AI branch name
            tracing::info!("Ticket {} has no branch yet, generating AI branch name...", ticket_id);
            
            // Generate branch name using AI
            let ai_branch = generate_ai_branch_name(
                &ticket,
                &repo_path_buf,
                agent_kind,
                db.inner().clone(),
                Some(&window),
            ).await;
            
            let branch = if let Some(name) = ai_branch {
                tracing::info!("AI generated branch name: {}", name);
                name
            } else {
                // Fallback to deterministic naming
                let fallback = generate_branch_name(&ticket.id, &ticket.title);
                tracing::warn!("AI branch generation failed, using fallback: {}", fallback);
                fallback
            };
            
            // Store the branch name on the ticket immediately
            // This is critical - if we fail to store, we must abort to prevent
            // orphaned branches and inconsistent state between DB and git
            if let Err(e) = db.set_ticket_branch(&ticket_id, &branch) {
                // Unlock the ticket before returning error
                let _ = db.unlock_ticket(&ticket_id);
                return Err(format!("Failed to store branch name on ticket: {}. Aborting run to prevent inconsistent state.", e));
            }
            tracing::info!("Stored branch name '{}' on ticket {}", branch, ticket_id);
            // Emit event for frontend to update
            let _ = window.emit("ticket-branch-updated", serde_json::json!({
                "ticketId": ticket_id,
                "branchName": branch,
            }));
            
            branch
        };
        
        // Now create worktree with the branch
        tracing::info!("Creating worktree for ticket {} with branch: {}", ticket_id, branch_to_use);
        
        // Try to create worktree with existing branch first (for subsequent runs)
        let worktree = if ticket.branch_name.is_some() {
            // Branch already exists, use it
            match create_worktree_with_existing_branch(&repo_path_buf, &branch_to_use, &run_id, None) {
                Ok(info) => {
                    tracing::info!(
                        "Created worktree for run {} at {} using existing branch {}",
                        run_id,
                        info.path.display(),
                        info.branch_name
                    );
                    Some(info)
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to create worktree with existing branch, falling back to main repo: {}",
                        e
                    );
                    None
                }
            }
        } else {
            // First run - create new worktree with new branch
            match create_worktree(&WorktreeConfig {
                repo_path: repo_path_buf.clone(),
                branch_name: branch_to_use.clone(),
                run_id: run_id.clone(),
                base_dir: None,
            }) {
                Ok(info) => {
                    tracing::info!(
                        "Created new worktree for run {} at {} with new branch {}",
                        run_id,
                        info.path.display(),
                        info.branch_name
                    );
                    Some(info)
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to create new worktree, falling back to main repo: {}",
                        e
                    );
                    None
                }
            }
        };
        
        (worktree, branch_to_use)
    };
    
    // Track whether worktree (and thus the branch) was successfully created
    let worktree_created = worktree_info.is_some();
    
    // Use worktree path if available, otherwise fall back to main repo
    let working_path = worktree_info
        .as_ref()
        .map(|w| w.path.clone())
        .unwrap_or_else(|| std::path::PathBuf::from(&repo_path));
    let working_path_str = working_path.to_string_lossy().to_string();
    
    tracing::info!("Agent will work in: {} (worktree_created: {})", working_path_str, worktree_created);

    // Get hook script path for updating project hooks
    let app_handle = window.app_handle();
    let hook_script_path = get_hook_script_path(&app_handle);
    
    // Update project hooks with run-specific configuration
    // This ensures the hook script receives AGENT_KANBAN_RUN_ID and API credentials
    // Install hooks in the working directory (worktree or main repo)
    if let Some(ref hook_path) = hook_script_path {
        if let Err(e) = update_project_hooks_for_run(
            &working_path,
            hook_path,
            &api_url,
            &api_token,
            &run_id,
            agent_kind,
        ) {
            tracing::warn!("Failed to update project hooks: {}", e);
            // Continue anyway - hooks might already be configured or not needed
        }
    } else {
        tracing::warn!("Could not determine hook script path, hooks may not track this run");
    }

    // All tickets now use multi-stage workflow
    tracing::info!("Workflow type: {:?}, run_id: {}, working_path: {}", ticket.workflow_type, run_id, working_path.display());

    let db_clone = db.inner().clone();
    let run_id_for_task = run_id.clone();
    let ticket_id_for_task = ticket_id.clone();
    let window_clone = window.clone();
    
    // Store original repo path for worktree cleanup
    let main_repo_path = std::path::PathBuf::from(&repo_path);

    // Clone the Arc<Mutex<HashMap>> so we can move it into the async task
    let running_agents_handles = running_agents.handles.clone();

    // Execute multi-stage workflow
    {
        let ticket_for_orchestrator = ticket.clone();
        let api_url_for_orchestrator = api_url.clone();
        let api_token_for_orchestrator = api_token.clone();
        let hook_script_path_for_orchestrator = hook_script_path.clone();
        let cancel_handles_for_orchestrator = running_agents_handles.clone();
        let worktree_for_cleanup = worktree_info.clone();
        let main_repo_for_cleanup = main_repo_path.clone();
        let branch_name_for_orchestrator = branch_name.clone();
        let db_for_heartbeat = db.inner().clone();
        let ticket_id_for_heartbeat = ticket_id.clone();
        let run_id_for_heartbeat = run_id.clone();
        
        tauri::async_runtime::spawn(async move {
            if let Err(e) = db_clone.update_run_status(&run_id_for_task, RunStatus::Running, None, None) {
                tracing::error!("Failed to update run status: {}", e);
            }

            // Start heartbeat to keep the lock alive during long-running workflows
            let running_flag = Arc::new(AtomicBool::new(true));
            let heartbeat_handle = start_heartbeat(
                db_for_heartbeat,
                ticket_id_for_heartbeat,
                run_id_for_heartbeat,
                running_flag.clone(),
            );
            tracing::info!("Started heartbeat for run {}", run_id_for_task);

            // Clone for cleanup after orchestrator takes ownership
            let cancel_handles_for_cleanup = cancel_handles_for_orchestrator.clone();
            
            // Use the working path (worktree if created, otherwise main repo)
            let orchestrator_working_path = worktree_for_cleanup
                .as_ref()
                .map(|w| w.path.clone())
                .unwrap_or_else(|| main_repo_for_cleanup.clone());
            
            // Always pass the branch name to skip branch name generation
            // The branch name was already generated (via AI or fallback) and stored in DB
            // The `branch_already_created` flag tells orchestrator if it needs to create the branch
            //
            // Branch already created if:
            // - Worktree was created (branch created/attached via worktree), OR
            // - Ticket already had a branch name from a previous run (branch exists in git)
            //
            // Note: We check the original ticket.branch_name, NOT the newly generated one,
            // because ticket.branch_name reflects whether the branch existed BEFORE this run.
            let worktree_branch = Some(branch_name_for_orchestrator);
            let branch_already_created = worktree_for_cleanup.is_some() || ticket_for_orchestrator.branch_name.is_some();
            
            tracing::info!(
                "Orchestrator config: worktree_branch={:?}, branch_already_created={}",
                worktree_branch, branch_already_created
            );
            
            // Get the next pending task for this ticket (for task-based workflow)
            let task = db_clone.get_next_pending_task(&ticket_for_orchestrator.id).ok().flatten();
            
            // Mark task as in progress if we found one - CRITICAL: must succeed before continuing
            // If this fails (e.g., task is not pending, already claimed by another run),
            // we must abort to prevent complete_task/fail_task from failing later
            // (they require status = 'in_progress')
            if let Some(ref t) = task {
                if let Err(e) = db_clone.start_task(&t.id, &run_id_for_task) {
                    tracing::error!(
                        "Failed to mark task {} as in_progress: {}. Aborting run to prevent stuck task.",
                        t.id, e
                    );
                    // Update run status to Error and emit error event
                    let _ = db_clone.update_run_status(
                        &run_id_for_task,
                        RunStatus::Error,
                        None,
                        Some(&format!("Failed to start task: {}", e)),
                    );
                    let _ = db_clone.unlock_ticket(&ticket_id_for_task);
                    
                    // Clean up worktree if created
                    if let Some(ref worktree) = worktree_for_cleanup {
                        use agents::worktree::remove_worktree;
                        let _ = remove_worktree(&worktree.path, &main_repo_for_cleanup);
                    }
                    
                    let event = AgentErrorEvent {
                        run_id: run_id_for_task.clone(),
                        error: format!("Failed to start task: {}", e),
                    };
                    let _ = window_clone.emit("agent-error", &event);
                    return;
                }
            }
            
            let orchestrator = WorkflowOrchestrator::new(OrchestratorConfig {
                db: db_clone.clone(),
                window: Some(window_clone.clone()),
                app_handle: None, // Direct runs use Window for event emission
                parent_run_id: run_id_for_task.clone(),
                ticket: ticket_for_orchestrator.clone(),
                task: task.clone(),
                repo_path: orchestrator_working_path,
                agent_kind,
                api_url: api_url_for_orchestrator,
                api_token: api_token_for_orchestrator,
                hook_script_path: hook_script_path_for_orchestrator,
                cancel_handles: cancel_handles_for_orchestrator,
                worktree_branch,
                branch_already_created,
            });

            // Execute workflow - log callbacks are handled per-stage with correct sub-run IDs
            tracing::info!("Starting multi-stage workflow execution for run {}", run_id_for_task);
            let start_time = std::time::Instant::now();
            let result = orchestrator.execute().await;
            let duration_secs = start_time.elapsed().as_secs_f64();
            
            tracing::info!("Multi-stage workflow execution completed for run {} in {:.1}s, result: {:?}", 
                run_id_for_task, duration_secs, result.is_ok());

            // Stop heartbeat
            running_flag.store(false, Ordering::SeqCst);
            heartbeat_handle.abort();
            tracing::info!("Stopped heartbeat for run {}", run_id_for_task);

            // Clean up cancel handles for the parent run
            {
                let mut handles = cancel_handles_for_cleanup.lock().expect("cancel handles mutex poisoned");
                handles.remove(&run_id_for_task);
            }

            // Update parent run status based on result
            match result {
                Ok(()) => {
                    tracing::info!("Updating parent run {} status to Finished", run_id_for_task);
                    if let Err(e) = db_clone.update_run_status(
                        &run_id_for_task,
                        RunStatus::Finished,
                        Some(0),
                        Some("Multi-stage workflow completed successfully"),
                    ) {
                        tracing::error!("Failed to update run status to Finished: {}", e);
                    } else {
                        tracing::info!("Successfully updated parent run {} status to Finished", run_id_for_task);
                    }
                    
                    // Mark task as completed
                    if let Some(ref t) = task {
                        if let Err(e) = db_clone.complete_task(&t.id) {
                            tracing::warn!("Failed to mark task {} as completed: {}", t.id, e);
                        }
                    }
                    
                    let event = AgentCompleteEvent {
                        run_id: run_id_for_task.clone(),
                        status: "finished".to_string(),
                        exit_code: Some(0),
                        duration_secs,
                    };
                    if let Err(e) = window_clone.emit("agent-complete", &event) {
                        tracing::error!("Failed to emit agent-complete event: {}", e);
                    } else {
                        tracing::info!("Emitted agent-complete event for run {}", run_id_for_task);
                    }
                }
                Err(e) => {
                    tracing::error!("Multi-stage workflow failed for run {}: {}", run_id_for_task, e);
                    if let Err(db_err) = db_clone.update_run_status(
                        &run_id_for_task,
                        RunStatus::Error,
                        None,
                        Some(&format!("Multi-stage workflow failed: {}", e)),
                    ) {
                        tracing::error!("Failed to update run status to Error: {}", db_err);
                    }
                    
                    // Mark task as failed
                    if let Some(ref t) = task {
                        if let Err(fail_err) = db_clone.fail_task(&t.id) {
                            tracing::warn!("Failed to mark task {} as failed: {}", t.id, fail_err);
                        }
                    }
                    
                    let event = AgentErrorEvent {
                        run_id: run_id_for_task.clone(),
                        error: e,
                    };
                    if let Err(emit_err) = window_clone.emit("agent-error", &event) {
                        tracing::error!("Failed to emit agent-error event: {}", emit_err);
                    }
                }
            }

            // Unlock the ticket
            tracing::info!("Unlocking ticket {} after multi-stage workflow", ticket_id_for_task);
            if let Err(e) = db_clone.unlock_ticket(&ticket_id_for_task) {
                tracing::error!("Failed to unlock ticket: {}", e);
            } else {
                tracing::info!("Successfully unlocked ticket {}", ticket_id_for_task);
            }
            
            // Clean up the worktree if we created one
            if let Some(ref worktree) = worktree_for_cleanup {
                use agents::worktree::remove_worktree;
                if let Err(e) = remove_worktree(&worktree.path, &main_repo_for_cleanup) {
                    tracing::error!("Failed to remove worktree {}: {}", worktree.path.display(), e);
                } else {
                    tracing::info!("Removed worktree at {}", worktree.path.display());
                }
            }
        });
    }

    Ok(run_id)
}

#[tauri::command]
pub async fn cancel_agent_run(
    run_id: String,
    db: State<'_, Arc<Database>>,
    running_agents: State<'_, RunningAgents>,
) -> Result<(), String> {
    tracing::info!("Cancelling agent run: {}", run_id);

    // Try to cancel via handle
    if let Some(handle) = running_agents
        .handles
        .lock()
        .expect("running agents mutex poisoned")
        .get(&run_id)
    {
        handle.cancel();
    }

    // Update the status in the database
    db.update_run_status(&run_id, RunStatus::Aborted, None, Some("Cancelled by user"))
        .map_err(|e| e.to_string())?;
    
    // Also unlock any ticket that was locked by this run
    // We need to find the ticket first
    if let Ok(run) = db.get_run(&run_id) {
        if let Err(e) = db.unlock_ticket(&run.ticket_id) {
            tracing::warn!("Failed to unlock ticket after cancel: {}", e);
        }
    }
    
    Ok(())
}

/// Clean up stale runs that are stuck in "Running" status
/// This is useful for runs that crashed or were interrupted without proper cleanup
#[tauri::command]
pub async fn cleanup_stale_runs(
    db: State<'_, Arc<Database>>,
) -> Result<u32, String> {
    tracing::info!("Cleaning up stale runs");
    
    // Find all runs with status "running" and mark them as aborted
    let count = db.cleanup_stale_running_status()
        .map_err(|e| format!("Failed to cleanup stale runs: {}", e))?;
    
    tracing::info!("Cleaned up {} stale runs", count);
    Ok(count)
}

#[tauri::command]
pub async fn get_agent_runs(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<AgentRun>, String> {
    tracing::info!("Getting agent runs for ticket: {}", ticket_id);
    db.get_runs(&ticket_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_recent_runs(
    limit: Option<u32>,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<AgentRun>, String> {
    let limit = limit.unwrap_or(50);
    tracing::info!("Getting recent {} agent runs", limit);
    db.get_recent_runs(limit).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_agent_run(run_id: String, db: State<'_, Arc<Database>>) -> Result<AgentRun, String> {
    tracing::info!("Getting agent run: {}", run_id);
    db.get_run(&run_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_run_events(
    run_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<crate::db::AgentEvent>, String> {
    tracing::info!("Getting events for run: {}", run_id);
    db.get_events(&run_id).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_agents_new_is_empty() {
        let ra = RunningAgents::new();
        assert!(ra.handles.lock().unwrap().is_empty());
    }

    #[test]
    fn agent_log_event_serializes() {
        let event = AgentLogEvent {
            run_id: "run-1".to_string(),
            stream: "stdout".to_string(),
            content: "Hello".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("runId"));
        assert!(json.contains("stdout"));
    }

    #[test]
    fn agent_complete_event_serializes() {
        let event = AgentCompleteEvent {
            run_id: "run-1".to_string(),
            status: "success".to_string(),
            exit_code: Some(0),
            duration_secs: 123.45,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("durationSecs"));
        assert!(json.contains("exitCode"));
    }

    #[test]
    fn running_agents_default_same_as_new() {
        let default = RunningAgents::default();
        let new = RunningAgents::new();
        assert!(default.handles.lock().unwrap().is_empty());
        assert!(new.handles.lock().unwrap().is_empty());
    }

    #[test]
    fn agent_error_event_serializes() {
        let event = AgentErrorEvent {
            run_id: "run-1".to_string(),
            error: "Something went wrong".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("runId"));
        assert!(json.contains("error"));
        assert!(json.contains("Something went wrong"));
    }

    #[test]
    fn start_run_input_deserializes() {
        let json = r#"{"ticketId":"t1","agentType":"cursor","repoPath":"/tmp"}"#;
        let input: StartRunInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.ticket_id, "t1");
        assert_eq!(input.agent_type, "cursor");
        assert_eq!(input.repo_path, "/tmp");
    }

    #[test]
    fn agent_complete_event_null_exit_code() {
        let event = AgentCompleteEvent {
            run_id: "run-1".to_string(),
            status: "timeout".to_string(),
            exit_code: None,
            duration_secs: 3600.0,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"exitCode\":null"));
    }

}
