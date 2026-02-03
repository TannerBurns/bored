//! Claude CLI availability checking.

use std::process::Command;

pub fn is_claude_available() -> bool {
    Command::new("claude")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn get_claude_version() -> Option<String> {
    Command::new("claude")
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
    fn is_claude_available_returns_bool() {
        let _result = is_claude_available();
    }

    #[test]
    fn get_claude_version_returns_option() {
        let result = get_claude_version();
        if let Some(version) = result {
            assert!(!version.is_empty());
        }
    }
}
