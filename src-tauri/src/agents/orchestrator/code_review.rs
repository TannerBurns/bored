//! Code review functions for the workflow orchestrator.

use crate::agents::claude::provider::extract_text_from_stream_json;

/// Parse code review output for issue count.
///
/// Looks for a line like `ISSUES_FOUND: 3` in the output and returns the number.
pub fn parse_code_review_issues(output: &str) -> Option<usize> {
    let text = extract_text_from_stream_json(output).unwrap_or_else(|| output.to_string());

    text.lines()
        .find(|l| l.trim().starts_with("ISSUES_FOUND:"))
        .and_then(|l| l.split(':').nth(1)?.trim().parse().ok())
}

/// Extract the issues section from code review output.
///
/// Extracts content between "## Issues Found" and "## Summary" for passing to the fix phase.
pub fn extract_issues_section(output: &str) -> String {
    let text = extract_text_from_stream_json(output).unwrap_or_else(|| output.to_string());

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

    text
}
