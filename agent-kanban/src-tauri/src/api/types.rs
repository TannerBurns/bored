use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::db::{Priority, AgentType, AgentPref, Ticket, Column};

// ===== Ticket Types =====

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTicketRequest {
    pub board_id: String,
    pub column_id: String,
    pub title: String,
    #[serde(default)]
    pub description_md: String,
    #[serde(default = "default_priority")]
    pub priority: Priority,
    #[serde(default)]
    pub labels: Vec<String>,
    pub project_id: Option<String>,
    pub agent_pref: Option<AgentPref>,
}

fn default_priority() -> Priority {
    Priority::Medium
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTicketRequest {
    pub title: Option<String>,
    pub description_md: Option<String>,
    pub priority: Option<Priority>,
    pub labels: Option<Vec<String>>,
    pub project_id: Option<String>,
    pub agent_pref: Option<AgentPref>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveTicketRequest {
    pub column_id: String,
}

// ===== Reservation Types =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReservationResponse {
    pub run_id: String,
    pub ticket_id: String,
    pub lock_expires_at: DateTime<Utc>,
    pub heartbeat_interval_secs: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReserveTicketRequest {
    pub agent_type: AgentType,
    #[serde(default)]
    pub repo_path: Option<String>,
}

// ===== Run Types =====

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRunRequest {
    pub ticket_id: String,
    pub agent_type: AgentType,
    pub repo_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRunRequest {
    pub status: Option<String>,
    pub exit_code: Option<i32>,
    pub summary_md: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeartbeatResponse {
    pub run_id: String,
    pub lock_expires_at: DateTime<Utc>,
    pub ok: bool,
}

// ===== Event Types =====

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEventRequest {
    pub event_type: String,
    pub payload: serde_json::Value,
    #[serde(default = "Utc::now")]
    pub timestamp: DateTime<Utc>,
}

// ===== Comment Types =====

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCommentRequest {
    pub body_md: String,
    #[serde(default = "default_author_type")]
    pub author_type: String,
    pub metadata: Option<serde_json::Value>,
}

fn default_author_type() -> String {
    "agent".to_string()
}

// ===== Queue Types =====

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueNextRequest {
    pub agent_type: AgentType,
    #[serde(default)]
    pub repo_path: Option<String>,
    #[serde(default)]
    pub board_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueNextResponse {
    pub ticket: Ticket,
    pub run_id: String,
    pub lock_expires_at: DateTime<Utc>,
    pub heartbeat_interval_secs: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueStatusResponse {
    pub ready_count: usize,
    pub in_progress_count: usize,
    pub boards: Vec<BoardQueueStatus>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardQueueStatus {
    pub board_id: String,
    pub board_name: String,
    pub ready_count: usize,
}

// ===== Board Response Types =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardWithColumns {
    pub id: String,
    pub name: String,
    pub default_project_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub columns: Vec<Column>,
}

// ===== Generic Responses =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResponse {
    pub deleted: bool,
    pub id: String,
}

pub const LOCK_DURATION_MINUTES: i64 = 30;
pub const HEARTBEAT_INTERVAL_SECS: u64 = 60;
