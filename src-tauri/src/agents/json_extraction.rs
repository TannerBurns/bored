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
fn extract_json_code_block(text: &str) -> Option<String> {
    if let Some(fence_start) = text.find("```json") {
        let content_start = fence_start + 7;
        let content_start = skip_newline(text, content_start);
        if let Some(end_offset) = text[content_start..].find("```") {
            let content = text[content_start..content_start + end_offset].trim();
            if !content.is_empty() {
                return Some(content.to_string());
            }
        }
    }

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
/// Tries code-fence extraction first, then falls back to depth-counting
/// brace matching starting from the first `{`.
pub fn extract_json_object(text: &str) -> Option<String> {
    if let Some(block) = extract_json_code_block(text) {
        if block.starts_with('{') {
            return Some(block);
        }
    }

    let trimmed = text.trim();
    find_balanced(trimmed, '{', '}')
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

    if let Ok(val) = serde_json::from_str::<T>(trimmed) {
        return Some(val);
    }

    if let Some(block) = extract_json_code_block(trimmed) {
        if let Ok(val) = serde_json::from_str::<T>(&block) {
            return Some(val);
        }
    }

    if let Some(obj) = find_balanced(trimmed, '{', '}') {
        if let Ok(val) = serde_json::from_str::<T>(&obj) {
            return Some(val);
        }
    }
    if let Some(arr) = find_balanced(trimmed, '[', ']') {
        if let Ok(val) = serde_json::from_str::<T>(&arr) {
            return Some(val);
        }
    }

    None
}

/// Extract the content of *all* markdown code fences from the text.
///
/// Like `extract_json_code_block` but returns every fenced block instead of
/// just the first. Handles both ` ```json ` and plain ` ``` ` fences.
fn extract_all_json_code_blocks(text: &str) -> Vec<String> {
    let segments: Vec<&str> = text.split("```").collect();
    let mut results = Vec::new();
    for (i, segment) in segments.iter().enumerate() {
        if i % 2 == 0 {
            continue;
        }
        let content = segment.trim_start();
        let json_str = content
            .strip_prefix("json")
            .map(|s| s.trim())
            .unwrap_or(content.trim());
        if !json_str.is_empty() {
            results.push(json_str.to_string());
        }
    }
    results
}

/// Parse all JSON blocks from agent text, returning parsed `serde_json::Value`s.
///
/// Tries fenced code blocks first (` ```json ... ``` `). If no valid JSON is
/// found in fences, falls back to scanning for bare `{ ... }` JSON objects on
/// individual lines. This covers agents that omit fences.
pub fn parse_all_json_blocks(text: &str) -> Vec<serde_json::Value> {
    let mut results = Vec::new();

    for block in extract_all_json_code_blocks(text) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&block) {
            results.push(v);
        }
    }

    if results.is_empty() {
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('{') && trimmed.ends_with('}') {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    if v.is_object() {
                        results.push(v);
                    }
                }
            }
        }
    }

    results
}

/// Find the first balanced pair of open/close characters using depth counting.
///
/// Starts from the first `open` character and walks forward, incrementing
/// depth on `open` and decrementing on `close`. Returns the substring from
/// the opening character to its matching close (inclusive).
fn find_balanced(text: &str, open: char, close: char) -> Option<String> {
    let start = text.find(open)?;
    let mut depth = 0;
    for (i, c) in text[start..].char_indices() {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(text[start..start + i + c.len_utf8()].to_string());
            }
        }
    }
    None
}

fn skip_newline(text: &str, pos: usize) -> usize {
    if pos >= text.len() {
        return pos;
    }
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

    // ── extract_json_code_block ────────────────────────────────

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

    // ── extract_json_object ────────────────────────────────────

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

    // ── parse_json_response ────────────────────────────────────

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

    // ── edge cases ────────────────────────────────────────────

    #[test]
    fn object_falls_through_when_fence_has_array() {
        let text = "stray text {\"real\":true} more\n```json\n[1,2]\n```";
        assert_eq!(
            extract_json_object(text),
            Some("{\"real\":true}".to_string())
        );
    }

    #[test]
    fn parse_falls_back_to_bracket_when_fence_invalid() {
        #[derive(serde::Deserialize)]
        struct S {
            ok: bool,
        }
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
    fn object_multiple_independent_objects() {
        let text = r#"{"a":1} some text {"b":2}"#;
        assert_eq!(
            extract_json_object(text),
            Some(r#"{"a":1}"#.to_string())
        );
    }

    #[test]
    fn object_multiple_objects_with_preamble() {
        let text = "Here is the result:\n{\"spec_complete\":true} and also {\"other\":false}";
        let result = extract_json_object(text).unwrap();
        assert!(result.contains("spec_complete"));
        assert!(!result.contains("other"));
    }

    #[test]
    fn parse_multiple_objects_picks_first_valid() {
        #[derive(serde::Deserialize)]
        struct S {
            x: i32,
        }
        let text = r#"preamble {"x":42} middle {"x":99} end"#;
        let result: Option<S> = parse_json_response(text);
        assert_eq!(result.unwrap().x, 42);
    }

    #[test]
    fn parse_multiple_arrays_picks_first_valid() {
        let text = "result: [1,2] and also [3,4]";
        let result: Option<Vec<i32>> = parse_json_response(text);
        assert_eq!(result.unwrap(), vec![1, 2]);
    }

    #[test]
    fn find_balanced_unmatched_returns_none() {
        assert_eq!(extract_json_object("{"), None);
    }

    #[test]
    fn find_balanced_with_multibyte_content() {
        let text = r#"{"emoji":"🎉","text":"héllo"}"#;
        let result = extract_json_object(text);
        assert_eq!(result, Some(text.to_string()));
    }

    #[test]
    fn find_balanced_multibyte_delimiters() {
        let result = find_balanced("before «inner» after", '«', '»');
        assert_eq!(result, Some("«inner»".to_string()));
    }

    #[test]
    fn find_balanced_nested_multibyte_delimiters() {
        let result = find_balanced("«outer «inner» end»", '«', '»');
        assert_eq!(result, Some("«outer «inner» end»".to_string()));
    }

    #[test]
    fn skip_newline_at_text_end() {
        let text = "some text```json";
        let pos = text.len();
        assert_eq!(skip_newline(text, pos), pos);
    }

    #[test]
    fn skip_newline_beyond_text_end() {
        let text = "short";
        assert_eq!(skip_newline(text, text.len() + 5), text.len() + 5);
    }

    #[test]
    fn code_block_text_ending_at_json_fence() {
        let text = "some text```json";
        assert_eq!(extract_json_code_block(text), None);
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
    fn extract_object_empty_braces() {
        assert_eq!(extract_json_object("{}"), Some("{}".to_string()));
    }

    // ── extract_all_json_code_blocks ────────────────────────────

    #[test]
    fn all_blocks_single_fence() {
        let text = "```json\n{\"a\":1}\n```";
        let blocks = extract_all_json_code_blocks(text);
        assert_eq!(blocks, vec!["{\"a\":1}"]);
    }

    #[test]
    fn all_blocks_multiple_fences() {
        let text = "Step 1:\n```json\n{\"a\":1}\n```\nStep 2:\n```json\n{\"b\":2}\n```";
        let blocks = extract_all_json_code_blocks(text);
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].contains("\"a\""));
        assert!(blocks[1].contains("\"b\""));
    }

    #[test]
    fn all_blocks_plain_fence() {
        let text = "```\n{\"x\":1}\n```";
        let blocks = extract_all_json_code_blocks(text);
        assert_eq!(blocks, vec!["{\"x\":1}"]);
    }

    #[test]
    fn all_blocks_empty_fence_skipped() {
        let text = "```json\n\n```";
        let blocks = extract_all_json_code_blocks(text);
        assert!(blocks.is_empty());
    }

    #[test]
    fn all_blocks_no_fences() {
        let blocks = extract_all_json_code_blocks("no fences here");
        assert!(blocks.is_empty());
    }

    // ── parse_all_json_blocks ──────────────────────────────────

    #[test]
    fn all_json_blocks_from_fenced() {
        let text = r#"Here is the plan:
```json
{ "start_app": { "command": "npm run dev" } }
```
Done."#;
        let blocks = parse_all_json_blocks(text);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].get("start_app").is_some());
    }

    #[test]
    fn all_json_blocks_multiple() {
        let text = r#"Step 1:
```json
{ "run_command": { "command": "npm install" } }
```
Step 2:
```json
{ "start_app": { "command": "npm run dev" } }
```"#;
        let blocks = parse_all_json_blocks(text);
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn all_json_blocks_bare_fallback() {
        let text = r#"I will start the app now.
{ "start_app": { "command": "npm run dev", "port": 3000 } }
That should work."#;
        let blocks = parse_all_json_blocks(text);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].get("start_app").is_some());
    }

    #[test]
    fn all_json_blocks_bare_skips_non_object() {
        let text = "42\n\"hello\"";
        let blocks = parse_all_json_blocks(text);
        assert!(blocks.is_empty());
    }

    #[test]
    fn all_json_blocks_no_json_returns_empty() {
        let blocks = parse_all_json_blocks("Just some text with no JSON.");
        assert!(blocks.is_empty());
    }

    #[test]
    fn all_json_blocks_ignores_invalid_in_fence() {
        let text = "```json\n{ not valid }\n```";
        let blocks = parse_all_json_blocks(text);
        assert!(blocks.is_empty());
    }

    #[test]
    fn all_json_blocks_prefers_fenced_over_bare() {
        let text = r#"```json
{ "a": 1 }
```
{ "b": 2 }"#;
        let blocks = parse_all_json_blocks(text);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].get("a").is_some());
    }
}
