//! Diagnostic agent for analyzing worktree and git failures.
//!
//! When worktree creation fails (e.g., due to SSH auth issues or timeouts),
//! a diagnostic agent can be spawned to analyze the error and provide
//! helpful guidance to the user via a ticket comment.

use std::path::PathBuf;
use std::sync::Arc;
use tauri::AppHandle;

use crate::db::{AgentType, AuthorType, CreateComment, CreateRun, Database, RunStatus};
use super::worktree::{DiagnosticType, WorktreeError};
use super::spawner;
use super::{AgentKind, AgentRunConfig, extract_agent_text};

/// Context for diagnostic analysis
#[derive(Debug, Clone)]
pub struct DiagnosticContext {
    /// The repository path where the operation was attempted
    pub repo_path: PathBuf,
    /// Description of the operation that failed (e.g., "git fetch --all")
    pub operation: String,
    /// Classified type of the diagnostic issue
    pub error_type: DiagnosticType,
    /// Standard error output from the failed command
    pub stderr: String,
    /// Exit code if available
    pub exit_code: Option<i32>,
    /// Additional context (branch name, ticket title, etc.)
    pub additional_context: Option<String>,
}

/// Error type for diagnostic operations
#[derive(Debug, thiserror::Error)]
pub enum DiagnosticError {
    #[error("Failed to create diagnostic run: {0}")]
    RunCreationFailed(String),
    
    #[error("Failed to spawn diagnostic agent: {0}")]
    SpawnFailed(String),
    
    #[error("Database error: {0}")]
    DatabaseError(#[from] crate::db::DbError),
}

/// Build a diagnostic prompt for the agent
pub fn build_diagnostic_prompt(context: &DiagnosticContext) -> String {
    let error_type_str = match context.error_type {
        DiagnosticType::SshAuth => "SSH Authentication",
        DiagnosticType::Timeout => "Operation Timeout",
        DiagnosticType::Permission => "Permission Denied",
        DiagnosticType::NetworkError => "Network Error",
        DiagnosticType::GitError => "Git Error",
        DiagnosticType::UnbornBranch => "Unborn Branch (No Commits)",
        DiagnosticType::Unknown => "Unknown Error",
    };
    
    let mut prompt = format!(
        r#"# Diagnose Git Operation Failure

A git operation failed and needs troubleshooting. Analyze the error and provide helpful guidance.

## Error Context
- **Operation attempted:** {}
- **Error type:** {}
- **Exit code:** {}
- **Stderr output:**
```
{}
```
"#,
        context.operation,
        error_type_str,
        context.exit_code.map(|c| c.to_string()).unwrap_or_else(|| "N/A".to_string()),
        context.stderr,
    );
    
    if let Some(ref additional) = context.additional_context {
        prompt.push_str(&format!("\n## Additional Context\n{}\n", additional));
    }
    
    prompt.push_str(r#"
## Your Task

Write a helpful comment on this ticket explaining:
1. What the error means in plain language
2. Step-by-step instructions to resolve it
3. Any commands the user should run

## Guidelines

- Use markdown formatting for clarity
- Include copy-pasteable commands where helpful
- Be specific to macOS if relevant
- If this is an SSH issue, explain ssh-agent setup for persistent keys
- If unclear, suggest diagnostic commands to run

## SSH-Specific Guidance

If this is an SSH authentication failure:
1. Check if ssh-agent is running: `ssh-add -l`
2. If no identities, add key: `ssh-add ~/.ssh/id_ed25519` (or relevant key)
3. For passphrase-protected keys, consider adding to Keychain: `ssh-add --apple-use-keychain ~/.ssh/id_ed25519`
4. Test connection: `ssh -T git@github.com` (or appropriate host)

IMPORTANT: Write ONLY the comment text. Start your response directly with the troubleshooting content.
"#);
    
    prompt
}

/// Classify a WorktreeError for diagnostic purposes
pub fn classify_worktree_error(error: &WorktreeError) -> DiagnosticContext {
    let error_type = error.diagnostic_type();
    let operation = error.operation().unwrap_or("unknown operation").to_string();
    let stderr = error.stderr().unwrap_or("").to_string();
    let exit_code = error.exit_code();
    
    DiagnosticContext {
        repo_path: PathBuf::new(), // Will be filled in by caller
        operation,
        error_type,
        stderr,
        exit_code,
        additional_context: None,
    }
}

/// Run a diagnostic agent to analyze an error and write a helpful comment.
///
/// This spawns an agent (using the configured agent type) that:
/// 1. Analyzes the error context
/// 2. Produces troubleshooting guidance as output
/// 3. Posts the output as a comment on the ticket
///
/// The agent runs against the main repo (not a worktree) since worktree creation failed.
#[allow(clippy::too_many_arguments)]
pub async fn run_diagnostic_agent(
    db: Arc<Database>,
    _app_handle: Option<AppHandle>,
    ticket_id: &str,
    context: DiagnosticContext,
    api_url: &str,
    api_token: &str,
    model: Option<String>,
    agent_kind: AgentKind,
) -> Result<(), DiagnosticError> {
    let run_id = uuid::Uuid::new_v4().to_string();
    let ticket_id_owned = ticket_id.to_string();
    
    tracing::info!(
        "Starting diagnostic agent for ticket {}: error_type={:?}, operation={}",
        ticket_id,
        context.error_type,
        context.operation
    );
    
    // Create a diagnostic run in the database
    let db_agent_type = match agent_kind {
        AgentKind::Cursor => AgentType::Cursor,
        AgentKind::Claude => AgentType::Claude,
    };
    
    let run = db.create_run(&CreateRun {
        ticket_id: ticket_id.to_string(),
        agent_type: db_agent_type,
        repo_path: context.repo_path.to_string_lossy().to_string(),
        parent_run_id: None,
        stage: Some("diagnostic".to_string()),
    }).map_err(|e| DiagnosticError::RunCreationFailed(e.to_string()))?;
    
    // Update run to running status
    if let Err(e) = db.update_run_status(&run.id, RunStatus::Running, None, None) {
        tracing::warn!("Failed to update diagnostic run status: {}", e);
    }
    
    // Build the diagnostic prompt
    let prompt = build_diagnostic_prompt(&context);
    
    // Configure and run the agent using the ticket's model preference
    let agent_config = AgentRunConfig {
        kind: agent_kind,
        ticket_id: ticket_id.to_string(),
        run_id: run_id.clone(),
        repo_path: context.repo_path.clone(),
        prompt,
        timeout_secs: Some(300), // 5 minute timeout for diagnostics
        api_url: api_url.to_string(),
        api_token: api_token.to_string(),
        model,
        claude_api_config: None,
    };
    
    // Spawn the agent in a blocking task since spawner uses sync I/O
    let result = tokio::task::spawn_blocking(move || {
        spawner::run_agent(agent_config, None)
    }).await;
    
    match result {
        Ok(Ok(agent_result)) => {
            let exit_code = agent_result.exit_code;
            let status = if exit_code == Some(0) {
                RunStatus::Finished
            } else {
                RunStatus::Error
            };
            
            // Try to extract text from the agent's output
            let extracted_text = agent_result.captured_stdout
                .as_ref()
                .map(|output| extract_agent_text(output))
                .filter(|s| !s.is_empty());
            
            if let Err(e) = db.update_run_status(
                &run.id,
                status.clone(),
                exit_code,
                extracted_text.as_deref(),
            ) {
                tracing::warn!("Failed to update diagnostic run status: {}", e);
            }
            
            tracing::info!(
                "Diagnostic agent completed for ticket {}: exit_code={:?}, has_output={}",
                ticket_id_owned,
                exit_code,
                extracted_text.is_some()
            );
            
            // If we got text output from the agent, post it as a comment
            if let Some(ref comment_text) = extracted_text {
                if !comment_text.trim().is_empty() {
                    tracing::info!(
                        "Posting diagnostic comment for ticket {} ({} chars)",
                        ticket_id_owned,
                        comment_text.len()
                    );
                    
                    if let Err(e) = db.create_comment(&CreateComment {
                        ticket_id: ticket_id_owned.clone(),
                        author_type: AuthorType::System,
                        body_md: comment_text.clone(),
                        metadata: None,
                    }) {
                        tracing::error!(
                            "Failed to create diagnostic comment for ticket {}: {}",
                            ticket_id_owned, e
                        );
                        return Err(DiagnosticError::SpawnFailed(format!(
                            "Failed to post diagnostic comment: {}", e
                        )));
                    }
                    
                    return Ok(());
                }
            }
            
            // No output extracted - return error so fallback comment is used
            let error_msg = format!(
                "Diagnostic agent produced no usable output (exit_code={:?})",
                exit_code
            );
            tracing::warn!("{}", error_msg);
            Err(DiagnosticError::SpawnFailed(error_msg))
        }
        Ok(Err(spawn_error)) => {
            let error_msg = format!("Diagnostic agent spawn failed: {}", spawn_error);
            tracing::error!("{}", error_msg);
            
            if let Err(e) = db.update_run_status(
                &run.id,
                RunStatus::Error,
                None,
                Some(&error_msg),
            ) {
                tracing::warn!("Failed to update diagnostic run status: {}", e);
            }
            
            Err(DiagnosticError::SpawnFailed(error_msg))
        }
        Err(join_error) => {
            let error_msg = format!("Diagnostic agent task panicked: {}", join_error);
            tracing::error!("{}", error_msg);
            
            if let Err(e) = db.update_run_status(
                &run.id,
                RunStatus::Error,
                None,
                Some(&error_msg),
            ) {
                tracing::warn!("Failed to update diagnostic run status: {}", e);
            }
            
            Err(DiagnosticError::SpawnFailed(error_msg))
        }
    }
}

/// Create a fallback comment when the diagnostic agent cannot be spawned.
/// This provides basic troubleshooting guidance based on the error type.
pub fn create_fallback_diagnostic_comment(context: &DiagnosticContext) -> String {
    match context.error_type {
        DiagnosticType::SshAuth => {
            format!(
                r#"## SSH Authentication Failed

The agent couldn't access the git remote due to an SSH authentication issue.

**Error:** {}

### How to Fix

1. **Check if your SSH key is loaded:**
   ```bash
   ssh-add -l
   ```
   If you see "The agent has no identities", add your key:
   ```bash
   ssh-add ~/.ssh/id_ed25519  # or your key file
   ```

2. **For persistent keys (macOS):**
   ```bash
   ssh-add --apple-use-keychain ~/.ssh/id_ed25519
   ```

3. **Test your connection:**
   ```bash
   ssh -T git@github.com
   ```

4. **If using a passphrase-protected key:**
   Your key requires a passphrase that the agent cannot provide interactively.
   Use ssh-agent with keychain integration to cache the passphrase.

Once SSH is working, move this ticket back to Ready to retry."#,
                context.stderr.lines().next().unwrap_or("Unknown error")
            )
        }
        DiagnosticType::Timeout => {
            format!(
                r#"## Operation Timed Out

The operation `{}` took too long and was cancelled.

### Possible Causes
- Network connectivity issues
- Remote server is slow or unresponsive
- Large repository with slow fetch

### How to Fix
1. Check your network connection
2. Try the operation manually to see if it completes:
   ```bash
   {}
   ```
3. If the operation works manually, try again

Once resolved, move this ticket back to Ready to retry."#,
                context.operation,
                context.operation
            )
        }
        DiagnosticType::NetworkError => {
            format!(
                r#"## Network Error

Couldn't connect to the git remote.

**Error:** {}

### How to Fix
1. Check your internet connection
2. Verify the remote is accessible:
   ```bash
   git remote -v
   ping github.com  # or your git host
   ```

Once connectivity is restored, move this ticket back to Ready to retry."#,
                context.stderr.lines().next().unwrap_or("Network unreachable")
            )
        }
        DiagnosticType::UnbornBranch => {
            format!(
                "## Repository Has No Commits Yet\n\n\
The git worktree operation failed because your repository doesn't have any commits yet. \
Git needs at least one commit before it can create worktrees and branches.\n\n\
**Error:** {}\n\n\
### How to Fix\n\n\
Run these commands in your repository:\n\n\
```bash\n\
cd {}\n\n\
# Stage any existing files, or create a placeholder\n\
git add -A\n\n\
# If there are no files to commit, create a simple one:\n\
# echo \"# Project\" > README.md && git add README.md\n\n\
# Create the initial commit\n\
git commit -m \"Initial commit\"\n\
```\n\n\
After creating the initial commit, move this ticket back to Ready to retry.",
                context.stderr.lines().next().unwrap_or("No commits in repository"),
                context.repo_path.display()
            )
        }
        _ => {
            format!(
                r#"## Git Operation Failed

The operation `{}` failed with an error.

**Error output:**
```
{}
```

Please investigate the error and resolve it manually. Once fixed, move this ticket back to Ready to retry."#,
                context.operation,
                context.stderr
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_diagnostic_prompt_ssh() {
        let context = DiagnosticContext {
            repo_path: PathBuf::from("/tmp/repo"),
            operation: "git fetch --all".to_string(),
            error_type: DiagnosticType::SshAuth,
            stderr: "Permission denied (publickey)".to_string(),
            exit_code: Some(128),
            additional_context: None,
        };
        
        let prompt = build_diagnostic_prompt(&context);
        assert!(prompt.contains("SSH Authentication"));
        assert!(prompt.contains("git fetch --all"));
        assert!(prompt.contains("Permission denied"));
    }
    
    #[test]
    fn test_build_diagnostic_prompt_with_context() {
        let context = DiagnosticContext {
            repo_path: PathBuf::from("/tmp/repo"),
            operation: "git worktree add".to_string(),
            error_type: DiagnosticType::Timeout,
            stderr: "".to_string(),
            exit_code: None,
            additional_context: Some("Branch: feature/test, Ticket: Fix login bug".to_string()),
        };
        
        let prompt = build_diagnostic_prompt(&context);
        assert!(prompt.contains("Operation Timeout"));
        assert!(prompt.contains("Branch: feature/test"));
    }
    
    #[test]
    fn test_fallback_comment_ssh() {
        let context = DiagnosticContext {
            repo_path: PathBuf::from("/tmp/repo"),
            operation: "git fetch".to_string(),
            error_type: DiagnosticType::SshAuth,
            stderr: "Permission denied (publickey)".to_string(),
            exit_code: Some(128),
            additional_context: None,
        };
        
        let comment = create_fallback_diagnostic_comment(&context);
        assert!(comment.contains("SSH Authentication Failed"));
        assert!(comment.contains("ssh-add"));
    }
    
    #[test]
    fn test_fallback_comment_timeout() {
        let context = DiagnosticContext {
            repo_path: PathBuf::from("/tmp/repo"),
            operation: "git fetch --all".to_string(),
            error_type: DiagnosticType::Timeout,
            stderr: "".to_string(),
            exit_code: None,
            additional_context: None,
        };
        
        let comment = create_fallback_diagnostic_comment(&context);
        assert!(comment.contains("Timed Out"));
        assert!(comment.contains("git fetch --all"));
    }
    
    #[test]
    fn test_classify_ssh_error() {
        let error = WorktreeError::SshAuthFailed {
            message: "Auth failed".to_string(),
            stderr: "Permission denied".to_string(),
            exit_code: Some(128),
            operation: "git fetch".to_string(),
        };
        
        let context = classify_worktree_error(&error);
        assert_eq!(context.error_type, DiagnosticType::SshAuth);
        assert_eq!(context.operation, "git fetch");
        assert_eq!(context.stderr, "Permission denied");
    }
    
    #[test]
    fn test_classify_timeout_error() {
        let error = WorktreeError::Timeout {
            timeout_secs: 60,
            operation: "git clone".to_string(),
        };
        
        let context = classify_worktree_error(&error);
        assert_eq!(context.error_type, DiagnosticType::Timeout);
        assert_eq!(context.operation, "git clone");
    }
    
    #[test]
    fn test_classify_network_error() {
        let error = WorktreeError::NetworkError {
            message: "Connection refused".to_string(),
            stderr: "ssh: connect to host github.com port 22: Connection refused".to_string(),
            exit_code: Some(128),
            operation: "git fetch".to_string(),
        };
        
        let context = classify_worktree_error(&error);
        assert_eq!(context.error_type, DiagnosticType::NetworkError);
        assert_eq!(context.operation, "git fetch");
        assert!(context.stderr.contains("Connection refused"));
    }
    
    #[test]
    fn test_fallback_comment_network_error() {
        let context = DiagnosticContext {
            repo_path: PathBuf::from("/tmp/repo"),
            operation: "git fetch".to_string(),
            error_type: DiagnosticType::NetworkError,
            stderr: "ssh: connect to host github.com port 22: Connection refused".to_string(),
            exit_code: Some(128),
            additional_context: None,
        };
        
        let comment = create_fallback_diagnostic_comment(&context);
        assert!(comment.contains("Network Error"));
        assert!(comment.contains("internet connection"));
        // Should NOT contain SSH key troubleshooting
        assert!(!comment.contains("ssh-add"));
    }
    
    #[test]
    fn test_network_error_not_classified_as_ssh() {
        // This is the key test - network errors should get NetworkError type, not SshAuth
        let error = WorktreeError::NetworkError {
            message: "Connection timed out".to_string(),
            stderr: "Connection timed out".to_string(),
            exit_code: Some(128),
            operation: "git fetch --all".to_string(),
        };
        
        let context = classify_worktree_error(&error);
        // Should be NetworkError, NOT SshAuth
        assert_eq!(context.error_type, DiagnosticType::NetworkError);
        assert_ne!(context.error_type, DiagnosticType::SshAuth);
    }
    
    #[test]
    fn test_classify_git_error_extracts_details() {
        let error = WorktreeError::GitError {
            message: "Failed to create worktree".to_string(),
            stderr: "fatal: worktree 'path' is locked".to_string(),
            exit_code: Some(128),
            operation: "git worktree add /tmp/worktree branch".to_string(),
        };
        
        let context = classify_worktree_error(&error);
        assert_eq!(context.error_type, DiagnosticType::GitError);
        assert_eq!(context.operation, "git worktree add /tmp/worktree branch");
        assert_eq!(context.stderr, "fatal: worktree 'path' is locked");
        assert_eq!(context.exit_code, Some(128));
    }
    
    #[test]
    fn test_classify_git_error_with_permission_denied() {
        let error = WorktreeError::GitError {
            message: "Failed to create directory".to_string(),
            stderr: "error: Permission denied while creating /tmp/worktree".to_string(),
            exit_code: Some(1),
            operation: "git worktree add".to_string(),
        };
        
        let context = classify_worktree_error(&error);
        assert_eq!(context.error_type, DiagnosticType::Permission);
        assert_eq!(context.stderr, "error: Permission denied while creating /tmp/worktree");
        assert_eq!(context.exit_code, Some(1));
    }
    
    #[test]
    fn test_fallback_comment_unborn_branch() {
        let context = DiagnosticContext {
            repo_path: PathBuf::from("/Users/test/my-project"),
            operation: "git worktree add".to_string(),
            error_type: DiagnosticType::UnbornBranch,
            stderr: "fatal: invalid reference: main".to_string(),
            exit_code: Some(128),
            additional_context: None,
        };
        
        let comment = create_fallback_diagnostic_comment(&context);
        assert!(comment.contains("No Commits Yet"));
        assert!(comment.contains("Initial commit"));
        assert!(comment.contains("/Users/test/my-project"));
        // Should NOT contain SSH troubleshooting
        assert!(!comment.contains("ssh-add"));
    }
    
    #[test]
    fn test_build_diagnostic_prompt_unborn_branch() {
        let context = DiagnosticContext {
            repo_path: PathBuf::from("/tmp/repo"),
            operation: "git worktree add".to_string(),
            error_type: DiagnosticType::UnbornBranch,
            stderr: "fatal: invalid reference: main".to_string(),
            exit_code: Some(128),
            additional_context: None,
        };
        
        let prompt = build_diagnostic_prompt(&context);
        assert!(prompt.contains("Unborn Branch"));
        assert!(prompt.contains("git worktree add"));
        assert!(prompt.contains("invalid reference"));
    }
}
