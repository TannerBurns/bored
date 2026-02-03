//! Claude hooks configuration and installation.

use std::path::Path;

use super::settings::user_settings_path;
use super::shell_escape;

#[derive(Debug, Clone, Default)]
pub struct HooksConfig<'a> {
    pub hook_script_path: &'a str,
    pub api_url: Option<&'a str>,
    pub api_token: Option<&'a str>,
    pub run_id: Option<&'a str>,
    pub ticket_id: Option<&'a str>,
}

#[allow(dead_code)]
pub fn generate_hooks_config(api_url: &str, hook_script_path: &str) -> serde_json::Value {
    let escaped_path = shell_escape(hook_script_path);
    serde_json::json!({
        "hooks": {
            "UserPromptSubmit": [{
                "matcher": "",
                "hooks": [{
                    "type": "command",
                    "command": format!("{} UserPromptSubmit", escaped_path)
                }]
            }],
            "PreToolUse": [{
                "matcher": ".*",
                "hooks": [{
                    "type": "command",
                    "command": format!("{} PreToolUse", escaped_path)
                }]
            }],
            "PostToolUse": [{
                "matcher": ".*",
                "hooks": [{
                    "type": "command",
                    "command": format!("{} PostToolUse", escaped_path)
                }]
            }],
            "Stop": [{
                "matcher": "",
                "hooks": [{
                    "type": "command",
                    "command": format!("{} Stop", escaped_path)
                }]
            }]
        },
        "_meta": {
            "api_url": api_url
        }
    })
}

pub fn generate_hooks_settings(hook_script_path: &str) -> serde_json::Value {
    generate_hooks_settings_with_api(hook_script_path, None, None)
}

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

pub fn generate_hooks_settings_with_config(config: HooksConfig) -> serde_json::Value {
    // Build environment variables for the hook script, with proper shell escaping
    let mut env_vars = String::new();

    if let Some(url) = config.api_url {
        env_vars.push_str(&format!("AGENT_KANBAN_API_URL={} ", shell_escape(url)));
    }
    if let Some(token) = config.api_token {
        env_vars.push_str(&format!("AGENT_KANBAN_API_TOKEN={} ", shell_escape(token)));
    }
    if let Some(run_id) = config.run_id {
        env_vars.push_str(&format!("AGENT_KANBAN_RUN_ID={} ", shell_escape(run_id)));
    }
    if let Some(ticket_id) = config.ticket_id {
        env_vars.push_str(&format!(
            "AGENT_KANBAN_TICKET_ID={} ",
            shell_escape(ticket_id)
        ));
    }

    // Shell-escape the hook script path to handle spaces and special characters
    let escaped_path = shell_escape(config.hook_script_path);

    let make_command = |event: &str| {
        if env_vars.is_empty() {
            format!("{} {}", escaped_path, event)
        } else {
            // Use env to set environment variables
            format!("env {}{} {}", env_vars, escaped_path, event)
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
        serde_json::to_string_pretty(&settings)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?,
    )?;

    Ok(())
}

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
        serde_json::to_string_pretty(&settings)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?,
    )?;

    Ok(())
}

pub fn install_local_hooks(
    project: &Path,
    hook_script_path: &str,
    api_url: Option<&str>,
    api_token: Option<&str>,
) -> std::io::Result<()> {
    install_local_hooks_with_run_id(project, hook_script_path, api_url, api_token, None)
}

/// Install hooks to project's .claude/settings.local.json with run_id support
pub fn install_local_hooks_with_run_id(
    project: &Path,
    hook_script_path: &str,
    api_url: Option<&str>,
    api_token: Option<&str>,
    run_id: Option<&str>,
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

    let hooks = generate_hooks_settings_with_config(HooksConfig {
        hook_script_path,
        api_url,
        api_token,
        run_id,
        ticket_id: None,
    });
    if let Some(obj) = settings.as_object_mut() {
        obj.insert("hooks".to_string(), hooks["hooks"].clone());
    }

    std::fs::write(
        settings_path,
        serde_json::to_string_pretty(&settings)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?,
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::settings::check_project_hooks_installed;

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
    fn check_project_hooks_installed_returns_true_when_present() {
        let temp_dir = std::env::temp_dir().join(format!("claude_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        install_project_hooks(&temp_dir, "/path/to/hook.js", None, None).unwrap();
        assert!(check_project_hooks_installed(&temp_dir));

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

        // URL contains ':' which is not a safe character, so it gets quoted
        assert!(command.contains("AGENT_KANBAN_API_URL='http://localhost:7432'"));
        // Token only contains safe chars, so not quoted
        assert!(command.contains("AGENT_KANBAN_API_TOKEN=my-token"));
    }

    #[test]
    fn generate_hooks_settings_with_config_includes_run_and_ticket_id() {
        let config = generate_hooks_settings_with_config(HooksConfig {
            hook_script_path: "/path/to/hook.js",
            api_url: None,
            api_token: None,
            run_id: Some("run-123"),
            ticket_id: Some("ticket-456"),
        });
        let hooks = config.get("hooks").unwrap();
        let user_prompt = hooks.get("UserPromptSubmit").unwrap();
        let first_matcher = user_prompt.as_array().unwrap().first().unwrap();
        let first_hook = first_matcher["hooks"].as_array().unwrap().first().unwrap();
        let command = first_hook.get("command").unwrap().as_str().unwrap();

        assert!(command.contains("AGENT_KANBAN_RUN_ID=run-123"));
        assert!(command.contains("AGENT_KANBAN_TICKET_ID=ticket-456"));
    }

    #[test]
    fn generate_hooks_settings_without_env_vars_uses_simple_command() {
        let config = generate_hooks_settings("/path/to/hook.js");
        let hooks = config.get("hooks").unwrap();
        let user_prompt = hooks.get("UserPromptSubmit").unwrap();
        let first_matcher = user_prompt.as_array().unwrap().first().unwrap();
        let first_hook = first_matcher["hooks"].as_array().unwrap().first().unwrap();
        let command = first_hook.get("command").unwrap().as_str().unwrap();

        // Should be simple command without env prefix
        assert_eq!(command, "/path/to/hook.js UserPromptSubmit");
        assert!(!command.contains("env "));
    }

    #[test]
    fn generate_hooks_settings_includes_pretooluse_matchers() {
        let config = generate_hooks_settings("/path/to/hook.js");
        let hooks = config.get("hooks").unwrap();
        let pre_tool_use = hooks.get("PreToolUse").unwrap().as_array().unwrap();

        // Should have two matchers: Bash and Read|Edit|Write
        assert_eq!(pre_tool_use.len(), 2);

        let matchers: Vec<&str> = pre_tool_use
            .iter()
            .map(|m| m.get("matcher").unwrap().as_str().unwrap())
            .collect();
        assert!(matchers.contains(&"Bash"));
        assert!(matchers.contains(&"Read|Edit|Write"));
    }
}
