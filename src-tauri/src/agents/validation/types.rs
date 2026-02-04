//! Types for worker environment validation.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub valid: bool,
    pub checks: Vec<ValidationCheck>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationCheck {
    pub name: String,
    pub passed: bool,
    pub message: String,
    pub fix_action: Option<String>,
    #[serde(default)]
    pub is_warning: bool,
}

impl ValidationCheck {
    pub fn pass(name: &str, message: &str) -> Self {
        Self {
            name: name.to_string(),
            passed: true,
            message: message.to_string(),
            fix_action: None,
            is_warning: false,
        }
    }

    pub fn fail(name: &str, message: &str, fix_action: Option<&str>) -> Self {
        Self {
            name: name.to_string(),
            passed: false,
            message: message.to_string(),
            fix_action: fix_action.map(|s| s.to_string()),
            is_warning: false,
        }
    }

    pub fn warning(name: &str, message: &str, fix_action: Option<&str>) -> Self {
        Self {
            name: name.to_string(),
            passed: true, // Warnings don't fail validation
            message: message.to_string(),
            fix_action: fix_action.map(|s| s.to_string()),
            is_warning: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_check_pass() {
        let check = ValidationCheck::pass("test", "Test passed");
        assert!(check.passed);
        assert!(!check.is_warning);
        assert_eq!(check.name, "test");
        assert!(check.fix_action.is_none());
    }

    #[test]
    fn validation_check_fail() {
        let check = ValidationCheck::fail("test", "Test failed", Some("fix_it"));
        assert!(!check.passed);
        assert!(!check.is_warning);
        assert_eq!(check.fix_action, Some("fix_it".to_string()));
    }

    #[test]
    fn validation_check_warning() {
        let check = ValidationCheck::warning("test", "Warning message", None);
        assert!(check.passed); // Warnings don't fail
        assert!(check.is_warning); // But they are marked as warnings
    }

    #[test]
    fn validation_result_serializes() {
        let result = ValidationResult {
            valid: true,
            checks: vec![ValidationCheck::pass("test", "OK")],
            errors: vec![],
            warnings: vec!["A warning".to_string()],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"valid\":true"));
        assert!(json.contains("\"checks\""));
    }
}
