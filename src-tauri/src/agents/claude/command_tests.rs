//! Tests for Claude CLI command building (provider path).

use super::command::*;
use crate::agents::provider::AgentRunConfig;
use std::path::PathBuf;

fn create_provider_config() -> AgentRunConfig {
    AgentRunConfig {
        agent_id: "claude".to_string(),
        ticket_id: "t".to_string(),
        run_id: "r".to_string(),
        repo_path: PathBuf::from("/tmp/test"),
        prompt: "Test prompt".to_string(),
        timeout_secs: None,
        model: None,
        agent_config: std::collections::HashMap::new(),
    }
}

#[test]
fn provider_build_returns_claude_command() {
    let config = create_provider_config();
    let (cmd, _) = build_command_from_provider_config(&config);
    assert_eq!(cmd, "claude");
}

#[test]
fn provider_build_includes_stream_json_and_verbose() {
    let config = create_provider_config();
    let (_, args) = build_command_from_provider_config(&config);
    assert!(args.contains(&"--output-format".to_string()));
    assert!(args.contains(&"stream-json".to_string()));
    assert!(args.contains(&"--verbose".to_string()));
    assert!(args.contains(&"--dangerously-skip-permissions".to_string()));
}

#[test]
fn provider_build_omits_model_when_none() {
    let config = create_provider_config();
    let (_, args) = build_command_from_provider_config(&config);
    assert!(!args.contains(&"--model".to_string()));
}

#[test]
fn provider_build_passes_model_through() {
    let mut config = create_provider_config();
    config.model = Some("claude-sonnet-4-5".to_string());
    let (_, args) = build_command_from_provider_config(&config);
    assert!(args.contains(&"claude-sonnet-4-5".to_string()));
}

#[test]
fn provider_build_includes_thinking_by_default() {
    let config = create_provider_config();
    let (_, args) = build_command_from_provider_config(&config);
    assert!(args.contains(&"--settings".to_string()));
}

#[test]
fn provider_build_disables_thinking_via_agent_config() {
    let mut config = create_provider_config();
    config
        .agent_config
        .insert("thinking_enabled".to_string(), serde_json::json!(false));
    let (_, args) = build_command_from_provider_config(&config);
    assert!(!args.contains(&"--settings".to_string()));
}

#[test]
fn provider_build_enables_betas_via_agent_config() {
    let mut config = create_provider_config();
    config
        .agent_config
        .insert("extended_context_enabled".to_string(), serde_json::json!(true));
    let (_, args) = build_command_from_provider_config(&config);
    assert!(args.contains(&"--betas".to_string()));
}

#[test]
fn provider_build_enables_chrome_via_agent_config() {
    let mut config = create_provider_config();
    config
        .agent_config
        .insert("chrome_enabled".to_string(), serde_json::json!(true));
    let (_, args) = build_command_from_provider_config(&config);
    assert!(args.contains(&"--chrome".to_string()));
}

#[test]
fn provider_build_prompt_is_last() {
    let mut config = create_provider_config();
    config
        .agent_config
        .insert("thinking_enabled".to_string(), serde_json::json!(true));
    config
        .agent_config
        .insert("chrome_enabled".to_string(), serde_json::json!(true));
    let (_, args) = build_command_from_provider_config(&config);
    assert_eq!(args.last(), Some(&"Test prompt".to_string()));
}

#[test]
fn provider_build_excludes_betas_by_default() {
    let config = create_provider_config();
    let (_, args) = build_command_from_provider_config(&config);
    assert!(
        !args.contains(&"--betas".to_string()),
        "Extended context should be off by default"
    );
}

#[test]
fn provider_build_excludes_chrome_by_default() {
    let config = create_provider_config();
    let (_, args) = build_command_from_provider_config(&config);
    assert!(
        !args.contains(&"--chrome".to_string()),
        "Chrome should be off by default"
    );
}

#[test]
fn provider_build_all_cli_options_enabled() {
    let mut config = create_provider_config();
    config
        .agent_config
        .insert("thinking_enabled".to_string(), serde_json::json!(true));
    config
        .agent_config
        .insert("extended_context_enabled".to_string(), serde_json::json!(true));
    config
        .agent_config
        .insert("chrome_enabled".to_string(), serde_json::json!(true));
    let (_, args) = build_command_from_provider_config(&config);
    assert!(args.contains(&"--settings".to_string()));
    assert!(args.contains(&"--betas".to_string()));
    assert!(args.contains(&"--chrome".to_string()));
}

#[test]
fn provider_build_all_cli_options_disabled() {
    let mut config = create_provider_config();
    config
        .agent_config
        .insert("thinking_enabled".to_string(), serde_json::json!(false));
    config
        .agent_config
        .insert("extended_context_enabled".to_string(), serde_json::json!(false));
    config
        .agent_config
        .insert("chrome_enabled".to_string(), serde_json::json!(false));
    let (_, args) = build_command_from_provider_config(&config);
    assert!(!args.contains(&"--settings".to_string()));
    assert!(!args.contains(&"--betas".to_string()));
    assert!(!args.contains(&"--chrome".to_string()));
}

#[test]
fn provider_build_uses_model_as_is() {
    let test_cases = [
        "claude-opus-4-6",
        "claude-sonnet-4-5",
        "unknown-model",
    ];

    for model in test_cases {
        let mut config = create_provider_config();
        config.model = Some(model.to_string());
        let (_, args) = build_command_from_provider_config(&config);
        assert!(
            args.contains(&model.to_string()),
            "Builder should pass model '{}' through unchanged",
            model
        );
    }
}

#[test]
fn provider_build_prompt_immediately_follows_p_flag() {
    let config = create_provider_config();
    let (_, args) = build_command_from_provider_config(&config);

    let p_index = args
        .iter()
        .position(|a| a == "-p")
        .expect("-p flag must be present");
    let prompt_index = args
        .iter()
        .position(|a| a == "Test prompt")
        .expect("prompt must be present");

    assert_eq!(
        prompt_index,
        p_index + 1,
        "-p must be immediately followed by the prompt"
    );
}

#[test]
fn provider_build_prompt_is_last_with_cli_options() {
    let mut config = create_provider_config();
    config
        .agent_config
        .insert("thinking_enabled".to_string(), serde_json::json!(true));
    config
        .agent_config
        .insert("extended_context_enabled".to_string(), serde_json::json!(true));
    config
        .agent_config
        .insert("chrome_enabled".to_string(), serde_json::json!(true));
    let (_, args) = build_command_from_provider_config(&config);
    assert_eq!(
        args.last(),
        Some(&"Test prompt".to_string()),
        "Prompt must be the last argument even with all CLI options enabled"
    );
}