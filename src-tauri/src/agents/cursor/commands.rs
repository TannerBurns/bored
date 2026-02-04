//! Cursor command template management.

use std::path::{Path, PathBuf};

pub const COMMAND_TEMPLATES: &[&str] = &[
    "add-and-commit.md",
    "cleanup.md",
    "deslop.md",
    "review-changes.md",
    "unit-tests.md",
];

pub fn check_project_commands_installed(repo_path: &Path) -> bool {
    let commands_dir = repo_path.join(".cursor").join("commands");
    if !commands_dir.exists() {
        return false;
    }

    COMMAND_TEMPLATES
        .iter()
        .all(|name| commands_dir.join(name).exists())
}

/// Get the user-level commands directory (~/.cursor/commands/)
pub fn user_commands_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".cursor").join("commands"))
}

/// Check if commands are installed at the user level (~/.cursor/commands/)
pub fn check_user_commands_installed() -> bool {
    user_commands_path()
        .map(|p| {
            if !p.exists() {
                return false;
            }
            COMMAND_TEMPLATES.iter().all(|name| p.join(name).exists())
        })
        .unwrap_or(false)
}

/// Get the bundled commands path, checking development path first.
/// This version doesn't have access to Tauri's resource resolver.
pub fn get_bundled_commands_path() -> Option<PathBuf> {
    // Check development path (only works in dev builds)
    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("commands");
    if dev_path.exists() {
        return Some(dev_path);
    }
    None
}

/// Get the bundled commands path with Tauri resource resolver fallback.
/// In production builds, uses Tauri's resource API to locate bundled commands.
pub fn get_bundled_commands_path_with_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Option<PathBuf> {
    use tauri::Manager;

    // First, check development path
    if let Some(path) = get_bundled_commands_path() {
        return Some(path);
    }

    // In production, resolve via Tauri's resource API
    // The commands are bundled under scripts/commands/
    app.path()
        .resolve("scripts/commands", tauri::path::BaseDirectory::Resource)
        .ok()
        .filter(|p| p.exists())
}

pub fn install_commands(repo_path: &Path, commands_source: &Path) -> std::io::Result<Vec<String>> {
    let commands_dir = repo_path.join(".cursor").join("commands");
    std::fs::create_dir_all(&commands_dir)?;

    let mut installed = Vec::new();

    for name in COMMAND_TEMPLATES {
        let source = commands_source.join(name);
        let dest = commands_dir.join(name);

        if source.exists() {
            std::fs::copy(&source, &dest)?;
            installed.push(name.to_string());
        }
    }

    Ok(installed)
}

/// Install command templates to the user-level directory (~/.cursor/commands/)
pub fn install_user_commands(commands_source: &Path) -> std::io::Result<Vec<String>> {
    let commands_dir = user_commands_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine home directory",
        )
    })?;

    std::fs::create_dir_all(&commands_dir)?;

    let mut installed = Vec::new();

    for name in COMMAND_TEMPLATES {
        let source = commands_source.join(name);
        let dest = commands_dir.join(name);

        if source.exists() {
            std::fs::copy(&source, &dest)?;
            installed.push(name.to_string());
        }
    }

    Ok(installed)
}

pub fn get_available_commands(commands_source: &Path) -> Vec<String> {
    COMMAND_TEMPLATES
        .iter()
        .filter(|name| commands_source.join(name).exists())
        .map(|s| s.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_templates_list_has_all_commands() {
        assert_eq!(COMMAND_TEMPLATES.len(), 5);
        assert!(COMMAND_TEMPLATES.contains(&"add-and-commit.md"));
        assert!(COMMAND_TEMPLATES.contains(&"cleanup.md"));
        assert!(COMMAND_TEMPLATES.contains(&"deslop.md"));
        assert!(COMMAND_TEMPLATES.contains(&"review-changes.md"));
        assert!(COMMAND_TEMPLATES.contains(&"unit-tests.md"));
    }

    #[test]
    fn check_project_commands_installed_returns_false_when_missing() {
        let temp_dir = std::env::temp_dir().join(format!("cursor_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        assert!(!check_project_commands_installed(&temp_dir));

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn install_commands_creates_directory_and_files() {
        let temp_dir = std::env::temp_dir().join(format!("cursor_test_{}", uuid::Uuid::new_v4()));
        let source_dir = temp_dir.join("source");
        std::fs::create_dir_all(&source_dir).unwrap();

        // Create source command files
        for name in COMMAND_TEMPLATES {
            std::fs::write(source_dir.join(name), format!("# {}", name)).unwrap();
        }

        let project_dir = temp_dir.join("project");
        std::fs::create_dir_all(&project_dir).unwrap();

        let installed = install_commands(&project_dir, &source_dir).unwrap();
        assert_eq!(installed.len(), 5);

        // Verify files exist
        let commands_dir = project_dir.join(".cursor").join("commands");
        for name in COMMAND_TEMPLATES {
            assert!(commands_dir.join(name).exists());
        }

        assert!(check_project_commands_installed(&project_dir));

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn get_available_commands_returns_existing_files() {
        let temp_dir = std::env::temp_dir().join(format!("cursor_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Create only some command files
        std::fs::write(temp_dir.join("cleanup.md"), "# cleanup").unwrap();
        std::fs::write(temp_dir.join("deslop.md"), "# deslop").unwrap();

        let available = get_available_commands(&temp_dir);
        assert_eq!(available.len(), 2);
        assert!(available.contains(&"cleanup.md".to_string()));
        assert!(available.contains(&"deslop.md".to_string()));

        std::fs::remove_dir_all(&temp_dir).ok();
    }
}
