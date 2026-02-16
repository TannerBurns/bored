use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    /// Hook installation status per agent (keyed by agent ID, e.g. "cursor" -> true).
    pub hooks_installed: HashMap<String, bool>,
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
        let mut hooks = HashMap::new();
        hooks.insert("cursor".to_string(), true);
        let project = Project {
            id: "p1".to_string(),
            name: "Test".to_string(),
            path: "/tmp".to_string(),
            hooks_installed: hooks,
            allow_shell_commands: true,
            allow_file_writes: false,
            blocked_patterns: vec!["*.log".to_string()],
            settings: serde_json::json!({}),
            requires_git: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let json = serde_json::to_string(&project).unwrap();
        assert!(json.contains("\"hooksInstalled\""));
        assert!(json.contains("\"cursor\":true"));
        assert!(json.contains("\"allowFileWrites\":false"));
        assert!(json.contains("\"requiresGit\":true"));
    }

    #[test]
    fn create_project_deserializes_from_camel_case() {
        let json = r#"{"name":"Proj","path":"/tmp"}"#;
        let input: CreateProject = serde_json::from_str(json).unwrap();
        assert_eq!(input.name, "Proj");
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
