//! Response parsing for brainstorm conversations.

use crate::db::StructuredSpec;

use super::config::{BrainstormError, BrainstormResponse};

/// Parse an agent response into a BrainstormResponse.
/// 
/// Supports two formats:
/// 1. **Structured JSON** (preferred): The response contains a JSON block with
///    `spec_complete`, `observations`, `questions`, and optionally `structured_spec`.
/// 2. **Legacy markdown**: The response uses `## Observations` / `## Questions` headers
///    with an optional ```json completion block.
pub fn parse_response(response: &str) -> Result<BrainstormResponse, BrainstormError> {
    // Try structured JSON parsing first (new format)
    if let Some(result) = try_parse_structured_json(response) {
        return result;
    }

    // Fall back to legacy markdown + JSON fence parsing
    parse_legacy_response(response)
}

/// Try to parse the response as our structured JSON format.
/// Returns None if no structured JSON was found, Some(result) if parsed.
fn try_parse_structured_json(response: &str) -> Option<Result<BrainstormResponse, BrainstormError>> {
    // Extract JSON from code fence or raw JSON
    let json_str = extract_json_block(response)?;
    
    let parsed: serde_json::Value = serde_json::from_str(&json_str).ok()?;
    
    // Must have spec_complete field to be our structured format
    let is_complete = parsed.get("spec_complete")?.as_bool()?;
    
    let observations = parsed.get("observations")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    
    if is_complete {
        // Completion: extract structured_spec
        if let Some(spec_value) = parsed.get("structured_spec") {
            let structured_spec: StructuredSpec = match serde_json::from_value(spec_value.clone()) {
                Ok(s) => s,
                Err(e) => return Some(Err(BrainstormError::ParseError(
                    format!("Failed to parse structured_spec: {}", e)
                ))),
            };
            
            let message = if observations.is_empty() {
                "Great! I have enough information to proceed with the specification.".to_string()
            } else {
                format!("## Observations\n{}", observations)
            };
            
            return Some(Ok(BrainstormResponse {
                message,
                is_complete: true,
                has_questions: false,
                structured_spec: Some(structured_spec),
            }));
        }
    } else {
        // Not complete: extract observations and questions
        let questions = parsed.get("questions")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        
        let has_questions = !questions.is_empty() && questions.len() > 5;
        
        // Build a readable message from observations + questions
        let mut message = String::new();
        if !observations.is_empty() {
            message.push_str("## Observations\n");
            message.push_str(&observations);
        }
        if !questions.is_empty() {
            if !message.is_empty() {
                message.push_str("\n\n");
            }
            message.push_str("## Questions\n");
            message.push_str(&questions);
        }
        if message.is_empty() {
            message = response.trim().to_string();
        }
        
        return Some(Ok(BrainstormResponse {
            message,
            is_complete: false,
            has_questions,
            structured_spec: None,
        }));
    }
    
    None
}

/// Extract a JSON string from the response, supporting both code-fenced and raw JSON.
fn extract_json_block(response: &str) -> Option<String> {
    // Try code-fenced JSON first
    if let Some(fence_start) = response.find("```json") {
        let content_start = fence_start + 7;
        if let Some(fence_end) = response[content_start..].find("```") {
            let json_str = response[content_start..content_start + fence_end].trim();
            if !json_str.is_empty() {
                return Some(json_str.to_string());
            }
        }
    }
    
    // Try raw JSON with spec_complete
    if let Some(json_start) = response.find("{\"spec_complete\"") {
        let mut depth = 0;
        for (i, c) in response[json_start..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(response[json_start..json_start + i + 1].to_string());
                    }
                }
                _ => {}
            }
        }
    }
    
    None
}

/// Legacy parsing: handles old-style markdown with ## Observations / ## Questions headers
fn parse_legacy_response(response: &str) -> Result<BrainstormResponse, BrainstormError> {
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

    // === New structured JSON format tests ===

    #[test]
    fn parse_structured_json_with_questions() {
        let response_text = r#"```json
{
  "spec_complete": false,
  "observations": "Found JWT auth patterns in src/auth/.\nThe API uses middleware for auth checks.",
  "questions": "1. Which auth provider?\n   A) Google\n   B) GitHub\n   C) Both"
}
```"#;

        let response = parse_response(response_text).unwrap();
        assert!(!response.is_complete);
        assert!(response.has_questions);
        assert!(response.message.contains("## Observations"));
        assert!(response.message.contains("JWT auth"));
        assert!(response.message.contains("## Questions"));
        assert!(response.message.contains("auth provider"));
    }

    #[test]
    fn parse_structured_json_completion() {
        let response_text = r#"```json
{
  "spec_complete": true,
  "observations": "Final summary of findings",
  "structured_spec": {
    "requirements": "Build OAuth integration",
    "decisions": ["Use OAuth 2.0", "Support Google"],
    "constraints": ["Must work offline"],
    "technical_notes": "Extend existing auth module"
  }
}
```"#;

        let response = parse_response(response_text).unwrap();
        assert!(response.is_complete);
        assert!(response.structured_spec.is_some());
        let spec = response.structured_spec.unwrap();
        assert!(spec.requirements.contains("OAuth"));
        assert_eq!(spec.decisions.len(), 2);
        assert!(response.message.contains("Observations"));
    }

    #[test]
    fn parse_structured_json_no_questions_field() {
        let response_text = r#"```json
{
  "spec_complete": false,
  "observations": "Explored the codebase and found all patterns."
}
```"#;

        let response = parse_response(response_text).unwrap();
        assert!(!response.is_complete);
        assert!(!response.has_questions);
        assert!(response.message.contains("Observations"));
    }

    #[test]
    fn parse_structured_json_raw_no_fence() {
        let response_text = r#"{"spec_complete": true, "structured_spec": {"requirements": "Build auth", "decisions": [], "constraints": []}}"#;

        let response = parse_response(response_text).unwrap();
        assert!(response.is_complete);
        assert!(response.structured_spec.is_some());
    }

    // === Legacy format tests (backward compatibility) ===

    #[test]
    fn parse_legacy_simple_message() {
        let response = parse_response(
            "What authentication method would you prefer?\n\nA) OAuth\nB) JWT\nC) Session-based"
        ).unwrap();

        assert!(!response.is_complete);
        assert!(response.structured_spec.is_none());
        assert!(response.message.contains("authentication"));
    }

    #[test]
    fn parse_legacy_completion_json() {
        let response_text = r#"```json
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
    }

    #[test]
    fn parse_legacy_text_before_completion_json() {
        let response_text = r#"I've gathered all the information needed.

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
        // With new parser, observations come from JSON or default message
        assert!(!response.message.is_empty());
    }

    #[test]
    fn parse_legacy_default_message_when_no_observations() {
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
    fn parse_legacy_with_technical_notes() {
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
    fn parse_legacy_nested_json_in_notes() {
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

    // === response_has_questions tests ===

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
    fn parse_legacy_sets_has_questions_true() {
        let response = parse_response(
            "## Observations\nFound patterns.\n\n## Questions\nWhich approach?"
        ).unwrap();

        assert!(!response.is_complete);
        assert!(response.has_questions);
    }

    #[test]
    fn parse_legacy_sets_has_questions_false_for_observations_only() {
        let response = parse_response(
            "## Observations\nFound all the patterns needed. No further questions."
        ).unwrap();

        assert!(!response.is_complete);
        assert!(!response.has_questions);
    }
}
