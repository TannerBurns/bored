use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    RunStarted,
    RunStopped,
    Error,
    Custom(String),
}

impl EventType {
    pub fn as_str(&self) -> String {
        match self {
            EventType::RunStarted => "run_started".to_string(),
            EventType::RunStopped => "run_stopped".to_string(),
            EventType::Error => "error".to_string(),
            EventType::Custom(s) => s.clone(),
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
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
    pub agent_type: String,
    pub event_type: EventType,
    pub payload: AgentEventPayload,
    pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_returns_snake_case() {
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
        assert_eq!(EventType::parse("run_started"), EventType::RunStarted);
        assert_eq!(EventType::parse("error"), EventType::Error);
    }

    #[test]
    fn parse_unknown_returns_custom() {
        let parsed = EventType::parse("custom_event");
        assert_eq!(parsed, EventType::Custom("custom_event".to_string()));
    }

    #[test]
    fn parse_removed_hook_event_types_return_custom() {
        let removed = ["command_requested", "command_executed", "file_read", "file_edited"];
        for name in removed {
            let parsed = EventType::parse(name);
            assert_eq!(parsed, EventType::Custom(name.to_string()),
                "Old hook type '{}' should parse as Custom", name);
        }
    }

    #[test]
    fn parse_roundtrips_through_as_str() {
        let cases = [
            EventType::RunStarted,
            EventType::RunStopped,
            EventType::Error,
            EventType::Custom("log_stdout".to_string()),
            EventType::Custom("log_stderr".to_string()),
        ];
        for event_type in cases {
            let s = event_type.as_str();
            let parsed = EventType::parse(&s);
            assert_eq!(parsed, event_type, "Roundtrip failed for '{}'", s);
        }
    }

    #[test]
    fn event_type_serde_roundtrip() {
        let cases = [
            EventType::RunStarted,
            EventType::RunStopped,
            EventType::Error,
            EventType::Custom("log_stdout".to_string()),
        ];
        for event_type in &cases {
            let json = serde_json::to_string(event_type).unwrap();
            let restored: EventType = serde_json::from_str(&json).unwrap();
            assert_eq!(&restored, event_type, "Serde roundtrip failed for {:?}", event_type);
        }
    }

    #[test]
    fn parse_run_stopped() {
        assert_eq!(EventType::parse("run_stopped"), EventType::RunStopped);
    }

    #[test]
    fn log_event_types_are_custom() {
        let stdout = EventType::Custom("log_stdout".to_string());
        let stderr = EventType::Custom("log_stderr".to_string());
        assert_eq!(stdout.as_str(), "log_stdout");
        assert_eq!(stderr.as_str(), "log_stderr");
    }
}
