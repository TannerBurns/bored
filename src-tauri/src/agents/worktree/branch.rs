//! Branch utilities for worktree operations

use std::path::Path;

use super::error::WorktreeError;
use super::git::git_command;

/// Check if a branch exists in the repository
pub fn branch_exists(repo_path: &Path, branch_name: &str) -> Result<bool, WorktreeError> {
    let output = git_command()
        .args([
            "rev-parse",
            "--verify",
            &format!("refs/heads/{}", branch_name),
        ])
        .current_dir(repo_path)
        .output()?;

    Ok(output.status.success())
}

/// Generate a branch name for a ticket (fallback deterministic naming)
pub fn generate_branch_name(ticket_id: &str, ticket_title: &str) -> String {
    // Sanitize the title for use in a branch name
    let sanitized_title: String = ticket_title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .take(6) // Limit to first 6 words
        .collect::<Vec<_>>()
        .join("-");

    // Use first 8 chars of ticket ID (char-based to avoid UTF-8 boundary issues)
    let short_id: String = ticket_id.chars().take(8).collect();

    format!("ticket/{}/{}", short_id, sanitized_title)
}
