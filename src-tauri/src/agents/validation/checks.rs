//! Individual validation check functions.

use std::path::Path;

use super::types::ValidationCheck;
use crate::agents::claude;
use crate::agents::cursor;
use crate::agents::AgentKind;

pub fn check_cli_available(agent_type: AgentKind) -> ValidationCheck {
    let (available, name) = match agent_type {
        AgentKind::Cursor => (cursor::is_cursor_available(), "cursor"),
        AgentKind::Claude => (claude::is_claude_available(), "claude"),
    };

    if available {
        ValidationCheck::pass("cli_available", &format!("{} CLI is available", name))
    } else {
        ValidationCheck::fail(
            "cli_available",
            &format!("{} CLI is not installed or not in PATH", name),
            None,
        )
    }
}

pub fn check_hooks_configured(agent_type: AgentKind, repo_path: &Path) -> ValidationCheck {
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
        let location = if project_installed {
            "project"
        } else {
            "global"
        };
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

pub fn check_commands_installed(agent_type: AgentKind, repo_path: &Path) -> ValidationCheck {
    // Check user-level commands first (~/.cursor/commands/ or ~/.claude/commands/)
    let (user_installed, project_installed) = match agent_type {
        AgentKind::Cursor => (
            cursor::check_user_commands_installed(),
            cursor::check_project_commands_installed(repo_path),
        ),
        AgentKind::Claude => (
            claude::check_user_commands_installed(),
            claude::check_project_commands_installed(repo_path),
        ),
    };

    if user_installed || project_installed {
        let location = if user_installed { "user" } else { "project" };
        ValidationCheck::pass(
            "commands_installed",
            &format!("Command templates are installed ({})", location),
        )
    } else {
        ValidationCheck::fail(
            "commands_installed",
            "Command templates are not installed",
            Some("install_commands"),
        )
    }
}

pub fn check_git_repository(repo_path: &Path) -> ValidationCheck {
    let git_dir = repo_path.join(".git");

    if git_dir.exists() && git_dir.is_dir() {
        ValidationCheck::pass("git_repository", "Valid git repository")
    } else {
        ValidationCheck::fail("git_repository", "Not a git repository", None)
    }
}

pub fn check_api_url_configured(api_url: &str) -> ValidationCheck {
    if api_url.starts_with("http://") || api_url.starts_with("https://") {
        ValidationCheck::pass(
            "api_url_configured",
            &format!("API URL configured: {}", api_url),
        )
    } else {
        ValidationCheck::fail(
            "api_url_configured",
            &format!(
                "Invalid API URL format (must start with http:// or https://): {}",
                api_url
            ),
            None,
        )
    }
}

pub fn check_git_clean_state(repo_path: &Path) -> ValidationCheck {
    use std::process::Command;

    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_path)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.trim().is_empty() {
                ValidationCheck::pass("git_clean_state", "Git working tree is clean")
            } else {
                ValidationCheck::warning(
                    "git_clean_state",
                    "Git working tree has uncommitted changes",
                    None,
                )
            }
        }
        _ => ValidationCheck::warning("git_clean_state", "Could not check git status", None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn check_git_repository_detects_git_dir() {
        let temp_dir =
            std::env::temp_dir().join(format!("validation_test_{}", uuid::Uuid::new_v4()));
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
    fn check_git_clean_state_in_non_git_dir() {
        let temp_dir =
            std::env::temp_dir().join(format!("validation_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let check = check_git_clean_state(&temp_dir);
        // Should return a warning (passed=true, is_warning=true) since it can't check git status
        assert!(check.passed);
        assert!(check.is_warning);
        assert!(check.message.contains("Could not check"));

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn git_clean_state_warning_included_in_validation_result() {
        // Create a temp git repo with uncommitted changes
        let temp_dir =
            std::env::temp_dir().join(format!("validation_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Initialize git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&temp_dir)
            .output()
            .ok();

        // Create an untracked file to make the working tree dirty
        std::fs::write(temp_dir.join("test.txt"), "test content").unwrap();

        // Run the git clean check directly
        let check = check_git_clean_state(&temp_dir);
        assert!(
            check.is_warning,
            "Git clean state check should be a warning when there are uncommitted changes"
        );
        assert!(check.passed, "Warnings should have passed=true");
        assert!(
            check.message.contains("uncommitted"),
            "Warning message should mention uncommitted changes"
        );

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn is_environment_valid_returns_bool() {
        let temp_dir = PathBuf::from("/nonexistent/path");
        assert!(!super::super::is_environment_valid(
            AgentKind::Cursor,
            &temp_dir
        ));
    }
}
