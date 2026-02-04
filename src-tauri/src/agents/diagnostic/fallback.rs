//! Fallback diagnostic comment generation.

use super::context::DiagnosticContext;
use crate::agents::worktree::DiagnosticType;

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
                context.operation, context.operation
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
                context
                    .stderr
                    .lines()
                    .next()
                    .unwrap_or("Network unreachable")
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
                context
                    .stderr
                    .lines()
                    .next()
                    .unwrap_or("No commits in repository"),
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
                context.operation, context.stderr
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
}
