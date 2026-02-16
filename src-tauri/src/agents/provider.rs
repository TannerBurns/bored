//! Agent provider trait — the core abstraction for agent-agnostic execution.
//!
//! Every agent backend (Claude, Cursor, future agents) implements `AgentProvider`
//! so that the orchestrator, spawner, and runner can work with any agent without
//! hard-coding agent-specific logic.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::cost::RunCostData;

/// Configuration for running an agent — agent-agnostic.
#[derive(Debug, Clone)]
pub struct AgentRunConfig {
    /// Which agent to use (e.g. "cursor", "claude")
    pub agent_id: String,
    pub ticket_id: String,
    pub run_id: String,
    pub repo_path: PathBuf,
    pub prompt: String,
    pub timeout_secs: Option<u64>,
    pub api_url: String,
    pub api_token: String,
    pub model: Option<String>,
    /// Opaque agent-specific configuration.
    /// Each provider knows its own keys (e.g. Claude uses "auth_token", "api_key", etc.).
    pub agent_config: HashMap<String, serde_json::Value>,
}

/// The trait every agent implementation must satisfy.
///
/// Methods are synchronous where CLI access is blocking (availability, version),
/// and take shared references to allow concurrent use behind `Arc`.
pub trait AgentProvider: Send + Sync + std::fmt::Debug {
    /// Unique lowercase identifier (e.g. "cursor", "claude").
    fn id(&self) -> &str;

    /// Human-readable display name (e.g. "Cursor", "Claude Code").
    fn display_name(&self) -> &str;

    // ── Command building ────────────────────────────────────────────

    /// Build the CLI command and argument list for a run.
    fn build_command(&self, config: &AgentRunConfig) -> (String, Vec<String>);

    /// Build agent-specific environment variables.
    ///
    /// Base environment variables (AGENT_KANBAN_*) are added by the spawner;
    /// this method only needs to return additional, agent-specific variables.
    fn build_env_vars(&self, config: &AgentRunConfig) -> Vec<(String, String)>;

    // ── Output parsing ──────────────────────────────────────────────

    /// Extract the meaningful text from raw agent output.
    ///
    /// Claude uses stream-json and needs parsing; Cursor returns plain text.
    fn extract_text(&self, output: &str) -> String;

    /// Extract cost/token data from agent output, or estimate it.
    fn extract_cost(
        &self,
        stdout: &str,
        model: &str,
        duration_secs: f64,
    ) -> Option<RunCostData>;

    // ── Availability ────────────────────────────────────────────────

    /// Check whether this agent's CLI is available on the system.
    fn is_available(&self) -> bool;

    /// Get the agent CLI version string, if available.
    fn get_version(&self) -> Option<String>;

    // ── Directory configuration ─────────────────────────────────────

    /// The dot-directory name this agent uses (e.g. ".cursor", ".claude").
    fn config_dir_name(&self) -> &str;

    /// Subdirectory under `config_dir_name()` where command instruction files
    /// live in a repo. For Cursor this is "rules", for Claude this is "commands".
    fn command_instructions_subdir(&self) -> &str;

    /// Format a command reference as this agent expects it in a prompt.
    /// e.g. Cursor returns "/deslop", Claude returns ".claude/commands/deslop.md".
    fn format_command_reference(&self, command: &str) -> String;

    // ── Hooks ───────────────────────────────────────────────────────

    /// Install project-level hooks for a specific run.
    ///
    /// Called by the orchestrator/runner before each stage so the hook script
    /// can report events back to the API with the correct run ID.
    fn install_hooks_for_run(
        &self,
        repo_path: &Path,
        hook_script_path: &str,
        api_url: Option<&str>,
        api_token: Option<&str>,
        run_id: Option<&str>,
    ) -> Result<(), String>;

    /// Check whether hooks are installed at the global/user level.
    fn check_hooks_installed_global(&self) -> bool {
        false
    }

    /// Check whether hooks are installed for a specific project.
    fn check_hooks_installed_project(&self, _repo_path: &Path) -> bool {
        false
    }

    // ── Commands checking and installation ───────────────────────────

    /// Check whether command templates are installed in a project.
    fn check_commands_installed_project(&self, _repo_path: &Path) -> bool {
        false
    }

    /// Check whether command templates are installed at the user level.
    fn check_commands_installed_user(&self) -> bool {
        false
    }

    /// Install command templates into a project directory.
    fn install_commands_to_project(
        &self,
        _repo_path: &Path,
        _commands_source: &Path,
    ) -> Result<Vec<String>, String> {
        Ok(vec![])
    }

    /// Install command templates to the user-level directory.
    fn install_commands_to_user(
        &self,
        _commands_source: &Path,
    ) -> Result<Vec<String>, String> {
        Ok(vec![])
    }

    /// Get the path to bundled command templates (dev builds).
    fn get_bundled_commands_path(&self) -> Option<PathBuf> {
        let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("scripts")
            .join("commands");
        if dev_path.exists() {
            Some(dev_path)
        } else {
            None
        }
    }
}
