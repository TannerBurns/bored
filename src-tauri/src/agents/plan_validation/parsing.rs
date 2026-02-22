//! Parsing utilities for plan validation responses.

use crate::agents::json_extraction;

use super::config::{PlanValidationError, PlanValidationResult};

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
}
