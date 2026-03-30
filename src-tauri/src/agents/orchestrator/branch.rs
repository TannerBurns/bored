//! Branch creation and management for the workflow orchestrator.

use super::WorkflowOrchestrator;
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
                let text_content = self.extract_text(output);
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

        // Store the new branch name now — the primary is already renamed, so
        // the ticket must reference the new name regardless of secondary outcomes.
        self.store_branch_name(&new_branch_name);

        // Rename branches in all secondary workspace worktrees. If any fail,
        // halt the workflow so the mismatch is surfaced rather than silently
        // proceeding with a broken state (secondary on old branch, ticket on new).
        self.rename_workspace_secondary_branches(temp_branch_name, &new_branch_name)?;

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
                let text_content = self.extract_text(output);

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

    /// Rename branches in all secondary workspace worktrees using direct git
    /// commands. The primary worktree's branch is already renamed by the agent;
    /// this ensures secondary projects also have the new branch name so that
    /// `get_ticket_working_dirs` can find all worktrees by branch.
    ///
    /// Returns `Err` if any rename fails. Failing silently would leave the
    /// ticket's stored branch name out of sync with the secondary worktree,
    /// causing downstream stages to operate on the wrong directory.
    fn rename_workspace_secondary_branches(
        &self,
        old_branch: &str,
        new_branch: &str,
    ) -> Result<(), String> {
        if self.workspace_paths.is_empty() {
            return Ok(());
        }

        let primary = self.repo_path.to_string_lossy().to_string();
        let mut failed: Vec<String> = Vec::new();

        for ws_path in &self.workspace_paths {
            let ws_path_str = ws_path.to_string_lossy().to_string();
            if ws_path_str == primary {
                continue;
            }

            if !ws_path.exists() {
                failed.push(format!("{} (directory missing)", ws_path_str));
                continue;
            }

            tracing::info!(
                "Renaming branch '{}' -> '{}' in workspace worktree {}",
                old_branch, new_branch, ws_path_str
            );

            let rename_result = std::process::Command::new("git")
                .args(["branch", "-m", old_branch, new_branch])
                .current_dir(ws_path)
                .output();

            match rename_result {
                Ok(output) if output.status.success() => {
                    let push_result = std::process::Command::new("git")
                        .args(["push", "-u", "origin", new_branch])
                        .current_dir(ws_path)
                        .output();
                    if let Ok(po) = push_result {
                        if !po.status.success() {
                            let stderr = String::from_utf8_lossy(&po.stderr);
                            tracing::warn!(
                                "Failed to push renamed branch in workspace worktree {}: {}",
                                ws_path_str, stderr.trim()
                            );
                        }
                    }
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    tracing::error!(
                        "Failed to rename branch in workspace worktree {}: {}",
                        ws_path_str, stderr.trim()
                    );
                    failed.push(format!("{} ({})", ws_path_str, stderr.trim()));
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to run git branch -m in workspace worktree {}: {}",
                        ws_path_str, e
                    );
                    failed.push(format!("{} ({})", ws_path_str, e));
                }
            }
        }

        if failed.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "Failed to rename branch in {} workspace worktree(s): {}",
                failed.len(),
                failed.join("; ")
            ))
        }
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
