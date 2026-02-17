//! Claude CLI availability checking.

use crate::agents::cli_utils;

pub fn is_claude_available() -> bool {
    cli_utils::is_cli_available("claude")
}

pub fn get_claude_version() -> Option<String> {
    cli_utils::get_cli_version("claude")
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
