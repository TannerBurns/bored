use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Workflow type for ticket execution
/// Note: Basic workflow has been removed - all tickets now use MultiStage
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowType {
    #[default]
    MultiStage,
}

impl WorkflowType {
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkflowType::MultiStage => "multi_stage",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            // Accept both for backward compatibility during migration
            "basic" | "multi_stage" => Some(WorkflowType::MultiStage),
            _ => None,
        }
    }
}

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
    /// Whether this project requires git for agent operations.
    /// When false, workers will skip git validation and git-related workflow steps.
    pub requires_git: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProject {
    pub name: String,
    pub path: String,
    pub preferred_agent: Option<AgentPref>,
    /// Whether this project requires git (defaults to true if not specified)
    #[serde(default = "default_requires_git")]
    pub requires_git: bool,
}

fn default_requires_git() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProject {
    pub name: Option<String>,
    pub preferred_agent: Option<AgentPref>,
    pub allow_shell_commands: Option<bool>,
    pub allow_file_writes: Option<bool>,
    pub blocked_patterns: Option<Vec<String>>,
    pub requires_git: Option<bool>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateComment {
    pub ticket_id: String,
    pub author_type: AuthorType,
    pub body_md: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTicket {
    pub title: Option<String>,
    pub description_md: Option<String>,
    pub priority: Option<Priority>,
    pub labels: Option<Vec<String>>,
    pub project_id: Option<String>,
    pub agent_pref: Option<AgentPref>,
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

// ===== Task Queue System =====

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

// ===== Spec System =====

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
    /// Preferred agent type for executing the plan
    pub agent_pref: Option<String>,
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
    /// Preferred agent type (cursor, claude, any)
    pub agent_pref: Option<String>,
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
    pub agent_pref: Option<String>,
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
    pub requirements: String,
    pub decisions: Vec<String>,
    pub constraints: Vec<String>,
    pub technical_notes: Option<String>,
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
                requirements: "Build auth system".to_string(),
                decisions: vec!["Use JWT".to_string(), "Support OAuth".to_string()],
                constraints: vec!["Must be fast".to_string()],
                technical_notes: Some("Use middleware pattern".to_string()),
            };
            let json = serde_json::to_value(&spec).unwrap();
            assert_eq!(json["requirements"], "Build auth system");
            assert_eq!(json["decisions"].as_array().unwrap().len(), 2);
            assert_eq!(json["technicalNotes"], "Use middleware pattern");
        }

        #[test]
        fn serialize_without_technical_notes() {
            let spec = StructuredSpec {
                requirements: "Build feature".to_string(),
                decisions: vec![],
                constraints: vec![],
                technical_notes: None,
            };
            let json = serde_json::to_value(&spec).unwrap();
            assert_eq!(json["requirements"], "Build feature");
            assert!(json["technicalNotes"].is_null());
        }

        #[test]
        fn deserialize_with_technical_notes() {
            let json = r#"{
                "requirements": "Build auth",
                "decisions": ["Use JWT"],
                "constraints": ["Must be fast"],
                "technicalNotes": "Consider caching"
            }"#;
            let spec: StructuredSpec = serde_json::from_str(json).unwrap();
            assert_eq!(spec.requirements, "Build auth");
            assert_eq!(spec.decisions.len(), 1);
            assert_eq!(spec.technical_notes, Some("Consider caching".to_string()));
        }

        #[test]
        fn deserialize_without_technical_notes() {
            let json = r#"{
                "requirements": "Build feature",
                "decisions": [],
                "constraints": []
            }"#;
            let spec: StructuredSpec = serde_json::from_str(json).unwrap();
            assert_eq!(spec.requirements, "Build feature");
            assert!(spec.technical_notes.is_none());
        }

        #[test]
        fn roundtrip_serialization() {
            let spec = StructuredSpec {
                requirements: "Complex feature".to_string(),
                decisions: vec!["A".to_string(), "B".to_string()],
                constraints: vec!["X".to_string()],
                technical_notes: Some("Notes here".to_string()),
            };
            let json = serde_json::to_string(&spec).unwrap();
            let parsed: StructuredSpec = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed.requirements, spec.requirements);
            assert_eq!(parsed.decisions, spec.decisions);
            assert_eq!(parsed.constraints, spec.constraints);
            assert_eq!(parsed.technical_notes, spec.technical_notes);
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

    mod workflow_type_tests {
        use super::*;

        #[test]
        fn as_str_returns_snake_case() {
            assert_eq!(WorkflowType::MultiStage.as_str(), "multi_stage");
        }

        #[test]
        fn parse_valid_values() {
            // Both "basic" and "multi_stage" parse to MultiStage for backward compatibility
            assert_eq!(WorkflowType::parse("basic"), Some(WorkflowType::MultiStage));
            assert_eq!(
                WorkflowType::parse("multi_stage"),
                Some(WorkflowType::MultiStage)
            );
        }

        #[test]
        fn parse_invalid_returns_none() {
            assert_eq!(WorkflowType::parse(""), None);
            assert_eq!(WorkflowType::parse("invalid"), None);
            assert_eq!(WorkflowType::parse("BASIC"), None);
        }

        #[test]
        fn default_is_multi_stage() {
            assert_eq!(WorkflowType::default(), WorkflowType::MultiStage);
        }

        #[test]
        fn roundtrip_as_str_parse() {
            assert_eq!(
                WorkflowType::parse(WorkflowType::MultiStage.as_str()),
                Some(WorkflowType::MultiStage)
            );
        }

        #[test]
        fn serializes_to_snake_case() {
            assert_eq!(
                serde_json::to_string(&WorkflowType::MultiStage).unwrap(),
                "\"multi_stage\""
            );
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
                requires_git: true,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            let json = serde_json::to_string(&project).unwrap();
            assert!(json.contains("\"cursorHooksInstalled\":true"));
            assert!(json.contains("\"allowFileWrites\":false"));
            assert!(json.contains("\"preferredAgent\":\"cursor\""));
            assert!(json.contains("\"requiresGit\":true"));
        }

        #[test]
        fn create_project_deserializes_from_camel_case() {
            let json = r#"{"name":"Proj","path":"/tmp","preferredAgent":"claude"}"#;
            let input: CreateProject = serde_json::from_str(json).unwrap();
            assert_eq!(input.name, "Proj");
            assert_eq!(input.preferred_agent, Some(AgentPref::Claude));
            // requires_git should default to true when not specified
            assert!(input.requires_git);
        }

        #[test]
        fn create_project_requires_git_can_be_false() {
            let json = r#"{"name":"Proj","path":"/tmp","requiresGit":false}"#;
            let input: CreateProject = serde_json::from_str(json).unwrap();
            assert!(!input.requires_git);
        }

        #[test]
        fn readiness_check_serializes_variants() {
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
                agent_pref: None,
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
}
