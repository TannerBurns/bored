use std::sync::Arc;
use serde::Deserialize;
use tauri::State;

use crate::db::{CreateTicket, Database, Priority, Ticket, AgentPref, UpdateTicket, Comment, CreateComment, AuthorType};

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
    tracing::info!("Creating ticket: {}", ticket.title);
    let create = CreateTicket {
        board_id: ticket.board_id,
        column_id: ticket.column_id,
        title: ticket.title,
        description_md: ticket.description_md,
        priority: ticket.priority,
        labels: ticket.labels,
        project_id: ticket.project_id,
        agent_pref: ticket.agent_pref,
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
    db.move_ticket(&ticket_id, &column_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_ticket(
    ticket_id: String,
    updates: UpdateTicket,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!("Updating ticket: {}", ticket_id);
    db.update_ticket(&ticket_id, &updates)
        .map(|_| ())
        .map_err(|e| e.to_string())
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