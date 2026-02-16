//! Tests for Claude CLI command building (both legacy and provider paths).

use super::command::*;
use crate::agents::ClaudeApiConfig;
use crate::agents::provider::AgentRunConfig as ProviderAgentRunConfig;
use std::path::PathBuf;

fn create_test_config() -> crate::agents::AgentRunConfig {
    crate::agents::AgentRunConfig {
        kind: crate::agents::AgentKind::Claude,
        ticket_id: "test-ticket".to_string(),
        run_id: "test-run".to_string(),
        repo_path: PathBuf::from("/tmp/test"),
        prompt: "Test prompt".to_string(),
        timeout_secs: Some(300),
        api_url: "http://localhost:7432".to_string(),
        api_token: "token".to_string(),
        model: None,
        claude_api_config: None,
        agent_config: std::collections::HashMap::new(),
    }
}

fn create_provider_config() -> ProviderAgentRunConfig {
    ProviderAgentRunConfig {
        agent_id: "claude".to_string(),
        ticket_id: "t".to_string(),
        run_id: "r".to_string(),
        repo_path: PathBuf::from("/tmp/test"),
        prompt: "Test prompt".to_string(),
        timeout_secs: None,
        api_url: "http://localhost:7432".to_string(),
        api_token: "tok".to_string(),
        model: None,
        agent_config: std::collections::HashMap::new(),
    }
}

// ── Legacy build_command tests ──────────────────────────────────────

#[test]
fn build_command_returns_claude() {
    let config = create_test_config();
    let (cmd, _) = build_command(&config);
    assert_eq!(cmd, "claude");
}

#[test]
fn build_command_includes_prompt() {
    let config = create_test_config();
    let (_, args) = build_command(&config);
    assert!(args.contains(&"-p".to_string()));
    assert!(args.contains(&"Test prompt".to_string()));
}

#[test]
fn build_command_prompt_immediately_follows_p_flag() {
    let config = create_test_config();
    let (_, args) = build_command(&config);

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
fn build_command_includes_model_when_specified() {
    let mut config = create_test_config();
    config.model = Some("sonnet-4.5".to_string());
    let (_, args) = build_command(&config);
    assert!(args.contains(&"--model".to_string()));
    assert!(args.contains(&"claude-sonnet-4-5".to_string()));
    assert_eq!(args.last(), Some(&"Test prompt".to_string()));
}

#[test]
fn build_command_maps_model_names_correctly() {
    let test_cases = [
        ("opus-4.6", "claude-opus-4-6"),
        ("opus-4.5", "claude-opus-4-5"),
        ("sonnet-4.5", "claude-sonnet-4-5"),
        ("unknown-model", "unknown-model"),
    ];

    for (input, expected) in test_cases {
        let mut config = create_test_config();
        config.model = Some(input.to_string());
        let (_, args) = build_command(&config);
        assert!(
            args.contains(&expected.to_string()),
            "Expected {} to be mapped to {}",
            input,
            expected
        );
    }
}

#[test]
fn build_command_defaults_to_opus_4_6_when_none() {
    let config = create_test_config();
    let (_, args) = build_command(&config);
    assert!(args.contains(&"--model".to_string()));
    assert!(
        args.contains(&"claude-opus-4-6".to_string()),
        "Should default to claude-opus-4-6 when no model specified"
    );
}

#[test]
fn build_command_ignores_model_override() {
    let mut config = create_test_config();
    config.model = Some("sonnet-4.5".to_string());
    config.claude_api_config = Some(ClaudeApiConfig {
        model_override: Some("custom-model-override".to_string()),
        ..Default::default()
    });
    let (_, args) = build_command(&config);
    assert!(
        args.contains(&"claude-sonnet-4-5".to_string()),
        "Per-stage model from workflow settings should always be used"
    );
    assert!(
        !args.contains(&"custom-model-override".to_string()),
        "model_override should never override workflow settings"
    );
}

#[test]
fn build_command_excludes_betas_by_default() {
    let config = create_test_config();
    let (_, args) = build_command(&config);
    assert!(
        !args.contains(&"--betas".to_string()),
        "Extended context should be off by default"
    );
}

#[test]
fn build_command_includes_betas_when_enabled() {
    let mut config = create_test_config();
    config.claude_api_config = Some(ClaudeApiConfig {
        extended_context_enabled: Some(true),
        ..Default::default()
    });
    let (_, args) = build_command(&config);
    let beta_index = args
        .iter()
        .position(|a| a == "--betas")
        .expect("--betas flag must be present when enabled");
    assert_eq!(
        args[beta_index + 1], "context-1m-2025-08-07",
        "--betas must be followed by context-1m-2025-08-07"
    );
}

#[test]
fn build_command_includes_thinking_by_default() {
    let config = create_test_config();
    let (_, args) = build_command(&config);
    assert!(
        args.contains(&"--settings".to_string()),
        "Thinking should be enabled by default"
    );
    assert!(
        args.contains(&r#"{"alwaysThinkingEnabled": true}"#.to_string()),
        "Thinking settings value should be present"
    );
}

#[test]
fn build_command_excludes_thinking_when_disabled() {
    let mut config = create_test_config();
    config.claude_api_config = Some(ClaudeApiConfig {
        thinking_enabled: Some(false),
        ..Default::default()
    });
    let (_, args) = build_command(&config);
    assert!(
        !args.contains(&"--settings".to_string()),
        "Thinking flag should not be present when disabled"
    );
}

#[test]
fn build_command_excludes_chrome_by_default() {
    let config = create_test_config();
    let (_, args) = build_command(&config);
    assert!(
        !args.contains(&"--chrome".to_string()),
        "Chrome should be off by default"
    );
}

#[test]
fn build_command_includes_chrome_when_enabled() {
    let mut config = create_test_config();
    config.claude_api_config = Some(ClaudeApiConfig {
        chrome_enabled: Some(true),
        ..Default::default()
    });
    let (_, args) = build_command(&config);
    assert!(
        args.contains(&"--chrome".to_string()),
        "Chrome flag should be present when enabled"
    );
}

#[test]
fn build_command_all_cli_options_enabled() {
    let mut config = create_test_config();
    config.claude_api_config = Some(ClaudeApiConfig {
        thinking_enabled: Some(true),
        extended_context_enabled: Some(true),
        chrome_enabled: Some(true),
        ..Default::default()
    });
    let (_, args) = build_command(&config);
    assert!(args.contains(&"--settings".to_string()));
    assert!(args.contains(&"--betas".to_string()));
    assert!(args.contains(&"--chrome".to_string()));
}

#[test]
fn build_command_all_cli_options_disabled() {
    let mut config = create_test_config();
    config.claude_api_config = Some(ClaudeApiConfig {
        thinking_enabled: Some(false),
        extended_context_enabled: Some(false),
        chrome_enabled: Some(false),
        ..Default::default()
    });
    let (_, args) = build_command(&config);
    assert!(!args.contains(&"--settings".to_string()));
    assert!(!args.contains(&"--betas".to_string()));
    assert!(!args.contains(&"--chrome".to_string()));
}

#[test]
fn default_settings() {
    let settings = ClaudeSettings::default();
    assert!(settings.executable_path.is_none());
    assert!(settings.system_prompt.is_none());
    assert!(settings.permission_mode.is_none());
}

#[test]
fn build_with_system_prompt() {
    let config = create_test_config();
    let settings = ClaudeSettings {
        system_prompt: Some("Be helpful".to_string()),
        ..Default::default()
    };
    let (_, args) = build_command_with_settings(&config, &settings);
    assert!(args.contains(&"--append-system-prompt".to_string()));
    assert!(args.contains(&"Be helpful".to_string()));
}

#[test]
fn build_with_permission_mode() {
    let config = create_test_config();
    let settings = ClaudeSettings {
        permission_mode: Some("ask".to_string()),
        ..Default::default()
    };
    let (_, args) = build_command_with_settings(&config, &settings);
    assert!(args.contains(&"--permission-mode".to_string()));
    assert!(args.contains(&"ask".to_string()));
}

#[test]
fn build_with_custom_executable() {
    let config = create_test_config();
    let settings = ClaudeSettings {
        executable_path: Some("/usr/local/bin/claude".to_string()),
        ..Default::default()
    };
    let (cmd, _) = build_command_with_settings(&config, &settings);
    assert_eq!(cmd, "/usr/local/bin/claude");
}

#[test]
fn build_with_settings_includes_thinking_by_default() {
    let config = create_test_config();
    let settings = ClaudeSettings::default();
    let (_, args) = build_command_with_settings(&config, &settings);
    assert!(
        args.contains(&"--settings".to_string()),
        "Thinking should be on by default in build_command_with_settings"
    );
}

#[test]
fn build_with_settings_excludes_betas_by_default() {
    let config = create_test_config();
    let settings = ClaudeSettings::default();
    let (_, args) = build_command_with_settings(&config, &settings);
    assert!(
        !args.contains(&"--betas".to_string()),
        "Extended context should be off by default in build_command_with_settings"
    );
}

#[test]
fn build_with_settings_excludes_chrome_by_default() {
    let config = create_test_config();
    let settings = ClaudeSettings::default();
    let (_, args) = build_command_with_settings(&config, &settings);
    assert!(
        !args.contains(&"--chrome".to_string()),
        "Chrome should be off by default in build_command_with_settings"
    );
}

#[test]
fn build_with_settings_respects_cli_options() {
    let mut config = create_test_config();
    config.claude_api_config = Some(ClaudeApiConfig {
        thinking_enabled: Some(false),
        extended_context_enabled: Some(true),
        chrome_enabled: Some(true),
        ..Default::default()
    });
    let settings = ClaudeSettings::default();
    let (_, args) = build_command_with_settings(&config, &settings);
    assert!(!args.contains(&"--settings".to_string()));
    assert!(args.contains(&"--betas".to_string()));
    assert!(args.contains(&"--chrome".to_string()));
}

#[test]
fn build_command_explicit_true_same_as_none_for_thinking() {
    let config_none = create_test_config();
    let (_, args_none) = build_command(&config_none);

    let mut config_explicit = create_test_config();
    config_explicit.claude_api_config = Some(ClaudeApiConfig {
        thinking_enabled: Some(true),
        ..Default::default()
    });
    let (_, args_explicit) = build_command(&config_explicit);

    assert!(args_none.contains(&"--settings".to_string()));
    assert!(args_explicit.contains(&"--settings".to_string()));
}

#[test]
fn build_command_prompt_is_last_with_cli_options() {
    let mut config = create_test_config();
    config.claude_api_config = Some(ClaudeApiConfig {
        thinking_enabled: Some(true),
        extended_context_enabled: Some(true),
        chrome_enabled: Some(true),
        ..Default::default()
    });
    let (_, args) = build_command(&config);
    assert_eq!(
        args.last(),
        Some(&"Test prompt".to_string()),
        "Prompt must be the last argument even with all CLI options enabled"
    );
}

// ── Provider-based build_command_from_provider_config tests ─────────

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
fn provider_build_defaults_to_opus_model() {
    let config = create_provider_config();
    let (_, args) = build_command_from_provider_config(&config);
    assert!(args.contains(&"claude-opus-4-6".to_string()));
}

#[test]
fn provider_build_maps_model_name() {
    let mut config = create_provider_config();
    config.model = Some("sonnet-4.5".to_string());
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
