use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::AgentPref;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub cursor_hooks_installed: bool,
    pub claude_hooks_installed: bool,
    pub preferred_agent: Option<AgentPref>,
    pub allow_shell_commands: bool,
    pub allow_file_writes: bool,
    pub blocked_patterns: Vec<String>,
    pub settings: serde_json::Value,
    /// Whether this project requires git for agent operations.
    /// When false, workers will skip git validation and git-related workflow steps.
    pub requires_git: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProject {
    pub name: String,
    pub path: String,
    pub preferred_agent: Option<AgentPref>,
    /// Whether this project requires git (defaults to true if not specified)
    #[serde(default = "default_requires_git")]
    pub requires_git: bool,
}

fn default_requires_git() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProject {
    pub name: Option<String>,
    pub preferred_agent: Option<AgentPref>,
    pub allow_shell_commands: Option<bool>,
    pub allow_file_writes: Option<bool>,
    pub blocked_patterns: Option<Vec<String>>,
    pub requires_git: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_serializes_to_camel_case() {
        let project = Project {
            id: "p1".to_string(),
            name: "Test".to_string(),
            path: "/tmp".to_string(),
            cursor_hooks_installed: true,
            claude_hooks_installed: false,
            preferred_agent: Some(AgentPref::Cursor),
            allow_shell_commands: true,
            allow_file_writes: false,
            blocked_patterns: vec!["*.log".to_string()],
            settings: serde_json::json!({}),
            requires_git: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let json = serde_json::to_string(&project).unwrap();
        assert!(json.contains("\"cursorHooksInstalled\":true"));
        assert!(json.contains("\"allowFileWrites\":false"));
        assert!(json.contains("\"preferredAgent\":\"cursor\""));
        assert!(json.contains("\"requiresGit\":true"));
    }

    #[test]
    fn create_project_deserializes_from_camel_case() {
        let json = r#"{"name":"Proj","path":"/tmp","preferredAgent":"claude"}"#;
        let input: CreateProject = serde_json::from_str(json).unwrap();
        assert_eq!(input.name, "Proj");
        assert_eq!(input.preferred_agent, Some(AgentPref::Claude));
        // requires_git should default to true when not specified
        assert!(input.requires_git);
    }

    #[test]
    fn create_project_requires_git_can_be_false() {
        let json = r#"{"name":"Proj","path":"/tmp","requiresGit":false}"#;
        let input: CreateProject = serde_json::from_str(json).unwrap();
        assert!(!input.requires_git);
    }
}
