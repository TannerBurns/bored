//! Worker environment validation for agents.

use std::path::Path;

use crate::agents::AgentKind;

mod checks;
mod types;

pub use types::{ValidationCheck, ValidationResult};

pub fn validate_worker_environment(
    agent_type: AgentKind,
    repo_path: &Path,
    api_url: Option<&str>,
) -> ValidationResult {
    validate_worker_environment_with_options(agent_type, repo_path, api_url, true)
}

/// Validate worker environment with configurable git requirement.
pub fn validate_worker_environment_with_options(
    agent_type: AgentKind,
    repo_path: &Path,
    api_url: Option<&str>,
    requires_git: bool,
) -> ValidationResult {
    let mut all_checks = Vec::new();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    let cli_check = checks::check_cli_available(agent_type);
    if !cli_check.passed {
        errors.push(cli_check.message.clone());
    }
    all_checks.push(cli_check);

    let hooks_check = checks::check_hooks_configured(agent_type, repo_path);
    if !hooks_check.passed {
        errors.push(hooks_check.message.clone());
    }
    all_checks.push(hooks_check);

    let commands_check = checks::check_commands_installed(agent_type, repo_path);
    if !commands_check.passed {
        errors.push(commands_check.message.clone());
    }
    all_checks.push(commands_check);

    if requires_git {
        let git_check = checks::check_git_repository(repo_path);
        if !git_check.passed {
            errors.push(git_check.message.clone());
        }
        all_checks.push(git_check);
    } else {
        all_checks.push(ValidationCheck::pass(
            "git_repository",
            "Git not required for this project",
        ));
    }

    if let Some(url) = api_url {
        let api_check = checks::check_api_url_configured(url);
        if !api_check.passed {
            errors.push(api_check.message.clone());
        }
        all_checks.push(api_check);
    }

    if requires_git {
        let clean_check = checks::check_git_clean_state(repo_path);
        if clean_check.is_warning {
            warnings.push(clean_check.message.clone());
        }
        all_checks.push(clean_check);
    }

    ValidationResult {
        valid: errors.is_empty(),
        checks: all_checks,
        errors,
        warnings,
    }
}

pub fn is_environment_valid(agent_type: AgentKind, repo_path: &Path) -> bool {
    let result = validate_worker_environment(agent_type, repo_path, None);
    result.valid
}

/// Check if environment is valid with configurable git requirement.
pub fn is_environment_valid_with_options(
    agent_type: AgentKind,
    repo_path: &Path,
    requires_git: bool,
) -> bool {
    let result =
        validate_worker_environment_with_options(agent_type, repo_path, None, requires_git);
    result.valid
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn validate_worker_environment_returns_result() {
        let temp_dir =
            std::env::temp_dir().join(format!("validation_test_{}", uuid::Uuid::new_v4()));
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
    fn validate_with_requires_git_false_skips_git_check() {
        let temp_dir =
            std::env::temp_dir().join(format!("validation_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Validate with requires_git=false - should not fail on missing git
        let result = validate_worker_environment_with_options(
            AgentKind::Cursor,
            &temp_dir,
            None,
            false, // requires_git = false
        );

        // Check that git_repository check shows "not required"
        let git_check = result.checks.iter().find(|c| c.name == "git_repository");
        assert!(git_check.is_some());
        assert!(git_check.unwrap().passed);
        assert!(git_check.unwrap().message.contains("not required"));

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn validate_with_requires_git_true_fails_on_missing_git() {
        let temp_dir =
            std::env::temp_dir().join(format!("validation_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Validate with requires_git=true - should fail on missing git
        let result = validate_worker_environment_with_options(
            AgentKind::Cursor,
            &temp_dir,
            None,
            true, // requires_git = true
        );

        // Check that git_repository check fails
        let git_check = result.checks.iter().find(|c| c.name == "git_repository");
        assert!(git_check.is_some());
        assert!(!git_check.unwrap().passed);

        std::fs::remove_dir_all(&temp_dir).ok();
    }
}
