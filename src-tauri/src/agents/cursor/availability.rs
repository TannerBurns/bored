//! Cursor CLI availability checking.

use std::process::Command;

pub fn is_cursor_available() -> bool {
    Command::new("cursor")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn get_cursor_version() -> Option<String> {
    Command::new("cursor")
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
    fn is_cursor_available_returns_bool() {
        let _result = is_cursor_available();
    }

    #[test]
    fn get_cursor_version_returns_option() {
        // Verify function runs without panic
        let result = get_cursor_version();
        // If cursor is available, version should be non-empty
        if let Some(version) = result {
            assert!(!version.is_empty());
        }
    }
}
