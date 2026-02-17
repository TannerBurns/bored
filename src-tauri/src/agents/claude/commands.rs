//! Claude command template management.
//!
//! Delegates to the shared `command_templates` module with ".claude" as the
//! config directory.

use std::path::{Path, PathBuf};

use crate::agents::command_templates;

pub const COMMAND_TEMPLATES: &[&str] = command_templates::COMMAND_TEMPLATES;

const CONFIG_DIR: &str = ".claude";

pub fn check_project_commands_installed(project: &Path) -> bool {
    command_templates::check_project_commands_installed(project, CONFIG_DIR)
}

pub fn check_user_commands_installed() -> bool {
    command_templates::check_user_commands_installed(CONFIG_DIR)
}

pub fn get_bundled_commands_path() -> Option<PathBuf> {
    command_templates::get_bundled_commands_path()
}

pub fn get_bundled_commands_path_with_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Option<PathBuf> {
    command_templates::get_bundled_commands_path_with_app(app)
}

pub fn install_commands(project: &Path, commands_source: &Path) -> std::io::Result<Vec<String>> {
    command_templates::install_commands(project, CONFIG_DIR, commands_source)
}

pub fn install_user_commands(commands_source: &Path) -> std::io::Result<Vec<String>> {
    command_templates::install_user_commands(CONFIG_DIR, commands_source)
}

pub fn get_available_commands(commands_source: &Path) -> Vec<String> {
    command_templates::get_available_commands(commands_source)
}
