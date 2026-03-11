//! Comment management for the workflow orchestrator.

use super::WorkflowOrchestrator;
use crate::db::{AuthorType, CreateComment};

impl WorkflowOrchestrator {
    /// Add a completion summary comment for the workflow.
    ///
    /// Retrieves the plan and implementation stage outputs from the database
    /// so the summary describes *what* was planned and done, not just which
    /// stages ran.
    pub(super) fn add_workflow_summary_comment(&self) {
        let stage_outputs = self
            .db
            .get_completed_stage_outputs(&self.parent_run_id)
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to retrieve stage outputs for summary: {}", e);
                std::collections::HashMap::new()
            });

        let comment_text = build_workflow_summary(&self.ticket.title, &stage_outputs);
        let create_comment = CreateComment {
            ticket_id: self.ticket.id.clone(),
            author_type: AuthorType::Agent,
            body_md: comment_text.clone(),
            metadata: Some(serde_json::json!({
                "type": "workflow_complete",
                "parent_run_id": self.parent_run_id,
            })),
        };
        if let Err(e) = self.db.create_comment(&create_comment) {
            tracing::warn!("Failed to add workflow summary comment: {}", e);
        } else {
            tracing::info!(
                "Added workflow summary comment for ticket {}",
                self.ticket.id
            );
            let _ = self.emit_event(
                "ticket-comment-added",
                &serde_json::json!({
                    "ticketId": self.ticket.id,
                    "comment": comment_text,
                }),
            );
        }
    }

    /// Retrieve stage output from previous run (used when resuming)
    pub(super) fn get_previous_stage_output(&self, stage: &str) -> Option<String> {
        self.previous_stage_outputs.get(stage).cloned()
    }

    /// Retrieve the saved plan from previous run or comments (used when resuming)
    pub(super) fn get_saved_plan(&self) -> Option<String> {
        // First, try to get from previous run's stage outputs (more reliable)
        if let Some(plan) = self.get_previous_stage_output("plan") {
            tracing::info!(
                "Retrieved saved plan from previous run ({} chars)",
                plan.len()
            );
            return Some(plan);
        }

        // Fallback: try to get from comments (legacy support)
        match self.db.get_comments(&self.ticket.id) {
            Ok(comments) => {
                // Find the most recent plan comment (by looking for metadata with type="plan")
                for comment in comments.iter().rev() {
                    if let Some(metadata) = &comment.metadata {
                        if metadata.get("type").and_then(|v| v.as_str()) == Some("plan") {
                            // Extract the plan content from the comment body
                            // The format is: "## Implementation Plan\n\n{plan}\n\n---\n..."
                            let body = &comment.body_md;
                            if let Some(start) = body.find("## Implementation Plan\n\n") {
                                let content_start = start + "## Implementation Plan\n\n".len();
                                if let Some(end) = body[content_start..].find("\n\n---\n") {
                                    let plan = body[content_start..content_start + end].to_string();
                                    tracing::info!(
                                        "Retrieved saved plan from comment ({} chars)",
                                        plan.len()
                                    );
                                    return Some(plan);
                                }
                            }
                        }
                    }
                }
                tracing::warn!(
                    "No plan found in previous run or comments for ticket {}",
                    self.ticket.id
                );
                None
            }
            Err(e) => {
                tracing::warn!("Failed to get comments for plan retrieval: {}", e);
                None
            }
        }
    }

    /// Add a comment with the extracted plan for visibility and debugging
    pub(super) fn add_plan_comment(&self, plan: &str) {
        // The agent's text output often includes exploration/thinking text before the
        // actual plan, and the plan itself starts with "## Implementation Plan".
        // Since we wrap the content with that same header below, strip the agent's
        // header (and any preceding exploration text) to avoid duplication.
        let plan_body = strip_plan_header(plan);

        let comment_text = format!(
            "## Implementation Plan\n\n{}\n\n---\n*This plan was extracted from the planning stage and will guide the implementation.*",
            plan_body.trim()
        );
        let create_comment = CreateComment {
            ticket_id: self.ticket.id.clone(),
            author_type: AuthorType::Agent,
            body_md: comment_text.clone(),
            metadata: Some(serde_json::json!({
                "type": "plan",
                "parent_run_id": self.parent_run_id,
            })),
        };
        if let Err(e) = self.db.create_comment(&create_comment) {
            tracing::warn!("Failed to add plan comment: {}", e);
        } else {
            tracing::info!(
                "Added plan comment for ticket {} ({} chars)",
                self.ticket.id,
                plan.len()
            );
            let _ = self.emit_event(
                "ticket-comment-added",
                &serde_json::json!({
                    "ticketId": self.ticket.id,
                    "comment": comment_text,
                }),
            );
        }
    }

    /// Add a clarification request comment when the plan needs user input
    pub(super) fn add_clarification_comment(&self, message: &str) {
        let footer = "Edit the task's instructions with the requested information, then click **Resolve & Move to Ready** to continue.";

        let comment_text = format!(
            "## Clarification Needed\n\n{}\n\n---\n*{}*",
            message.trim(),
            footer
        );
        let task = self.get_task();
        let create_comment = CreateComment {
            ticket_id: self.ticket.id.clone(),
            author_type: AuthorType::Agent,
            body_md: comment_text.clone(),
            metadata: Some(serde_json::json!({
                "type": "clarification",
                "parent_run_id": self.parent_run_id,
                "task_id": task.as_ref().map(|t| &t.id),
                "task_order_index": task.as_ref().map(|t| t.order_index),
            })),
        };
        if let Err(e) = self.db.create_comment(&create_comment) {
            tracing::warn!("Failed to add clarification comment: {}", e);
        } else {
            tracing::info!(
                "Added clarification comment for ticket {} ({} chars)",
                self.ticket.id,
                message.len()
            );
            let _ = self.emit_event(
                "ticket-comment-added",
                &serde_json::json!({
                    "ticketId": self.ticket.id,
                    "comment": comment_text,
                }),
            );
        }
    }

    /// Add a comment explaining what the auto-clarification agent decided.
    pub(super) fn add_auto_clarification_comment(&self, action_label: &str, reason: &str) {
        let task = self.get_task();
        let task_label = task
            .as_ref()
            .and_then(|t| t.title.as_deref())
            .unwrap_or("(untitled task)");

        let comment_text = format!(
            "## Auto-Clarification Resolved\n\n\
             **Action:** {action_label}\n\
             **Task:** {task_label}\n\
             **Reason:** {reason}\n\n\
             ---\n\
             *Clarification was resolved automatically by the agent.*"
        );
        let create_comment = CreateComment {
            ticket_id: self.ticket.id.clone(),
            author_type: AuthorType::Agent,
            body_md: comment_text.clone(),
            metadata: Some(serde_json::json!({
                "type": "auto_clarification",
                "parent_run_id": self.parent_run_id,
                "task_id": task.as_ref().map(|t| &t.id),
                "action": action_label,
            })),
        };
        if let Err(e) = self.db.create_comment(&create_comment) {
            tracing::warn!("Failed to add auto-clarification comment: {}", e);
        } else {
            tracing::info!(
                "Added auto-clarification comment for ticket {} (action={})",
                self.ticket.id,
                action_label,
            );
            let _ = self.emit_event(
                "ticket-comment-added",
                &serde_json::json!({
                    "ticketId": self.ticket.id,
                    "comment": comment_text,
                }),
            );
        }
    }
}

const IMPL_SUMMARY_MAX_LEN: usize = 20_000;

/// Build the markdown body for the workflow-complete comment.
///
/// The plan is intentionally excluded here because it is already posted as
/// its own comment by `add_plan_comment` during the planning stage. Including
/// it again would duplicate content and eat into the implementation summary
/// budget, causing useful output to be truncated.
///
/// Pure function so it can be unit-tested without the full orchestrator.
fn build_workflow_summary(
    title: &str,
    stage_outputs: &std::collections::HashMap<String, String>,
) -> String {
    let mut sections = Vec::new();

    sections.push(format!(
        "## Workflow Complete\n\nMulti-stage workflow completed successfully for ticket **{}**.",
        title
    ));

    if let Some(impl_output) = stage_outputs.get("implement") {
        let impl_body = impl_output.trim();
        if !impl_body.is_empty() {
            let truncated = truncate_to_char_boundary(impl_body, IMPL_SUMMARY_MAX_LEN);
            let suffix = if truncated.len() < impl_body.len() {
                "\n\n*...(truncated)*"
            } else {
                ""
            };
            sections.push(format!(
                "### Implementation Summary\n\n{}{}",
                truncated, suffix
            ));
        }
    }

    sections.join("\n\n")
}

/// Truncate a string to at most `max_len` bytes, ensuring the cut lands on a
/// valid UTF-8 character boundary. Returns the longest prefix that fits.
fn truncate_to_char_boundary(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        return s;
    }
    let mut end = max_len;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Strip the "## Implementation Plan" header (and any preceding exploration text)
/// from the agent's plan output.
///
/// The agent is prompted to format its plan starting with "## Implementation Plan",
/// but its raw text output may also include thinking/exploration text before the
/// plan header. Since `add_plan_comment` wraps the body in its own
/// "## Implementation Plan" header, we need to extract just the plan content to
/// avoid duplication.
///
/// Uses the *last* occurrence of the header to handle cases where the agent's
/// exploration text itself contains earlier "## Implementation Plan" mentions.
fn strip_plan_header(plan: &str) -> &str {
    const HEADER: &str = "## Implementation Plan";

    // Find the last occurrence — the agent may mention the header in exploration
    // text before outputting the actual plan.
    if let Some(pos) = plan.rfind(HEADER) {
        let after_header = &plan[pos + HEADER.len()..];
        // Skip optional newlines between the header and the body
        after_header.trim_start_matches('\n')
    } else {
        plan
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_workflow_summary, strip_plan_header, truncate_to_char_boundary,
        IMPL_SUMMARY_MAX_LEN,
    };
    use std::collections::HashMap;

    #[test]
    fn strips_header_from_clean_plan() {
        let plan = "## Implementation Plan\n\n### Files to Modify\n- file.rs\n\n### Steps\n1. Do something";
        let result = strip_plan_header(plan);
        assert_eq!(result, "### Files to Modify\n- file.rs\n\n### Steps\n1. Do something");
    }

    #[test]
    fn strips_exploration_text_and_header() {
        let plan = "Let me explore the codebase first.\n\nI found the relevant files.\n\n## Implementation Plan\n\n### Steps\n1. Fix the bug";
        let result = strip_plan_header(plan);
        assert_eq!(result, "### Steps\n1. Fix the bug");
    }

    #[test]
    fn uses_last_occurrence_when_duplicated() {
        // The agent's exploration text mentions the header, and then the actual plan follows
        let plan = "Now I have everything. Here's the complete implementation plan:\n\n---\n\n\
                     ## Implementation Plan\n\n### Analysis\n\nFirst draft...\n\n\
                     Now I have everything. Here's the complete implementation plan:\n\n---\n\n\
                     ## Implementation Plan\n\n### Analysis\n\nFinal plan content.";
        let result = strip_plan_header(plan);
        assert_eq!(result, "### Analysis\n\nFinal plan content.");
    }

    #[test]
    fn returns_plan_unchanged_when_no_header() {
        let plan = "### Steps\n1. Do something\n2. Do something else";
        let result = strip_plan_header(plan);
        assert_eq!(result, plan);
    }

    #[test]
    fn handles_empty_input() {
        assert_eq!(strip_plan_header(""), "");
    }

    #[test]
    fn handles_header_only() {
        let result = strip_plan_header("## Implementation Plan");
        assert_eq!(result, "");
    }

    #[test]
    fn handles_header_with_trailing_newlines() {
        let result = strip_plan_header("## Implementation Plan\n\n\n");
        assert_eq!(result, "");
    }

    #[test]
    fn truncate_noop_when_short() {
        assert_eq!(truncate_to_char_boundary("hello", 10), "hello");
    }

    #[test]
    fn truncate_at_exact_length() {
        assert_eq!(truncate_to_char_boundary("hello", 5), "hello");
    }

    #[test]
    fn truncate_cuts_ascii() {
        assert_eq!(truncate_to_char_boundary("hello world", 5), "hello");
    }

    #[test]
    fn truncate_respects_char_boundary() {
        // 'é' is 2 bytes in UTF-8; cutting at byte 1 would split it
        let s = "é";
        assert_eq!(s.len(), 2);
        // max_len=1 should back up to 0 rather than split the char
        assert_eq!(truncate_to_char_boundary(s, 1), "");
    }

    #[test]
    fn truncate_multibyte_preserves_whole_chars() {
        // Each CJK char is 3 bytes; "你好" = 6 bytes
        let s = "你好世界";
        assert_eq!(truncate_to_char_boundary(s, 6), "你好");
    }

    #[test]
    fn truncate_zero_max_returns_empty() {
        assert_eq!(truncate_to_char_boundary("hello", 0), "");
    }

    #[test]
    fn truncate_empty_string() {
        assert_eq!(truncate_to_char_boundary("", 10), "");
    }

    // --- build_workflow_summary tests ---

    #[test]
    fn summary_with_no_stage_outputs() {
        let outputs = HashMap::new();
        let result = build_workflow_summary("My Ticket", &outputs);
        assert!(result.starts_with("## Workflow Complete"));
        assert!(result.contains("**My Ticket**"));
        assert!(!result.contains("### Plan"));
        assert!(!result.contains("### Implementation Summary"));
    }

    #[test]
    fn summary_excludes_plan_even_when_present() {
        let mut outputs = HashMap::new();
        outputs.insert("plan".into(), "## Implementation Plan\n\n### Steps\n1. Do X".into());
        let result = build_workflow_summary("Ticket A", &outputs);
        assert!(!result.contains("### Plan"));
        assert!(!result.contains("### Steps"));
    }

    #[test]
    fn summary_with_implement_output() {
        let mut outputs = HashMap::new();
        outputs.insert("implement".into(), "Created foo.rs and bar.rs".into());
        let result = build_workflow_summary("Ticket B", &outputs);
        assert!(!result.contains("### Plan"));
        assert!(result.contains("### Implementation Summary\n\nCreated foo.rs and bar.rs"));
        assert!(!result.contains("(truncated)"));
    }

    #[test]
    fn summary_with_both_shows_only_implement() {
        let mut outputs = HashMap::new();
        outputs.insert("plan".into(), "### Steps\n1. Add feature".into());
        outputs.insert("implement".into(), "Added the feature to main.rs".into());
        let result = build_workflow_summary("Ticket C", &outputs);
        assert!(!result.contains("### Plan"));
        assert!(result.contains("### Implementation Summary"));
    }

    #[test]
    fn summary_skips_whitespace_only_implement() {
        let mut outputs = HashMap::new();
        outputs.insert("implement".into(), "   \n\n  ".into());
        let result = build_workflow_summary("Ticket E", &outputs);
        assert!(!result.contains("### Implementation Summary"));
    }

    #[test]
    fn summary_truncates_long_implement_output() {
        let mut outputs = HashMap::new();
        let long_output = "x".repeat(IMPL_SUMMARY_MAX_LEN + 500);
        outputs.insert("implement".into(), long_output);
        let result = build_workflow_summary("Ticket F", &outputs);
        assert!(result.contains("### Implementation Summary"));
        assert!(result.contains("*...(truncated)*"));
        let after_header = result
            .split("### Implementation Summary\n\n")
            .nth(1)
            .unwrap();
        let before_suffix = after_header.split("\n\n*...(truncated)*").next().unwrap();
        assert!(before_suffix.len() <= IMPL_SUMMARY_MAX_LEN);
    }

    #[test]
    fn summary_no_truncation_marker_when_within_limit() {
        let mut outputs = HashMap::new();
        outputs.insert("implement".into(), "x".repeat(IMPL_SUMMARY_MAX_LEN));
        let result = build_workflow_summary("Ticket G", &outputs);
        assert!(result.contains("### Implementation Summary"));
        assert!(!result.contains("(truncated)"));
    }

    #[test]
    fn summary_ignores_unrelated_stage_outputs() {
        let mut outputs = HashMap::new();
        outputs.insert("deslop".into(), "some deslop output".into());
        outputs.insert("code-review".into(), "review output".into());
        let result = build_workflow_summary("Ticket H", &outputs);
        assert!(!result.contains("deslop"));
        assert!(!result.contains("review"));
        assert!(!result.contains("### Plan"));
        assert!(!result.contains("### Implementation Summary"));
    }
}
