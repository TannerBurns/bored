use std::sync::Arc;
use serde::Deserialize;
use tauri::State;

use crate::db::{CreateTicket, Database, Priority, Ticket, AgentPref, UpdateTicket, Comment, CreateComment, AuthorType, WorkflowType, EpicProgress};

/// Input struct for creating tickets via Tauri command.
/// Allows setting is_epic and epic_id at creation time.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTicketInput {
    pub board_id: String,
    pub column_id: String,
    pub title: String,
    pub description_md: String,
    pub priority: Priority,
    pub labels: Vec<String>,
    pub project_id: Option<String>,
    pub agent_pref: Option<AgentPref>,
    #[serde(default)]
    pub workflow_type: Option<WorkflowType>,
    pub model: Option<String>,
    /// Optional pre-defined branch name (if not provided, will be AI-generated on first run)
    pub branch_name: Option<String>,
    /// Whether to create this ticket as an epic
    #[serde(default)]
    pub is_epic: bool,
    /// The parent epic ID (when creating a child ticket)
    pub epic_id: Option<String>,
}

/// Input struct for updating tickets via Tauri command.
/// Excludes is_epic, epic_id, and order_in_epic fields to prevent clients from
/// directly modifying epic relationships. Use dedicated epic commands instead:
/// - add_ticket_to_epic
/// - remove_ticket_from_epic
/// - reorder_epic_children
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTicketInput {
    pub title: Option<String>,
    pub description_md: Option<String>,
    pub priority: Option<Priority>,
    pub labels: Option<Vec<String>>,
    pub project_id: Option<String>,
    pub agent_pref: Option<AgentPref>,
    pub workflow_type: Option<WorkflowType>,
    pub model: Option<String>,
    pub branch_name: Option<String>,
    pub column_id: Option<String>,
}

#[tauri::command]
pub async fn get_tickets(
    board_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<Ticket>, String> {
    tracing::info!("Getting tickets for board: {}", board_id);
    db.get_tickets(&board_id, None).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_ticket(
    ticket: CreateTicketInput,
    db: State<'_, Arc<Database>>,
) -> Result<Ticket, String> {
    tracing::info!("Creating ticket: {} (epic: {})", ticket.title, ticket.is_epic);
    let create = CreateTicket {
        board_id: ticket.board_id,
        column_id: ticket.column_id,
        title: ticket.title,
        description_md: ticket.description_md,
        priority: ticket.priority,
        labels: ticket.labels,
        project_id: ticket.project_id,
        agent_pref: ticket.agent_pref,
        workflow_type: ticket.workflow_type.unwrap_or_default(),
        model: ticket.model,
        branch_name: ticket.branch_name,
        is_epic: ticket.is_epic,
        epic_id: ticket.epic_id,
    };
    db.create_ticket(&create).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn move_ticket(
    ticket_id: String,
    column_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!("Moving ticket {} to column {}", ticket_id, column_id);
    
    // Get the ticket before moving to check if it's an epic
    let ticket = db.get_ticket(&ticket_id).map_err(|e| e.to_string())?;
    
    // Get the target column name
    let columns = db.get_columns(&ticket.board_id).map_err(|e| e.to_string())?;
    let target_column = columns.iter().find(|c| c.id == column_id);
    let target_column_name = target_column.map(|c| c.name.as_str()).unwrap_or("");
    
    // Perform the move
    db.move_ticket(&ticket_id, &column_id).map_err(|e| e.to_string())?;
    
    // Epic lifecycle: when an epic is moved to Ready, advance its first child
    if ticket.is_epic && target_column_name.eq_ignore_ascii_case("Ready") {
        // Refresh ticket after move
        let updated_ticket = db.get_ticket(&ticket_id).map_err(|e| e.to_string())?;
        if let Err(e) = crate::lifecycle::epic::on_epic_moved_to_ready(&db, &updated_ticket) {
            tracing::warn!("Failed to advance epic children: {}", e);
            // Don't fail the move, just log the warning
        }
    }
    
    Ok(())
}

#[tauri::command]
pub async fn update_ticket(
    ticket_id: String,
    updates: UpdateTicketInput,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!("Updating ticket: {}", ticket_id);
    
    // Get the ticket before updating to check for column changes and epic status
    let ticket = db.get_ticket(&ticket_id).map_err(|e| e.to_string())?;
    let old_column_id = ticket.column_id.clone();
    let is_column_changing = updates.column_id.as_ref().map(|new_col| new_col != &old_column_id).unwrap_or(false);
    
    // Convert to UpdateTicket, explicitly setting epic fields to None to prevent
    // clients from modifying epic relationships through this command.
    // Use dedicated epic commands (add_ticket_to_epic, remove_ticket_from_epic,
    // reorder_epic_children) to manage epic associations.
    let update = UpdateTicket {
        title: updates.title,
        description_md: updates.description_md,
        priority: updates.priority,
        labels: updates.labels,
        project_id: updates.project_id,
        agent_pref: updates.agent_pref,
        workflow_type: updates.workflow_type,
        model: updates.model,
        branch_name: updates.branch_name,
        column_id: updates.column_id.clone(),
        is_epic: None,
        epic_id: None,
        order_in_epic: None,
    };
    db.update_ticket(&ticket_id, &update)
        .map(|_| ())
        .map_err(|e| e.to_string())?;
    
    // Epic lifecycle: if an epic is moved to Ready via update, advance its first child
    if is_column_changing && ticket.is_epic {
        if let Some(new_column_id) = updates.column_id {
            // Get the target column name
            let columns = db.get_columns(&ticket.board_id).map_err(|e| e.to_string())?;
            let target_column = columns.iter().find(|c| c.id == new_column_id);
            let target_column_name = target_column.map(|c| c.name.as_str()).unwrap_or("");
            
            if target_column_name.eq_ignore_ascii_case("Ready") {
                // Refresh ticket after update
                let updated_ticket = db.get_ticket(&ticket_id).map_err(|e| e.to_string())?;
                if let Err(e) = crate::lifecycle::epic::on_epic_moved_to_ready(&db, &updated_ticket) {
                    tracing::warn!("Failed to advance epic children on update: {}", e);
                    // Don't fail the update, just log the warning
                }
            }
        }
    }
    
    Ok(())
}

#[tauri::command]
pub async fn delete_ticket(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!("Deleting ticket: {}", ticket_id);
    db.delete_ticket(&ticket_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_comments(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<Comment>, String> {
    tracing::info!("Getting comments for ticket: {}", ticket_id);
    db.get_comments(&ticket_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_comment(
    ticket_id: String,
    body: String,
    author_type: String,
    db: State<'_, Arc<Database>>,
) -> Result<Comment, String> {
    tracing::info!("Adding comment to ticket: {}", ticket_id);
    let author = match author_type.as_str() {
        "user" => AuthorType::User,
        "system" => AuthorType::System,
        _ => AuthorType::Agent,
    };
    let create = CreateComment {
        ticket_id,
        author_type: author,
        body_md: body,
        metadata: None,
    };
    db.create_comment(&create).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_comment(
    comment_id: String,
    body: String,
    db: State<'_, Arc<Database>>,
) -> Result<Comment, String> {
    tracing::info!("Updating comment: {}", comment_id);
    db.update_comment(&comment_id, &body).map_err(|e| e.to_string())
}

// ===== Epic Commands =====

#[tauri::command]
pub async fn get_epic_children(
    epic_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<Ticket>, String> {
    tracing::info!("Getting children for epic: {}", epic_id);
    db.get_epic_children(&epic_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_epic_progress(
    epic_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<EpicProgress, String> {
    tracing::info!("Getting progress for epic: {}", epic_id);
    db.get_epic_progress(&epic_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_ticket_to_epic(
    epic_id: String,
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!("Adding ticket {} to epic {}", ticket_id, epic_id);
    db.add_ticket_to_epic(&epic_id, &ticket_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_ticket_from_epic(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!("Removing ticket {} from epic", ticket_id);
    db.remove_ticket_from_epic(&ticket_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reorder_epic_children(
    epic_id: String,
    child_ids: Vec<String>,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!("Reordering children for epic {}: {:?}", epic_id, child_ids);
    db.reorder_epic_children(&epic_id, &child_ids).map_err(|e| e.to_string())
}