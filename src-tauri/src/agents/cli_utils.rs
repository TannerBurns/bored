//! Shared CLI availability checking utilities.
//!
//! Both Claude and Cursor agents need to check if their CLI tool is installed
//! and get its version. This module provides generic functions parameterized
//! by the command name.

use std::process::Command;

/// Check whether a CLI tool is available on the system.
pub fn is_cli_available(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get the version string of a CLI tool, if available.
pub fn get_cli_version(cmd: &str) -> Option<String> {
    Command::new(cmd)
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_cli_available_returns_false_for_missing_command() {
        assert!(!is_cli_available("nonexistent-command-12345"));
    }

    #[test]
    fn get_cli_version_returns_none_for_missing_command() {
        let result = get_cli_version("nonexistent-command-12345");
        assert!(result.is_none());
    }
}
