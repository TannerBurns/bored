//! Cursor CLI integration module.
//!
//! This module provides functions for building Cursor CLI commands,
//! managing hooks, and checking availability.
//!
//! Submodules:
//! - `command`: Command building
//! - `availability`: CLI availability checking
//! - `hooks`: Hook configuration generation and installation
//! - `settings`: Settings path utilities
//! - `commands`: Command template management


// Submodules
mod availability;
mod command;
mod commands;
mod hooks;
mod settings;

// Public re-exports
pub use availability::{get_cursor_version, is_cursor_available};
pub use command::{build_command, build_command_with_settings, CursorSettings};
pub use commands::{
    check_project_commands_installed, check_user_commands_installed, get_available_commands,
    get_bundled_commands_path, get_bundled_commands_path_with_app, install_commands,
    install_user_commands, COMMAND_TEMPLATES,
};
pub use hooks::{
    generate_hooks_config, generate_hooks_json, generate_hooks_json_with_api,
    generate_hooks_json_with_config, install_global_hooks, install_global_hooks_with_run_id,
    install_hooks, install_hooks_with_run_id, HooksConfig,
};
pub use settings::{check_global_hooks_installed, check_project_hooks_installed, global_hooks_path};
