//! Tests for the orchestrator module.

use super::code_review::{extract_issues_section, parse_code_review_issues};
use super::config::{
    build_full_stage_order, expand_stage_key, StageEvent, DEFAULT_OPTIONAL_STAGE_ORDER,
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
    // Basic stages
    assert!(MULTI_STAGE_WORKFLOW.contains(&"branch"));
    assert!(MULTI_STAGE_WORKFLOW.contains(&"plan"));
    assert!(MULTI_STAGE_WORKFLOW.contains(&"implement"));
    assert!(MULTI_STAGE_WORKFLOW.contains(&"add-and-commit"));

    // QA stages
    assert!(MULTI_STAGE_WORKFLOW.contains(&"deslop"));
    assert!(MULTI_STAGE_WORKFLOW.contains(&"cleanup"));
    assert!(MULTI_STAGE_WORKFLOW.contains(&"unit-tests"));

    // Contextual repeated stages for cleanup/review cycles
    assert!(
        MULTI_STAGE_WORKFLOW.contains(&"cleanup-post-tests"),
        "Missing cleanup-post-tests stage"
    );
    assert!(
        MULTI_STAGE_WORKFLOW.contains(&"review-changes"),
        "Missing review-changes stage"
    );
    assert!(
        MULTI_STAGE_WORKFLOW.contains(&"cleanup-post-review"),
        "Missing cleanup-post-review stage"
    );
    assert!(
        MULTI_STAGE_WORKFLOW.contains(&"review-changes-final"),
        "Missing review-changes-final stage"
    );
}

#[test]
fn multi_stage_workflow_correct_order() {
    let expected_qa_order = [
        "cleanup",
        "unit-tests",
        "cleanup-post-tests",
        "review-changes",
        "cleanup-post-review",
        "review-changes-final",
        "deslop",
        "add-and-commit",
    ];

    let positions: Vec<_> = expected_qa_order
        .iter()
        .map(|stage| {
            MULTI_STAGE_WORKFLOW
                .iter()
                .position(|s| s == stage)
                .unwrap_or_else(|| panic!("Stage '{}' not found in workflow", stage))
        })
        .collect();

    for i in 1..positions.len() {
        assert!(
            positions[i] > positions[i - 1],
            "Stage '{}' should come after '{}' but doesn't",
            expected_qa_order[i],
            expected_qa_order[i - 1]
        );
    }
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
        "codeReview"
    );
    assert_eq!(
        WorkflowOrchestrator::stage_config_key("code-review-fix"),
        "codeReview"
    );
}

#[test]
fn stage_config_key_maps_deslop() {
    assert_eq!(WorkflowOrchestrator::stage_config_key("deslop"), "deslop");
}

#[test]
fn stage_config_key_maps_cleanup() {
    assert_eq!(WorkflowOrchestrator::stage_config_key("cleanup"), "cleanup");
}

#[test]
fn stage_config_key_maps_unit_test_stages() {
    assert_eq!(
        WorkflowOrchestrator::stage_config_key("unit-tests"),
        "unitTests"
    );
    assert_eq!(
        WorkflowOrchestrator::stage_config_key("cleanup-post-tests"),
        "unitTests"
    );
}

#[test]
fn stage_config_key_maps_final_review_stages() {
    assert_eq!(
        WorkflowOrchestrator::stage_config_key("review-changes"),
        "finalReview"
    );
    assert_eq!(
        WorkflowOrchestrator::stage_config_key("cleanup-post-review"),
        "finalReview"
    );
    assert_eq!(
        WorkflowOrchestrator::stage_config_key("review-changes-final"),
        "finalReview"
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
fn stage_config_key_passes_through_unknown() {
    assert_eq!(
        WorkflowOrchestrator::stage_config_key("branch"),
        "branch"
    );
    assert_eq!(
        WorkflowOrchestrator::stage_config_key("unknown-stage"),
        "unknown-stage"
    );
}

// --- expand_stage_key tests ---

#[test]
fn expand_stage_key_maps_all_known_keys() {
    assert_eq!(expand_stage_key("codeReview"), &["code-review"]);
    assert_eq!(expand_stage_key("cleanup"), &["cleanup"]);
    assert_eq!(
        expand_stage_key("unitTests"),
        &["unit-tests", "cleanup-post-tests"]
    );
    assert_eq!(
        expand_stage_key("finalReview"),
        &["review-changes", "cleanup-post-review", "review-changes-final"]
    );
    assert_eq!(expand_stage_key("deslop"), &["deslop"]);
    assert_eq!(expand_stage_key("commit"), &["add-and-commit"]);
}

#[test]
fn expand_stage_key_returns_empty_for_unknown() {
    assert!(expand_stage_key("unknown").is_empty());
    assert!(expand_stage_key("branchGen").is_empty());
}

// --- build_full_stage_order tests ---

#[test]
fn build_full_stage_order_default_includes_all_stages() {
    let order: Vec<String> = DEFAULT_OPTIONAL_STAGE_ORDER
        .iter()
        .map(|s| s.to_string())
        .collect();
    let full = build_full_stage_order(&order);

    assert_eq!(full[0], "branch-gen");
    assert_eq!(full[1], "branch");
    assert_eq!(full[2], "plan");
    assert_eq!(full[3], "plan-validation");
    assert_eq!(full[4], "implement");
    assert!(full.contains(&"code-review"));
    assert!(full.contains(&"cleanup"));
    assert!(full.contains(&"unit-tests"));
    assert!(full.contains(&"deslop"));
    assert_eq!(*full.last().unwrap(), "add-and-commit");
}

#[test]
fn build_full_stage_order_respects_custom_ordering() {
    let custom = vec![
        "deslop".to_string(),
        "cleanup".to_string(),
        "codeReview".to_string(),
    ];
    let full = build_full_stage_order(&custom);

    let deslop_pos = full.iter().position(|&s| s == "deslop").unwrap();
    let cleanup_pos = full.iter().position(|&s| s == "cleanup").unwrap();
    let review_pos = full.iter().position(|&s| s == "code-review").unwrap();

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
fn build_full_stage_order_filters_required_keys_from_frontend() {
    let frontend_order = vec![
        "branchGen".to_string(),
        "plan".to_string(),
        "implement".to_string(),
        "codeReview".to_string(),
        "cleanup".to_string(),
        "unitTests".to_string(),
        "finalReview".to_string(),
        "deslop".to_string(),
        "commit".to_string(),
    ];
    let full = build_full_stage_order(&frontend_order);

    let mut seen = std::collections::HashSet::new();
    for stage in &full {
        assert!(
            seen.insert(stage),
            "Duplicate stage found in full_execution_order: {}",
            stage
        );
    }
    assert_eq!(*full.last().unwrap(), "add-and-commit");
    let commit_count = full.iter().filter(|&&s| s == "add-and-commit").count();
    assert_eq!(commit_count, 1, "add-and-commit should appear exactly once");
}

#[test]
fn build_full_stage_order_frontend_input_matches_optional_only_input() {
    let optional_only: Vec<String> = DEFAULT_OPTIONAL_STAGE_ORDER
        .iter()
        .map(|s| s.to_string())
        .collect();
    let full_from_optional = build_full_stage_order(&optional_only);

    let frontend_all = vec![
        "branchGen".to_string(),
        "plan".to_string(),
        "implement".to_string(),
        "codeReview".to_string(),
        "cleanup".to_string(),
        "unitTests".to_string(),
        "finalReview".to_string(),
        "deslop".to_string(),
        "commit".to_string(),
    ];
    let full_from_frontend = build_full_stage_order(&frontend_all);

    assert_eq!(full_from_optional, full_from_frontend);
}
