//! Discovery of bundled and custom command templates.
//!
//! Commands are read at prompt-generation time — no files are
//! installed into project directories.
//!
//! In development, the bundled commands live at `$CARGO_MANIFEST_DIR/scripts/commands`.
//! In production builds that path doesn't exist, so the Tauri resource-resolved
//! path (set once at startup via [`init_resource_commands_path`]) is used instead.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Resource-resolved path to the bundled commands directory, set once during
/// Tauri app setup so that production builds can locate the bundled `.md` files.
static RESOURCE_COMMANDS_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Register the Tauri-resolved resource path for bundled commands.
///
/// Call this once during app setup after the Tauri `AppHandle` is available.
pub fn init_resource_commands_path(path: PathBuf) {
    if let Err(_existing) = RESOURCE_COMMANDS_PATH.set(path) {
        tracing::warn!("Resource commands path was already initialized; ignoring duplicate call");
    }
}

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

/// Get the bundled commands path.
///
/// Checks the compile-time development path first (`CARGO_MANIFEST_DIR`),
/// then falls back to the resource path registered at startup.
pub fn get_bundled_commands_path() -> Option<PathBuf> {
    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("commands");
    if dev_path.exists() {
        return Some(dev_path);
    }

    RESOURCE_COMMANDS_PATH
        .get()
        .filter(|p| p.exists())
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn get_bundled_commands_path_returns_some_in_dev() {
        let path = get_bundled_commands_path();
        assert!(path.is_some(), "Should find bundled commands in dev build");
        assert!(path.unwrap().ends_with("scripts/commands"));
    }
}
