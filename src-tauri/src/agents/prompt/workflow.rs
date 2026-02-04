//! Workflow command prompt generation.

use std::path::Path;

/// Generate a prompt for a QA command stage (deslop, cleanup, unit-tests, etc.)
pub fn generate_command_prompt(command: &str, repo_path: &Path) -> String {
    // Try to read the command file content from various locations
    let locations = [
        repo_path
            .join(".cursor/rules")
            .join(format!("{}.md", command)),
        repo_path
            .join(".claude/commands")
            .join(format!("{}.md", command)),
        // Fallback to our bundled command files (for code-review, code-review-fix, etc.)
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("scripts/commands")
            .join(format!("{}.md", command)),
    ];

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
        // Fallback prompts if command file not found
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
If a command file exists at .cursor/rules/{command}.md or .claude/commands/{command}.md, follow those instructions.
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
}
