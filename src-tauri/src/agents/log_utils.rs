//! Shared utilities for parsing and displaying agent CLI log output.

/// Truncate a string to at most `max_bytes` bytes, ensuring the cut
/// falls on a UTF-8 character boundary. Returns the full string if it
/// is already within the limit.
pub fn truncate_to_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Extract a human-readable message from a raw log line.
/// Claude Code stdout lines are JSON objects like:
///   {"type":"assistant","message":{"content":[{"type":"text","text":"..."},{"type":"tool_use","name":"Read",...}]}}
///   {"type":"user","message":{"content":[{"tool_use_id":"...","content":"..."}]}}
/// We extract tool names, short descriptions, or skip uninteresting lines.
pub fn extract_log_display_message(content: &str) -> Option<String> {
    // Non-JSON lines (e.g., stderr warnings) — show as-is
    if !content.starts_with('{') {
        return Some(content.to_string());
    }

    let json: serde_json::Value = serde_json::from_str(content).ok()?;
    let msg_type = json.get("type")?.as_str()?;

    match msg_type {
        "assistant" => {
            let content_arr = json.get("message")?.get("content")?.as_array()?;

            for item in content_arr {
                if item.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                    let tool_name = item.get("name").and_then(|n| n.as_str()).unwrap_or("tool");
                    let detail = item.get("input").and_then(|input| {
                        input
                            .get("file_path")
                            .and_then(|v| v.as_str())
                            .or_else(|| input.get("path").and_then(|v| v.as_str()))
                            .or_else(|| input.get("command").and_then(|v| v.as_str()))
                            .or_else(|| input.get("pattern").and_then(|v| v.as_str()))
                            .or_else(|| input.get("query").and_then(|v| v.as_str()))
                    });

                    return match detail {
                        Some(d) => {
                            let d_short = truncate_to_char_boundary(d, 60);
                            Some(format!("{}: {}", tool_name, d_short))
                        }
                        None => Some(format!("Using {}", tool_name)),
                    };
                }
            }

            None
        }
        "system" => {
            let subtype = json.get("subtype").and_then(|s| s.as_str());
            if subtype == Some("init") {
                Some("Agent starting...".to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_within_limit_returns_full_string() {
        assert_eq!(truncate_to_char_boundary("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact_length_returns_full_string() {
        assert_eq!(truncate_to_char_boundary("hello", 5), "hello");
    }

    #[test]
    fn truncate_ascii_at_boundary() {
        assert_eq!(truncate_to_char_boundary("hello world", 5), "hello");
    }

    #[test]
    fn truncate_multibyte_does_not_split_char() {
        let s = "é";
        assert_eq!(s.len(), 2);
        assert_eq!(truncate_to_char_boundary(s, 1), "");
    }

    #[test]
    fn truncate_multibyte_keeps_complete_chars() {
        let s = "aé";
        assert_eq!(truncate_to_char_boundary(s, 2), "a");
    }

    #[test]
    fn truncate_emoji_boundary() {
        let s = "😀x";
        assert_eq!(truncate_to_char_boundary(s, 2), "");
        assert_eq!(truncate_to_char_boundary(s, 4), "😀");
    }

    #[test]
    fn truncate_empty_string() {
        assert_eq!(truncate_to_char_boundary("", 5), "");
    }

    #[test]
    fn truncate_zero_max() {
        assert_eq!(truncate_to_char_boundary("hello", 0), "");
    }

    #[test]
    fn log_display_non_json_returns_as_is() {
        assert_eq!(
            extract_log_display_message("some stderr warning"),
            Some("some stderr warning".to_string())
        );
    }

    #[test]
    fn log_display_assistant_tool_use_with_path() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"src/main.rs"}}]}}"#;
        assert_eq!(
            extract_log_display_message(line),
            Some("Read: src/main.rs".to_string())
        );
    }

    #[test]
    fn log_display_assistant_tool_use_with_command() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"cargo test"}}]}}"#;
        assert_eq!(
            extract_log_display_message(line),
            Some("Bash: cargo test".to_string())
        );
    }

    #[test]
    fn log_display_assistant_tool_use_no_detail() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"CustomTool","input":{"other_field":"value"}}]}}"#;
        assert_eq!(
            extract_log_display_message(line),
            Some("Using CustomTool".to_string())
        );
    }

    #[test]
    fn log_display_assistant_text_only_returns_none() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Let me think about this..."}]}}"#;
        assert_eq!(extract_log_display_message(line), None);
    }

    #[test]
    fn log_display_system_init_returns_starting() {
        let line = r#"{"type":"system","subtype":"init"}"#;
        assert_eq!(
            extract_log_display_message(line),
            Some("Agent starting...".to_string())
        );
    }

    #[test]
    fn log_display_system_non_init_returns_none() {
        let line = r#"{"type":"system","subtype":"other"}"#;
        assert_eq!(extract_log_display_message(line), None);
    }

    #[test]
    fn log_display_user_message_returns_none() {
        let line = r#"{"type":"user","message":{"content":[{"tool_use_id":"toolu_1","content":"file contents"}]}}"#;
        assert_eq!(extract_log_display_message(line), None);
    }

    #[test]
    fn log_display_invalid_json_returns_none() {
        assert_eq!(extract_log_display_message("{invalid json}"), None);
    }

    #[test]
    fn log_display_truncates_long_detail() {
        let long_path = "a".repeat(100);
        let line = format!(
            r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","name":"Read","input":{{"file_path":"{}"}}}}]}}}}"#,
            long_path
        );
        let result = extract_log_display_message(&line).unwrap();
        assert!(result.starts_with("Read: "));
        assert!(result.len() <= 66);
    }
}
