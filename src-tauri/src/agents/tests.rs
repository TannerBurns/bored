//! Tests for the agents module (types, serialization, config conversion).

use super::*;

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

// ── AgentRunConfig tests ────────────────────────────────────

#[test]
fn agent_run_config_basic_fields() {
    let config = AgentRunConfig {
        agent_id: "claude".to_string(),
        ticket_id: "ticket-1".to_string(),
        run_id: "run-1".to_string(),
        repo_path: std::path::PathBuf::from("/repo"),
        prompt: "do stuff".to_string(),
        timeout_secs: Some(300),
        api_url: "http://localhost:7432".to_string(),
        api_token: "tok".to_string(),
        model: Some("sonnet-4.5".to_string()),
        agent_config: std::collections::HashMap::new(),
    };
    assert_eq!(config.agent_id, "claude");
    assert_eq!(config.ticket_id, "ticket-1");
    assert_eq!(config.run_id, "run-1");
    assert_eq!(config.prompt, "do stuff");
    assert_eq!(config.timeout_secs, Some(300));
    assert_eq!(config.model.as_deref(), Some("sonnet-4.5"));
}

#[test]
fn agent_run_config_with_agent_config() {
    let mut agent_config = std::collections::HashMap::new();
    agent_config.insert("auth_token".to_string(), serde_json::json!("my-token"));
    agent_config.insert("thinking_enabled".to_string(), serde_json::json!(false));

    let config = AgentRunConfig {
        agent_id: "claude".to_string(),
        ticket_id: "t".to_string(),
        run_id: "r".to_string(),
        repo_path: std::path::PathBuf::from("/"),
        prompt: "p".to_string(),
        timeout_secs: None,
        api_url: "http://x".to_string(),
        api_token: "tok".to_string(),
        model: None,
        agent_config,
    };
    assert_eq!(
        config.agent_config.get("auth_token").and_then(|v| v.as_str()),
        Some("my-token")
    );
    assert_eq!(
        config
            .agent_config
            .get("thinking_enabled")
            .and_then(|v| v.as_bool()),
        Some(false)
    );
}

#[test]
fn agent_run_config_empty_agent_config() {
    let config = AgentRunConfig {
        agent_id: "cursor".to_string(),
        ticket_id: "t".to_string(),
        run_id: "r".to_string(),
        repo_path: std::path::PathBuf::from("/"),
        prompt: "p".to_string(),
        timeout_secs: None,
        api_url: "http://x".to_string(),
        api_token: "tok".to_string(),
        model: None,
        agent_config: std::collections::HashMap::new(),
    };
    assert!(config.agent_config.is_empty());
    assert_eq!(config.agent_id, "cursor");
}
