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

#[test]
fn claude_api_config_roundtrip_all_fields() {
    let original = ClaudeApiConfig {
        auth_token: Some("auth".to_string()),
        api_key: Some("key".to_string()),
        base_url: Some("https://api.example.com".to_string()),
        model_override: Some("custom".to_string()),
        thinking_enabled: Some(false),
        extended_context_enabled: Some(true),
        chrome_enabled: Some(true),
    };
    let map = original.to_agent_config();
    assert_eq!(map.len(), 7);
    let restored = ClaudeApiConfig::from_agent_config(&map);
    assert_eq!(restored.auth_token.as_deref(), Some("auth"));
    assert_eq!(restored.api_key.as_deref(), Some("key"));
    assert_eq!(restored.base_url.as_deref(), Some("https://api.example.com"));
    assert_eq!(restored.model_override.as_deref(), Some("custom"));
    assert_eq!(restored.thinking_enabled, Some(false));
    assert_eq!(restored.extended_context_enabled, Some(true));
    assert_eq!(restored.chrome_enabled, Some(true));
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
    config
        .agent_config
        .insert("api_key".to_string(), serde_json::json!("k"));
    config
        .agent_config
        .insert("base_url".to_string(), serde_json::json!("https://x.com"));
    let env = p.build_env_vars(&config);
    assert!(env.iter().any(|(k, v)| k == "ANTHROPIC_API_KEY" && v == "k"));
    assert!(env
        .iter()
        .any(|(k, v)| k == "ANTHROPIC_BASE_URL" && v == "https://x.com"));
}

#[test]
fn build_env_vars_all_three_vars() {
    let p = ClaudeProvider::new();
    let mut config = make_config();
    config
        .agent_config
        .insert("auth_token".to_string(), serde_json::json!("a"));
    config
        .agent_config
        .insert("api_key".to_string(), serde_json::json!("b"));
    config
        .agent_config
        .insert("base_url".to_string(), serde_json::json!("c"));
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
