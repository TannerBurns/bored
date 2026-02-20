//! Tests for the orchestrator module.

use super::code_review::{extract_issues_section, parse_code_review_issues};
use super::config::{
    build_full_stage_order, expand_stage_key, StageEvent, DEFAULT_STAGE_ORDER,
    MULTI_STAGE_WORKFLOW,
};

#[test]
fn module_reexports_are_accessible() {
    use super::{
        extract_issues_section as reexported_extract,
        parse_code_review_issues as reexported_parse, OrchestratorConfig,
        StageEvent as ReexportedStageEvent, MULTI_STAGE_WORKFLOW as REEXPORTED_WORKFLOW,
    };

    assert_eq!(
        reexported_parse("ISSUES_FOUND: 5"),
        parse_code_review_issues("ISSUES_FOUND: 5")
    );
    assert_eq!(
        reexported_extract("## Issues Found\ntest\n## Summary"),
        extract_issues_section("## Issues Found\ntest\n## Summary")
    );
    assert_eq!(REEXPORTED_WORKFLOW, MULTI_STAGE_WORKFLOW);

    let event = ReexportedStageEvent {
        parent_run_id: "test".to_string(),
        stage: "plan".to_string(),
        status: "running".to_string(),
        sub_run_id: None,
        duration_secs: Some(1.5),
    };
    assert_eq!(event.stage, "plan");
    assert_eq!(event.duration_secs, Some(1.5));

    fn _type_check(_: OrchestratorConfig) {}
}

#[test]
fn stage_event_serializes_with_optional_fields() {
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
    assert!(MULTI_STAGE_WORKFLOW.contains(&"deslop"));
    assert!(MULTI_STAGE_WORKFLOW.contains(&"cleanup"));
    assert!(MULTI_STAGE_WORKFLOW.contains(&"unit-tests"));
    assert!(MULTI_STAGE_WORKFLOW.contains(&"code-review"));
    assert!(MULTI_STAGE_WORKFLOW.contains(&"code-review-fix"));
    assert!(MULTI_STAGE_WORKFLOW.contains(&"review-changes"));
}

#[test]
fn multi_stage_workflow_all_stages_unique() {
    let mut seen = std::collections::HashSet::new();
    for stage in MULTI_STAGE_WORKFLOW {
        assert!(
            seen.insert(stage),
            "Duplicate stage found: {}",
            stage
        );
    }
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

// --- stage_config_key mapping tests ---

use super::WorkflowOrchestrator;

#[test]
fn stage_config_key_maps_branch_gen() {
    assert_eq!(
        WorkflowOrchestrator::stage_config_key("branch-gen"),
        "branchGen"
    );
    assert_eq!(
        WorkflowOrchestrator::stage_config_key("branch"),
        "branchGen"
    );
}

#[test]
fn stage_config_key_maps_plan_stages() {
    assert_eq!(WorkflowOrchestrator::stage_config_key("plan"), "plan");
    assert_eq!(
        WorkflowOrchestrator::stage_config_key("plan-validation"),
        "plan"
    );
}

#[test]
fn stage_config_key_maps_implement() {
    assert_eq!(
        WorkflowOrchestrator::stage_config_key("implement"),
        "implement"
    );
}

#[test]
fn stage_config_key_maps_code_review_stages() {
    assert_eq!(
        WorkflowOrchestrator::stage_config_key("code-review"),
        "code-review"
    );
    assert_eq!(
        WorkflowOrchestrator::stage_config_key("code-review-fix"),
        "code-review"
    );
}

#[test]
fn stage_config_key_maps_commit() {
    assert_eq!(
        WorkflowOrchestrator::stage_config_key("add-and-commit"),
        "commit"
    );
}

#[test]
fn stage_config_key_passes_through_commands() {
    assert_eq!(WorkflowOrchestrator::stage_config_key("cleanup"), "cleanup");
    assert_eq!(WorkflowOrchestrator::stage_config_key("deslop"), "deslop");
    assert_eq!(
        WorkflowOrchestrator::stage_config_key("unit-tests"),
        "unit-tests"
    );
    assert_eq!(
        WorkflowOrchestrator::stage_config_key("review-changes"),
        "review-changes"
    );
    assert_eq!(
        WorkflowOrchestrator::stage_config_key("my-custom-command"),
        "my-custom-command"
    );
}

// --- expand_stage_key tests ---

#[test]
fn expand_stage_key_maps_required_keys() {
    assert_eq!(expand_stage_key("branchGen"), vec!["branch-gen", "branch"]);
    assert_eq!(expand_stage_key("plan"), vec!["plan", "plan-validation"]);
    assert_eq!(expand_stage_key("implement"), vec!["implement"]);
    assert_eq!(expand_stage_key("commit"), vec!["add-and-commit"]);
}

#[test]
fn expand_stage_key_maps_code_review() {
    assert_eq!(
        expand_stage_key("code-review"),
        vec!["code-review", "code-review-fix"]
    );
}

#[test]
fn expand_stage_key_passes_through_commands() {
    assert_eq!(expand_stage_key("cleanup"), vec!["cleanup"]);
    assert_eq!(expand_stage_key("deslop"), vec!["deslop"]);
    assert_eq!(expand_stage_key("unit-tests"), vec!["unit-tests"]);
    assert_eq!(
        expand_stage_key("my-custom-cmd"),
        vec!["my-custom-cmd"]
    );
}

// --- build_full_stage_order tests ---

#[test]
fn build_full_stage_order_default_includes_all_stages() {
    let order: Vec<String> = DEFAULT_STAGE_ORDER
        .iter()
        .map(|s| s.to_string())
        .collect();
    let full = build_full_stage_order(&order);

    assert_eq!(full[0], "branch-gen");
    assert_eq!(full[1], "branch");
    assert_eq!(full[2], "plan");
    assert_eq!(full[3], "plan-validation");
    assert_eq!(full[4], "implement");
    assert!(full.contains(&"code-review".to_string()));
    assert!(full.contains(&"cleanup".to_string()));
    assert!(full.contains(&"unit-tests".to_string()));
    assert!(full.contains(&"deslop".to_string()));
    assert_eq!(*full.last().unwrap(), "add-and-commit");
}

#[test]
fn build_full_stage_order_respects_custom_ordering() {
    let custom = vec![
        "branchGen".to_string(),
        "plan".to_string(),
        "implement".to_string(),
        "deslop".to_string(),
        "cleanup".to_string(),
        "code-review".to_string(),
        "commit".to_string(),
    ];
    let full = build_full_stage_order(&custom);

    let deslop_pos = full.iter().position(|s| s == "deslop").unwrap();
    let cleanup_pos = full.iter().position(|s| s == "cleanup").unwrap();
    let review_pos = full.iter().position(|s| s == "code-review").unwrap();

    assert!(
        deslop_pos < cleanup_pos,
        "deslop should come before cleanup in custom order"
    );
    assert!(
        cleanup_pos < review_pos,
        "cleanup should come before code-review in custom order"
    );
}

#[test]
fn build_full_stage_order_no_duplicates() {
    let order: Vec<String> = DEFAULT_STAGE_ORDER
        .iter()
        .map(|s| s.to_string())
        .collect();
    let full = build_full_stage_order(&order);

    let mut seen = std::collections::HashSet::new();
    for stage in &full {
        assert!(
            seen.insert(stage),
            "Duplicate stage found in full_execution_order: {}",
            stage
        );
    }
    let commit_count = full.iter().filter(|s| s.as_str() == "add-and-commit").count();
    assert_eq!(commit_count, 1, "add-and-commit should appear exactly once");
}

/// Regression test: resuming from "code-review-fix" must NOT cause the
/// code-review loop to be skipped.  The execute loop checks
/// `should_skip_stage("code-review-fix")` (the last sub-stage of the group)
/// so that pausing mid-loop re-enters the loop on resume.
#[test]
fn resume_from_code_review_fix_does_not_skip_loop() {
    let order: Vec<String> = DEFAULT_STAGE_ORDER
        .iter()
        .map(|s| s.to_string())
        .collect();
    let full = build_full_stage_order(&order);

    let resume_stage = "code-review-fix";
    let resume_idx = full.iter().position(|s| s == resume_stage).unwrap();
    let cr_fix_idx = full.iter().position(|s| s == "code-review-fix").unwrap();
    let cr_idx = full.iter().position(|s| s == "code-review").unwrap();

    assert_eq!(
        cr_fix_idx, resume_idx,
        "code-review-fix IS the resume point so it must not be skipped"
    );
    assert!(
        cr_idx < resume_idx,
        "code-review (first sub-stage) IS before the resume point — \
         this is why the skip check must use code-review-fix, not code-review"
    );
}

#[test]
fn default_stage_order_has_correct_structure() {
    assert_eq!(DEFAULT_STAGE_ORDER.len(), 9);
    assert_eq!(DEFAULT_STAGE_ORDER[0], "branchGen");
    assert_eq!(DEFAULT_STAGE_ORDER[1], "plan");
    assert_eq!(DEFAULT_STAGE_ORDER[2], "implement");
    assert_eq!(*DEFAULT_STAGE_ORDER.last().unwrap(), "commit");

    let required = &DEFAULT_STAGE_ORDER[..3];
    assert_eq!(required, &["branchGen", "plan", "implement"]);
}

#[test]
fn default_stage_order_expansion_covers_multi_stage_workflow() {
    let order: Vec<String> = DEFAULT_STAGE_ORDER
        .iter()
        .map(|s| s.to_string())
        .collect();
    let expanded = build_full_stage_order(&order);
    let expanded_strs: Vec<&str> = expanded.iter().map(|s| s.as_str()).collect();

    for stage in MULTI_STAGE_WORKFLOW {
        assert!(
            expanded_strs.contains(stage),
            "MULTI_STAGE_WORKFLOW stage '{}' missing from expanded DEFAULT_STAGE_ORDER",
            stage
        );
    }
}

#[test]
fn build_full_stage_order_includes_custom_commands() {
    let order = vec![
        "branchGen".to_string(),
        "plan".to_string(),
        "implement".to_string(),
        "my-custom-lint".to_string(),
        "another-check".to_string(),
        "commit".to_string(),
    ];
    let full = build_full_stage_order(&order);

    assert!(full.contains(&"my-custom-lint".to_string()));
    assert!(full.contains(&"another-check".to_string()));

    let custom_pos = full.iter().position(|s| s == "my-custom-lint").unwrap();
    let another_pos = full.iter().position(|s| s == "another-check").unwrap();
    let commit_pos = full.iter().position(|s| s == "add-and-commit").unwrap();
    assert!(custom_pos < another_pos);
    assert!(another_pos < commit_pos);
}

#[test]
fn build_full_stage_order_empty_input() {
    let full = build_full_stage_order(&[]);
    assert!(full.is_empty());
}

#[test]
fn resume_past_code_review_group_skips_loop() {
    let order: Vec<String> = DEFAULT_STAGE_ORDER
        .iter()
        .map(|s| s.to_string())
        .collect();
    let full = build_full_stage_order(&order);

    let resume_stage = "cleanup";
    let resume_idx = full.iter().position(|s| s == resume_stage).unwrap();
    let cr_fix_idx = full.iter().position(|s| s == "code-review-fix").unwrap();

    assert!(
        cr_fix_idx < resume_idx,
        "code-review-fix should be skipped when resuming from a later stage"
    );
}
