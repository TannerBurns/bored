//! Code review functions for the workflow orchestrator.
//!
//! These functions operate on already-extracted plain text. The caller is
//! responsible for using the agent provider's `extract_text` to convert raw
//! agent output before passing it here.
//!
//! The code-review command is instructed to emit a fenced JSON block at the end
//! of its output. We try to parse that first; if it's missing or malformed we
//! fall back to the legacy `ISSUES_FOUND:` line-based parser.

/// A single issue from the structured JSON output.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CodeReviewIssue {
    pub title: String,
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub lines: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub description: String,
}

/// Structured output from the code-review command.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CodeReviewOutput {
    pub issues_found: usize,
    #[serde(default)]
    pub issues: Vec<CodeReviewIssue>,
}

/// Try to parse structured JSON output from a code-review response.
///
/// Looks for the last fenced ` ```json ... ``` ` block in the text and
/// attempts to deserialize it as `CodeReviewOutput`. If strict
/// deserialization fails (e.g. the LLM wrapped the object in an extra key
/// or omitted `issues_found`), a best-effort fallback extracts what it can
/// from the raw JSON value.
pub fn parse_structured_review(text: &str) -> Option<CodeReviewOutput> {
    let mut last_json_block: Option<&str> = None;

    let mut search_from = 0;
    while let Some(start) = text[search_from..].find("```json") {
        let abs_start = search_from + start + "```json".len();
        if let Some(end) = text[abs_start..].find("```") {
            let block = text[abs_start..abs_start + end].trim();
            last_json_block = Some(block);
            search_from = abs_start + end + 3;
        } else {
            break;
        }
    }

    let block = last_json_block?;

    if let Ok(output) = serde_json::from_str::<CodeReviewOutput>(block) {
        return Some(output);
    }

    parse_structured_review_fallback(block)
}

/// Best-effort extraction from a JSON block that doesn't match the strict
/// `CodeReviewOutput` schema. Handles two common LLM deviations:
///
/// 1. Wrapper objects — `{ "review": { "issues_found": …, "issues": … } }`
/// 2. Missing `issues_found` — derives from the `issues` array length
fn parse_structured_review_fallback(block: &str) -> Option<CodeReviewOutput> {
    let val: serde_json::Value = serde_json::from_str(block).ok()?;
    let obj = unwrap_to_inner_object(&val)?;

    let issues = obj
        .get("issues")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(issue_from_value).collect::<Vec<_>>())
        .unwrap_or_default();

    let issues_found = obj
        .get("issues_found")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(issues.len());

    if issues_found == 0 && issues.is_empty() && !obj.contains_key("issues_found") {
        return None;
    }

    Some(CodeReviewOutput {
        issues_found,
        issues,
    })
}

/// If the top-level object has no `issues` key but contains exactly one key
/// whose value is an object (e.g. `"review"`), unwrap to that inner object.
fn unwrap_to_inner_object(
    val: &serde_json::Value,
) -> Option<&serde_json::Map<String, serde_json::Value>> {
    let map = val.as_object()?;

    if !map.contains_key("issues") && !map.contains_key("issues_found") && map.len() == 1 {
        if let Some(inner) = map.values().next().and_then(|v| v.as_object()) {
            return Some(inner);
        }
    }

    Some(map)
}

/// Build a `CodeReviewIssue` from a JSON value, tolerating `files` (array)
/// in place of `file` (string).
fn issue_from_value(val: &serde_json::Value) -> Option<CodeReviewIssue> {
    let obj = val.as_object()?;
    let title = obj.get("title")?.as_str()?.to_string();

    let file = obj
        .get("file")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            obj.get("files")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_default();

    let lines = obj
        .get("lines")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let severity = obj
        .get("severity")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let description = obj
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    Some(CodeReviewIssue {
        title,
        file,
        lines,
        severity,
        description,
    })
}

/// Parse code review output for issue count.
///
/// Prefers structured JSON output; falls back to the legacy `ISSUES_FOUND:` line.
/// Handles markdown formatting (e.g. `**ISSUES_FOUND: 0**`).
pub fn parse_code_review_issues(text: &str) -> Option<usize> {
    if let Some(output) = parse_structured_review(text) {
        return Some(output.issues_found);
    }

    parse_issues_found_line(text)
}

/// Legacy parser: looks for `ISSUES_FOUND: N` anywhere in a line.
fn parse_issues_found_line(text: &str) -> Option<usize> {
    for line in text.lines() {
        let stripped = line.trim().trim_start_matches('*').trim_end_matches('*');
        if let Some(rest) = stripped.trim().strip_prefix("ISSUES_FOUND:") {
            if let Ok(n) = rest.trim().parse::<usize>() {
                return Some(n);
            }
        }
    }
    None
}

/// Extract the issues section from code review output.
///
/// Prefers the structured JSON issues list (formatted as markdown for the fix prompt);
/// falls back to extracting content between "## Issues Found" and "## Summary".
pub fn extract_issues_section(text: &str) -> String {
    if let Some(output) = parse_structured_review(text) {
        return extract_issues_from_structured_or_legacy(&output, text);
    }
    extract_issues_section_legacy(text)
}

/// Same as `extract_issues_section` but accepts an already-parsed output
/// to avoid re-parsing the JSON block.
pub fn extract_issues_with_parsed(parsed: Option<&CodeReviewOutput>, text: &str) -> String {
    if let Some(output) = parsed {
        return extract_issues_from_structured_or_legacy(output, text);
    }
    extract_issues_section_legacy(text)
}

fn extract_issues_from_structured_or_legacy(output: &CodeReviewOutput, text: &str) -> String {
    if !output.issues.is_empty() {
        return format_issues_as_markdown(&output.issues);
    }
    extract_issues_section_legacy(text)
}

fn format_issues_as_markdown(issues: &[CodeReviewIssue]) -> String {
    let mut md = String::new();
    for (i, issue) in issues.iter().enumerate() {
        if i > 0 {
            md.push('\n');
        }
        md.push_str(&format!("### Issue {}: {}\n", i + 1, issue.title));
        if !issue.file.is_empty() {
            md.push_str(&format!("- **File:** `{}`\n", issue.file));
        }
        if !issue.lines.is_empty() {
            md.push_str(&format!("- **Lines:** {}\n", issue.lines));
        }
        if !issue.severity.is_empty() {
            md.push_str(&format!("- **Severity:** {}\n", issue.severity));
        }
        if !issue.description.is_empty() {
            md.push_str(&format!("- **Description:** {}\n", issue.description));
        }
    }
    md
}

/// Legacy extractor: content between "## Issues Found" and "## Summary".
fn extract_issues_section_legacy(text: &str) -> String {
    let start_marker = "## Issues Found";
    let end_marker = "## Summary";

    if let Some(start_idx) = text.find(start_marker) {
        let issues_start = start_idx + start_marker.len();
        if let Some(end_idx) = text[issues_start..].find(end_marker) {
            return text[issues_start..issues_start + end_idx]
                .trim()
                .to_string();
        }
        return text[issues_start..].trim().to_string();
    }

    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_structured_review ───────────────────────────────────

    #[test]
    fn structured_review_parses_json_block() {
        let text = r#"Some markdown analysis.

```json
{
  "issues_found": 2,
  "issues": [
    {"title": "Bug A", "file": "src/a.rs", "lines": "10", "severity": "high", "description": "Oops"},
    {"title": "Bug B", "file": "src/b.rs", "lines": "20-25", "severity": "low", "description": "Minor"}
  ]
}
```"#;
        let result = parse_structured_review(text).unwrap();
        assert_eq!(result.issues_found, 2);
        assert_eq!(result.issues.len(), 2);
        assert_eq!(result.issues[0].title, "Bug A");
        assert_eq!(result.issues[1].severity, "low");
    }

    #[test]
    fn structured_review_parses_clean() {
        let text = "All good.\n\n```json\n{\"issues_found\": 0, \"issues\": []}\n```";
        let result = parse_structured_review(text).unwrap();
        assert_eq!(result.issues_found, 0);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn structured_review_takes_last_json_block() {
        let text = "```json\n{\"issues_found\": 99, \"issues\": []}\n```\n\nMore text.\n\n```json\n{\"issues_found\": 1, \"issues\": []}\n```";
        let result = parse_structured_review(text).unwrap();
        assert_eq!(result.issues_found, 1);
    }

    #[test]
    fn structured_review_returns_none_for_no_json() {
        assert!(parse_structured_review("Just plain text").is_none());
    }

    #[test]
    fn structured_review_returns_none_for_malformed_json() {
        let text = "```json\n{not valid json}\n```";
        assert!(parse_structured_review(text).is_none());
    }

    #[test]
    fn structured_review_tolerates_missing_optional_fields() {
        let text = "```json\n{\"issues_found\": 1, \"issues\": [{\"title\": \"X\"}]}\n```";
        let result = parse_structured_review(text).unwrap();
        assert_eq!(result.issues[0].file, "");
    }

    // ── parse_code_review_issues (prefers structured, falls back to legacy) ──

    #[test]
    fn parse_issues_prefers_structured_json() {
        let text = "ISSUES_FOUND: 5\n\n```json\n{\"issues_found\": 2, \"issues\": []}\n```";
        assert_eq!(parse_code_review_issues(text), Some(2));
    }

    #[test]
    fn parse_issues_falls_back_to_legacy() {
        assert_eq!(parse_code_review_issues("ISSUES_FOUND: 3"), Some(3));
    }

    #[test]
    fn parse_issues_found_zero() {
        assert_eq!(parse_code_review_issues("ISSUES_FOUND: 0"), Some(0));
    }

    #[test]
    fn parse_issues_found_with_surrounding_text() {
        let text = "Review complete.\n  ISSUES_FOUND: 5\nSee above.";
        assert_eq!(parse_code_review_issues(text), Some(5));
    }

    #[test]
    fn parse_issues_returns_none_when_missing() {
        assert_eq!(parse_code_review_issues("No issues marker here"), None);
    }

    #[test]
    fn parse_issues_returns_none_for_non_numeric() {
        assert_eq!(parse_code_review_issues("ISSUES_FOUND: many"), None);
    }

    #[test]
    fn parse_issues_empty_input() {
        assert_eq!(parse_code_review_issues(""), None);
    }

    #[test]
    fn parse_issues_markdown_bold() {
        assert_eq!(parse_code_review_issues("**ISSUES_FOUND: 0**"), Some(0));
    }

    #[test]
    fn parse_issues_markdown_bold_with_surrounding_text() {
        let text = "Summary of review.\n\n**ISSUES_FOUND: 3**\n\nDone.";
        assert_eq!(parse_code_review_issues(text), Some(3));
    }

    #[test]
    fn parse_issues_partial_bold() {
        assert_eq!(parse_code_review_issues("**ISSUES_FOUND: 7"), Some(7));
    }

    // ── extract_issues_section ────────────────────────────────────

    #[test]
    fn extract_issues_from_structured_json() {
        let text = r#"Some text.

```json
{
  "issues_found": 1,
  "issues": [{"title": "NPE risk", "file": "app.ts", "lines": "42", "severity": "high", "description": "Could be null"}]
}
```"#;
        let result = extract_issues_section(text);
        assert!(result.contains("### Issue 1: NPE risk"));
        assert!(result.contains("`app.ts`"));
        assert!(result.contains("high"));
    }

    #[test]
    fn extract_issues_falls_back_to_legacy_markers() {
        let text = "Preamble\n## Issues Found\n- bug A\n- bug B\n## Summary\nDone.";
        assert_eq!(extract_issues_section(text), "- bug A\n- bug B");
    }

    #[test]
    fn extract_section_no_summary_marker() {
        let text = "## Issues Found\n- bug A\n- bug B";
        assert_eq!(extract_issues_section(text), "- bug A\n- bug B");
    }

    #[test]
    fn extract_section_no_issues_marker_returns_full_text() {
        let text = "Just some text without markers";
        assert_eq!(extract_issues_section(text), text);
    }

    #[test]
    fn extract_section_empty_between_markers() {
        let text = "## Issues Found\n## Summary\nDone.";
        assert_eq!(extract_issues_section(text), "");
    }

    #[test]
    fn extract_issues_structured_clean_returns_full_text() {
        let text = "All clean.\n\n```json\n{\"issues_found\": 0, \"issues\": []}\n```";
        let result = extract_issues_section(text);
        assert_eq!(result, text);
    }

    #[test]
    fn extract_issues_structured_multiple_issues() {
        let text = r#"Analysis.

```json
{
  "issues_found": 2,
  "issues": [
    {"title": "NPE", "file": "a.rs", "lines": "1", "severity": "high", "description": "Null ref"},
    {"title": "Leak", "file": "b.rs", "lines": "2-5", "severity": "medium", "description": "Unclosed handle"}
  ]
}
```"#;
        let result = extract_issues_section(text);
        assert!(result.contains("### Issue 1: NPE"));
        assert!(result.contains("### Issue 2: Leak"));
        assert!(result.contains("`a.rs`"));
        assert!(result.contains("`b.rs`"));
        assert!(result.contains("high"));
        assert!(result.contains("medium"));
    }

    #[test]
    fn extract_issues_structured_partial_fields() {
        let text = "```json\n{\"issues_found\": 1, \"issues\": [{\"title\": \"Missing check\"}]}\n```";
        let result = extract_issues_section(text);
        assert!(result.contains("### Issue 1: Missing check"));
        assert!(!result.contains("**File:**"));
        assert!(!result.contains("**Lines:**"));
    }

    // ── fallback parsing ────────────────────────────────────────

    #[test]
    fn fallback_summary_instead_of_issues_found() {
        let text = r#"Review analysis.

```json
{
  "summary": "7 issues found",
  "issues": [
    {"id": 1, "title": "Bug A", "severity": "high", "file": "src/a.ts", "lines": "80-91", "type": "bug"},
    {"id": 2, "title": "Bug B", "severity": "low", "file": "src/b.ts", "lines": "10", "type": "edge-case"}
  ]
}
```"#;
        let result = parse_structured_review(text).unwrap();
        assert_eq!(result.issues_found, 2);
        assert_eq!(result.issues.len(), 2);
        assert_eq!(result.issues[0].title, "Bug A");
        assert_eq!(result.issues[0].file, "src/a.ts");
        assert_eq!(result.issues[1].severity, "low");
    }

    #[test]
    fn fallback_review_wrapper_with_files_array() {
        let text = r#"Code review.

```json
{
  "review": {
    "branch": "feature/x",
    "base": "origin/main",
    "files_reviewed": 16,
    "issues_found": 4,
    "issues": [
      {
        "id": 1,
        "title": "Missing refetch on param change",
        "severity": "high",
        "type": "bug",
        "files": ["src/UserPage.tsx:42-50", "src/WorkspacePage.tsx:43-51"],
        "description": "Ref stays true across navigations"
      },
      {
        "id": 2,
        "title": "Dead state field",
        "severity": "low",
        "type": "dead-code",
        "files": ["src/store.ts:86"],
        "description": "Field no longer populated"
      }
    ]
  }
}
```"#;
        let result = parse_structured_review(text).unwrap();
        assert_eq!(result.issues_found, 4);
        assert_eq!(result.issues.len(), 2);
        assert_eq!(result.issues[0].title, "Missing refetch on param change");
        assert_eq!(result.issues[0].file, "src/UserPage.tsx:42-50");
        assert_eq!(result.issues[0].severity, "high");
        assert_eq!(result.issues[1].file, "src/store.ts:86");
    }

    #[test]
    fn fallback_no_issues_found_derives_from_array() {
        let text = r#"```json
{
  "issues": [
    {"title": "Null check", "file": "a.rs", "lines": "5", "severity": "high", "description": "Missing null check"}
  ]
}
```"#;
        let result = parse_structured_review(text).unwrap();
        assert_eq!(result.issues_found, 1);
        assert_eq!(result.issues.len(), 1);
    }

    #[test]
    fn fallback_returns_none_for_unrelated_json() {
        let text = "```json\n{\"name\": \"test\", \"version\": \"1.0\"}\n```";
        assert!(parse_structured_review(text).is_none());
    }

    #[test]
    fn fallback_zero_issues_with_explicit_field() {
        let text = "```json\n{\"result\": {\"issues_found\": 0, \"issues\": []}}\n```";
        let result = parse_structured_review(text).unwrap();
        assert_eq!(result.issues_found, 0);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn fallback_issues_found_as_string_derives_from_array() {
        let text = r#"```json
{"issues_found": "seven", "issues": [{"title": "A"}, {"title": "B"}, {"title": "C"}]}
```"#;
        let result = parse_structured_review(text).unwrap();
        assert_eq!(result.issues_found, 3);
        assert_eq!(result.issues.len(), 3);
    }

    #[test]
    fn fallback_filters_invalid_issues() {
        let text = r#"```json
{
  "issues": [
    {"title": "Good one", "file": "a.rs"},
    {"no_title": true},
    {"title": "Another good one"}
  ]
}
```"#;
        let result = parse_structured_review(text).unwrap();
        assert_eq!(result.issues_found, 2);
        assert_eq!(result.issues.len(), 2);
        assert_eq!(result.issues[0].title, "Good one");
        assert_eq!(result.issues[1].title, "Another good one");
    }

    #[test]
    fn fallback_issues_found_without_issues_array() {
        let text = "```json\n{\"issues_found\": 3}\n```";
        let result = parse_structured_review(text).unwrap();
        assert_eq!(result.issues_found, 3);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn fallback_multi_key_wrapper_not_unwrapped() {
        let text = r#"```json
{"meta": {"version": 1}, "data": {"issues_found": 2, "issues": [{"title": "X"}]}}
```"#;
        assert!(parse_structured_review(text).is_none());
    }

    #[test]
    fn fallback_non_object_json_returns_none() {
        let text = "```json\n[1, 2, 3]\n```";
        assert!(parse_structured_review(text).is_none());
    }

    #[test]
    fn fallback_single_key_non_object_value_returns_none() {
        let text = "```json\n{\"result\": \"all good\"}\n```";
        assert!(parse_structured_review(text).is_none());
    }

    #[test]
    fn fallback_issue_prefers_file_over_files() {
        let text = r#"```json
{
  "issues": [{"title": "X", "file": "preferred.rs", "files": ["other.rs"]}]
}
```"#;
        let result = parse_structured_review(text).unwrap();
        assert_eq!(result.issues[0].file, "preferred.rs");
    }

    #[test]
    fn fallback_issue_empty_files_array_defaults() {
        let text = r#"```json
{"issues": [{"title": "X", "files": []}]}
```"#;
        let result = parse_structured_review(text).unwrap();
        assert_eq!(result.issues[0].file, "");
    }

    #[test]
    fn fallback_issue_non_object_items_filtered() {
        let text = r#"```json
{"issues": [42, "string", null, {"title": "Real"}]}
```"#;
        let result = parse_structured_review(text).unwrap();
        assert_eq!(result.issues_found, 1);
        assert_eq!(result.issues[0].title, "Real");
    }

    // ── integration: fallback flows through public API ──────────

    #[test]
    fn parse_issues_count_via_fallback() {
        let text = r#"```json
{"summary": "3 issues", "issues": [{"title": "A"}, {"title": "B"}, {"title": "C"}]}
```"#;
        assert_eq!(parse_code_review_issues(text), Some(3));
    }

    #[test]
    fn extract_issues_section_via_fallback() {
        let text = r#"Analysis.

```json
{
  "review": {
    "issues_found": 1,
    "issues": [{"title": "NPE risk", "file": "app.ts", "lines": "42", "severity": "high", "description": "Could be null"}]
  }
}
```"#;
        let result = extract_issues_section(text);
        assert!(result.contains("### Issue 1: NPE risk"));
        assert!(result.contains("`app.ts`"));
        assert!(result.contains("high"));
    }

    #[test]
    fn parse_structured_review_unclosed_fence_returns_none() {
        let text = "```json\n{\"issues_found\": 1, \"issues\": []}";
        assert!(parse_structured_review(text).is_none());
    }

    #[test]
    fn parse_structured_review_non_json_fence_ignored() {
        let text = "```python\nprint('hi')\n```\nISSUES_FOUND: 4";
        assert!(parse_structured_review(text).is_none());
        assert_eq!(parse_code_review_issues(text), Some(4));
    }
}
