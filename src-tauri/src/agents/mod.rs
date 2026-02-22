pub mod brainstorm;
pub mod claude;
pub mod cli_utils;
pub mod codex;
pub mod command_templates;
pub mod cost;
pub mod cursor;
pub mod json_extraction;
pub mod log_utils;
pub mod models;
pub mod provider;
pub mod registry;
pub mod validation_agent;
pub mod diagnostic;
pub mod eta;
pub mod orchestrator;
pub mod plan_validation;
pub mod planner;
pub mod prompt;
pub mod runner;
pub mod spawner;
pub mod validation;
pub mod worker;
pub mod worktree;

pub use planner::{
    PlannerAgent, PlannerConfig, PlannerConfigWithEvents, PlannerError, PlannerResult,
};
pub use worker::{Worker, WorkerConfig, WorkerManager, WorkerState, WorkerStatus};
pub use spawner::{
    run_agent_via_provider, run_agent_via_provider_with_cancel,
    CancelHandle, OnSpawnCallback, SpawnError,
};
pub use brainstorm::{
    BrainstormAgent, BrainstormConfig, BrainstormError, BrainstormResponse,
    build_conversation_prompt, build_initial_prompt, parse_response, response_has_questions,
};
pub use runner::{
    create_cancel_handles, execute_agent_run, AgentCompleteEvent, AgentErrorEvent, AgentLogEvent,
    CancelHandlesMap, RunnerConfig, RunnerResult,
};
pub use prompt::{
    generate_branch_name_generation_prompt, generate_branch_prompt, generate_command_prompt,
    generate_command_prompt_with_providers,
    generate_custom_prompt, generate_get_branch_name_prompt, generate_implement_prompt,
    generate_plan_prompt, generate_system_prompt, generate_task_implement_prompt,
    generate_task_plan_prompt, generate_task_prompt, generate_ticket_prompt,
    generate_ticket_prompt_full, generate_ticket_prompt_with_workflow,
    parse_branch_name_from_output,
};
pub use diagnostic::{
    build_diagnostic_prompt, classify_worktree_error, create_fallback_diagnostic_comment,
    run_diagnostic_agent, DiagnosticContext, DiagnosticError,
};
pub use eta::{calculate_eta, calculate_remaining_time, calculate_timing_stats};
pub use plan_validation::{
    build_clarification_message_prompt, build_plan_validation_prompt,
    generate_clarification_message, parse_validation_response, validate_plan_for_clarification,
    PlanValidationConfig, PlanValidationError, PlanValidationResult,
};
pub use cost::{AggregatedCost, RunCostData};
pub use provider::{AgentProvider, AgentRunConfig};
pub use registry::AgentRegistry;
pub use validation::{
    is_environment_valid, is_environment_valid_with_options, validate_worker_environment,
    validate_worker_environment_with_options, ValidationCheck, ValidationResult,
};

use serde::Serialize;

/// Result of an agent run
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunResult {
    pub run_id: String,
    pub exit_code: Option<i32>,
    pub status: RunOutcome,
    pub summary: Option<String>,
    pub duration_secs: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captured_stdout: Option<String>,
}

/// Outcome of a run
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RunOutcome {
    Success,
    Error,
    Timeout,
    Cancelled,
}

/// Callback for receiving log output
pub type LogCallback = Box<dyn Fn(LogLine) + Send + Sync>;

/// A line of log output
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub stream: LogStream,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogStream {
    Stdout,
    Stderr,
}

#[cfg(test)]
mod tests;
