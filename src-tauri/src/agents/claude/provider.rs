//! Claude Code `AgentProvider` implementation.

use std::path::Path;

use crate::agents::cost::{self, RunCostData};
use crate::agents::provider::{AgentProvider, AgentRunConfig};

use super::availability;
use super::command;
use super::hooks;

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
    /// Extract Claude-specific config from the generic agent_config map.
    pub fn from_agent_config(map: &std::collections::HashMap<String, serde_json::Value>) -> Self {
        Self {
            auth_token: map
                .get("auth_token")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            api_key: map
                .get("api_key")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            base_url: map
                .get("base_url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            model_override: map
                .get("model_override")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            thinking_enabled: map.get("thinking_enabled").and_then(|v| v.as_bool()),
            extended_context_enabled: map
                .get("extended_context_enabled")
                .and_then(|v| v.as_bool()),
            chrome_enabled: map.get("chrome_enabled").and_then(|v| v.as_bool()),
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
        command::build_command_from_provider_config(config)
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
        availability::is_claude_available()
    }

    fn get_version(&self) -> Option<String> {
        availability::get_claude_version()
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
}

/// Extract text content from Claude's stream-json format.
///
/// This is also used directly by modules that need Claude-specific parsing
/// (e.g. code review, branch name extraction) and is kept public for that reason.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn make_config() -> AgentRunConfig {
        AgentRunConfig {
            agent_id: "claude".to_string(),
            ticket_id: "t".to_string(),
            run_id: "r".to_string(),
            repo_path: PathBuf::from("/tmp/test"),
            prompt: "Test".to_string(),
            timeout_secs: None,
            api_url: "http://localhost:7432".to_string(),
            api_token: "tok".to_string(),
            model: None,
            agent_config: HashMap::new(),
        }
    }

    #[test]
    fn provider_id_and_display_name() {
        let p = ClaudeProvider::new();
        assert_eq!(p.id(), "claude");
        assert_eq!(p.display_name(), "Claude Code");
    }

    #[test]
    fn build_command_returns_claude() {
        let p = ClaudeProvider::new();
        let (cmd, _) = p.build_command(&make_config());
        assert_eq!(cmd, "claude");
    }

    #[test]
    fn build_env_vars_empty_when_no_config() {
        let p = ClaudeProvider::new();
        let env = p.build_env_vars(&make_config());
        assert!(env.is_empty());
    }

    #[test]
    fn build_env_vars_includes_auth_token() {
        let p = ClaudeProvider::new();
        let mut config = make_config();
        config
            .agent_config
            .insert("auth_token".to_string(), serde_json::json!("my-token"));
        let env = p.build_env_vars(&config);
        assert!(env
            .iter()
            .any(|(k, v)| k == "ANTHROPIC_AUTH_TOKEN" && v == "my-token"));
    }

    #[test]
    fn build_env_vars_skips_empty_values() {
        let p = ClaudeProvider::new();
        let mut config = make_config();
        config
            .agent_config
            .insert("auth_token".to_string(), serde_json::json!(""));
        let env = p.build_env_vars(&config);
        assert!(!env.iter().any(|(k, _)| k == "ANTHROPIC_AUTH_TOKEN"));
    }

    #[test]
    fn extract_text_plain_passthrough() {
        let p = ClaudeProvider::new();
        assert_eq!(p.extract_text("plain output"), "plain output");
    }

    #[test]
    fn extract_text_stream_json() {
        let p = ClaudeProvider::new();
        let input = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}}"#;
        assert_eq!(p.extract_text(input), "Hello");
    }

    #[test]
    fn claude_api_config_roundtrip() {
        let original = ClaudeApiConfig {
            auth_token: Some("tok".to_string()),
            thinking_enabled: Some(true),
            ..Default::default()
        };
        let map = original.to_agent_config();
        let restored = ClaudeApiConfig::from_agent_config(&map);
        assert_eq!(restored.auth_token.as_deref(), Some("tok"));
        assert_eq!(restored.thinking_enabled, Some(true));
        assert!(restored.api_key.is_none());
    }
}
