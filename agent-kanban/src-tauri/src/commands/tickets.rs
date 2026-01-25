use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    Medium,
    High,
    Urgent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AgentPref {
    Cursor,
    Claude,
    Any,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Ticket {
    pub id: String,
    pub board_id: String,
    pub column_id: String,
    pub title: String,
    pub description_md: String,
    pub priority: Priority,
    pub labels: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub locked_by_run_id: Option<String>,
    pub lock_expires_at: Option<DateTime<Utc>>,
    pub repo_path: Option<String>,
    pub agent_pref: Option<AgentPref>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTicketInput {
    pub board_id: String,
    pub column_id: String,
    pub title: String,
    pub description_md: String,
    pub priority: Priority,
    pub labels: Vec<String>,
    pub repo_path: Option<String>,
    pub agent_pref: Option<AgentPref>,
}

impl Ticket {
    pub fn from_input(input: CreateTicketInput) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            board_id: input.board_id,
            column_id: input.column_id,
            title: input.title,
            description_md: input.description_md,
            priority: input.priority,
            labels: input.labels,
            created_at: now,
            updated_at: now,
            locked_by_run_id: None,
            lock_expires_at: None,
            repo_path: input.repo_path,
            agent_pref: input.agent_pref,
        }
    }
}

#[tauri::command]
pub async fn get_tickets(board_id: String) -> Result<Vec<Ticket>, String> {
    tracing::info!("Getting tickets for board: {}", board_id);
    Ok(vec![])
}

#[tauri::command]
pub async fn create_ticket(ticket: CreateTicketInput) -> Result<Ticket, String> {
    tracing::info!("Creating ticket: {}", ticket.title);
    Ok(Ticket::from_input(ticket))
}

#[tauri::command]
pub async fn move_ticket(ticket_id: String, column_id: String) -> Result<(), String> {
    tracing::info!("Moving ticket {} to column {}", ticket_id, column_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_input() -> CreateTicketInput {
        CreateTicketInput {
            board_id: "board-1".to_string(),
            column_id: "col-1".to_string(),
            title: "Test Ticket".to_string(),
            description_md: "Description".to_string(),
            priority: Priority::Medium,
            labels: vec!["bug".to_string()],
            repo_path: Some("/path/to/repo".to_string()),
            agent_pref: Some(AgentPref::Cursor),
        }
    }

    #[test]
    fn ticket_from_input_copies_fields() {
        let input = sample_input();
        let ticket = Ticket::from_input(input.clone());
        
        assert_eq!(ticket.board_id, input.board_id);
        assert_eq!(ticket.column_id, input.column_id);
        assert_eq!(ticket.title, input.title);
        assert_eq!(ticket.description_md, input.description_md);
        assert_eq!(ticket.priority, input.priority);
        assert_eq!(ticket.labels, input.labels);
        assert_eq!(ticket.repo_path, input.repo_path);
        assert_eq!(ticket.agent_pref, input.agent_pref);
    }

    #[test]
    fn ticket_from_input_generates_uuid() {
        let ticket = Ticket::from_input(sample_input());
        assert!(uuid::Uuid::parse_str(&ticket.id).is_ok());
    }

    #[test]
    fn ticket_from_input_sets_timestamps() {
        let before = Utc::now();
        let ticket = Ticket::from_input(sample_input());
        let after = Utc::now();
        
        assert!(ticket.created_at >= before && ticket.created_at <= after);
        assert_eq!(ticket.created_at, ticket.updated_at);
    }

    #[test]
    fn ticket_from_input_initializes_optional_fields_as_none() {
        let ticket = Ticket::from_input(sample_input());
        assert!(ticket.locked_by_run_id.is_none());
        assert!(ticket.lock_expires_at.is_none());
    }

    #[test]
    fn priority_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&Priority::Low).unwrap(), "\"low\"");
        assert_eq!(serde_json::to_string(&Priority::High).unwrap(), "\"high\"");
        assert_eq!(serde_json::to_string(&Priority::Urgent).unwrap(), "\"urgent\"");
    }

    #[test]
    fn priority_deserializes_lowercase() {
        assert_eq!(serde_json::from_str::<Priority>("\"low\"").unwrap(), Priority::Low);
        assert_eq!(serde_json::from_str::<Priority>("\"medium\"").unwrap(), Priority::Medium);
    }

    #[test]
    fn agent_pref_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&AgentPref::Cursor).unwrap(), "\"cursor\"");
        assert_eq!(serde_json::to_string(&AgentPref::Claude).unwrap(), "\"claude\"");
        assert_eq!(serde_json::to_string(&AgentPref::Any).unwrap(), "\"any\"");
    }
}
