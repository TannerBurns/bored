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
    assert!(base.to_string_lossy().contains("bored"));
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
        "fatal: 'feature/test' is already checked out at '/tmp/bored/worktrees/abc123'";
    let result = git::extract_worktree_path_from_error(stderr);
    assert_eq!(
        result,
        Some("/tmp/bored/worktrees/abc123".to_string())
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
fn test_is_our_worktree_with_bored_path() {
    // Should detect paths in our temp directory
    assert!(manage::is_our_worktree("/tmp/bored/worktrees/abc123"));
    assert!(manage::is_our_worktree(
        "/private/var/folders/89/xmt0wws/T/bored/worktrees/62e286f9"
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
    let stderr = "fatal: 'fix/cff1ae76/remove-empty-categories-summary' is already used by worktree at '/private/var/folders/89/xmt0wws13ksdtn4_wm0g1_p40000gn/T/bored/worktrees/ccbc02ff-6c66-45fc-8b83-330bcb4f5f98'";
    let result = git::extract_worktree_path_from_error(stderr);
    assert_eq!(result, Some("/private/var/folders/89/xmt0wws13ksdtn4_wm0g1_p40000gn/T/bored/worktrees/ccbc02ff-6c66-45fc-8b83-330bcb4f5f98".to_string()));
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

// --- safety_commit_if_needed tests ---

#[test]
fn test_safety_commit_clean_worktree_returns_none() {
    let temp_dir =
        std::env::temp_dir().join(format!("safety_commit_clean_{}", uuid::Uuid::new_v4()));
    init_repo_with_commit(&temp_dir);

    let result = manage::safety_commit_if_needed(&temp_dir, "run-123");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), None);

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_safety_commit_dirty_worktree_returns_hash() {
    let temp_dir =
        std::env::temp_dir().join(format!("safety_commit_dirty_{}", uuid::Uuid::new_v4()));
    init_repo_with_commit(&temp_dir);

    std::fs::write(temp_dir.join("new_file.txt"), "uncommitted work").unwrap();

    let result = manage::safety_commit_if_needed(&temp_dir, "run-456");
    assert!(result.is_ok());
    let hash = result.unwrap();
    assert!(hash.is_some(), "Expected a commit hash, got None");
    assert!(!hash.as_ref().unwrap().is_empty());

    // Worktree should now be clean
    let status = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&status.stdout).trim().is_empty(),
        "Worktree should be clean after safety commit"
    );

    // Verify the commit message contains the run ID
    let log = std::process::Command::new("git")
        .args(["log", "-1", "--format=%s"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    let message = String::from_utf8_lossy(&log.stdout);
    assert!(
        message.contains("run-456"),
        "Commit message should contain run ID, got: {}",
        message.trim()
    );

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_safety_commit_staged_changes_returns_hash() {
    let temp_dir =
        std::env::temp_dir().join(format!("safety_commit_staged_{}", uuid::Uuid::new_v4()));
    init_repo_with_commit(&temp_dir);

    std::fs::write(temp_dir.join("staged.txt"), "staged content").unwrap();
    std::process::Command::new("git")
        .args(["add", "staged.txt"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();

    let result = manage::safety_commit_if_needed(&temp_dir, "run-789");
    assert!(result.is_ok());
    assert!(result.unwrap().is_some());

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_safety_commit_nonexistent_path_returns_none() {
    let path = std::env::temp_dir().join(format!("nonexistent_{}", uuid::Uuid::new_v4()));
    let result = manage::safety_commit_if_needed(&path, "run-000");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), None);
}

#[test]
fn test_safety_commit_modified_file_returns_hash() {
    let temp_dir =
        std::env::temp_dir().join(format!("safety_commit_modified_{}", uuid::Uuid::new_v4()));
    init_repo_with_commit(&temp_dir);

    // Modify an existing tracked file
    std::fs::write(temp_dir.join("README.md"), "modified content").unwrap();

    let result = manage::safety_commit_if_needed(&temp_dir, "run-mod");
    assert!(result.is_ok());
    assert!(result.unwrap().is_some());

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_safety_commit_deleted_file_returns_hash() {
    let temp_dir =
        std::env::temp_dir().join(format!("safety_commit_deleted_{}", uuid::Uuid::new_v4()));
    init_repo_with_commit(&temp_dir);

    std::fs::remove_file(temp_dir.join("README.md")).unwrap();

    let result = manage::safety_commit_if_needed(&temp_dir, "run-del");
    assert!(result.is_ok());
    assert!(result.unwrap().is_some());

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_safety_commit_message_format() {
    let temp_dir =
        std::env::temp_dir().join(format!("safety_commit_msg_{}", uuid::Uuid::new_v4()));
    init_repo_with_commit(&temp_dir);

    std::fs::write(temp_dir.join("change.txt"), "content").unwrap();

    let result = manage::safety_commit_if_needed(&temp_dir, "abc-123-def");
    assert!(result.is_ok());
    assert!(result.unwrap().is_some());

    let log = std::process::Command::new("git")
        .args(["log", "-1", "--format=%s"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    let message = String::from_utf8_lossy(&log.stdout).trim().to_string();
    assert_eq!(
        message,
        "bored: auto-save uncommitted changes from run abc-123-def"
    );

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_safety_commit_idempotent_second_call_returns_none() {
    let temp_dir =
        std::env::temp_dir().join(format!("safety_commit_idempotent_{}", uuid::Uuid::new_v4()));
    init_repo_with_commit(&temp_dir);

    std::fs::write(temp_dir.join("file.txt"), "content").unwrap();

    let first = manage::safety_commit_if_needed(&temp_dir, "run-first");
    assert!(first.is_ok());
    assert!(first.unwrap().is_some());

    let second = manage::safety_commit_if_needed(&temp_dir, "run-second");
    assert!(second.is_ok());
    assert_eq!(second.unwrap(), None, "Second call on clean worktree should return None");

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_safety_commit_mixed_changes() {
    let temp_dir =
        std::env::temp_dir().join(format!("safety_commit_mixed_{}", uuid::Uuid::new_v4()));
    init_repo_with_commit(&temp_dir);

    // Create a tracked file, commit it, then set up mixed state
    std::fs::write(temp_dir.join("tracked.txt"), "original").unwrap();
    std::process::Command::new("git")
        .args(["add", "tracked.txt"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "add tracked"])
        .current_dir(&temp_dir)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .output()
        .unwrap();

    // Now create mixed state: new file + modified file + deleted file
    std::fs::write(temp_dir.join("new_file.txt"), "new").unwrap();
    std::fs::write(temp_dir.join("tracked.txt"), "modified").unwrap();
    std::fs::remove_file(temp_dir.join("README.md")).unwrap();

    let result = manage::safety_commit_if_needed(&temp_dir, "run-mixed");
    assert!(result.is_ok());
    assert!(result.unwrap().is_some());

    // Worktree should be clean after
    let status = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&status.stdout).trim().is_empty(),
        "Worktree should be clean after safety commit with mixed changes"
    );

    std::fs::remove_dir_all(&temp_dir).ok();
}

// --- resolve_remote_default_branch tests ---

/// Helper: initialize a git repo at `path` with one commit.
fn init_repo_with_commit(path: &std::path::Path) {
    std::fs::create_dir_all(path).unwrap();
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(path)
        .output()
        .unwrap();
    std::fs::write(path.join("README.md"), "test").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(path)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .output()
        .unwrap();
}

#[test]
fn test_resolve_remote_default_branch_no_remote() {
    let temp_dir =
        std::env::temp_dir().join(format!("resolve_no_remote_{}", uuid::Uuid::new_v4()));
    init_repo_with_commit(&temp_dir);

    let result = git::resolve_remote_default_branch(&temp_dir);
    assert_eq!(result, None);

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_resolve_remote_default_branch_nonexistent_path() {
    let path = std::env::temp_dir().join(format!("nonexistent_{}", uuid::Uuid::new_v4()));
    let result = git::resolve_remote_default_branch(&path);
    assert_eq!(result, None);
}

#[test]
fn test_resolve_remote_default_branch_falls_back_to_rev_parse() {
    let base =
        std::env::temp_dir().join(format!("resolve_revparse_{}", uuid::Uuid::new_v4()));
    let remote_dir = base.join("remote");
    let local_dir = base.join("local");

    // Create "remote" repo with a commit
    init_repo_with_commit(&remote_dir);

    // Create local repo, add remote, fetch (no symbolic-ref is set by fetch)
    std::fs::create_dir_all(&local_dir).unwrap();
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&local_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["remote", "add", "origin", remote_dir.to_str().unwrap()])
        .current_dir(&local_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["fetch", "origin"])
        .current_dir(&local_dir)
        .output()
        .unwrap();

    let result = git::resolve_remote_default_branch(&local_dir);
    assert!(result.is_some(), "Expected Some, got None");
    let branch = result.unwrap();
    assert!(
        branch == "origin/main" || branch == "origin/master",
        "Expected origin/main or origin/master, got {}",
        branch
    );

    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_resolve_remote_default_branch_via_symbolic_ref() {
    let base =
        std::env::temp_dir().join(format!("resolve_symref_{}", uuid::Uuid::new_v4()));
    let remote_dir = base.join("remote");
    let local_dir = base.join("local");

    init_repo_with_commit(&remote_dir);

    std::fs::create_dir_all(&local_dir).unwrap();
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&local_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["remote", "add", "origin", remote_dir.to_str().unwrap()])
        .current_dir(&local_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["fetch", "origin"])
        .current_dir(&local_dir)
        .output()
        .unwrap();

    // Discover what remote branch was fetched
    let branch_output = std::process::Command::new("git")
        .args(["branch", "-r", "--format=%(refname:short)"])
        .current_dir(&local_dir)
        .output()
        .unwrap();
    let remote_branch = String::from_utf8_lossy(&branch_output.stdout)
        .lines()
        .find(|l| l.starts_with("origin/") && !l.contains("HEAD"))
        .unwrap()
        .to_string();

    // Manually set symbolic-ref (git fetch does not set this; git clone does)
    std::process::Command::new("git")
        .args([
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            &format!("refs/remotes/{}", remote_branch),
        ])
        .current_dir(&local_dir)
        .output()
        .unwrap();

    let result = git::resolve_remote_default_branch(&local_dir);
    assert_eq!(result, Some(remote_branch));

    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_resolve_remote_default_branch_custom_branch_via_symbolic_ref() {
    let base =
        std::env::temp_dir().join(format!("resolve_custom_{}", uuid::Uuid::new_v4()));
    let remote_dir = base.join("remote");
    let local_dir = base.join("local");

    // Create remote with a "develop" branch
    std::fs::create_dir_all(&remote_dir).unwrap();
    std::process::Command::new("git")
        .args(["init", "-b", "develop"])
        .current_dir(&remote_dir)
        .output()
        .unwrap();
    std::fs::write(remote_dir.join("README.md"), "test").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&remote_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&remote_dir)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .output()
        .unwrap();

    // Create local repo, add remote, fetch
    std::fs::create_dir_all(&local_dir).unwrap();
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&local_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["remote", "add", "origin", remote_dir.to_str().unwrap()])
        .current_dir(&local_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["fetch", "origin"])
        .current_dir(&local_dir)
        .output()
        .unwrap();

    // Set symbolic-ref to origin/develop
    std::process::Command::new("git")
        .args([
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/develop",
        ])
        .current_dir(&local_dir)
        .output()
        .unwrap();

    // Should resolve via symbolic-ref to origin/develop (not main/master)
    let result = git::resolve_remote_default_branch(&local_dir);
    assert_eq!(result, Some("origin/develop".to_string()));

    // Without symbolic-ref, rev-parse fallback would NOT find origin/main or origin/master
    // since the remote only has "develop". Verify by removing symbolic-ref.
    std::process::Command::new("git")
        .args(["symbolic-ref", "--delete", "refs/remotes/origin/HEAD"])
        .current_dir(&local_dir)
        .output()
        .unwrap();

    let fallback_result = git::resolve_remote_default_branch(&local_dir);
    assert_eq!(
        fallback_result, None,
        "Should return None when only non-standard branch exists and no symbolic-ref"
    );

    std::fs::remove_dir_all(&base).ok();
}

// --- merge_detour_into_target tests ---

/// Helper: create a git commit with a given message and file content.
fn commit_file(repo: &std::path::Path, filename: &str, content: &str, message: &str) {
    std::fs::write(repo.join(filename), content).unwrap();
    std::process::Command::new("git")
        .args(["add", filename])
        .current_dir(repo)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(repo)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .output()
        .unwrap();
}

/// Helper: get the HEAD commit hash of a branch.
fn branch_head(repo: &std::path::Path, branch: &str) -> String {
    let output = std::process::Command::new("git")
        .args(["rev-parse", &format!("refs/heads/{}", branch)])
        .current_dir(repo)
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn test_merge_detour_nothing_to_merge_same_commit() {
    let temp_dir =
        std::env::temp_dir().join(format!("detour_nothing_{}", uuid::Uuid::new_v4()));
    init_repo_with_commit(&temp_dir);

    // Create a detour branch at the same commit as main
    std::process::Command::new("git")
        .args(["branch", "agent-detour/abc12345"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();

    let fork_point = branch_head(&temp_dir, "main");
    let result = manage::merge_detour_into_target(
        &temp_dir,
        "agent-detour/abc12345",
        "main",
        &fork_point,
    );

    assert!(result.is_ok());
    match result.unwrap() {
        manage::DetourMergeResult::NothingToMerge => {}
        other => panic!("Expected NothingToMerge, got {:?}", other),
    }

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_merge_detour_fast_forward_success() {
    let temp_dir =
        std::env::temp_dir().join(format!("detour_ff_{}", uuid::Uuid::new_v4()));
    init_repo_with_commit(&temp_dir);

    let fork_point = branch_head(&temp_dir, "main");

    // Create a detour branch and add a commit to it
    std::process::Command::new("git")
        .args(["checkout", "-b", "agent-detour/abc12345"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();

    commit_file(&temp_dir, "detour_work.txt", "agent work", "agent commit");
    let detour_head = branch_head(&temp_dir, "agent-detour/abc12345");

    // Switch back to main
    std::process::Command::new("git")
        .args(["checkout", "main"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();

    // Verify main is still at fork point
    assert_eq!(branch_head(&temp_dir, "main"), fork_point);

    let result = manage::merge_detour_into_target(
        &temp_dir,
        "agent-detour/abc12345",
        "main",
        &fork_point,
    );

    assert!(result.is_ok());
    match result.unwrap() {
        manage::DetourMergeResult::Merged { ref new_head } => {
            assert_eq!(new_head, &detour_head);
        }
        other => panic!("Expected Merged, got {:?}", other),
    }

    // Verify main now points to the detour's commit
    assert_eq!(branch_head(&temp_dir, "main"), detour_head);

    // Branch should still exist (merge_detour_into_target doesn't delete it)
    let branch_check = std::process::Command::new("git")
        .args(["rev-parse", "--verify", "refs/heads/agent-detour/abc12345"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    assert!(
        branch_check.status.success(),
        "Detour branch should still exist (deleted separately after worktree removal)"
    );

    // Now test delete_branch separately
    assert!(manage::delete_branch(&temp_dir, "agent-detour/abc12345"));

    // Verify it's gone
    let branch_check2 = std::process::Command::new("git")
        .args(["rev-parse", "--verify", "refs/heads/agent-detour/abc12345"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    assert!(
        !branch_check2.status.success(),
        "Detour branch should be deleted after delete_branch"
    );

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_merge_detour_diverged_returns_diverged() {
    let temp_dir =
        std::env::temp_dir().join(format!("detour_diverged_{}", uuid::Uuid::new_v4()));
    init_repo_with_commit(&temp_dir);

    let fork_point = branch_head(&temp_dir, "main");

    // Create detour branch and add a commit
    std::process::Command::new("git")
        .args(["checkout", "-b", "agent-detour/abc12345"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    commit_file(&temp_dir, "detour_work.txt", "agent work", "detour commit");

    // Switch back to main and add a DIFFERENT commit (causes divergence)
    std::process::Command::new("git")
        .args(["checkout", "main"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    commit_file(&temp_dir, "user_work.txt", "user work", "user commit");

    let main_head = branch_head(&temp_dir, "main");
    assert_ne!(main_head, fork_point, "main should have moved");

    let result = manage::merge_detour_into_target(
        &temp_dir,
        "agent-detour/abc12345",
        "main",
        &fork_point,
    );

    assert!(result.is_ok());
    match result.unwrap() {
        manage::DetourMergeResult::Diverged { ref current_head } => {
            assert_eq!(current_head, &main_head);
        }
        other => panic!("Expected Diverged, got {:?}", other),
    }

    // Main should NOT have moved
    assert_eq!(branch_head(&temp_dir, "main"), main_head);

    // Detour branch should still exist
    let branch_check = std::process::Command::new("git")
        .args(["rev-parse", "--verify", "refs/heads/agent-detour/abc12345"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    assert!(
        branch_check.status.success(),
        "Detour branch should be preserved when diverged"
    );

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_merge_detour_error_nonexistent_target() {
    let temp_dir =
        std::env::temp_dir().join(format!("detour_no_target_{}", uuid::Uuid::new_v4()));
    init_repo_with_commit(&temp_dir);

    std::process::Command::new("git")
        .args(["branch", "agent-detour/abc12345"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();

    let result = manage::merge_detour_into_target(
        &temp_dir,
        "agent-detour/abc12345",
        "nonexistent-branch",
        "fake-fork-point",
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_msg = format!("{}", err);
    assert!(
        err_msg.contains("nonexistent-branch"),
        "Error should mention the missing branch: {}",
        err_msg
    );

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_merge_detour_error_nonexistent_detour() {
    let temp_dir =
        std::env::temp_dir().join(format!("detour_no_detour_{}", uuid::Uuid::new_v4()));
    init_repo_with_commit(&temp_dir);

    let result = manage::merge_detour_into_target(
        &temp_dir,
        "nonexistent-detour",
        "main",
        "fake-fork-point",
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_msg = format!("{}", err);
    assert!(
        err_msg.contains("nonexistent-detour"),
        "Error should mention the missing detour branch: {}",
        err_msg
    );

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_merge_detour_after_agent_sync_merge() {
    let temp_dir =
        std::env::temp_dir().join(format!("detour_synced_{}", uuid::Uuid::new_v4()));
    init_repo_with_commit(&temp_dir);

    let fork_point = branch_head(&temp_dir, "main");

    // Create detour branch and add a commit
    std::process::Command::new("git")
        .args(["checkout", "-b", "agent-detour/abc12345"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    commit_file(&temp_dir, "detour_work.txt", "agent work", "detour commit");

    // Switch to main and add a commit (causes divergence)
    std::process::Command::new("git")
        .args(["checkout", "main"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    commit_file(&temp_dir, "user_work.txt", "user work", "user commit");

    // Simulate agent's detour-sync: merge main into detour
    std::process::Command::new("git")
        .args(["checkout", "agent-detour/abc12345"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    let merge_output = std::process::Command::new("git")
        .args(["merge", "main", "-m", "merge main into detour"])
        .current_dir(&temp_dir)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .output()
        .unwrap();
    assert!(merge_output.status.success(), "Merge should succeed");

    let detour_head = branch_head(&temp_dir, "agent-detour/abc12345");

    // Switch back so we can do the merge-back
    std::process::Command::new("git")
        .args(["checkout", "main"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();

    // Now merge_detour_into_target should succeed because main is an ancestor
    // of the merge commit on the detour
    let result = manage::merge_detour_into_target(
        &temp_dir,
        "agent-detour/abc12345",
        "main",
        &fork_point,
    );

    assert!(result.is_ok());
    match result.unwrap() {
        manage::DetourMergeResult::Merged { ref new_head } => {
            assert_eq!(new_head, &detour_head);
        }
        other => panic!("Expected Merged after agent sync, got {:?}", other),
    }

    // Main should now be at the merge commit
    assert_eq!(branch_head(&temp_dir, "main"), detour_head);

    std::fs::remove_dir_all(&temp_dir).ok();
}

// --- create_detour_worktree tests ---

#[test]
fn test_create_detour_worktree_sets_target_and_fork_point() {
    let temp_dir =
        std::env::temp_dir().join(format!("detour_create_{}", uuid::Uuid::new_v4()));
    init_repo_with_commit(&temp_dir);

    let main_head = branch_head(&temp_dir, "main");

    let worktree_base =
        std::env::temp_dir().join(format!("detour_wt_base_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&worktree_base).unwrap();

    let run_id = "abcdef12-3456-7890-abcd-ef1234567890";
    let config = create::WorktreeConfig {
        repo_path: temp_dir.clone(),
        branch_name: "agent-detour/abcdef12".to_string(),
        run_id: run_id.to_string(),
        base_dir: Some(worktree_base.clone()),
        base_branch: Some("main".to_string()),
    };

    let result = create::create_worktree(&config);
    assert!(result.is_ok(), "create_worktree should succeed: {:?}", result.err());

    let mut info = result.unwrap();
    // Simulate what create_detour_worktree does after calling create_worktree
    info.target_branch = Some("main".to_string());
    info.detour_fork_point = Some(main_head.clone());

    assert_eq!(info.target_branch.as_deref(), Some("main"));
    assert_eq!(info.detour_fork_point.as_deref(), Some(main_head.as_str()));
    assert_eq!(info.branch_name, "agent-detour/abcdef12");
    assert!(info.path.starts_with(&worktree_base));

    // Clean up
    let _ = manage::remove_worktree(&info.path, &temp_dir);
    std::fs::remove_dir_all(&temp_dir).ok();
    std::fs::remove_dir_all(&worktree_base).ok();
}

#[test]
fn test_worktree_info_defaults_none_for_non_detour() {
    let temp_dir =
        std::env::temp_dir().join(format!("wt_defaults_{}", uuid::Uuid::new_v4()));
    init_repo_with_commit(&temp_dir);

    let worktree_base =
        std::env::temp_dir().join(format!("wt_defaults_base_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&worktree_base).unwrap();

    let config = create::WorktreeConfig {
        repo_path: temp_dir.clone(),
        branch_name: "normal-branch".to_string(),
        run_id: "run-id-12345678".to_string(),
        base_dir: Some(worktree_base.clone()),
        base_branch: None,
    };

    let result = create::create_worktree(&config);
    assert!(result.is_ok());

    let info = result.unwrap();
    assert_eq!(info.target_branch, None);
    assert_eq!(info.detour_fork_point, None);

    let _ = manage::remove_worktree(&info.path, &temp_dir);
    std::fs::remove_dir_all(&temp_dir).ok();
    std::fs::remove_dir_all(&worktree_base).ok();
}

#[test]
fn test_merge_detour_multiple_commits_fast_forward() {
    let temp_dir =
        std::env::temp_dir().join(format!("detour_multi_{}", uuid::Uuid::new_v4()));
    init_repo_with_commit(&temp_dir);

    let fork_point = branch_head(&temp_dir, "main");

    // Create detour and add multiple commits
    std::process::Command::new("git")
        .args(["checkout", "-b", "agent-detour/multi"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    commit_file(&temp_dir, "file1.txt", "work 1", "commit 1");
    commit_file(&temp_dir, "file2.txt", "work 2", "commit 2");
    commit_file(&temp_dir, "file3.txt", "work 3", "commit 3");

    let detour_head = branch_head(&temp_dir, "agent-detour/multi");

    std::process::Command::new("git")
        .args(["checkout", "main"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();

    let result = manage::merge_detour_into_target(
        &temp_dir,
        "agent-detour/multi",
        "main",
        &fork_point,
    );

    assert!(result.is_ok());
    match result.unwrap() {
        manage::DetourMergeResult::Merged { ref new_head } => {
            assert_eq!(new_head, &detour_head);
        }
        other => panic!("Expected Merged, got {:?}", other),
    }

    // All three files should be accessible on main now
    assert_eq!(branch_head(&temp_dir, "main"), detour_head);

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_delete_branch_nonexistent_returns_false() {
    let temp_dir =
        std::env::temp_dir().join(format!("delete_noexist_{}", uuid::Uuid::new_v4()));
    init_repo_with_commit(&temp_dir);

    assert!(!manage::delete_branch(&temp_dir, "nonexistent-branch"));

    std::fs::remove_dir_all(&temp_dir).ok();
}
