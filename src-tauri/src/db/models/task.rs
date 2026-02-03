use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Type of task - determines prompt generation strategy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    /// User-defined instructions (from description or manual entry)
    #[default]
    Custom,
    /// Merge main branch, resolve conflicts
    SyncWithMain,
    /// Add test coverage for recent changes
    AddTests,
    /// Review code, fix issues, polish
    ReviewPolish,
    /// Fix all lint/type errors
    FixLint,
}

impl TaskType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskType::Custom => "custom",
            TaskType::SyncWithMain => "sync_with_main",
            TaskType::AddTests => "add_tests",
            TaskType::ReviewPolish => "review_polish",
            TaskType::FixLint => "fix_lint",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "custom" => Some(TaskType::Custom),
            "sync_with_main" => Some(TaskType::SyncWithMain),
            "add_tests" => Some(TaskType::AddTests),
            "review_polish" => Some(TaskType::ReviewPolish),
            "fix_lint" => Some(TaskType::FixLint),
            _ => None,
        }
    }

    /// Get a human-readable display name for the task type
    pub fn display_name(&self) -> &'static str {
        match self {
            TaskType::Custom => "Custom Task",
            TaskType::SyncWithMain => "Sync with Main",
            TaskType::AddTests => "Add Tests",
            TaskType::ReviewPolish => "Review & Polish",
            TaskType::FixLint => "Fix Lint Errors",
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
        fn as_str_returns_snake_case() {
            assert_eq!(TaskType::Custom.as_str(), "custom");
            assert_eq!(TaskType::SyncWithMain.as_str(), "sync_with_main");
            assert_eq!(TaskType::AddTests.as_str(), "add_tests");
            assert_eq!(TaskType::ReviewPolish.as_str(), "review_polish");
            assert_eq!(TaskType::FixLint.as_str(), "fix_lint");
        }

        #[test]
        fn parse_valid_values() {
            assert_eq!(TaskType::parse("custom"), Some(TaskType::Custom));
            assert_eq!(TaskType::parse("sync_with_main"), Some(TaskType::SyncWithMain));
            assert_eq!(TaskType::parse("add_tests"), Some(TaskType::AddTests));
            assert_eq!(TaskType::parse("review_polish"), Some(TaskType::ReviewPolish));
            assert_eq!(TaskType::parse("fix_lint"), Some(TaskType::FixLint));
        }

        #[test]
        fn parse_invalid_returns_none() {
            assert_eq!(TaskType::parse(""), None);
            assert_eq!(TaskType::parse("invalid"), None);
            assert_eq!(TaskType::parse("CUSTOM"), None);
        }

        #[test]
        fn display_name_returns_human_readable() {
            assert_eq!(TaskType::Custom.display_name(), "Custom Task");
            assert_eq!(TaskType::SyncWithMain.display_name(), "Sync with Main");
            assert_eq!(TaskType::AddTests.display_name(), "Add Tests");
            assert_eq!(TaskType::ReviewPolish.display_name(), "Review & Polish");
            assert_eq!(TaskType::FixLint.display_name(), "Fix Lint Errors");
        }

        #[test]
        fn default_is_custom() {
            assert_eq!(TaskType::default(), TaskType::Custom);
        }

        #[test]
        fn roundtrip_as_str_parse() {
            for t in [
                TaskType::Custom,
                TaskType::SyncWithMain,
                TaskType::AddTests,
                TaskType::ReviewPolish,
                TaskType::FixLint,
            ] {
                assert_eq!(TaskType::parse(t.as_str()), Some(t));
            }
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
