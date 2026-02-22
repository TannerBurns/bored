//! Shared utilities for extracting JSON from agent text responses.
//!
//! Agents typically return JSON embedded in markdown code fences, surrounded by
//! preamble/postamble text, or as raw JSON. This module provides a single set
//! of extraction functions used across brainstorm, planner, plan-validation,
//! auto-pilot, and other subsystems.

use serde::de::DeserializeOwned;

/// Extract the content of the first markdown code fence from the text.
///
/// Handles ` ```json `, plain ` ``` `, and both `\n` / `\r\n` line endings.
/// Returns the trimmed content inside the fence without parsing it.
pub fn extract_json_code_block(text: &str) -> Option<String> {
    // Strategy 1: ```json ... ```
    if let Some(fence_start) = text.find("```json") {
        let content_start = fence_start + 7; // len("```json")
        // Skip optional newline after the marker
        let content_start = skip_newline(text, content_start);
        if let Some(end_offset) = text[content_start..].find("```") {
            let content = text[content_start..content_start + end_offset].trim();
            if !content.is_empty() {
                return Some(content.to_string());
            }
        }
    }

    // Strategy 2: plain ``` blocks that start with { or [
    for pattern in &["```\n", "```\r\n"] {
        if let Some(fence_start) = text.find(pattern) {
            let content_start = fence_start + pattern.len();
            if let Some(end_offset) = text[content_start..].find("```") {
                let content = text[content_start..content_start + end_offset].trim();
                if !content.is_empty()
                    && (content.starts_with('{') || content.starts_with('['))
                {
                    return Some(content.to_string());
                }
            }
        }
    }

    None
}

/// Extract the first JSON object (`{...}`) from agent text.
///
/// Tries code-fence extraction first, then falls back to bracket-finding
/// (first `{` to last `}`).
pub fn extract_json_object(text: &str) -> Option<String> {
    if let Some(block) = extract_json_code_block(text) {
        if block.starts_with('{') {
            return Some(block);
        }
    }

    let trimmed = text.trim();
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end > start {
        Some(trimmed[start..=end].to_string())
    } else {
        None
    }
}

/// Extract the first JSON array (`[...]`) from agent text.
///
/// Tries code-fence extraction first, then falls back to bracket-finding
/// (first `[` to last `]`).
pub fn extract_json_array(text: &str) -> Option<String> {
    if let Some(block) = extract_json_code_block(text) {
        if block.starts_with('[') {
            return Some(block);
        }
    }

    let trimmed = text.trim();
    let start = trimmed.find('[')?;
    let end = trimmed.rfind(']')?;
    if end > start {
        Some(trimmed[start..=end].to_string())
    } else {
        None
    }
}

/// Parse a JSON response of type `T` from agent text.
///
/// Tries strategies in order:
/// 1. Direct `serde_json::from_str` on the full text
/// 2. Code-fence extraction + parse
/// 3. Bracket-finding (object `{...}` or array `[...]`) + parse
///
/// Returns `None` if all strategies fail.
pub fn parse_json_response<T: DeserializeOwned>(text: &str) -> Option<T> {
    let trimmed = text.trim();

    // 1. Direct parse
    if let Ok(val) = serde_json::from_str::<T>(trimmed) {
        return Some(val);
    }

    // 2. Code-fence extraction
    if let Some(block) = extract_json_code_block(trimmed) {
        if let Ok(val) = serde_json::from_str::<T>(&block) {
            return Some(val);
        }
    }

    // 3. Bracket-finding -- try object first, then array
    if let Some(obj) = bracket_extract(trimmed, '{', '}') {
        if let Ok(val) = serde_json::from_str::<T>(&obj) {
            return Some(val);
        }
    }
    if let Some(arr) = bracket_extract(trimmed, '[', ']') {
        if let Ok(val) = serde_json::from_str::<T>(&arr) {
            return Some(val);
        }
    }

    None
}

fn bracket_extract(text: &str, open: char, close: char) -> Option<String> {
    let start = text.find(open)?;
    let end = text.rfind(close)?;
    if end > start {
        Some(text[start..=end].to_string())
    } else {
        None
    }
}

fn skip_newline(text: &str, pos: usize) -> usize {
    if text[pos..].starts_with("\r\n") {
        pos + 2
    } else if text[pos..].starts_with('\n') {
        pos + 1
    } else {
        pos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- extract_json_code_block --

    #[test]
    fn code_block_json_fence() {
        let text = "prefix\n```json\n{\"key\":\"value\"}\n```\nsuffix";
        let result = extract_json_code_block(text);
        assert_eq!(result, Some("{\"key\":\"value\"}".to_string()));
    }

    #[test]
    fn code_block_plain_fence_object() {
        let text = "prefix\n```\n{\"key\":\"value\"}\n```\nsuffix";
        let result = extract_json_code_block(text);
        assert_eq!(result, Some("{\"key\":\"value\"}".to_string()));
    }

    #[test]
    fn code_block_plain_fence_array() {
        let text = "prefix\n```\n[1, 2, 3]\n```\nsuffix";
        let result = extract_json_code_block(text);
        assert_eq!(result, Some("[1, 2, 3]".to_string()));
    }

    #[test]
    fn code_block_crlf_line_endings() {
        let text = "prefix\r\n```json\r\n{\"k\":\"v\"}\r\n```\r\nsuffix";
        let result = extract_json_code_block(text);
        assert_eq!(result, Some("{\"k\":\"v\"}".to_string()));
    }

    #[test]
    fn code_block_none_when_absent() {
        assert_eq!(extract_json_code_block("no code block here"), None);
    }

    #[test]
    fn code_block_empty_fence_returns_none() {
        let text = "```json\n\n```";
        assert_eq!(extract_json_code_block(text), None);
    }

    // -- extract_json_object --

    #[test]
    fn object_from_code_fence() {
        let text = "Here:\n```json\n{\"a\":1}\n```\nDone";
        assert_eq!(extract_json_object(text), Some("{\"a\":1}".to_string()));
    }

    #[test]
    fn object_from_preamble() {
        let text = "Here is the result:\n{\"a\":1}\nDone!";
        assert_eq!(extract_json_object(text), Some("{\"a\":1}".to_string()));
    }

    #[test]
    fn object_none_when_no_braces() {
        assert_eq!(extract_json_object("no json here"), None);
    }

    #[test]
    fn object_prefers_code_fence_over_bracket_finding() {
        let text = "stray { brace\n```json\n{\"correct\":true}\n```\nmore } text";
        assert_eq!(
            extract_json_object(text),
            Some("{\"correct\":true}".to_string())
        );
    }

    // -- extract_json_array --

    #[test]
    fn array_from_code_fence() {
        let text = "```json\n[1,2,3]\n```";
        assert_eq!(extract_json_array(text), Some("[1,2,3]".to_string()));
    }

    #[test]
    fn array_from_preamble() {
        let text = "Result:\n[{\"a\":1}]\nDone";
        assert_eq!(extract_json_array(text), Some("[{\"a\":1}]".to_string()));
    }

    #[test]
    fn array_none_when_absent() {
        assert_eq!(extract_json_array("no arrays"), None);
    }

    // -- parse_json_response --

    #[test]
    fn parse_direct_json_object() {
        #[derive(serde::Deserialize)]
        struct S {
            key: String,
        }
        let result: Option<S> = parse_json_response(r#"{"key":"val"}"#);
        assert_eq!(result.unwrap().key, "val");
    }

    #[test]
    fn parse_direct_json_array() {
        let result: Option<Vec<i32>> = parse_json_response("[1,2,3]");
        assert_eq!(result.unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn parse_from_code_fence() {
        #[derive(serde::Deserialize)]
        struct S {
            x: i32,
        }
        let text = "Here:\n```json\n{\"x\":42}\n```\nDone";
        let result: Option<S> = parse_json_response(text);
        assert_eq!(result.unwrap().x, 42);
    }

    #[test]
    fn parse_from_bracket_finding() {
        #[derive(serde::Deserialize)]
        struct S {
            x: i32,
        }
        let text = "Preamble text\n{\"x\":99}\npostamble";
        let result: Option<S> = parse_json_response(text);
        assert_eq!(result.unwrap().x, 99);
    }

    #[test]
    fn parse_array_with_surrounding_text() {
        let text = "Recommended:\n[1,2]\nEnd.";
        let result: Option<Vec<i32>> = parse_json_response(text);
        assert_eq!(result.unwrap(), vec![1, 2]);
    }

    #[test]
    fn parse_returns_none_for_garbage() {
        let result: Option<serde_json::Value> = parse_json_response("not json at all");
        assert!(result.is_none());
    }

    #[test]
    fn parse_nested_braces() {
        #[derive(serde::Deserialize)]
        struct Inner {
            b: i32,
        }
        #[derive(serde::Deserialize)]
        struct Outer {
            a: Inner,
        }
        let text = r#"Result: {"a":{"b":5}} done"#;
        let result: Option<Outer> = parse_json_response(text);
        assert_eq!(result.unwrap().a.b, 5);
    }

    #[test]
    fn parse_empty_array_from_code_fence() {
        let text = "```json\n[]\n```";
        let result: Option<Vec<String>> = parse_json_response(text);
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn parse_prefers_direct_over_fence() {
        #[derive(serde::Deserialize, PartialEq, Debug)]
        struct S {
            v: i32,
        }
        let text = r#"{"v":1}"#;
        let result: Option<S> = parse_json_response(text);
        assert_eq!(result.unwrap().v, 1);
    }

    // -- edge cases --

    #[test]
    fn object_falls_through_when_fence_has_array() {
        // Code fence contains an array, but we want an object -- should use bracket-finding
        let text = "stray text {\"real\":true} more\n```json\n[1,2]\n```";
        assert_eq!(
            extract_json_object(text),
            Some("{\"real\":true}".to_string())
        );
    }

    #[test]
    fn array_falls_through_when_fence_has_object() {
        let text = "stray [1,2,3] text\n```json\n{\"obj\":true}\n```";
        assert_eq!(extract_json_array(text), Some("[1,2,3]".to_string()));
    }

    #[test]
    fn parse_falls_back_to_bracket_when_fence_invalid() {
        #[derive(serde::Deserialize)]
        struct S {
            ok: bool,
        }
        // Code fence has invalid JSON, but bracket-finding finds the valid one
        let text = "```json\nnot json!\n```\nanyway {\"ok\":true} here";
        let result: Option<S> = parse_json_response(text);
        assert!(result.unwrap().ok);
    }

    #[test]
    fn code_block_takes_first_fence() {
        let text = "```json\n{\"first\":1}\n```\ntext\n```json\n{\"second\":2}\n```";
        assert_eq!(
            extract_json_code_block(text),
            Some("{\"first\":1}".to_string())
        );
    }

    #[test]
    fn code_block_whitespace_only_fence_returns_none() {
        let text = "```json\n   \n```";
        assert_eq!(extract_json_code_block(text), None);
    }

    #[test]
    fn bracket_extract_same_position_returns_none() {
        // Only one brace character — start == end, so end > start is false
        assert_eq!(extract_json_object("{"), None);
    }

    #[test]
    fn parse_whitespace_padded_input() {
        #[derive(serde::Deserialize)]
        struct S {
            a: i32,
        }
        let text = "   \n  {\"a\":7}  \n  ";
        let result: Option<S> = parse_json_response(text);
        assert_eq!(result.unwrap().a, 7);
    }

    #[test]
    fn parse_pretty_printed_object_with_preamble() {
        #[derive(serde::Deserialize)]
        struct S {
            x: String,
        }
        let text = "Here is the result:\n\n{\n  \"x\": \"hello\"\n}\n\nDone.";
        let result: Option<S> = parse_json_response(text);
        assert_eq!(result.unwrap().x, "hello");
    }

    #[test]
    fn extract_array_empty_brackets() {
        assert_eq!(extract_json_array("[]"), Some("[]".to_string()));
    }

    #[test]
    fn extract_object_empty_braces() {
        assert_eq!(extract_json_object("{}"), Some("{}".to_string()));
    }
}
