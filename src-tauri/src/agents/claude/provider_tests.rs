//! Tests for the Claude AgentProvider implementation.

use super::provider::*;
use crate::agents::provider::{AgentProvider, AgentRunConfig, HookAction};
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

// ── map_model_name ─────────────────────────────────────────────

#[test]
fn map_model_name_maps_known_models() {
    let p = ClaudeProvider::new();
    assert_eq!(p.map_model_name("opus-4.6"), "claude-opus-4-6");
    assert_eq!(p.map_model_name("opus-4.5"), "claude-opus-4-5");
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

// ── Hook normalization tests ───────────────────────────────────────

#[test]
fn normalize_hook_event_pretooluse_bash() {
    let p = ClaudeProvider::new();
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": { "command": "ls -la", "timeout": 30 }
    });
    let result = p.normalize_hook_event("PreToolUse", &payload);
    assert_eq!(result.event_type, "command_requested");
    assert_eq!(result.structured["tool"], "bash");
    assert_eq!(result.structured["command"], "ls -la");
}

#[test]
fn normalize_hook_event_pretooluse_read() {
    let p = ClaudeProvider::new();
    let payload = serde_json::json!({
        "tool_name": "Read",
        "tool_input": { "file_path": "/src/main.rs" }
    });
    let result = p.normalize_hook_event("PreToolUse", &payload);
    assert_eq!(result.event_type, "command_requested");
    assert_eq!(result.structured["tool"], "read");
    assert_eq!(result.structured["filePath"], "/src/main.rs");
}

#[test]
fn normalize_hook_event_pretooluse_edit() {
    let p = ClaudeProvider::new();
    let payload = serde_json::json!({
        "tool_name": "Edit",
        "tool_input": { "file_path": "/src/lib.rs" }
    });
    let result = p.normalize_hook_event("PreToolUse", &payload);
    assert_eq!(result.structured["tool"], "edit");
    assert_eq!(result.structured["filePath"], "/src/lib.rs");
}

#[test]
fn normalize_hook_event_pretooluse_unknown_tool() {
    let p = ClaudeProvider::new();
    let payload = serde_json::json!({
        "tool_name": "WebSearch",
        "tool_input": { "query": "rust async" }
    });
    let result = p.normalize_hook_event("PreToolUse", &payload);
    assert_eq!(result.event_type, "command_requested");
    assert_eq!(result.structured["tool"], "WebSearch");
}

#[test]
fn normalize_hook_event_stop() {
    let p = ClaudeProvider::new();
    let payload = serde_json::json!({
        "stop_reason": "end_turn",
        "transcript_path": "/tmp/transcript.json"
    });
    let result = p.normalize_hook_event("Stop", &payload);
    assert_eq!(result.event_type, "run_stopped");
    assert_eq!(result.structured["reason"], "end_turn");
}

#[test]
fn normalize_hook_event_user_prompt() {
    let p = ClaudeProvider::new();
    let payload = serde_json::json!({ "prompt": "Fix the bug" });
    let result = p.normalize_hook_event("UserPromptSubmit", &payload);
    assert_eq!(result.event_type, "prompt_submitted");
}

#[test]
fn normalize_hook_event_session_start() {
    let p = ClaudeProvider::new();
    let result = p.normalize_hook_event("SessionStart", &serde_json::json!({}));
    assert_eq!(result.event_type, "run_started");
}

#[test]
fn normalize_hook_event_unknown_passes_through() {
    let p = ClaudeProvider::new();
    let payload = serde_json::json!({ "foo": "bar" });
    let result = p.normalize_hook_event("CustomEvent", &payload);
    assert_eq!(result.event_type, "CustomEvent");
    assert_eq!(result.structured, payload);
}

// ── Hook action tests ──────────────────────────────────────────────

#[test]
fn hook_action_pretooluse_allows_safe_command() {
    let p = ClaudeProvider::new();
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": { "command": "cargo test" }
    });
    let action = p.hook_action("PreToolUse", &payload, Some("t1"), Some("r1"));
    assert_eq!(action, HookAction::Allow);
}

#[test]
fn hook_action_pretooluse_denies_dangerous_command() {
    let p = ClaudeProvider::new();
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": { "command": "rm -rf /" }
    });
    let action = p.hook_action("PreToolUse", &payload, Some("t1"), Some("r1"));
    assert!(matches!(action, HookAction::Deny { .. }));
}

#[test]
fn hook_action_pretooluse_denies_force_push() {
    let p = ClaudeProvider::new();
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": { "command": "git push origin main --force" }
    });
    let action = p.hook_action("PreToolUse", &payload, Some("t1"), Some("r1"));
    assert!(matches!(action, HookAction::Deny { .. }));
}

#[test]
fn hook_action_pretooluse_allows_non_bash_tool() {
    let p = ClaudeProvider::new();
    let payload = serde_json::json!({
        "tool_name": "Read",
        "tool_input": { "file_path": "/etc/passwd" }
    });
    let action = p.hook_action("PreToolUse", &payload, Some("t1"), Some("r1"));
    assert_eq!(action, HookAction::Allow);
}

#[test]
fn hook_action_user_prompt_injects_context() {
    let p = ClaudeProvider::new();
    let payload = serde_json::json!({});
    let action = p.hook_action("UserPromptSubmit", &payload, Some("ticket-42"), Some("run-1"));
    match action {
        HookAction::InjectContext { context } => {
            assert!(context.contains("ticket-42"));
            assert!(context.contains("run-1"));
        }
        _ => panic!("Expected InjectContext, got {:?}", action),
    }
}

#[test]
fn hook_action_user_prompt_no_ticket_returns_no_action() {
    let p = ClaudeProvider::new();
    let action = p.hook_action("UserPromptSubmit", &serde_json::json!({}), None, None);
    assert_eq!(action, HookAction::NoAction);
}

#[test]
fn hook_action_posttooluse_allows() {
    let p = ClaudeProvider::new();
    let action = p.hook_action("PostToolUse", &serde_json::json!({}), Some("t"), Some("r"));
    assert_eq!(action, HookAction::Allow);
}

// ── Stop event normalization tests ─────────────────────────────────

#[test]
fn normalize_stop_event_error() {
    let p = ClaudeProvider::new();
    let payload = serde_json::json!({ "stop_reason": "error" });
    let result = p.normalize_stop_event(&payload);
    assert_eq!(result.status, "error");
    assert_eq!(result.exit_code, 1);
}

#[test]
fn normalize_stop_event_tool_error() {
    let p = ClaudeProvider::new();
    let payload = serde_json::json!({ "stop_reason": "tool_error" });
    let result = p.normalize_stop_event(&payload);
    assert_eq!(result.status, "error");
}

#[test]
fn normalize_stop_event_user_cancelled() {
    let p = ClaudeProvider::new();
    let payload = serde_json::json!({ "stop_reason": "user_cancelled" });
    let result = p.normalize_stop_event(&payload);
    assert_eq!(result.status, "aborted");
    assert_eq!(result.exit_code, 130);
}

#[test]
fn normalize_stop_event_normal_end() {
    let p = ClaudeProvider::new();
    let payload = serde_json::json!({ "stop_reason": "end_turn" });
    let result = p.normalize_stop_event(&payload);
    assert_eq!(result.status, "finished");
    assert_eq!(result.exit_code, 0);
}

// ── Additional normalization coverage ───────────────────────────────

#[test]
fn normalize_hook_event_post_tool_use_failure_maps_to_error() {
    let p = ClaudeProvider::new();
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": { "command": "false" }
    });
    let result = p.normalize_hook_event("PostToolUseFailure", &payload);
    assert_eq!(result.event_type, "error");
    assert_eq!(result.structured["tool"], "bash");
}

#[test]
fn normalize_hook_event_session_end_maps_to_run_stopped() {
    let p = ClaudeProvider::new();
    let payload = serde_json::json!({
        "stop_reason": "end_turn",
        "transcript_path": "/tmp/t.json"
    });
    let result = p.normalize_hook_event("SessionEnd", &payload);
    assert_eq!(result.event_type, "run_stopped");
    assert_eq!(result.structured["reason"], "end_turn");
}

#[test]
fn normalize_hook_event_pretooluse_write() {
    let p = ClaudeProvider::new();
    let payload = serde_json::json!({
        "tool_name": "Write",
        "tool_input": { "file_path": "/tmp/out.txt" }
    });
    let result = p.normalize_hook_event("PreToolUse", &payload);
    assert_eq!(result.structured["tool"], "write");
    assert_eq!(result.structured["filePath"], "/tmp/out.txt");
}

#[test]
fn normalize_hook_event_pretooluse_missing_tool_input() {
    let p = ClaudeProvider::new();
    let payload = serde_json::json!({ "tool_name": "Bash" });
    let result = p.normalize_hook_event("PreToolUse", &payload);
    assert_eq!(result.structured["tool"], "bash");
    assert_eq!(result.structured["command"], "");
}

#[test]
fn normalize_stop_event_empty_reason_is_finished() {
    let p = ClaudeProvider::new();
    let result = p.normalize_stop_event(&serde_json::json!({}));
    assert_eq!(result.status, "finished");
    assert_eq!(result.exit_code, 0);
}

// is_dangerous_command tests live in agents::cli_utils::tests
