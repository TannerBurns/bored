pub mod brainstorm;
pub mod claude;
pub mod cost;
pub mod cursor;
pub mod log_utils;
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
    run_agent, run_agent_via_provider, run_agent_via_provider_with_cancel,
    run_agent_with_cancel_callback, run_agent_with_capture, CancelHandle,
    OnSpawnCallback, SpawnError,
};
pub use claude::{
    build_command as build_claude_command, check_global_hooks_installed as check_claude_global_hooks,
    check_project_commands_installed as check_claude_project_commands,
    check_project_hooks_installed as check_claude_project_hooks,
    check_user_commands_installed as check_claude_user_commands,
    generate_hooks_config as generate_claude_hooks_config,
    generate_hooks_settings, generate_hooks_settings_with_api, generate_hooks_settings_with_config,
    get_bundled_commands_path as get_claude_bundled_commands_path,
    get_bundled_commands_path_with_app as get_claude_bundled_commands_path_with_app,
    get_claude_version, install_commands as install_claude_commands,
    install_local_hooks as install_claude_local_hooks,
    install_local_hooks_with_run_id as install_claude_local_hooks_with_run_id,
    install_project_hooks as install_claude_project_hooks,
    install_user_commands as install_claude_user_commands,
    install_user_hooks as install_claude_user_hooks,
    is_claude_available, local_settings_path, project_settings_path, user_settings_path,
    ClaudeSettings, HooksConfig as ClaudeHooksConfig, COMMAND_TEMPLATES as CLAUDE_COMMAND_TEMPLATES,
};
pub use cursor::{
    build_command as build_cursor_command,
    check_global_hooks_installed as check_cursor_global_hooks,
    check_project_commands_installed as check_cursor_project_commands,
    check_project_hooks_installed as check_cursor_project_hooks,
    check_user_commands_installed as check_cursor_user_commands,
    generate_hooks_config as generate_cursor_hooks_config,
    generate_hooks_json, generate_hooks_json_with_api, generate_hooks_json_with_config,
    get_bundled_commands_path as get_cursor_bundled_commands_path,
    get_bundled_commands_path_with_app as get_cursor_bundled_commands_path_with_app,
    get_cursor_version, global_hooks_path,
    install_commands as install_cursor_commands,
    install_global_hooks as install_cursor_global_hooks,
    install_global_hooks_with_run_id as install_cursor_global_hooks_with_run_id,
    install_hooks as install_cursor_hooks,
    install_hooks_with_run_id as install_cursor_hooks_with_run_id,
    install_user_commands as install_cursor_user_commands,
    is_cursor_available, CursorSettings, HooksConfig as CursorHooksConfig,
    COMMAND_TEMPLATES as CURSOR_COMMAND_TEMPLATES,
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
pub use provider::{AgentProvider, AgentRunConfig as ProviderAgentRunConfig};
pub use registry::AgentRegistry;
pub use validation::{
    is_environment_valid, is_environment_valid_with_options, validate_worker_environment,
    validate_worker_environment_with_options, ValidationCheck, ValidationResult,
};

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Claude API settings for overriding environment configuration when spawning agents
#[derive(Debug, Clone, Default)]
pub struct ClaudeApiConfig {
    pub auth_token: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model_override: Option<String>,
    /// Enable extended thinking. Defaults to true when None.
    pub thinking_enabled: Option<bool>,
    /// Enable 1M token extended context. Defaults to false when None.
    pub extended_context_enabled: Option<bool>,
    /// Enable browser automation via Chrome. Defaults to false when None.
    pub chrome_enabled: Option<bool>,
}

impl From<crate::commands::claude::ClaudeApiSettings> for ClaudeApiConfig {
    fn from(s: crate::commands::claude::ClaudeApiSettings) -> Self {
        Self {
            auth_token: s.auth_token,
            api_key: s.api_key,
            base_url: s.base_url,
            model_override: s.model_override,
            thinking_enabled: s.thinking_enabled,
            extended_context_enabled: s.extended_context_enabled,
            chrome_enabled: s.chrome_enabled,
        }
    }
}

/// Agent type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentKind {
    Cursor,
    Claude,
}

impl AgentKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentKind::Cursor => "cursor",
            AgentKind::Claude => "claude",
        }
    }
}

/// Configuration for running an agent
#[derive(Debug, Clone)]
pub struct AgentRunConfig {
    pub kind: AgentKind,
    pub ticket_id: String,
    pub run_id: String,
    pub repo_path: PathBuf,
    pub prompt: String,
    pub timeout_secs: Option<u64>,
    pub api_url: String,
    pub api_token: String,
    pub model: Option<String>,
    /// Claude-specific API configuration (auth token, api key, base url, model override).
    /// Deprecated: prefer populating `agent_config` for agent-agnostic usage.
    pub claude_api_config: Option<ClaudeApiConfig>,
    /// Agent-agnostic configuration map.
    /// Each provider knows its own keys. Populated automatically from
    /// `claude_api_config` when using the legacy construction path.
    pub agent_config: std::collections::HashMap<String, serde_json::Value>,
}

impl AgentRunConfig {
    /// Convert this legacy config to the provider-based `AgentRunConfig`.
    pub fn to_provider_config(&self) -> provider::AgentRunConfig {
        let mut agent_config = self.agent_config.clone();

        // If the legacy claude_api_config is set but agent_config is empty,
        // populate agent_config from it for backward compatibility.
        if agent_config.is_empty() {
            if let Some(ref c) = self.claude_api_config {
                if let Some(ref v) = c.auth_token {
                    agent_config.insert("auth_token".to_string(), serde_json::json!(v));
                }
                if let Some(ref v) = c.api_key {
                    agent_config.insert("api_key".to_string(), serde_json::json!(v));
                }
                if let Some(ref v) = c.base_url {
                    agent_config.insert("base_url".to_string(), serde_json::json!(v));
                }
                if let Some(ref v) = c.model_override {
                    agent_config.insert("model_override".to_string(), serde_json::json!(v));
                }
                if let Some(v) = c.thinking_enabled {
                    agent_config.insert("thinking_enabled".to_string(), serde_json::json!(v));
                }
                if let Some(v) = c.extended_context_enabled {
                    agent_config.insert("extended_context_enabled".to_string(), serde_json::json!(v));
                }
                if let Some(v) = c.chrome_enabled {
                    agent_config.insert("chrome_enabled".to_string(), serde_json::json!(v));
                }
            }
        }

        provider::AgentRunConfig {
            agent_id: self.kind.as_str().to_string(),
            ticket_id: self.ticket_id.clone(),
            run_id: self.run_id.clone(),
            repo_path: self.repo_path.clone(),
            prompt: self.prompt.clone(),
            timeout_secs: self.timeout_secs,
            api_url: self.api_url.clone(),
            api_token: self.api_token.clone(),
            model: self.model.clone(),
            agent_config,
        }
    }
}

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

/// Extract text from agent output, handling both Claude stream-json and plain text.
/// Tries stream-json parsing first, falls back to raw output.
pub fn extract_agent_text(output: &str) -> String {
    extract_text_from_stream_json(output).unwrap_or_else(|| output.to_string())
}

/// Extract text content from Claude's stream-json format.
///
/// Delegates to the canonical implementation in `claude::provider`.
pub fn extract_text_from_stream_json(stream_output: &str) -> Option<String> {
    claude::provider::extract_text_from_stream_json(stream_output)
}

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
