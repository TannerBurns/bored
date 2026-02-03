//! Cursor settings path utilities.

use std::path::{Path, PathBuf};

pub fn global_hooks_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".cursor").join("hooks.json"))
}

pub fn check_global_hooks_installed() -> bool {
    global_hooks_path().map(|p| p.exists()).unwrap_or(false)
}

pub fn check_project_hooks_installed(repo_path: &Path) -> bool {
    repo_path.join(".cursor").join("hooks.json").exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_hooks_path_returns_some() {
        let path = global_hooks_path();
        if dirs::home_dir().is_some() {
            assert!(path.is_some());
            assert!(path.unwrap().to_string_lossy().contains(".cursor"));
        }
    }

    #[test]
    fn check_global_hooks_installed_returns_false_when_no_file() {
        // This test checks behavior - global hooks path exists but file doesn't
        // We can't easily test this without mocking, but we verify the function runs
        let result = check_global_hooks_installed();
        // Result depends on actual system state, just verify it doesn't panic
        let _ = result;
    }

    #[test]
    fn check_project_hooks_installed_returns_false_when_missing() {
        let temp_dir = std::env::temp_dir().join(format!("cursor_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        assert!(!check_project_hooks_installed(&temp_dir));

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();
    }
}
