use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChatMode {
    #[default]
    General,
    SpecBuilder,
    TicketBuilder,
    Review,
}

impl ChatMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChatMode::General => "general",
            ChatMode::SpecBuilder => "spec_builder",
            ChatMode::TicketBuilder => "ticket_builder",
            ChatMode::Review => "review",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "general" => Some(ChatMode::General),
            "spec_builder" => Some(ChatMode::SpecBuilder),
            "ticket_builder" => Some(ChatMode::TicketBuilder),
            "review" => Some(ChatMode::Review),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChatStatus {
    #[default]
    Active,
    Thinking,
    Completed,
    Error,
}

impl ChatStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChatStatus::Active => "active",
            ChatStatus::Thinking => "thinking",
            ChatStatus::Completed => "completed",
            ChatStatus::Error => "error",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "active" => Some(ChatStatus::Active),
            "thinking" => Some(ChatStatus::Thinking),
            "completed" => Some(ChatStatus::Completed),
            "error" => Some(ChatStatus::Error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChatRunStatus {
    #[default]
    Running,
    Finished,
    Error,
}

impl ChatRunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChatRunStatus::Running => "running",
            ChatRunStatus::Finished => "finished",
            ChatRunStatus::Error => "error",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "running" => Some(ChatRunStatus::Running),
            "finished" => Some(ChatRunStatus::Finished),
            "error" => Some(ChatRunStatus::Error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ChatMessageRole {
    User,
    Assistant,
    System,
}

impl ChatMessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChatMessageRole::User => "user",
            ChatMessageRole::Assistant => "assistant",
            ChatMessageRole::System => "system",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "user" => Some(ChatMessageRole::User),
            "assistant" => Some(ChatMessageRole::Assistant),
            "system" => Some(ChatMessageRole::System),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chat {
    pub id: String,
    pub title: Option<String>,
    pub agent_type: String,
    pub project_id: Option<String>,
    pub workspace_id: Option<String>,
    pub mode: ChatMode,
    pub board_id: Option<String>,
    pub ticket_id: Option<String>,
    pub spec_id: Option<String>,
    pub model: Option<String>,
    pub status: ChatStatus,
    /// CLI session ID from the agent provider, used to resume conversations via --resume.
    #[serde(skip_serializing)]
    pub agent_session_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateChat {
    pub agent_type: String,
    pub project_id: Option<String>,
    pub workspace_id: Option<String>,
    pub mode: ChatMode,
    pub board_id: Option<String>,
    pub ticket_id: Option<String>,
    pub spec_id: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: String,
    pub chat_id: String,
    pub role: ChatMessageRole,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatEvent {
    pub id: String,
    pub chat_id: String,
    pub message_id: Option<String>,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRun {
    pub id: String,
    pub chat_id: String,
    pub chat_message_id: Option<String>,
    pub agent_type: String,
    pub status: ChatRunStatus,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    mod chat_mode_tests {
        use super::*;

        #[test]
        fn as_str_returns_snake_case() {
            assert_eq!(ChatMode::General.as_str(), "general");
            assert_eq!(ChatMode::SpecBuilder.as_str(), "spec_builder");
            assert_eq!(ChatMode::TicketBuilder.as_str(), "ticket_builder");
            assert_eq!(ChatMode::Review.as_str(), "review");
        }

        #[test]
        fn parse_valid_values() {
            assert_eq!(ChatMode::parse("general"), Some(ChatMode::General));
            assert_eq!(ChatMode::parse("spec_builder"), Some(ChatMode::SpecBuilder));
            assert_eq!(
                ChatMode::parse("ticket_builder"),
                Some(ChatMode::TicketBuilder)
            );
            assert_eq!(ChatMode::parse("review"), Some(ChatMode::Review));
        }

        #[test]
        fn parse_invalid_returns_none() {
            assert_eq!(ChatMode::parse(""), None);
            assert_eq!(ChatMode::parse("invalid"), None);
            assert_eq!(ChatMode::parse("GENERAL"), None);
        }

        #[test]
        fn default_is_general() {
            assert_eq!(ChatMode::default(), ChatMode::General);
        }

        #[test]
        fn roundtrip_as_str_parse() {
            for mode in [
                ChatMode::General,
                ChatMode::SpecBuilder,
                ChatMode::TicketBuilder,
                ChatMode::Review,
            ] {
                assert_eq!(ChatMode::parse(mode.as_str()), Some(mode));
            }
        }
    }

    mod chat_status_tests {
        use super::*;

        #[test]
        fn as_str_returns_snake_case() {
            assert_eq!(ChatStatus::Active.as_str(), "active");
            assert_eq!(ChatStatus::Thinking.as_str(), "thinking");
            assert_eq!(ChatStatus::Completed.as_str(), "completed");
            assert_eq!(ChatStatus::Error.as_str(), "error");
        }

        #[test]
        fn parse_valid_values() {
            assert_eq!(ChatStatus::parse("active"), Some(ChatStatus::Active));
            assert_eq!(ChatStatus::parse("thinking"), Some(ChatStatus::Thinking));
            assert_eq!(ChatStatus::parse("completed"), Some(ChatStatus::Completed));
            assert_eq!(ChatStatus::parse("error"), Some(ChatStatus::Error));
        }

        #[test]
        fn parse_invalid_returns_none() {
            assert_eq!(ChatStatus::parse(""), None);
            assert_eq!(ChatStatus::parse("invalid"), None);
        }

        #[test]
        fn default_is_active() {
            assert_eq!(ChatStatus::default(), ChatStatus::Active);
        }

        #[test]
        fn roundtrip_as_str_parse() {
            for status in [
                ChatStatus::Active,
                ChatStatus::Thinking,
                ChatStatus::Completed,
                ChatStatus::Error,
            ] {
                assert_eq!(ChatStatus::parse(status.as_str()), Some(status));
            }
        }
    }

    mod chat_run_status_tests {
        use super::*;

        #[test]
        fn as_str_returns_snake_case() {
            assert_eq!(ChatRunStatus::Running.as_str(), "running");
            assert_eq!(ChatRunStatus::Finished.as_str(), "finished");
            assert_eq!(ChatRunStatus::Error.as_str(), "error");
        }

        #[test]
        fn parse_valid_values() {
            assert_eq!(
                ChatRunStatus::parse("running"),
                Some(ChatRunStatus::Running)
            );
            assert_eq!(
                ChatRunStatus::parse("finished"),
                Some(ChatRunStatus::Finished)
            );
            assert_eq!(ChatRunStatus::parse("error"), Some(ChatRunStatus::Error));
        }

        #[test]
        fn roundtrip_as_str_parse() {
            for status in [
                ChatRunStatus::Running,
                ChatRunStatus::Finished,
                ChatRunStatus::Error,
            ] {
                assert_eq!(ChatRunStatus::parse(status.as_str()), Some(status));
            }
        }
    }

    mod chat_message_role_tests {
        use super::*;

        #[test]
        fn as_str_returns_lowercase() {
            assert_eq!(ChatMessageRole::User.as_str(), "user");
            assert_eq!(ChatMessageRole::Assistant.as_str(), "assistant");
            assert_eq!(ChatMessageRole::System.as_str(), "system");
        }

        #[test]
        fn parse_valid_values() {
            assert_eq!(
                ChatMessageRole::parse("user"),
                Some(ChatMessageRole::User)
            );
            assert_eq!(
                ChatMessageRole::parse("assistant"),
                Some(ChatMessageRole::Assistant)
            );
            assert_eq!(
                ChatMessageRole::parse("system"),
                Some(ChatMessageRole::System)
            );
        }

        #[test]
        fn roundtrip_as_str_parse() {
            for role in [
                ChatMessageRole::User,
                ChatMessageRole::Assistant,
                ChatMessageRole::System,
            ] {
                assert_eq!(ChatMessageRole::parse(role.as_str()), Some(role));
            }
        }
    }
}
