use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::WorkflowType;

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
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub workflow_type: WorkflowType,
    pub model: Option<String>,
    /// The git branch name for this ticket (agent-generated)
    pub branch_name: Option<String>,
    /// Whether this ticket is an epic (contains child tickets)
    #[serde(default)]
    pub is_epic: bool,
    /// The parent epic ID (if this ticket is a child of an epic)
    pub epic_id: Option<String>,
    /// The order of this ticket within its parent epic
    pub order_in_epic: Option<i32>,
    /// Cross-epic dependency: which epic must complete before this epic can start (primary)
    pub depends_on_epic_id: Option<String>,
    /// All epic dependencies as array of IDs (for display)
    #[serde(default)]
    pub depends_on_epic_ids: Vec<String>,
    /// Link back to spec version that created this ticket
    pub spec_version_id: Option<String>,
    /// When the ticket was paused (if currently paused)
    pub paused_at: Option<DateTime<Utc>>,
    /// Which workflow stage was active when paused (e.g., "branch", "implement", "deslop", "review")
    pub paused_at_stage: Option<String>,
    /// The run ID that was in progress when paused
    pub paused_run_id: Option<String>,
}

impl Ticket {
    /// Check if this epic is a consolidation epic (by title convention).
    /// Consolidation epics are identified by titles starting with "Consolidate".
    pub fn is_consolidation_epic(&self) -> bool {
        self.is_epic && self.title.to_lowercase().starts_with("consolidate")
    }
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
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub workflow_type: WorkflowType,
    pub model: Option<String>,
    /// Optional pre-defined branch name (if not provided, will be AI-generated on first run)
    pub branch_name: Option<String>,
    /// Whether to create this ticket as an epic
    #[serde(default)]
    pub is_epic: bool,
    /// The parent epic ID (when creating a child ticket)
    pub epic_id: Option<String>,
    /// Cross-epic dependency: which epic must complete before this epic can start (primary)
    pub depends_on_epic_id: Option<String>,
    /// All epic dependencies (for display in progress views)
    #[serde(default)]
    pub depends_on_epic_ids: Vec<String>,
    /// Link back to spec version that created this ticket
    pub spec_version_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTicket {
    pub title: Option<String>,
    pub description_md: Option<String>,
    pub priority: Option<Priority>,
    pub labels: Option<Vec<String>>,
    pub project_id: Option<String>,
    pub workspace_id: Option<String>,
    pub workflow_type: Option<WorkflowType>,
    pub model: Option<String>,
    pub branch_name: Option<String>,
    pub column_id: Option<String>,
    /// Set is_epic status
    pub is_epic: Option<bool>,
    /// Set or clear the parent epic ID
    pub epic_id: Option<String>,
    /// Set the order within the parent epic
    pub order_in_epic: Option<i32>,
    /// Set or clear the depends_on_epic_id
    pub depends_on_epic_id: Option<String>,
    /// Update all epic dependencies
    #[serde(default)]
    pub depends_on_epic_ids: Vec<String>,
    /// Set or clear the spec_version_id
    pub spec_version_id: Option<String>,
}

/// Progress information for an epic's children
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EpicProgress {
    /// Total number of child tickets
    pub total: i32,
    /// Children in Backlog
    pub backlog: i32,
    /// Children in Ready
    pub ready: i32,
    /// Children in In Progress
    pub in_progress: i32,
    /// Children in Blocked
    pub blocked: i32,
    /// Children in Review
    pub review: i32,
    /// Children in Done
    pub done: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReadinessCheck {
    Ready {
        project_id: String,
    },
    /// Serializes as `{ "noProject": null }` to match TypeScript discriminated union
    NoProject(Option<()>),
    /// Serializes as `{ "projectNotFound": null }` to match TypeScript discriminated union
    ProjectNotFound(Option<()>),
    ProjectPathMissing {
        path: String,
    },
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
            for p in [
                Priority::Low,
                Priority::Medium,
                Priority::High,
                Priority::Urgent,
            ] {
                assert_eq!(Priority::parse(p.as_str()), Some(p));
            }
        }
    }

    mod ticket_tests {
        use super::*;

        fn make_ticket(title: &str, is_epic: bool) -> Ticket {
            Ticket {
                id: "t1".to_string(),
                board_id: "b1".to_string(),
                column_id: "c1".to_string(),
                title: title.to_string(),
                description_md: "".to_string(),
                priority: Priority::Medium,
                labels: vec![],
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                locked_by_run_id: None,
                lock_expires_at: None,
                project_id: None,
                workspace_id: None,
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic,
                epic_id: None,
                order_in_epic: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
                paused_at: None,
                paused_at_stage: None,
                paused_run_id: None,
            }
        }

        #[test]
        fn is_consolidation_epic_true_for_consolidate_title() {
            let ticket = make_ticket("Consolidate Changes", true);
            assert!(ticket.is_consolidation_epic());
        }

        #[test]
        fn is_consolidation_epic_true_for_lowercase_consolidate() {
            let ticket = make_ticket("consolidate all work", true);
            assert!(ticket.is_consolidation_epic());
        }

        #[test]
        fn is_consolidation_epic_false_for_non_epic() {
            let ticket = make_ticket("Consolidate Changes", false);
            assert!(!ticket.is_consolidation_epic());
        }

        #[test]
        fn is_consolidation_epic_false_for_other_title() {
            let ticket = make_ticket("User Profile Backend", true);
            assert!(!ticket.is_consolidation_epic());
        }

        #[test]
        fn is_consolidation_epic_false_for_consolidate_not_at_start() {
            let ticket = make_ticket("Final Consolidate Step", true);
            assert!(!ticket.is_consolidation_epic());
        }
    }

    mod readiness_check_tests {
        use super::*;

        #[test]
        fn serializes_variants() {
            let ready = ReadinessCheck::Ready {
                project_id: "p1".to_string(),
            };
            let json = serde_json::to_string(&ready).unwrap();
            assert!(json.contains("ready"));

            let missing = ReadinessCheck::ProjectPathMissing {
                path: "/gone".to_string(),
            };
            let json = serde_json::to_string(&missing).unwrap();
            assert!(json.contains("projectPathMissing"));
        }
    }
}
