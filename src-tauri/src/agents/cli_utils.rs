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

/// Check if a shell command matches known dangerous patterns.
pub fn is_dangerous_command(command: &str) -> bool {
    use std::sync::OnceLock;

    static PATTERNS: OnceLock<Vec<regex::Regex>> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        [
            r"rm\s+-rf\s+/",
            r"rm\s+-rf\s+~/",
            r"git\s+push\s+.*--force",
            r"sudo\s+rm",
            r"mkfs\.",
            r"dd\s+if=.*of=/dev",
            r":\(\)\{\s*:\|:&\s*\};:",
        ]
        .iter()
        .filter_map(|p| regex::Regex::new(p).ok())
        .collect()
    });

    patterns.iter().any(|r| r.is_match(command))
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

    // ── is_dangerous_command ───────────────────────────────────────

    #[test]
    fn dangerous_rm_rf_root() {
        assert!(is_dangerous_command("rm -rf /"));
    }

    #[test]
    fn dangerous_rm_rf_home() {
        assert!(is_dangerous_command("rm -rf ~/"));
    }

    #[test]
    fn dangerous_force_push() {
        assert!(is_dangerous_command("git push origin main --force"));
    }

    #[test]
    fn dangerous_sudo_rm() {
        assert!(is_dangerous_command("sudo rm -rf something"));
    }

    #[test]
    fn safe_cargo_test() {
        assert!(!is_dangerous_command("cargo test"));
    }

    #[test]
    fn safe_git_push() {
        assert!(!is_dangerous_command("git push origin main"));
    }

    #[test]
    fn safe_rm_single_file() {
        assert!(!is_dangerous_command("rm temp.txt"));
    }

    #[test]
    fn dangerous_mkfs() {
        assert!(is_dangerous_command("mkfs.ext4 /dev/sda1"));
    }

    #[test]
    fn dangerous_dd_to_device() {
        assert!(is_dangerous_command("dd if=/dev/zero of=/dev/sda"));
    }

    #[test]
    fn dangerous_fork_bomb() {
        assert!(is_dangerous_command(":(){:|:&};:"));
    }

    #[test]
    fn safe_dd_to_file() {
        assert!(!is_dangerous_command("dd if=/dev/zero of=output.img bs=1M count=100"));
    }

    #[test]
    fn safe_empty_command() {
        assert!(!is_dangerous_command(""));
    }
}
