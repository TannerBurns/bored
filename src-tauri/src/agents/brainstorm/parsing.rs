//! Response parsing for brainstorm conversations.

use crate::db::StructuredSpec;

use super::config::{BrainstormError, BrainstormResponse};

/// Parse an agent response into a BrainstormResponse
pub fn parse_response(response: &str) -> Result<BrainstormResponse, BrainstormError> {
    if let Some(json_start) = response.find("```json") {
        if let Some(json_end) = response[json_start..].find("```\n").or_else(|| {
            response[json_start + 7..].find("```").map(|i| i + 7)
        }) {
            let json_str = response[json_start + 7..json_start + json_end].trim();

            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                if parsed.get("spec_complete").and_then(|v| v.as_bool()) == Some(true) {
                    if let Some(spec_value) = parsed.get("structured_spec") {
                        let structured_spec: StructuredSpec =
                            serde_json::from_value(spec_value.clone()).map_err(|e| {
                                BrainstormError::ParseError(format!(
                                    "Failed to parse structured_spec: {}",
                                    e
                                ))
                            })?;

                        // Extract any text before the JSON as the final message
                        let message = response[..json_start].trim().to_string();
                        let final_message = if message.is_empty() {
                            "Great! I have enough information to proceed with the specification.".to_string()
                        } else {
                            message
                        };

                        return Ok(BrainstormResponse {
                            message: final_message,
                            is_complete: true,
                            has_questions: false,
                            structured_spec: Some(structured_spec),
                        });
                    }
                }
            }
        }
    }

    // Also check for raw JSON (without code fence)
    if let Some(json_start) = response.find("{\"spec_complete\"") {
        // Find the end of the JSON object
        let mut depth = 0;
        let mut json_end = json_start;
        for (i, c) in response[json_start..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        json_end = json_start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }

        if json_end > json_start {
            let json_str = &response[json_start..json_end];
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                if parsed.get("spec_complete").and_then(|v| v.as_bool()) == Some(true) {
                    if let Some(spec_value) = parsed.get("structured_spec") {
                        let structured_spec: StructuredSpec =
                            serde_json::from_value(spec_value.clone()).map_err(|e| {
                                BrainstormError::ParseError(format!(
                                    "Failed to parse structured_spec: {}",
                                    e
                                ))
                            })?;

                        let message = response[..json_start].trim().to_string();
                        let final_message = if message.is_empty() {
                            "Great! I have enough information to proceed with the specification.".to_string()
                        } else {
                            message
                        };

                        return Ok(BrainstormResponse {
                            message: final_message,
                            is_complete: true,
                            has_questions: false,
                            structured_spec: Some(structured_spec),
                        });
                    }
                }
            }
        }
    }

    // No completion signal - check if response has questions
    let has_questions = response_has_questions(response);
    
    Ok(BrainstormResponse {
        message: response.trim().to_string(),
        is_complete: false,
        has_questions,
        structured_spec: None,
    })
}

/// Check if a response contains questions (looks for "## Questions" section with content)
pub fn response_has_questions(response: &str) -> bool {
    // Look for "## Questions" header
    if let Some(questions_start) = response.find("## Questions") {
        let after_header = &response[questions_start + 12..]; // Skip "## Questions"
        
        // Find the next section header or end
        let section_end = after_header.find("\n## ").unwrap_or(after_header.len());
        let questions_section = after_header[..section_end].trim();
        
        // Check if there's actual content (not just whitespace or "None")
        !questions_section.is_empty() 
            && !questions_section.eq_ignore_ascii_case("none")
            && !questions_section.eq_ignore_ascii_case("n/a")
            && questions_section.len() > 5 // At least some substantive content
    } else {
        // No "## Questions" section found - check for question marks in the response
        response.contains('?')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_response_simple_message() {
        let response = parse_response(
            "What authentication method would you prefer?\n\nA) OAuth\nB) JWT\nC) Session-based"
        ).unwrap();

        assert!(!response.is_complete);
        assert!(response.structured_spec.is_none());
        assert!(response.message.contains("authentication"));
    }

    #[test]
    fn parse_response_with_completion_json() {
        let response_text = r#"Great, I have all the information I need!

```json
{
  "spec_complete": true,
  "structured_spec": {
    "requirements": "Build a user auth system with OAuth",
    "decisions": ["Use OAuth 2.0", "Support Google and GitHub"],
    "constraints": ["Must work offline"],
    "technical_notes": "Consider using passport.js"
  }
}
```"#;

        let response = parse_response(response_text).unwrap();

        assert!(response.is_complete);
        assert!(response.structured_spec.is_some());
        let spec = response.structured_spec.unwrap();
        assert!(spec.requirements.contains("OAuth"));
        assert_eq!(spec.decisions.len(), 2);
        assert_eq!(spec.constraints.len(), 1);
    }

    #[test]
    fn parse_response_with_raw_json() {
        let response_text = r#"{"spec_complete": true, "structured_spec": {"requirements": "Build auth", "decisions": [], "constraints": []}}"#;

        let response = parse_response(response_text).unwrap();

        assert!(response.is_complete);
        assert!(response.structured_spec.is_some());
    }

    #[test]
    fn parse_response_with_incomplete_json_treated_as_message() {
        // JSON that doesn't have spec_complete: true
        let response_text = r#"```json
{
  "spec_complete": false,
  "message": "Need more info"
}
```"#;

        let response = parse_response(response_text).unwrap();
        assert!(!response.is_complete);
        assert!(response.structured_spec.is_none());
    }

    #[test]
    fn parse_response_extracts_message_before_json() {
        let response_text = r#"I've gathered all the information needed for the spec.

```json
{
  "spec_complete": true,
  "structured_spec": {
    "requirements": "Build feature X",
    "decisions": [],
    "constraints": []
  }
}
```"#;

        let response = parse_response(response_text).unwrap();
        assert!(response.is_complete);
        assert!(response.message.contains("gathered all the information"));
    }

    #[test]
    fn parse_response_provides_default_message_when_no_text_before_json() {
        let response_text = r#"```json
{
  "spec_complete": true,
  "structured_spec": {
    "requirements": "Build feature X",
    "decisions": [],
    "constraints": []
  }
}
```"#;

        let response = parse_response(response_text).unwrap();
        assert!(response.is_complete);
        assert!(response.message.contains("enough information"));
    }

    #[test]
    fn parse_response_with_technical_notes() {
        let response_text = r#"```json
{
  "spec_complete": true,
  "structured_spec": {
    "requirements": "Build auth",
    "decisions": ["Use JWT"],
    "constraints": ["Must be fast"],
    "technicalNotes": "Consider using middleware pattern"
  }
}
```"#;

        let response = parse_response(response_text).unwrap();
        assert!(response.is_complete);
        let spec = response.structured_spec.unwrap();
        assert_eq!(spec.technical_notes, Some("Consider using middleware pattern".to_string()));
    }

    #[test]
    fn response_has_questions_with_questions_section() {
        let response = r#"## Observations
I found some interesting patterns in the codebase.

## Questions
What authentication method would you prefer?
A) OAuth
B) JWT
C) Session-based"#;

        assert!(response_has_questions(response));
    }

    #[test]
    fn response_has_questions_empty_questions_section() {
        let response = r#"## Observations
I found all the information needed from the codebase exploration.

## Questions
"#;

        assert!(!response_has_questions(response));
    }

    #[test]
    fn response_has_questions_no_questions_section_but_has_question_mark() {
        let response = "What do you think about this approach?";
        assert!(response_has_questions(response));
    }

    #[test]
    fn response_has_questions_observations_only() {
        let response = r#"## Observations
I found all the information needed from the codebase exploration.
The existing auth module uses JWT tokens.
The API follows RESTful conventions."#;

        assert!(!response_has_questions(response));
    }

    #[test]
    fn parse_response_sets_has_questions_true() {
        let response = parse_response(
            "## Observations\nFound patterns.\n\n## Questions\nWhich approach?"
        ).unwrap();

        assert!(!response.is_complete);
        assert!(response.has_questions);
    }

    #[test]
    fn parse_response_sets_has_questions_false_for_observations_only() {
        let response = parse_response(
            "## Observations\nFound all the patterns needed. No further questions."
        ).unwrap();

        assert!(!response.is_complete);
        assert!(!response.has_questions);
    }

    #[test]
    fn parse_response_with_nested_json_in_notes() {
        let response_text = r#"```json
{
  "spec_complete": true,
  "structured_spec": {
    "requirements": "Build API",
    "decisions": ["RESTful design"],
    "constraints": ["Must handle {nested} braces"],
    "technicalNotes": "Use pattern: { key: value }"
  }
}
```"#;

        let response = parse_response(response_text).unwrap();
        assert!(response.is_complete);
        let spec = response.structured_spec.unwrap();
        assert!(spec.constraints[0].contains("{nested}"));
    }
}
