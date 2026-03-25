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
        session_id: None,
        workspace_file: None,
        workspace_paths: vec![],
        debug_mode: false,
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
fn provider_build_extended_context_appends_1m_suffix_opus() {
    let mut config = create_provider_config();
    config.model = Some("claude-opus-4-6".to_string());
    config
        .agent_config
        .insert("extended_context_enabled".to_string(), serde_json::json!(true));
    let (_, args) = build_command_from_provider_config(&config);
    assert!(args.contains(&"claude-opus-4-6[1m]".to_string()));
    assert!(!args.contains(&"--betas".to_string()));
}

#[test]
fn provider_build_extended_context_appends_1m_suffix_sonnet() {
    let mut config = create_provider_config();
    config.model = Some("claude-sonnet-4-6".to_string());
    config
        .agent_config
        .insert("extended_context_enabled".to_string(), serde_json::json!(true));
    let (_, args) = build_command_from_provider_config(&config);
    assert!(args.contains(&"claude-sonnet-4-6[1m]".to_string()));
}

#[test]
fn provider_build_extended_context_skips_ineligible_model() {
    let mut config = create_provider_config();
    config.model = Some("claude-opus-4-5".to_string());
    config
        .agent_config
        .insert("extended_context_enabled".to_string(), serde_json::json!(true));
    let (_, args) = build_command_from_provider_config(&config);
    assert!(
        args.contains(&"claude-opus-4-5".to_string()),
        "4.5 model should not get [1m] suffix"
    );
    assert!(
        !args.iter().any(|a| a.contains("[1m]")),
        "No argument should contain [1m] for ineligible models"
    );
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
fn provider_build_no_1m_suffix_by_default() {
    let mut config = create_provider_config();
    config.model = Some("claude-opus-4-6".to_string());
    let (_, args) = build_command_from_provider_config(&config);
    assert!(
        args.contains(&"claude-opus-4-6".to_string()),
        "Model should not have [1m] suffix when extended context is off"
    );
    assert!(
        !args.iter().any(|a| a.contains("[1m]")),
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
    config.model = Some("claude-opus-4-6".to_string());
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
    assert!(args.contains(&"claude-opus-4-6[1m]".to_string()));
    assert!(args.contains(&"--chrome".to_string()));
}

#[test]
fn provider_build_all_cli_options_disabled() {
    let mut config = create_provider_config();
    config.model = Some("claude-opus-4-6".to_string());
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
    assert!(
        args.contains(&"claude-opus-4-6".to_string()),
        "Model should not have [1m] suffix when disabled"
    );
    assert!(!args.contains(&"--chrome".to_string()));
}

#[test]
fn provider_build_passes_cli_names_through() {
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
            "Builder should pass CLI model '{}' through unchanged",
            model
        );
    }
}

#[test]
fn provider_build_normalizes_short_model_names() {
    let test_cases = [
        ("opus-4.6", "claude-opus-4-6"),
        ("opus-4.5", "claude-opus-4-5"),
        ("sonnet-4.6", "claude-sonnet-4-6"),
        ("sonnet-4.5", "claude-sonnet-4-5"),
    ];

    for (short, expected) in test_cases {
        let mut config = create_provider_config();
        config.model = Some(short.to_string());
        let (_, args) = build_command_from_provider_config(&config);
        assert!(
            args.contains(&expected.to_string()),
            "Short name '{}' should be normalized to '{}'",
            short, expected
        );
    }
}

#[test]
fn normalize_model_for_cli_maps_short_names() {
    assert_eq!(normalize_model_for_cli("opus-4.6"), "claude-opus-4-6");
    assert_eq!(normalize_model_for_cli("opus-4.5"), "claude-opus-4-5");
    assert_eq!(normalize_model_for_cli("sonnet-4.6"), "claude-sonnet-4-6");
    assert_eq!(normalize_model_for_cli("sonnet-4.5"), "claude-sonnet-4-5");
}

#[test]
fn normalize_model_for_cli_passes_through_full_names() {
    assert_eq!(normalize_model_for_cli("claude-opus-4-6"), "claude-opus-4-6");
    assert_eq!(normalize_model_for_cli("claude-sonnet-4-5"), "claude-sonnet-4-5");
    assert_eq!(normalize_model_for_cli("unknown-model"), "unknown-model");
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
    config.model = Some("claude-opus-4-6".to_string());
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

// ── session resume command tests ──────────────────────────────

#[test]
fn build_command_with_session_id_includes_resume() {
    let mut config = create_provider_config();
    config.session_id = Some("sess-1234".to_string());
    let (_, args) = build_command_from_provider_config(&config);
    let idx = args.iter().position(|a| a == "--resume").expect("--resume must be present");
    assert_eq!(args[idx + 1], "sess-1234");
}

#[test]
fn build_command_resume_appears_before_prompt() {
    let mut config = create_provider_config();
    config.session_id = Some("sess-xyz".to_string());
    let (_, args) = build_command_from_provider_config(&config);
    let resume_idx = args.iter().position(|a| a == "--resume").unwrap();
    let p_idx = args.iter().position(|a| a == "-p").unwrap();
    assert!(resume_idx < p_idx, "--resume must appear before -p");
    assert_eq!(args.last(), Some(&"Test prompt".to_string()));
}

#[test]
fn build_command_without_session_id_omits_resume() {
    let config = create_provider_config();
    let (_, args) = build_command_from_provider_config(&config);
    assert!(!args.iter().any(|a| a == "--resume"), "--resume should not appear without session_id");
}

#[test]
fn provider_build_does_not_include_effort_arg() {
    let config = create_provider_config();
    let (_, args) = build_command_from_provider_config(&config);
    assert!(
        !args.contains(&"--effort".to_string()),
        "effort is set via CLAUDE_CODE_EFFORT_LEVEL env var, not CLI arg"
    );
}

#[test]
fn provider_build_effort_not_in_args_even_when_configured() {
    let mut config = create_provider_config();
    config.agent_config.insert("effort".to_string(), serde_json::json!("max"));
    let (_, args) = build_command_from_provider_config(&config);
    assert!(
        !args.contains(&"--effort".to_string()),
        "effort is set via CLAUDE_CODE_EFFORT_LEVEL env var, not CLI arg"
    );
}

#[test]
fn provider_build_extended_context_with_short_model_name() {
    let mut config = create_provider_config();
    config.model = Some("opus-4.6".to_string());
    config
        .agent_config
        .insert("extended_context_enabled".to_string(), serde_json::json!(true));
    let (_, args) = build_command_from_provider_config(&config);
    assert!(
        args.contains(&"claude-opus-4-6[1m]".to_string()),
        "Short name should be normalized then get [1m] suffix"
    );
}

#[test]
fn provider_build_extended_context_with_model_override_ineligible() {
    let mut config = create_provider_config();
    config.model = Some("claude-opus-4-6".to_string());
    config.agent_config.insert("model_override".to_string(), serde_json::json!("my-local-llama"));
    config.agent_config.insert("extended_context_enabled".to_string(), serde_json::json!(true));
    let (_, args) = build_command_from_provider_config(&config);
    assert!(
        args.contains(&"my-local-llama".to_string()),
        "Override model should be used"
    );
    assert!(
        !args.iter().any(|a| a.contains("[1m]")),
        "Non-eligible override model should not get [1m] suffix"
    );
}

#[test]
fn provider_build_extended_context_no_model_omits_suffix() {
    let mut config = create_provider_config();
    config
        .agent_config
        .insert("extended_context_enabled".to_string(), serde_json::json!(true));
    let (_, args) = build_command_from_provider_config(&config);
    assert!(
        !args.contains(&"--model".to_string()),
        "No model arg when model is None"
    );
    assert!(
        !args.iter().any(|a| a.contains("[1m]")),
        "No [1m] suffix when no model is set"
    );
}

#[test]
fn provider_build_extended_context_with_short_sonnet_name() {
    let mut config = create_provider_config();
    config.model = Some("sonnet-4.6".to_string());
    config
        .agent_config
        .insert("extended_context_enabled".to_string(), serde_json::json!(true));
    let (_, args) = build_command_from_provider_config(&config);
    assert!(
        args.contains(&"claude-sonnet-4-6[1m]".to_string()),
        "Short sonnet name should be normalized then get [1m] suffix"
    );
}

#[test]
fn provider_build_extended_context_frontend_key() {
    let mut config = create_provider_config();
    config.model = Some("claude-opus-4-6".to_string());
    config
        .agent_config
        .insert("extendedContext".to_string(), serde_json::json!(true));
    let (_, args) = build_command_from_provider_config(&config);
    assert!(
        args.contains(&"claude-opus-4-6[1m]".to_string()),
        "Frontend key 'extendedContext' should be accepted as alias for extended_context_enabled"
    );
}

#[test]
fn provider_build_extended_context_short_name_ineligible() {
    let mut config = create_provider_config();
    config.model = Some("sonnet-4.5".to_string());
    config
        .agent_config
        .insert("extended_context_enabled".to_string(), serde_json::json!(true));
    let (_, args) = build_command_from_provider_config(&config);
    assert!(
        args.contains(&"claude-sonnet-4-5".to_string()),
        "Ineligible short name should be normalized but not get [1m] suffix"
    );
    assert!(
        !args.iter().any(|a| a.contains("[1m]")),
        "No [1m] suffix for ineligible models"
    );
}

#[test]
fn build_command_workspace_paths_adds_extra_dirs() {
    let mut config = create_provider_config();
    config.workspace_paths = vec![
        PathBuf::from("/tmp/test"),
        PathBuf::from("/tmp/backend"),
        PathBuf::from("/tmp/shared"),
    ];
    let (_, args) = build_command_from_provider_config(&config);
    let add_dir_indices: Vec<usize> = args.iter().enumerate()
        .filter(|(_, a)| a.as_str() == "--add-dir")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(add_dir_indices.len(), 2, "should add 2 dirs (not repo_path)");
    assert_eq!(args[add_dir_indices[0] + 1], "/tmp/backend");
    assert_eq!(args[add_dir_indices[1] + 1], "/tmp/shared");
    let p_idx = args.iter().position(|a| a == "-p").unwrap();
    assert!(add_dir_indices.iter().all(|i| *i < p_idx), "--add-dir must appear before -p");
}

#[test]
fn build_command_no_workspace_paths_no_add_dir() {
    let config = create_provider_config();
    let (_, args) = build_command_from_provider_config(&config);
    assert!(!args.contains(&"--add-dir".to_string()));
}
