//! Utility functions for agent spawning.

use super::super::{AgentKind, AgentRunConfig};
use super::config::TRANSIENT_ERROR_PATTERNS;

/// Check if an error message indicates a transient error that should be retried
pub fn is_transient_error(output: &str) -> bool {
    let lower = output.to_lowercase();
    TRANSIENT_ERROR_PATTERNS
        .iter()
        .any(|pattern| lower.contains(&pattern.to_lowercase()))
}

/// Build environment variables for the agent process
pub fn build_env_vars(config: &AgentRunConfig) -> Vec<(String, String)> {
    let mut env_vars = vec![
        (
            "AGENT_KANBAN_TICKET_ID".to_string(),
            config.ticket_id.clone(),
        ),
        ("AGENT_KANBAN_RUN_ID".to_string(), config.run_id.clone()),
        ("AGENT_KANBAN_API_URL".to_string(), config.api_url.clone()),
        (
            "AGENT_KANBAN_API_TOKEN".to_string(),
            config.api_token.clone(),
        ),
        (
            "AGENT_KANBAN_REPO_PATH".to_string(),
            config.repo_path.to_string_lossy().to_string(),
        ),
    ];

    if let (AgentKind::Claude, Some(ref c)) = (config.kind, &config.claude_api_config) {
        if let Some(v) = c.auth_token.as_ref().filter(|s| !s.is_empty()) {
            env_vars.push(("ANTHROPIC_AUTH_TOKEN".to_string(), v.clone()));
        }
        if let Some(v) = c.api_key.as_ref().filter(|s| !s.is_empty()) {
            env_vars.push(("ANTHROPIC_API_KEY".to_string(), v.clone()));
        }
        if let Some(v) = c.base_url.as_ref().filter(|s| !s.is_empty()) {
            env_vars.push(("ANTHROPIC_BASE_URL".to_string(), v.clone()));
        }
    }

    env_vars
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn build_env_vars_includes_all_fields() {
        let config = AgentRunConfig {
            kind: AgentKind::Cursor,
            ticket_id: "ticket-123".to_string(),
            run_id: "run-456".to_string(),
            repo_path: PathBuf::from("/tmp/repo"),
            prompt: "test prompt".to_string(),
            timeout_secs: Some(300),
            api_url: "http://localhost:7432".to_string(),
            api_token: "test-token".to_string(),
            model: None,
            claude_api_config: None,
            agent_config: std::collections::HashMap::new(),
        };

        let env_vars = build_env_vars(&config);

        assert!(env_vars
            .iter()
            .any(|(k, v)| k == "AGENT_KANBAN_TICKET_ID" && v == "ticket-123"));
        assert!(env_vars
            .iter()
            .any(|(k, v)| k == "AGENT_KANBAN_RUN_ID" && v == "run-456"));
        assert!(env_vars
            .iter()
            .any(|(k, v)| k == "AGENT_KANBAN_API_URL" && v == "http://localhost:7432"));
        assert!(env_vars
            .iter()
            .any(|(k, v)| k == "AGENT_KANBAN_API_TOKEN" && v == "test-token"));
        assert!(env_vars
            .iter()
            .any(|(k, v)| k == "AGENT_KANBAN_REPO_PATH" && v == "/tmp/repo"));
    }

    #[test]
    fn env_vars_count() {
        let config = AgentRunConfig {
            kind: AgentKind::Claude,
            ticket_id: "t".to_string(),
            run_id: "r".to_string(),
            repo_path: PathBuf::from("/"),
            prompt: "p".to_string(),
            timeout_secs: None,
            api_url: "http://x".to_string(),
            api_token: "tok".to_string(),
            model: None,
            claude_api_config: None,
            agent_config: std::collections::HashMap::new(),
        };
        let env_vars = build_env_vars(&config);
        assert_eq!(env_vars.len(), 5);
    }

    #[test]
    fn build_env_vars_includes_claude_api_config() {
        use crate::agents::ClaudeApiConfig;

        let config = AgentRunConfig {
            kind: AgentKind::Claude,
            ticket_id: "t".to_string(),
            run_id: "r".to_string(),
            repo_path: PathBuf::from("/"),
            prompt: "p".to_string(),
            timeout_secs: None,
            api_url: "http://x".to_string(),
            api_token: "tok".to_string(),
            model: None,
            claude_api_config: Some(ClaudeApiConfig {
                auth_token: Some("my-auth-token".to_string()),
                api_key: Some("my-api-key".to_string()),
                base_url: Some("https://custom.api.com".to_string()),
                model_override: Some("claude-opus-4-6".to_string()),
                ..Default::default()
            }),
            agent_config: std::collections::HashMap::new(),
        };
        let env_vars = build_env_vars(&config);

        // Base 5 + 3 Claude API vars (auth_token, api_key, base_url)
        // model_override is not set as env var, it's used in build_command
        assert_eq!(env_vars.len(), 8);

        assert!(env_vars
            .iter()
            .any(|(k, v)| k == "ANTHROPIC_AUTH_TOKEN" && v == "my-auth-token"));
        assert!(env_vars
            .iter()
            .any(|(k, v)| k == "ANTHROPIC_API_KEY" && v == "my-api-key"));
        assert!(env_vars
            .iter()
            .any(|(k, v)| k == "ANTHROPIC_BASE_URL" && v == "https://custom.api.com"));
    }

    #[test]
    fn build_env_vars_skips_empty_claude_values() {
        use crate::agents::ClaudeApiConfig;

        let config = AgentRunConfig {
            kind: AgentKind::Claude,
            ticket_id: "t".to_string(),
            run_id: "r".to_string(),
            repo_path: PathBuf::from("/"),
            prompt: "p".to_string(),
            timeout_secs: None,
            api_url: "http://x".to_string(),
            api_token: "tok".to_string(),
            model: None,
            claude_api_config: Some(ClaudeApiConfig {
                auth_token: Some("".to_string()), // Empty string should be skipped
                api_key: Some("key".to_string()),
                base_url: None,
                model_override: None,
                ..Default::default()
            }),
            agent_config: std::collections::HashMap::new(),
        };
        let env_vars = build_env_vars(&config);

        // Base 5 + only 1 Claude var (api_key)
        assert_eq!(env_vars.len(), 6);
        assert!(!env_vars.iter().any(|(k, _)| k == "ANTHROPIC_AUTH_TOKEN"));
        assert!(env_vars
            .iter()
            .any(|(k, v)| k == "ANTHROPIC_API_KEY" && v == "key"));
    }

    #[test]
    fn build_env_vars_cursor_ignores_claude_config() {
        use crate::agents::ClaudeApiConfig;

        let config = AgentRunConfig {
            kind: AgentKind::Cursor, // Not Claude
            ticket_id: "t".to_string(),
            run_id: "r".to_string(),
            repo_path: PathBuf::from("/"),
            prompt: "p".to_string(),
            timeout_secs: None,
            api_url: "http://x".to_string(),
            api_token: "tok".to_string(),
            model: None,
            claude_api_config: Some(ClaudeApiConfig {
                auth_token: Some("token".to_string()),
                api_key: Some("key".to_string()),
                base_url: Some("url".to_string()),
                model_override: Some("model".to_string()),
                ..Default::default()
            }),
            agent_config: std::collections::HashMap::new(),
        };
        let env_vars = build_env_vars(&config);

        // Should only have base 5 vars, Claude config ignored for Cursor
        assert_eq!(env_vars.len(), 5);
    }

    #[test]
    fn is_transient_error_detects_connection_stalled() {
        assert!(is_transient_error("C: Connection stalled"));
        assert!(is_transient_error(
            "Error: connection stalled during request"
        ));
    }

    #[test]
    fn is_transient_error_detects_connection_reset() {
        assert!(is_transient_error("connection reset by peer"));
        assert!(is_transient_error("ECONNRESET"));
    }

    #[test]
    fn is_transient_error_detects_rate_limit() {
        assert!(is_transient_error("rate limit exceeded"));
        assert!(is_transient_error("rate_limit_error"));
        assert!(is_transient_error("too many requests"));
    }

    #[test]
    fn is_transient_error_detects_http_errors() {
        assert!(is_transient_error("HTTP 502 Bad Gateway"));
        assert!(is_transient_error("503 Service Unavailable"));
        assert!(is_transient_error("504 Gateway Timeout"));
    }

    #[test]
    fn is_transient_error_detects_network_errors() {
        assert!(is_transient_error("ETIMEDOUT"));
        assert!(is_transient_error("ENOTFOUND"));
        assert!(is_transient_error("socket hang up"));
        assert!(is_transient_error("connection timed out"));
    }

    #[test]
    fn is_transient_error_case_insensitive() {
        assert!(is_transient_error("CONNECTION STALLED"));
        assert!(is_transient_error("Rate Limit"));
        assert!(is_transient_error("Service Unavailable"));
    }

    #[test]
    fn is_transient_error_returns_false_for_other_errors() {
        assert!(!is_transient_error("File not found"));
        assert!(!is_transient_error("Permission denied"));
        assert!(!is_transient_error("Syntax error in code"));
        assert!(!is_transient_error("Invalid argument"));
    }

    #[test]
    fn is_transient_error_empty_string() {
        assert!(!is_transient_error(""));
    }
}
