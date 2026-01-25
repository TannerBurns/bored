//! Worker environment validation for agents.

use std::path::Path;
use serde::{Deserialize, Serialize};
use super::AgentKind;
use super::cursor;
use super::claude;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub valid: bool,
    pub checks: Vec<ValidationCheck>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationCheck {
    pub name: String,
    pub passed: bool,
    pub message: String,
    pub fix_action: Option<String>,
}

impl ValidationCheck {
    fn pass(name: &str, message: &str) -> Self {
        Self {
            name: name.to_string(),
            passed: true,
            message: message.to_string(),
            fix_action: None,
        }
    }

    fn fail(name: &str, message: &str, fix_action: Option<&str>) -> Self {
        Self {
            name: name.to_string(),
            passed: false,
            message: message.to_string(),
            fix_action: fix_action.map(|s| s.to_string()),
        }
    }

    fn warning(name: &str, message: &str, fix_action: Option<&str>) -> Self {
        Self {
            name: name.to_string(),
            passed: true, // Warnings don't fail validation
            message: message.to_string(),
            fix_action: fix_action.map(|s| s.to_string()),
        }
    }
}

pub fn validate_worker_environment(
    agent_type: AgentKind,
    repo_path: &Path,
    api_url: Option<&str>,
) -> ValidationResult {
    let mut checks = Vec::new();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    let cli_check = check_cli_available(agent_type);
    if !cli_check.passed {
        errors.push(cli_check.message.clone());
    }
    checks.push(cli_check);

    let hooks_check = check_hooks_configured(agent_type, repo_path);
    if !hooks_check.passed {
        errors.push(hooks_check.message.clone());
    }
    checks.push(hooks_check);

    let commands_check = check_commands_installed(agent_type, repo_path);
    if !commands_check.passed {
        errors.push(commands_check.message.clone());
    }
    checks.push(commands_check);

    let git_check = check_git_repository(repo_path);
    if !git_check.passed {
        errors.push(git_check.message.clone());
    }
    checks.push(git_check);

    if let Some(url) = api_url {
        let api_check = check_api_url_configured(url);
        if !api_check.passed {
            errors.push(api_check.message.clone());
        }
        checks.push(api_check);
    }

    let clean_check = check_git_clean_state(repo_path);
    if !clean_check.passed {
        warnings.push(clean_check.message.clone());
    }
    checks.push(clean_check);

    ValidationResult {
        valid: errors.is_empty(),
        checks,
        errors,
        warnings,
    }
}

fn check_cli_available(agent_type: AgentKind) -> ValidationCheck {
    let (available, name) = match agent_type {
        AgentKind::Cursor => (cursor::is_cursor_available(), "cursor"),
        AgentKind::Claude => (claude::is_claude_available(), "claude"),
    };

    if available {
        ValidationCheck::pass(
            "cli_available",
            &format!("{} CLI is available", name),
        )
    } else {
        ValidationCheck::fail(
            "cli_available",
            &format!("{} CLI is not installed or not in PATH", name),
            None,
        )
    }
}

fn check_hooks_configured(agent_type: AgentKind, repo_path: &Path) -> ValidationCheck {
    let (global_installed, project_installed) = match agent_type {
        AgentKind::Cursor => (
            cursor::check_global_hooks_installed(),
            cursor::check_project_hooks_installed(repo_path),
        ),
        AgentKind::Claude => (
            claude::check_global_hooks_installed(),
            claude::check_project_hooks_installed(repo_path),
        ),
    };

    if global_installed || project_installed {
        let location = if project_installed { "project" } else { "global" };
        ValidationCheck::pass(
            "hooks_configured",
            &format!("Hooks are configured ({})", location),
        )
    } else {
        ValidationCheck::fail(
            "hooks_configured",
            "Hooks are not configured",
            Some("install_hooks"),
        )
    }
}

fn check_commands_installed(agent_type: AgentKind, repo_path: &Path) -> ValidationCheck {
    let installed = match agent_type {
        AgentKind::Cursor => cursor::check_project_commands_installed(repo_path),
        AgentKind::Claude => claude::check_project_commands_installed(repo_path),
    };

    if installed {
        ValidationCheck::pass(
            "commands_installed",
            "Command templates are installed",
        )
    } else {
        ValidationCheck::fail(
            "commands_installed",
            "Command templates are not installed",
            Some("install_commands"),
        )
    }
}

fn check_git_repository(repo_path: &Path) -> ValidationCheck {
    let git_dir = repo_path.join(".git");
    
    if git_dir.exists() && git_dir.is_dir() {
        ValidationCheck::pass(
            "git_repository",
            "Valid git repository",
        )
    } else {
        ValidationCheck::fail(
            "git_repository",
            "Not a git repository",
            None,
        )
    }
}

fn check_api_url_configured(api_url: &str) -> ValidationCheck {
    if api_url.starts_with("http://") || api_url.starts_with("https://") {
        ValidationCheck::pass(
            "api_url_configured",
            &format!("API URL configured: {}", api_url),
        )
    } else {
        ValidationCheck::fail(
            "api_url_configured",
            &format!("Invalid API URL format (must start with http:// or https://): {}", api_url),
            None,
        )
    }
}

fn check_git_clean_state(repo_path: &Path) -> ValidationCheck {
    use std::process::Command;
    
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_path)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.trim().is_empty() {
                ValidationCheck::pass(
                    "git_clean_state",
                    "Git working tree is clean",
                )
            } else {
                ValidationCheck::warning(
                    "git_clean_state",
                    "Git working tree has uncommitted changes",
                    None,
                )
            }
        }
        _ => ValidationCheck::warning(
            "git_clean_state",
            "Could not check git status",
            None,
        ),
    }
}

pub fn is_environment_valid(agent_type: AgentKind, repo_path: &Path) -> bool {
    let result = validate_worker_environment(agent_type, repo_path, None);
    result.valid
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn validation_check_pass() {
        let check = ValidationCheck::pass("test", "Test passed");
        assert!(check.passed);
        assert_eq!(check.name, "test");
        assert!(check.fix_action.is_none());
    }

    #[test]
    fn validation_check_fail() {
        let check = ValidationCheck::fail("test", "Test failed", Some("fix_it"));
        assert!(!check.passed);
        assert_eq!(check.fix_action, Some("fix_it".to_string()));
    }

    #[test]
    fn validation_check_warning() {
        let check = ValidationCheck::warning("test", "Warning message", None);
        assert!(check.passed); // Warnings don't fail
    }

    #[test]
    fn validation_result_serializes() {
        let result = ValidationResult {
            valid: true,
            checks: vec![ValidationCheck::pass("test", "OK")],
            errors: vec![],
            warnings: vec!["A warning".to_string()],
        };
        
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"valid\":true"));
        assert!(json.contains("\"checks\""));
    }

    #[test]
    fn check_git_repository_detects_git_dir() {
        let temp_dir = std::env::temp_dir().join(format!("validation_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        // No .git directory
        let check = check_git_repository(&temp_dir);
        assert!(!check.passed);
        
        // Create .git directory
        std::fs::create_dir_all(temp_dir.join(".git")).unwrap();
        let check = check_git_repository(&temp_dir);
        assert!(check.passed);
        
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn check_api_url_configured_validates_url_format() {
        let check = check_api_url_configured("http://localhost:7432");
        assert!(check.passed);
        assert_eq!(check.name, "api_url_configured");
        
        let check = check_api_url_configured("https://api.example.com");
        assert!(check.passed);
        
        let check = check_api_url_configured("invalid-url");
        assert!(!check.passed);
        assert!(check.message.contains("Invalid API URL format"));
    }

    #[test]
    fn validate_worker_environment_returns_result() {
        let temp_dir = std::env::temp_dir().join(format!("validation_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let result = validate_worker_environment(AgentKind::Cursor, &temp_dir, None);
        
        // Should have multiple checks
        assert!(!result.checks.is_empty());
        
        // Should fail because not a git repo, no hooks, no commands
        assert!(!result.valid);
        assert!(!result.errors.is_empty());
        
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn is_environment_valid_returns_bool() {
        let temp_dir = PathBuf::from("/nonexistent/path");
        assert!(!is_environment_valid(AgentKind::Cursor, &temp_dir));
    }

    #[test]
    fn check_git_clean_state_in_non_git_dir() {
        let temp_dir = std::env::temp_dir().join(format!("validation_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let check = check_git_clean_state(&temp_dir);
        // Should return a warning (passed=true) since it can't check git status
        assert!(check.passed);
        assert!(check.message.contains("Could not check"));
        
        std::fs::remove_dir_all(&temp_dir).ok();
    }
}
