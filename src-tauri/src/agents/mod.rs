pub mod brainstorm;
pub mod claude;
pub mod cursor;
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
    run_agent, run_agent_with_cancel_callback, run_agent_with_capture, CancelHandle,
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
}

impl From<crate::commands::claude::ClaudeApiSettings> for ClaudeApiConfig {
    fn from(s: crate::commands::claude::ClaudeApiSettings) -> Self {
        Self {
            auth_token: s.auth_token,
            api_key: s.api_key,
            base_url: s.base_url,
            model_override: s.model_override,
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
    /// Claude-specific API configuration (auth token, api key, base url, model override)
    pub claude_api_config: Option<ClaudeApiConfig>,
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
/// The stream-json format has one JSON object per line with structure:
/// {"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"..."}}}
pub fn extract_text_from_stream_json(stream_output: &str) -> Option<String> {
    let mut text_parts = Vec::new();
    // Track the last assistant message text separately.
    // The stdout contains one JSON line per conversation turn, so intermediate
    // assistant messages (tool-use narration like "Let me explore...") appear
    // before the final response. We only want the LAST assistant message.
    let mut last_assistant_text: Option<String> = None;

    for line in stream_output.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }

        // Try to parse as JSON
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(msg_type) = json.get("type").and_then(|t| t.as_str()) {
                match msg_type {
                    "stream_event" => {
                        // Claude stream-json format wraps events in stream_event
                        // Text deltas are at: .event.delta.text
                        if let Some(event) = json.get("event") {
                            if let Some(event_type) = event.get("type").and_then(|t| t.as_str()) {
                                if event_type == "content_block_delta" {
                                    if let Some(text) = event
                                        .get("delta")
                                        .and_then(|d| d.get("text"))
                                        .and_then(|t| t.as_str())
                                    {
                                        text_parts.push(text.to_string());
                                    }
                                }
                            }
                        }
                    }
                    "result" => {
                        // Final result message contains the complete text
                        if let Some(result) = json.get("result").and_then(|r| r.as_str()) {
                            text_parts.push(result.to_string());
                        }
                    }
                    "assistant" => {
                        // Assistant message with content array.
                        // Each line is a complete conversation turn. Only keep
                        // the last one — earlier turns are intermediate narration
                        // during tool use, not the final response.
                        if let Some(text) = json
                            .get("message")
                            .and_then(|m| m.get("content"))
                            .and_then(|c| c.as_array())
                            .and_then(|arr| {
                                arr.iter().find(|v| {
                                    v.get("type").and_then(|t| t.as_str()) == Some("text")
                                })
                            })
                            .and_then(|v| v.get("text"))
                            .and_then(|t| t.as_str())
                        {
                            last_assistant_text = Some(text.to_string());
                        }
                    }
                    "content_block_delta" => {
                        // Legacy/direct content_block_delta (without stream_event wrapper)
                        if let Some(delta) = json
                            .get("delta")
                            .and_then(|d| d.get("text"))
                            .and_then(|t| t.as_str())
                        {
                            text_parts.push(delta.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Prefer stream deltas / result text if present, otherwise use last assistant message
    if !text_parts.is_empty() {
        Some(text_parts.join(""))
    } else {
        last_assistant_text
    }
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
mod tests {
    use super::*;

    #[test]
    fn claude_api_config_default() {
        let config = ClaudeApiConfig::default();
        assert!(config.auth_token.is_none());
        assert!(config.api_key.is_none());
        assert!(config.base_url.is_none());
        assert!(config.model_override.is_none());
    }

    #[test]
    fn claude_api_config_with_values() {
        let config = ClaudeApiConfig {
            auth_token: Some("auth123".to_string()),
            api_key: Some("key456".to_string()),
            base_url: Some("https://custom.api.com".to_string()),
            model_override: Some("claude-opus-4-6".to_string()),
        };
        assert_eq!(config.auth_token.as_deref(), Some("auth123"));
        assert_eq!(config.api_key.as_deref(), Some("key456"));
        assert_eq!(config.base_url.as_deref(), Some("https://custom.api.com"));
        assert_eq!(config.model_override.as_deref(), Some("claude-opus-4-6"));
    }

    #[test]
    fn agent_kind_as_str() {
        assert_eq!(AgentKind::Cursor.as_str(), "cursor");
        assert_eq!(AgentKind::Claude.as_str(), "claude");
    }

    #[test]
    fn agent_kind_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&AgentKind::Cursor).unwrap(),
            "\"cursor\""
        );
        assert_eq!(
            serde_json::to_string(&AgentKind::Claude).unwrap(),
            "\"claude\""
        );
    }

    #[test]
    fn run_outcome_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&RunOutcome::Success).unwrap(),
            "\"success\""
        );
        assert_eq!(
            serde_json::to_string(&RunOutcome::Timeout).unwrap(),
            "\"timeout\""
        );
    }

    #[test]
    fn log_stream_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&LogStream::Stdout).unwrap(),
            "\"stdout\""
        );
        assert_eq!(
            serde_json::to_string(&LogStream::Stderr).unwrap(),
            "\"stderr\""
        );
    }

    #[test]
    fn extract_text_from_stream_event_format() {
        // Actual Claude stream-json format with stream_event wrapper
        let stream_output = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello "}}}
{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"world!"}}}
"#;
        let result = extract_text_from_stream_json(stream_output);
        assert_eq!(result, Some("Hello world!".to_string()));
    }

    #[test]
    fn extract_text_from_stream_event_with_plan() {
        // Simulates extracting a plan from Claude's output
        // Note: In JSON, \n represents a newline; when parsed, it becomes an actual newline
        let stream_output = "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"## Plan\\n\\n\"}}}\n\
{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"1. First step\\n\"}}}\n\
{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"2. Second step\\n\"}}}\n";
        let result = extract_text_from_stream_json(stream_output);
        assert_eq!(
            result,
            Some("## Plan\n\n1. First step\n2. Second step\n".to_string())
        );
    }

    #[test]
    fn extract_text_ignores_non_text_events() {
        // Stream with tool use events that should be ignored
        let stream_output = r#"{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}}
{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"The plan"}}}
{"type":"stream_event","event":{"type":"content_block_stop","index":0}}
{"type":"stream_event","event":{"type":"tool_use","name":"read_file"}}
"#;
        let result = extract_text_from_stream_json(stream_output);
        assert_eq!(result, Some("The plan".to_string()));
    }

    #[test]
    fn extract_text_from_result_message() {
        // Result message format
        let stream_output = r#"{"type":"result","result":"Final plan text"}"#;
        let result = extract_text_from_stream_json(stream_output);
        assert_eq!(result, Some("Final plan text".to_string()));
    }

    #[test]
    fn extract_text_from_assistant_message() {
        // Assistant message format
        let stream_output = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Assistant response"}]}}"#;
        let result = extract_text_from_stream_json(stream_output);
        assert_eq!(result, Some("Assistant response".to_string()));
    }

    #[test]
    fn extract_text_returns_none_for_empty() {
        let result = extract_text_from_stream_json("");
        assert_eq!(result, None);
    }

    #[test]
    fn extract_agent_text_from_stream_json() {
        // Claude stream-json format should be parsed
        let stream_output = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello world"}}}"#;
        let result = extract_agent_text(stream_output);
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn extract_agent_text_from_plain_text() {
        // Plain text (e.g., Cursor output) should be returned as-is
        let plain_output = "This is plain text output from the agent.";
        let result = extract_agent_text(plain_output);
        assert_eq!(result, plain_output);
    }

    #[test]
    fn extract_agent_text_empty_returns_empty() {
        let result = extract_agent_text("");
        assert_eq!(result, "");
    }

    #[test]
    fn extract_text_returns_none_for_no_text_content() {
        let stream_output =
            r#"{"type":"stream_event","event":{"type":"tool_use","name":"read_file"}}"#;
        let result = extract_text_from_stream_json(stream_output);
        assert_eq!(result, None);
    }

    #[test]
    fn extract_text_handles_mixed_content() {
        // Mix of stream events and other message types
        let stream_output = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Part 1"}}}
{"type":"result","result":" Part 2"}
"#;
        let result = extract_text_from_stream_json(stream_output);
        assert_eq!(result, Some("Part 1 Part 2".to_string()));
    }

    #[test]
    fn extract_text_uses_only_last_assistant_message() {
        // Simulates a multi-turn conversation where the agent makes tool calls.
        // Each assistant turn produces a JSON line. Only the LAST assistant
        // message should be returned (the final response), not the intermediate
        // narration from tool-use turns.
        let stream_output = concat!(
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Let me explore the codebase..."},{"type":"tool_use","id":"toolu_1","name":"read_file"}]}}"#, "\n",
            r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_1","content":"file contents"}]}}"#, "\n",
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Now let me check another file..."},{"type":"tool_use","id":"toolu_2","name":"read_file"}]}}"#, "\n",
            r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_2","content":"more file contents"}]}}"#, "\n",
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Here are my findings.\n\n1. What approach do you prefer?"}]}}"#, "\n",
        );
        let result = extract_text_from_stream_json(stream_output);
        assert_eq!(
            result,
            Some("Here are my findings.\n\n1. What approach do you prefer?".to_string())
        );
    }

    #[test]
    fn extract_text_single_assistant_message_still_works() {
        // A single assistant message (no tool use) should still be extracted
        let stream_output =
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Direct response"}]}}"#;
        let result = extract_text_from_stream_json(stream_output);
        assert_eq!(result, Some("Direct response".to_string()));
    }

    #[test]
    fn extract_text_stream_events_preferred_over_assistant() {
        // If stream_event deltas are present, they should be used
        // (even if assistant messages are also in the output)
        let stream_output = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Old message"}]}}
{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Streamed response"}}}
"#;
        let result = extract_text_from_stream_json(stream_output);
        assert_eq!(result, Some("Streamed response".to_string()));
    }
}
