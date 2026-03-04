//! Claude Code `AgentProvider` implementation.

use crate::agents::cli_utils;
use crate::agents::cost::{self, RunCostData};
use crate::agents::provider::{AgentProvider, AgentRunConfig};

use super::command;

/// Configuration extracted from the generic `agent_config` map.
#[derive(Debug, Clone, Default)]
pub struct ClaudeApiConfig {
    pub use_local_provider: Option<bool>,
    pub auth_token: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model_override: Option<String>,
    pub thinking_enabled: Option<bool>,
    pub extended_context_enabled: Option<bool>,
    pub chrome_enabled: Option<bool>,
    pub max_turns: Option<u32>,
}

impl ClaudeApiConfig {
    fn get_str(map: &std::collections::HashMap<String, serde_json::Value>, snake: &str, camel: &str) -> Option<String> {
        map.get(snake).or_else(|| map.get(camel)).and_then(|v| v.as_str()).map(|s| s.to_string())
    }

    fn get_bool(map: &std::collections::HashMap<String, serde_json::Value>, snake: &str, camel: &str) -> Option<bool> {
        map.get(snake).or_else(|| map.get(camel)).and_then(|v| v.as_bool())
    }

    fn get_u32(map: &std::collections::HashMap<String, serde_json::Value>, snake: &str, camel: &str) -> Option<u32> {
        map.get(snake).or_else(|| map.get(camel)).and_then(|v| v.as_u64()).map(|v| v as u32)
    }

    /// Accepts both snake_case and camelCase keys for backward compatibility.
    pub fn from_agent_config(map: &std::collections::HashMap<String, serde_json::Value>) -> Self {
        Self {
            use_local_provider: Self::get_bool(map, "use_local_provider", "useLocalProvider"),
            auth_token: Self::get_str(map, "auth_token", "authToken"),
            api_key: Self::get_str(map, "api_key", "apiKey"),
            base_url: Self::get_str(map, "base_url", "baseUrl"),
            model_override: Self::get_str(map, "model_override", "modelOverride"),
            thinking_enabled: Self::get_bool(map, "thinking_enabled", "thinkingEnabled"),
            extended_context_enabled: Self::get_bool(map, "extended_context_enabled", "extendedContextEnabled"),
            chrome_enabled: Self::get_bool(map, "chrome_enabled", "chromeEnabled"),
            max_turns: Self::get_u32(map, "max_turns", "maxTurns"),
        }
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
        command::build_command_from_provider_config(config)
    }

    fn build_env_vars(&self, config: &AgentRunConfig) -> Vec<(String, String)> {
        let api_config = ClaudeApiConfig::from_agent_config(&config.agent_config);
        let mut env_vars = Vec::new();

        if !api_config.use_local_provider.unwrap_or(false)
            || api_config.base_url.as_ref().is_none_or(|s| s.is_empty())
        {
            return env_vars;
        }

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

    fn extract_session_id(&self, output: &str) -> Option<String> {
        extract_session_id_from_stream_json(output)
    }

    fn brand_color(&self) -> Option<&str> {
        Some("#da7756")
    }

    fn available_models(&self) -> Vec<(&str, &str)> {
        vec![
            ("claude-opus-4-6", "Claude Opus 4.6"),
            ("claude-opus-4-5", "Claude Opus 4.5"),
            ("claude-sonnet-4-6", "Claude Sonnet 4.6"),
            ("claude-sonnet-4-5", "Claude Sonnet 4.5"),
        ]
    }

    fn is_local_override(&self, agent_config: &std::collections::HashMap<String, serde_json::Value>) -> bool {
        let api_config = ClaudeApiConfig::from_agent_config(agent_config);
        api_config.use_local_provider.unwrap_or(false)
            && api_config.base_url.as_ref().is_some_and(|s| !s.is_empty())
    }

    fn effective_cost_model(
        &self,
        stage_model: &str,
        agent_config: &std::collections::HashMap<String, serde_json::Value>,
    ) -> String {
        let api_config = ClaudeApiConfig::from_agent_config(agent_config);
        api_config
            .model_override
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| stage_model.to_string())
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
    let mut result_text: Option<String> = None;

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
                        if let Some(r) = json.get("result").and_then(|r| r.as_str()) {
                            result_text = Some(r.to_string());
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
        result_text.or(last_assistant_text)
    }
}

/// Extract `session_id` from stream-json output (shared by Claude Code and Cursor CLIs).
///
/// Looks for the `session_id` field in the `system` init message first, falling back
/// to any `result` or `assistant` message that carries one.
pub fn extract_session_id_from_stream_json(stream_output: &str) -> Option<String> {
    let mut fallback: Option<String> = None;

    for line in stream_output.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }

        let json: serde_json::Value = match serde_json::from_str(line) {
            Ok(j) => j,
            Err(_) => continue,
        };

        if let Some(sid) = json.get("session_id").and_then(|v| v.as_str()) {
            let msg_type = json.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if msg_type == "system" {
                return Some(sid.to_string());
            }
            if fallback.is_none() {
                fallback = Some(sid.to_string());
            }
        }
    }

    fallback
}

