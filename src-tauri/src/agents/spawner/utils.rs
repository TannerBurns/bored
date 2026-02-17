//! Utility functions for agent spawning.

use super::config::TRANSIENT_ERROR_PATTERNS;

/// Check if an error message indicates a transient error that should be retried
pub fn is_transient_error(output: &str) -> bool {
    let lower = output.to_lowercase();
    TRANSIENT_ERROR_PATTERNS
        .iter()
        .any(|pattern| lower.contains(&pattern.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
