//! Tests for worktree operations

use super::*;
use std::path::PathBuf;

#[test]
fn test_generate_branch_name() {
    let branch = generate_branch_name(
        "abc12345-def6-7890-ghij-klmnopqrstuv",
        "Add user authentication feature",
    );
    assert_eq!(branch, "ticket/abc12345/add-user-authentication-feature");
}

#[test]
fn test_generate_branch_name_special_chars() {
    let branch =
        generate_branch_name("test-id-123", "Fix bug: can't login with special chars!@#");
    assert!(branch.starts_with("ticket/test-id-"));
    assert!(!branch.contains('!'));
    assert!(!branch.contains('@'));
    assert!(!branch.contains(':'));
}

#[test]
fn test_generate_branch_name_long_title() {
    let branch = generate_branch_name(
        "id123456",
        "This is a very long title with many words that should be truncated",
    );
    // Should only have 6 words from title
    let parts: Vec<_> = branch.split('/').collect();
    assert_eq!(parts.len(), 3);
    let title_part = parts[2];
    let word_count = title_part.split('-').count();
    assert!(word_count <= 6);
}

#[test]
fn test_default_worktree_base() {
    let base = get_default_worktree_base();
    assert!(base.to_string_lossy().contains("agent-kanban"));
    assert!(base.to_string_lossy().contains("worktrees"));
}

#[test]
fn test_is_network_error_connection_refused() {
    assert!(git::is_network_error(
        "ssh: connect to host github.com port 22: Connection refused"
    ));
    assert!(git::is_network_error("Connection refused"));
}

#[test]
fn test_is_network_error_connection_timed_out() {
    assert!(git::is_network_error(
        "ssh: connect to host github.com port 22: Connection timed out"
    ));
    assert!(git::is_network_error("Connection timed out"));
}

#[test]
fn test_is_network_error_host_resolution() {
    assert!(git::is_network_error("ssh: Could not resolve host github.com"));
    assert!(git::is_network_error(
        "fatal: Could not resolve host: github.com"
    ));
}

#[test]
fn test_is_network_error_unreachable() {
    assert!(git::is_network_error("Network is unreachable"));
    assert!(git::is_network_error("No route to host"));
}

#[test]
fn test_network_error_not_ssh_auth() {
    // Network errors should NOT be detected as SSH auth errors
    assert!(!git::is_ssh_auth_error("Connection refused"));
    assert!(!git::is_ssh_auth_error("Connection timed out"));
    assert!(!git::is_ssh_auth_error("Could not resolve host"));
}

#[test]
fn test_ssh_auth_error_patterns() {
    // These should still be detected as SSH auth errors
    assert!(git::is_ssh_auth_error("Permission denied (publickey)"));
    assert!(git::is_ssh_auth_error("Host key verification failed"));
    assert!(git::is_ssh_auth_error("passphrase for key"));
}

#[test]
fn test_extract_network_error_message_connection_refused() {
    let msg = git::extract_network_error_message("ssh: connect to host github.com port 22: Connection refused");
    assert!(msg.contains("Connection refused"));
    assert!(msg.contains("remote server"));
}

#[test]
fn test_extract_network_error_message_timed_out() {
    let msg = git::extract_network_error_message("Connection timed out");
    assert!(msg.contains("timed out"));
    assert!(msg.contains("network connection"));
}

#[test]
fn test_extract_network_error_message_host_resolution() {
    let msg = git::extract_network_error_message("fatal: Could not resolve host: github.com");
    assert!(msg.contains("resolve hostname"));
    assert!(msg.contains("DNS"));
}

#[test]
fn test_extract_network_error_message_unreachable() {
    let msg = git::extract_network_error_message("Network is unreachable");
    assert!(msg.contains("unreachable"));
    assert!(msg.contains("internet connection"));
}

#[test]
fn test_extract_network_error_message_no_route() {
    let msg = git::extract_network_error_message("No route to host");
    assert!(msg.contains("No route to host"));
}

#[test]
fn test_extract_network_error_message_reset_by_peer() {
    let msg = git::extract_network_error_message("Connection reset by peer");
    assert!(msg.contains("reset"));
    assert!(msg.contains("remote server"));
}

#[test]
fn test_extract_network_error_message_fallback() {
    let msg = git::extract_network_error_message("Some unknown network error\nSecond line");
    assert_eq!(msg, "Some unknown network error");
}

#[test]
fn test_extract_ssh_error_message_publickey() {
    let msg = git::extract_ssh_error_message("Permission denied (publickey).");
    assert!(msg.contains("SSH key authentication failed"));
    assert!(msg.contains("ssh-agent"));
}

#[test]
fn test_extract_ssh_error_message_passphrase() {
    let msg = git::extract_ssh_error_message("Enter passphrase for key '/home/user/.ssh/id_rsa':");
    assert!(msg.contains("passphrase"));
    assert!(msg.contains("agent"));
}

#[test]
fn test_extract_ssh_error_message_askpass() {
    let msg = git::extract_ssh_error_message("ssh_askpass: exec(/usr/bin/ssh-askpass): No such file");
    assert!(msg.contains("passphrase"));
}

#[test]
fn test_extract_ssh_error_message_host_key() {
    let msg = git::extract_ssh_error_message("Host key verification failed");
    assert!(msg.contains("host key verification"));
    assert!(msg.contains("remote host"));
}

#[test]
fn test_extract_ssh_error_message_fallback() {
    let msg = git::extract_ssh_error_message("Some unknown SSH error\nSecond line");
    assert_eq!(msg, "Some unknown SSH error");
}

#[test]
fn test_network_error_diagnostic_type() {
    let error = WorktreeError::NetworkError {
        message: "Connection refused".to_string(),
        stderr: "ssh: connect to host github.com port 22: Connection refused".to_string(),
        exit_code: Some(128),
        operation: "git fetch".to_string(),
    };

    assert_eq!(error.diagnostic_type(), DiagnosticType::NetworkError);
    assert_eq!(error.operation(), Some("git fetch"));
    assert!(error.stderr().is_some());
}

#[test]
fn test_git_error_diagnostic_type() {
    let error = WorktreeError::GitError {
        message: "Failed to create worktree".to_string(),
        stderr: "fatal: 'branch' is already checked out at '/tmp/worktree'".to_string(),
        exit_code: Some(128),
        operation: "git worktree add".to_string(),
    };

    assert_eq!(error.diagnostic_type(), DiagnosticType::GitError);
    assert_eq!(error.operation(), Some("git worktree add"));
    assert_eq!(
        error.stderr(),
        Some("fatal: 'branch' is already checked out at '/tmp/worktree'")
    );
    assert_eq!(error.exit_code(), Some(128));
}

#[test]
fn test_prune_stale_worktrees_in_git_repo() {
    // Create a temp git repo
    let temp_dir = std::env::temp_dir().join(format!("prune_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Initialize git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&temp_dir)
        .output()
        .ok();

    // Make an initial commit so we have a valid repo
    std::fs::write(temp_dir.join("README.md"), "test").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&temp_dir)
        .output()
        .ok();
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&temp_dir)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .output()
        .ok();

    // prune_stale_worktrees should succeed (no-op if nothing to prune)
    let result = prune_stale_worktrees(&temp_dir);
    assert!(result.is_ok());

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_extract_worktree_path_with_quotes() {
    let stderr =
        "fatal: 'feature/test' is already checked out at '/tmp/agent-kanban/worktrees/abc123'";
    let result = git::extract_worktree_path_from_error(stderr);
    assert_eq!(
        result,
        Some("/tmp/agent-kanban/worktrees/abc123".to_string())
    );
}

#[test]
fn test_extract_worktree_path_without_quotes() {
    let stderr = "fatal: branch is already checked out at /var/folders/89/test/worktree";
    let result = git::extract_worktree_path_from_error(stderr);
    assert_eq!(result, Some("/var/folders/89/test/worktree".to_string()));
}

#[test]
fn test_extract_worktree_path_no_match() {
    let stderr = "fatal: some other error occurred";
    let result = git::extract_worktree_path_from_error(stderr);
    assert_eq!(result, None);
}

#[test]
fn test_is_our_worktree_with_agent_kanban_path() {
    // Should detect paths in our temp directory
    assert!(manage::is_our_worktree("/tmp/agent-kanban/worktrees/abc123"));
    assert!(manage::is_our_worktree(
        "/private/var/folders/89/xmt0wws/T/agent-kanban/worktrees/62e286f9"
    ));
}

#[test]
fn test_is_our_worktree_with_external_path() {
    // Should not match external paths
    assert!(!manage::is_our_worktree(
        "/home/user/my-project/.git/worktrees/feature"
    ));
    assert!(!manage::is_our_worktree("/Users/dev/code/worktree"));
}

#[test]
fn test_get_worktree_repo_path_with_valid_gitdir() {
    // Create a temp worktree-like structure
    let temp_dir =
        std::env::temp_dir().join(format!("worktree_repo_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Create a fake .git file
    let git_file = temp_dir.join(".git");
    std::fs::write(
        &git_file,
        "gitdir: /Users/test/my-repo/.git/worktrees/abc123\n",
    )
    .unwrap();

    let result = manage::get_worktree_repo_path(temp_dir.to_string_lossy().as_ref());
    assert_eq!(result, Some(PathBuf::from("/Users/test/my-repo")));

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_get_worktree_repo_path_with_no_git_file() {
    let temp_dir =
        std::env::temp_dir().join(format!("worktree_repo_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    // No .git file
    let result = manage::get_worktree_repo_path(temp_dir.to_string_lossy().as_ref());
    assert_eq!(result, None);

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_git_error_permission_denied_in_stderr() {
    let error = WorktreeError::GitError {
        message: "Failed".to_string(),
        stderr: "error: Permission denied while writing".to_string(),
        exit_code: Some(1),
        operation: "git checkout".to_string(),
    };

    assert_eq!(error.diagnostic_type(), DiagnosticType::Permission);
}

#[test]
fn test_git_error_network_error_in_stderr() {
    let error = WorktreeError::GitError {
        message: "Failed to fetch".to_string(),
        stderr: "fatal: Could not resolve host: github.com".to_string(),
        exit_code: Some(128),
        operation: "git fetch".to_string(),
    };

    assert_eq!(error.diagnostic_type(), DiagnosticType::NetworkError);
}

#[test]
fn test_git_error_network_unreachable_in_message() {
    let error = WorktreeError::GitError {
        message: "Network is unreachable".to_string(),
        stderr: "".to_string(),
        exit_code: Some(128),
        operation: "git push".to_string(),
    };

    assert_eq!(error.diagnostic_type(), DiagnosticType::NetworkError);
}

#[test]
fn test_git_error_generic_falls_through() {
    let error = WorktreeError::GitError {
        message: "Something went wrong".to_string(),
        stderr: "fatal: unexpected error".to_string(),
        exit_code: Some(1),
        operation: "git status".to_string(),
    };

    assert_eq!(error.diagnostic_type(), DiagnosticType::GitError);
}

#[test]
fn test_is_worktree_conflict_error_old_format() {
    // Older git versions use "already checked out"
    assert!(git::is_worktree_conflict_error(
        "fatal: 'branch' is already checked out at '/path'"
    ));
}

#[test]
fn test_is_worktree_conflict_error_new_format() {
    // Newer git versions use "already used by worktree"
    assert!(git::is_worktree_conflict_error(
        "fatal: 'fix/abc123' is already used by worktree at '/private/var/folders/...'"
    ));
}

#[test]
fn test_is_worktree_conflict_error_already_exists() {
    assert!(git::is_worktree_conflict_error("fatal: branch already exists"));
}

#[test]
fn test_is_worktree_conflict_error_no_match() {
    assert!(!git::is_worktree_conflict_error("fatal: some other error"));
    assert!(!git::is_worktree_conflict_error("fatal: Permission denied"));
}

#[test]
fn test_extract_worktree_path_new_git_format() {
    // Newer git format: "already used by worktree at 'path'"
    let stderr = "fatal: 'fix/cff1ae76/remove-empty-categories-summary' is already used by worktree at '/private/var/folders/89/xmt0wws13ksdtn4_wm0g1_p40000gn/T/agent-kanban/worktrees/ccbc02ff-6c66-45fc-8b83-330bcb4f5f98'";
    let result = git::extract_worktree_path_from_error(stderr);
    assert_eq!(result, Some("/private/var/folders/89/xmt0wws13ksdtn4_wm0g1_p40000gn/T/agent-kanban/worktrees/ccbc02ff-6c66-45fc-8b83-330bcb4f5f98".to_string()));
}

#[test]
fn test_extract_worktree_path_new_git_format_without_quotes() {
    // Fallback pattern 3: "used by worktree at" without quotes
    let stderr = "fatal: branch is already used by worktree at /var/folders/test/worktree";
    let result = git::extract_worktree_path_from_error(stderr);
    assert_eq!(result, Some("/var/folders/test/worktree".to_string()));
}

#[test]
fn test_extract_worktree_path_old_format_without_quotes() {
    // Fallback pattern 3: "checked out at" without quotes
    let stderr = "fatal: branch is already checked out at /tmp/worktree-dir";
    let result = git::extract_worktree_path_from_error(stderr);
    assert_eq!(result, Some("/tmp/worktree-dir".to_string()));
}

#[test]
fn test_extract_worktree_path_new_git_format_with_quotes() {
    // Pattern 2: "used by worktree at 'path'" with quotes
    let stderr = "fatal: branch is already used by worktree at '/path/with/quote'";
    let result = git::extract_worktree_path_from_error(stderr);
    assert_eq!(result, Some("/path/with/quote".to_string()));
}

#[test]
fn test_is_unborn_branch_error_invalid_reference() {
    assert!(error::is_unborn_branch_error("fatal: invalid reference: main"));
    assert!(error::is_unborn_branch_error("fatal: invalid reference: HEAD"));
}

#[test]
fn test_is_unborn_branch_error_not_valid_object_name() {
    assert!(error::is_unborn_branch_error(
        "fatal: not a valid object name: 'main'"
    ));
    assert!(error::is_unborn_branch_error("error: not a valid object name"));
}

#[test]
fn test_is_unborn_branch_error_no_commits() {
    assert!(error::is_unborn_branch_error(
        "fatal: your current branch 'main' does not have any commits yet"
    ));
}

#[test]
fn test_is_unborn_branch_error_bad_revision() {
    assert!(error::is_unborn_branch_error("fatal: bad revision 'HEAD'"));
    assert!(error::is_unborn_branch_error(
        "fatal: unknown revision or path not in the working tree"
    ));
}

#[test]
fn test_is_unborn_branch_error_not_other_errors() {
    assert!(!error::is_unborn_branch_error("fatal: Permission denied"));
    assert!(!error::is_unborn_branch_error("fatal: could not resolve host"));
    assert!(!error::is_unborn_branch_error(
        "error: pathspec 'file' did not match"
    ));
}

#[test]
fn test_unborn_branch_error_diagnostic_type() {
    let error = WorktreeError::UnbornBranch {
        message: "Repository has no commits".to_string(),
        stderr: "fatal: invalid reference: main".to_string(),
    };

    assert_eq!(error.diagnostic_type(), DiagnosticType::UnbornBranch);
    assert_eq!(error.stderr(), Some("fatal: invalid reference: main"));
    assert_eq!(error.operation(), Some("git worktree add"));
}

#[test]
fn test_git_error_unborn_branch_detection() {
    // GitError should also be classified as UnbornBranch when stderr matches patterns
    let error = WorktreeError::GitError {
        message: "Failed to create worktree".to_string(),
        stderr: "fatal: invalid reference: main".to_string(),
        exit_code: Some(128),
        operation: "git worktree add".to_string(),
    };

    assert_eq!(error.diagnostic_type(), DiagnosticType::UnbornBranch);
}

#[test]
fn test_diagnostic_type_as_str() {
    assert_eq!(DiagnosticType::SshAuth.as_str(), "ssh_auth");
    assert_eq!(DiagnosticType::Timeout.as_str(), "timeout");
    assert_eq!(DiagnosticType::Permission.as_str(), "permission");
    assert_eq!(DiagnosticType::NetworkError.as_str(), "network_error");
    assert_eq!(DiagnosticType::GitError.as_str(), "git_error");
    assert_eq!(DiagnosticType::UnbornBranch.as_str(), "unborn_branch");
    assert_eq!(DiagnosticType::Unknown.as_str(), "unknown");
}

#[test]
fn test_repo_has_commits_on_new_repo() {
    // Create a fresh git repo with no commits
    let temp_dir =
        std::env::temp_dir().join(format!("repo_commits_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&temp_dir)
        .output()
        .ok();

    // Should return false for repo with no commits
    assert!(!git::repo_has_commits(&temp_dir));

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_repo_has_commits_after_commit() {
    // Create a git repo and add a commit
    let temp_dir =
        std::env::temp_dir().join(format!("repo_commits_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&temp_dir)
        .output()
        .ok();

    std::fs::write(temp_dir.join("test.txt"), "test").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&temp_dir)
        .output()
        .ok();
    std::process::Command::new("git")
        .args(["commit", "-m", "test"])
        .current_dir(&temp_dir)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .output()
        .ok();

    // Should return true for repo with commits
    assert!(git::repo_has_commits(&temp_dir));

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_create_initial_commit_on_empty_repo() {
    // Create a fresh git repo with no files
    let temp_dir =
        std::env::temp_dir().join(format!("init_commit_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&temp_dir)
        .output()
        .ok();

    // Should not have commits initially
    assert!(!git::repo_has_commits(&temp_dir));

    // Create initial commit
    let result = git::create_initial_commit(&temp_dir);
    assert!(result.is_ok(), "create_initial_commit failed: {:?}", result);

    // Should have commits now
    assert!(git::repo_has_commits(&temp_dir));

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_create_initial_commit_with_existing_files() {
    // Create a git repo with uncommitted files
    let temp_dir =
        std::env::temp_dir().join(format!("init_commit_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&temp_dir)
        .output()
        .ok();

    // Add some files
    std::fs::write(temp_dir.join("app.js"), "console.log('hello');").unwrap();
    std::fs::write(temp_dir.join("package.json"), "{}").unwrap();

    // Create initial commit (should include the files)
    let result = git::create_initial_commit(&temp_dir);
    assert!(result.is_ok());

    // Verify commit was created
    assert!(git::repo_has_commits(&temp_dir));

    std::fs::remove_dir_all(&temp_dir).ok();
}
