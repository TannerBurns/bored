//! Code review functions for the workflow orchestrator.
//!
//! These functions operate on already-extracted plain text. The caller is
//! responsible for using the agent provider's `extract_text` to convert raw
//! agent output before passing it here.

/// Parse code review output for issue count.
///
/// Looks for a line like `ISSUES_FOUND: 3` in the extracted text and returns the number.
pub fn parse_code_review_issues(text: &str) -> Option<usize> {
    text.lines()
        .find(|l| l.trim().starts_with("ISSUES_FOUND:"))
        .and_then(|l| l.split(':').nth(1)?.trim().parse().ok())
}

/// Extract the issues section from code review output.
///
/// Extracts content between "## Issues Found" and "## Summary" for passing to the fix phase.
pub fn extract_issues_section(text: &str) -> String {
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

    // ── parse_code_review_issues ──────────────────────────────────

    #[test]
    fn parse_issues_found_zero() {
        assert_eq!(parse_code_review_issues("ISSUES_FOUND: 0"), Some(0));
    }

    #[test]
    fn parse_issues_found_positive() {
        assert_eq!(parse_code_review_issues("ISSUES_FOUND: 3"), Some(3));
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

    // ── extract_issues_section ────────────────────────────────────

    #[test]
    fn extract_section_between_markers() {
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
}
