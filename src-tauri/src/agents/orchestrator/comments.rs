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
        let comment_text = format!(
            "## Implementation Plan\n\n{}\n\n---\n*This plan was extracted from the planning stage and will guide the implementation.*",
            plan.trim()
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
        let comment_text = format!(
            "## Clarification Needed\n\n{}\n\n---\n*Please update the ticket description with the requested information and move this ticket back to Ready to continue.*",
            message.trim()
        );
        let create_comment = CreateComment {
            ticket_id: self.ticket.id.clone(),
            author_type: AuthorType::Agent,
            body_md: comment_text.clone(),
            metadata: Some(serde_json::json!({
                "type": "clarification",
                "parent_run_id": self.parent_run_id,
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
