//! Default model constants used as fallbacks when no model is configured.
//!
//! These use full Claude CLI identifiers since Claude is the primary agent.
//! Other providers normalize through their own command building.

pub const DEFAULT_STAGE_MODEL: &str = "claude-opus-4-6";
pub const DEFAULT_DIAGNOSTIC_MODEL: &str = "claude-sonnet-4-6";
pub const DEFAULT_GENERAL_CHAT_MODEL: &str = "claude-opus-4-6";
pub const DEFAULT_PLANNER_CHAT_MODEL: &str = "claude-opus-4-5";
pub const DEFAULT_VALIDATION_CHAT_MODEL: &str = "claude-sonnet-4-6";
