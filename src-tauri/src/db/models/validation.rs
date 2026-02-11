use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Status of a validation session
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ValidationSessionStatus {
    /// Session created, not yet started
    #[default]
    Created,
    /// User is chatting with validation agent
    Chatting,
    /// App is running as subprocess
    AppRunning,
    /// Validation passed
    Passed,
    /// Validation failed (fix tasks created)
    Failed,
}

impl ValidationSessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ValidationSessionStatus::Created => "created",
            ValidationSessionStatus::Chatting => "chatting",
            ValidationSessionStatus::AppRunning => "app_running",
            ValidationSessionStatus::Passed => "passed",
            ValidationSessionStatus::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "created" => Some(ValidationSessionStatus::Created),
            "chatting" => Some(ValidationSessionStatus::Chatting),
            "app_running" => Some(ValidationSessionStatus::AppRunning),
            "passed" => Some(ValidationSessionStatus::Passed),
            "failed" => Some(ValidationSessionStatus::Failed),
            _ => None,
        }
    }
}

/// A validation session for a completed ticket
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationSession {
    pub id: String,
    pub ticket_id: String,
    pub project_id: Option<String>,
    pub status: ValidationSessionStatus,
    /// Command to start the app (e.g. "npm run dev")
    pub app_command: Option<String>,
    /// Port the app runs on
    pub app_port: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Create a new validation session
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateValidationSession {
    pub ticket_id: String,
    pub project_id: Option<String>,
    pub app_command: Option<String>,
    pub app_port: Option<i32>,
}

/// Update a validation session
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateValidationSession {
    pub status: Option<ValidationSessionStatus>,
    pub app_command: Option<String>,
    pub app_port: Option<i32>,
}

/// A message in a validation chat
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationMessage {
    pub id: String,
    pub session_id: String,
    pub role: ValidationMessageRole,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// Role in a validation message
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ValidationMessageRole {
    User,
    Assistant,
    System,
}

impl ValidationMessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            ValidationMessageRole::User => "user",
            ValidationMessageRole::Assistant => "assistant",
            ValidationMessageRole::System => "system",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "user" => Some(ValidationMessageRole::User),
            "assistant" => Some(ValidationMessageRole::Assistant),
            "system" => Some(ValidationMessageRole::System),
            _ => None,
        }
    }
}

/// Create a new validation message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateValidationMessage {
    pub session_id: String,
    pub role: ValidationMessageRole,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
}

/// A fix task proposed during validation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixTask {
    pub title: String,
    pub description: String,
    pub acceptance_criteria: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    mod validation_session_status_tests {
        use super::*;

        #[test]
        fn as_str_returns_lowercase() {
            assert_eq!(ValidationSessionStatus::Created.as_str(), "created");
            assert_eq!(ValidationSessionStatus::Chatting.as_str(), "chatting");
            assert_eq!(ValidationSessionStatus::AppRunning.as_str(), "app_running");
            assert_eq!(ValidationSessionStatus::Passed.as_str(), "passed");
            assert_eq!(ValidationSessionStatus::Failed.as_str(), "failed");
        }

        #[test]
        fn parse_valid_values() {
            assert_eq!(
                ValidationSessionStatus::parse("created"),
                Some(ValidationSessionStatus::Created)
            );
            assert_eq!(
                ValidationSessionStatus::parse("chatting"),
                Some(ValidationSessionStatus::Chatting)
            );
            assert_eq!(
                ValidationSessionStatus::parse("app_running"),
                Some(ValidationSessionStatus::AppRunning)
            );
            assert_eq!(
                ValidationSessionStatus::parse("passed"),
                Some(ValidationSessionStatus::Passed)
            );
            assert_eq!(
                ValidationSessionStatus::parse("failed"),
                Some(ValidationSessionStatus::Failed)
            );
        }

        #[test]
        fn parse_invalid_returns_none() {
            assert_eq!(ValidationSessionStatus::parse(""), None);
            assert_eq!(ValidationSessionStatus::parse("invalid"), None);
            assert_eq!(ValidationSessionStatus::parse("CREATED"), None);
        }

        #[test]
        fn default_is_created() {
            assert_eq!(
                ValidationSessionStatus::default(),
                ValidationSessionStatus::Created
            );
        }

        #[test]
        fn roundtrip_as_str_parse() {
            for status in [
                ValidationSessionStatus::Created,
                ValidationSessionStatus::Chatting,
                ValidationSessionStatus::AppRunning,
                ValidationSessionStatus::Passed,
                ValidationSessionStatus::Failed,
            ] {
                assert_eq!(
                    ValidationSessionStatus::parse(status.as_str()),
                    Some(status)
                );
            }
        }
    }

    mod validation_message_role_tests {
        use super::*;

        #[test]
        fn as_str_returns_lowercase() {
            assert_eq!(ValidationMessageRole::User.as_str(), "user");
            assert_eq!(ValidationMessageRole::Assistant.as_str(), "assistant");
            assert_eq!(ValidationMessageRole::System.as_str(), "system");
        }

        #[test]
        fn parse_valid_values() {
            assert_eq!(
                ValidationMessageRole::parse("user"),
                Some(ValidationMessageRole::User)
            );
            assert_eq!(
                ValidationMessageRole::parse("assistant"),
                Some(ValidationMessageRole::Assistant)
            );
            assert_eq!(
                ValidationMessageRole::parse("system"),
                Some(ValidationMessageRole::System)
            );
        }

        #[test]
        fn roundtrip_as_str_parse() {
            for role in [
                ValidationMessageRole::User,
                ValidationMessageRole::Assistant,
                ValidationMessageRole::System,
            ] {
                assert_eq!(ValidationMessageRole::parse(role.as_str()), Some(role));
            }
        }
    }
}
