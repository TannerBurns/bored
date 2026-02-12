//! Tauri commands for validation sessions and messages

use std::sync::Arc;
use std::path::Path;
use tauri::State;
use tokio::sync::broadcast;

use crate::agents::validation_agent::AppProcessManager;
use crate::agents::{AgentKind, ClaudeApiConfig};
use crate::api::state::LiveEvent;
use crate::commands::claude::ClaudeApiSettingsState;
use crate::commands::next_steps::{get_branch_diff_sync, get_ticket_working_dir};
use crate::commands::ApiConnState;
use crate::db::models::{
    CreateValidationMessage, CreateValidationSession, FixTask,
    ValidationMessage, ValidationMessageRole, ValidationSession, ValidationSessionStatus,
};
use crate::db::Database;

/// Parsed start_app block from agent response
struct StartAppBlock {
    command: String,
    port: Option<i32>,
}

/// Parsed create_fix_tasks block from agent response
struct CreateFixTasksBlock {
    tasks: Vec<FixTask>,
}

/// Parse all fenced JSON blocks from the agent response
fn parse_fenced_json_blocks(response_text: &str) -> Vec<serde_json::Value> {
    let blocks: Vec<&str> = response_text.split("```").collect();
    let mut results = Vec::new();
    for (i, segment) in blocks.iter().enumerate() {
        if i % 2 == 0 {
            continue;
        }
        let content = segment.trim_start();
        let json_str = content.strip_prefix("json").map(|s| s.trim()).unwrap_or(content);
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
            results.push(v);
        }
    }
    results
}

fn parse_start_app_from_response(response_text: &str) -> Option<StartAppBlock> {
    for v in parse_fenced_json_blocks(response_text) {
        if let Some(start_app) = v.get("start_app").and_then(|s| s.as_object()) {
            if let Some(command) = start_app.get("command").and_then(|c| c.as_str()) {
                let port = start_app.get("port").and_then(|p| p.as_i64()).map(|p| p as i32);
                return Some(StartAppBlock {
                    command: command.to_string(),
                    port,
                });
            }
        }
    }
    None
}

fn parse_fix_task_from_json_obj(obj: &serde_json::Map<String, serde_json::Value>) -> FixTask {
    let title = obj.get("title").and_then(|t| t.as_str()).unwrap_or("Fix task");
    let description = obj.get("description").and_then(|d| d.as_str()).unwrap_or("");
    let acceptance_criteria = obj
        .get("acceptance_criteria")
        .or_else(|| obj.get("acceptanceCriteria"))
        .and_then(|ac| ac.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });
    FixTask {
        title: title.to_string(),
        description: description.to_string(),
        acceptance_criteria,
    }
}

fn parse_create_fix_tasks_from_response(response_text: &str) -> Option<CreateFixTasksBlock> {
    for v in parse_fenced_json_blocks(response_text) {
        // Singular form: { "create_fix_task": { "title": "...", "description": "..." } }
        if let Some(task_obj) = v.get("create_fix_task").and_then(|s| s.as_object()) {
            return Some(CreateFixTasksBlock {
                tasks: vec![parse_fix_task_from_json_obj(task_obj)],
            });
        }
        // Plural form (backward compat): { "create_fix_tasks": { "tasks": [...] } }
        if let Some(cft) = v.get("create_fix_tasks").and_then(|s| s.as_object()) {
            if let Some(tasks_arr) = cft.get("tasks").and_then(|t| t.as_array()) {
                let tasks: Vec<FixTask> = tasks_arr
                    .iter()
                    .filter_map(|tv| tv.as_object().map(parse_fix_task_from_json_obj))
                    .collect();
                if !tasks.is_empty() {
                    return Some(CreateFixTasksBlock { tasks });
                }
            }
        }
    }
    None
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

fn resolve_validation_agent_kind(agent_type: Option<&str>) -> AgentKind {
    if agent_type == Some("cursor") {
        AgentKind::Cursor
    } else {
        AgentKind::Claude
    }
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
    claude_api_state: State<'_, ClaudeApiSettingsState>,
    app_process_manager: State<'_, AppProcessManager>,
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

    let agent_kind = resolve_validation_agent_kind(session.agent_type.as_deref());
    let claude_api_config = Some(ClaudeApiConfig::from(claude_api_state.get()));

    let model = options.as_ref().and_then(|o| o.model.clone());
    let timeout_minutes = options.and_then(|o| o.timeout_minutes);
    let timeout_secs: u64 = timeout_minutes.unwrap_or(10).saturating_mul(60).into();

    let config = crate::agents::validation_agent::ValidationAgentConfig {
        session_id: session_id.clone(),
        repo_path: std::path::PathBuf::from(&project.path),
        api_url: api_conn.url.clone(),
        api_token: api_conn.token.clone(),
        model: model.clone(),
        claude_api_config,
        agent_kind,
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

    // If the agent requested to start the app, start it and run a follow-up for testing instructions
    if let Some(start_app) = parse_start_app_from_response(&response_text) {
        let (working_dir_path, branch_name) = get_ticket_working_dir(db.inner(), &session.ticket_id)
            .unwrap_or_else(|_| (project.path.clone(), String::new()));
        let working_dir = Path::new(&working_dir_path);

        // Ensure the working directory is on the ticket's branch
        if !branch_name.is_empty() {
            let checkout_result = std::process::Command::new("git")
                .args(["checkout", &branch_name])
                .current_dir(working_dir)
                .output();
            match checkout_result {
                Ok(output) if output.status.success() => {
                    tracing::info!("Checked out branch {} in {}", branch_name, working_dir_path);
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    tracing::warn!("git checkout {} failed: {}", branch_name, stderr);
                }
                Err(e) => {
                    tracing::warn!("Failed to run git checkout: {}", e);
                }
            }
        }
        if app_process_manager
            .start(
                session_id.clone(),
                start_app.command.clone(),
                working_dir,
                event_tx.inner().clone(),
            )
            .is_ok()
        {
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

            let follow_up_prompt = start_app
                .port
                .map(|p| format!(
                    "The application is now running on port {}. Please provide testing instructions.",
                    p
                ))
                .unwrap_or_else(|| {
                    "The application is now running. Please provide testing instructions.".to_string()
                });
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

            let second_assistant = db
                .create_validation_message(&CreateValidationMessage {
                    session_id: session_id.clone(),
                    role: ValidationMessageRole::Assistant,
                    content: follow_up_response,
                    metadata: None,
                })
                .map_err(|e| e.to_string())?;
            let _ = event_tx.send(LiveEvent::ValidationMessageAdded {
                session_id: session_id.clone(),
                message_id: second_assistant.id.clone(),
                role: "assistant".to_string(),
            });

            return Ok(second_assistant);
        }
    }

    // If the agent requested to create fix tasks, create them automatically
    if let Some(fix_block) = parse_create_fix_tasks_from_response(&response_text) {
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
                ticket_id: session.ticket_id.clone(),
                task_type: crate::db::models::TaskType::Custom,
                title: Some(fix_task.title.clone()),
                content: Some(description),
            }) {
                task_ids.push(task.id);
            }
        }
        if !task_ids.is_empty() {
            let _ = db.update_validation_session_status(
                &session_id,
                &ValidationSessionStatus::Failed,
            );
            let task_summary = fix_block
                .tasks
                .iter()
                .map(|t| format!("- {}", t.title))
                .collect::<Vec<_>>()
                .join("\n");
            let _ = db.create_validation_message(&CreateValidationMessage {
                session_id: session_id.clone(),
                role: ValidationMessageRole::System,
                content: format!(
                    "Fix tasks created for ticket **{}**:\n{}\n\nA worker agent will pick these up. You'll be notified when the work completes.",
                    ticket.title, task_summary
                ),
                metadata: Some(serde_json::json!({
                    "type": "fix_tasks_created",
                    "task_ids": task_ids,
                })),
            });
            // Move ticket to Ready so workers pick it up
            if let Ok(columns) = db.get_columns(&ticket.board_id) {
                if let Some(ready_col) = columns.iter().find(|c| c.name == "Ready") {
                    let _ = db.move_ticket(&session.ticket_id, &ready_col.id);
                }
            }
            let _ = event_tx.send(LiveEvent::ValidationFixTasksCreated {
                session_id: session_id.clone(),
                ticket_id: session.ticket_id.clone(),
                task_count: task_ids.len(),
            });
        }
    }

    Ok(assistant_msg)
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
