//! Comment management for the workflow orchestrator.

use super::WorkflowOrchestrator;
use crate::db::{AuthorType, CreateComment};

impl WorkflowOrchestrator {
    /// Add a completion summary comment for the workflow
    pub(super) fn add_workflow_summary_comment(&self) {
        let comment_text = format!(
            "## Workflow Complete\n\nMulti-stage workflow completed successfully for ticket **{}**.\n\n\
            Stages completed: branch, plan, implement, code-review loop, deslop, cleanup, unit-tests, review-changes, add-and-commit",
            self.ticket.title
        );
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
        let is_followup_task = self
            .task
            .as_ref()
            .map(|t| t.order_index > 0)
            .unwrap_or(false);

        let footer = if is_followup_task {
            "Edit the blocked task's instructions with the requested information, then click **Resolve & Move to Ready** to continue."
        } else {
            "Update the ticket description with the requested information, then click **Resolve & Move to Ready** to continue."
        };

        let comment_text = format!(
            "## Clarification Needed\n\n{}\n\n---\n*{}*",
            message.trim(),
            footer
        );
        let create_comment = CreateComment {
            ticket_id: self.ticket.id.clone(),
            author_type: AuthorType::Agent,
            body_md: comment_text.clone(),
            metadata: Some(serde_json::json!({
                "type": "clarification",
                "parent_run_id": self.parent_run_id,
                "task_id": self.task.as_ref().map(|t| &t.id),
                "task_order_index": self.task.as_ref().map(|t| t.order_index),
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
    use super::strip_plan_header;

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
}
