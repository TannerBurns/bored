use super::AgentRunConfig;

pub fn build_command(config: &AgentRunConfig) -> (String, Vec<String>) {
    let command = "claude".to_string();
    let args = vec!["-p".to_string(), config.prompt.clone()];
    (command, args)
}

/// Claude-specific settings
#[derive(Debug, Clone)]
pub struct ClaudeSettings {
    /// Path to claude executable (if not in PATH)
    pub executable_path: Option<String>,

    /// System prompt to append
    pub system_prompt: Option<String>,

    /// Path to system prompt file
    pub system_prompt_file: Option<String>,

    /// Additional CLI flags
    pub extra_flags: Vec<String>,

    /// Permission mode (default, ask, deny)
    pub permission_mode: Option<String>,
}

impl Default for ClaudeSettings {
    fn default() -> Self {
        Self {
            executable_path: None,
            system_prompt: None,
            system_prompt_file: None,
            extra_flags: vec![],
            permission_mode: None,
        }
    }
}

/// Build command with custom settings
#[allow(dead_code)]
pub fn build_command_with_settings(
    config: &AgentRunConfig,
    settings: &ClaudeSettings,
) -> (String, Vec<String>) {
    let command = settings
        .executable_path
        .clone()
        .unwrap_or_else(|| "claude".to_string());

    let mut args = vec![];

    if let Some(ref prompt) = settings.system_prompt {
        args.push("--append-system-prompt".to_string());
        args.push(prompt.clone());
    } else if let Some(ref file) = settings.system_prompt_file {
        args.push("--system-prompt-file".to_string());
        args.push(file.clone());
    }

    if let Some(ref mode) = settings.permission_mode {
        args.push("--permission-mode".to_string());
        args.push(mode.clone());
    }

    args.push("-p".to_string());
    args.push(config.prompt.clone());
    args.extend(settings.extra_flags.clone());

    (command, args)
}

/// Generate the settings.json hooks content for Claude Code
#[allow(dead_code)]
pub fn generate_hooks_config(api_url: &str, hook_script_path: &str) -> serde_json::Value {
    serde_json::json!({
        "hooks": {
            "UserPromptSubmit": [{
                "matcher": "",
                "hooks": [{
                    "type": "command",
                    "command": format!("{} UserPromptSubmit", hook_script_path)
                }]
            }],
            "PreToolUse": [{
                "matcher": ".*",
                "hooks": [{
                    "type": "command",
                    "command": format!("{} PreToolUse", hook_script_path)
                }]
            }],
            "PostToolUse": [{
                "matcher": ".*",
                "hooks": [{
                    "type": "command",
                    "command": format!("{} PostToolUse", hook_script_path)
                }]
            }],
            "Stop": [{
                "matcher": "",
                "hooks": [{
                    "type": "command",
                    "command": format!("{} Stop", hook_script_path)
                }]
            }]
        },
        "_meta": {
            "api_url": api_url
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_config() -> AgentRunConfig {
        AgentRunConfig {
            kind: super::super::AgentKind::Claude,
            ticket_id: "test-ticket".to_string(),
            run_id: "test-run".to_string(),
            repo_path: PathBuf::from("/tmp/test"),
            prompt: "Test prompt".to_string(),
            timeout_secs: Some(300),
            api_url: "http://localhost:7432".to_string(),
            api_token: "token".to_string(),
        }
    }

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
    fn generate_hooks_config_structure() {
        let config = generate_hooks_config("http://localhost:7432", "/path/to/hook.sh");
        assert!(config.get("hooks").is_some());
        let hooks = config.get("hooks").unwrap();
        assert!(hooks.get("UserPromptSubmit").is_some());
        assert!(hooks.get("PreToolUse").is_some());
        assert!(hooks.get("PostToolUse").is_some());
        assert!(hooks.get("Stop").is_some());
    }
}
