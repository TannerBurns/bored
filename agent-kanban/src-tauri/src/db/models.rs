use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub cursor_hooks_installed: bool,
    pub claude_hooks_installed: bool,
    pub preferred_agent: Option<AgentPref>,
    pub allow_shell_commands: bool,
    pub allow_file_writes: bool,
    pub blocked_patterns: Vec<String>,
    pub settings: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProject {
    pub name: String,
    pub path: String,
    pub preferred_agent: Option<AgentPref>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProject {
    pub name: Option<String>,
    pub preferred_agent: Option<AgentPref>,
    pub allow_shell_commands: Option<bool>,
    pub allow_file_writes: Option<bool>,
    pub blocked_patterns: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Board {
    pub id: String,
    pub name: String,
    pub default_project_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Column {
    pub id: String,
    pub board_id: String,
    pub name: String,
    pub position: i32,
    pub wip_limit: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    Medium,
    High,
    Urgent,
}

impl Priority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Priority::Low => "low",
            Priority::Medium => "medium",
            Priority::High => "high",
            Priority::Urgent => "urgent",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "low" => Some(Priority::Low),
            "medium" => Some(Priority::Medium),
            "high" => Some(Priority::High),
            "urgent" => Some(Priority::Urgent),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AgentPref {
    Cursor,
    Claude,
    Any,
}

impl AgentPref {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentPref::Cursor => "cursor",
            AgentPref::Claude => "claude",
            AgentPref::Any => "any",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "cursor" => Some(AgentPref::Cursor),
            "claude" => Some(AgentPref::Claude),
            "any" => Some(AgentPref::Any),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    pub project_id: Option<String>,
    pub agent_pref: Option<AgentPref>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AuthorType {
    User,
    Agent,
    System,
}

impl AuthorType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthorType::User => "user",
            AuthorType::Agent => "agent",
            AuthorType::System => "system",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: String,
    pub ticket_id: String,
    pub author_type: AuthorType,
    pub body_md: String,
    pub created_at: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AgentType {
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
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunStatus::Queued => "queued",
            RunStatus::Running => "running",
            RunStatus::Finished => "finished",
            RunStatus::Error => "error",
            RunStatus::Aborted => "aborted",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "queued" => Some(RunStatus::Queued),
            "running" => Some(RunStatus::Running),
            "finished" => Some(RunStatus::Finished),
            "error" => Some(RunStatus::Error),
            "aborted" => Some(RunStatus::Aborted),
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    CommandRequested,
    CommandExecuted,
    FileRead,
    FileEdited,
    RunStarted,
    RunStopped,
    Error,
    Custom(String),
}

impl EventType {
    pub fn as_str(&self) -> String {
        match self {
            EventType::CommandRequested => "command_requested".to_string(),
            EventType::CommandExecuted => "command_executed".to_string(),
            EventType::FileRead => "file_read".to_string(),
            EventType::FileEdited => "file_edited".to_string(),
            EventType::RunStarted => "run_started".to_string(),
            EventType::RunStopped => "run_stopped".to_string(),
            EventType::Error => "error".to_string(),
            EventType::Custom(s) => s.clone(),
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "command_requested" => EventType::CommandRequested,
            "command_executed" => EventType::CommandExecuted,
            "file_read" => EventType::FileRead,
            "file_edited" => EventType::FileEdited,
            "run_started" => EventType::RunStarted,
            "run_stopped" => EventType::RunStopped,
            "error" => EventType::Error,
            other => EventType::Custom(other.to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEvent {
    pub id: String,
    pub run_id: String,
    pub ticket_id: String,
    pub event_type: EventType,
    pub payload: AgentEventPayload,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEventPayload {
    pub raw: Option<String>,
    pub structured: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedEvent {
    pub run_id: String,
    pub ticket_id: String,
    pub agent_type: AgentType,
    pub event_type: EventType,
    pub payload: AgentEventPayload,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTicket {
    pub board_id: String,
    pub column_id: String,
    pub title: String,
    pub description_md: String,
    pub priority: Priority,
    pub labels: Vec<String>,
    pub project_id: Option<String>,
    pub agent_pref: Option<AgentPref>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRun {
    pub ticket_id: String,
    pub agent_type: AgentType,
    pub repo_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReadinessCheck {
    Ready { project_id: String },
    /// Serializes as `{ "noProject": null }` to match TypeScript discriminated union
    NoProject(Option<()>),
    /// Serializes as `{ "projectNotFound": null }` to match TypeScript discriminated union
    ProjectNotFound(Option<()>),
    ProjectPathMissing { path: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    mod priority_tests {
        use super::*;

        #[test]
        fn as_str_returns_lowercase() {
            assert_eq!(Priority::Low.as_str(), "low");
            assert_eq!(Priority::Medium.as_str(), "medium");
            assert_eq!(Priority::High.as_str(), "high");
            assert_eq!(Priority::Urgent.as_str(), "urgent");
        }

        #[test]
        fn parse_valid_values() {
            assert_eq!(Priority::parse("low"), Some(Priority::Low));
            assert_eq!(Priority::parse("medium"), Some(Priority::Medium));
            assert_eq!(Priority::parse("high"), Some(Priority::High));
            assert_eq!(Priority::parse("urgent"), Some(Priority::Urgent));
        }

        #[test]
        fn parse_invalid_returns_none() {
            assert_eq!(Priority::parse(""), None);
            assert_eq!(Priority::parse("invalid"), None);
            assert_eq!(Priority::parse("LOW"), None);
        }

        #[test]
        fn roundtrip_as_str_parse() {
            for p in [Priority::Low, Priority::Medium, Priority::High, Priority::Urgent] {
                assert_eq!(Priority::parse(p.as_str()), Some(p));
            }
        }
    }

    mod agent_pref_tests {
        use super::*;

        #[test]
        fn as_str_returns_lowercase() {
            assert_eq!(AgentPref::Cursor.as_str(), "cursor");
            assert_eq!(AgentPref::Claude.as_str(), "claude");
            assert_eq!(AgentPref::Any.as_str(), "any");
        }

        #[test]
        fn parse_valid_values() {
            assert_eq!(AgentPref::parse("cursor"), Some(AgentPref::Cursor));
            assert_eq!(AgentPref::parse("claude"), Some(AgentPref::Claude));
            assert_eq!(AgentPref::parse("any"), Some(AgentPref::Any));
        }

        #[test]
        fn parse_invalid_returns_none() {
            assert_eq!(AgentPref::parse(""), None);
            assert_eq!(AgentPref::parse("other"), None);
        }
    }

    mod run_status_tests {
        use super::*;

        #[test]
        fn as_str_returns_lowercase() {
            assert_eq!(RunStatus::Queued.as_str(), "queued");
            assert_eq!(RunStatus::Running.as_str(), "running");
            assert_eq!(RunStatus::Finished.as_str(), "finished");
            assert_eq!(RunStatus::Error.as_str(), "error");
            assert_eq!(RunStatus::Aborted.as_str(), "aborted");
        }

        #[test]
        fn parse_valid_values() {
            assert_eq!(RunStatus::parse("queued"), Some(RunStatus::Queued));
            assert_eq!(RunStatus::parse("running"), Some(RunStatus::Running));
            assert_eq!(RunStatus::parse("finished"), Some(RunStatus::Finished));
            assert_eq!(RunStatus::parse("error"), Some(RunStatus::Error));
            assert_eq!(RunStatus::parse("aborted"), Some(RunStatus::Aborted));
        }

        #[test]
        fn parse_invalid_returns_none() {
            assert_eq!(RunStatus::parse("unknown"), None);
        }
    }

    mod event_type_tests {
        use super::*;

        #[test]
        fn as_str_returns_snake_case() {
            assert_eq!(EventType::CommandRequested.as_str(), "command_requested");
            assert_eq!(EventType::CommandExecuted.as_str(), "command_executed");
            assert_eq!(EventType::FileRead.as_str(), "file_read");
            assert_eq!(EventType::FileEdited.as_str(), "file_edited");
            assert_eq!(EventType::RunStarted.as_str(), "run_started");
            assert_eq!(EventType::RunStopped.as_str(), "run_stopped");
            assert_eq!(EventType::Error.as_str(), "error");
        }

        #[test]
        fn as_str_custom_returns_inner_value() {
            let custom = EventType::Custom("my_event".to_string());
            assert_eq!(custom.as_str(), "my_event");
        }

        #[test]
        fn parse_known_values() {
            assert_eq!(EventType::parse("command_requested"), EventType::CommandRequested);
            assert_eq!(EventType::parse("file_edited"), EventType::FileEdited);
            assert_eq!(EventType::parse("error"), EventType::Error);
        }

        #[test]
        fn parse_unknown_returns_custom() {
            let parsed = EventType::parse("custom_event");
            assert_eq!(parsed, EventType::Custom("custom_event".to_string()));
        }
    }

    mod serialization_tests {
        use super::*;

        #[test]
        fn project_serializes_to_camel_case() {
            let project = Project {
                id: "p1".to_string(),
                name: "Test".to_string(),
                path: "/tmp".to_string(),
                cursor_hooks_installed: true,
                claude_hooks_installed: false,
                preferred_agent: Some(AgentPref::Cursor),
                allow_shell_commands: true,
                allow_file_writes: false,
                blocked_patterns: vec!["*.log".to_string()],
                settings: serde_json::json!({}),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            let json = serde_json::to_string(&project).unwrap();
            assert!(json.contains("\"cursorHooksInstalled\":true"));
            assert!(json.contains("\"allowFileWrites\":false"));
            assert!(json.contains("\"preferredAgent\":\"cursor\""));
        }

        #[test]
        fn create_project_deserializes_from_camel_case() {
            let json = r#"{"name":"Proj","path":"/tmp","preferredAgent":"claude"}"#;
            let input: CreateProject = serde_json::from_str(json).unwrap();
            assert_eq!(input.name, "Proj");
            assert_eq!(input.preferred_agent, Some(AgentPref::Claude));
        }

        #[test]
        fn readiness_check_serializes_variants() {
            let ready = ReadinessCheck::Ready { project_id: "p1".to_string() };
            let json = serde_json::to_string(&ready).unwrap();
            assert!(json.contains("ready"));

            let missing = ReadinessCheck::ProjectPathMissing { path: "/gone".to_string() };
            let json = serde_json::to_string(&missing).unwrap();
            assert!(json.contains("projectPathMissing"));
        }
    }
}
