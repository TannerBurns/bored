use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::AgentType;

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

#[cfg(test)]
mod tests {
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
        assert_eq!(
            EventType::parse("command_requested"),
            EventType::CommandRequested
        );
        assert_eq!(EventType::parse("file_edited"), EventType::FileEdited);
        assert_eq!(EventType::parse("error"), EventType::Error);
    }

    #[test]
    fn parse_unknown_returns_custom() {
        let parsed = EventType::parse("custom_event");
        assert_eq!(parsed, EventType::Custom("custom_event".to_string()));
    }
}
