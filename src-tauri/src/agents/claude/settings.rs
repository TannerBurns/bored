//! Claude settings path utilities.

use std::path::{Path, PathBuf};

pub fn user_settings_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("settings.json"))
}

pub fn project_settings_path(project: &Path) -> PathBuf {
    project.join(".claude").join("settings.json")
}

pub fn local_settings_path(project: &Path) -> PathBuf {
    project.join(".claude").join("settings.local.json")
}

/// Check if a settings file contains hooks configuration
fn settings_file_has_hooks(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }

    match std::fs::read_to_string(path) {
        Ok(content) => {
            match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(json) => {
                    // Check if "hooks" key exists and is not empty
                    json.get("hooks")
                        .and_then(|h| h.as_object())
                        .map(|obj| !obj.is_empty())
                        .unwrap_or(false)
                }
                Err(_) => false,
            }
        }
        Err(_) => false,
    }
}

pub fn check_global_hooks_installed() -> bool {
    user_settings_path()
        .map(|p| settings_file_has_hooks(&p))
        .unwrap_or(false)
}

pub fn check_project_hooks_installed(project: &Path) -> bool {
    settings_file_has_hooks(&project_settings_path(project))
        || settings_file_has_hooks(&local_settings_path(project))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_settings_path_returns_some() {
        let path = user_settings_path();
        if dirs::home_dir().is_some() {
            assert!(path.is_some());
            assert!(path.unwrap().to_string_lossy().contains(".claude"));
        }
    }

    #[test]
    fn project_settings_path_is_correct() {
        let project = PathBuf::from("/tmp/my-project");
        let path = project_settings_path(&project);
        assert_eq!(path, PathBuf::from("/tmp/my-project/.claude/settings.json"));
    }

    #[test]
    fn local_settings_path_is_correct() {
        let project = PathBuf::from("/tmp/my-project");
        let path = local_settings_path(&project);
        assert_eq!(
            path,
            PathBuf::from("/tmp/my-project/.claude/settings.local.json")
        );
    }

    #[test]
    fn check_project_hooks_installed_returns_false_when_missing() {
        let temp_dir = std::env::temp_dir().join(format!("claude_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        assert!(!check_project_hooks_installed(&temp_dir));

        std::fs::remove_dir_all(&temp_dir).ok();
    }
}
