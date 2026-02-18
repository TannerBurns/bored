//! Tests for the Cursor AgentProvider implementation.

use super::provider::*;
use crate::agents::provider::{AgentProvider, AgentRunConfig};
use std::collections::HashMap;
use std::path::PathBuf;

fn make_config() -> AgentRunConfig {
    AgentRunConfig {
        agent_id: "cursor".to_string(),
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
    let p = CursorProvider::new();
    assert_eq!(p.id(), "cursor");
    assert_eq!(p.display_name(), "Cursor");
}

#[test]
fn build_command_returns_cursor() {
    let p = CursorProvider::new();
    let (cmd, args) = p.build_command(&make_config());
    assert_eq!(cmd, "cursor");
    assert!(args.contains(&"agent".to_string()));
}

#[test]
fn build_env_vars_empty() {
    let p = CursorProvider::new();
    let env = p.build_env_vars(&make_config());
    assert!(env.is_empty());
}

#[test]
fn extract_text_passthrough() {
    let p = CursorProvider::new();
    assert_eq!(p.extract_text("hello world"), "hello world");
}

#[test]
fn extract_cost_estimates() {
    let p = CursorProvider::new();
    let cost = p.extract_cost("some output", "opus-4.6", 10.0);
    assert!(cost.is_some());
    assert!(cost.unwrap().is_estimated);
}

#[test]
fn extract_cost_empty_returns_none() {
    let p = CursorProvider::new();
    let cost = p.extract_cost("", "opus-4.6", 0.0);
    assert!(cost.is_none());
}

// ── map_model_name ─────────────────────────────────────────────

#[test]
fn map_model_name_is_passthrough() {
    let p = CursorProvider::new();
    assert_eq!(p.map_model_name("opus-4.6"), "opus-4.6");
    assert_eq!(p.map_model_name("opus-4.5"), "opus-4.5");
    assert_eq!(p.map_model_name("sonnet-4.6"), "sonnet-4.6");
    assert_eq!(p.map_model_name("sonnet-4.5"), "sonnet-4.5");
    assert_eq!(p.map_model_name("custom-model"), "custom-model");
    assert_eq!(p.map_model_name(""), "");
}

#[test]
fn build_command_maps_model_name_end_to_end() {
    let p = CursorProvider::new();
    let mut config = make_config();
    config.model = Some("opus-4.6".to_string());
    let (_, args) = p.build_command(&config);
    assert!(
        args.contains(&"opus-4.6-thinking".to_string()),
        "Default thinking=true should produce opus-4.6-thinking"
    );
}

#[test]
fn build_command_no_model_omits_model_flag() {
    let p = CursorProvider::new();
    let config = make_config();
    let (_, args) = p.build_command(&config);
    assert!(!args.contains(&"--model".to_string()));
}

// ── Thinking setting ──────────────────────────────────────────

#[test]
fn build_command_thinking_enabled_appends_suffix() {
    let p = CursorProvider::new();
    let mut config = make_config();
    config.model = Some("opus-4.5".to_string());
    config.agent_config.insert(
        "thinking_enabled".to_string(),
        serde_json::json!(true),
    );
    let (_, args) = p.build_command(&config);
    assert!(
        args.contains(&"opus-4.5-thinking".to_string()),
        "thinking_enabled=true should produce opus-4.5-thinking"
    );
}

#[test]
fn build_command_thinking_disabled_no_suffix() {
    let p = CursorProvider::new();
    let mut config = make_config();
    config.model = Some("opus-4.5".to_string());
    config.agent_config.insert(
        "thinking_enabled".to_string(),
        serde_json::json!(false),
    );
    let (_, args) = p.build_command(&config);
    assert!(
        args.contains(&"opus-4.5".to_string()),
        "thinking_enabled=false should produce opus-4.5 without suffix"
    );
    assert!(
        !args.contains(&"opus-4.5-thinking".to_string()),
        "thinking_enabled=false should NOT have -thinking suffix"
    );
}

#[test]
fn build_command_thinking_defaults_to_true() {
    let p = CursorProvider::new();
    let mut config = make_config();
    config.model = Some("sonnet-4.5".to_string());
    let (_, args) = p.build_command(&config);
    assert!(
        args.contains(&"sonnet-4.5-thinking".to_string()),
        "Empty agent_config should default thinking=true"
    );
}

#[test]
fn build_command_sonnet_4_6_appends_thinking_suffix() {
    let p = CursorProvider::new();
    let mut config = make_config();
    config.model = Some("sonnet-4.6".to_string());
    let (_, args) = p.build_command(&config);
    assert!(
        args.contains(&"sonnet-4.6-thinking".to_string()),
        "sonnet-4.6 with default thinking=true should produce sonnet-4.6-thinking"
    );
}

#[test]
fn build_command_thinking_accepts_camel_case_key() {
    let p = CursorProvider::new();
    let mut config = make_config();
    config.model = Some("opus-4.6".to_string());
    config.agent_config.insert(
        "thinkingEnabled".to_string(),
        serde_json::json!(false),
    );
    let (_, args) = p.build_command(&config);
    assert!(
        args.contains(&"opus-4.6".to_string()),
        "camelCase thinkingEnabled=false should work"
    );
    assert!(
        !args.contains(&"opus-4.6-thinking".to_string()),
        "camelCase thinkingEnabled=false should NOT have -thinking suffix"
    );
}

#[test]
fn build_command_thinking_with_unknown_model_appends_suffix() {
    let p = CursorProvider::new();
    let mut config = make_config();
    config.model = Some("custom-model-xyz".to_string());
    let (_, args) = p.build_command(&config);
    assert!(
        args.contains(&"custom-model-xyz-thinking".to_string()),
        "Unknown model with default thinking=true should get -thinking suffix"
    );
}

#[test]
fn build_command_thinking_disabled_with_unknown_model_no_suffix() {
    let p = CursorProvider::new();
    let mut config = make_config();
    config.model = Some("custom-model-xyz".to_string());
    config.agent_config.insert(
        "thinking_enabled".to_string(),
        serde_json::json!(false),
    );
    let (_, args) = p.build_command(&config);
    assert!(
        args.contains(&"custom-model-xyz".to_string()),
        "Unknown model with thinking=false should pass through unchanged"
    );
    assert!(
        !args.contains(&"custom-model-xyz-thinking".to_string()),
        "Unknown model with thinking=false should NOT have -thinking suffix"
    );
}

#[test]
fn build_command_snake_case_takes_precedence_over_camel_case() {
    let p = CursorProvider::new();
    let mut config = make_config();
    config.model = Some("opus-4.5".to_string());
    config.agent_config.insert(
        "thinking_enabled".to_string(),
        serde_json::json!(false),
    );
    config.agent_config.insert(
        "thinkingEnabled".to_string(),
        serde_json::json!(true),
    );
    let (_, args) = p.build_command(&config);
    assert!(
        args.contains(&"opus-4.5".to_string()),
        "snake_case thinking_enabled should take precedence over camelCase"
    );
}

// ── Trait methods coverage ────────────────────────────────────

#[test]
fn config_dir_name_returns_cursor() {
    let p = CursorProvider::new();
    assert_eq!(p.config_dir_name(), ".cursor");
}

#[test]
fn command_instructions_subdir_returns_commands() {
    let p = CursorProvider::new();
    assert_eq!(p.command_instructions_subdir(), "commands");
}

#[test]
fn format_command_reference_returns_slash_command() {
    let p = CursorProvider::new();
    assert_eq!(p.format_command_reference("deslop"), "/deslop");
    assert_eq!(p.format_command_reference("add-and-commit"), "/add-and-commit");
}

#[test]
fn check_commands_installed_project_returns_false_for_missing_dir() {
    let temp = std::env::temp_dir().join(format!("cursor_prov_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp).unwrap();
    let p = CursorProvider::new();
    assert!(!p.check_commands_installed_project(&temp));
    std::fs::remove_dir_all(&temp).ok();
}

#[test]
fn available_models_includes_claude_and_codex_models() {
    let p = CursorProvider::new();
    let models = p.available_models();
    assert!(models.len() >= 4);
    let ids: Vec<&str> = models.iter().map(|(id, _)| *id).collect();
    assert!(ids.contains(&"opus-4.6"), "should include Claude models");
    assert!(ids.contains(&"sonnet-4.6"), "should include Claude models");
    assert!(ids.contains(&"gpt-5.3-codex"), "should include Codex models");
    assert!(ids.contains(&"gpt-5.2-codex"), "should include Codex models");
    for (id, label) in &models {
        assert!(!id.is_empty());
        assert!(!label.is_empty());
    }
}
