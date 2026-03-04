use std::path::PathBuf;
use std::sync::Arc;

use tauri::State;
use tokio::sync::broadcast;

use crate::agents::chat::{ChatAgent, ChatAgentConfig, TicketBuilderOutput};
use crate::agents::cost::AggregatedCost;
use crate::agents::registry::AgentRegistry;
use crate::agents::validation_agent::AppProcessManager;
use crate::api::state::LiveEvent;
use crate::db::models::{
    Chat, ChatEvent, ChatMessage, ChatMessageRole, ChatMode, CreateChat, CreateTask, CreateTicket,
    WorkflowType,
};
use crate::db::Database;

use super::AgentSettingsManager;

#[tauri::command]
pub async fn create_chat(
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
    input: CreateChat,
) -> Result<Chat, String> {
    match input.mode {
        ChatMode::TicketBuilder => {
            if input.board_id.is_none() {
                return Err("board_id is required for ticket_builder mode".into());
            }
        }
        ChatMode::Review => {
            if input.board_id.is_none() || input.ticket_id.is_none() {
                return Err("board_id and ticket_id are required for review mode".into());
            }
        }
        _ => {}
    }

    let chat = db.create_chat(&input).map_err(|e| e.to_string())?;
    let _ = event_tx.send(LiveEvent::ChatCreated {
        chat_id: chat.id.clone(),
    });
    Ok(chat)
}

#[tauri::command]
pub async fn get_chats(
    db: State<'_, Arc<Database>>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<Chat>, String> {
    let limit = limit.unwrap_or(10);
    let offset = offset.unwrap_or(0);
    db.get_chats(limit, offset).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_chat(
    db: State<'_, Arc<Database>>,
    chat_id: String,
) -> Result<Chat, String> {
    db.get_chat(&chat_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_chat(
    db: State<'_, Arc<Database>>,
    chat_id: String,
) -> Result<(), String> {
    db.delete_chat(&chat_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_chat_messages(
    db: State<'_, Arc<Database>>,
    chat_id: String,
) -> Result<Vec<ChatMessage>, String> {
    db.get_chat_messages(&chat_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_chat_events(
    db: State<'_, Arc<Database>>,
    chat_id: String,
) -> Result<Vec<ChatEvent>, String> {
    db.get_chat_events(&chat_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_chat_cost(
    db: State<'_, Arc<Database>>,
    chat_id: String,
) -> Result<AggregatedCost, String> {
    db.get_chat_cost(&chat_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn send_chat_message(
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
    registry: State<'_, Arc<AgentRegistry>>,
    agent_settings: State<'_, AgentSettingsManager>,
    app_process_manager: State<'_, AppProcessManager>,
    chat_id: String,
    content: String,
    timeout_secs: Option<u64>,
) -> Result<ChatMessage, String> {
    let chat = db.get_chat(&chat_id).map_err(|e| e.to_string())?;
    let project = db
        .get_project(&chat.project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project not found: {}", chat.project_id))?;

    let user_msg = db
        .create_chat_message(&chat_id, ChatMessageRole::User, &content, None)
        .map_err(|e| e.to_string())?;
    let _ = event_tx.send(LiveEvent::ChatMessageAdded {
        chat_id: chat_id.clone(),
        message_id: user_msg.id.clone(),
        role: "user".to_string(),
    });

    let messages = db.get_chat_messages(&chat_id).map_err(|e| e.to_string())?;

    let agent_config = agent_settings.agent_config_for(&chat.agent_type);

    let config = ChatAgentConfig {
        chat_id: chat_id.clone(),
        mode: chat.mode,
        agent_id: chat.agent_type.clone(),
        repo_path: PathBuf::from(&project.path),
        model: chat.model.clone(),
        agent_config,
        timeout_secs,
    };

    let agent = ChatAgent::new(
        db.inner().clone(),
        config,
        event_tx.inner().clone(),
        registry.inner().clone(),
    );

    let assistant_msg = agent
        .process_message(messages, Some(&*app_process_manager))
        .await
        .map_err(|e| e.to_string())?;

    Ok(assistant_msg)
}

#[tauri::command]
pub async fn stop_chat_app(
    app_manager: State<'_, AppProcessManager>,
    chat_id: String,
) -> Result<(), String> {
    app_manager.stop(&chat_id);
    Ok(())
}

#[tauri::command]
pub async fn get_chat_app_status(
    app_manager: State<'_, AppProcessManager>,
    chat_id: String,
) -> Result<bool, String> {
    Ok(app_manager.is_running(&chat_id))
}

#[tauri::command]
pub async fn create_tickets_from_chat(
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
    chat_id: String,
    tickets_json: String,
) -> Result<Vec<String>, String> {
    let chat = db.get_chat(&chat_id).map_err(|e| e.to_string())?;
    let board_id = chat
        .board_id
        .ok_or_else(|| "No board_id on chat".to_string())?;

    let columns = db.get_columns(&board_id).map_err(|e| e.to_string())?;
    let backlog_column = columns
        .iter()
        .find(|c| c.name == "Backlog")
        .ok_or_else(|| "No Backlog column found".to_string())?;

    let output: TicketBuilderOutput =
        serde_json::from_str(&tickets_json).map_err(|e| e.to_string())?;

    let mut ticket_ids = Vec::new();

    for ticket_data in &output.tickets {
        let priority = ticket_data.resolved_priority();

        let ticket = db
            .create_ticket(&CreateTicket {
                board_id: board_id.clone(),
                column_id: backlog_column.id.clone(),
                title: ticket_data.title.clone(),
                description_md: ticket_data.description.clone(),
                priority,
                labels: vec![],
                project_id: Some(chat.project_id.clone()),
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            })
            .map_err(|e| e.to_string())?;

        if let Some(ref tasks) = ticket_data.tasks {
            for task in tasks {
                db.create_task(&CreateTask {
                    ticket_id: ticket.id.clone(),
                    task_type: Default::default(),
                    title: Some(task.title.clone()),
                    content: None,
                })
                .map_err(|e| e.to_string())?;
            }
        }

        let _ = event_tx.send(LiveEvent::TicketCreated {
            ticket_id: ticket.id.clone(),
            board_id: board_id.clone(),
        });

        ticket_ids.push(ticket.id);
    }

    if let Ok(sys_msg) = db.create_chat_message(
        &chat_id,
        ChatMessageRole::System,
        &format!("Created {} ticket(s)", ticket_ids.len()),
        Some(&serde_json::json!({
            "type": "tickets_created",
            "ticketIds": &ticket_ids,
        })),
    ) {
        let _ = event_tx.send(LiveEvent::ChatMessageAdded {
            chat_id: chat_id.clone(),
            message_id: sys_msg.id,
            role: "system".to_string(),
        });
    }

    Ok(ticket_ids)
}
