//! Codex CLI command building.

use crate::agents::provider::AgentRunConfig;

use super::provider::CodexApiConfig;

/// Build the `codex exec --json` command from an `AgentRunConfig`.
pub fn build_command_from_provider_config(config: &AgentRunConfig) -> (String, Vec<String>) {
    let api_config = CodexApiConfig::from_agent_config(&config.agent_config);

    let command = "codex".to_string();
    let mut args = vec![
        "exec".to_string(),
        "--json".to_string(),
        "--dangerously-bypass-approvals-and-sandbox".to_string(),
    ];

    if api_config.oss_enabled.unwrap_or(false) {
        args.push("--oss".to_string());

        if let Some(provider) = api_config.local_provider.as_ref().filter(|s| !s.is_empty()) {
            args.push("--local-provider".to_string());
            args.push(provider.clone());
        }
    }

    let effective_model = api_config
        .model_override
        .as_ref()
        .filter(|s| !s.is_empty())
        .cloned()
        .or_else(|| config.model.clone());

    if let Some(ref model) = effective_model {
        args.push("--model".to_string());
        args.push(model.clone());
    }

    let effort = api_config
        .reasoning_effort
        .as_ref()
        .filter(|s| !s.is_empty())
        .cloned()
        .unwrap_or_else(|| "high".to_string());
    args.push("--config".to_string());
    args.push(format!("model_reasoning_effort=\"{}\"", effort));

    args.push(config.prompt.clone());
    (command, args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_config() -> AgentRunConfig {
        AgentRunConfig {
            agent_id: "codex".to_string(),
            ticket_id: "test-ticket".to_string(),
            run_id: "test-run".to_string(),
            repo_path: PathBuf::from("/tmp/test"),
            prompt: "Test prompt".to_string(),
            timeout_secs: Some(300),
            api_url: "http://localhost:7432".to_string(),
            api_token: "token".to_string(),
            model: None,
            agent_config: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn build_command_returns_codex() {
        let config = create_test_config();
        let (cmd, _) = build_command_from_provider_config(&config);
        assert_eq!(cmd, "codex");
    }

    #[test]
    fn build_command_includes_exec_and_json() {
        let config = create_test_config();
        let (_, args) = build_command_from_provider_config(&config);
        assert_eq!(args[0], "exec");
        assert!(args.contains(&"--json".to_string()));
    }

    #[test]
    fn build_command_includes_bypass_flag() {
        let config = create_test_config();
        let (_, args) = build_command_from_provider_config(&config);
        assert!(args.contains(&"--dangerously-bypass-approvals-and-sandbox".to_string()));
    }

    #[test]
    fn build_command_includes_prompt_last() {
        let config = create_test_config();
        let (_, args) = build_command_from_provider_config(&config);
        assert_eq!(args.last(), Some(&"Test prompt".to_string()));
    }

    #[test]
    fn build_command_includes_model_when_specified() {
        let mut config = create_test_config();
        config.model = Some("gpt-5.3-codex".to_string());
        let (_, args) = build_command_from_provider_config(&config);
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"gpt-5.3-codex".to_string()));
    }

    #[test]
    fn build_command_omits_model_when_none() {
        let config = create_test_config();
        let (_, args) = build_command_from_provider_config(&config);
        assert!(!args.contains(&"--model".to_string()));
    }

    #[test]
    fn build_command_oss_disabled_no_extra_flags() {
        let mut config = create_test_config();
        config.agent_config.insert("oss_enabled".into(), serde_json::json!(false));
        let (_, args) = build_command_from_provider_config(&config);
        assert!(!args.contains(&"--oss".to_string()));
        assert!(!args.contains(&"--local-provider".to_string()));
    }

    #[test]
    fn build_command_oss_ollama() {
        let mut config = create_test_config();
        config.agent_config.insert("ossEnabled".into(), serde_json::json!(true));
        config.agent_config.insert("localProvider".into(), serde_json::json!("ollama"));
        config.agent_config.insert("modelOverride".into(), serde_json::json!("llama3.2"));
        let (_, args) = build_command_from_provider_config(&config);
        assert!(args.contains(&"--oss".to_string()));
        assert!(args.contains(&"--local-provider".to_string()));
        assert!(args.contains(&"ollama".to_string()));
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"llama3.2".to_string()));
        assert_eq!(args.last(), Some(&"Test prompt".to_string()));
    }

    #[test]
    fn build_command_oss_lmstudio() {
        let mut config = create_test_config();
        config.agent_config.insert("oss_enabled".into(), serde_json::json!(true));
        config.agent_config.insert("local_provider".into(), serde_json::json!("lmstudio"));
        config.agent_config.insert("model_override".into(), serde_json::json!("codestral"));
        let (_, args) = build_command_from_provider_config(&config);
        assert!(args.contains(&"--oss".to_string()));
        assert!(args.contains(&"--local-provider".to_string()));
        assert!(args.contains(&"lmstudio".to_string()));
        assert!(args.contains(&"codestral".to_string()));
    }

    #[test]
    fn build_command_model_override_takes_precedence() {
        let mut config = create_test_config();
        config.model = Some("gpt-5.3-codex".to_string());
        config.agent_config.insert("modelOverride".into(), serde_json::json!("my-custom-model"));
        let (_, args) = build_command_from_provider_config(&config);
        assert!(args.contains(&"my-custom-model".to_string()));
        assert!(!args.contains(&"gpt-5.3-codex".to_string()));
    }

    #[test]
    fn build_command_empty_model_override_falls_back_to_stage_model() {
        let mut config = create_test_config();
        config.model = Some("gpt-5.3-codex".to_string());
        config.agent_config.insert("modelOverride".into(), serde_json::json!(""));
        let (_, args) = build_command_from_provider_config(&config);
        assert!(args.contains(&"gpt-5.3-codex".to_string()));
    }

    #[test]
    fn build_command_oss_without_local_provider() {
        let mut config = create_test_config();
        config.agent_config.insert("ossEnabled".into(), serde_json::json!(true));
        let (_, args) = build_command_from_provider_config(&config);
        assert!(args.contains(&"--oss".to_string()));
        assert!(!args.contains(&"--local-provider".to_string()));
    }

    #[test]
    fn build_command_oss_with_empty_local_provider() {
        let mut config = create_test_config();
        config.agent_config.insert("ossEnabled".into(), serde_json::json!(true));
        config.agent_config.insert("localProvider".into(), serde_json::json!(""));
        let (_, args) = build_command_from_provider_config(&config);
        assert!(args.contains(&"--oss".to_string()));
        assert!(!args.contains(&"--local-provider".to_string()));
    }

    #[test]
    fn build_command_default_reasoning_effort() {
        let config = create_test_config();
        let (_, args) = build_command_from_provider_config(&config);
        assert!(args.contains(&"--config".to_string()));
        assert!(args.contains(&r#"model_reasoning_effort="high""#.to_string()));
    }

    #[test]
    fn build_command_custom_reasoning_effort() {
        let mut config = create_test_config();
        config.agent_config.insert("reasoningEffort".into(), serde_json::json!("xhigh"));
        let (_, args) = build_command_from_provider_config(&config);
        assert!(args.contains(&r#"model_reasoning_effort="xhigh""#.to_string()));
    }

    #[test]
    fn build_command_empty_reasoning_effort_falls_back_to_high() {
        let mut config = create_test_config();
        config.agent_config.insert("reasoningEffort".into(), serde_json::json!(""));
        let (_, args) = build_command_from_provider_config(&config);
        assert!(args.contains(&r#"model_reasoning_effort="high""#.to_string()));
    }

    #[test]
    fn build_command_reasoning_effort_snake_case_key() {
        let mut config = create_test_config();
        config.agent_config.insert("reasoning_effort".into(), serde_json::json!("medium"));
        let (_, args) = build_command_from_provider_config(&config);
        assert!(args.contains(&r#"model_reasoning_effort="medium""#.to_string()));
    }

    #[test]
    fn build_command_reasoning_effort_with_oss() {
        let mut config = create_test_config();
        config.agent_config.insert("ossEnabled".into(), serde_json::json!(true));
        config.agent_config.insert("localProvider".into(), serde_json::json!("ollama"));
        config.agent_config.insert("reasoningEffort".into(), serde_json::json!("low"));
        let (_, args) = build_command_from_provider_config(&config);
        assert!(args.contains(&"--oss".to_string()));
        assert!(args.contains(&"--config".to_string()));
        assert!(args.contains(&r#"model_reasoning_effort="low""#.to_string()));
        assert_eq!(args.last(), Some(&"Test prompt".to_string()));
    }

    #[test]
    fn build_command_reasoning_effort_before_prompt() {
        let mut config = create_test_config();
        config.agent_config.insert("reasoningEffort".into(), serde_json::json!("low"));
        let (_, args) = build_command_from_provider_config(&config);
        let config_idx = args.iter().position(|a| a == "--config").unwrap();
        let effort_idx = args.iter().position(|a| a == r#"model_reasoning_effort="low""#).unwrap();
        assert_eq!(effort_idx, config_idx + 1);
        assert_eq!(args.last(), Some(&"Test prompt".to_string()));
    }
}
