use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AgentType {
    Cursor,
    Claude,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Queued,
    Running,
    Finished,
    Error,
    Aborted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentRun {
    pub id: String,
    pub ticket_id: String,
    pub agent_type: AgentType,
    pub repo_path: String,
    pub status: RunStatus,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
    pub summary_md: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

impl AgentRun {
    pub fn new(ticket_id: String, agent_type: AgentType, repo_path: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            ticket_id,
            agent_type,
            repo_path,
            status: RunStatus::Queued,
            started_at: Utc::now(),
            ended_at: None,
            exit_code: None,
            summary_md: None,
            metadata: None,
        }
    }
}

#[tauri::command]
pub async fn start_agent_run(
    ticket_id: String,
    agent_type: AgentType,
    repo_path: String,
) -> Result<AgentRun, String> {
    tracing::info!("Starting {:?} agent run for ticket: {}", agent_type, ticket_id);
    Ok(AgentRun::new(ticket_id, agent_type, repo_path))
}

#[tauri::command]
pub async fn get_agent_runs(ticket_id: String) -> Result<Vec<AgentRun>, String> {
    tracing::info!("Getting agent runs for ticket: {}", ticket_id);
    Ok(vec![])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_run_new_sets_fields() {
        let run = AgentRun::new(
            "ticket-1".to_string(),
            AgentType::Cursor,
            "/path/to/repo".to_string(),
        );
        
        assert_eq!(run.ticket_id, "ticket-1");
        assert_eq!(run.agent_type, AgentType::Cursor);
        assert_eq!(run.repo_path, "/path/to/repo");
    }

    #[test]
    fn agent_run_new_starts_queued() {
        let run = AgentRun::new("t".to_string(), AgentType::Claude, "/".to_string());
        assert_eq!(run.status, RunStatus::Queued);
    }

    #[test]
    fn agent_run_new_generates_uuid() {
        let run = AgentRun::new("t".to_string(), AgentType::Cursor, "/".to_string());
        assert!(uuid::Uuid::parse_str(&run.id).is_ok());
    }

    #[test]
    fn agent_run_new_sets_started_at() {
        let before = Utc::now();
        let run = AgentRun::new("t".to_string(), AgentType::Cursor, "/".to_string());
        let after = Utc::now();
        
        assert!(run.started_at >= before && run.started_at <= after);
    }

    #[test]
    fn agent_run_new_leaves_optional_fields_none() {
        let run = AgentRun::new("t".to_string(), AgentType::Cursor, "/".to_string());
        assert!(run.ended_at.is_none());
        assert!(run.exit_code.is_none());
        assert!(run.summary_md.is_none());
        assert!(run.metadata.is_none());
    }

    #[test]
    fn agent_type_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&AgentType::Cursor).unwrap(), "\"cursor\"");
        assert_eq!(serde_json::to_string(&AgentType::Claude).unwrap(), "\"claude\"");
    }

    #[test]
    fn run_status_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&RunStatus::Queued).unwrap(), "\"queued\"");
        assert_eq!(serde_json::to_string(&RunStatus::Running).unwrap(), "\"running\"");
        assert_eq!(serde_json::to_string(&RunStatus::Finished).unwrap(), "\"finished\"");
        assert_eq!(serde_json::to_string(&RunStatus::Error).unwrap(), "\"error\"");
        assert_eq!(serde_json::to_string(&RunStatus::Aborted).unwrap(), "\"aborted\"");
    }
}
