//! Branch creation and management for the workflow orchestrator.

use super::WorkflowOrchestrator;
use crate::agents::extract_text_from_stream_json;
use crate::agents::prompt::{generate_branch_name_generation_prompt, parse_branch_name_from_output};

impl WorkflowOrchestrator {
    /// Handle branch creation based on current state.
    /// Returns Ok(()) if branch handling completed (or was skipped), Err if cancelled/failed.
    pub(super) async fn handle_branch_creation(&self) -> Result<(), String> {
        // Skip if we're resuming past the branch stage
        if self.should_skip_stage("branch") {
            tracing::info!("Skipping branch stage (resuming from later stage)");
            return Ok(());
        }

        if let Some(ref branch_name) = self.worktree_branch {
            if self.is_temp_branch {
                self.rename_temp_branch(branch_name).await
            } else {
                self.handle_existing_branch(branch_name).await
            }
        } else {
            self.generate_and_create_branch().await
        }
    }

    /// Rename a temporary branch to an AI-generated name.
    async fn rename_temp_branch(&self, temp_branch_name: &str) -> Result<(), String> {
        tracing::info!(
            "Temp branch '{}' exists, generating AI name and renaming...",
            temp_branch_name
        );

        if self.is_cancelled() {
            return Err("Workflow cancelled".to_string());
        }

        let branch_gen_result = self
            .run_stage(
                "branch-gen",
                &generate_branch_name_generation_prompt(&self.ticket),
            )
            .await?;

        // Try to parse the generated branch name
        let generated_branch = branch_gen_result
            .captured_stdout
            .as_ref()
            .and_then(|output| {
                let text_content =
                    extract_text_from_stream_json(output).unwrap_or_else(|| output.clone());
                tracing::debug!("Branch-gen output (extracted): {}", text_content);
                parse_branch_name_from_output(&text_content)
            });

        // Use generated name or fall back to deterministic
        let new_branch_name = if let Some(ref name) = generated_branch {
            tracing::info!("Agent generated branch name: {}", name);
            name.clone()
        } else {
            let fallback =
                crate::agents::worktree::generate_branch_name(&self.ticket.id, &self.ticket.title);
            tracing::warn!(
                "Could not parse generated branch name, using fallback: {}",
                fallback
            );
            fallback
        };

        // Rename the temp branch to the new name BEFORE updating the database.
        // This ensures the database only records the new branch name after the git
        // rename succeeds. If we updated the database first and the rename failed,
        // the database would have the new name while git still has the old name,
        // causing subsequent runs to fail when they try to use the recorded branch.
        if self.is_cancelled() {
            return Err("Workflow cancelled".to_string());
        }

        let rename_prompt = format!(
            r#"Rename the current git branch to a better name.

## Task
Rename the current branch from `{}` to `{}`

## Instructions
1. You should already be on the branch `{}`
2. Rename the current branch: `git branch -m {}`
3. Push the renamed branch to origin: `git push -u origin {}`
4. Delete the old branch from origin (if it was pushed): `git push origin --delete {}` (ignore errors if it doesn't exist remotely)

Do NOT start implementing any code changes. Just rename the branch.
"#,
            temp_branch_name,
            new_branch_name,
            temp_branch_name,
            new_branch_name,
            new_branch_name,
            temp_branch_name
        );

        let _rename_result = self.run_stage("branch", &rename_prompt).await?;

        // Now that the git rename succeeded, store the NEW branch name on ticket
        self.store_branch_name(&new_branch_name);

        Ok(())
    }

    /// Handle an existing (non-temporary) branch.
    async fn handle_existing_branch(&self, branch_name: &str) -> Result<(), String> {
        tracing::info!("Using pre-determined branch name: {}", branch_name);

        // Store branch name on ticket if not already set
        if self.ticket.branch_name.is_none() {
            if let Err(e) = self.db.set_ticket_branch(&self.ticket.id, branch_name) {
                tracing::warn!("Failed to store branch name on ticket: {}", e);
            } else {
                tracing::info!(
                    "Stored branch name '{}' on ticket {}",
                    branch_name,
                    self.ticket.id
                );
            }
        }

        // If branch wasn't already created (e.g., worktree creation failed),
        // we need to create it now
        if !self.branch_already_created {
            tracing::info!("Branch '{}' not yet created, creating now...", branch_name);

            if self.is_cancelled() {
                return Err("Workflow cancelled".to_string());
            }

            let branch_prompt = format!(
                r#"Create a new git branch for this task.

## Task
Create and switch to a new branch: `{}`

## Instructions
1. Check if you're on a clean working tree (stash changes if needed)
2. Switch to the main branch (or master if main doesn't exist)
3. Pull the latest changes from origin: `git pull origin main`
4. Create and switch to the new branch from main
5. Push the branch to origin with -u flag

Do NOT start implementing any code changes. Just create the branch.
"#,
                branch_name
            );

            let _branch_result = self.run_stage("branch", &branch_prompt).await?;
        }

        Ok(())
    }

    /// Generate and create a new branch (fallback path).
    async fn generate_and_create_branch(&self) -> Result<(), String> {
        // No branch name yet - generate and create a branch
        // (This path is kept for backwards compatibility but shouldn't normally be hit)
        tracing::info!("No branch name provided, generating and creating new branch...");

        if self.is_cancelled() {
            return Err("Workflow cancelled".to_string());
        }

        let branch_gen_result = self
            .run_stage(
                "branch-gen",
                &generate_branch_name_generation_prompt(&self.ticket),
            )
            .await?;

        // Try to parse the generated branch name
        // For Claude, we need to extract text from stream-json format first
        let generated_branch = branch_gen_result
            .captured_stdout
            .as_ref()
            .and_then(|output| {
                // Try extracting text from stream-json (Claude format)
                let text_content =
                    extract_text_from_stream_json(output).unwrap_or_else(|| output.clone());

                tracing::debug!("Branch-gen output (extracted): {}", text_content);
                parse_branch_name_from_output(&text_content)
            });

        // Use generated name or fall back to deterministic
        let branch_to_create = if let Some(ref name) = generated_branch {
            tracing::info!("Agent generated branch name: {}", name);
            name.clone()
        } else {
            // Fallback to deterministic naming
            let fallback =
                crate::agents::worktree::generate_branch_name(&self.ticket.id, &self.ticket.title);
            tracing::warn!(
                "Could not parse generated branch name, using fallback: {}",
                fallback
            );
            fallback
        };

        // Store branch name on ticket BEFORE creating the branch
        // This allows the UI to show the branch immediately
        self.store_branch_name(&branch_to_create);

        // Now have the agent create the branch with that name
        if self.is_cancelled() {
            return Err("Workflow cancelled".to_string());
        }

        let branch_prompt = format!(
            r#"Create a new git branch for this task.

## Task
Create and switch to a new branch: `{}`

## Instructions
1. Check if you're on a clean working tree (stash changes if needed)
2. Switch to the main branch (or master if main doesn't exist)
3. Pull the latest changes from origin: `git pull origin main`
4. Create and switch to the new branch from main
5. Push the branch to origin with -u flag

Do NOT start implementing any code changes. Just create the branch.
"#,
            branch_to_create
        );

        let _branch_result = self.run_stage("branch", &branch_prompt).await?;

        Ok(())
    }

    /// Store a branch name on the ticket and emit update event.
    fn store_branch_name(&self, branch_name: &str) {
        if let Err(e) = self.db.set_ticket_branch(&self.ticket.id, branch_name) {
            tracing::warn!("Failed to store branch name on ticket: {}", e);
        } else {
            tracing::info!(
                "Stored branch name '{}' on ticket {}",
                branch_name,
                self.ticket.id
            );
            // Emit event for frontend to update the ticket display
            let _ = self.emit_event(
                "ticket-branch-updated",
                &serde_json::json!({
                    "ticketId": self.ticket.id,
                    "branchName": branch_name,
                }),
            );
        }
    }
}
