//! Codex CLI command building.

use crate::agents::provider::AgentRunConfig;

/// Build the `codex exec --json` command from an `AgentRunConfig`.
pub fn build_command_from_provider_config(config: &AgentRunConfig) -> (String, Vec<String>) {
    let command = "codex".to_string();
    let mut args = vec![
        "exec".to_string(),
        "--json".to_string(),
        "--dangerously-bypass-approvals-and-sandbox".to_string(),
    ];

    if let Some(ref model) = config.model {
        args.push("--model".to_string());
        args.push(model.clone());
    }

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
}
