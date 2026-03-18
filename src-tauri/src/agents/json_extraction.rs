//! Shared utilities for extracting JSON from agent text responses.
//!
//! Agents typically return JSON embedded in markdown code fences, surrounded by
//! preamble/postamble text, or as raw JSON. This module provides a single set
//! of extraction functions used across spec_discovery, planner, plan-validation,
//! auto-pilot, and other subsystems.

use serde::de::DeserializeOwned;

/// Extract the content of the first markdown code fence from the text.
///
/// Handles ` ```json `, plain ` ``` `, and both `\n` / `\r\n` line endings.
/// Returns the trimmed content inside the fence without parsing it.
///
/// For ` ```json ` fences, brace-matching is tried first so that triple-backtick
/// sequences *inside* JSON string values (e.g. code examples in spec fields) do
/// not prematurely terminate the extraction. The closing-fence string search is
/// kept as a fallback for non-JSON content inside the fence.
fn extract_json_code_block(text: &str) -> Option<String> {
    if let Some(fence_start) = text.find("```json") {
        let content_start = fence_start + 7;
        let content_start = skip_newline(text, content_start);
        let remaining = &text[content_start..];
        // Prefer balanced brace/bracket matching: immune to backticks inside JSON strings.
        // Guard with starts_with so preamble text that contains { or } does not cause
        // find_balanced to latch onto the wrong delimiter; fall through to fence-search instead.
        if remaining.trim_start().starts_with('{') {
            if let Some(json) = find_balanced(remaining, '{', '}') {
                return Some(json);
            }
        } else if remaining.trim_start().starts_with('[') {
            if let Some(json) = find_balanced(remaining, '[', ']') {
                return Some(json);
            }
        }
        // Fallback: closing-fence string search (original behaviour for non-JSON fences).
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
/// 3. Bracket-finding (object `{...}` or array `[...]`) + parse, trying
///    successive positions if the first balanced match doesn't deserialize.
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

    if let Some(val) = find_balanced_and_parse::<T>(trimmed, '{', '}') {
        return Some(val);
    }
    if let Some(val) = find_balanced_and_parse::<T>(trimmed, '[', ']') {
        return Some(val);
    }

    None
}

/// Try successive balanced-bracket matches until one deserializes as `T`.
///
/// Unlike a single `find_balanced` call, this handles text where prose
/// contains brackets before the actual JSON (e.g. `"Based on [the analysis]...
/// [{"command":"cleanup"}]"`).
fn find_balanced_and_parse<T: DeserializeOwned>(
    text: &str,
    open: char,
    close: char,
) -> Option<T> {
    let mut search_from = 0;
    while search_from < text.len() {
        if let Some(candidate) = find_balanced_from(text, search_from, open, close) {
            if let Ok(val) = serde_json::from_str::<T>(&candidate.matched) {
                return Some(val);
            }
            search_from = candidate.end_byte;
        } else {
            break;
        }
    }
    None
}

/// Extract the content of *all* markdown code fences from the text.
///
/// Like `extract_json_code_block` but returns every fenced block instead of
/// just the first. Handles ` ```json `, plain ` ``` ` fences, and `<json>` tags.
///
/// Uses balanced brace-matching for JSON content so that triple-backtick
/// sequences *inside* JSON string values (e.g. code examples in task
/// descriptions) do not break extraction.
fn extract_all_json_code_blocks(text: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut pos = 0;

    while pos < text.len() {
        let remaining = &text[pos..];

        let backtick_pos = remaining.find("```");
        let json_tag_pos = remaining.find("<json>");

        // Pick whichever fence-like opening comes first.
        enum FenceKind {
            Backtick,
            JsonTag,
        }
        let (kind, offset) = match (backtick_pos, json_tag_pos) {
            (Some(bp), Some(jp)) if bp <= jp => (FenceKind::Backtick, bp),
            (Some(_), Some(jp)) => (FenceKind::JsonTag, jp),
            (Some(bp), None) => (FenceKind::Backtick, bp),
            (None, Some(jp)) => (FenceKind::JsonTag, jp),
            (None, None) => break,
        };

        let abs_pos = pos + offset;

        match kind {
            FenceKind::Backtick => {
                let after_backticks = abs_pos + 3;
                if after_backticks > text.len() {
                    break;
                }
                let content_after = &text[after_backticks..];

                // Determine where the body content begins (after the fence tag + newline).
                let content_start = if content_after.starts_with("json") {
                    skip_newline(text, after_backticks + 4)
                } else if content_after.starts_with('\n') {
                    after_backticks + 1
                } else if content_after.starts_with("\r\n") {
                    after_backticks + 2
                } else {
                    // Not a recognized fence opening (e.g. ```sql); skip past.
                    pos = after_backticks;
                    continue;
                };

                if let Some(extracted) = try_extract_json_body(text, content_start) {
                    pos = extracted.resume_from;
                    results.push(extracted.json);
                    continue;
                }

                pos = content_start;
            }
            FenceKind::JsonTag => {
                let content_start = skip_newline(text, abs_pos + 6);

                if let Some(extracted) = try_extract_json_body(text, content_start) {
                    pos = extracted.resume_from;
                    results.push(extracted.json);
                    continue;
                }

                pos = content_start;
            }
        }
    }

    results
}

struct ExtractedJson {
    json: String,
    resume_from: usize,
}

/// Try to extract a JSON body starting at `content_start` in `text`.
///
/// Uses balanced brace/bracket matching when the content opens with `{` or `[`.
/// Falls back to a closing-fence search for other content.
fn try_extract_json_body(text: &str, content_start: usize) -> Option<ExtractedJson> {
    let body = &text[content_start..];
    let trimmed = body.trim_start();

    if trimmed.starts_with('{') {
        if let Some(m) = find_balanced_from(text, content_start, '{', '}') {
            return Some(ExtractedJson {
                json: m.matched,
                resume_from: skip_closing_fence(text, m.end_byte),
            });
        }
    } else if trimmed.starts_with('[') {
        if let Some(m) = find_balanced_from(text, content_start, '[', ']') {
            return Some(ExtractedJson {
                json: m.matched,
                resume_from: skip_closing_fence(text, m.end_byte),
            });
        }
    }

    // Fallback: closing-fence search for non-JSON fenced content.
    if let Some(end_off) = body.find("```") {
        let block = body[..end_off].trim();
        if !block.is_empty() {
            return Some(ExtractedJson {
                json: block.to_string(),
                resume_from: content_start + end_off + 3,
            });
        }
    }

    None
}

/// After extracting JSON via brace-matching, skip past a trailing closing
/// fence (` ``` ` or `</json>`) so it isn't misinterpreted as a new opening.
fn skip_closing_fence(text: &str, pos: usize) -> usize {
    let remaining = &text[pos..];
    let trimmed = remaining.trim_start();
    if trimmed.starts_with("```") {
        let whitespace_len = remaining.len() - trimmed.len();
        pos + whitespace_len + 3
    } else if trimmed.starts_with("</json>") {
        let whitespace_len = remaining.len() - trimmed.len();
        pos + whitespace_len + 7
    } else {
        pos
    }
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
        let mut search_pos = 0;
        while search_pos < text.len() {
            if let Some(m) = find_balanced_from(text, search_pos, '{', '}') {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&m.matched) {
                    if v.is_object() {
                        results.push(v);
                    }
                }
                search_pos = m.end_byte;
            } else {
                break;
            }
        }
    }

    results
}

struct BalancedMatch {
    matched: String,
    /// Byte offset just past the closing delimiter — used to resume searching.
    end_byte: usize,
}

/// Find a balanced pair of open/close characters starting at or after `from_byte`.
///
/// Skips over characters inside JSON string literals (`"..."`) so that
/// braces/brackets embedded in string values (e.g. `{"msg": "missing }"}`)
/// do not cause a premature depth-zero match.
fn find_balanced_from(
    text: &str,
    from_byte: usize,
    open: char,
    close: char,
) -> Option<BalancedMatch> {
    let slice = &text[from_byte..];
    let start = slice.find(open)?;
    let abs_start = from_byte + start;
    let mut depth = 0;
    let mut in_string = false;
    let mut escape_next = false;
    for (i, c) in text[abs_start..].char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }
        if in_string {
            if c == '\\' {
                escape_next = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        if c == '"' {
            in_string = true;
            continue;
        }
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                let end = abs_start + i + c.len_utf8();
                return Some(BalancedMatch {
                    matched: text[abs_start..end].to_string(),
                    end_byte: end,
                });
            }
        }
    }
    None
}

/// Find the first balanced pair of open/close characters using depth counting.
///
/// Starts from the first `open` character and walks forward, incrementing
/// depth on `open` and decrementing on `close`. Returns the substring from
/// the opening character to its matching close (inclusive).
fn find_balanced(text: &str, open: char, close: char) -> Option<String> {
    find_balanced_from(text, 0, open, close).map(|m| m.matched)
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
    fn code_block_json_with_nested_backticks_in_string_value() {
        // Agent outputs a JSON code fence whose string values contain sub-fences
        // (e.g. code examples in spec technical_notes). The naive find("```") would
        // stop at the inner fence; brace-matching must return the full object.
        let text = concat!(
            "```json\n",
            "{\n",
            "  \"spec_complete\": true,\n",
            "  \"notes\": [\"Create main.go with:\\n```go\\npackage main\\n```\"]\n",
            "}\n",
            "```",
        );
        let result = extract_json_code_block(text).expect("should extract full JSON object");
        assert!(result.contains("spec_complete"), "must contain spec_complete key");
        assert!(result.contains("notes"), "must contain notes key");
        // Verify the extracted string is valid JSON
        serde_json::from_str::<serde_json::Value>(&result)
            .expect("extracted content must be valid JSON");
    }

    #[test]
    fn code_block_json_fence_array_with_nested_backticks() {
        // Covers the new `[`-prefixed brace-matching path inside a ```json fence.
        // The naive find("```") would stop at the inner ``` and return a truncated array.
        let text = concat!(
            "```json\n",
            "[\n",
            "  {\"cmd\": \"go build\", \"example\": \"```go\\npackage main\\n```\"},\n",
            "  {\"cmd\": \"go test\"}\n",
            "]\n",
            "```",
        );
        let result = extract_json_code_block(text).expect("should extract full JSON array");
        let parsed = serde_json::from_str::<serde_json::Value>(&result)
            .expect("extracted content must be valid JSON");
        let arr = parsed.as_array().expect("must be an array");
        assert_eq!(arr.len(), 2, "both array elements must be present");
        assert_eq!(arr[0]["cmd"].as_str().unwrap(), "go build");
        assert_eq!(arr[1]["cmd"].as_str().unwrap(), "go test");
    }

    #[test]
    fn code_block_json_fence_non_json_content_uses_fence_search_fallback() {
        // When the content after ```json doesn't start with { or [, brace-matching
        // is skipped and we fall through to the original closing-``` search.
        // This ensures the fallback path is exercised.
        let text = "```json\n\"a plain string value\"\n```\ntrailing text";
        let result = extract_json_code_block(text).expect("fence-search fallback should find content");
        assert_eq!(result, "\"a plain string value\"");
    }

    #[test]
    fn code_block_json_fence_preamble_with_braces_uses_fence_search() {
        // If preamble text before the JSON contains { or }, brace-matching would
        // latch onto the wrong delimiter and return garbage. The trim_start guard
        // must detect that content doesn't open with { and skip to fence-search.
        let text = "```json\nThe schema {x: y} is:\n{\"real\": true}\n```";
        let result = extract_json_code_block(text).expect("fence-search fallback should return content");
        assert!(result.contains("{\"real\": true}"), "result must include the JSON object");
        assert!(result.contains("schema"), "result includes preamble returned by fence-search");
    }

    #[test]
    fn code_block_json_fence_brace_match_fallback_when_unbalanced() {
        // If the content starts with { but brace-matching finds no closing brace
        // (malformed JSON), we fall through to closing-fence search so we still
        // return whatever was in the fence.
        let text = "```json\n{no closing brace\n```";
        let result = extract_json_code_block(text).expect("fence-search fallback should return content");
        assert_eq!(result, "{no closing brace");
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

    // ── find_balanced_and_parse (multi-match) ─────────────────────

    #[derive(serde::Deserialize, Debug, PartialEq)]
    struct Cmd {
        command: String,
        model: String,
    }

    #[test]
    fn parse_array_skips_prose_brackets() {
        let text = r#"Based on [the analysis], I recommend:
[{"command": "cleanup", "model": "sonnet-4.6"}]"#;
        let result: Option<Vec<Cmd>> = parse_json_response(text);
        let cmds = result.expect("should parse despite prose brackets");
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].command, "cleanup");
    }

    #[test]
    fn parse_array_skips_checkbox_brackets() {
        let text = r#"Checklist:
- [x] reviewed code
- [x] ran tests
[{"command": "unit-tests", "model": "opus-4.5"}]"#;
        let result: Option<Vec<Cmd>> = parse_json_response(text);
        let cmds = result.expect("should skip checkbox brackets");
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].command, "unit-tests");
    }

    #[test]
    fn parse_object_skips_prose_braces() {
        let text = r#"The result {summary} is:
{"command": "cleanup", "model": "sonnet-4.6"}"#;
        let result: Option<Cmd> = parse_json_response(text);
        let cmd = result.expect("should skip prose braces");
        assert_eq!(cmd.command, "cleanup");
    }

    #[test]
    fn parse_array_with_result_text_appended() {
        let text = r#"[{"command":"code-review","model":"opus-4.6"}]I selected code-review."#;
        let result: Option<Vec<Cmd>> = parse_json_response(text);
        let cmds = result.expect("should parse with trailing text");
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].command, "code-review");
    }

    #[test]
    fn parse_no_valid_match_returns_none() {
        let text = "No JSON here, just [prose] and {words}.";
        let result: Option<Vec<Cmd>> = parse_json_response(text);
        assert!(result.is_none());
    }

    #[test]
    fn find_balanced_from_skips_to_offset() {
        let text = r#"[x] then [{"a":1}]"#;
        let m = find_balanced_from(text, 4, '[', ']').expect("should find second match");
        assert_eq!(m.matched, r#"[{"a":1}]"#);
    }

    #[test]
    fn find_balanced_from_at_text_end_returns_none() {
        let text = "[1,2]";
        assert!(find_balanced_from(text, text.len(), '[', ']').is_none());
    }

    #[test]
    fn find_balanced_from_no_open_after_offset() {
        let text = "[first] nothing here";
        assert!(find_balanced_from(text, 8, '[', ']').is_none());
    }

    #[test]
    fn find_balanced_from_end_byte_is_correct() {
        let text = "[a][b][c]";
        let m = find_balanced_from(text, 0, '[', ']').unwrap();
        assert_eq!(m.matched, "[a]");
        assert_eq!(m.end_byte, 3);

        let m2 = find_balanced_from(text, m.end_byte, '[', ']').unwrap();
        assert_eq!(m2.matched, "[b]");
        assert_eq!(m2.end_byte, 6);

        let m3 = find_balanced_from(text, m2.end_byte, '[', ']').unwrap();
        assert_eq!(m3.matched, "[c]");
        assert_eq!(m3.end_byte, 9);

        assert!(find_balanced_from(text, m3.end_byte, '[', ']').is_none());
    }

    #[test]
    fn parse_skips_multiple_invalid_before_valid() {
        let text = r#"[x] and [y] and [z] finally [{"command":"cleanup","model":"s"}]"#;
        let result: Option<Vec<Cmd>> = parse_json_response(text);
        let cmds = result.expect("should find valid array after 3 invalid matches");
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].command, "cleanup");
    }

    #[test]
    fn find_balanced_and_parse_empty_text() {
        let result: Option<Vec<Cmd>> = find_balanced_and_parse("", '[', ']');
        assert!(result.is_none());
    }

    #[test]
    fn find_balanced_and_parse_object_skips_multiple_invalid() {
        let text = r#"see {x} and {y} then {"command":"review","model":"opus-4.6"}"#;
        let result: Option<Cmd> = find_balanced_and_parse(text, '{', '}');
        let cmd = result.expect("should find valid object after 2 invalid");
        assert_eq!(cmd.command, "review");
    }

    // ── nested backticks in JSON strings (primary bug fix) ────────

    #[test]
    fn all_blocks_nested_backticks_in_description() {
        // Reproduces the exact production bug: task descriptions contain
        // markdown code fences (```sql, ```go) inside JSON string values.
        // The old split("```") approach broke on these; brace-matching must
        // extract the full object.
        let text = concat!(
            "```json\n",
            "{\n",
            "  \"create_fix_tasks\": {\n",
            "    \"tasks\": [\n",
            "      {\n",
            "        \"title\": \"Fix SQL query\",\n",
            "        \"description\": \"The query is:\\n```sql\\nSELECT * FROM t\\n```\\nFix it.\"\n",
            "      }\n",
            "    ]\n",
            "  }\n",
            "}\n",
            "```",
        );
        let blocks = extract_all_json_code_blocks(text);
        assert_eq!(blocks.len(), 1, "must extract exactly one block");
        let parsed: serde_json::Value =
            serde_json::from_str(&blocks[0]).expect("extracted block must be valid JSON");
        let tasks = parsed["create_fix_tasks"]["tasks"]
            .as_array()
            .expect("must have tasks array");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0]["title"].as_str().unwrap(), "Fix SQL query");
    }

    #[test]
    fn all_blocks_multiple_nested_backtick_languages() {
        // Multiple code examples with different language tags inside one JSON block.
        let text = concat!(
            "```json\n",
            "{\n",
            "  \"create_fix_tasks\": {\n",
            "    \"tasks\": [\n",
            "      {\n",
            "        \"title\": \"Fix A\",\n",
            "        \"description\": \"See:\\n```sql\\nSELECT 1\\n```\\nAnd:\\n```go\\npackage main\\n```\"\n",
            "      },\n",
            "      {\n",
            "        \"title\": \"Fix B\",\n",
            "        \"description\": \"Run:\\n```bash\\necho hello\\n```\"\n",
            "      }\n",
            "    ]\n",
            "  }\n",
            "}\n",
            "```",
        );
        let blocks = extract_all_json_code_blocks(text);
        assert_eq!(blocks.len(), 1);
        let parsed: serde_json::Value = serde_json::from_str(&blocks[0]).unwrap();
        let tasks = parsed["create_fix_tasks"]["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0]["title"].as_str().unwrap(), "Fix A");
        assert_eq!(tasks[1]["title"].as_str().unwrap(), "Fix B");
    }

    #[test]
    fn parse_all_blocks_with_nested_backticks() {
        // End-to-end: parse_all_json_blocks must succeed despite nested fences.
        let text = concat!(
            "Creating task:\n",
            "```json\n",
            "{ \"create_fix_task\": { \"title\": \"Fix it\", \"description\": \"See:\\n```go\\nfmt.Println()\\n```\" } }\n",
            "```\n",
            "Done.",
        );
        let blocks = parse_all_json_blocks(text);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].get("create_fix_task").is_some());
    }

    // ── <json> tag support ────────────────────────────────────────

    #[test]
    fn all_blocks_json_tag() {
        let text = "<json>\n{\"a\":1}\n</json>";
        let blocks = extract_all_json_code_blocks(text);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0], "{\"a\":1}");
    }

    #[test]
    fn all_blocks_json_tag_with_backtick_close() {
        // LLM mixed <json> opening with ``` closing (observed in production).
        let text = "<json>\n{\"a\":1}\n```";
        let blocks = extract_all_json_code_blocks(text);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0], "{\"a\":1}");
    }

    #[test]
    fn all_blocks_json_tag_inline() {
        let text = "Result: <json>{\"ok\":true}</json> done.";
        let blocks = extract_all_json_code_blocks(text);
        assert_eq!(blocks.len(), 1);
        let parsed: serde_json::Value = serde_json::from_str(&blocks[0]).unwrap();
        assert_eq!(parsed["ok"].as_bool().unwrap(), true);
    }

    #[test]
    fn parse_all_blocks_json_tag_create_fix_tasks() {
        let text = concat!(
            "Creating tasks:\n\n",
            "<json>\n",
            "{ \"create_fix_tasks\": { \"tasks\": [\n",
            "  { \"title\": \"Task A\", \"description\": \"Do A\" },\n",
            "  { \"title\": \"Task B\", \"description\": \"Do B\" }\n",
            "] } }\n",
            "```\n",
            "\nTwo tasks created.",
        );
        let blocks = parse_all_json_blocks(text);
        assert_eq!(blocks.len(), 1);
        let tasks = blocks[0]["create_fix_tasks"]["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0]["title"].as_str().unwrap(), "Task A");
    }

    // ── string-aware brace matching ──────────────────────────────

    #[test]
    fn find_balanced_skips_close_brace_in_json_string() {
        let text = r#"{"msg": "missing }"}"#;
        let m = find_balanced(text, '{', '}').expect("must find full object");
        assert_eq!(m, text);
        serde_json::from_str::<serde_json::Value>(&m).expect("must be valid JSON");
    }

    #[test]
    fn find_balanced_skips_open_brace_in_json_string() {
        let text = r#"{"msg": "extra { here"}"#;
        let m = find_balanced(text, '{', '}').expect("must find full object");
        assert_eq!(m, text);
        serde_json::from_str::<serde_json::Value>(&m).expect("must be valid JSON");
    }

    #[test]
    fn find_balanced_handles_escaped_quote_before_brace() {
        // The \" inside the string is an escaped quote — not the end of string.
        // The } after it is still inside the string.
        let text = r#"{"msg": "say \"}\""}"#;
        let m = find_balanced(text, '{', '}').expect("must find full object");
        assert_eq!(m, text);
        serde_json::from_str::<serde_json::Value>(&m).expect("must be valid JSON");
    }

    #[test]
    fn find_balanced_handles_escaped_backslash_before_quote() {
        // \\\\ in raw string = two literal backslashes in the string.
        // In JSON: \\\\ decodes to \\. The " after it ends the string properly.
        let text = r#"{"path": "C:\\"}"#;
        let m = find_balanced(text, '{', '}').expect("must find full object");
        assert_eq!(m, text);
        serde_json::from_str::<serde_json::Value>(&m).expect("must be valid JSON");
    }

    #[test]
    fn find_balanced_multiple_braces_in_string() {
        let text = r#"{"tpl": "{{.Name}}"}"#;
        let m = find_balanced(text, '{', '}').expect("must find full object");
        assert_eq!(m, text);
        serde_json::from_str::<serde_json::Value>(&m).expect("must be valid JSON");
    }

    #[test]
    fn parse_response_with_close_brace_in_string() {
        #[derive(serde::Deserialize)]
        struct S {
            msg: String,
        }
        let text = r#"Result: {"msg": "missing }"} done"#;
        let result: Option<S> = parse_json_response(text);
        assert_eq!(result.unwrap().msg, "missing }");
    }

    #[test]
    fn parse_all_blocks_bare_with_brace_in_string() {
        let text = r#"Here: {"msg": "a } b"} done"#;
        let blocks = parse_all_json_blocks(text);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["msg"].as_str().unwrap(), "a } b");
    }

    #[test]
    fn code_block_fence_with_brace_in_string_value() {
        let text = "```json\n{\"msg\": \"missing }\"}\n```";
        let result = extract_json_code_block(text).expect("should extract");
        serde_json::from_str::<serde_json::Value>(&result).expect("must be valid JSON");
        assert!(result.contains("missing }"));
    }

    #[test]
    fn all_blocks_fence_with_brace_in_string_value() {
        let text = "```json\n{\"desc\": \"fix } here\"}\n```";
        let blocks = extract_all_json_code_blocks(text);
        assert_eq!(blocks.len(), 1);
        serde_json::from_str::<serde_json::Value>(&blocks[0]).expect("must be valid JSON");
    }

    // ── multi-line bare JSON fallback ─────────────────────────────

    #[test]
    fn all_json_blocks_bare_multiline() {
        let text = "Here is the result:\n{\n  \"x\": 42\n}\nDone.";
        let blocks = parse_all_json_blocks(text);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["x"].as_i64().unwrap(), 42);
    }

    #[test]
    fn all_json_blocks_bare_multiple_objects() {
        let text = "First: {\"a\":1} and second: {\"b\":2}";
        let blocks = parse_all_json_blocks(text);
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].get("a").is_some());
        assert!(blocks[1].get("b").is_some());
    }
}
