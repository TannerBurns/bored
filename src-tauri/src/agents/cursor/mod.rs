//! Cursor CLI integration: commands, hooks, and availability checking.

// Submodules
mod command;
mod hooks;
pub mod provider;
#[cfg(test)]
mod provider_tests;
mod settings;

// Public re-exports
pub use hooks::{
    generate_hooks_config, generate_hooks_json, generate_hooks_json_with_api,
    generate_hooks_json_with_config, install_global_hooks, install_global_hooks_with_run_id,
    install_hooks, install_hooks_with_run_id, HooksConfig,
};
pub use settings::{check_global_hooks_installed, check_project_hooks_installed, global_hooks_path};
