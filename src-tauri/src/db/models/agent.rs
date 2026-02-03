use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AgentType {
    #[default]
    Cursor,
    Claude,
}

impl AgentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentType::Cursor => "cursor",
            AgentType::Claude => "claude",
        }
    }
}

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

/// An agent run with additional context for display (board, project, ticket info)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunWithContext {
    #[serde(flatten)]
    pub run: AgentRun,
    /// The ticket title
    pub ticket_title: String,
    /// The board ID this run's ticket belongs to
    pub board_id: String,
    /// The board name
    pub board_name: String,
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
}
