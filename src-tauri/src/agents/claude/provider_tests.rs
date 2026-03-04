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
        session_id: None,
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

// ── model override ────────────────────────────────────────────

#[test]
fn build_command_passes_model_through_without_mapping() {
    let p = ClaudeProvider::new();
    let mut config = make_config();
    config.model = Some("claude-opus-4-6".to_string());
    let (_, args) = p.build_command(&config);
    assert!(
        args.contains(&"claude-opus-4-6".to_string()),
        "Provider build_command should pass model name through without mapping"
    );
}

#[test]
fn build_command_model_override_takes_precedence() {
    let p = ClaudeProvider::new();
    let mut config = make_config();
    config.model = Some("claude-opus-4-6".to_string());
    config.agent_config.insert("model_override".into(), serde_json::json!("my-local-llama"));
    let (_, args) = p.build_command(&config);
    assert!(args.contains(&"my-local-llama".to_string()));
    assert!(!args.contains(&"claude-opus-4-6".to_string()));
}

#[test]
fn build_command_empty_model_override_falls_back_to_stage_model() {
    let p = ClaudeProvider::new();
    let mut config = make_config();
    config.model = Some("claude-sonnet-4-6".to_string());
    config.agent_config.insert("modelOverride".into(), serde_json::json!(""));
    let (_, args) = p.build_command(&config);
    assert!(args.contains(&"claude-sonnet-4-6".to_string()));
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
fn available_models_returns_claude_models() {
    let p = ClaudeProvider::new();
    let models = p.available_models();
    assert!(!models.is_empty());
    let ids: Vec<&str> = models.iter().map(|(id, _)| *id).collect();
    assert!(ids.contains(&"claude-opus-4-6"));
    assert!(ids.contains(&"claude-opus-4-5"));
    assert!(ids.contains(&"claude-sonnet-4-6"));
    assert!(ids.contains(&"claude-sonnet-4-5"));
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

// ── extract_text_from_stream_json edge cases ───────────────────

#[test]
fn extract_text_does_not_append_result_summary_to_deltas() {
    let input = concat!(
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"[{\"command\":\"cleanup\",\"model\":\"sonnet-4.6\"}]"}}}"#,
        "\n",
        r#"{"type":"result","result":"I selected cleanup for the QA pipeline.","subtype":"success"}"#,
    );
    let text = extract_text_from_stream_json(input).unwrap();
    assert_eq!(
        text,
        r#"[{"command":"cleanup","model":"sonnet-4.6"}]"#,
        "result summary must not be appended to streaming deltas"
    );
}

#[test]
fn extract_text_uses_result_as_fallback_when_no_deltas() {
    let input = r#"{"type":"result","result":"fallback text","subtype":"success"}"#;
    let text = extract_text_from_stream_json(input).unwrap();
    assert_eq!(text, "fallback text");
}

#[test]
fn extract_text_prefers_deltas_over_result() {
    let input = concat!(
        r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"from delta"}}"#,
        "\n",
        r#"{"type":"result","result":"from result","subtype":"success"}"#,
    );
    let text = extract_text_from_stream_json(input).unwrap();
    assert_eq!(text, "from delta");
}

#[test]
fn extract_text_result_fallback_over_assistant() {
    let input = concat!(
        r#"{"type":"result","result":"result text","subtype":"success"}"#,
        "\n",
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"assistant text"}]}}"#,
    );
    let text = extract_text_from_stream_json(input).unwrap();
    assert_eq!(text, "result text");
}

#[test]
fn extract_text_multiple_result_events_uses_last() {
    let input = concat!(
        r#"{"type":"result","result":"first result","subtype":"success"}"#,
        "\n",
        r#"{"type":"result","result":"second result","subtype":"success"}"#,
    );
    let text = extract_text_from_stream_json(input).unwrap();
    assert_eq!(text, "second result", "last result event should win");
}

#[test]
fn extract_text_no_events_returns_none() {
    assert!(extract_text_from_stream_json("").is_none());
    assert!(extract_text_from_stream_json("not json\njust text").is_none());
}

#[test]
fn extract_text_result_non_string_field_ignored() {
    let input = r#"{"type":"result","result":{"nested":"object"},"subtype":"success"}"#;
    assert!(
        extract_text_from_stream_json(input).is_none(),
        "non-string result field should not produce text"
    );
}

#[test]
fn extract_text_assistant_fallback_when_no_result_or_deltas() {
    let input = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"assistant only"}]}}"#;
    let text = extract_text_from_stream_json(input).unwrap();
    assert_eq!(text, "assistant only");
}

// ── Real CLI output end-to-end tests ───────────────────────────

#[derive(Debug, serde::Deserialize)]
struct CmdSel {
    command: String,
    model: String,
}

#[test]
fn extract_text_real_cli_non_streaming_response() {
    let input = concat!(
        r#"{"type":"system","subtype":"init","cwd":"/tmp/test","session_id":"abc","tools":[],"model":"claude-sonnet-4-6"}"#,
        "\n",
        r#"{"type":"assistant","message":{"model":"claude-sonnet-4-6","id":"msg_01X","type":"message","role":"assistant","content":[{"type":"text","text":"[{\"command\": \"cleanup\", \"model\": \"sonnet-4.6\"}]"}],"stop_reason":null},"session_id":"abc"}"#,
        "\n",
        r#"{"type":"result","subtype":"success","is_error":false,"duration_ms":1096,"result":"[{\"command\": \"cleanup\", \"model\": \"sonnet-4.6\"}]","session_id":"abc","total_cost_usd":0.01}"#,
    );
    let text = extract_text_from_stream_json(input).unwrap();
    assert_eq!(text, r#"[{"command": "cleanup", "model": "sonnet-4.6"}]"#);

    let parsed: Vec<CmdSel> =
        crate::agents::json_extraction::parse_json_response(&text).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].command, "cleanup");
    assert_eq!(parsed[0].model, "sonnet-4.6");
}

#[test]
fn extract_text_real_cli_streaming_response() {
    let input = concat!(
        r#"{"type":"system","subtype":"init","cwd":"/tmp/test","session_id":"abc","tools":[],"model":"claude-sonnet-4-6"}"#,
        "\n",
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"["}}}"#,
        "\n",
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"{\"command\": \"code-review\", \"model\": \"opus-4.6\"}"}}}"#,
        "\n",
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":", {\"command\": \"cleanup\", \"model\": \"sonnet-4.6\"}"}}}"#,
        "\n",
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"]"}}}"#,
        "\n",
        r#"{"type":"assistant","message":{"model":"claude-sonnet-4-6","id":"msg_02Y","type":"message","role":"assistant","content":[{"type":"text","text":"[{\"command\": \"code-review\", \"model\": \"opus-4.6\"}, {\"command\": \"cleanup\", \"model\": \"sonnet-4.6\"}]"}],"stop_reason":"end_turn"},"session_id":"abc"}"#,
        "\n",
        r#"{"type":"result","subtype":"success","is_error":false,"result":"I selected code-review and cleanup.","session_id":"abc","total_cost_usd":0.02}"#,
    );
    let text = extract_text_from_stream_json(input).unwrap();
    assert_eq!(
        text,
        r#"[{"command": "code-review", "model": "opus-4.6"}, {"command": "cleanup", "model": "sonnet-4.6"}]"#,
        "streaming deltas should be preferred over result summary"
    );

    let parsed: Vec<CmdSel> =
        crate::agents::json_extraction::parse_json_response(&text).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].command, "code-review");
    assert_eq!(parsed[1].command, "cleanup");
}

#[test]
fn extract_text_real_cli_response_with_prose_wrapping_json() {
    let input = concat!(
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Based on the ticket (a simple bug fix), here are the commands:\n\n"}}}"#,
        "\n",
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"[{\"command\": \"unit-tests\", \"model\": \"sonnet-4.6\"}, {\"command\": \"cleanup\", \"model\": \"sonnet-4.6\"}]"}}}"#,
        "\n",
        r#"{"type":"result","subtype":"success","result":"Based on the ticket (a simple bug fix), here are the commands:\n\n[{\"command\": \"unit-tests\", \"model\": \"sonnet-4.6\"}, {\"command\": \"cleanup\", \"model\": \"sonnet-4.6\"}]","session_id":"abc"}"#,
    );
    let text = extract_text_from_stream_json(input).unwrap();
    assert!(text.contains(r#"[{"command": "unit-tests"#), "extracted text should contain the JSON array");

    let parsed: Vec<CmdSel> =
        crate::agents::json_extraction::parse_json_response(&text).unwrap();
    assert_eq!(parsed.len(), 2, "should parse JSON even when wrapped in prose");
    assert_eq!(parsed[0].command, "unit-tests");
    assert_eq!(parsed[1].command, "cleanup");
}

// ── extract_session_id tests ──────────────────────────────────

#[test]
fn extract_session_id_from_system_init() {
    let output = concat!(
        r#"{"type":"system","subtype":"init","cwd":"/tmp","session_id":"sess-abc-123","model":"claude-sonnet-4-6"}"#,
        "\n",
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"done"}]},"session_id":"sess-abc-123"}"#,
        "\n",
        r#"{"type":"result","subtype":"success","result":"done","session_id":"sess-abc-123"}"#,
    );
    let sid = extract_session_id_from_stream_json(output);
    assert_eq!(sid, Some("sess-abc-123".to_string()));
}

#[test]
fn extract_session_id_fallback_to_result() {
    let output = r#"{"type":"result","subtype":"success","result":"ok","session_id":"fallback-id"}"#;
    let sid = extract_session_id_from_stream_json(output);
    assert_eq!(sid, Some("fallback-id".to_string()));
}

#[test]
fn extract_session_id_no_session_returns_none() {
    let output = r#"{"type":"assistant","message":{"role":"assistant","content":[]}}"#;
    let sid = extract_session_id_from_stream_json(output);
    assert!(sid.is_none());
}

#[test]
fn extract_session_id_empty_output() {
    assert!(extract_session_id_from_stream_json("").is_none());
}

#[test]
fn extract_session_id_skips_malformed_json_lines() {
    let output = concat!(
        "not json at all\n",
        "{broken json\n",
        r#"{"type":"system","subtype":"init","session_id":"good-id","model":"m"}"#,
    );
    assert_eq!(
        extract_session_id_from_stream_json(output),
        Some("good-id".to_string()),
    );
}

#[test]
fn extract_session_id_system_type_takes_priority_over_fallback() {
    let output = concat!(
        r#"{"type":"result","session_id":"result-id"}"#, "\n",
        r#"{"type":"system","subtype":"init","session_id":"system-id","model":"m"}"#, "\n",
        r#"{"type":"assistant","session_id":"assistant-id"}"#,
    );
    assert_eq!(
        extract_session_id_from_stream_json(output),
        Some("system-id".to_string()),
        "system init session_id should take priority",
    );
}

#[test]
fn extract_session_id_skips_whitespace_only_lines() {
    let output = concat!(
        "  \n",
        "\t\n",
        r#"{"type":"result","session_id":"ws-test"}"#,
    );
    assert_eq!(
        extract_session_id_from_stream_json(output),
        Some("ws-test".to_string()),
    );
}

#[test]
fn extract_session_id_via_provider_trait() {
    let provider = ClaudeProvider::new();
    let output = r#"{"type":"system","subtype":"init","session_id":"provider-test","model":"m"}"#;
    assert_eq!(provider.extract_session_id(output), Some("provider-test".to_string()));
}

// ── build_command with session_id tests ────────────────────────

#[test]
fn build_command_includes_resume_when_session_id_set() {
    let p = ClaudeProvider::new();
    let mut config = make_config();
    config.session_id = Some("resume-session-42".to_string());
    let (cmd, args) = p.build_command(&config);
    assert_eq!(cmd, "claude");
    let resume_idx = args.iter().position(|a| a == "--resume").expect("should have --resume");
    assert_eq!(args[resume_idx + 1], "resume-session-42");
    assert!(args.contains(&"-p".to_string()));
}

#[test]
fn build_command_omits_resume_when_no_session_id() {
    let p = ClaudeProvider::new();
    let config = make_config();
    let (_, args) = p.build_command(&config);
    assert!(!args.contains(&"--resume".to_string()));
}

// ── lightweight_agent_config tests ─────────────────────────────

#[test]
fn lightweight_config_disables_thinking_and_chrome() {
    let p = ClaudeProvider::new();
    let mut full = HashMap::new();
    full.insert("thinkingEnabled".into(), serde_json::json!(true));
    full.insert("chromeEnabled".into(), serde_json::json!(true));

    let cfg = p.lightweight_agent_config(&full);
    assert_eq!(cfg["thinkingEnabled"], serde_json::json!(false));
    assert_eq!(cfg["chromeEnabled"], serde_json::json!(false));
    assert_eq!(cfg["extendedContextEnabled"], serde_json::json!(false));
    assert_eq!(cfg["maxTurns"], serde_json::json!(1));
}

#[test]
fn lightweight_config_preserves_auth_keys() {
    let p = ClaudeProvider::new();
    let mut full = HashMap::new();
    full.insert("authToken".into(), serde_json::json!("secret"));
    full.insert("baseUrl".into(), serde_json::json!("http://localhost"));
    full.insert("unrelateSetting".into(), serde_json::json!("dropped"));

    let cfg = p.lightweight_agent_config(&full);
    assert_eq!(cfg["authToken"], serde_json::json!("secret"));
    assert_eq!(cfg["baseUrl"], serde_json::json!("http://localhost"));
    assert!(!cfg.contains_key("unrelateSetting"));
}

// is_dangerous_command tests live in agents::cli_utils::tests
