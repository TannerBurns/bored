//! Tauri commands for validation sessions and messages

use std::sync::Arc;
use tauri::State;
use tokio::sync::broadcast;

use crate::agents::{AgentKind, ClaudeApiConfig};
use crate::api::state::LiveEvent;
use crate::commands::claude::ClaudeApiSettingsState;
use crate::commands::next_steps::get_branch_diff_sync;
use crate::commands::ApiConnState;
use crate::db::models::{
    CreateValidationMessage, CreateValidationSession, FixTask,
    ValidationMessage, ValidationMessageRole, ValidationSession, ValidationSessionStatus,
};
use crate::db::Database;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateValidationSessionInput {
    pub ticket_id: String,
    pub project_id: Option<String>,
    pub app_command: Option<String>,
    pub app_port: Option<i32>,
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
            app_command: input.app_command,
            app_port: input.app_port,
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

fn resolve_validation_agent_kind(agent_type: Option<&str>) -> AgentKind {
    if agent_type == Some("cursor") {
        AgentKind::Cursor
    } else {
        AgentKind::Claude
    }
}

#[tauri::command]
pub async fn send_validation_message(
    session_id: String,
    content: String,
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
    api_conn: State<'_, ApiConnState>,
    claude_api_state: State<'_, ClaudeApiSettingsState>,
) -> Result<ValidationMessage, String> {
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

    let config = crate::agents::validation_agent::ValidationAgentConfig {
        session_id: session_id.clone(),
        repo_path: std::path::PathBuf::from(&project.path),
        api_url: api_conn.url.clone(),
        api_token: api_conn.token.clone(),
        model: None,
        claude_api_config,
        agent_kind,
        ticket_title: ticket.title.clone(),
        ticket_description: ticket.description_md.clone(),
        branch_diff,
        acceptance_criteria: None,
        timeout_secs: 600,
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

    let assistant_msg = db
        .create_validation_message(&CreateValidationMessage {
            session_id: session_id.clone(),
            role: ValidationMessageRole::Assistant,
            content: response_text.clone(),
            metadata: None,
        })
        .map_err(|e| e.to_string())?;

    let _ = event_tx.send(LiveEvent::ValidationMessageAdded {
        session_id: session_id.clone(),
        message_id: assistant_msg.id.clone(),
        role: "assistant".to_string(),
    });

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
