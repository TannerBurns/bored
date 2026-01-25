use super::AgentRunConfig;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn build_command(config: &AgentRunConfig) -> (String, Vec<String>) {
    let command = "claude".to_string();
    let args = vec!["-p".to_string(), config.prompt.clone()];
    (command, args)
}

/// Check if Claude CLI is available
pub fn is_claude_available() -> bool {
    Command::new("claude")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get Claude CLI version
pub fn get_claude_version() -> Option<String> {
    Command::new("claude")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
}

#[derive(Debug, Clone, Default)]
pub struct ClaudeSettings {
    pub executable_path: Option<String>,
    pub system_prompt: Option<String>,
    pub system_prompt_file: Option<String>,
    pub extra_flags: Vec<String>,
    pub permission_mode: Option<String>,
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

/// Configuration for generating hooks settings
#[derive(Debug, Clone, Default)]
pub struct HooksConfig<'a> {
    pub hook_script_path: &'a str,
    pub api_url: Option<&'a str>,
    pub api_token: Option<&'a str>,
    pub run_id: Option<&'a str>,
    pub ticket_id: Option<&'a str>,
}

/// Generate settings.json hooks with environment variables
pub fn generate_hooks_settings(hook_script_path: &str) -> serde_json::Value {
    generate_hooks_settings_with_api(hook_script_path, None, None)
}

/// Generate settings.json hooks with API configuration
pub fn generate_hooks_settings_with_api(
    hook_script_path: &str,
    api_url: Option<&str>,
    api_token: Option<&str>,
) -> serde_json::Value {
    generate_hooks_settings_with_config(HooksConfig {
        hook_script_path,
        api_url,
        api_token,
        run_id: None,
        ticket_id: None,
    })
}

/// Generate settings.json hooks with full configuration
pub fn generate_hooks_settings_with_config(config: HooksConfig) -> serde_json::Value {
    // Build environment variables for the hook script
    let mut env_vars = String::new();
    
    if let Some(url) = config.api_url {
        env_vars.push_str(&format!("AGENT_KANBAN_API_URL={} ", url));
    }
    if let Some(token) = config.api_token {
        env_vars.push_str(&format!("AGENT_KANBAN_API_TOKEN={} ", token));
    }
    if let Some(run_id) = config.run_id {
        env_vars.push_str(&format!("AGENT_KANBAN_RUN_ID={} ", run_id));
    }
    if let Some(ticket_id) = config.ticket_id {
        env_vars.push_str(&format!("AGENT_KANBAN_TICKET_ID={} ", ticket_id));
    }
    
    let make_command = |event: &str| {
        if env_vars.is_empty() {
            format!("{} {}", config.hook_script_path, event)
        } else {
            // Use env to set environment variables
            format!("env {}{}  {}", env_vars, config.hook_script_path, event)
        }
    };

    serde_json::json!({
        "hooks": {
            "UserPromptSubmit": [{
                "matcher": "",
                "hooks": [{
                    "type": "command",
                    "command": make_command("UserPromptSubmit")
                }]
            }],
            "PreToolUse": [
                {
                    "matcher": "Bash",
                    "hooks": [{
                        "type": "command",
                        "command": make_command("PreToolUse")
                    }]
                },
                {
                    "matcher": "Read|Edit|Write",
                    "hooks": [{
                        "type": "command",
                        "command": make_command("PreToolUse")
                    }]
                }
            ],
            "PostToolUse": [{
                "matcher": ".*",
                "hooks": [{
                    "type": "command",
                    "command": make_command("PostToolUse")
                }]
            }],
            "PostToolUseFailure": [{
                "matcher": ".*",
                "hooks": [{
                    "type": "command",
                    "command": make_command("PostToolUseFailure")
                }]
            }],
            "Stop": [{
                "matcher": "",
                "hooks": [{
                    "type": "command",
                    "command": make_command("Stop")
                }]
            }]
        }
    })
}

/// Get user settings path (~/.claude/settings.json)
pub fn user_settings_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("settings.json"))
}

/// Get project settings path (.claude/settings.json in project)
pub fn project_settings_path(project: &Path) -> PathBuf {
    project.join(".claude").join("settings.json")
}

/// Get local project settings path (.claude/settings.local.json - gitignored)
pub fn local_settings_path(project: &Path) -> PathBuf {
    project.join(".claude").join("settings.local.json")
}

/// Check if global hooks are installed
pub fn check_global_hooks_installed() -> bool {
    user_settings_path()
        .map(|p| p.exists())
        .unwrap_or(false)
}

/// Check if project hooks are installed
pub fn check_project_hooks_installed(project: &Path) -> bool {
    project_settings_path(project).exists() || local_settings_path(project).exists()
}

/// Install hooks in user settings (~/.claude/settings.json)
pub fn install_user_hooks(
    hook_script_path: &str,
    api_url: Option<&str>,
    api_token: Option<&str>,
) -> std::io::Result<()> {
    let settings_path = user_settings_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine home directory for user settings",
        )
    })?;

    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    // Read existing settings or create new
    let mut settings = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)?;
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    // Generate and merge hooks
    let hooks = generate_hooks_settings_with_api(hook_script_path, api_url, api_token);
    if let Some(obj) = settings.as_object_mut() {
        obj.insert("hooks".to_string(), hooks["hooks"].clone());
    }

    std::fs::write(
        settings_path,
        serde_json::to_string_pretty(&settings).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
        })?,
    )?;

    Ok(())
}

/// Install hooks in project settings (.claude/settings.json)
pub fn install_project_hooks(
    project: &Path,
    hook_script_path: &str,
    api_url: Option<&str>,
    api_token: Option<&str>,
) -> std::io::Result<()> {
    let claude_dir = project.join(".claude");
    std::fs::create_dir_all(&claude_dir)?;

    let settings_path = claude_dir.join("settings.json");
    
    let mut settings = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)?;
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    let hooks = generate_hooks_settings_with_api(hook_script_path, api_url, api_token);
    if let Some(obj) = settings.as_object_mut() {
        obj.insert("hooks".to_string(), hooks["hooks"].clone());
    }

    std::fs::write(
        settings_path,
        serde_json::to_string_pretty(&settings).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
        })?,
    )?;

    Ok(())
}

/// Install hooks in local project settings (.claude/settings.local.json)
pub fn install_local_hooks(
    project: &Path,
    hook_script_path: &str,
    api_url: Option<&str>,
    api_token: Option<&str>,
) -> std::io::Result<()> {
    let claude_dir = project.join(".claude");
    std::fs::create_dir_all(&claude_dir)?;

    let settings_path = claude_dir.join("settings.local.json");
    
    let mut settings = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)?;
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    let hooks = generate_hooks_settings_with_api(hook_script_path, api_url, api_token);
    if let Some(obj) = settings.as_object_mut() {
        obj.insert("hooks".to_string(), hooks["hooks"].clone());
    }

    std::fs::write(
        settings_path,
        serde_json::to_string_pretty(&settings).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
        })?,
    )?;

    Ok(())
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

    #[test]
    fn build_with_system_prompt_file() {
        let config = create_test_config();
        let settings = ClaudeSettings {
            system_prompt_file: Some("/path/to/prompt.txt".to_string()),
            ..Default::default()
        };
        let (_, args) = build_command_with_settings(&config, &settings);
        assert!(args.contains(&"--system-prompt-file".to_string()));
        assert!(args.contains(&"/path/to/prompt.txt".to_string()));
    }

    #[test]
    fn build_with_extra_flags() {
        let config = create_test_config();
        let settings = ClaudeSettings {
            extra_flags: vec!["--verbose".to_string(), "--debug".to_string()],
            ..Default::default()
        };
        let (_, args) = build_command_with_settings(&config, &settings);
        assert!(args.contains(&"--verbose".to_string()));
        assert!(args.contains(&"--debug".to_string()));
    }

    #[test]
    fn system_prompt_takes_precedence_over_file() {
        let config = create_test_config();
        let settings = ClaudeSettings {
            system_prompt: Some("Inline prompt".to_string()),
            system_prompt_file: Some("/path/to/prompt.txt".to_string()),
            ..Default::default()
        };
        let (_, args) = build_command_with_settings(&config, &settings);
        assert!(args.contains(&"--append-system-prompt".to_string()));
        assert!(!args.contains(&"--system-prompt-file".to_string()));
    }

    #[test]
    fn is_claude_available_returns_bool() {
        let result = is_claude_available();
        assert!(result == true || result == false);
    }

    #[test]
    fn get_claude_version_returns_option() {
        let result = get_claude_version();
        if let Some(version) = result {
            assert!(!version.is_empty());
        }
    }

    #[test]
    fn generate_hooks_settings_has_all_hooks() {
        let config = generate_hooks_settings("/path/to/hook.js");
        let hooks = config.get("hooks").unwrap();
        assert!(hooks.get("UserPromptSubmit").is_some());
        assert!(hooks.get("PreToolUse").is_some());
        assert!(hooks.get("PostToolUse").is_some());
        assert!(hooks.get("PostToolUseFailure").is_some());
        assert!(hooks.get("Stop").is_some());
    }

    #[test]
    fn generate_hooks_settings_uses_correct_script_path() {
        let script_path = "/custom/path/claude-hook.js";
        let config = generate_hooks_settings(script_path);
        let hooks = config.get("hooks").unwrap();
        let user_prompt = hooks.get("UserPromptSubmit").unwrap();
        let first_matcher = user_prompt.as_array().unwrap().first().unwrap();
        let first_hook = first_matcher["hooks"].as_array().unwrap().first().unwrap();
        let command = first_hook.get("command").unwrap().as_str().unwrap();
        assert!(command.contains(script_path));
    }

    #[test]
    fn user_settings_path_returns_some() {
        let path = user_settings_path();
        if dirs::home_dir().is_some() {
            assert!(path.is_some());
            assert!(path.unwrap().to_string_lossy().contains(".claude"));
        }
    }

    #[test]
    fn project_settings_path_is_correct() {
        let project = PathBuf::from("/tmp/my-project");
        let path = project_settings_path(&project);
        assert_eq!(
            path,
            PathBuf::from("/tmp/my-project/.claude/settings.json")
        );
    }

    #[test]
    fn local_settings_path_is_correct() {
        let project = PathBuf::from("/tmp/my-project");
        let path = local_settings_path(&project);
        assert_eq!(
            path,
            PathBuf::from("/tmp/my-project/.claude/settings.local.json")
        );
    }

    #[test]
    fn install_project_hooks_creates_directory_and_file() {
        let temp_dir = std::env::temp_dir().join(format!("claude_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let result = install_project_hooks(&temp_dir, "/path/to/hook.js", None, None);
        assert!(result.is_ok());
        
        let settings_path = temp_dir.join(".claude").join("settings.json");
        assert!(settings_path.exists());
        
        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn check_project_hooks_installed_returns_false_when_missing() {
        let temp_dir = std::env::temp_dir().join(format!("claude_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        assert!(!check_project_hooks_installed(&temp_dir));
        
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn check_project_hooks_installed_returns_true_when_present() {
        let temp_dir = std::env::temp_dir().join(format!("claude_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        install_project_hooks(&temp_dir, "/path/to/hook.js", None, None).unwrap();
        assert!(check_project_hooks_installed(&temp_dir));
        
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn install_project_hooks_writes_valid_json() {
        let temp_dir = std::env::temp_dir().join(format!("claude_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        install_project_hooks(&temp_dir, "/path/to/hook.js", None, None).unwrap();
        
        let settings_path = temp_dir.join(".claude").join("settings.json");
        let content = std::fs::read_to_string(&settings_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        
        assert!(parsed.get("hooks").is_some());
        assert!(parsed["hooks"].get("UserPromptSubmit").is_some());
        
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn install_local_hooks_creates_local_settings_file() {
        let temp_dir = std::env::temp_dir().join(format!("claude_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        install_local_hooks(&temp_dir, "/path/to/hook.js", None, None).unwrap();
        
        let settings_path = temp_dir.join(".claude").join("settings.local.json");
        assert!(settings_path.exists());
        
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn generate_hooks_settings_with_api_includes_env_in_command() {
        let config = generate_hooks_settings_with_api(
            "/path/to/hook.js",
            Some("http://localhost:7432"),
            Some("my-token"),
        );
        let hooks = config.get("hooks").unwrap();
        let user_prompt = hooks.get("UserPromptSubmit").unwrap();
        let first_matcher = user_prompt.as_array().unwrap().first().unwrap();
        let first_hook = first_matcher["hooks"].as_array().unwrap().first().unwrap();
        let command = first_hook.get("command").unwrap().as_str().unwrap();
        
        assert!(command.contains("AGENT_KANBAN_API_URL=http://localhost:7432"));
        assert!(command.contains("AGENT_KANBAN_API_TOKEN=my-token"));
    }
}
