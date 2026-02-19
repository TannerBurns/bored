//! Tauri commands for validation sessions and messages

use std::path::Path;
use std::sync::Arc;
use tauri::State;
use tokio::sync::broadcast;

use super::validation_parsing::{
    parse_create_fix_tasks_from_response, parse_run_command_from_response,
    parse_start_app_from_response,
};
use crate::agents::validation_agent::{AppProcessManager, StartResult};
use crate::agents::AgentRegistry;
use crate::api::state::LiveEvent;
use crate::commands::agent_settings::AgentSettingsManager;
use crate::commands::next_steps::{get_branch_diff_sync, get_ticket_working_dir};
use crate::commands::ApiConnState;
use crate::db::models::{
    CreateValidationMessage, CreateValidationSession, FixTask, TaskStatus, ValidationMessage,
    ValidationMessageRole, ValidationSession, ValidationSessionStatus,
};
use crate::db::Database;

/// Helper: get a follow-up response from the agent, save it, process fix tasks, and return the
/// response text + saved message + any created fix-task IDs.
async fn send_agent_followup(
    agent: &crate::agents::validation_agent::ValidationAgent,
    db: &Arc<Database>,
    event_tx: &broadcast::Sender<LiveEvent>,
    session_id: &str,
    session: &ValidationSession,
    ticket: &crate::db::models::Ticket,
) -> Result<(String, ValidationMessage, Vec<String>), String> {
    let msgs = db.get_validation_messages(session_id).map_err(|e| e.to_string())?;
    let next_response = agent.process_message(&msgs).await.map_err(|e| {
        tracing::error!("Validation agent follow-up error: {}", e);
        e
    })?;

    let has_fix = parse_create_fix_tasks_from_response(&next_response).is_some();
    let meta = if has_fix {
        Some(serde_json::json!({ "type": "fix_task_response" }))
    } else {
        None
    };

    let next_msg = db.create_validation_message(&CreateValidationMessage {
        session_id: session_id.to_string(),
        role: ValidationMessageRole::Assistant,
        content: next_response.clone(),
        metadata: meta,
    }).map_err(|e| e.to_string())?;
    let _ = event_tx.send(LiveEvent::ValidationMessageAdded {
        session_id: session_id.to_string(),
        message_id: next_msg.id.clone(),
        role: "assistant".to_string(),
    });

    let fix_task_ids = process_fix_tasks_in_response(
        &next_response, db, session_id, &session.ticket_id,
        &ticket.title, &ticket.board_id, event_tx,
    );

    Ok((next_response, next_msg, fix_task_ids))
}

/// Process any create_fix_task blocks found in an agent response.
/// Creates tasks in the DB, updates session status, moves ticket to Ready, and emits events.
/// Returns the IDs of any tasks that were created.
fn process_fix_tasks_in_response(
    response_text: &str,
    db: &Arc<Database>,
    session_id: &str,
    ticket_id: &str,
    ticket_title: &str,
    ticket_board_id: &str,
    event_tx: &broadcast::Sender<LiveEvent>,
) -> Vec<String> {
    let fix_block = match parse_create_fix_tasks_from_response(response_text) {
        Some(block) => block,
        None => return Vec::new(),
    };

    let mut task_ids = Vec::new();
    for fix_task in &fix_block.tasks {
        let mut description = fix_task.description.clone();
        if let Some(ref criteria) = fix_task.acceptance_criteria {
            description.push_str("\n\n## Acceptance Criteria\n");
            for criterion in criteria {
                description.push_str(&format!("- {}\n", criterion));
            }
        }
        if let Ok(task) = db.create_task(&crate::db::models::CreateTask {
            ticket_id: ticket_id.to_string(),
            task_type: crate::db::models::TaskType::Custom,
            title: Some(fix_task.title.clone()),
            content: Some(description),
        }) {
            task_ids.push(task.id);
        }
    }

    if !task_ids.is_empty() {
        let _ = db.update_validation_session_status(
            session_id,
            &ValidationSessionStatus::Failed,
        );
        let task_summary = fix_block
            .tasks
            .iter()
            .map(|t| format!("- {}", t.title))
            .collect::<Vec<_>>()
            .join("\n");
        let _ = db.create_validation_message(&CreateValidationMessage {
            session_id: session_id.to_string(),
            role: ValidationMessageRole::System,
            content: format!(
                "Fix tasks created for ticket **{}**:\n{}\n\nA worker agent will pick these up. You'll be notified when the work completes.",
                ticket_title, task_summary
            ),
            metadata: Some(serde_json::json!({
                "type": "fix_tasks_created",
                "task_ids": task_ids,
            })),
        });
        if let Ok(columns) = db.get_columns(ticket_board_id) {
            if let Some(ready_col) = columns.iter().find(|c| c.name == "Ready") {
                let _ = db.move_ticket(ticket_id, &ready_col.id);
            }
        }
        let _ = event_tx.send(LiveEvent::ValidationFixTasksCreated {
            session_id: session_id.to_string(),
            ticket_id: ticket_id.to_string(),
            task_count: task_ids.len(),
        });
    }

    task_ids
}

/// Poll the DB until all tasks in `task_ids` reach a terminal state (completed/failed),
/// emitting progress log events so the UI keeps showing the thinking indicator.
async fn wait_for_fix_tasks(
    task_ids: &[String],
    db: &Arc<Database>,
    event_tx: &broadcast::Sender<LiveEvent>,
    session_id: &str,
) {
    if task_ids.is_empty() {
        return;
    }

    const POLL_INTERVAL_SECS: u64 = 5;
    const MAX_WAIT_SECS: u64 = 30 * 60;
    let timeout = std::time::Duration::from_secs(MAX_WAIT_SECS);
    let start = std::time::Instant::now();

    let _ = event_tx.send(LiveEvent::ValidationLogEntry {
        session_id: session_id.to_string(),
        stream: "stdout".to_string(),
        message: "Waiting for worker agent to complete fix tasks...".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    });

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;

        let mut completed = 0usize;
        let mut failed = 0usize;
        let mut still_pending = 0usize;

        for id in task_ids {
            match db.get_task(id) {
                Ok(task) => match task.status {
                    TaskStatus::Completed => completed += 1,
                    TaskStatus::Failed => failed += 1,
                    _ => still_pending += 1,
                },
                Err(_) => completed += 1,
            }
        }

        if still_pending == 0 {
            break;
        }

        if start.elapsed() > timeout {
            tracing::warn!(
                "Timed out waiting for fix tasks in session {} ({} still pending)",
                session_id, still_pending
            );
            break;
        }

        let _ = event_tx.send(LiveEvent::ValidationLogEntry {
            session_id: session_id.to_string(),
            stream: "stdout".to_string(),
            message: format!(
                "Fix tasks: {} completed, {} failed, {} in progress",
                completed, failed, still_pending
            ),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }
}

/// After fix tasks finish, post a system message summarizing the outcome
/// and asking the user about next steps (without auto-starting the app).
fn post_fix_tasks_completion_message(
    task_ids: &[String],
    db: &Arc<Database>,
    event_tx: &broadcast::Sender<LiveEvent>,
    session_id: &str,
) -> Option<ValidationMessage> {
    if task_ids.is_empty() {
        return None;
    }

    let mut completed = 0usize;
    let mut failed = 0usize;
    for id in task_ids {
        if let Ok(task) = db.get_task(id) {
            match task.status {
                TaskStatus::Completed => completed += 1,
                TaskStatus::Failed => failed += 1,
                _ => {}
            }
        }
    }

    let content = if failed == 0 && completed > 0 {
        format!(
            "All {} fix task(s) completed successfully. Would you like to re-validate the changes, or is there anything else to check?",
            completed
        )
    } else if completed == 0 && failed > 0 {
        format!(
            "All {} fix task(s) failed. Would you like to review what went wrong and try again?",
            failed
        )
    } else {
        format!(
            "{} fix task(s) completed and {} failed. Would you like to review the results?",
            completed, failed
        )
    };

    let msg = db
        .create_validation_message(&CreateValidationMessage {
            session_id: session_id.to_string(),
            role: ValidationMessageRole::System,
            content,
            metadata: Some(serde_json::json!({ "type": "fix_tasks_completed" })),
        })
        .ok()?;

    let _ = event_tx.send(LiveEvent::ValidationMessageAdded {
        session_id: session_id.to_string(),
        message_id: msg.id.clone(),
        role: "system".to_string(),
    });

    Some(msg)
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateValidationSessionInput {
    pub ticket_id: String,
    pub project_id: Option<String>,
    /// Agent for validation chat (e.g. "cursor", "claude")
    pub agent_type: Option<String>,
}

#[tauri::command]
pub async fn create_validation_session(
    input: CreateValidationSessionInput,
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
) -> Result<ValidationSession, String> {
    let session = db
        .create_validation_session(&CreateValidationSession {
            ticket_id: input.ticket_id.clone(),
            project_id: input.project_id,
            agent_type: input.agent_type,
        })
        .map_err(|e| e.to_string())?;

    let _ = event_tx.send(LiveEvent::ValidationSessionCreated {
        session_id: session.id.clone(),
        ticket_id: input.ticket_id,
    });

    Ok(session)
}

#[tauri::command]
pub async fn get_validation_session(
    session_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<ValidationSession, String> {
    db.get_validation_session(&session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_validation_sessions(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<ValidationSession>, String> {
    db.get_validation_sessions(&ticket_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_validation_session_status(
    session_id: String,
    status: String,
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
) -> Result<(), String> {
    let status = ValidationSessionStatus::parse(&status)
        .ok_or_else(|| format!("Invalid validation status: {}", status))?;

    db.update_validation_session_status(&session_id, &status)
        .map_err(|e| e.to_string())?;

    let _ = event_tx.send(LiveEvent::ValidationSessionUpdated {
        session_id,
    });

    Ok(())
}

#[tauri::command]
pub async fn delete_validation_session(
    session_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.delete_validation_session(&session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_validation_messages(
    session_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<ValidationMessage>, String> {
    db.get_validation_messages(&session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_validation_app(
    session_id: String,
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
    app_process_manager: State<'_, AppProcessManager>,
) -> Result<(), String> {
    app_process_manager.stop(&session_id);

    let system_msg = db
        .create_validation_message(&CreateValidationMessage {
            session_id: session_id.clone(),
            role: ValidationMessageRole::System,
            content: "App stopped.".to_string(),
            metadata: None,
        })
        .map_err(|e| e.to_string())?;
    let _ = db.update_validation_session_status(&session_id, &ValidationSessionStatus::Chatting);
    let _ = event_tx.send(LiveEvent::ValidationMessageAdded {
        session_id: session_id.clone(),
        message_id: system_msg.id,
        role: "system".to_string(),
    });
    let _ = event_tx.send(LiveEvent::ValidationSessionUpdated {
        session_id: session_id.clone(),
    });

    Ok(())
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationAppStatus {
    pub running: bool,
}

#[tauri::command]
pub async fn get_validation_app_status(
    session_id: String,
    app_process_manager: State<'_, AppProcessManager>,
) -> Result<ValidationAppStatus, String> {
    Ok(ValidationAppStatus {
        running: app_process_manager.is_running(&session_id),
    })
}

fn resolve_validation_agent_id(agent_type: Option<&str>, registry: &AgentRegistry) -> String {
    agent_type
        .map(|s| s.to_string())
        .unwrap_or_else(|| registry.default_agent_id())
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendValidationMessageOptions {
    pub model: Option<String>,
    pub timeout_minutes: Option<u32>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendValidationMessageRequest {
    pub session_id: String,
    pub content: String,
    pub options: Option<SendValidationMessageOptions>,
}

#[tauri::command]
pub async fn send_validation_message(
    request: SendValidationMessageRequest,
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
    api_conn: State<'_, ApiConnState>,
    agent_settings: State<'_, AgentSettingsManager>,
    app_process_manager: State<'_, AppProcessManager>,
    registry: State<'_, AgentRegistry>,
) -> Result<ValidationMessage, String> {
    let session_id = request.session_id;
    let content = request.content;
    let options = request.options;
    // Store the user message
    let user_message = db
        .create_validation_message(&CreateValidationMessage {
            session_id: session_id.clone(),
            role: ValidationMessageRole::User,
            content: content.clone(),
            metadata: None,
        })
        .map_err(|e| e.to_string())?;

    let _ = event_tx.send(LiveEvent::ValidationMessageAdded {
        session_id: session_id.clone(),
        message_id: user_message.id.clone(),
        role: "user".to_string(),
    });

    let session = db
        .get_validation_session(&session_id)
        .map_err(|e| e.to_string())?;
    if session.status == ValidationSessionStatus::Created {
        let _ = db.update_validation_session_status(
            &session_id,
            &ValidationSessionStatus::Chatting,
        );
    }

    let ticket = db
        .get_ticket(&session.ticket_id)
        .map_err(|e| e.to_string())?;

    let project_id = session
        .project_id
        .as_ref()
        .or(ticket.project_id.as_ref())
        .ok_or_else(|| "No project for ticket".to_string())?;
    let project = db
        .get_project(project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project not found: {}", project_id))?;

    let branch_diff = get_branch_diff_sync(db.inner(), &session.ticket_id)
        .map_err(|e| e.to_string())?
        .diff;

    let agent_id = resolve_validation_agent_id(session.agent_type.as_deref(), &registry);
    let provider = registry
        .get(&agent_id)
        .ok_or_else(|| format!("Unknown agent: {}", agent_id))?;

    let agent_config = agent_settings.agent_config_for(&agent_id);

    let model = options.as_ref().and_then(|o| o.model.clone());
    let timeout_minutes = options.and_then(|o| o.timeout_minutes);
    let timeout_secs: u64 = timeout_minutes.unwrap_or(10).saturating_mul(60).into();

    let config = crate::agents::validation_agent::ValidationAgentConfig {
        session_id: session_id.clone(),
        repo_path: std::path::PathBuf::from(&project.path),
        api_url: api_conn.url.clone(),
        api_token: api_conn.token.clone(),
        model: model.clone(),
        agent_config,
        agent_id,
        provider,
        ticket_title: ticket.title.clone(),
        ticket_description: ticket.description_md.clone(),
        branch_diff,
        acceptance_criteria: None,
        timeout_secs,
    };

    let agent = crate::agents::validation_agent::ValidationAgent::new(
        config,
        event_tx.inner().clone(),
    );

    let messages = db
        .get_validation_messages(&session_id)
        .map_err(|e| e.to_string())?;

    let response_text = agent.process_message(&messages).await.map_err(|e| {
        tracing::error!("Validation agent error: {}", e);
        e
    })?;

    // Detect structured blocks before saving so we can tag the message
    let has_fix_task = parse_create_fix_tasks_from_response(&response_text).is_some();
    let assistant_metadata = if has_fix_task {
        Some(serde_json::json!({ "type": "fix_task_response" }))
    } else {
        None
    };

    let assistant_msg = db
        .create_validation_message(&CreateValidationMessage {
            session_id: session_id.clone(),
            role: ValidationMessageRole::Assistant,
            content: response_text.clone(),
            metadata: assistant_metadata,
        })
        .map_err(|e| e.to_string())?;

    let _ = event_tx.send(LiveEvent::ValidationMessageAdded {
        session_id: session_id.clone(),
        message_id: assistant_msg.id.clone(),
        role: "assistant".to_string(),
    });

    let (working_dir_path, worktree_path, repo_path_for_cleanup) = {
        match get_ticket_working_dir(db.inner(), &session.ticket_id) {
            Ok((wt_path, _branch)) => {
                if wt_path != project.path {
                    (wt_path, None, None)
                } else {
                    let ticket_for_branch = db.get_ticket(&session.ticket_id).ok();
                    let branch = ticket_for_branch.as_ref().and_then(|t| t.branch_name.clone());
                    if let Some(branch_name) = branch {
                        let repo = std::path::PathBuf::from(&project.path);
                        match crate::agents::worktree::create_worktree_with_existing_branch(
                            &repo,
                            &branch_name,
                            &format!("validation-{}", session_id),
                            None,
                        ) {
                            Ok(wt_info) => {
                                tracing::info!(
                                    "Created validation worktree at {} for branch {}",
                                    wt_info.path.display(),
                                    branch_name
                                );
                                let wt = wt_info.path.clone();
                                (wt.to_string_lossy().to_string(), Some(wt), Some(repo))
                            }
                            Err(e) => {
                                tracing::warn!("Failed to create validation worktree: {}", e);
                                (project.path.clone(), None, None)
                            }
                        }
                    } else {
                        (project.path.clone(), None, None)
                    }
                }
            }
            Err(_) => (project.path.clone(), None, None),
        }
    };
    let working_dir = Path::new(&working_dir_path);

    // Unified command loop: handles run_command and start_app in a single loop
    // so the agent can freely chain: run_command -> start_app -> (fail) -> run_command -> start_app
    let mut current_response = response_text.clone();
    let mut last_assistant_msg = assistant_msg;
    let mut all_fix_task_ids: Vec<String> = Vec::new();
    const MAX_ROUNDS: usize = 10;

    for _round in 0..MAX_ROUNDS {
        // 1. Check for run_command first
        if let Some(rc) = parse_run_command_from_response(&current_response) {
            tracing::info!("Running validation command: {}", rc.command);

            let cmd_output = std::process::Command::new("sh")
                .args(["-c", &rc.command])
                .current_dir(working_dir)
                .output();

            let (exit_code, stdout_str, stderr_str) = match cmd_output {
                Ok(output) => (
                    output.status.code().unwrap_or(-1),
                    String::from_utf8_lossy(&output.stdout).to_string(),
                    String::from_utf8_lossy(&output.stderr).to_string(),
                ),
                Err(e) => (-1, String::new(), format!("Failed to execute: {}", e)),
            };

            let combined = if stderr_str.is_empty() {
                stdout_str.chars().take(3000).collect::<String>()
            } else {
                let out: String = stdout_str.chars().take(1500).collect();
                let err: String = stderr_str.chars().take(1500).collect();
                format!("stdout:\n{}\nstderr:\n{}", out, err)
            };

            let status_label = if exit_code == 0 { "success" } else { "failed" };
            let _ = db.create_validation_message(&CreateValidationMessage {
                session_id: session_id.clone(),
                role: ValidationMessageRole::System,
                content: format!("Ran `{}` (exit {}, {})", rc.command, exit_code, status_label),
                metadata: None,
            });

            let _ = db.create_validation_message(&CreateValidationMessage {
                session_id: session_id.clone(),
                role: ValidationMessageRole::User,
                content: format!(
                    "Command `{}` finished with exit code {}.\n\nOutput:\n```\n{}\n```",
                    rc.command, exit_code, combined
                ),
                metadata: None,
            });

            let (next_response, next_msg, fix_ids) = send_agent_followup(
                &agent, &db, &event_tx, &session_id, &session, &ticket,
            ).await?;
            all_fix_task_ids.extend(fix_ids);
            current_response = next_response;
            last_assistant_msg = next_msg;
            continue;
        }

        // 2. Check for start_app
        if let Some(start_app) = parse_start_app_from_response(&current_response) {
            let start_result = app_process_manager.start(
                session_id.clone(),
                start_app.command.clone(),
                working_dir,
                event_tx.inner().clone(),
                worktree_path.clone(),
                repo_path_for_cleanup.clone(),
            );

            match start_result {
                Ok(StartResult::ExitedEarly { exit_code, output }) => {
                    let _ = db.create_validation_message(&CreateValidationMessage {
                        session_id: session_id.clone(),
                        role: ValidationMessageRole::System,
                        content: format!("App failed to start: `{}` (exit {})", start_app.command, exit_code),
                        metadata: None,
                    });
                    let _ = db.create_validation_message(&CreateValidationMessage {
                        session_id: session_id.clone(),
                        role: ValidationMessageRole::User,
                        content: format!(
                            "The app failed to start. Command `{}` exited with code {}.\n\nLast output:\n```\n{}\n```\n\nPlease diagnose the issue and output a `run_command` to fix it, or a new `start_app` to try again.",
                            start_app.command, exit_code, output.chars().take(3000).collect::<String>()
                        ),
                        metadata: None,
                    });

                    let (next_response, next_msg, fix_ids) = send_agent_followup(
                        &agent, &db, &event_tx, &session_id, &session, &ticket,
                    ).await?;
                    all_fix_task_ids.extend(fix_ids);
                    current_response = next_response;
                    last_assistant_msg = next_msg;
                    continue; // agent may respond with run_command or start_app
                }
                Err(e) => {
                    tracing::error!("Failed to start app: {}", e);
                }
                Ok(StartResult::Running) => {}
            }

            // App is running successfully -- send follow-up for testing instructions
            if app_process_manager.is_running(&session_id) {
            let _ = db.update_validation_session_status(
                &session_id,
                &ValidationSessionStatus::AppRunning,
            );
            let _ = event_tx.send(LiveEvent::ValidationSessionUpdated {
                session_id: session_id.clone(),
            });

            let port_suffix = start_app
                .port
                .map(|p| format!(" on port {}", p))
                .unwrap_or_default();
            let log_file_path = working_dir.join(".validation-app.log");
            let log_file_str = log_file_path.to_string_lossy();
            let system_msg = db
                .create_validation_message(&CreateValidationMessage {
                    session_id: session_id.clone(),
                    role: ValidationMessageRole::System,
                    content: format!(
                        "App started: `{}`{}. Logs are streaming in the panel.",
                        start_app.command, port_suffix
                    ),
                    metadata: None,
                })
                .map_err(|e| e.to_string())?;
            let _ = event_tx.send(LiveEvent::ValidationMessageAdded {
                session_id: session_id.clone(),
                message_id: system_msg.id.clone(),
                role: "system".to_string(),
            });

            let follow_up_prompt = format!(
                "The application is now running{}. App logs are being written to `{}` — you can read that file to check for errors. Please provide testing instructions.",
                port_suffix, log_file_str
            );
            let follow_up_user = db
                .create_validation_message(&CreateValidationMessage {
                    session_id: session_id.clone(),
                    role: ValidationMessageRole::User,
                    content: follow_up_prompt,
                    metadata: None,
                })
                .map_err(|e| e.to_string())?;
            let _ = event_tx.send(LiveEvent::ValidationMessageAdded {
                session_id: session_id.clone(),
                message_id: follow_up_user.id.clone(),
                role: "user".to_string(),
            });

            let messages = db
                .get_validation_messages(&session_id)
                .map_err(|e| e.to_string())?;
            let follow_up_response = agent.process_message(&messages).await.map_err(|e| {
                tracing::error!("Validation agent follow-up error: {}", e);
                e
            })?;

            // Process fix tasks from both the initial response and the follow-up
            // before returning. Without this, fix tasks in a response that also
            // contains a start_app block would be silently dropped by the early return.
            let fix_ids_1 = process_fix_tasks_in_response(
                &response_text, db.inner(), &session_id, &session.ticket_id,
                &ticket.title, &ticket.board_id, event_tx.inner(),
            );
            let fix_ids_2 = process_fix_tasks_in_response(
                &follow_up_response, db.inner(), &session_id, &session.ticket_id,
                &ticket.title, &ticket.board_id, event_tx.inner(),
            );
            all_fix_task_ids.extend(fix_ids_1);
            all_fix_task_ids.extend(fix_ids_2);

            let follow_up_has_fix_task =
                parse_create_fix_tasks_from_response(&follow_up_response).is_some();
            let follow_up_metadata = if follow_up_has_fix_task {
                Some(serde_json::json!({ "type": "fix_task_response" }))
            } else {
                None
            };

            let second_assistant = db
                .create_validation_message(&CreateValidationMessage {
                    session_id: session_id.clone(),
                    role: ValidationMessageRole::Assistant,
                    content: follow_up_response,
                    metadata: follow_up_metadata,
                })
                .map_err(|e| e.to_string())?;
            let _ = event_tx.send(LiveEvent::ValidationMessageAdded {
                session_id: session_id.clone(),
                message_id: second_assistant.id.clone(),
                role: "assistant".to_string(),
            });

            // Wait for any fix tasks to finish before returning to the user
            if !all_fix_task_ids.is_empty() {
                wait_for_fix_tasks(&all_fix_task_ids, db.inner(), event_tx.inner(), &session_id).await;
                if let Some(completion_msg) = post_fix_tasks_completion_message(
                    &all_fix_task_ids, db.inner(), event_tx.inner(), &session_id,
                ) {
                    return Ok(completion_msg);
                }
            }

            return Ok(second_assistant);
            }
        }
        break; // start_app handled (or neither run_command nor start_app found), exit loop
    }

    // If no app process ended up running for this session, clean up the
    // worktree now — otherwise it would be leaked.  When a process IS running
    // the AppProcessManager owns the worktree_path and stop() handles cleanup.
    if !app_process_manager.is_running(&session_id) {
        if let (Some(wt), Some(repo)) = (worktree_path, repo_path_for_cleanup) {
            if let Err(e) = crate::agents::worktree::remove_worktree(&wt, &repo) {
                tracing::warn!("Failed to remove validation worktree after loop: {}", e);
            }
        }
    }

    // If the agent requested to create fix tasks, create them automatically
    let fix_ids = process_fix_tasks_in_response(
        &current_response, db.inner(), &session_id, &session.ticket_id,
        &ticket.title, &ticket.board_id, event_tx.inner(),
    );
    all_fix_task_ids.extend(fix_ids);

    // Wait for any fix tasks to finish before returning to the user
    if !all_fix_task_ids.is_empty() {
        wait_for_fix_tasks(&all_fix_task_ids, db.inner(), event_tx.inner(), &session_id).await;
        if let Some(completion_msg) = post_fix_tasks_completion_message(
            &all_fix_task_ids, db.inner(), event_tx.inner(), &session_id,
        ) {
            return Ok(completion_msg);
        }
    }

    Ok(last_assistant_msg)
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFixTasksInput {
    pub session_id: String,
    pub ticket_id: String,
    pub tasks: Vec<FixTask>,
}

#[tauri::command]
pub async fn create_fix_tasks(
    input: CreateFixTasksInput,
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
) -> Result<Vec<String>, String> {
    let ticket = db
        .get_ticket(&input.ticket_id)
        .map_err(|e| e.to_string())?;

    let mut task_ids = Vec::new();

    for fix_task in input.tasks.iter() {
        // Build description with acceptance criteria
        let mut description = fix_task.description.clone();
        if let Some(ref criteria) = fix_task.acceptance_criteria {
            description.push_str("\n\n## Acceptance Criteria\n");
            for criterion in criteria {
                description.push_str(&format!("- {}\n", criterion));
            }
        }

        let task = db
            .create_task(&crate::db::models::CreateTask {
                ticket_id: input.ticket_id.clone(),
                task_type: crate::db::models::TaskType::Custom,
                title: Some(fix_task.title.clone()),
                content: Some(description),
            })
            .map_err(|e| e.to_string())?;

        task_ids.push(task.id);
    }

    // Update validation session status to failed
    let _ = db.update_validation_session_status(
        &input.session_id,
        &ValidationSessionStatus::Failed,
    );

    // Add a system message noting fix tasks were created
    let task_summary = input
        .tasks
        .iter()
        .map(|t| format!("- {}", t.title))
        .collect::<Vec<_>>()
        .join("\n");

    let _ = db.create_validation_message(&CreateValidationMessage {
        session_id: input.session_id.clone(),
        role: ValidationMessageRole::System,
        content: format!(
            "Fix tasks created for ticket **{}**:\n{}",
            ticket.title, task_summary
        ),
        metadata: Some(serde_json::json!({
            "type": "fix_tasks_created",
            "task_ids": task_ids,
        })),
    });

    // Move ticket back to Ready column if possible
    let columns = db
        .get_columns(&ticket.board_id)
        .map_err(|e| e.to_string())?;
    if let Some(ready_col) = columns.iter().find(|c| c.name == "Ready") {
        let _ = db.move_ticket(&input.ticket_id, &ready_col.id);
    }

    let _ = event_tx.send(LiveEvent::ValidationFixTasksCreated {
        session_id: input.session_id,
        ticket_id: input.ticket_id,
        task_count: task_ids.len(),
    });

    Ok(task_ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::cost::RunCostData;
    use crate::agents::provider::{AgentProvider, AgentRunConfig};
    use crate::agents::registry::AgentRegistry;

    #[derive(Debug)]
    struct FakeProvider {
        name: String,
        available: bool,
    }

    impl AgentProvider for FakeProvider {
        fn id(&self) -> &str { &self.name }
        fn display_name(&self) -> &str { &self.name }
        fn build_command(&self, _: &AgentRunConfig) -> (String, Vec<String>) { (self.name.clone(), vec![]) }
        fn build_env_vars(&self, _: &AgentRunConfig) -> Vec<(String, String)> { vec![] }
        fn extract_text(&self, o: &str) -> String { o.to_string() }
        fn extract_cost(&self, _: &str, _: &str, _: f64) -> Option<RunCostData> { None }
        fn is_available(&self) -> bool { self.available }
        fn get_version(&self) -> Option<String> { None }
        fn config_dir_name(&self) -> &str { ".fake" }
        fn command_instructions_subdir(&self) -> &str { "commands" }
        fn format_command_reference(&self, c: &str) -> String { format!("/{}", c) }
    }

    fn make_registry(providers: Vec<(&str, bool)>) -> AgentRegistry {
        let mut reg = AgentRegistry::new();
        for (name, available) in providers {
            reg.register(std::sync::Arc::new(FakeProvider {
                name: name.to_string(),
                available,
            }));
        }
        reg
    }

    #[test]
    fn resolve_explicit_agent_type() {
        let reg = make_registry(vec![("cursor", true), ("claude", true)]);
        assert_eq!(resolve_validation_agent_id(Some("cursor"), &reg), "cursor");
    }

    #[test]
    fn resolve_unknown_explicit_passes_through() {
        let reg = make_registry(vec![("cursor", true)]);
        assert_eq!(resolve_validation_agent_id(Some("new-agent"), &reg), "new-agent");
    }

    #[test]
    fn resolve_none_returns_first_available() {
        let reg = make_registry(vec![("offline", false), ("online", true)]);
        assert_eq!(resolve_validation_agent_id(None, &reg), "online");
    }

    #[test]
    fn resolve_none_falls_back_to_first_registered_when_none_available() {
        let reg = make_registry(vec![("offline", false)]);
        assert_eq!(resolve_validation_agent_id(None, &reg), "offline");
    }

    #[test]
    fn resolve_none_returns_empty_when_registry_empty() {
        let reg = AgentRegistry::new();
        assert_eq!(resolve_validation_agent_id(None, &reg), "");
    }
}
