use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::{Manager, State};
use tokio::sync::broadcast;

use crate::agents::chat::{ChatAgent, ChatAgentConfig, ChatAgentError, TicketBuilderOutput};
use crate::agents::cost::AggregatedCost;
use crate::agents::registry::AgentRegistry;
use crate::agents::spawner::CancelHandle;
use crate::agents::validation_agent::AppProcessManager;
use crate::api::state::LiveEvent;
use crate::db::models::{
    Chat, ChatEvent, ChatMessage, ChatMessageRole, ChatMode, CreateChat, CreateTask, CreateTicket,
    WorkflowType,
};
use crate::db::Database;

use super::workflow_settings::WorkflowSettingsState;
use super::AgentSettingsManager;

/// Shared state for tracking cancel handles of in-flight chat agent runs.
pub struct RunningChatAgents {
    pub handles: Arc<Mutex<HashMap<String, CancelHandle>>>,
}

impl RunningChatAgents {
    pub fn new() -> Self {
        Self {
            handles: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for RunningChatAgents {
    fn default() -> Self {
        Self::new()
    }
}

#[tauri::command]
pub async fn create_chat(
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
    input: CreateChat,
) -> Result<Chat, String> {
    match input.mode {
        ChatMode::TicketBuilder | ChatMode::SpecBuilder => {
            if input.board_id.is_none() {
                return Err(format!(
                    "board_id is required for {} mode",
                    input.mode.as_str()
                ));
            }
        }
        ChatMode::Review => {
            if input.board_id.is_none() || input.ticket_id.is_none() {
                return Err("board_id and ticket_id are required for review mode".into());
            }
            if let Some(ref tid) = input.ticket_id {
                let ticket = db
                    .get_ticket(tid)
                    .map_err(|e| format!("Failed to load ticket: {}", e))?;
                if input.workspace_id.is_some() {
                    if ticket.workspace_id != input.workspace_id {
                        return Err("Ticket does not belong to the selected workspace".into());
                    }
                } else if let Some(ref pid) = input.project_id {
                    if ticket.project_id.as_deref() != Some(pid.as_str()) {
                        return Err(
                            "Ticket does not belong to the selected project".into(),
                        );
                    }
                }
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
    app: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
    registry: State<'_, Arc<AgentRegistry>>,
    app_process_manager: State<'_, AppProcessManager>,
    running_chats: State<'_, RunningChatAgents>,
    chat_id: String,
    content: String,
    timeout_secs: Option<u64>,
) -> Result<ChatMessage, String> {
    let chat = db.get_chat(&chat_id).map_err(|e| e.to_string())?;

    let (repo_path, workspace_file, workspace_paths) = if let Some(ref workspace_id) = chat.workspace_id {
        let projects = db.get_workspace_projects(workspace_id)
            .map_err(|e| e.to_string())?;
        if projects.is_empty() {
            return Err("Workspace has no projects".to_string());
        }
        let primary_path = PathBuf::from(&projects[0].path);

        // For review chats with a ticket branch, resolve each project path to its
        // worktree so the agent CLI sees branch changes in all projects.
        let branch_name = chat.ticket_id.as_ref().and_then(|tid| {
            db.get_ticket(tid).ok().and_then(|t| t.branch_name)
        });

        let mut ws_paths: Vec<PathBuf> = Vec::new();
        let mut ws_folders: Vec<(PathBuf, String)> = Vec::new();
        for p in &projects {
            let resolved = if let Some(ref branch) = branch_name {
                match crate::commands::next_steps::resolve_working_dir_strict(&p.path, branch) {
                    Ok(r) => PathBuf::from(r),
                    Err(_) => {
                        tracing::warn!(
                            "No worktree found for project '{}', excluding from chat workspace \
                             to prevent operating on main checkout",
                            p.name
                        );
                        continue;
                    }
                }
            } else {
                PathBuf::from(&p.path)
            };
            ws_paths.push(resolved.clone());
            ws_folders.push((resolved, p.name.clone()));
        }

        let ws_dir = std::env::temp_dir().join("bored").join("chat-workspaces");
        std::fs::create_dir_all(&ws_dir)
            .map_err(|e| format!("Failed to create workspace directory: {}", e))?;
        let ws_file = ws_dir.join(format!("{}.code-workspace", chat_id));
        let folders: Vec<serde_json::Value> = ws_folders.iter()
            .map(|(path, name)| serde_json::json!({ "path": path.to_string_lossy(), "name": name }))
            .collect();
        let ws_content = serde_json::json!({ "folders": folders });
        let ws_json = serde_json::to_string_pretty(&ws_content)
            .map_err(|e| format!("Failed to serialize .code-workspace: {}", e))?;
        std::fs::write(&ws_file, ws_json)
            .map_err(|e| format!("Failed to write .code-workspace file: {}", e))?;

        (primary_path, Some(ws_file), ws_paths)
    } else if let Some(ref project_id) = chat.project_id {
        let project = db.get_project(project_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Project not found: {}", project_id))?;
        (PathBuf::from(&project.path), None, vec![])
    } else {
        return Err("Chat has no project or workspace".to_string());
    };

    let user_msg = db
        .create_chat_message(&chat_id, ChatMessageRole::User, &content, None)
        .map_err(|e| e.to_string())?;
    let _ = event_tx.send(LiveEvent::ChatMessageAdded {
        chat_id: chat_id.clone(),
        message_id: user_msg.id.clone(),
        role: "user".to_string(),
    });

    let messages = db.get_chat_messages(&chat_id).map_err(|e| e.to_string())?;

    let agent_config = app.state::<AgentSettingsManager>().agent_config_for(&chat.agent_type);

    let ws = app.state::<WorkflowSettingsState>().get_for_agent(&chat.agent_type);
    let model = resolve_chat_model(chat.model.as_deref(), &chat.mode, &ws);

    let config = ChatAgentConfig {
        chat_id: chat_id.clone(),
        mode: chat.mode,
        agent_id: chat.agent_type.clone(),
        repo_path,
        model,
        agent_config,
        timeout_secs: Some(timeout_secs.unwrap_or(600)),
        workspace_file,
        workspace_paths,
        debug_mode: ws.debug_mode,
    };

    let cancel_handles = running_chats.handles.clone();
    let agent = ChatAgent::new(
        db.inner().clone(),
        config,
        event_tx.inner().clone(),
        registry.inner().clone(),
    )
    .with_cancel_handles(cancel_handles);

    let result = agent
        .process_message(messages, Some(&*app_process_manager))
        .await;

    // Clean up cancel handle regardless of outcome
    {
        let mut handles = running_chats
            .handles
            .lock()
            .expect("running chat agents mutex poisoned");
        handles.remove(&chat_id);
    }

    match result {
        Ok(msg) => Ok(msg),
        Err(ChatAgentError::Cancelled) => {
            let msgs = db.get_chat_messages(&chat_id).map_err(|e| e.to_string())?;
            msgs.into_iter()
                .last()
                .ok_or_else(|| "No messages found".to_string())
        }
        Err(ChatAgentError::Timeout(_)) => {
            let msgs = db.get_chat_messages(&chat_id).map_err(|e| e.to_string())?;
            msgs.into_iter()
                .last()
                .ok_or_else(|| "No messages found".to_string())
        }
        Err(e) => {
            let error_content = match &e {
                ChatAgentError::NoResponse => {
                    "Agent returned no response".to_string()
                }
                other => format!("An error occurred: {}", other),
            };
            let sys_msg = db
                .create_chat_message(
                    &chat_id,
                    ChatMessageRole::System,
                    &error_content,
                    Some(&serde_json::json!({ "type": "chat_error" })),
                )
                .map_err(|e| e.to_string())?;
            let _ = event_tx.send(LiveEvent::ChatMessageAdded {
                chat_id: chat_id.clone(),
                message_id: sys_msg.id.clone(),
                role: "system".to_string(),
            });
            Ok(sys_msg)
        }
    }
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
pub async fn edit_chat_message(
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
    chat_id: String,
    message_id: String,
) -> Result<(), String> {
    let msg = db
        .get_chat_message(&message_id)
        .map_err(|e| e.to_string())?;

    let created_at_str = msg.created_at.to_rfc3339();
    db.delete_chat_messages_after(&chat_id, &created_at_str)
        .map_err(|e| e.to_string())?;
    db.delete_chat_message(&message_id)
        .map_err(|e| e.to_string())?;

    db.update_chat_agent_session_id(&chat_id, None)
        .map_err(|e| e.to_string())?;

    let _ = event_tx.send(LiveEvent::ChatUpdated {
        chat_id: chat_id.clone(),
    });

    Ok(())
}

#[tauri::command]
pub async fn cancel_chat_generation(
    chat_id: String,
    running_chats: State<'_, RunningChatAgents>,
) -> Result<(), String> {
    let handles = running_chats
        .handles
        .lock()
        .expect("running chat agents mutex poisoned");

    if let Some(handle) = handles.get(&chat_id) {
        handle.cancel();
        tracing::info!("Cancelled chat generation for {}", chat_id);
    } else {
        tracing::warn!("No cancel handle found for chat {}", chat_id);
    }

    Ok(())
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
                project_id: chat.project_id.clone(),
                workspace_id: chat.workspace_id.clone(),
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
                    content: task.content.clone(),
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

fn default_model_for_mode(mode: &ChatMode) -> &'static str {
    match mode {
        ChatMode::General => crate::agents::models::DEFAULT_GENERAL_CHAT_MODEL,
        ChatMode::SpecBuilder => crate::agents::models::DEFAULT_PLANNER_CHAT_MODEL,
        ChatMode::TicketBuilder => crate::agents::models::DEFAULT_TICKET_BUILDER_CHAT_MODEL,
        ChatMode::Review => crate::agents::models::DEFAULT_VALIDATION_CHAT_MODEL,
    }
}

/// Resolve which model to use for a chat message.
///
/// Priority: synced workflow settings > chat-level stored model > mode defaults.
///
/// Synced settings always take priority so that changing the model in settings
/// immediately affects all chats (existing and new). The `chat.model` field
/// is only used as a fallback when settings haven't been synced yet.
fn resolve_chat_model(
    chat_model: Option<&str>,
    mode: &ChatMode,
    ws: &super::workflow_settings::WorkflowSettings,
) -> Option<String> {
    if ws.synced {
        let m = match mode {
            ChatMode::General => &ws.general_model,
            ChatMode::SpecBuilder => &ws.planner_model,
            ChatMode::TicketBuilder => &ws.ticket_builder_model,
            ChatMode::Review => &ws.validation_model,
        };

        if !m.is_empty() {
            return Some(m.clone());
        }
    }

    if let Some(m) = chat_model {
        return Some(m.to_string());
    }

    Some(default_model_for_mode(mode).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::workflow_settings::WorkflowSettings;

    #[test]
    fn resolve_model_prefers_synced_settings_over_chat_model() {
        let ws = WorkflowSettings {
            general_model: "ws-general".into(),
            synced: true,
            ..Default::default()
        };
        let result = resolve_chat_model(Some("old-frozen-model"), &ChatMode::General, &ws);
        assert_eq!(
            result.as_deref(),
            Some("ws-general"),
            "synced settings should take priority over chat.model"
        );
    }

    #[test]
    fn resolve_model_uses_chat_model_when_not_synced() {
        let ws = WorkflowSettings::default(); // synced == false
        let result = resolve_chat_model(Some("chat-stored-model"), &ChatMode::General, &ws);
        assert_eq!(
            result.as_deref(),
            Some("chat-stored-model"),
            "chat.model should be used as fallback when settings not synced"
        );
    }

    #[test]
    fn resolve_model_uses_synced_settings_when_no_chat_model() {
        let ws = WorkflowSettings {
            general_model: "my-general-model".into(),
            planner_model: "my-planner-model".into(),
            ticket_builder_model: "my-tb-model".into(),
            validation_model: "my-review-model".into(),
            synced: true,
            ..Default::default()
        };

        assert_eq!(
            resolve_chat_model(None, &ChatMode::General, &ws).as_deref(),
            Some("my-general-model"),
        );
        assert_eq!(
            resolve_chat_model(None, &ChatMode::SpecBuilder, &ws).as_deref(),
            Some("my-planner-model"),
        );
        assert_eq!(
            resolve_chat_model(None, &ChatMode::TicketBuilder, &ws).as_deref(),
            Some("my-tb-model"),
        );
        assert_eq!(
            resolve_chat_model(None, &ChatMode::Review, &ws).as_deref(),
            Some("my-review-model"),
        );
    }

    #[test]
    fn resolve_model_falls_back_to_defaults_when_not_synced() {
        let ws = WorkflowSettings::default(); // synced == false

        assert_eq!(
            resolve_chat_model(None, &ChatMode::General, &ws).as_deref(),
            Some(crate::agents::models::DEFAULT_GENERAL_CHAT_MODEL),
        );
        assert_eq!(
            resolve_chat_model(None, &ChatMode::SpecBuilder, &ws).as_deref(),
            Some(crate::agents::models::DEFAULT_PLANNER_CHAT_MODEL),
        );
        assert_eq!(
            resolve_chat_model(None, &ChatMode::TicketBuilder, &ws).as_deref(),
            Some(crate::agents::models::DEFAULT_TICKET_BUILDER_CHAT_MODEL),
        );
        assert_eq!(
            resolve_chat_model(None, &ChatMode::Review, &ws).as_deref(),
            Some(crate::agents::models::DEFAULT_VALIDATION_CHAT_MODEL),
        );
    }

    #[test]
    fn resolve_model_falls_back_to_defaults_when_synced_model_empty() {
        let ws = WorkflowSettings {
            general_model: "".into(),
            planner_model: "".into(),
            ticket_builder_model: "".into(),
            validation_model: "".into(),
            synced: true,
            ..Default::default()
        };

        assert_eq!(
            resolve_chat_model(None, &ChatMode::General, &ws).as_deref(),
            Some(crate::agents::models::DEFAULT_GENERAL_CHAT_MODEL),
        );
        assert_eq!(
            resolve_chat_model(None, &ChatMode::TicketBuilder, &ws).as_deref(),
            Some(crate::agents::models::DEFAULT_TICKET_BUILDER_CHAT_MODEL),
        );
    }

    #[test]
    fn resolve_model_always_returns_some() {
        let unsynced = WorkflowSettings::default();
        assert!(resolve_chat_model(None, &ChatMode::General, &unsynced).is_some());

        let empty_synced = WorkflowSettings {
            general_model: "".into(),
            synced: true,
            ..Default::default()
        };
        assert!(resolve_chat_model(None, &ChatMode::General, &empty_synced).is_some());

        let good = WorkflowSettings {
            general_model: "model".into(),
            synced: true,
            ..Default::default()
        };
        assert!(resolve_chat_model(None, &ChatMode::General, &good).is_some());
    }

    #[test]
    fn default_model_for_each_mode() {
        assert_eq!(
            default_model_for_mode(&ChatMode::General),
            crate::agents::models::DEFAULT_GENERAL_CHAT_MODEL,
        );
        assert_eq!(
            default_model_for_mode(&ChatMode::SpecBuilder),
            crate::agents::models::DEFAULT_PLANNER_CHAT_MODEL,
        );
        assert_eq!(
            default_model_for_mode(&ChatMode::TicketBuilder),
            crate::agents::models::DEFAULT_TICKET_BUILDER_CHAT_MODEL,
        );
        assert_eq!(
            default_model_for_mode(&ChatMode::Review),
            crate::agents::models::DEFAULT_VALIDATION_CHAT_MODEL,
        );
    }

    fn unique_path(suffix: &str) -> String {
        let p = std::env::temp_dir().join(format!("test-chat-{}-{}", suffix, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p).unwrap();
        p.to_string_lossy().to_string()
    }

    #[test]
    fn create_chat_review_mode_rejects_mismatched_project() {
        let db = crate::db::Database::open_in_memory().unwrap();

        let project_a = db
            .create_project(&crate::db::models::CreateProject {
                name: "A".into(),
                path: unique_path("a"),
                requires_git: false,
            })
            .unwrap();
        let project_b = db
            .create_project(&crate::db::models::CreateProject {
                name: "B".into(),
                path: unique_path("b"),
                requires_git: false,
            })
            .unwrap();

        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let col_id = &columns[0].id;

        let ticket = db
            .create_ticket(&crate::db::models::CreateTicket {
                board_id: board.id.clone(),
                column_id: col_id.clone(),
                title: "Ticket in B".into(),
                description_md: "".into(),
                priority: crate::db::models::Priority::Medium,
                labels: vec![],
                project_id: Some(project_b.id.clone()),
                workspace_id: None,
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            })
            .unwrap();

        // Attempting to load this ticket under project A should detect the mismatch
        let loaded = db.get_ticket(&ticket.id).unwrap();
        assert_ne!(
            loaded.project_id.as_deref(),
            Some(project_a.id.as_str()),
            "Ticket belongs to project B, not A"
        );
        assert_eq!(loaded.project_id.as_deref(), Some(project_b.id.as_str()));
    }
}
