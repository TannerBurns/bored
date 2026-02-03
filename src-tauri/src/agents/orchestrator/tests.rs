//! Tests for the orchestrator module.

#[cfg(test)]
mod tests {
    use super::super::code_review::{extract_issues_section, parse_code_review_issues};
    use super::super::config::{StageEvent, MULTI_STAGE_WORKFLOW};

    // Test that public re-exports from mod.rs work correctly
    #[test]
    fn module_reexports_are_accessible() {
        // Verify types are accessible via the module's public API
        use super::super::{
            extract_issues_section as reexported_extract,
            parse_code_review_issues as reexported_parse, OrchestratorConfig, StageEvent as ReexportedStageEvent,
            MULTI_STAGE_WORKFLOW as REEXPORTED_WORKFLOW,
        };

        // Verify the re-exports point to the same implementations
        assert_eq!(
            reexported_parse("ISSUES_FOUND: 5"),
            parse_code_review_issues("ISSUES_FOUND: 5")
        );
        assert_eq!(
            reexported_extract("## Issues Found\ntest\n## Summary"),
            extract_issues_section("## Issues Found\ntest\n## Summary")
        );
        assert_eq!(REEXPORTED_WORKFLOW, MULTI_STAGE_WORKFLOW);

        // Verify StageEvent can be constructed via re-export
        let event = ReexportedStageEvent {
            parent_run_id: "test".to_string(),
            stage: "plan".to_string(),
            status: "running".to_string(),
            sub_run_id: None,
            duration_secs: Some(1.5),
        };
        assert_eq!(event.stage, "plan");
        assert_eq!(event.duration_secs, Some(1.5));

        // Verify OrchestratorConfig fields exist (compile-time check)
        // We can't construct it without a real Database, but we can verify the type exists
        fn _type_check(_: OrchestratorConfig) {}
    }

    #[test]
    fn stage_event_serializes_with_optional_fields() {
        // Test with all optional fields populated
        let event = StageEvent {
            parent_run_id: "run-123".to_string(),
            stage: "implement".to_string(),
            status: "finished".to_string(),
            sub_run_id: Some("sub-456".to_string()),
            duration_secs: Some(42.5),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"subRunId\":\"sub-456\""));
        assert!(json.contains("\"durationSecs\":42.5"));
    }

    #[test]
    fn stage_event_serializes() {
        let event = StageEvent {
            parent_run_id: "run-1".to_string(),
            stage: "plan".to_string(),
            status: "running".to_string(),
            sub_run_id: None,
            duration_secs: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("parentRunId"));
        assert!(json.contains("plan"));
    }

    #[test]
    fn multi_stage_workflow_has_expected_stages() {
        assert!(MULTI_STAGE_WORKFLOW.contains(&"branch"));
        assert!(MULTI_STAGE_WORKFLOW.contains(&"plan"));
        assert!(MULTI_STAGE_WORKFLOW.contains(&"implement"));
        assert!(MULTI_STAGE_WORKFLOW.contains(&"add-and-commit"));
    }

    #[test]
    fn parse_code_review_issues_extracts_count() {
        let output = r#"## Issues Found

### Issue 1: Missing null check
- **File:** `src/foo.rs`
- **Lines:** 42-48
- **Severity:** high
- **Description:** Missing null check could cause panic.

## Summary
ISSUES_FOUND: 1
"#;
        assert_eq!(parse_code_review_issues(output), Some(1));
    }

    #[test]
    fn parse_code_review_issues_handles_zero() {
        let output = r#"## Issues Found

No issues found in the code review.

## Summary
ISSUES_FOUND: 0
"#;
        assert_eq!(parse_code_review_issues(output), Some(0));
    }

    #[test]
    fn parse_code_review_issues_handles_multiple() {
        let output = "Some text\nISSUES_FOUND: 5\nMore text";
        assert_eq!(parse_code_review_issues(output), Some(5));
    }

    #[test]
    fn parse_code_review_issues_returns_none_for_missing() {
        let output = "No issues marker in this output";
        assert_eq!(parse_code_review_issues(output), None);
    }

    #[test]
    fn parse_code_review_issues_handles_whitespace() {
        let output = "  ISSUES_FOUND:   3  ";
        assert_eq!(parse_code_review_issues(output), Some(3));
    }

    #[test]
    fn extract_issues_section_extracts_content() {
        let output = r#"Some preamble

## Issues Found

### Issue 1: Bug description
Details here

### Issue 2: Another bug
More details

## Summary
ISSUES_FOUND: 2
"#;
        let section = extract_issues_section(output);
        assert!(section.contains("Issue 1: Bug description"));
        assert!(section.contains("Issue 2: Another bug"));
        assert!(!section.contains("ISSUES_FOUND"));
        assert!(!section.contains("Some preamble"));
    }

    #[test]
    fn extract_issues_section_handles_no_end_marker() {
        let output = r#"## Issues Found

### Issue 1: Something
"#;
        let section = extract_issues_section(output);
        assert!(section.contains("Issue 1: Something"));
    }

    #[test]
    fn extract_issues_section_returns_all_when_no_marker() {
        let output = "Just plain text without markers";
        let section = extract_issues_section(output);
        assert_eq!(section, output);
    }

    #[test]
    fn parse_code_review_issues_handles_large_count() {
        let output = "ISSUES_FOUND: 99";
        assert_eq!(parse_code_review_issues(output), Some(99));
    }

    #[test]
    fn parse_code_review_issues_ignores_invalid_number() {
        let output = "ISSUES_FOUND: abc";
        assert_eq!(parse_code_review_issues(output), None);
    }

    #[test]
    fn parse_code_review_issues_takes_first_match() {
        let output = "ISSUES_FOUND: 2\nISSUES_FOUND: 5";
        assert_eq!(parse_code_review_issues(output), Some(2));
    }

    #[test]
    fn extract_issues_section_handles_empty_section() {
        let output = "## Issues Found\n## Summary\nISSUES_FOUND: 0";
        let section = extract_issues_section(output);
        assert_eq!(section, "");
    }
}
