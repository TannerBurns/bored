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
fn claude_extract_text_from_stream_event_format() {
    use crate::agents::claude::provider::ClaudeProvider;
    let provider = ClaudeProvider::new();
    let stream_output = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello "}}}
{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"world!"}}}
"#;
    let result = provider.extract_text(stream_output);
    assert_eq!(result, "Hello world!");
}

#[test]
fn claude_extract_text_from_stream_event_with_plan() {
    use crate::agents::claude::provider::ClaudeProvider;
    let provider = ClaudeProvider::new();
    let stream_output = "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"## Plan\\n\\n\"}}}\n\
{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"1. First step\\n\"}}}\n\
{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"2. Second step\\n\"}}}\n";
    let result = provider.extract_text(stream_output);
    assert_eq!(result, "## Plan\n\n1. First step\n2. Second step\n");
}

#[test]
fn claude_extract_text_ignores_non_text_events() {
    use crate::agents::claude::provider::ClaudeProvider;
    let provider = ClaudeProvider::new();
    let stream_output = r#"{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}}
{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"The plan"}}}
{"type":"stream_event","event":{"type":"content_block_stop","index":0}}
{"type":"stream_event","event":{"type":"tool_use","name":"read_file"}}
"#;
    let result = provider.extract_text(stream_output);
    assert_eq!(result, "The plan");
}

#[test]
fn claude_extract_text_from_result_message() {
    use crate::agents::claude::provider::ClaudeProvider;
    let provider = ClaudeProvider::new();
    let stream_output = r#"{"type":"result","result":"Final plan text"}"#;
    let result = provider.extract_text(stream_output);
    assert_eq!(result, "Final plan text");
}

#[test]
fn claude_extract_text_from_assistant_message() {
    use crate::agents::claude::provider::ClaudeProvider;
    let provider = ClaudeProvider::new();
    let stream_output = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Assistant response"}]}}"#;
    let result = provider.extract_text(stream_output);
    assert_eq!(result, "Assistant response");
}

#[test]
fn claude_provider_extract_text_from_stream_json() {
    use crate::agents::claude::provider::ClaudeProvider;
    let provider = ClaudeProvider::new();
    let stream_output = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello world"}}}"#;
    let result = provider.extract_text(stream_output);
    assert_eq!(result, "Hello world");
}

#[test]
fn cursor_provider_extract_text_plain_fallback() {
    use crate::agents::cursor::provider::CursorProvider;
    let provider = CursorProvider::new();
    let plain_output = "This is plain text output from the agent.";
    let result = provider.extract_text(plain_output);
    assert_eq!(result, plain_output);
}

#[test]
fn cursor_provider_extract_text_empty() {
    use crate::agents::cursor::provider::CursorProvider;
    let provider = CursorProvider::new();
    let result = provider.extract_text("");
    assert_eq!(result, "");
}

#[test]
fn cursor_provider_extract_text_from_stream_json() {
    use crate::agents::cursor::provider::CursorProvider;
    let provider = CursorProvider::new();
    let stream_output = concat!(
        r#"{"type":"system","subtype":"init","apiKeySource":"login","cwd":"/Users/tanner","session_id":"abc123","model":"Claude 4.5 Opus","permissionMode":"default"}"#, "\n",
        r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Say exactly: hello world"}]},"session_id":"abc123"}"#, "\n",
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"hello world"}]},"session_id":"abc123"}"#, "\n",
        r#"{"type":"result","subtype":"success","duration_ms":2218,"duration_api_ms":2218,"is_error":false,"result":"hello world","session_id":"abc123"}"#, "\n",
    );
    let result = provider.extract_text(stream_output);
    assert_eq!(result, "hello world");
}

#[test]
fn cursor_provider_extract_text_result_only() {
    use crate::agents::cursor::provider::CursorProvider;
    let provider = CursorProvider::new();
    let stream_output = r#"{"type":"result","subtype":"success","result":"done"}"#;
    let result = provider.extract_text(stream_output);
    assert_eq!(result, "done");
}

#[test]
fn cursor_provider_extract_text_assistant_only() {
    use crate::agents::cursor::provider::CursorProvider;
    let provider = CursorProvider::new();
    let stream_output = concat!(
        r#"{"type":"system","subtype":"init","session_id":"s1","model":"test"}"#, "\n",
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"assistant-only response"}]},"session_id":"s1"}"#, "\n",
    );
    let result = provider.extract_text(stream_output);
    assert_eq!(result, "assistant-only response");
}

#[test]
fn cursor_provider_extract_text_skips_malformed_json() {
    use crate::agents::cursor::provider::CursorProvider;
    let provider = CursorProvider::new();
    let stream_output = concat!(
        "not json at all\n",
        "{broken json\n",
        r#"{"type":"result","subtype":"success","result":"survived"}"#, "\n",
    );
    let result = provider.extract_text(stream_output);
    assert_eq!(result, "survived");
}

#[test]
fn cursor_provider_extract_text_multi_turn_uses_last_assistant() {
    use crate::agents::cursor::provider::CursorProvider;
    let provider = CursorProvider::new();
    let stream_output = concat!(
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"first turn"}]}}"#, "\n",
        r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"follow up"}]}}"#, "\n",
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"second turn"}]}}"#, "\n",
    );
    let result = provider.extract_text(stream_output);
    assert_eq!(result, "second turn");
}

#[test]
fn claude_provider_extract_text_no_text_content() {
    use crate::agents::claude::provider::ClaudeProvider;
    let provider = ClaudeProvider::new();
    let stream_output =
        r#"{"type":"stream_event","event":{"type":"tool_use","name":"read_file"}}"#;
    let result = provider.extract_text(stream_output);
    assert_eq!(result, stream_output);
}

#[test]
fn claude_extract_text_handles_mixed_content() {
    use crate::agents::claude::provider::ClaudeProvider;
    let provider = ClaudeProvider::new();
    let stream_output = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Part 1"}}}
{"type":"result","result":" Part 2"}
"#;
    let result = provider.extract_text(stream_output);
    assert_eq!(result, "Part 1", "streaming deltas take precedence; result summary is not appended");
}

#[test]
fn claude_extract_text_uses_only_last_assistant_message() {
    use crate::agents::claude::provider::ClaudeProvider;
    let provider = ClaudeProvider::new();
    let stream_output = concat!(
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Let me explore the codebase..."},{"type":"tool_use","id":"toolu_1","name":"read_file"}]}}"#, "\n",
        r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_1","content":"file contents"}]}}"#, "\n",
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Now let me check another file..."},{"type":"tool_use","id":"toolu_2","name":"read_file"}]}}"#, "\n",
        r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_2","content":"more file contents"}]}}"#, "\n",
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Here are my findings.\n\n1. What approach do you prefer?"}]}}"#, "\n",
    );
    let result = provider.extract_text(stream_output);
    assert_eq!(
        result,
        "Here are my findings.\n\n1. What approach do you prefer?"
    );
}

#[test]
fn claude_extract_text_single_assistant_message_still_works() {
    use crate::agents::claude::provider::ClaudeProvider;
    let provider = ClaudeProvider::new();
    let stream_output =
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Direct response"}]}}"#;
    let result = provider.extract_text(stream_output);
    assert_eq!(result, "Direct response");
}

#[test]
fn claude_extract_text_stream_events_preferred_over_assistant() {
    use crate::agents::claude::provider::ClaudeProvider;
    let provider = ClaudeProvider::new();
    let stream_output = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Old message"}]}}
{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Streamed response"}}}
"#;
    let result = provider.extract_text(stream_output);
    assert_eq!(result, "Streamed response");
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
        model: Some("sonnet-4.5".to_string()),
        agent_config: std::collections::HashMap::new(),
        session_id: None,
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
        model: None,
        agent_config,
        session_id: None,
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
        model: None,
        agent_config: std::collections::HashMap::new(),
        session_id: None,
    };
    assert!(config.agent_config.is_empty());
    assert_eq!(config.agent_id, "cursor");
}

#[test]
fn agent_run_config_with_session_id() {
    let config = AgentRunConfig {
        agent_id: "claude".to_string(),
        ticket_id: "t".to_string(),
        run_id: "r".to_string(),
        repo_path: std::path::PathBuf::from("/"),
        prompt: "p".to_string(),
        timeout_secs: None,
        model: None,
        agent_config: std::collections::HashMap::new(),
        session_id: Some("sess-abc-123".to_string()),
    };
    assert_eq!(config.session_id, Some("sess-abc-123".to_string()));
}

// ── AgentProvider trait method tests ──────────────────────────

#[test]
fn claude_provider_brand_color() {
    use crate::agents::claude::provider::ClaudeProvider;
    let p = ClaudeProvider::new();
    assert_eq!(p.brand_color(), Some("#da7756"));
}

#[test]
fn cursor_provider_brand_color_is_none() {
    use crate::agents::cursor::provider::CursorProvider;
    let p = CursorProvider::new();
    assert_eq!(p.brand_color(), None);
}

// ── AgentProvider trait default method tests ──────────────────

#[derive(Debug)]
struct StubProvider;

impl AgentProvider for StubProvider {
    fn id(&self) -> &str { "stub" }
    fn display_name(&self) -> &str { "Stub" }
    fn build_command(&self, _: &AgentRunConfig) -> (String, Vec<String>) { ("stub".into(), vec![]) }
    fn build_env_vars(&self, _: &AgentRunConfig) -> Vec<(String, String)> { vec![] }
    fn extract_text(&self, output: &str) -> String { output.to_string() }
    fn extract_cost(&self, _: &str, _: &str, _: f64) -> Option<crate::agents::cost::RunCostData> { None }
    fn is_available(&self) -> bool { false }
    fn get_version(&self) -> Option<String> { None }
    fn config_dir_name(&self) -> &str { ".stub" }
    fn command_instructions_subdir(&self) -> &str { "commands" }
    fn format_command_reference(&self, cmd: &str) -> String { format!("/{}", cmd) }
}

#[test]
fn default_brand_color_is_none() {
    let p = StubProvider;
    assert!(p.brand_color().is_none());
}

// ── Provider trait surface ──────────────────────────────────────────

#[test]
fn cursor_provider_implements_agent_provider_without_hooks() {
    use crate::agents::cursor::provider::CursorProvider;
    let p: Box<dyn AgentProvider> = Box::new(CursorProvider::new());
    assert_eq!(p.id(), "cursor");
    assert_eq!(p.display_name(), "Cursor");
    assert_eq!(p.config_dir_name(), ".cursor");
}

#[test]
fn claude_provider_implements_agent_provider_without_hooks() {
    use crate::agents::claude::provider::ClaudeProvider;
    let p: Box<dyn AgentProvider> = Box::new(ClaudeProvider::new());
    assert_eq!(p.id(), "claude");
    assert_eq!(p.display_name(), "Claude Code");
    assert_eq!(p.config_dir_name(), ".claude");
}

#[test]
fn stub_provider_trait_object_works() {
    let p = StubProvider;
    let provider: &dyn AgentProvider = &p;
    assert_eq!(provider.id(), "stub");
    assert_eq!(provider.config_dir_name(), ".stub");
    assert!(provider.brand_color().is_none());
}

#[test]
fn provider_trait_still_has_command_reference_methods() {
    let p = StubProvider;
    let provider: &dyn AgentProvider = &p;
    assert_eq!(provider.command_instructions_subdir(), "commands");
    assert_eq!(provider.format_command_reference("deslop"), "/deslop");
}

#[test]
fn provider_trait_no_longer_has_install_or_check_methods() {
    let p = StubProvider;
    assert_eq!(p.config_dir_name(), ".stub");
    assert_eq!(p.command_instructions_subdir(), "commands");
    assert_eq!(p.format_command_reference("test"), "/test");
}
