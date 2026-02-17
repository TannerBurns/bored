//! Claude Code `AgentProvider` implementation.

use std::path::Path;

use crate::agents::cli_utils::{self, is_dangerous_command};
use crate::agents::command_templates;
use crate::agents::cost::{self, RunCostData};
use crate::agents::provider::{
    AgentProvider, AgentRunConfig, HookAction, NormalizedHookEvent, StopEventResult,
};

use super::command;
use super::hooks;
use super::settings;

/// Configuration extracted from the generic `agent_config` map.
#[derive(Debug, Clone, Default)]
pub struct ClaudeApiConfig {
    pub auth_token: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model_override: Option<String>,
    pub thinking_enabled: Option<bool>,
    pub extended_context_enabled: Option<bool>,
    pub chrome_enabled: Option<bool>,
}

impl ClaudeApiConfig {
    fn get_str(map: &std::collections::HashMap<String, serde_json::Value>, snake: &str, camel: &str) -> Option<String> {
        map.get(snake).or_else(|| map.get(camel)).and_then(|v| v.as_str()).map(|s| s.to_string())
    }

    fn get_bool(map: &std::collections::HashMap<String, serde_json::Value>, snake: &str, camel: &str) -> Option<bool> {
        map.get(snake).or_else(|| map.get(camel)).and_then(|v| v.as_bool())
    }

    /// Accepts both snake_case and camelCase keys for backward compatibility.
    pub fn from_agent_config(map: &std::collections::HashMap<String, serde_json::Value>) -> Self {
        Self {
            auth_token: Self::get_str(map, "auth_token", "authToken"),
            api_key: Self::get_str(map, "api_key", "apiKey"),
            base_url: Self::get_str(map, "base_url", "baseUrl"),
            model_override: Self::get_str(map, "model_override", "modelOverride"),
            thinking_enabled: Self::get_bool(map, "thinking_enabled", "thinkingEnabled"),
            extended_context_enabled: Self::get_bool(map, "extended_context_enabled", "extendedContextEnabled"),
            chrome_enabled: Self::get_bool(map, "chrome_enabled", "chromeEnabled"),
        }
    }

    /// Convert this config into a generic agent_config map.
    pub fn to_agent_config(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        if let Some(ref v) = self.auth_token {
            map.insert("auth_token".to_string(), serde_json::json!(v));
        }
        if let Some(ref v) = self.api_key {
            map.insert("api_key".to_string(), serde_json::json!(v));
        }
        if let Some(ref v) = self.base_url {
            map.insert("base_url".to_string(), serde_json::json!(v));
        }
        if let Some(ref v) = self.model_override {
            map.insert("model_override".to_string(), serde_json::json!(v));
        }
        if let Some(v) = self.thinking_enabled {
            map.insert("thinking_enabled".to_string(), serde_json::json!(v));
        }
        if let Some(v) = self.extended_context_enabled {
            map.insert(
                "extended_context_enabled".to_string(),
                serde_json::json!(v),
            );
        }
        if let Some(v) = self.chrome_enabled {
            map.insert("chrome_enabled".to_string(), serde_json::json!(v));
        }
        map
    }
}

#[derive(Debug)]
pub struct ClaudeProvider;

impl ClaudeProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClaudeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentProvider for ClaudeProvider {
    fn id(&self) -> &str {
        "claude"
    }

    fn display_name(&self) -> &str {
        "Claude Code"
    }

    fn build_command(&self, config: &AgentRunConfig) -> (String, Vec<String>) {
        let mut config = config.clone();
        config.model = config.model.map(|m| self.map_model_name(&m));
        command::build_command_from_provider_config(&config)
    }

    fn build_env_vars(&self, config: &AgentRunConfig) -> Vec<(String, String)> {
        let api_config = ClaudeApiConfig::from_agent_config(&config.agent_config);
        let mut env_vars = Vec::new();

        if let Some(v) = api_config.auth_token.as_ref().filter(|s| !s.is_empty()) {
            env_vars.push(("ANTHROPIC_AUTH_TOKEN".to_string(), v.clone()));
        }
        if let Some(v) = api_config.api_key.as_ref().filter(|s| !s.is_empty()) {
            env_vars.push(("ANTHROPIC_API_KEY".to_string(), v.clone()));
        }
        if let Some(v) = api_config.base_url.as_ref().filter(|s| !s.is_empty()) {
            env_vars.push(("ANTHROPIC_BASE_URL".to_string(), v.clone()));
        }

        env_vars
    }

    fn extract_text(&self, output: &str) -> String {
        extract_text_from_stream_json(output).unwrap_or_else(|| output.to_string())
    }

    fn extract_cost(
        &self,
        stdout: &str,
        model: &str,
        duration_secs: f64,
    ) -> Option<RunCostData> {
        if let Some(mut parsed) = cost::extract_cost_from_stream_json(stdout) {
            // Backfill model_usage if the API gave us a total but no per-model breakdown
            if parsed.model_usage.is_empty()
                && (parsed.total_cost_usd > 0.0
                    || parsed.input_tokens > 0
                    || parsed.output_tokens > 0
                    || parsed.cache_read_tokens > 0
                    || parsed.cache_creation_tokens > 0)
            {
                parsed.model_usage.insert(
                    cost::normalize_model_name(model),
                    cost::ModelCostData {
                        input_tokens: parsed.input_tokens,
                        output_tokens: parsed.output_tokens,
                        cache_read_tokens: parsed.cache_read_tokens,
                        cache_creation_tokens: parsed.cache_creation_tokens,
                        cost_usd: parsed.total_cost_usd,
                    },
                );
            }
            return Some(parsed);
        }

        // Fall back to estimation if no stream-json cost data
        let output_chars = stdout.len();
        if output_chars > 0 || duration_secs > 0.0 {
            Some(cost::estimate_cost(model, output_chars, duration_secs))
        } else {
            None
        }
    }

    fn is_available(&self) -> bool {
        cli_utils::is_cli_available("claude")
    }

    fn get_version(&self) -> Option<String> {
        cli_utils::get_cli_version("claude")
    }

    fn config_dir_name(&self) -> &str {
        ".claude"
    }

    fn command_instructions_subdir(&self) -> &str {
        "commands"
    }

    fn format_command_reference(&self, command: &str) -> String {
        format!(".claude/commands/{}.md", command)
    }

    fn install_hooks_for_run(
        &self,
        repo_path: &Path,
        hook_script_path: &str,
        api_url: Option<&str>,
        api_token: Option<&str>,
        run_id: Option<&str>,
    ) -> Result<(), String> {
        hooks::install_local_hooks_with_run_id(repo_path, hook_script_path, api_url, api_token, run_id)
            .map_err(|e| format!("Failed to update Claude settings.local.json: {}", e))
    }

    fn check_hooks_installed_global(&self) -> bool {
        settings::check_global_hooks_installed()
    }

    fn check_hooks_installed_project(&self, repo_path: &Path) -> bool {
        settings::check_project_hooks_installed(repo_path)
    }

    fn install_hooks_global(
        &self,
        hook_script_path: &str,
        api_url: Option<&str>,
        api_token: Option<&str>,
    ) -> Result<(), String> {
        hooks::install_user_hooks(hook_script_path, api_url, api_token)
            .map_err(|e| e.to_string())
    }

    fn install_hooks_project(
        &self,
        repo_path: &Path,
        hook_script_path: &str,
        api_url: Option<&str>,
        api_token: Option<&str>,
    ) -> Result<(), String> {
        hooks::install_project_hooks(repo_path, hook_script_path, api_url, api_token)
            .map_err(|e| e.to_string())
    }

    fn generate_hooks_config_json(&self, hook_script_path: &str) -> Result<String, String> {
        let config = hooks::generate_hooks_settings(hook_script_path);
        serde_json::to_string_pretty(&config).map_err(|e| e.to_string())
    }

    fn hook_script_name(&self) -> &str {
        "claude-hook.js"
    }

    fn normalize_hook_event(
        &self,
        raw_event_type: &str,
        raw_payload: &serde_json::Value,
    ) -> NormalizedHookEvent {
        let event_type = match raw_event_type {
            "UserPromptSubmit" => "prompt_submitted",
            "PreToolUse" => "command_requested",
            "PostToolUse" => "command_executed",
            "PostToolUseFailure" => "error",
            "Stop" | "SessionEnd" => "run_stopped",
            "SessionStart" => "run_started",
            other => other,
        };

        let structured = match raw_event_type {
            "PreToolUse" | "PostToolUse" | "PostToolUseFailure" => {
                let tool = raw_payload
                    .get("tool_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let input = raw_payload
                    .get("tool_input")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                match tool {
                    "Bash" => serde_json::json!({
                        "tool": "bash",
                        "command": input.get("command").and_then(|v| v.as_str()).unwrap_or(""),
                        "timeout": input.get("timeout"),
                    }),
                    "Read" => serde_json::json!({
                        "tool": "read",
                        "filePath": input.get("file_path").and_then(|v| v.as_str()).unwrap_or(""),
                    }),
                    "Edit" | "Write" => serde_json::json!({
                        "tool": tool.to_lowercase(),
                        "filePath": input.get("file_path").and_then(|v| v.as_str()).unwrap_or(""),
                    }),
                    _ => serde_json::json!({ "tool": tool, "input": input }),
                }
            }
            "Stop" | "SessionEnd" => serde_json::json!({
                "reason": raw_payload.get("stop_reason").and_then(|v| v.as_str()).unwrap_or(""),
                "transcriptPath": raw_payload.get("transcript_path").and_then(|v| v.as_str()),
            }),
            _ => raw_payload.clone(),
        };

        NormalizedHookEvent {
            event_type: event_type.to_string(),
            structured,
        }
    }

    fn hook_action(
        &self,
        raw_event_type: &str,
        raw_payload: &serde_json::Value,
        ticket_id: Option<&str>,
        run_id: Option<&str>,
    ) -> HookAction {
        match raw_event_type {
            "UserPromptSubmit" => {
                if let Some(tid) = ticket_id {
                    let context = format!(
                        "\n## Agent Kanban Context\n\n\
                         Working on ticket: {}\n\
                         Run ID: {}\n\n\
                         Your actions are being tracked. Please:\n\
                         1. Focus on completing the task\n\
                         2. Make incremental changes\n\
                         3. Commit with descriptive messages\n",
                        tid,
                        run_id.unwrap_or("unknown"),
                    );
                    HookAction::InjectContext { context }
                } else {
                    HookAction::NoAction
                }
            }
            "PreToolUse" => {
                let tool = raw_payload
                    .get("tool_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if tool == "Bash" {
                    let command = raw_payload
                        .get("tool_input")
                        .and_then(|i| i.get("command"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if is_dangerous_command(command) {
                        return HookAction::Deny {
                            reason: format!("Blocked dangerous command: {}", command),
                        };
                    }
                }
                HookAction::Allow
            }
            _ => HookAction::Allow,
        }
    }

    fn normalize_stop_event(
        &self,
        raw_payload: &serde_json::Value,
    ) -> StopEventResult {
        let reason = raw_payload
            .get("stop_reason")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match reason {
            "error" | "tool_error" => StopEventResult {
                status: "error".to_string(),
                exit_code: 1,
                summary: format!("Error: {}", reason),
            },
            "user_cancelled" => StopEventResult {
                status: "aborted".to_string(),
                exit_code: 130,
                summary: "Cancelled by user.".to_string(),
            },
            _ => StopEventResult {
                status: "finished".to_string(),
                exit_code: 0,
                summary: "Completed successfully.".to_string(),
            },
        }
    }

    fn map_model_name(&self, model: &str) -> String {
        crate::agents::models::map_model_name(model)
    }

    fn brand_color(&self) -> Option<&str> {
        Some("#da7756")
    }

    fn check_commands_installed_project(&self, repo_path: &Path) -> bool {
        command_templates::check_project_commands_installed(repo_path, self.config_dir_name())
    }

    fn check_commands_installed_user(&self) -> bool {
        command_templates::check_user_commands_installed(self.config_dir_name())
    }

    fn install_commands_to_project(
        &self,
        repo_path: &Path,
        commands_source: &Path,
    ) -> Result<Vec<String>, String> {
        command_templates::install_commands(repo_path, self.config_dir_name(), commands_source)
            .map_err(|e| format!("Failed to install {} commands: {}", self.display_name(), e))
    }

    fn install_commands_to_user(
        &self,
        commands_source: &Path,
    ) -> Result<Vec<String>, String> {
        command_templates::install_user_commands(self.config_dir_name(), commands_source)
            .map_err(|e| format!("Failed to install {} user commands: {}", self.display_name(), e))
    }
}

/// Extract text content from stream-json output format.
///
/// Handles the NDJSON line format used by both Claude Code and Cursor agent CLIs.
/// Public because it is shared across agent providers and used by modules that
/// need direct parsing (e.g. code review, branch name extraction).
pub fn extract_text_from_stream_json(stream_output: &str) -> Option<String> {
    let mut text_parts = Vec::new();
    let mut last_assistant_text: Option<String> = None;

    for line in stream_output.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(msg_type) = json.get("type").and_then(|t| t.as_str()) {
                match msg_type {
                    "stream_event" => {
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
                        if let Some(result) = json.get("result").and_then(|r| r.as_str()) {
                            text_parts.push(result.to_string());
                        }
                    }
                    "assistant" => {
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

    if !text_parts.is_empty() {
        Some(text_parts.join(""))
    } else {
        last_assistant_text
    }
}

