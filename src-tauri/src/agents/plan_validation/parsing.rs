//! Parsing utilities for plan validation responses.

use crate::agents::json_extraction;

use super::config::{
    AutoClarificationAction, AutoClarificationResult, PlanValidationError, PlanValidationResult,
};

/// Parse the validation agent's response to extract the structured result.
///
/// Expects pre-extracted text (callers should use `provider.extract_text()`
/// before passing output here).
pub fn parse_validation_response(
    output: &str,
) -> Result<PlanValidationResult, PlanValidationError> {
    let trimmed = output.trim();

    if let Some(json_str) = json_extraction::extract_json_object(trimmed) {
        if let Ok(result) = serde_json::from_str::<PlanValidationResult>(&json_str) {
            return Ok(result);
        }

        // Fallback: handle camelCase / snake_case field name variations
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json_str) {
            let needs_clarification = value
                .get("needs_clarification")
                .or_else(|| value.get("needsClarification"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let reason = value
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("No reason provided")
                .to_string();

            return Ok(PlanValidationResult {
                needs_clarification,
                reason,
            });
        }
    }

    Err(PlanValidationError::ParseFailed(format!(
        "Could not parse validation response: {}",
        if trimmed.len() > 200 {
            &trimmed[..200]
        } else {
            trimmed
        }
    )))
}

/// Parse the auto-clarification agent's response into a structured result.
///
/// Expects pre-extracted text (callers should use `provider.extract_text()`
/// before passing output here).
pub fn parse_auto_clarification_response(
    output: &str,
) -> Result<AutoClarificationResult, PlanValidationError> {
    let trimmed = output.trim();

    if let Some(json_str) = json_extraction::extract_json_object(trimmed) {
        if let Ok(result) = serde_json::from_str::<AutoClarificationResult>(&json_str) {
            return Ok(result);
        }

        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json_str) {
            let action_str = value
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("cannot_resolve");

            let reason = value
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("No reason provided")
                .to_string();

            let action = match action_str {
                "update_task" => {
                    let content = value
                        .get("updated_content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    if content.is_empty() {
                        AutoClarificationAction::CannotResolve
                    } else {
                        AutoClarificationAction::UpdateTask {
                            updated_content: content,
                        }
                    }
                }
                "delete_task" => AutoClarificationAction::DeleteTask,
                _ => AutoClarificationAction::CannotResolve,
            };

            return Ok(AutoClarificationResult { action, reason });
        }
    }

    Err(PlanValidationError::ParseFailed(format!(
        "Could not parse auto-clarification response: {}",
        if trimmed.len() > 200 {
            &trimmed[..200]
        } else {
            trimmed
        }
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_validation_response_valid_json_needs_clarification() {
        let response =
            r#"{"needs_clarification": true, "reason": "Plan asks which framework to use"}"#;
        let result = parse_validation_response(response).unwrap();
        assert!(result.needs_clarification);
        assert_eq!(result.reason, "Plan asks which framework to use");
    }

    #[test]
    fn parse_validation_response_valid_json_no_clarification() {
        let response =
            r#"{"needs_clarification": false, "reason": "Plan has clear implementation path"}"#;
        let result = parse_validation_response(response).unwrap();
        assert!(!result.needs_clarification);
        assert_eq!(result.reason, "Plan has clear implementation path");
    }

    #[test]
    fn parse_validation_response_with_preamble() {
        let response = r#"Here is my analysis:
{"needs_clarification": true, "reason": "Multiple options presented"}
That's my assessment."#;
        let result = parse_validation_response(response).unwrap();
        assert!(result.needs_clarification);
        assert_eq!(result.reason, "Multiple options presented");
    }

    #[test]
    fn parse_validation_response_camel_case() {
        let response = r#"{"needsClarification": true, "reason": "Question asked"}"#;
        let result = parse_validation_response(response).unwrap();
        assert!(result.needs_clarification);
    }

    #[test]
    fn parse_validation_response_no_whitespace() {
        let response = r#"{"needs_clarification":true,"reason":"Compact JSON"}"#;
        let result = parse_validation_response(response).unwrap();
        assert!(result.needs_clarification);
    }

    #[test]
    fn parse_validation_response_extra_fields() {
        let response = r#"{"needs_clarification": false, "reason": "Ready", "confidence": 0.95}"#;
        let result = parse_validation_response(response).unwrap();
        assert!(!result.needs_clarification);
        assert_eq!(result.reason, "Ready");
    }

    #[test]
    fn parse_validation_response_missing_reason() {
        let response = r#"{"needs_clarification": true}"#;
        let result = parse_validation_response(response).unwrap();
        assert!(result.needs_clarification);
        assert_eq!(result.reason, "No reason provided");
    }

    #[test]
    fn parse_validation_response_invalid_json_fails() {
        let response = "This is not valid JSON at all";
        let result = parse_validation_response(response);
        assert!(result.is_err());
    }

    #[test]
    fn parse_validation_response_from_code_fence() {
        let response = r#"Here is my analysis:
```json
{"needs_clarification": true, "reason": "From code fence"}
```
Done."#;
        let result = parse_validation_response(response).unwrap();
        assert!(result.needs_clarification);
        assert_eq!(result.reason, "From code fence");
    }

    #[test]
    fn parse_validation_response_bracket_finding_fallback() {
        let response = r#"```json
not valid json inside fence!
```
Anyway {"needs_clarification": false, "reason": "Found via bracket"} here"#;
        let result = parse_validation_response(response).unwrap();
        assert!(!result.needs_clarification);
        assert_eq!(result.reason, "Found via bracket");
    }

    #[test]
    fn parse_validation_response_code_fence_camel_case() {
        let response = "```json\n{\"needsClarification\": true, \"reason\": \"Fenced camel\"}\n```";
        let result = parse_validation_response(response).unwrap();
        assert!(result.needs_clarification);
        assert_eq!(result.reason, "Fenced camel");
    }

    #[test]
    fn parse_validation_response_error_truncates_long_output() {
        let long_output = "x".repeat(500);
        let result = parse_validation_response(&long_output);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.len() < 500, "Error message should truncate long input");
    }

    #[test]
    fn parse_auto_clarification_update_task() {
        let response = r#"{"action": "update_task", "updated_content": "Use React", "reason": "Chose based on deps"}"#;
        let result = parse_auto_clarification_response(response).unwrap();
        assert_eq!(result.reason, "Chose based on deps");
        match result.action {
            AutoClarificationAction::UpdateTask { updated_content } => {
                assert_eq!(updated_content, "Use React");
            }
            _ => panic!("Expected UpdateTask"),
        }
    }

    #[test]
    fn parse_auto_clarification_delete_task() {
        let response =
            r#"{"action": "delete_task", "reason": "Already completed by previous task"}"#;
        let result = parse_auto_clarification_response(response).unwrap();
        assert_eq!(result.reason, "Already completed by previous task");
        assert!(matches!(
            result.action,
            AutoClarificationAction::DeleteTask
        ));
    }

    #[test]
    fn parse_auto_clarification_cannot_resolve() {
        let response =
            r#"{"action": "cannot_resolve", "reason": "Needs user preference on UI style"}"#;
        let result = parse_auto_clarification_response(response).unwrap();
        assert_eq!(result.reason, "Needs user preference on UI style");
        assert!(matches!(
            result.action,
            AutoClarificationAction::CannotResolve
        ));
    }

    #[test]
    fn parse_auto_clarification_with_preamble() {
        let response = r#"Here is my decision:
{"action": "delete_task", "reason": "Duplicate work"}
That's my analysis."#;
        let result = parse_auto_clarification_response(response).unwrap();
        assert!(matches!(
            result.action,
            AutoClarificationAction::DeleteTask
        ));
        assert_eq!(result.reason, "Duplicate work");
    }

    #[test]
    fn parse_auto_clarification_from_code_fence() {
        let response = "```json\n{\"action\": \"cannot_resolve\", \"reason\": \"Fenced response\"}\n```";
        let result = parse_auto_clarification_response(response).unwrap();
        assert!(matches!(
            result.action,
            AutoClarificationAction::CannotResolve
        ));
        assert_eq!(result.reason, "Fenced response");
    }

    #[test]
    fn parse_auto_clarification_unknown_action_becomes_cannot_resolve() {
        let response = r#"{"action": "something_else", "reason": "Unknown action type"}"#;
        let result = parse_auto_clarification_response(response).unwrap();
        assert!(matches!(
            result.action,
            AutoClarificationAction::CannotResolve
        ));
    }

    #[test]
    fn parse_auto_clarification_update_task_empty_content_becomes_cannot_resolve() {
        let response =
            r#"{"action": "update_task", "updated_content": "", "reason": "Empty rewrite"}"#;
        let result = parse_auto_clarification_response(response).unwrap();
        assert!(
            matches!(result.action, AutoClarificationAction::CannotResolve),
            "Empty updated_content should fall back to CannotResolve"
        );
    }

    #[test]
    fn parse_auto_clarification_update_task_missing_content_becomes_cannot_resolve() {
        let response = r#"{"action": "update_task", "reason": "Forgot the content field"}"#;
        let result = parse_auto_clarification_response(response).unwrap();
        assert!(
            matches!(result.action, AutoClarificationAction::CannotResolve),
            "Missing updated_content should fall back to CannotResolve"
        );
    }

    #[test]
    fn parse_auto_clarification_missing_reason_defaults() {
        let response = r#"{"action": "delete_task"}"#;
        let result = parse_auto_clarification_response(response).unwrap();
        assert!(matches!(
            result.action,
            AutoClarificationAction::DeleteTask
        ));
        assert_eq!(result.reason, "No reason provided");
    }

    #[test]
    fn parse_auto_clarification_missing_action_defaults_to_cannot_resolve() {
        let response = r#"{"reason": "No action specified"}"#;
        let result = parse_auto_clarification_response(response).unwrap();
        assert!(matches!(
            result.action,
            AutoClarificationAction::CannotResolve
        ));
    }

    #[test]
    fn parse_auto_clarification_invalid_json_fails() {
        let response = "This is not valid JSON at all";
        let result = parse_auto_clarification_response(response);
        assert!(result.is_err());
    }

    #[test]
    fn parse_auto_clarification_error_truncates_long_output() {
        let long_output = "z".repeat(500);
        let result = parse_auto_clarification_response(&long_output);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.len() < 500,
            "Error message should truncate long input"
        );
    }

    #[test]
    fn parse_auto_clarification_extra_fields_ignored() {
        let response = r#"{"action": "delete_task", "reason": "Done", "confidence": 0.99, "extra": true}"#;
        let result = parse_auto_clarification_response(response).unwrap();
        assert!(matches!(
            result.action,
            AutoClarificationAction::DeleteTask
        ));
        assert_eq!(result.reason, "Done");
    }

    #[test]
    fn parse_auto_clarification_no_whitespace() {
        let response = r#"{"action":"update_task","updated_content":"Compact","reason":"No spaces"}"#;
        let result = parse_auto_clarification_response(response).unwrap();
        match result.action {
            AutoClarificationAction::UpdateTask { updated_content } => {
                assert_eq!(updated_content, "Compact");
            }
            _ => panic!("Expected UpdateTask"),
        }
        assert_eq!(result.reason, "No spaces");
    }
}
