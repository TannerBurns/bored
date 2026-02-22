//! Codex `AgentProvider` implementation.
//!
//! Codex CLI outputs NDJSON with `item.completed` and `turn.completed` events.
//! This provider parses that format for text extraction and cost data.

use crate::agents::cli_utils;
use crate::agents::cost::RunCostData;
use crate::agents::provider::{AgentProvider, AgentRunConfig};

use super::command;

/// Configuration extracted from the generic `agent_config` map.
#[derive(Debug, Clone, Default)]
pub struct CodexApiConfig {
    pub oss_enabled: Option<bool>,
    pub local_provider: Option<String>,
    pub model_override: Option<String>,
    pub reasoning_effort: Option<String>,
    pub multi_agent_enabled: Option<bool>,
}

impl CodexApiConfig {
    fn get_str(map: &std::collections::HashMap<String, serde_json::Value>, snake: &str, camel: &str) -> Option<String> {
        map.get(snake).or_else(|| map.get(camel)).and_then(|v| v.as_str()).map(|s| s.to_string())
    }

    fn get_bool(map: &std::collections::HashMap<String, serde_json::Value>, snake: &str, camel: &str) -> Option<bool> {
        map.get(snake).or_else(|| map.get(camel)).and_then(|v| v.as_bool())
    }

    /// Accepts both snake_case and camelCase keys for backward compatibility.
    pub fn from_agent_config(map: &std::collections::HashMap<String, serde_json::Value>) -> Self {
        Self {
            oss_enabled: Self::get_bool(map, "oss_enabled", "ossEnabled"),
            local_provider: Self::get_str(map, "local_provider", "localProvider"),
            model_override: Self::get_str(map, "model_override", "modelOverride"),
            reasoning_effort: Self::get_str(map, "reasoning_effort", "reasoningEffort"),
            multi_agent_enabled: Self::get_bool(map, "multi_agent_enabled", "multiAgentEnabled"),
        }
    }
}

#[derive(Debug)]
pub struct CodexProvider;

impl CodexProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CodexProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentProvider for CodexProvider {
    fn id(&self) -> &str {
        "codex"
    }

    fn display_name(&self) -> &str {
        "Codex"
    }

    fn build_command(&self, config: &AgentRunConfig) -> (String, Vec<String>) {
        command::build_command_from_provider_config(config)
    }

    fn build_env_vars(&self, _config: &AgentRunConfig) -> Vec<(String, String)> {
        Vec::new()
    }

    fn extract_text(&self, output: &str) -> String {
        extract_text_from_codex_json(output).unwrap_or_else(|| output.to_string())
    }

    fn extract_cost(
        &self,
        stdout: &str,
        model: &str,
        duration_secs: f64,
    ) -> Option<RunCostData> {
        if let Some(cost) = extract_cost_from_codex_json(stdout, model) {
            return Some(cost);
        }
        let output_chars = stdout.len();
        if output_chars > 0 || duration_secs > 0.0 {
            Some(crate::agents::cost::estimate_cost(model, output_chars, duration_secs))
        } else {
            None
        }
    }

    fn is_available(&self) -> bool {
        cli_utils::is_cli_available("codex")
    }

    fn get_version(&self) -> Option<String> {
        cli_utils::get_cli_version("codex")
    }

    fn config_dir_name(&self) -> &str {
        ".codex"
    }

    fn command_instructions_subdir(&self) -> &str {
        "commands"
    }

    fn format_command_reference(&self, command: &str) -> String {
        format!("(see {} instructions)", command)
    }

    fn brand_color(&self) -> Option<&str> {
        Some("#0ea5e9")
    }

    fn available_models(&self) -> Vec<(&str, &str)> {
        vec![
            ("gpt-5.3-codex", "GPT-5.3 Codex"),
            ("gpt-5.2-codex", "GPT-5.2 Codex"),
        ]
    }

    fn is_local_override(&self, agent_config: &std::collections::HashMap<String, serde_json::Value>) -> bool {
        let api_config = CodexApiConfig::from_agent_config(agent_config);
        api_config.oss_enabled.unwrap_or(false)
            && api_config.local_provider.as_ref().is_some_and(|s| !s.is_empty())
    }

    fn effective_cost_model(
        &self,
        stage_model: &str,
        agent_config: &std::collections::HashMap<String, serde_json::Value>,
    ) -> String {
        let api_config = CodexApiConfig::from_agent_config(agent_config);
        api_config
            .model_override
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| stage_model.to_string())
    }
}

/// Extract text from Codex NDJSON output.
///
/// Collects text from `item.completed` events. Prioritises `agent_message`
/// items (the assistant's final answer). When no `agent_message` is found,
/// falls back to `command_execution` aggregated output so that callers still
/// receive meaningful content even when the agent only executes commands.
fn extract_text_from_codex_json(output: &str) -> Option<String> {
    let mut agent_texts = Vec::new();
    let mut command_outputs = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }

        let json = match serde_json::from_str::<serde_json::Value>(line) {
            Ok(j) => j,
            Err(_) => continue,
        };

        if json.get("type").and_then(|t| t.as_str()) != Some("item.completed") {
            continue;
        }

        let item = match json.get("item") {
            Some(i) => i,
            None => continue,
        };

        match item.get("type").and_then(|t| t.as_str()).unwrap_or("") {
            "agent_message" => {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    agent_texts.push(text.to_string());
                }
            }
            "command_execution" => {
                if let Some(out) = item.get("aggregated_output").and_then(|t| t.as_str()) {
                    if !out.is_empty() {
                        command_outputs.push(out.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    if !agent_texts.is_empty() {
        Some(agent_texts.join("\n"))
    } else if !command_outputs.is_empty() {
        Some(command_outputs.join("\n"))
    } else {
        None
    }
}

/// Extract cost/token data from `turn.completed` events in Codex NDJSON output.
fn extract_cost_from_codex_json(output: &str, model: &str) -> Option<RunCostData> {
    let mut total_input = 0u64;
    let mut total_cached = 0u64;
    let mut total_output = 0u64;
    let mut found = false;

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            if json.get("type").and_then(|t| t.as_str()) == Some("turn.completed") {
                if let Some(usage) = json.get("usage") {
                    total_input += usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    total_cached += usage.get("cached_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    total_output += usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    found = true;
                }
            }
        }
    }

    if !found {
        return None;
    }

    let normalized_model = crate::agents::cost::normalize_model_name(model);
    let mut model_usage = std::collections::HashMap::new();
    let cost_usd = crate::agents::cost::compute_cost_from_tokens(
        model,
        total_input,
        total_output,
        total_cached,
        0,
    );
    model_usage.insert(
        normalized_model,
        crate::agents::cost::ModelCostData {
            input_tokens: total_input,
            output_tokens: total_output,
            cache_read_tokens: total_cached,
            cache_creation_tokens: 0,
            cost_usd,
        },
    );

    Some(RunCostData {
        input_tokens: total_input,
        output_tokens: total_output,
        cache_read_tokens: total_cached,
        cache_creation_tokens: 0,
        total_cost_usd: cost_usd,
        model_usage,
        is_estimated: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn from_agent_config_reasoning_effort_camel_case() {
        let mut map = HashMap::new();
        map.insert("reasoningEffort".into(), serde_json::json!("xhigh"));
        let config = CodexApiConfig::from_agent_config(&map);
        assert_eq!(config.reasoning_effort, Some("xhigh".to_string()));
    }

    #[test]
    fn from_agent_config_reasoning_effort_snake_case() {
        let mut map = HashMap::new();
        map.insert("reasoning_effort".into(), serde_json::json!("low"));
        let config = CodexApiConfig::from_agent_config(&map);
        assert_eq!(config.reasoning_effort, Some("low".to_string()));
    }

    #[test]
    fn from_agent_config_reasoning_effort_snake_case_takes_precedence() {
        let mut map = HashMap::new();
        map.insert("reasoning_effort".into(), serde_json::json!("low"));
        map.insert("reasoningEffort".into(), serde_json::json!("xhigh"));
        let config = CodexApiConfig::from_agent_config(&map);
        assert_eq!(config.reasoning_effort, Some("low".to_string()));
    }

    #[test]
    fn from_agent_config_reasoning_effort_missing() {
        let map = HashMap::new();
        let config = CodexApiConfig::from_agent_config(&map);
        assert_eq!(config.reasoning_effort, None);
    }

    #[test]
    fn from_agent_config_reasoning_effort_non_string_ignored() {
        let mut map = HashMap::new();
        map.insert("reasoningEffort".into(), serde_json::json!(42));
        let config = CodexApiConfig::from_agent_config(&map);
        assert_eq!(config.reasoning_effort, None);
    }

    #[test]
    fn from_agent_config_multi_agent_enabled_camel_case() {
        let mut map = HashMap::new();
        map.insert("multiAgentEnabled".into(), serde_json::json!(true));
        let config = CodexApiConfig::from_agent_config(&map);
        assert_eq!(config.multi_agent_enabled, Some(true));
    }

    #[test]
    fn from_agent_config_multi_agent_enabled_snake_case() {
        let mut map = HashMap::new();
        map.insert("multi_agent_enabled".into(), serde_json::json!(false));
        let config = CodexApiConfig::from_agent_config(&map);
        assert_eq!(config.multi_agent_enabled, Some(false));
    }

    #[test]
    fn from_agent_config_multi_agent_enabled_missing() {
        let map = HashMap::new();
        let config = CodexApiConfig::from_agent_config(&map);
        assert_eq!(config.multi_agent_enabled, None);
    }
}
