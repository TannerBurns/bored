//! Cursor CLI availability checking.

use crate::agents::cli_utils;

pub fn is_cursor_available() -> bool {
    cli_utils::is_cli_available("cursor")
}

pub fn get_cursor_version() -> Option<String> {
    cli_utils::get_cli_version("cursor")
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
        let result = get_cursor_version();
        if let Some(version) = result {
            assert!(!version.is_empty());
        }
    }
}
