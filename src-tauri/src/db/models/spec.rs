use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Status of a spec version in the planning workflow
/// Note: Versions start at Conversing - Draft status has been removed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SpecVersionStatus {
    /// In brainstorming conversation - refining requirements
    #[default]
    Conversing,
    /// Agent is exploring the codebase
    Exploring,
    /// Agent is generating the plan
    Planning,
    /// Plan generated, waiting for user approval
    AwaitingApproval,
    /// User has approved the plan
    Approved,
    /// Plan is being executed (creating epics/tickets)
    Executing,
    /// Epics/tickets created, ready to start work
    Executed,
    /// Work has been started (epics moved to Ready, agents running)
    Working,
    /// Work is paused (can be resumed)
    Paused,
    /// Work has been halted (can be restarted from beginning)
    Halted,
    /// All epics completed successfully
    Completed,
    /// An error occurred
    Failed,
}

impl SpecVersionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SpecVersionStatus::Conversing => "conversing",
            SpecVersionStatus::Exploring => "exploring",
            SpecVersionStatus::Planning => "planning",
            SpecVersionStatus::AwaitingApproval => "awaiting_approval",
            SpecVersionStatus::Approved => "approved",
            SpecVersionStatus::Executing => "executing",
            SpecVersionStatus::Executed => "executed",
            SpecVersionStatus::Working => "working",
            SpecVersionStatus::Paused => "paused",
            SpecVersionStatus::Halted => "halted",
            SpecVersionStatus::Completed => "completed",
            SpecVersionStatus::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "conversing" => Some(SpecVersionStatus::Conversing),
            "exploring" => Some(SpecVersionStatus::Exploring),
            "planning" => Some(SpecVersionStatus::Planning),
            "awaiting_approval" => Some(SpecVersionStatus::AwaitingApproval),
            "approved" => Some(SpecVersionStatus::Approved),
            "executing" => Some(SpecVersionStatus::Executing),
            "executed" => Some(SpecVersionStatus::Executed),
            "working" => Some(SpecVersionStatus::Working),
            "paused" => Some(SpecVersionStatus::Paused),
            "halted" => Some(SpecVersionStatus::Halted),
            "completed" => Some(SpecVersionStatus::Completed),
            "failed" => Some(SpecVersionStatus::Failed),
            _ => None,
        }
    }
}

/// Alias for backward compatibility
pub type SpecStatus = SpecVersionStatus;

/// A single exploration query and its result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Exploration {
    pub query: String,
    pub response: String,
    pub timestamp: DateTime<Utc>,
}

/// A spec for the planning agent (top-level entity with shared conversation)
/// Versioned fields (status, exploration_log, plan, work_started_at) are in SpecVersion
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spec {
    pub id: String,
    /// The board this spec belongs to (for organization/display)
    pub board_id: String,
    /// The board where tickets will be created (defaults to board_id if not set)
    pub target_board_id: Option<String>,
    /// The project this spec is scoped to (required)
    pub project_id: String,
    pub name: String,
    pub user_input: String,
    /// Preferred model for the agent
    pub model: Option<String>,
    /// Settings for this spec (auto_approve, etc.)
    pub settings: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A version of a spec (contains versioned exploration/plan data)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecVersion {
    pub id: String,
    pub spec_id: String,
    pub version_number: i32,
    pub status: SpecVersionStatus,
    /// Log of exploration queries and responses
    pub exploration_log: Vec<Exploration>,
    /// Generated plan in markdown format (for display)
    pub plan_markdown: Option<String>,
    /// Parsed plan structure (for execution)
    pub plan_json: Option<serde_json::Value>,
    /// When work phase was started (for ETA calculation)
    pub work_started_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Spec with its latest version (convenience struct for API responses)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecWithVersion {
    #[serde(flatten)]
    pub spec: Spec,
    /// The latest version of this spec
    pub latest_version: Option<SpecVersion>,
    /// Total number of versions
    pub version_count: i32,
}

/// Create a new spec
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSpec {
    pub board_id: String,
    /// The board where tickets will be created (defaults to board_id if not set)
    pub target_board_id: Option<String>,
    /// The project this spec is scoped to (required)
    pub project_id: String,
    pub name: String,
    pub user_input: String,
    /// Preferred model
    pub model: Option<String>,
    #[serde(default)]
    pub settings: serde_json::Value,
}

/// Update a spec (non-versioned fields only)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSpec {
    pub name: Option<String>,
    pub user_input: Option<String>,
    pub model: Option<String>,
    pub settings: Option<serde_json::Value>,
}

/// Create a new spec version
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSpecVersion {
    pub spec_id: String,
}

/// Update a spec version
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSpecVersion {
    pub status: Option<SpecVersionStatus>,
    pub exploration_log: Option<Vec<Exploration>>,
    pub plan_markdown: Option<String>,
    pub plan_json: Option<serde_json::Value>,
    pub work_started_at: Option<DateTime<Utc>>,
}

/// An epic in a generated plan
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanEpic {
    pub title: String,
    pub description: String,
    /// Titles of epics this depends on (empty = root epic, no dependencies)
    #[serde(default, deserialize_with = "deserialize_depends_on")]
    pub depends_on: Vec<String>,
    pub tickets: Vec<PlanTicket>,
}

/// Custom deserializer to handle both old format (null or string) and new format (array)
fn deserialize_depends_on<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Deserialize;

    // Use an untagged enum to handle string or array
    // Order matters: try array first, then string
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrArray {
        Multiple(Vec<String>),
        Single(String),
    }

    // Deserialize as Option<StringOrArray> to handle null
    let value: Option<StringOrArray> = Option::deserialize(deserializer)?;

    match value {
        None => Ok(Vec::new()),
        Some(StringOrArray::Single(s)) => {
            if s.is_empty() {
                Ok(Vec::new())
            } else {
                Ok(vec![s])
            }
        }
        Some(StringOrArray::Multiple(v)) => Ok(v.into_iter().filter(|s| !s.is_empty()).collect()),
    }
}

/// A ticket in a generated plan
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanTicket {
    pub title: String,
    pub description: String,
    pub acceptance_criteria: Option<Vec<String>>,
    /// Branch name assigned at planning time (skips AI generation at work time if set)
    pub branch_name: Option<String>,
}

/// A generated project plan
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPlan {
    pub overview: String,
    pub epics: Vec<PlanEpic>,
}

/// Status of a single ticket within an epic
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecTicketStatus {
    pub id: String,
    pub title: String,
    pub column: String,
}

/// Status of a single epic within a spec
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecEpicStatus {
    pub id: String,
    pub title: String,
    pub column: String,
    /// The epics this one depends on (empty = independent/root epic)
    pub depends_on_ids: Vec<String>,
    /// Titles of the dependency epics (for display, in same order as depends_on_ids)
    pub depends_on_titles: Vec<String>,
    /// Child tickets in this epic
    pub tickets: Vec<SpecTicketStatus>,
}

/// Progress stats for a spec's epics
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecProgress {
    /// Number of epics
    pub total: usize,
    /// Epics in Done column
    pub done: usize,
    /// Epics in Ready/In Progress/Review
    pub in_progress: usize,
    /// Epics in Blocked column
    pub blocked: usize,
    /// Total number of all tickets (epics + child tickets)
    pub total_tickets: usize,
    /// List of epics with their status
    pub epics: Vec<SpecEpicStatus>,
}

/// ETA calculation result for a spec
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecEta {
    pub spec_id: String,
    /// When work phase was started
    pub work_started_at: Option<DateTime<Utc>>,
    /// Total number of tickets
    pub total_tickets: usize,
    /// Completed tickets
    pub completed_tickets: usize,
    /// Currently in-progress tickets
    pub in_progress_tickets: usize,
    /// Paused tickets
    pub paused_tickets: usize,
    /// Time elapsed since work started (seconds)
    pub elapsed_seconds: i64,
    /// Average seconds per completed ticket
    pub avg_seconds_per_ticket: Option<f64>,
    /// Average seconds per stage (for completed stages)
    pub avg_seconds_per_stage: std::collections::HashMap<String, f64>,
    /// Estimated seconds remaining
    pub estimated_seconds_remaining: Option<i64>,
    /// Estimated completion time (ISO 8601)
    pub estimated_completion_time: Option<DateTime<Utc>>,
    /// Confidence level based on sample size
    pub confidence: EtaConfidence,
}

/// Confidence level for ETA estimates
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum EtaConfidence {
    /// Not enough data for reliable estimate
    #[default]
    Low,
    /// Some data available
    Medium,
    /// Good sample size for reliable estimate
    High,
}

/// Role in a conversation message
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ConversationRole {
    User,
    Assistant,
    System,
}

impl ConversationRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConversationRole::User => "user",
            ConversationRole::Assistant => "assistant",
            ConversationRole::System => "system",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "user" => Some(ConversationRole::User),
            "assistant" => Some(ConversationRole::Assistant),
            "system" => Some(ConversationRole::System),
            _ => None,
        }
    }
}

/// A message in a spec brainstorming conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessage {
    pub id: String,
    pub spec_id: String,
    pub role: ConversationRole,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateConversationMessage {
    pub spec_id: String,
    pub role: ConversationRole,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructuredSpec {
    /// List of discrete, single-sentence requirement statements.
    /// Using Vec<String> (rather than a markdown blob) keeps each item small
    /// so agents do not embed code fences inside the values, which would break
    /// JSON extraction via fence-search heuristics.
    pub requirements: Vec<String>,
    pub decisions: Vec<String>,
    pub constraints: Vec<String>,
    /// List of implementation notes/steps (files to create, patterns to follow, etc.).
    /// Accepts both "technicalNotes" (camelCase) and "technical_notes" (snake_case)
    /// since the brainstorm prompt examples use snake_case.
    #[serde(alias = "technical_notes", default)]
    pub technical_notes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    mod spec_version_status_tests {
        use super::*;

        #[test]
        fn as_str_returns_lowercase() {
            assert_eq!(SpecVersionStatus::Conversing.as_str(), "conversing");
            assert_eq!(SpecVersionStatus::Exploring.as_str(), "exploring");
            assert_eq!(SpecVersionStatus::Planning.as_str(), "planning");
            assert_eq!(SpecVersionStatus::AwaitingApproval.as_str(), "awaiting_approval");
            assert_eq!(SpecVersionStatus::Approved.as_str(), "approved");
            assert_eq!(SpecVersionStatus::Executing.as_str(), "executing");
            assert_eq!(SpecVersionStatus::Executed.as_str(), "executed");
            assert_eq!(SpecVersionStatus::Working.as_str(), "working");
            assert_eq!(SpecVersionStatus::Paused.as_str(), "paused");
            assert_eq!(SpecVersionStatus::Halted.as_str(), "halted");
            assert_eq!(SpecVersionStatus::Completed.as_str(), "completed");
            assert_eq!(SpecVersionStatus::Failed.as_str(), "failed");
        }

        #[test]
        fn parse_valid_values() {
            assert_eq!(SpecVersionStatus::parse("conversing"), Some(SpecVersionStatus::Conversing));
            assert_eq!(SpecVersionStatus::parse("exploring"), Some(SpecVersionStatus::Exploring));
            assert_eq!(SpecVersionStatus::parse("planning"), Some(SpecVersionStatus::Planning));
            assert_eq!(SpecVersionStatus::parse("awaiting_approval"), Some(SpecVersionStatus::AwaitingApproval));
            assert_eq!(SpecVersionStatus::parse("approved"), Some(SpecVersionStatus::Approved));
            assert_eq!(SpecVersionStatus::parse("executing"), Some(SpecVersionStatus::Executing));
            assert_eq!(SpecVersionStatus::parse("executed"), Some(SpecVersionStatus::Executed));
            assert_eq!(SpecVersionStatus::parse("working"), Some(SpecVersionStatus::Working));
            assert_eq!(SpecVersionStatus::parse("paused"), Some(SpecVersionStatus::Paused));
            assert_eq!(SpecVersionStatus::parse("halted"), Some(SpecVersionStatus::Halted));
            assert_eq!(SpecVersionStatus::parse("completed"), Some(SpecVersionStatus::Completed));
            assert_eq!(SpecVersionStatus::parse("failed"), Some(SpecVersionStatus::Failed));
        }

        #[test]
        fn parse_invalid_returns_none() {
            assert_eq!(SpecVersionStatus::parse(""), None);
            assert_eq!(SpecVersionStatus::parse("invalid"), None);
            assert_eq!(SpecVersionStatus::parse("CONVERSING"), None);
            assert_eq!(SpecVersionStatus::parse("draft"), None);
        }

        #[test]
        fn default_is_conversing() {
            assert_eq!(SpecVersionStatus::default(), SpecVersionStatus::Conversing);
        }

        #[test]
        fn roundtrip_as_str_parse() {
            for status in [
                SpecVersionStatus::Conversing,
                SpecVersionStatus::Exploring,
                SpecVersionStatus::Planning,
                SpecVersionStatus::AwaitingApproval,
                SpecVersionStatus::Approved,
                SpecVersionStatus::Executing,
                SpecVersionStatus::Executed,
                SpecVersionStatus::Working,
                SpecVersionStatus::Paused,
                SpecVersionStatus::Halted,
                SpecVersionStatus::Completed,
                SpecVersionStatus::Failed,
            ] {
                assert_eq!(SpecVersionStatus::parse(status.as_str()), Some(status));
            }
        }
    }

    mod conversation_role_tests {
        use super::*;

        #[test]
        fn as_str_returns_lowercase() {
            assert_eq!(ConversationRole::User.as_str(), "user");
            assert_eq!(ConversationRole::Assistant.as_str(), "assistant");
            assert_eq!(ConversationRole::System.as_str(), "system");
        }

        #[test]
        fn parse_valid_values() {
            assert_eq!(ConversationRole::parse("user"), Some(ConversationRole::User));
            assert_eq!(ConversationRole::parse("assistant"), Some(ConversationRole::Assistant));
            assert_eq!(ConversationRole::parse("system"), Some(ConversationRole::System));
        }

        #[test]
        fn parse_invalid_returns_none() {
            assert_eq!(ConversationRole::parse(""), None);
            assert_eq!(ConversationRole::parse("invalid"), None);
            assert_eq!(ConversationRole::parse("USER"), None);
        }

        #[test]
        fn roundtrip_as_str_parse() {
            for role in [
                ConversationRole::User,
                ConversationRole::Assistant,
                ConversationRole::System,
            ] {
                assert_eq!(ConversationRole::parse(role.as_str()), Some(role));
            }
        }
    }

    mod structured_spec_tests {
        use super::*;

        #[test]
        fn serialize_with_technical_notes() {
            let spec = StructuredSpec {
                requirements: vec!["Build auth system".to_string(), "Support OAuth".to_string()],
                decisions: vec!["Use JWT".to_string(), "Support OAuth".to_string()],
                constraints: vec!["Must be fast".to_string()],
                technical_notes: vec!["Use middleware pattern".to_string()],
            };
            let json = serde_json::to_value(&spec).unwrap();
            assert_eq!(json["requirements"].as_array().unwrap().len(), 2);
            assert_eq!(json["requirements"][0], "Build auth system");
            assert_eq!(json["decisions"].as_array().unwrap().len(), 2);
            assert_eq!(json["technicalNotes"].as_array().unwrap()[0], "Use middleware pattern");
        }

        #[test]
        fn serialize_without_technical_notes() {
            let spec = StructuredSpec {
                requirements: vec!["Build feature".to_string()],
                decisions: vec![],
                constraints: vec![],
                technical_notes: vec![],
            };
            let json = serde_json::to_value(&spec).unwrap();
            assert_eq!(json["requirements"][0], "Build feature");
            assert!(json["technicalNotes"].as_array().unwrap().is_empty());
        }

        #[test]
        fn deserialize_with_technical_notes() {
            let json = r#"{
                "requirements": ["Build auth", "Support login"],
                "decisions": ["Use JWT"],
                "constraints": ["Must be fast"],
                "technicalNotes": ["Consider caching", "Follow existing middleware pattern"]
            }"#;
            let spec: StructuredSpec = serde_json::from_str(json).unwrap();
            assert_eq!(spec.requirements.len(), 2);
            assert_eq!(spec.requirements[0], "Build auth");
            assert_eq!(spec.decisions.len(), 1);
            assert_eq!(spec.technical_notes.len(), 2);
            assert_eq!(spec.technical_notes[0], "Consider caching");
        }

        #[test]
        fn deserialize_without_technical_notes() {
            let json = r#"{
                "requirements": ["Build feature"],
                "decisions": [],
                "constraints": []
            }"#;
            let spec: StructuredSpec = serde_json::from_str(json).unwrap();
            assert_eq!(spec.requirements[0], "Build feature");
            assert!(spec.technical_notes.is_empty());
        }

        #[test]
        fn deserialize_with_snake_case_technical_notes() {
            // The brainstorm prompt examples use "technical_notes" (snake_case),
            // so agents may output it that way instead of "technicalNotes" (camelCase).
            let json = r#"{
                "requirements": ["Build auth"],
                "decisions": ["Use JWT"],
                "constraints": ["Must be fast"],
                "technical_notes": ["Extend the existing auth module in src/auth/"]
            }"#;
            let spec: StructuredSpec = serde_json::from_str(json).unwrap();
            assert_eq!(spec.requirements[0], "Build auth");
            assert_eq!(
                spec.technical_notes[0],
                "Extend the existing auth module in src/auth/",
                "snake_case technical_notes should be accepted via serde alias"
            );
        }

        #[test]
        fn roundtrip_serialization() {
            let spec = StructuredSpec {
                requirements: vec!["Complex feature".to_string(), "Must scale".to_string()],
                decisions: vec!["A".to_string(), "B".to_string()],
                constraints: vec!["X".to_string()],
                technical_notes: vec!["Notes here".to_string()],
            };
            let json = serde_json::to_string(&spec).unwrap();
            let parsed: StructuredSpec = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed.requirements, spec.requirements);
            assert_eq!(parsed.decisions, spec.decisions);
            assert_eq!(parsed.constraints, spec.constraints);
            assert_eq!(parsed.technical_notes, spec.technical_notes);
        }
    }
}
