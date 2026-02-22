//! Shared command template management.
//!
//! Both Claude and Cursor agents install the same set of command template files
//! into their respective config directories. This module provides the shared
//! implementation parameterized by `config_dir` (e.g. ".claude", ".cursor").

use std::path::{Path, PathBuf};

/// The set of command templates bundled with the application.
pub const COMMAND_TEMPLATES: &[&str] = &[
    "add-and-commit.md",
    "cleanup.md",
    "code-review.md",
    "code-review-fix.md",
    "deslop.md",
    "review-changes.md",
    "unit-tests.md",
    "add-tests.md",
    "fix-lint.md",
    "sync-with-main.md",
    "review-polish.md",
    "patch-security.md",
    "api-contract-check.md",
    "observability-pass.md",
    "integration-test.md",
    "doc-sync.md",
];

/// Discover all `.md` command files from a source directory.
pub fn discover_commands(commands_source: &Path) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir(commands_source) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".md") {
                    names.push(name.to_string());
                }
            }
        }
    }
    names.sort();
    names
}

/// Check if all command templates are installed in a project's config directory.
pub fn check_project_commands_installed(project: &Path, config_dir: &str) -> bool {
    let commands_dir = project.join(config_dir).join("commands");
    if !commands_dir.exists() {
        return false;
    }

    COMMAND_TEMPLATES
        .iter()
        .all(|name| commands_dir.join(name).exists())
}

/// Get the user-level commands directory (e.g. ~/.claude/commands/).
pub fn user_commands_path(config_dir: &str) -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(config_dir).join("commands"))
}

/// Check if commands are installed at the user level.
pub fn check_user_commands_installed(config_dir: &str) -> bool {
    user_commands_path(config_dir)
        .map(|p| {
            if !p.exists() {
                return false;
            }
            COMMAND_TEMPLATES.iter().all(|name| p.join(name).exists())
        })
        .unwrap_or(false)
}

/// Get the bundled commands path, checking development path first.
pub fn get_bundled_commands_path() -> Option<PathBuf> {
    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("commands");
    if dev_path.exists() {
        return Some(dev_path);
    }
    None
}

/// Get the bundled commands path with Tauri resource resolver fallback.
pub fn get_bundled_commands_path_with_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Option<PathBuf> {
    use tauri::Manager;

    if let Some(path) = get_bundled_commands_path() {
        return Some(path);
    }

    app.path()
        .resolve("scripts/commands", tauri::path::BaseDirectory::Resource)
        .ok()
        .filter(|p| p.exists())
}

/// Install command templates from a source directory into a project's config directory.
pub fn install_commands(
    project: &Path,
    config_dir: &str,
    commands_source: &Path,
) -> std::io::Result<Vec<String>> {
    let commands_dir = project.join(config_dir).join("commands");
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

/// Install command templates to the user-level directory.
pub fn install_user_commands(
    config_dir: &str,
    commands_source: &Path,
) -> std::io::Result<Vec<String>> {
    let commands_dir = user_commands_path(config_dir).ok_or_else(|| {
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

/// Get available command templates from a source directory.
pub fn get_available_commands(commands_source: &Path) -> Vec<String> {
    discover_commands(commands_source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_templates_list_includes_all_known_commands() {
        assert!(COMMAND_TEMPLATES.contains(&"add-and-commit.md"));
        assert!(COMMAND_TEMPLATES.contains(&"cleanup.md"));
        assert!(COMMAND_TEMPLATES.contains(&"deslop.md"));
        assert!(COMMAND_TEMPLATES.contains(&"review-changes.md"));
        assert!(COMMAND_TEMPLATES.contains(&"unit-tests.md"));
        assert!(COMMAND_TEMPLATES.contains(&"code-review.md"));
        assert!(COMMAND_TEMPLATES.contains(&"code-review-fix.md"));
        assert!(COMMAND_TEMPLATES.contains(&"add-tests.md"));
        assert!(COMMAND_TEMPLATES.contains(&"fix-lint.md"));
        assert!(COMMAND_TEMPLATES.contains(&"sync-with-main.md"));
        assert!(COMMAND_TEMPLATES.contains(&"review-polish.md"));
        assert!(COMMAND_TEMPLATES.contains(&"patch-security.md"));
        assert!(COMMAND_TEMPLATES.contains(&"api-contract-check.md"));
        assert!(COMMAND_TEMPLATES.contains(&"observability-pass.md"));
        assert!(COMMAND_TEMPLATES.contains(&"integration-test.md"));
        assert!(COMMAND_TEMPLATES.contains(&"doc-sync.md"));
    }

    #[test]
    fn discover_commands_finds_md_files() {
        let temp_dir =
            std::env::temp_dir().join(format!("cmd_templates_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        std::fs::write(temp_dir.join("cleanup.md"), "# cleanup").unwrap();
        std::fs::write(temp_dir.join("deslop.md"), "# deslop").unwrap();
        std::fs::write(temp_dir.join("not-a-command.txt"), "nope").unwrap();

        let found = discover_commands(&temp_dir);
        assert_eq!(found.len(), 2);
        assert!(found.contains(&"cleanup.md".to_string()));
        assert!(found.contains(&"deslop.md".to_string()));

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn check_project_commands_installed_returns_false_when_missing() {
        let temp_dir =
            std::env::temp_dir().join(format!("cmd_templates_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        assert!(!check_project_commands_installed(&temp_dir, ".test"));

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn install_commands_creates_directory_and_files() {
        let temp_dir =
            std::env::temp_dir().join(format!("cmd_templates_test_{}", uuid::Uuid::new_v4()));
        let source_dir = temp_dir.join("source");
        std::fs::create_dir_all(&source_dir).unwrap();

        for name in COMMAND_TEMPLATES {
            std::fs::write(source_dir.join(name), format!("# {}", name)).unwrap();
        }

        let project_dir = temp_dir.join("project");
        std::fs::create_dir_all(&project_dir).unwrap();

        let installed = install_commands(&project_dir, ".test", &source_dir).unwrap();
        assert_eq!(installed.len(), COMMAND_TEMPLATES.len());

        let commands_dir = project_dir.join(".test").join("commands");
        for name in COMMAND_TEMPLATES {
            assert!(commands_dir.join(name).exists());
        }

        assert!(check_project_commands_installed(&project_dir, ".test"));

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn check_project_commands_returns_false_when_partial() {
        let temp_dir =
            std::env::temp_dir().join(format!("cmd_templates_test_{}", uuid::Uuid::new_v4()));
        let commands_dir = temp_dir.join(".test").join("commands");
        std::fs::create_dir_all(&commands_dir).unwrap();

        std::fs::write(commands_dir.join("cleanup.md"), "# cleanup").unwrap();
        std::fs::write(commands_dir.join("deslop.md"), "# deslop").unwrap();

        assert!(!check_project_commands_installed(&temp_dir, ".test"));

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn user_commands_path_returns_path_with_config_dir() {
        let path = user_commands_path(".testcli");
        assert!(path.is_some());
        let p = path.unwrap();
        assert!(p.to_string_lossy().contains(".testcli"));
        assert!(p.to_string_lossy().ends_with("commands"));
    }

    #[test]
    fn install_commands_skips_missing_source_files() {
        let temp_dir =
            std::env::temp_dir().join(format!("cmd_templates_test_{}", uuid::Uuid::new_v4()));
        let source_dir = temp_dir.join("source");
        std::fs::create_dir_all(&source_dir).unwrap();

        std::fs::write(source_dir.join("cleanup.md"), "# cleanup").unwrap();

        let project_dir = temp_dir.join("project");
        std::fs::create_dir_all(&project_dir).unwrap();

        let installed = install_commands(&project_dir, ".test", &source_dir).unwrap();
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0], "cleanup.md");

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn get_available_commands_returns_existing_files() {
        let temp_dir =
            std::env::temp_dir().join(format!("cmd_templates_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        std::fs::write(temp_dir.join("cleanup.md"), "# cleanup").unwrap();
        std::fs::write(temp_dir.join("deslop.md"), "# deslop").unwrap();

        let available = get_available_commands(&temp_dir);
        assert_eq!(available.len(), 2);
        assert!(available.contains(&"cleanup.md".to_string()));
        assert!(available.contains(&"deslop.md".to_string()));

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn discover_commands_returns_empty_for_nonexistent_dir() {
        let found = discover_commands(std::path::Path::new("/nonexistent/dir/that/does/not/exist"));
        assert!(found.is_empty());
    }

    #[test]
    fn discover_commands_returns_empty_for_empty_dir() {
        let temp_dir =
            std::env::temp_dir().join(format!("cmd_templates_empty_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let found = discover_commands(&temp_dir);
        assert!(found.is_empty());

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn discover_commands_returns_sorted_results() {
        let temp_dir =
            std::env::temp_dir().join(format!("cmd_templates_sort_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        std::fs::write(temp_dir.join("z-last.md"), "# z").unwrap();
        std::fs::write(temp_dir.join("a-first.md"), "# a").unwrap();
        std::fs::write(temp_dir.join("m-middle.md"), "# m").unwrap();

        let found = discover_commands(&temp_dir);
        assert_eq!(found, vec!["a-first.md", "m-middle.md", "z-last.md"]);

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn get_available_commands_discovers_non_template_md_files() {
        let temp_dir =
            std::env::temp_dir().join(format!("cmd_templates_custom_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        std::fs::write(temp_dir.join("cleanup.md"), "# cleanup").unwrap();
        std::fs::write(temp_dir.join("my-custom-command.md"), "# custom").unwrap();
        std::fs::write(temp_dir.join("not-markdown.txt"), "nope").unwrap();

        let available = get_available_commands(&temp_dir);
        assert_eq!(available.len(), 2);
        assert!(available.contains(&"cleanup.md".to_string()));
        assert!(available.contains(&"my-custom-command.md".to_string()));

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn get_available_commands_returns_empty_for_nonexistent_dir() {
        let available = get_available_commands(std::path::Path::new("/nonexistent"));
        assert!(available.is_empty());
    }
}
