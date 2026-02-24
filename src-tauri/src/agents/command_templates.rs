//! Discovery of bundled and custom command templates.
//!
//! Commands are read at prompt-generation time — no files are
//! installed into project directories.

use std::path::{Path, PathBuf};

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
