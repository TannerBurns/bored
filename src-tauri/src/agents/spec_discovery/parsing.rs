//! Response parsing for spec discovery conversations.

use crate::db::StructuredSpec;

use super::config::{SpecDiscoveryError, SpecDiscoveryResponse};

/// Parse an agent response into a SpecDiscoveryResponse.
/// Tries structured JSON first, falls back to legacy markdown headers.
pub fn parse_response(response: &str) -> Result<SpecDiscoveryResponse, SpecDiscoveryError> {
    if let Some(result) = try_parse_structured_json(response) {
        return result;
    }

    parse_legacy_response(response)
}

/// Try to parse the response as our structured JSON format.
/// Returns None if no structured JSON was found, Some(result) if parsed.
fn try_parse_structured_json(response: &str) -> Option<Result<SpecDiscoveryResponse, SpecDiscoveryError>> {
    let json_str = extract_spec_complete_json(response)?;
    let parsed: serde_json::Value = serde_json::from_str(&json_str).ok()?;
    let is_complete = parsed.get("spec_complete")?.as_bool()?;
    
    let observations = parsed.get("observations")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    
    if is_complete {
        let spec_value = match parsed.get("structured_spec") {
            Some(v) => v,
            None => return Some(Err(SpecDiscoveryError::ParseError(
                "spec_complete is true but structured_spec is missing".to_string()
            ))),
        };
        let structured_spec: StructuredSpec = match serde_json::from_value(spec_value.clone()) {
            Ok(s) => s,
            Err(e) => return Some(Err(SpecDiscoveryError::ParseError(
                format!("Failed to parse structured_spec: {}", e)
            ))),
        };
        
        let message = serde_json::json!({
            "observations": observations,
        }).to_string();

        Some(Ok(SpecDiscoveryResponse {
            message,
            is_complete: true,
            has_questions: false,
            structured_spec: Some(structured_spec),
        }))
    } else {
        let questions = extract_questions_text(parsed.get("questions"));
        let has_questions = !questions.is_empty();

        let message = serde_json::json!({
            "observations": observations,
            "questions": questions,
        }).to_string();
        
        Some(Ok(SpecDiscoveryResponse {
            message,
            is_complete: false,
            has_questions,
            structured_spec: None,
        }))
    }
}

/// Extract questions text from the JSON value.
/// The value is expected to be a markdown string. Also handles legacy array format.
fn extract_questions_text(value: Option<&serde_json::Value>) -> String {
    let value = match value {
        Some(v) => v,
        None => return String::new(),
    };
    
    if let Some(s) = value.as_str() {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    
    // Legacy: structured array format [{question, options}]
    if let Some(arr) = value.as_array() {
        let mut parts = Vec::new();
        for (i, item) in arr.iter().enumerate() {
            if let Some(q) = item.get("question").and_then(|v| v.as_str()) {
                let mut question_block = format!("{}. {}", i + 1, q);
                
                if let Some(options) = item.get("options").and_then(|v| v.as_array()) {
                    for opt in options {
                        if let Some(opt_str) = opt.as_str() {
                            question_block.push_str(&format!("\n   - {}", opt_str));
                        }
                    }
                }
                
                parts.push(question_block);
            }
        }
        if !parts.is_empty() {
            return parts.join("\n\n");
        }
    }
    
    String::new()
}

/// Extract a JSON string that contains the `"spec_complete"` key.
///
/// Tries generic code-fence / first-object extraction first. If the extracted
/// object doesn't contain `"spec_complete"`, falls back to locating the key in
/// the raw text and walking backward to the enclosing `{`, then brace-matching
/// forward. This handles agent responses with multiple unfenced JSON objects
/// where the spec discovery payload isn't the first one.
fn extract_spec_complete_json(response: &str) -> Option<String> {
    if let Some(ref json_str) = crate::agents::json_extraction::extract_json_object(response) {
        if json_str.contains("\"spec_complete\"") {
            return Some(json_str.clone());
        }
    }

    let key_pos = response.find("\"spec_complete\"")?;
    let brace_start = response[..key_pos].rfind('{')?;
    let mut depth = 0;
    for (i, c) in response[brace_start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(response[brace_start..brace_start + i + c.len_utf8()].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// Legacy parsing: handles old-style markdown with ## Observations / ## Questions headers
fn parse_legacy_response(response: &str) -> Result<SpecDiscoveryResponse, SpecDiscoveryError> {
    // No completion signal - check if response has questions
    let has_questions = response_has_questions(response);
    
    Ok(SpecDiscoveryResponse {
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
    fn parse_structured_json_with_markdown_questions() {
        let response_text = r#"```json
{
  "spec_complete": false,
  "observations": "Found JWT auth patterns in `src/auth/`.\n- The API uses middleware for auth checks.\n- Routes are defined in `src/api/`.",
  "questions": "1. Which auth provider do you want to support?\n   - A) Google\n   - B) GitHub\n   - C) Both\n\n2. Should sessions be stateless?\n   - A) Yes, use JWT\n   - B) No, use server-side sessions"
}
```"#;

        let response = parse_response(response_text).unwrap();
        assert!(!response.is_complete);
        assert!(response.has_questions);
        let msg: serde_json::Value = serde_json::from_str(&response.message).unwrap();
        assert!(msg["observations"].as_str().unwrap().contains("JWT auth"));
        assert!(msg["questions"].as_str().unwrap().contains("Which auth provider"));
        assert!(msg["questions"].as_str().unwrap().contains("Should sessions be stateless"));
    }

    #[test]
    fn parse_structured_json_with_questions_array_legacy() {
        let response_text = r#"```json
{
  "spec_complete": false,
  "observations": "Found patterns.",
  "questions": [
    {
      "question": "Which approach?",
      "options": ["A) First", "B) Second"]
    }
  ]
}
```"#;

        let response = parse_response(response_text).unwrap();
        assert!(!response.is_complete);
        assert!(response.has_questions);
        let msg: serde_json::Value = serde_json::from_str(&response.message).unwrap();
        assert!(msg["questions"].as_str().unwrap().contains("Which approach"));
    }

    #[test]
    fn parse_structured_json_completion() {
        let response_text = r#"```json
{
  "spec_complete": true,
  "observations": "Final summary of findings",
  "structured_spec": {
    "requirements": ["Build OAuth integration", "Support Google sign-in"],
    "decisions": ["Use OAuth 2.0", "Support Google"],
    "constraints": ["Must work offline"],
    "technical_notes": ["Extend existing auth module in src/auth/"]
  }
}
```"#;

        let response = parse_response(response_text).unwrap();
        assert!(response.is_complete);
        assert!(response.structured_spec.is_some());
        let spec = response.structured_spec.unwrap();
        assert!(spec.requirements[0].contains("OAuth"));
        assert_eq!(spec.decisions.len(), 2);
        assert_eq!(spec.technical_notes[0], "Extend existing auth module in src/auth/");
        let msg: serde_json::Value = serde_json::from_str(&response.message).unwrap();
        assert!(msg["observations"].as_str().unwrap().contains("Final summary"));
    }

    #[test]
    fn parse_structured_json_short_question_string() {
        let response_text = r#"```json
{
  "spec_complete": false,
  "observations": "Checked the repo.",
  "questions": "Why?"
}
```"#;

        let response = parse_response(response_text).unwrap();
        assert!(!response.is_complete);
        assert!(response.has_questions, "short question string should set has_questions=true");
        let msg: serde_json::Value = serde_json::from_str(&response.message).unwrap();
        assert_eq!(msg["questions"].as_str().unwrap(), "Why?");
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
        let msg: serde_json::Value = serde_json::from_str(&response.message).unwrap();
        assert!(msg["observations"].as_str().unwrap().contains("Explored the codebase"));
    }

    #[test]
    fn parse_structured_json_complete_missing_structured_spec_is_error() {
        let response_text = r#"```json
{
  "spec_complete": true,
  "observations": "I have everything I need."
}
```"#;

        let result = parse_response(response_text);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("structured_spec"));
    }

    #[test]
    fn parse_structured_json_raw_no_fence() {
        let response_text = r#"{"spec_complete": true, "structured_spec": {"requirements": ["Build auth"], "decisions": [], "constraints": []}}"#;

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
    "requirements": ["Build a user auth system with OAuth", "Support Google and GitHub login"],
    "decisions": ["Use OAuth 2.0", "Support Google and GitHub"],
    "constraints": ["Must work offline"],
    "technical_notes": ["Consider using passport.js", "Extend src/auth/index.ts"]
  }
}
```"#;

        let response = parse_response(response_text).unwrap();
        assert!(response.is_complete);
        assert!(response.structured_spec.is_some());
        let spec = response.structured_spec.unwrap();
        assert!(spec.requirements[0].contains("OAuth"));
        assert_eq!(spec.decisions.len(), 2);
    }

    #[test]
    fn parse_legacy_text_before_completion_json() {
        let response_text = r#"I've gathered all the information needed.

```json
{
  "spec_complete": true,
  "structured_spec": {
    "requirements": ["Build feature X"],
    "decisions": [],
    "constraints": []
  }
}
```"#;

        let response = parse_response(response_text).unwrap();
        assert!(response.is_complete);
        assert!(response.structured_spec.is_some());
    }

    #[test]
    fn parse_legacy_completion_without_observations() {
        let response_text = r#"```json
{
  "spec_complete": true,
  "structured_spec": {
    "requirements": ["Build feature X"],
    "decisions": [],
    "constraints": []
  }
}
```"#;

        let response = parse_response(response_text).unwrap();
        assert!(response.is_complete);
        let msg: serde_json::Value = serde_json::from_str(&response.message).unwrap();
        assert_eq!(msg["observations"].as_str().unwrap(), "");
    }

    #[test]
    fn parse_legacy_with_technical_notes() {
        let response_text = r#"```json
{
  "spec_complete": true,
  "structured_spec": {
    "requirements": ["Build auth"],
    "decisions": ["Use JWT"],
    "constraints": ["Must be fast"],
    "technicalNotes": ["Consider using middleware pattern", "Follow src/middleware/auth.ts"]
  }
}
```"#;

        let response = parse_response(response_text).unwrap();
        assert!(response.is_complete);
        let spec = response.structured_spec.unwrap();
        assert_eq!(spec.technical_notes[0], "Consider using middleware pattern");
        assert_eq!(spec.technical_notes.len(), 2);
    }

    #[test]
    fn parse_legacy_nested_json_in_notes() {
        let response_text = r#"```json
{
  "spec_complete": true,
  "structured_spec": {
    "requirements": ["Build API"],
    "decisions": ["RESTful design"],
    "constraints": ["Must handle {nested} braces"],
    "technicalNotes": ["Use pattern: { key: value }"]
  }
}
```"#;

        let response = parse_response(response_text).unwrap();
        assert!(response.is_complete);
        let spec = response.structured_spec.unwrap();
        assert!(spec.constraints[0].contains("{nested}"));
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

    // === Multiple unfenced JSON objects ===

    #[test]
    fn parse_spec_complete_not_first_unfenced_object() {
        let response_text = concat!(
            "I analyzed the codebase. Here's a summary: ",
            r#"{"analysis": "done", "files": 42}"#,
            "\n\nBased on my analysis:\n\n",
            r#"{"spec_complete": false, "observations": "Found auth patterns.", "questions": "Which provider do you want?"}"#,
        );

        let response = parse_response(response_text).unwrap();
        assert!(!response.is_complete);
        assert!(response.has_questions);
        let msg: serde_json::Value = serde_json::from_str(&response.message).unwrap();
        assert!(msg["observations"].as_str().unwrap().contains("auth patterns"));
        assert!(msg["questions"].as_str().unwrap().contains("Which provider"));
    }

    #[test]
    fn parse_spec_complete_not_first_unfenced_object_complete() {
        let response_text = concat!(
            r#"{"status": "analyzed"}"#,
            " some text ",
            r#"{"spec_complete": true, "structured_spec": {"requirements": ["Build auth"], "decisions": ["Use JWT"], "constraints": []}}"#,
        );

        let response = parse_response(response_text).unwrap();
        assert!(response.is_complete);
        assert!(response.structured_spec.is_some());
        let spec = response.structured_spec.unwrap();
        assert!(spec.requirements[0].contains("auth"));
    }

    // === Pretty-printed raw JSON tests (no code fence) ===

    #[test]
    fn parse_raw_pretty_printed_json_completion() {
        // Agent outputs pretty-printed JSON without code fences — the primary bug case.
        let response_text = r#"{
  "spec_complete": true,
  "observations": "All decisions made.",
  "structured_spec": {
    "requirements": ["Build a real-time app", "Support 1000 concurrent users"],
    "decisions": ["Use WebSockets"],
    "constraints": ["Must scale to 1000 users"],
    "technical_notes": ["Extend existing socket module in src/ws/", "Follow pattern in src/server.ts"]
  }
}"#;

        let response = parse_response(response_text).unwrap();
        assert!(response.is_complete, "pretty-printed raw JSON should be detected as complete");
        assert!(response.structured_spec.is_some());
        let spec = response.structured_spec.unwrap();
        assert!(spec.requirements[0].contains("real-time"));
        assert_eq!(spec.decisions.len(), 1);
        assert_eq!(spec.technical_notes[0], "Extend existing socket module in src/ws/");
    }

    #[test]
    fn parse_raw_pretty_printed_json_with_preamble() {
        // Agent outputs preamble text before pretty-printed JSON — common pattern.
        let response_text = concat!(
            "Now I have comprehensive research. Let me synthesize this into the final spec.\n\n",
            "{\n",
            "  \"spec_complete\": true,\n",
            "  \"observations\": \"All key technical decisions have been made.\",\n",
            "  \"structured_spec\": {\n",
            "    \"requirements\": [\"Build a cross-platform desktop app\", \"Support macOS and Windows\"],\n",
            "    \"decisions\": [\"Use Tauri v2\", \"Use React and TypeScript\"],\n",
            "    \"constraints\": [\"Must support macOS and Windows\"],\n",
            "    \"technical_notes\": [\"Extend existing patterns in src/\", \"Follow src/components/App.tsx\"]\n",
            "  }\n",
            "}",
        );

        let response = parse_response(response_text).unwrap();
        assert!(response.is_complete, "pretty-printed raw JSON with preamble should be detected as complete");
        assert!(response.structured_spec.is_some());
        let spec = response.structured_spec.unwrap();
        assert!(spec.requirements[0].contains("cross-platform"));
        assert_eq!(spec.decisions.len(), 2);
        assert_eq!(spec.technical_notes[0], "Extend existing patterns in src/");
    }

    #[test]
    fn parse_raw_pretty_printed_json_not_complete() {
        // Pretty-printed raw JSON with questions, not complete.
        let response_text = r#"{
  "spec_complete": false,
  "observations": "Found existing auth patterns.",
  "questions": "1. Which provider?\n   - A) Google\n   - B) GitHub"
}"#;

        let response = parse_response(response_text).unwrap();
        assert!(!response.is_complete);
        assert!(response.has_questions);
        let msg: serde_json::Value = serde_json::from_str(&response.message).unwrap();
        assert!(msg["questions"].as_str().unwrap().contains("Which provider"));
    }

    #[test]
    fn parse_json_fence_with_nested_backticks_in_array_values() {
        // Regression test: agent wraps the spec in ```json...``` and puts code examples
        // (with their own ``` fences) inside technical_notes strings. The naive
        // find("```") extractor would stop at the inner fence and return truncated,
        // unparseable JSON. Brace-matching must return the full object.
        let response_text = concat!(
            "```json\n",
            "{\n",
            "  \"spec_complete\": true,\n",
            "  \"observations\": \"Explored codebase.\",\n",
            "  \"structured_spec\": {\n",
            "    \"requirements\": [\"Expose GET /health returning JSON\"],\n",
            "    \"decisions\": [\"Use Gin framework\"],\n",
            "    \"constraints\": [\"No existing go.mod\"],\n",
            "    \"technical_notes\": [\n",
            "      \"Run: go mod init github.com/org/repo\",\n",
            "      \"Create cmd/smee/main.go as entry point\"\n",
            "    ]\n",
            "  }\n",
            "}\n",
            "```",
        );

        let response = parse_response(response_text).unwrap();
        assert!(response.is_complete, "spec_complete JSON inside a code fence must be detected as complete");
        let spec = response.structured_spec.unwrap();
        assert_eq!(spec.requirements[0], "Expose GET /health returning JSON");
        assert_eq!(spec.technical_notes.len(), 2);
        assert!(spec.technical_notes[0].contains("go mod init"));
    }
}
