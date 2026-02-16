//! Cursor CLI integration: commands, hooks, and availability checking.

// Submodules
mod availability;
mod command;
mod commands;
mod hooks;
pub mod provider;
mod settings;

// Public re-exports
pub use availability::{get_cursor_version, is_cursor_available};
pub use command::CursorSettings;
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
