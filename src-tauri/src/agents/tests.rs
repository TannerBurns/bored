//! Tests for the agents module (types, serialization, config conversion).

use super::*;

#[test]
fn claude_api_config_default() {
    let config = ClaudeApiConfig::default();
    assert!(config.auth_token.is_none());
    assert!(config.api_key.is_none());
    assert!(config.base_url.is_none());
    assert!(config.model_override.is_none());
    assert!(config.thinking_enabled.is_none());
    assert!(config.extended_context_enabled.is_none());
    assert!(config.chrome_enabled.is_none());
}

#[test]
fn claude_api_config_from_settings_maps_cli_options() {
    let settings = crate::commands::claude::ClaudeApiSettings {
        thinking_enabled: Some(false),
        extended_context_enabled: Some(true),
        chrome_enabled: Some(true),
        ..Default::default()
    };
    let config = ClaudeApiConfig::from(settings);
    assert_eq!(config.thinking_enabled, Some(false));
    assert_eq!(config.extended_context_enabled, Some(true));
    assert_eq!(config.chrome_enabled, Some(true));
}

#[test]
fn claude_api_config_from_settings_maps_none_cli_options() {
    let settings = crate::commands::claude::ClaudeApiSettings::default();
    let config = ClaudeApiConfig::from(settings);
    assert!(config.thinking_enabled.is_none());
    assert!(config.extended_context_enabled.is_none());
    assert!(config.chrome_enabled.is_none());
}

#[test]
fn claude_api_config_with_values() {
    let config = ClaudeApiConfig {
        auth_token: Some("auth123".to_string()),
        api_key: Some("key456".to_string()),
        base_url: Some("https://custom.api.com".to_string()),
        model_override: Some("claude-opus-4-6".to_string()),
        ..Default::default()
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
    let stream_output = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello "}}}
{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"world!"}}}
"#;
    let result = extract_text_from_stream_json(stream_output);
    assert_eq!(result, Some("Hello world!".to_string()));
}

#[test]
fn extract_text_from_stream_event_with_plan() {
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
    let stream_output = r#"{"type":"result","result":"Final plan text"}"#;
    let result = extract_text_from_stream_json(stream_output);
    assert_eq!(result, Some("Final plan text".to_string()));
}

#[test]
fn extract_text_from_assistant_message() {
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
    let stream_output = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello world"}}}"#;
    let result = extract_agent_text(stream_output);
    assert_eq!(result, "Hello world");
}

#[test]
fn extract_agent_text_from_plain_text() {
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
    let stream_output = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Part 1"}}}
{"type":"result","result":" Part 2"}
"#;
    let result = extract_text_from_stream_json(stream_output);
    assert_eq!(result, Some("Part 1 Part 2".to_string()));
}

#[test]
fn extract_text_uses_only_last_assistant_message() {
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
    let stream_output =
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Direct response"}]}}"#;
    let result = extract_text_from_stream_json(stream_output);
    assert_eq!(result, Some("Direct response".to_string()));
}

#[test]
fn extract_text_stream_events_preferred_over_assistant() {
    let stream_output = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Old message"}]}}
{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Streamed response"}}}
"#;
    let result = extract_text_from_stream_json(stream_output);
    assert_eq!(result, Some("Streamed response".to_string()));
}

// ── to_provider_config tests ────────────────────────────────────

#[test]
fn to_provider_config_maps_basic_fields() {
    let config = AgentRunConfig {
        kind: AgentKind::Claude,
        ticket_id: "ticket-1".to_string(),
        run_id: "run-1".to_string(),
        repo_path: std::path::PathBuf::from("/repo"),
        prompt: "do stuff".to_string(),
        timeout_secs: Some(300),
        api_url: "http://localhost:7432".to_string(),
        api_token: "tok".to_string(),
        model: Some("sonnet-4.5".to_string()),
        claude_api_config: None,
        agent_config: std::collections::HashMap::new(),
    };
    let p = config.to_provider_config();
    assert_eq!(p.agent_id, "claude");
    assert_eq!(p.ticket_id, "ticket-1");
    assert_eq!(p.run_id, "run-1");
    assert_eq!(p.prompt, "do stuff");
    assert_eq!(p.timeout_secs, Some(300));
    assert_eq!(p.model.as_deref(), Some("sonnet-4.5"));
}

#[test]
fn to_provider_config_populates_agent_config_from_legacy_claude() {
    let config = AgentRunConfig {
        kind: AgentKind::Claude,
        ticket_id: "t".to_string(),
        run_id: "r".to_string(),
        repo_path: std::path::PathBuf::from("/"),
        prompt: "p".to_string(),
        timeout_secs: None,
        api_url: "http://x".to_string(),
        api_token: "tok".to_string(),
        model: None,
        claude_api_config: Some(ClaudeApiConfig {
            auth_token: Some("my-token".to_string()),
            thinking_enabled: Some(false),
            chrome_enabled: Some(true),
            ..Default::default()
        }),
        agent_config: std::collections::HashMap::new(),
    };
    let p = config.to_provider_config();
    assert_eq!(
        p.agent_config.get("auth_token").and_then(|v| v.as_str()),
        Some("my-token")
    );
    assert_eq!(
        p.agent_config
            .get("thinking_enabled")
            .and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(
        p.agent_config
            .get("chrome_enabled")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert!(p.agent_config.get("api_key").is_none());
}

#[test]
fn to_provider_config_preserves_existing_agent_config() {
    let mut agent_config = std::collections::HashMap::new();
    agent_config.insert("custom_key".to_string(), serde_json::json!("custom_val"));

    let config = AgentRunConfig {
        kind: AgentKind::Cursor,
        ticket_id: "t".to_string(),
        run_id: "r".to_string(),
        repo_path: std::path::PathBuf::from("/"),
        prompt: "p".to_string(),
        timeout_secs: None,
        api_url: "http://x".to_string(),
        api_token: "tok".to_string(),
        model: None,
        claude_api_config: Some(ClaudeApiConfig {
            auth_token: Some("should-be-ignored".to_string()),
            ..Default::default()
        }),
        agent_config,
    };
    let p = config.to_provider_config();
    assert_eq!(
        p.agent_config.get("custom_key").and_then(|v| v.as_str()),
        Some("custom_val")
    );
    assert!(p.agent_config.get("auth_token").is_none());
}

#[test]
fn to_provider_config_empty_both_yields_empty_agent_config() {
    let config = AgentRunConfig {
        kind: AgentKind::Cursor,
        ticket_id: "t".to_string(),
        run_id: "r".to_string(),
        repo_path: std::path::PathBuf::from("/"),
        prompt: "p".to_string(),
        timeout_secs: None,
        api_url: "http://x".to_string(),
        api_token: "tok".to_string(),
        model: None,
        claude_api_config: None,
        agent_config: std::collections::HashMap::new(),
    };
    let p = config.to_provider_config();
    assert!(p.agent_config.is_empty());
    assert_eq!(p.agent_id, "cursor");
}
