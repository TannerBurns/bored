//! Workflow command prompt generation.

use std::path::Path;

use crate::agents::provider::AgentProvider;

/// Extract the base command name from a contextual stage name.
/// e.g., "cleanup-post-tests" -> "cleanup", "review-changes-final" -> "review-changes"
fn get_base_command(stage: &str) -> &str {
    if stage.starts_with("cleanup") {
        "cleanup"
    } else if stage.starts_with("review-changes") {
        "review-changes"
    } else {
        stage
    }
}

/// Build the list of command file search locations from registered providers.
///
/// For each provider, constructs `<repo>/<config_dir>/<subdir>/<command>.md`.
/// Appends the bundled commands directory as a final fallback.
pub(crate) fn build_command_search_paths(
    base_command: &str,
    repo_path: &Path,
    providers: &[&dyn AgentProvider],
) -> Vec<std::path::PathBuf> {
    let mut locations: Vec<std::path::PathBuf> = providers
        .iter()
        .map(|p| {
            repo_path
                .join(p.config_dir_name())
                .join(p.command_instructions_subdir())
                .join(format!("{}.md", base_command))
        })
        .collect();

    // Fallback to our bundled command files (for code-review, code-review-fix, etc.)
    locations.push(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("scripts/commands")
            .join(format!("{}.md", base_command)),
    );

    locations
}

/// Generate a prompt for a QA command stage (deslop, cleanup, unit-tests, etc.)
///
/// Searches bundled command files only. Use `generate_command_prompt_with_providers`
/// to also search provider-specific repo directories.
pub fn generate_command_prompt(command: &str, repo_path: &Path) -> String {
    generate_command_prompt_with_providers(command, repo_path, &[])
}

/// Like `generate_command_prompt`, but also searches each provider's command directory.
pub fn generate_command_prompt_with_providers(
    command: &str,
    repo_path: &Path,
    providers: &[&dyn AgentProvider],
) -> String {
    let base_command = get_base_command(command);

    let locations = build_command_search_paths(base_command, repo_path, providers);

    let cmd_content = locations
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok());

    if let Some(content) = cmd_content {
        format!(
            r#"Execute the following command: /{base_command}

## Command Instructions

{content}

Execute these instructions carefully. When complete, report what was done.
"#
        )
    } else {
        // Fallback prompts if command file not found
        get_fallback_command_prompt(base_command)
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
3. Create a detailed commit message describing:
   - What was changed
   - Why it was changed
   - Any notable implementation decisions

Use conventional commit format if the project uses it.
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
        let prompt = generate_command_prompt("deslop", Path::new("/nonexistent"));
        assert!(prompt.contains("deslop"));
        assert!(prompt.contains("AI-generated"));
    }

    #[test]
    fn generate_command_prompt_cleanup_contains_command_name() {
        let prompt = generate_command_prompt("cleanup", Path::new("/nonexistent"));
        assert!(prompt.contains("cleanup"));
    }

    #[test]
    fn generate_command_prompt_unit_tests_contains_command_name() {
        let prompt = generate_command_prompt("unit-tests", Path::new("/nonexistent"));
        assert!(prompt.contains("unit-tests"));
    }

    #[test]
    fn generate_command_prompt_review_changes_contains_command_name() {
        let prompt = generate_command_prompt("review-changes", Path::new("/nonexistent"));
        // May use bundled file or fallback - both are valid
        assert!(
            prompt.contains("review-changes") || prompt.contains("review")
        );
    }

    #[test]
    fn generate_command_prompt_fallback_add_and_commit() {
        let prompt = generate_command_prompt("add-and-commit", Path::new("/nonexistent"));
        assert!(prompt.contains("add-and-commit"));
        assert!(prompt.contains("commit"));
    }

    #[test]
    fn generate_command_prompt_unknown_returns_generic() {
        let prompt = generate_command_prompt("unknown-command", Path::new("/nonexistent"));
        assert!(prompt.contains("unknown-command"));
        assert!(prompt.contains("project's conventions"));
    }

    #[test]
    fn generate_command_prompt_reads_from_bundled_if_exists() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let scripts_dir = manifest_dir.join("scripts/commands");

        // Check if bundled command files exist
        if scripts_dir.join("code-review.md").exists() {
            let prompt = generate_command_prompt("code-review", Path::new("/nonexistent"));
            // Should contain content from the file, not fallback
            assert!(prompt.contains("Execute the following command"));
        }
    }

    #[test]
    fn format_macro_with_raw_string_interpolates_variables() {
        // This test verifies that format!() with raw string literals correctly
        // interpolates variables. Raw strings in Rust only affect escape sequence
        // handling, NOT macro variable interpolation.
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let cleanup_file = manifest_dir.join("scripts/commands/cleanup.md");

        if cleanup_file.exists() {
            let prompt = generate_command_prompt("cleanup", Path::new("/nonexistent"));

            // The format! macro should interpolate {base_command} to "cleanup"
            // If raw strings blocked interpolation, we'd see literal "{base_command}"
            assert!(
                !prompt.contains("{base_command}"),
                "Variable was not interpolated - prompt contains literal '{{base_command}}'"
            );
            assert!(
                !prompt.contains("{content}"),
                "Variable was not interpolated - prompt contains literal '{{content}}'"
            );

            // Verify the actual interpolation worked
            assert!(
                prompt.contains("/cleanup"),
                "base_command should be interpolated to '/cleanup'"
            );

            // The file content should be present (not the literal "{content}")
            assert!(
                prompt.contains("senior engineer"),
                "File content should be interpolated into prompt"
            );
        }
    }

    #[test]
    fn get_base_command_returns_cleanup_for_cleanup_variants() {
        assert_eq!(get_base_command("cleanup"), "cleanup");
        assert_eq!(get_base_command("cleanup-post-tests"), "cleanup");
        assert_eq!(get_base_command("cleanup-post-review"), "cleanup");
    }

    #[test]
    fn get_base_command_returns_review_changes_for_variants() {
        assert_eq!(get_base_command("review-changes"), "review-changes");
        assert_eq!(get_base_command("review-changes-final"), "review-changes");
    }

    #[test]
    fn get_base_command_returns_unchanged_for_other_commands() {
        assert_eq!(get_base_command("deslop"), "deslop");
        assert_eq!(get_base_command("unit-tests"), "unit-tests");
        assert_eq!(get_base_command("add-and-commit"), "add-and-commit");
        assert_eq!(get_base_command("implement"), "implement");
        assert_eq!(get_base_command("plan"), "plan");
    }

    #[test]
    fn contextual_cleanup_stages_use_cleanup_prompt() {
        // All cleanup variants should get the cleanup prompt (bundled or fallback)
        let cleanup_prompt = generate_command_prompt("cleanup", Path::new("/nonexistent"));
        let post_tests_prompt = generate_command_prompt("cleanup-post-tests", Path::new("/nonexistent"));
        let post_review_prompt = generate_command_prompt("cleanup-post-review", Path::new("/nonexistent"));

        // They should all contain cleanup-specific content (bundled file uses "lint", fallback uses "linting")
        assert!(
            cleanup_prompt.contains("lint") || cleanup_prompt.contains("cleanup"),
            "cleanup prompt missing expected content"
        );
        assert!(
            post_tests_prompt.contains("lint") || post_tests_prompt.contains("cleanup"),
            "cleanup-post-tests prompt missing expected content"
        );
        assert!(
            post_review_prompt.contains("lint") || post_review_prompt.contains("cleanup"),
            "cleanup-post-review prompt missing expected content"
        );
    }

    #[test]
    fn contextual_review_stages_use_review_changes_prompt() {
        // All review-changes variants should get the review-changes fallback prompt
        let review_prompt = generate_command_prompt("review-changes", Path::new("/nonexistent"));
        let final_prompt = generate_command_prompt("review-changes-final", Path::new("/nonexistent"));

        // They should all contain review-specific content
        assert!(review_prompt.contains("Review") || review_prompt.contains("review"));
        assert!(final_prompt.contains("Review") || final_prompt.contains("review"));
    }

    #[test]
    fn contextual_stages_show_base_command_in_prompt_header() {
        // Verify that the prompt header shows the base command, not the contextual stage name
        let post_tests_prompt =
            generate_command_prompt("cleanup-post-tests", Path::new("/nonexistent"));
        let post_review_prompt =
            generate_command_prompt("cleanup-post-review", Path::new("/nonexistent"));
        let review_final_prompt =
            generate_command_prompt("review-changes-final", Path::new("/nonexistent"));

        // Should show "/cleanup" not "/cleanup-post-tests"
        assert!(
            post_tests_prompt.contains("/cleanup") && !post_tests_prompt.contains("/cleanup-post"),
            "cleanup-post-tests should show /cleanup in header, not /cleanup-post-tests"
        );
        assert!(
            post_review_prompt.contains("/cleanup")
                && !post_review_prompt.contains("/cleanup-post"),
            "cleanup-post-review should show /cleanup in header, not /cleanup-post-review"
        );

        // Should show "/review-changes" not "/review-changes-final"
        assert!(
            review_final_prompt.contains("/review-changes")
                && !review_final_prompt.contains("/review-changes-final"),
            "review-changes-final should show /review-changes in header, not /review-changes-final"
        );
    }

    // ── Provider-aware path building ──────────────────────────────

    #[test]
    fn build_search_paths_with_providers() {
        use crate::agents::claude::provider::ClaudeProvider;
        use crate::agents::cursor::provider::CursorProvider;

        let claude = ClaudeProvider::new();
        let cursor = CursorProvider::new();
        let providers: Vec<&dyn crate::agents::provider::AgentProvider> =
            vec![&claude, &cursor];

        let paths = build_command_search_paths("deslop", Path::new("/repo"), &providers);

        // First two paths from providers, third is bundled fallback
        assert!(paths.len() >= 3);
        assert_eq!(paths[0], PathBuf::from("/repo/.claude/commands/deslop.md"));
        assert_eq!(paths[1], PathBuf::from("/repo/.cursor/rules/deslop.md"));
        // Last path is the bundled commands dir
        assert!(paths.last().unwrap().ends_with("scripts/commands/deslop.md"));
    }

    #[test]
    fn build_search_paths_empty_providers() {
        let paths = build_command_search_paths("deslop", Path::new("/repo"), &[]);
        // Only the bundled fallback
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("scripts/commands/deslop.md"));
    }

    #[test]
    fn generate_with_providers_falls_back_to_bundled() {
        // With providers pointing at nonexistent dirs, should still find bundled or fallback
        use crate::agents::claude::provider::ClaudeProvider;
        let claude = ClaudeProvider::new();
        let providers: Vec<&dyn crate::agents::provider::AgentProvider> = vec![&claude];

        let prompt = generate_command_prompt_with_providers(
            "deslop",
            Path::new("/nonexistent"),
            &providers,
        );
        // Should get either a bundled file or the hardcoded deslop fallback
        assert!(prompt.contains("deslop"));
    }
}
