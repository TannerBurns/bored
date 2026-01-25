use std::sync::Arc;
use serde::Deserialize;
use tauri::State;

use crate::db::{CreateTicket, Database, Priority, Ticket, AgentPref, UpdateTicket};

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