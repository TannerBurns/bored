use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Free-form agent identifier (e.g. "cursor", "claude").
pub type AgentType = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Queued,
    Running,
    Finished,
    Error,
    Aborted,
    /// The run was paused by the user - can be resumed later
    Paused,
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunStatus::Queued => "queued",
            RunStatus::Running => "running",
            RunStatus::Finished => "finished",
            RunStatus::Error => "error",
            RunStatus::Aborted => "aborted",
            RunStatus::Paused => "paused",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "queued" => Some(RunStatus::Queued),
            "running" => Some(RunStatus::Running),
            "finished" => Some(RunStatus::Finished),
            "error" => Some(RunStatus::Error),
            "aborted" => Some(RunStatus::Aborted),
            "paused" => Some(RunStatus::Paused),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    /// For sub-runs: the parent run ID
    pub parent_run_id: Option<String>,
    /// For sub-runs: the stage name (e.g., "branch", "plan", "implement", "deslop")
    pub stage: Option<String>,
    /// For resumed runs: the ID of the run this is resuming from
    pub resumed_from_run_id: Option<String>,
}

/// An agent run with additional context for display (board, project, ticket info).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunWithContext {
    #[serde(flatten)]
    pub run: AgentRun,
    /// The ticket title
    pub ticket_title: Option<String>,
    /// The board ID this run's ticket belongs to
    pub board_id: Option<String>,
    /// The board name
    pub board_name: Option<String>,
    /// The project ID (if the ticket has one)
    pub project_id: Option<String>,
    /// The project name (if the ticket has one)
    pub project_name: Option<String>,
    /// The current stage name for multi-stage workflows (if running)
    pub current_stage: Option<String>,
    /// Number of completed stages (sub-runs with status = finished)
    pub completed_stages: u32,
    /// Total number of stages (all sub-runs)
    pub total_stages: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CreateRun {
    #[serde(default)]
    pub ticket_id: String,
    #[serde(default)]
    pub agent_type: AgentType,
    #[serde(default)]
    pub repo_path: String,
    /// For sub-runs: the parent run ID
    #[serde(default)]
    pub parent_run_id: Option<String>,
    /// For sub-runs: the stage name
    #[serde(default)]
    pub stage: Option<String>,
    /// For resumed runs: the ID of the run this is resuming from
    #[serde(default)]
    pub resumed_from_run_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    mod run_status_tests {
        use super::*;

        #[test]
        fn as_str_returns_lowercase() {
            assert_eq!(RunStatus::Queued.as_str(), "queued");
            assert_eq!(RunStatus::Running.as_str(), "running");
            assert_eq!(RunStatus::Finished.as_str(), "finished");
            assert_eq!(RunStatus::Error.as_str(), "error");
            assert_eq!(RunStatus::Aborted.as_str(), "aborted");
            assert_eq!(RunStatus::Paused.as_str(), "paused");
        }

        #[test]
        fn parse_valid_values() {
            assert_eq!(RunStatus::parse("queued"), Some(RunStatus::Queued));
            assert_eq!(RunStatus::parse("running"), Some(RunStatus::Running));
            assert_eq!(RunStatus::parse("finished"), Some(RunStatus::Finished));
            assert_eq!(RunStatus::parse("error"), Some(RunStatus::Error));
            assert_eq!(RunStatus::parse("aborted"), Some(RunStatus::Aborted));
            assert_eq!(RunStatus::parse("paused"), Some(RunStatus::Paused));
        }

        #[test]
        fn parse_invalid_returns_none() {
            assert_eq!(RunStatus::parse("unknown"), None);
        }
    }

    mod agent_type_tests {
        use super::*;

        #[test]
        fn agent_type_is_string() {
            let agent: AgentType = "cursor".to_string();
            assert_eq!(agent, "cursor");
        }

        #[test]
        fn serializes_as_string() {
            let agent: AgentType = "claude".to_string();
            assert_eq!(serde_json::to_string(&agent).unwrap(), "\"claude\"");
        }

        #[test]
        fn deserializes_from_string() {
            let agent: AgentType = serde_json::from_str("\"cursor\"").unwrap();
            assert_eq!(agent, "cursor");
        }

        #[test]
        fn accepts_arbitrary_agent_ids() {
            let agent: AgentType = "new-agent".to_string();
            assert_eq!(agent, "new-agent");
        }
    }

    mod agent_run_with_context_tests {
        use super::*;

        fn make_run(ticket_id: &str) -> AgentRun {
            AgentRun {
                id: "run-1".to_string(),
                ticket_id: ticket_id.to_string(),
                agent_type: "claude".to_string(),
                repo_path: "/tmp".to_string(),
                status: RunStatus::Finished,
                started_at: chrono::Utc::now(),
                ended_at: None,
                exit_code: Some(0),
                summary_md: None,
                metadata: None,
                parent_run_id: None,
                stage: Some("planner".to_string()),
                resumed_from_run_id: None,
            }
        }

        #[test]
        fn serializes_with_all_context_fields() {
            let ctx = AgentRunWithContext {
                run: make_run("ticket-1"),
                ticket_title: Some("My Ticket".to_string()),
                board_id: Some("board-1".to_string()),
                board_name: Some("Board".to_string()),
                project_id: Some("proj-1".to_string()),
                project_name: Some("Project".to_string()),
                current_stage: None,
                completed_stages: 0,
                total_stages: 0,
            };
            let json = serde_json::to_value(&ctx).unwrap();
            assert_eq!(json["ticketTitle"], "My Ticket");
            assert_eq!(json["boardId"], "board-1");
            assert_eq!(json["boardName"], "Board");
        }

        #[test]
        fn serializes_with_null_context_fields() {
            let ctx = AgentRunWithContext {
                run: make_run("spec-abc"),
                ticket_title: None,
                board_id: None,
                board_name: None,
                project_id: None,
                project_name: None,
                current_stage: None,
                completed_stages: 0,
                total_stages: 0,
            };
            let json = serde_json::to_value(&ctx).unwrap();
            assert!(json["ticketTitle"].is_null());
            assert!(json["boardId"].is_null());
            assert!(json["boardName"].is_null());
        }

        #[test]
        fn deserializes_with_null_context_fields() {
            let json = serde_json::json!({
                "id": "run-1",
                "ticketId": "spec-abc",
                "agentType": "claude",
                "repoPath": "/tmp",
                "status": "finished",
                "startedAt": "2024-01-01T00:00:00Z",
                "ticketTitle": null,
                "boardId": null,
                "boardName": null,
                "completedStages": 0,
                "totalStages": 0
            });
            let ctx: AgentRunWithContext = serde_json::from_value(json).unwrap();
            assert_eq!(ctx.run.ticket_id, "spec-abc");
            assert_eq!(ctx.ticket_title, None);
            assert_eq!(ctx.board_id, None);
            assert_eq!(ctx.board_name, None);
        }

        #[test]
        fn deserializes_with_missing_context_fields() {
            let json = serde_json::json!({
                "id": "run-1",
                "ticketId": "spec-abc",
                "agentType": "claude",
                "repoPath": "/tmp",
                "status": "finished",
                "startedAt": "2024-01-01T00:00:00Z",
                "completedStages": 0,
                "totalStages": 0
            });
            let ctx: AgentRunWithContext = serde_json::from_value(json).unwrap();
            assert_eq!(ctx.ticket_title, None);
            assert_eq!(ctx.board_id, None);
        }
    }
}
