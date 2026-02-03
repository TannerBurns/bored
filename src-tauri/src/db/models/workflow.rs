use serde::{Deserialize, Serialize};

/// Workflow type for ticket execution
/// Note: Basic workflow has been removed - all tickets now use MultiStage
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowType {
    #[default]
    MultiStage,
}

impl WorkflowType {
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkflowType::MultiStage => "multi_stage",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            // Accept both for backward compatibility during migration
            "basic" | "multi_stage" => Some(WorkflowType::MultiStage),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_returns_snake_case() {
        assert_eq!(WorkflowType::MultiStage.as_str(), "multi_stage");
    }

    #[test]
    fn parse_valid_values() {
        // Both "basic" and "multi_stage" parse to MultiStage for backward compatibility
        assert_eq!(WorkflowType::parse("basic"), Some(WorkflowType::MultiStage));
        assert_eq!(
            WorkflowType::parse("multi_stage"),
            Some(WorkflowType::MultiStage)
        );
    }

    #[test]
    fn parse_invalid_returns_none() {
        assert_eq!(WorkflowType::parse(""), None);
        assert_eq!(WorkflowType::parse("invalid"), None);
        assert_eq!(WorkflowType::parse("BASIC"), None);
    }

    #[test]
    fn default_is_multi_stage() {
        assert_eq!(WorkflowType::default(), WorkflowType::MultiStage);
    }

    #[test]
    fn roundtrip_as_str_parse() {
        assert_eq!(
            WorkflowType::parse(WorkflowType::MultiStage.as_str()),
            Some(WorkflowType::MultiStage)
        );
    }

    #[test]
    fn serializes_to_snake_case() {
        assert_eq!(
            serde_json::to_string(&WorkflowType::MultiStage).unwrap(),
            "\"multi_stage\""
        );
    }
}
