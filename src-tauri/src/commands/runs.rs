use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State, Window};

use crate::agents::{self, cursor, AgentKind, AgentRunConfig, LogLine, RunOutcome, extract_text_from_stream_json};
use crate::agents::spawner::CancelHandle;
use crate::agents::orchestrator::{WorkflowOrchestrator, OrchestratorConfig};
use crate::db::models::{AgentRun, AgentType, CreateRun, RunStatus, EventType, NormalizedEvent, AgentEventPayload, WorkflowType, CreateComment, AuthorType};
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

fn extract_agent_summary(output: &str, agent_kind: AgentKind) -> Option<String> {
    let summary = match agent_kind {
        AgentKind::Claude => extract_text_from_stream_json(output),
        AgentKind::Cursor => extract_cursor_summary(output),
    };
    
    summary.map(|s| {
        let s = s.trim().to_string();
        if s.len() > 4000 {
            format!("{}...\n\n(truncated)", &s[..4000])
        } else {
            s
        }
    })
}

fn extract_cursor_summary(output: &str) -> Option<String> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return None;
    }
    
    if trimmed.len() <= 2000 {
        return Some(trimmed.to_string());
    }
    
    let start = trimmed.len().saturating_sub(2000);
    Some(format!("...\n{}", &trimmed[start..]))
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

    // Create a git worktree for isolated agent execution
    // This allows multiple agents to work on the same project in parallel
    let worktree_info = {
        use agents::worktree::{create_worktree, WorktreeConfig, generate_branch_name};
        
        let branch_name = generate_branch_name(&ticket_id, &ticket.title);
        let config = WorktreeConfig {
            repo_path: std::path::PathBuf::from(&repo_path),
            branch_name: branch_name.clone(),
            run_id: run_id.clone(),
            base_dir: None, // Use default temp directory
        };
        
        match create_worktree(&config) {
            Ok(info) => {
                tracing::info!(
                    "Created worktree for run {} at {} on branch {}",
                    run_id,
                    info.path.display(),
                    info.branch_name
                );
                Some(info)
            }
            Err(e) => {
                // If worktree creation fails, fall back to using the main repo
                // This handles non-git directories or other edge cases
                tracing::warn!(
                    "Failed to create worktree, falling back to main repo: {}",
                    e
                );
                None
            }
        }
    };
    
    // Use worktree path if available, otherwise fall back to main repo
    let working_path = worktree_info
        .as_ref()
        .map(|w| w.path.clone())
        .unwrap_or_else(|| std::path::PathBuf::from(&repo_path));
    let working_path_str = working_path.to_string_lossy().to_string();
    
    tracing::info!("Agent will work in: {}", working_path_str);

    let api_url = std::env::var("AGENT_KANBAN_API_URL")
        .unwrap_or_else(|_| format!("http://127.0.0.1:{}", 
            std::env::var("AGENT_KANBAN_API_PORT").unwrap_or_else(|_| "7432".to_string())
        ));
    let api_token = std::env::var("AGENT_KANBAN_API_TOKEN")
        .unwrap_or_else(|_| "default-token".to_string());

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

    // Branch on workflow type
    let is_multi_stage = ticket.workflow_type == WorkflowType::MultiStage;
    tracing::info!("Workflow type: {:?}, multi-stage: {}", ticket.workflow_type, is_multi_stage);

    let prompt = agents::prompt::generate_ticket_prompt_with_workflow(&ticket, Some(agent_kind));
    tracing::info!("Generated prompt with workflow for {} agent", agent_type);
    tracing::debug!("Prompt (first 500 chars): {}", &prompt.chars().take(500).collect::<String>());

    let config = AgentRunConfig {
        kind: agent_kind,
        ticket_id: ticket_id.clone(),
        run_id: run_id.clone(),
        repo_path: working_path.clone(),  // Use worktree path
        prompt: prompt.clone(),
        timeout_secs: Some(3600), // 1 hour default
        api_url: api_url.clone(),
        api_token: api_token.clone(),
        model: ticket.model.clone(),
    };
    
    tracing::info!("Agent config created - run_id: {}, working_path: {}, model: {:?}", run_id, working_path.display(), ticket.model);

    let db_clone = db.inner().clone();
    let run_id_for_task = run_id.clone();
    let run_id_for_cleanup = run_id.clone();
    let ticket_id_for_task = ticket_id.clone();
    let agent_kind_for_task = agent_kind;
    let db_agent_type_for_task = db_agent_type;
    let window_clone = window.clone();
    
    // Store original repo path for worktree cleanup
    let main_repo_path = std::path::PathBuf::from(&repo_path);

    // Clone the Arc<Mutex<HashMap>> so we can move it into the async task
    let running_agents_handles = running_agents.handles.clone();

    // Handle multi-stage workflow separately
    if is_multi_stage {
        let ticket_for_orchestrator = ticket.clone();
        let api_url_for_orchestrator = api_url.clone();
        let api_token_for_orchestrator = api_token.clone();
        let hook_script_path_for_orchestrator = hook_script_path.clone();
        let cancel_handles_for_orchestrator = running_agents_handles.clone();
        let worktree_for_cleanup = worktree_info.clone();
        let main_repo_for_cleanup = main_repo_path.clone();
        
        tauri::async_runtime::spawn(async move {
            if let Err(e) = db_clone.update_run_status(&run_id_for_task, RunStatus::Running, None, None) {
                tracing::error!("Failed to update run status: {}", e);
            }

            // Clone for cleanup after orchestrator takes ownership
            let cancel_handles_for_cleanup = cancel_handles_for_orchestrator.clone();
            
            // Use the working path (worktree if created, otherwise main repo)
            let orchestrator_working_path = worktree_for_cleanup
                .as_ref()
                .map(|w| w.path.clone())
                .unwrap_or_else(|| main_repo_for_cleanup.clone());
            
            let orchestrator = WorkflowOrchestrator::new(OrchestratorConfig {
                db: db_clone.clone(),
                window: Some(window_clone.clone()),
                parent_run_id: run_id_for_task.clone(),
                ticket: ticket_for_orchestrator,
                repo_path: orchestrator_working_path,
                agent_kind,
                api_url: api_url_for_orchestrator,
                api_token: api_token_for_orchestrator,
                hook_script_path: hook_script_path_for_orchestrator,
                cancel_handles: cancel_handles_for_orchestrator,
                worktree_branch: worktree_for_cleanup.as_ref().map(|w| w.branch_name.clone()),
            });

            // Execute workflow - log callbacks are handled per-stage with correct sub-run IDs
            tracing::info!("Starting multi-stage workflow execution for run {}", run_id_for_task);
            let start_time = std::time::Instant::now();
            let result = orchestrator.execute().await;
            let duration_secs = start_time.elapsed().as_secs_f64();
            
            tracing::info!("Multi-stage workflow execution completed for run {} in {:.1}s, result: {:?}", 
                run_id_for_task, duration_secs, result.is_ok());

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

        return Ok(run_id);
    }

    // Basic workflow (original implementation)
    // Clone worktree info for cleanup
    let worktree_for_single_cleanup = worktree_info;
    let main_repo_for_single_cleanup = main_repo_path;
    
    tauri::async_runtime::spawn(async move {
        if let Err(e) = db_clone.update_run_status(&run_id_for_task, RunStatus::Running, None, None)
        {
            tracing::error!("Failed to update run status: {}", e);
        }

        let window_for_logs = window_clone.clone();
        let run_id_for_logs = run_id_for_task.clone();
        let ticket_id_for_logs = ticket_id_for_task.clone();
        let db_for_logs = db_clone.clone();
        let agent_type_for_logs = db_agent_type_for_task;
        // Track if we've already commented a branch (to avoid duplicates)
        let branch_commented = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let branch_commented_clone = branch_commented.clone();
        let db_for_branch = db_clone.clone();
        let window_for_branch = window_clone.clone();
        let ticket_id_for_branch = ticket_id_for_task.clone();
        
        let on_log: Arc<agents::LogCallback> = Arc::new(Box::new(move |log: LogLine| {
            let stream_name = match log.stream {
                agents::LogStream::Stdout => "stdout",
                agents::LogStream::Stderr => "stderr",
            };
            tracing::debug!("LOG CALLBACK: [{}] {} - content length: {}", 
                stream_name,
                run_id_for_logs,
                log.content.len()
            );
            
            // Check for branch creation patterns (only once)
            if !branch_commented_clone.load(std::sync::atomic::Ordering::Relaxed) {
                let branch_patterns = [
                    r#"(?i)switched to (?:a )?new branch ['\"]?([^\s'\"]+)"#,
                    r#"(?i)created branch[:\s]+['\"]?([^\s'\"]+)"#,
                    r#"(?i)checkout -b ['\"]?([^\s'\"]+)"#,
                    r#"(?i)branch ['\"]?([^\s'\"]+)['\"]? created"#,
                ];
                
                for pattern in &branch_patterns {
                    if let Ok(re) = regex::Regex::new(pattern) {
                        if let Some(caps) = re.captures(&log.content) {
                            if let Some(branch_name) = caps.get(1) {
                                if branch_commented_clone.compare_exchange(
                                    false, true,
                                    std::sync::atomic::Ordering::SeqCst,
                                    std::sync::atomic::Ordering::Relaxed
                                ).is_ok() {
                                    let comment_text = format!("Branch created: `{}`", branch_name.as_str());
                                    let create_comment = CreateComment {
                                        ticket_id: ticket_id_for_branch.clone(),
                                        author_type: AuthorType::System,
                                        body_md: comment_text.clone(),
                                        metadata: Some(serde_json::json!({
                                            "type": "branch_created",
                                            "branch": branch_name.as_str(),
                                        })),
                                    };
                                    if let Err(e) = db_for_branch.create_comment(&create_comment) {
                                        tracing::warn!("Failed to add branch comment: {}", e);
                                    } else {
                                        tracing::info!("Added branch comment for ticket {}: {}", 
                                            ticket_id_for_branch, branch_name.as_str());
                                        // Emit event for frontend
                                        let _ = window_for_branch.emit("ticket-comment-added", serde_json::json!({
                                            "ticketId": ticket_id_for_branch,
                                            "comment": comment_text,
                                        }));
                                    }
                                }
                                break;
                            }
                        }
                    }
                }
            }
            
            // Store log to database
            let normalized_event = NormalizedEvent {
                run_id: run_id_for_logs.clone(),
                ticket_id: ticket_id_for_logs.clone(),
                agent_type: agent_type_for_logs,
                event_type: EventType::Custom(format!("log_{}", stream_name)),
                payload: AgentEventPayload {
                    raw: Some(log.content.clone()),
                    structured: None,
                },
                timestamp: log.timestamp,
            };
            if let Err(e) = db_for_logs.create_event(&normalized_event) {
                tracing::error!("Failed to persist log event: {}", e);
            }
            
            // Emit to frontend for real-time display
            let event = AgentLogEvent {
                run_id: run_id_for_logs.clone(),
                stream: stream_name.to_string(),
                content: log.content,
                timestamp: log.timestamp.to_rfc3339(),
            };
            if let Err(e) = window_for_logs.emit("agent-log", event) {
                tracing::error!("Failed to emit agent-log event: {}", e);
            }
        }));

        let config_clone = config.clone();
        let on_log_clone = on_log.clone();

        // Clone handles for use in the spawn callback
        let handles_for_spawn = running_agents_handles.clone();
        let run_id_for_spawn = run_id_for_task.clone();

        // Create the callback to store the cancel handle when the process spawns
        let on_spawn: agents::spawner::OnSpawnCallback = Box::new(move |cancel_handle| {
            tracing::info!("SPAWN CALLBACK: Agent process spawned for run {}", run_id_for_spawn);
            handles_for_spawn
                .lock()
                .expect("running agents mutex poisoned")
                .insert(run_id_for_spawn.clone(), cancel_handle);
        });
        
        tracing::info!("Calling run_agent_with_cancel_callback for run: {}", run_id_for_task);

        let result = tokio::task::spawn_blocking(move || {
            agents::spawner::run_agent_with_cancel_callback(
                config_clone,
                Some(on_log_clone),
                Some(on_spawn),
            )
        })
        .await;

        // Clean up the cancel handle now that the run is complete
        running_agents_handles
            .lock()
            .expect("running agents mutex poisoned")
            .remove(&run_id_for_cleanup);

        match result {
            Ok(Ok(agent_result)) => {
                let status = match agent_result.status {
                    RunOutcome::Success => RunStatus::Finished,
                    RunOutcome::Error => RunStatus::Error,
                    RunOutcome::Timeout => RunStatus::Error,
                    RunOutcome::Cancelled => RunStatus::Aborted,
                };
                let status_str = status.as_str().to_string();

                if let Err(e) = db_clone.update_run_status(
                    &agent_result.run_id,
                    status.clone(),
                    agent_result.exit_code,
                    agent_result.summary.as_deref(),
                ) {
                    tracing::error!("Failed to update run status: {}", e);
                }
                
                // Extract summary from captured output and create a comment
                if let Some(ref captured_stdout) = agent_result.captured_stdout {
                    if let Some(summary_text) = extract_agent_summary(captured_stdout, agent_kind_for_task) {
                        let comment = CreateComment {
                            ticket_id: ticket_id_for_task.clone(),
                            author_type: AuthorType::Agent,
                            body_md: format!("## Agent Summary\n\n{}", summary_text),
                            metadata: Some(serde_json::json!({
                                "type": "agent_summary",
                                "run_id": agent_result.run_id,
                                "status": status_str,
                            })),
                        };
                        if let Err(e) = db_clone.create_comment(&comment) {
                            tracing::warn!("Failed to create agent summary comment: {}", e);
                        } else {
                            tracing::info!("Created agent summary comment for ticket {}", ticket_id_for_task);
                        }
                    }
                }
                
                // Unlock the ticket now that the run is complete
                if let Err(e) = db_clone.unlock_ticket(&ticket_id_for_task) {
                    tracing::error!("Failed to unlock ticket: {}", e);
                }

                let event = AgentCompleteEvent {
                    run_id: agent_result.run_id,
                    status: status_str,
                    exit_code: agent_result.exit_code,
                    duration_secs: agent_result.duration_secs,
                };
                let _ = window_clone.emit("agent-complete", event);
            }
            Ok(Err(e)) => {
                tracing::error!("Agent run failed: {}", e);
                if let Err(db_err) = db_clone.update_run_status(
                    &run_id_for_task,
                    RunStatus::Error,
                    None,
                    Some(&e.to_string()),
                ) {
                    tracing::error!("Failed to update run status: {}", db_err);
                }
                
                // Unlock the ticket on error
                if let Err(unlock_err) = db_clone.unlock_ticket(&ticket_id_for_task) {
                    tracing::error!("Failed to unlock ticket: {}", unlock_err);
                }

                let event = AgentErrorEvent {
                    run_id: run_id_for_task,
                    error: e.to_string(),
                };
                let _ = window_clone.emit("agent-error", event);
            }
            Err(e) => {
                tracing::error!("Task join error: {}", e);
                if let Err(db_err) = db_clone.update_run_status(
                    &run_id_for_task,
                    RunStatus::Error,
                    None,
                    Some(&e.to_string()),
                ) {
                    tracing::error!("Failed to update run status: {}", db_err);
                }
                
                // Unlock the ticket on task join error
                if let Err(unlock_err) = db_clone.unlock_ticket(&ticket_id_for_task) {
                    tracing::error!("Failed to unlock ticket: {}", unlock_err);
                }

                let event = AgentErrorEvent {
                    run_id: run_id_for_task,
                    error: e.to_string(),
                };
                let _ = window_clone.emit("agent-error", event);
            }
        }
        
        // Clean up the worktree if we created one
        if let Some(ref worktree) = worktree_for_single_cleanup {
            use agents::worktree::remove_worktree;
            if let Err(e) = remove_worktree(&worktree.path, &main_repo_for_single_cleanup) {
                tracing::error!("Failed to remove worktree {}: {}", worktree.path.display(), e);
            } else {
                tracing::info!("Removed worktree at {}", worktree.path.display());
            }
        }
    });

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
