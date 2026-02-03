//! Git worktree utilities for agent isolation
//!
//! Git worktrees allow multiple working directories for the same repository,
//! enabling true parallel agent execution without conflicts.

mod branch;
mod create;
mod error;
mod git;
mod manage;

#[cfg(test)]
mod tests;

// Re-export public types and functions
pub use branch::{branch_exists, generate_branch_name};
pub use create::{
    create_worktree, create_worktree_with_existing_branch, get_default_worktree_base,
    prune_stale_worktrees, WorktreeConfig, WorktreeInfo,
};
pub use error::{DiagnosticType, WorktreeError};
pub use git::{create_initial_commit, get_repo_root, is_git_repo, repo_has_commits};
pub use manage::{cleanup_stale_worktrees, list_worktrees, remove_worktree};
