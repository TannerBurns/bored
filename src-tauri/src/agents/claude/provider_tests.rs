//! Tests for the Claude AgentProvider implementation.

use super::provider::*;
use crate::agents::provider::{AgentProvider, AgentRunConfig};
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
    config.agent_config.insert("use_local_provider".into(), serde_json::json!(true));
    config.agent_config.insert("base_url".into(), serde_json::json!("http://localhost:8080"));
    config.agent_config.insert("auth_token".into(), serde_json::json!("my-token"));
    let env = p.build_env_vars(&config);
    assert!(env.iter().any(|(k, v)| k == "ANTHROPIC_AUTH_TOKEN" && v == "my-token"));
}

#[test]
fn build_env_vars_skips_empty_values() {
    let p = ClaudeProvider::new();
    let mut config = make_config();
    config.agent_config.insert("use_local_provider".into(), serde_json::json!(true));
    config.agent_config.insert("base_url".into(), serde_json::json!("http://localhost:8080"));
    config.agent_config.insert("auth_token".into(), serde_json::json!(""));
    let env = p.build_env_vars(&config);
    assert!(!env.iter().any(|(k, _)| k == "ANTHROPIC_AUTH_TOKEN"));
}

#[test]
fn build_env_vars_empty_when_local_provider_enabled_without_base_url() {
    let p = ClaudeProvider::new();
    let mut config = make_config();
    config.agent_config.insert("use_local_provider".into(), serde_json::json!(true));
    config.agent_config.insert("auth_token".into(), serde_json::json!("tok"));
    config.agent_config.insert("api_key".into(), serde_json::json!("key"));
    let env = p.build_env_vars(&config);
    assert!(env.is_empty(), "env vars should not be set without a base_url");
}

#[test]
fn build_env_vars_empty_when_local_provider_disabled() {
    let p = ClaudeProvider::new();
    let mut config = make_config();
    config.agent_config.insert("use_local_provider".into(), serde_json::json!(false));
    config.agent_config.insert("auth_token".into(), serde_json::json!("tok"));
    config.agent_config.insert("base_url".into(), serde_json::json!("http://localhost:8080"));
    let env = p.build_env_vars(&config);
    assert!(env.is_empty(), "env vars should not be set when local provider is disabled");
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
fn claude_api_config_from_empty_map() {
    let map = HashMap::new();
    let config = ClaudeApiConfig::from_agent_config(&map);
    assert!(config.auth_token.is_none());
    assert!(config.api_key.is_none());
    assert!(config.thinking_enabled.is_none());
}

#[test]
fn claude_api_config_ignores_wrong_types() {
    let mut map = HashMap::new();
    map.insert("auth_token".to_string(), serde_json::json!(123));
    map.insert("thinking_enabled".to_string(), serde_json::json!("yes"));
    let config = ClaudeApiConfig::from_agent_config(&map);
    assert!(config.auth_token.is_none());
    assert!(config.thinking_enabled.is_none());
}

// ── build_env_vars additional coverage ──────────────────────────

#[test]
fn build_env_vars_includes_api_key_and_base_url() {
    let p = ClaudeProvider::new();
    let mut config = make_config();
    config.agent_config.insert("use_local_provider".into(), serde_json::json!(true));
    config.agent_config.insert("api_key".into(), serde_json::json!("k"));
    config.agent_config.insert("base_url".into(), serde_json::json!("https://x.com"));
    let env = p.build_env_vars(&config);
    assert!(env.iter().any(|(k, v)| k == "ANTHROPIC_API_KEY" && v == "k"));
    assert!(env.iter().any(|(k, v)| k == "ANTHROPIC_BASE_URL" && v == "https://x.com"));
}

#[test]
fn build_env_vars_all_three_vars() {
    let p = ClaudeProvider::new();
    let mut config = make_config();
    config.agent_config.insert("use_local_provider".into(), serde_json::json!(true));
    config.agent_config.insert("auth_token".into(), serde_json::json!("a"));
    config.agent_config.insert("api_key".into(), serde_json::json!("b"));
    config.agent_config.insert("base_url".into(), serde_json::json!("c"));
    let env = p.build_env_vars(&config);
    assert_eq!(env.len(), 3);
}

// ── extract_cost coverage ───────────────────────────────────────

#[test]
fn extract_cost_parses_stream_json() {
    let p = ClaudeProvider::new();
    let stream = r#"{"type":"result","result":"text","usage":{"input_tokens":100,"output_tokens":50,"total_cost_usd":0.01},"modelUsage":{"claude-opus-4-6":{"inputTokens":100,"outputTokens":50,"costUSD":0.01}}}"#;
    let cost = p.extract_cost(stream, "opus-4.6", 5.0).unwrap();
    assert!(!cost.is_estimated);
    assert_eq!(cost.input_tokens, 100);
}

#[test]
fn extract_cost_backfills_model_usage() {
    let p = ClaudeProvider::new();
    let stream = r#"{"type":"result","result":"text","usage":{"input_tokens":50,"output_tokens":25,"total_cost_usd":0.01}}"#;
    let cost = p.extract_cost(stream, "opus-4.6", 5.0).unwrap();
    assert!(!cost.is_estimated);
    assert!(cost.model_usage.contains_key("opus-4.6"));
    assert_eq!(cost.model_usage["opus-4.6"].input_tokens, 50);
}

#[test]
fn extract_cost_falls_back_to_estimation() {
    let p = ClaudeProvider::new();
    let cost = p
        .extract_cost("plain text output", "opus-4.6", 10.0)
        .unwrap();
    assert!(cost.is_estimated);
    assert!(cost.total_cost_usd > 0.0);
}

#[test]
fn extract_cost_empty_returns_none() {
    let p = ClaudeProvider::new();
    assert!(p.extract_cost("", "opus-4.6", 0.0).is_none());
}

// ── map_model_name ─────────────────────────────────────────────

#[test]
fn map_model_name_maps_known_models() {
    let p = ClaudeProvider::new();
    assert_eq!(p.map_model_name("opus-4.6"), "claude-opus-4-6");
    assert_eq!(p.map_model_name("opus-4.5"), "claude-opus-4-5");
    assert_eq!(p.map_model_name("sonnet-4.6"), "claude-sonnet-4-6");
    assert_eq!(p.map_model_name("sonnet-4.5"), "claude-sonnet-4-5");
}

#[test]
fn map_model_name_passes_through_unknown() {
    let p = ClaudeProvider::new();
    assert_eq!(p.map_model_name("custom-model"), "custom-model");
    assert_eq!(p.map_model_name("claude-opus-4-6"), "claude-opus-4-6");
}

#[test]
fn build_command_maps_model_name_end_to_end() {
    let p = ClaudeProvider::new();
    let mut config = make_config();
    config.model = Some("opus-4.6".to_string());
    let (_, args) = p.build_command(&config);
    assert!(
        args.contains(&"claude-opus-4-6".to_string()),
        "Provider build_command should map opus-4.6 -> claude-opus-4-6"
    );
}

// ── New trait methods coverage ───────────────────────────────────

#[test]
fn config_dir_name_returns_claude() {
    let p = ClaudeProvider::new();
    assert_eq!(p.config_dir_name(), ".claude");
}

#[test]
fn command_instructions_subdir_returns_commands() {
    let p = ClaudeProvider::new();
    assert_eq!(p.command_instructions_subdir(), "commands");
}

#[test]
fn format_command_reference_returns_file_path() {
    let p = ClaudeProvider::new();
    assert_eq!(
        p.format_command_reference("deslop"),
        ".claude/commands/deslop.md"
    );
    assert_eq!(
        p.format_command_reference("add-and-commit"),
        ".claude/commands/add-and-commit.md"
    );
}

#[test]
fn check_commands_installed_project_returns_false_for_missing_dir() {
    let temp = std::env::temp_dir().join(format!("claude_prov_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp).unwrap();
    let p = ClaudeProvider::new();
    assert!(!p.check_commands_installed_project(&temp));
    std::fs::remove_dir_all(&temp).ok();
}

#[test]
fn check_commands_installed_user_returns_bool() {
    let p = ClaudeProvider::new();
    let _ = p.check_commands_installed_user();
}

#[test]
fn available_models_returns_claude_models() {
    let p = ClaudeProvider::new();
    let models = p.available_models();
    assert!(!models.is_empty());
    let ids: Vec<&str> = models.iter().map(|(id, _)| *id).collect();
    assert!(ids.contains(&"opus-4.6"));
    assert!(ids.contains(&"opus-4.5"));
    assert!(ids.contains(&"sonnet-4.6"));
    assert!(ids.contains(&"sonnet-4.5"));
    for (id, label) in &models {
        assert!(!id.is_empty());
        assert!(!label.is_empty());
    }
}

// ── Local provider override tests ─────────────────────────────────

#[test]
fn is_local_override_false_when_empty_config() {
    let p = ClaudeProvider::new();
    let map = HashMap::new();
    assert!(!p.is_local_override(&map));
}

#[test]
fn is_local_override_false_when_no_base_url() {
    let p = ClaudeProvider::new();
    let mut map = HashMap::new();
    map.insert("api_key".into(), serde_json::json!("key"));
    assert!(!p.is_local_override(&map));
}

#[test]
fn is_local_override_false_when_base_url_empty() {
    let p = ClaudeProvider::new();
    let mut map = HashMap::new();
    map.insert("base_url".into(), serde_json::json!(""));
    assert!(!p.is_local_override(&map));
}

#[test]
fn is_local_override_false_when_toggle_off_with_base_url() {
    let p = ClaudeProvider::new();
    let mut map = HashMap::new();
    map.insert("use_local_provider".into(), serde_json::json!(false));
    map.insert("base_url".into(), serde_json::json!("http://localhost:8080"));
    assert!(!p.is_local_override(&map));
}

#[test]
fn is_local_override_true_when_toggle_on_and_base_url_set() {
    let p = ClaudeProvider::new();
    let mut map = HashMap::new();
    map.insert("use_local_provider".into(), serde_json::json!(true));
    map.insert("base_url".into(), serde_json::json!("http://localhost:8080"));
    assert!(p.is_local_override(&map));
}

#[test]
fn is_local_override_true_with_camel_case_keys() {
    let p = ClaudeProvider::new();
    let mut map = HashMap::new();
    map.insert("useLocalProvider".into(), serde_json::json!(true));
    map.insert("baseUrl".into(), serde_json::json!("http://192.168.1.10:5000"));
    assert!(p.is_local_override(&map));
}

#[test]
fn is_local_override_false_when_toggle_on_but_no_base_url() {
    let p = ClaudeProvider::new();
    let mut map = HashMap::new();
    map.insert("use_local_provider".into(), serde_json::json!(true));
    assert!(!p.is_local_override(&map));
}

#[test]
fn effective_cost_model_returns_stage_model_when_no_override() {
    let p = ClaudeProvider::new();
    let map = HashMap::new();
    assert_eq!(p.effective_cost_model("opus-4.6", &map), "opus-4.6");
}

#[test]
fn effective_cost_model_returns_override_when_set() {
    let p = ClaudeProvider::new();
    let mut map = HashMap::new();
    map.insert("model_override".into(), serde_json::json!("my-local-llama"));
    assert_eq!(p.effective_cost_model("opus-4.6", &map), "my-local-llama");
}

#[test]
fn effective_cost_model_falls_back_when_override_empty() {
    let p = ClaudeProvider::new();
    let mut map = HashMap::new();
    map.insert("modelOverride".into(), serde_json::json!(""));
    assert_eq!(p.effective_cost_model("sonnet-4.5", &map), "sonnet-4.5");
}

#[test]
fn extract_cost_with_local_override_tracks_override_model_and_zero_cost() {
    let p = ClaudeProvider::new();
    let mut map = HashMap::new();
    map.insert("use_local_provider".into(), serde_json::json!(true));
    map.insert("base_url".into(), serde_json::json!("http://localhost:8080"));
    map.insert("model_override".into(), serde_json::json!("my-local-model"));

    let effective = p.effective_cost_model("opus-4.6", &map);
    assert_eq!(effective, "my-local-model");
    assert!(p.is_local_override(&map));

    let stream = r#"{"type":"result","result":"text","usage":{"input_tokens":200,"output_tokens":100,"total_cost_usd":0.05}}"#;
    let mut cost = p.extract_cost(stream, &effective, 5.0).unwrap();
    assert_eq!(cost.input_tokens, 200);
    assert_eq!(cost.output_tokens, 100);
    assert!(cost.model_usage.contains_key("my-local-model"));

    cost.zero_out_costs();
    assert_eq!(cost.total_cost_usd, 0.0);
    assert_eq!(cost.model_usage["my-local-model"].cost_usd, 0.0);
    assert_eq!(cost.model_usage["my-local-model"].input_tokens, 200);
    assert_eq!(cost.model_usage["my-local-model"].output_tokens, 100);
}

#[test]
fn extract_cost_estimation_fallback_uses_override_model_name() {
    let p = ClaudeProvider::new();
    let mut map = HashMap::new();
    map.insert("useLocalProvider".into(), serde_json::json!(true));
    map.insert("baseUrl".into(), serde_json::json!("http://localhost:11434"));
    map.insert("modelOverride".into(), serde_json::json!("mistral-nemo"));

    let effective = p.effective_cost_model("sonnet-4.5", &map);
    assert_eq!(effective, "mistral-nemo");

    let mut cost = p.extract_cost("plain text without stream-json", &effective, 8.0).unwrap();
    assert!(cost.is_estimated);
    assert!(cost.model_usage.contains_key("mistral-nemo"));

    cost.zero_out_costs();
    assert_eq!(cost.total_cost_usd, 0.0);
    assert!(cost.model_usage["mistral-nemo"].input_tokens > 0);
}

#[test]
fn effective_cost_model_with_camel_case_override() {
    let p = ClaudeProvider::new();
    let mut map = HashMap::new();
    map.insert("modelOverride".into(), serde_json::json!("qwen2.5-coder"));
    assert_eq!(p.effective_cost_model("opus-4.6", &map), "qwen2.5-coder");
}

// is_dangerous_command tests live in agents::cli_utils::tests
