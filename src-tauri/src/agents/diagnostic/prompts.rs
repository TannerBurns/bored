//! Diagnostic prompt generation.

use super::context::DiagnosticContext;
use crate::agents::worktree::DiagnosticType;

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
        context
            .exit_code
            .map(|c| c.to_string())
            .unwrap_or_else(|| "N/A".to_string()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
