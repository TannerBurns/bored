//! Workflow command prompt generation.

use std::path::Path;

use crate::agents::command_templates;

/// Build the ordered list of command file search locations
/// (custom dir first, then bundled).
pub(crate) fn build_command_search_paths(
    command: &str,
    custom_commands_dir: Option<&Path>,
) -> Vec<std::path::PathBuf> {
    let mut locations: Vec<std::path::PathBuf> = Vec::new();

    if let Some(custom_dir) = custom_commands_dir {
        locations.push(custom_dir.join(format!("{}.md", command)));
    }

    if let Some(bundled_dir) = command_templates::get_bundled_commands_path() {
        locations.push(bundled_dir.join(format!("{}.md", command)));
    }

    locations
}

/// Generate a prompt for a QA command stage (deslop, cleanup, unit-tests, etc.).
pub fn generate_command_prompt(
    command: &str,
    custom_commands_dir: Option<&Path>,
) -> String {
    let locations = build_command_search_paths(command, custom_commands_dir);

    let cmd_content = locations
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok());

    if let Some(content) = cmd_content {
        format!(
            r#"Execute the following command: /{command}

## Command Instructions

{content}

Execute these instructions carefully. When complete, report what was done.
"#
        )
    } else {
        get_fallback_command_prompt(command)
    }
}

/// Get a fallback prompt for a command if the command file is not found
fn get_fallback_command_prompt(command: &str) -> String {
    match command {
        "deslop" => r#"Execute the /deslop command:

Remove AI-generated code patterns:
- Unnecessary comments explaining obvious code
- Overly verbose or redundant code
- Placeholder TODOs that should be resolved
- Defensive code that's not actually needed

Focus on making the code clean and production-ready.
"#
        .to_string(),

        "cleanup" => r#"Execute the /cleanup command:

Fix all linting and type errors:
1. Run the linter and fix any issues
2. Run type checking and fix any errors
3. Ensure all imports are correct
4. Fix any formatting issues

Report any issues that couldn't be automatically fixed.
"#
        .to_string(),

        "unit-tests" => r#"Execute the /unit-tests command:

Add test coverage for the recent changes:
1. Identify the new or modified code
2. Create unit tests covering the main functionality
3. Test edge cases and error conditions
4. Ensure tests pass

Focus on meaningful tests that verify behavior, not just coverage.
"#
        .to_string(),

        "review-changes" => r#"Execute the /review-changes command:

Review all recent changes:
1. Check for code quality issues
2. Verify the implementation matches requirements
3. Look for potential bugs or edge cases
4. Ensure consistent style and patterns

Make any necessary improvements.
"#
        .to_string(),

        "add-and-commit" => r#"Execute the /add-and-commit command:

Stage and commit all changes:
1. Review what will be committed
2. Stage all relevant files
3. Create a commit message in Conventional Commits (commitizen) format:
   - Subject: `<type>(<scope>): <description>` (e.g., `feat(auth): add OAuth2 flow`)
   - Types: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert
   - Body: what changed, why, and how
   - Footer: BREAKING CHANGE (if any), Refs
"#
        .to_string(),

        _ => format!(
            r#"Execute the /{command} command:

Follow the project's conventions for this command.
If a command file exists in the project's agent configuration directory, follow those instructions.
"#
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn generate_command_prompt_fallback_deslop() {
        let prompt = generate_command_prompt("deslop", None);
        assert!(prompt.contains("deslop"));
        assert!(prompt.contains("AI-generated"));
    }

    #[test]
    fn generate_command_prompt_cleanup_contains_command_name() {
        let prompt = generate_command_prompt("cleanup", None);
        assert!(prompt.contains("cleanup"));
    }

    #[test]
    fn generate_command_prompt_unit_tests_contains_command_name() {
        let prompt = generate_command_prompt("unit-tests", None);
        assert!(prompt.contains("unit-tests"));
    }

    #[test]
    fn generate_command_prompt_review_changes_contains_command_name() {
        let prompt = generate_command_prompt("review-changes", None);
        assert!(
            prompt.contains("review-changes") || prompt.contains("review")
        );
    }

    #[test]
    fn generate_command_prompt_fallback_add_and_commit() {
        let prompt = generate_command_prompt("add-and-commit", None);
        assert!(prompt.contains("add-and-commit"));
        assert!(prompt.contains("commit"));
    }

    #[test]
    fn generate_command_prompt_unknown_returns_generic() {
        let prompt = generate_command_prompt("unknown-command", None);
        assert!(prompt.contains("unknown-command"));
        assert!(prompt.contains("project's conventions"));
    }

    #[test]
    fn generate_command_prompt_reads_from_bundled_if_exists() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let scripts_dir = manifest_dir.join("scripts/commands");

        if scripts_dir.join("code-review.md").exists() {
            let prompt = generate_command_prompt("code-review", None);
            assert!(prompt.contains("Execute the following command"));
        }
    }

    #[test]
    fn format_string_interpolates_command_and_content() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let cleanup_file = manifest_dir.join("scripts/commands/cleanup.md");

        if cleanup_file.exists() {
            let prompt = generate_command_prompt("cleanup", None);

            assert!(
                !prompt.contains("{command}"),
                "Variable was not interpolated"
            );
            assert!(
                !prompt.contains("{content}"),
                "Variable was not interpolated"
            );
            assert!(
                prompt.contains("/cleanup"),
                "command should be interpolated to '/cleanup'"
            );
            assert!(
                prompt.contains("senior engineer"),
                "File content should be interpolated into prompt"
            );
        }
    }

    #[test]
    fn new_catalog_commands_generate_prompts() {
        let new_commands = [
            "code-review", "code-review-fix", "add-tests", "fix-lint",
            "sync-with-main", "review-polish", "patch-security",
            "api-contract-check", "observability-pass", "integration-test",
        ];
        for cmd in &new_commands {
            let prompt = generate_command_prompt(cmd, None);
            assert!(
                !prompt.is_empty(),
                "Prompt for '{}' should not be empty",
                cmd
            );
            assert!(
                prompt.contains(cmd) || prompt.contains("Execute"),
                "Prompt for '{}' should reference the command",
                cmd
            );
        }
    }

    #[test]
    fn custom_command_name_is_not_remapped() {
        let prompt = generate_command_prompt("cleanup-advanced", None);
        assert!(
            prompt.contains("/cleanup-advanced"),
            "Custom command should use its own name, not be remapped to 'cleanup'"
        );
    }

    #[test]
    fn build_search_paths_with_custom_dir() {
        let custom = PathBuf::from("/tmp/custom-commands");
        let paths = build_command_search_paths("deslop", Some(&custom));

        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], PathBuf::from("/tmp/custom-commands/deslop.md"));
        assert!(paths[1].ends_with("scripts/commands/deslop.md"));
    }

    #[test]
    fn build_search_paths_without_custom_dir() {
        let paths = build_command_search_paths("deslop", None);
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("scripts/commands/deslop.md"));
    }

    #[test]
    fn custom_command_dir_takes_priority_over_bundled() {
        let temp_dir = std::env::temp_dir().join(format!("cmd_search_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("cleanup.md"), "# Custom cleanup override").unwrap();

        let prompt = generate_command_prompt("cleanup", Some(&temp_dir));
        assert!(prompt.contains("Custom cleanup override"), "Custom dir should take priority");

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn custom_dir_missing_command_falls_through_to_bundled() {
        let temp_dir = std::env::temp_dir().join(format!("cmd_fallthrough_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("unrelated.md"), "# not the command").unwrap();

        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        if manifest_dir.join("scripts/commands/cleanup.md").exists() {
            let prompt = generate_command_prompt("cleanup", Some(&temp_dir));
            assert!(
                prompt.contains("Execute the following command"),
                "Should fall through to bundled when custom dir lacks the command"
            );
        }

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn empty_custom_dir_falls_through_to_bundled() {
        let temp_dir = std::env::temp_dir().join(format!("cmd_empty_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        if manifest_dir.join("scripts/commands/deslop.md").exists() {
            let prompt = generate_command_prompt("deslop", Some(&temp_dir));
            assert!(
                prompt.contains("Execute the following command"),
                "Empty custom dir should fall through to bundled"
            );
        }

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn build_search_paths_custom_dir_is_first() {
        let custom = PathBuf::from("/my/custom");
        let paths = build_command_search_paths("test-cmd", Some(&custom));
        assert_eq!(paths[0], PathBuf::from("/my/custom/test-cmd.md"));
        assert!(
            paths.last().unwrap().ends_with("scripts/commands/test-cmd.md"),
            "Bundled should always be last"
        );
    }

    #[test]
    fn generate_command_prompt_none_custom_dir_uses_bundled_or_fallback() {
        let prompt = generate_command_prompt("code-review", None);
        assert!(
            !prompt.is_empty(),
            "Should produce a prompt even without custom dir"
        );
        assert!(
            prompt.contains("code-review") || prompt.contains("Execute"),
            "Should reference the command"
        );
    }
}
