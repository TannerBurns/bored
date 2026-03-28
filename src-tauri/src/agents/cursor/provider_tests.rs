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
        model: None,
        agent_config: HashMap::new(),
        session_id: None,
        workspace_file: None,
        workspace_paths: vec![],
        debug_mode: false,
        allow_protected_branch: false,
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

#[test]
fn build_command_passes_model_through() {
    let p = CursorProvider::new();
    let mut config = make_config();
    config.model = Some("opus-4.6".to_string());
    let (_, args) = p.build_command(&config);
    assert!(
        args.contains(&"opus-4.6".to_string()),
        "Model should be passed through unchanged"
    );
    assert!(
        !args.contains(&"opus-4.6-thinking".to_string()),
        "No -thinking suffix should be appended"
    );
}

#[test]
fn build_command_passes_thinking_model_through() {
    let p = CursorProvider::new();
    let mut config = make_config();
    config.model = Some("opus-4.6-thinking".to_string());
    let (_, args) = p.build_command(&config);
    assert!(
        args.contains(&"opus-4.6-thinking".to_string()),
        "Thinking model ID should be passed through as-is"
    );
}

#[test]
fn build_command_no_model_omits_model_flag() {
    let p = CursorProvider::new();
    let config = make_config();
    let (_, args) = p.build_command(&config);
    assert!(!args.contains(&"--model".to_string()));
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
fn available_models_returns_injected_models() {
    let p = CursorProvider::with_models(vec![
        ("opus-4.6".into(), "Claude 4.6 Opus".into()),
        ("sonnet-4.5".into(), "Claude 4.5 Sonnet".into()),
        ("gpt-5.4".into(), "GPT-5.4".into()),
    ]);
    let models = p.available_models();
    assert_eq!(models.len(), 3);
    let ids: Vec<&str> = models.iter().map(|(id, _)| *id).collect();
    assert!(ids.contains(&"opus-4.6"));
    assert!(ids.contains(&"sonnet-4.5"));
    assert!(ids.contains(&"gpt-5.4"));
    for (id, label) in &models {
        assert!(!id.is_empty());
        assert!(!label.is_empty());
    }
}

#[test]
fn available_models_empty_when_no_models_injected() {
    let p = CursorProvider::with_models(vec![]);
    assert!(p.available_models().is_empty());
}

// ── session continuation tests ────────────────────────────────

#[test]
fn extract_session_id_returns_session_from_stream_json() {
    let p = CursorProvider::new();
    let output = concat!(
        r#"{"type":"system","subtype":"init","session_id":"cursor-sess-1","model":"test"}"#,
        "\n",
        r#"{"type":"result","subtype":"success","result":"ok","session_id":"cursor-sess-1"}"#,
    );
    assert_eq!(p.extract_session_id(output), Some("cursor-sess-1".to_string()));
}

#[test]
fn extract_session_id_returns_none_without_session() {
    let p = CursorProvider::new();
    assert!(p.extract_session_id(r#"{"type":"result","result":"ok"}"#).is_none());
}

#[test]
fn build_command_includes_resume_flag_when_session_set() {
    let p = CursorProvider::new();
    let mut config = make_config();
    config.session_id = Some("chat-xyz".to_string());
    let (_, args) = p.build_command(&config);
    let idx = args.iter().position(|a| a == "--resume").expect("--resume should be present");
    assert_eq!(args[idx + 1], "chat-xyz");
}

#[test]
fn build_command_omits_resume_flag_when_no_session() {
    let p = CursorProvider::new();
    let config = make_config();
    let (_, args) = p.build_command(&config);
    assert!(!args.contains(&"--resume".to_string()));
}
