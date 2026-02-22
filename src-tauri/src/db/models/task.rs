use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Type of task - determines prompt generation strategy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    /// User-defined instructions (from description or manual entry)
    #[default]
    Custom,
    /// A catalog command (built-in or custom) identified by its ID
    #[serde(untagged)]
    Command(String),
}

impl TaskType {
    pub fn to_db_string(&self) -> String {
        match self {
            TaskType::Custom => "custom".to_string(),
            TaskType::Command(id) => format!("command:{}", id),
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "custom" => Some(TaskType::Custom),
            // Legacy preset values -> Command with hyphenated IDs
            "sync_with_main" => Some(TaskType::Command("sync-with-main".to_string())),
            "add_tests" => Some(TaskType::Command("add-tests".to_string())),
            "review_polish" => Some(TaskType::Command("review-polish".to_string())),
            "fix_lint" => Some(TaskType::Command("fix-lint".to_string())),
            s if s.starts_with("command:") => {
                let id = s.strip_prefix("command:").unwrap();
                if id.is_empty() {
                    None
                } else {
                    Some(TaskType::Command(id.to_string()))
                }
            }
            _ => None,
        }
    }

    /// Get a human-readable display name for the task type
    pub fn display_name(&self) -> String {
        match self {
            TaskType::Custom => "Custom Task".to_string(),
            TaskType::Command(id) => id
                .split('-')
                .map(|w| {
                    let mut c = w.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().to_string() + c.as_str(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" "),
        }
    }

    pub fn command_id(&self) -> Option<&str> {
        match self {
            TaskType::Command(id) => Some(id),
            _ => None,
        }
    }
}

/// Status of a task in the queue
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Waiting to be worked
    #[default]
    Pending,
    /// Currently being executed
    InProgress,
    /// Successfully finished
    Completed,
    /// Encountered an error
    Failed,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(TaskStatus::Pending),
            "in_progress" => Some(TaskStatus::InProgress),
            "completed" => Some(TaskStatus::Completed),
            "failed" => Some(TaskStatus::Failed),
            _ => None,
        }
    }
}

/// A task in the ticket's task queue
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub ticket_id: String,
    pub order_index: i32,
    pub task_type: TaskType,
    /// Short summary (auto-generated or user-provided)
    pub title: Option<String>,
    /// The prompt/instructions for the agent
    pub content: Option<String>,
    pub status: TaskStatus,
    /// The run that executed this task
    pub run_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Create a new task
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTask {
    pub ticket_id: String,
    #[serde(default)]
    pub task_type: TaskType,
    pub title: Option<String>,
    pub content: Option<String>,
}

/// Update an existing task
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTask {
    pub title: Option<String>,
    pub content: Option<String>,
    pub status: Option<TaskStatus>,
    pub run_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    mod task_type_tests {
        use super::*;

        #[test]
        fn to_db_string_custom() {
            assert_eq!(TaskType::Custom.to_db_string(), "custom");
        }

        #[test]
        fn to_db_string_command() {
            assert_eq!(
                TaskType::Command("fix-lint".to_string()).to_db_string(),
                "command:fix-lint"
            );
            assert_eq!(
                TaskType::Command("my-custom-cmd".to_string()).to_db_string(),
                "command:my-custom-cmd"
            );
        }

        #[test]
        fn parse_custom() {
            assert_eq!(TaskType::parse("custom"), Some(TaskType::Custom));
        }

        #[test]
        fn parse_command_prefix() {
            assert_eq!(
                TaskType::parse("command:code-review"),
                Some(TaskType::Command("code-review".to_string()))
            );
        }

        #[test]
        fn parse_legacy_preset_values() {
            assert_eq!(
                TaskType::parse("sync_with_main"),
                Some(TaskType::Command("sync-with-main".to_string()))
            );
            assert_eq!(
                TaskType::parse("add_tests"),
                Some(TaskType::Command("add-tests".to_string()))
            );
            assert_eq!(
                TaskType::parse("review_polish"),
                Some(TaskType::Command("review-polish".to_string()))
            );
            assert_eq!(
                TaskType::parse("fix_lint"),
                Some(TaskType::Command("fix-lint".to_string()))
            );
        }

        #[test]
        fn parse_invalid_returns_none() {
            assert_eq!(TaskType::parse(""), None);
            assert_eq!(TaskType::parse("command:"), None);
            assert_eq!(TaskType::parse("CUSTOM"), None);
        }

        #[test]
        fn display_name_custom() {
            assert_eq!(TaskType::Custom.display_name(), "Custom Task");
        }

        #[test]
        fn display_name_command() {
            assert_eq!(
                TaskType::Command("fix-lint".to_string()).display_name(),
                "Fix Lint"
            );
            assert_eq!(
                TaskType::Command("code-review".to_string()).display_name(),
                "Code Review"
            );
            assert_eq!(
                TaskType::Command("sync-with-main".to_string()).display_name(),
                "Sync With Main"
            );
        }

        #[test]
        fn default_is_custom() {
            assert_eq!(TaskType::default(), TaskType::Custom);
        }

        #[test]
        fn roundtrip_custom() {
            let t = TaskType::Custom;
            assert_eq!(TaskType::parse(&t.to_db_string()), Some(t));
        }

        #[test]
        fn roundtrip_command() {
            for id in ["fix-lint", "code-review", "my-custom-cmd"] {
                let t = TaskType::Command(id.to_string());
                assert_eq!(TaskType::parse(&t.to_db_string()), Some(t));
            }
        }

        #[test]
        fn command_id_returns_id() {
            assert_eq!(
                TaskType::Command("fix-lint".to_string()).command_id(),
                Some("fix-lint")
            );
            assert_eq!(TaskType::Custom.command_id(), None);
        }
    }

    mod task_status_tests {
        use super::*;

        #[test]
        fn as_str_returns_snake_case() {
            assert_eq!(TaskStatus::Pending.as_str(), "pending");
            assert_eq!(TaskStatus::InProgress.as_str(), "in_progress");
            assert_eq!(TaskStatus::Completed.as_str(), "completed");
            assert_eq!(TaskStatus::Failed.as_str(), "failed");
        }

        #[test]
        fn parse_valid_values() {
            assert_eq!(TaskStatus::parse("pending"), Some(TaskStatus::Pending));
            assert_eq!(TaskStatus::parse("in_progress"), Some(TaskStatus::InProgress));
            assert_eq!(TaskStatus::parse("completed"), Some(TaskStatus::Completed));
            assert_eq!(TaskStatus::parse("failed"), Some(TaskStatus::Failed));
        }

        #[test]
        fn parse_invalid_returns_none() {
            assert_eq!(TaskStatus::parse(""), None);
            assert_eq!(TaskStatus::parse("unknown"), None);
        }

        #[test]
        fn default_is_pending() {
            assert_eq!(TaskStatus::default(), TaskStatus::Pending);
        }

        #[test]
        fn roundtrip_as_str_parse() {
            for s in [
                TaskStatus::Pending,
                TaskStatus::InProgress,
                TaskStatus::Completed,
                TaskStatus::Failed,
            ] {
                assert_eq!(TaskStatus::parse(s.as_str()), Some(s));
            }
        }
    }
}
