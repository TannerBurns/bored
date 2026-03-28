//! Stage execution and retry logic for the workflow orchestrator.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::Emitter;

use super::code_review::{extract_issues_with_parsed, parse_code_review_issues, parse_structured_review};
use super::config::StageEvent;
use super::WorkflowOrchestrator;
use crate::agents::prompt::generate_command_prompt;
use crate::agents::{AgentRunConfig, AgentRunResult};
use crate::agents::{LogCallback, LogLine, LogStream, RunOutcome};
use crate::db::{AgentEventPayload, CreateRun, EventType, NormalizedEvent, RunStatus};

impl WorkflowOrchestrator {
    /// Run a single stage of the workflow with retry support.
    pub(super) async fn run_stage(
        &self,
        stage: &str,
        prompt: &str,
    ) -> Result<AgentRunResult, String> {
        let sid = self.get_workflow_session_id();
        self.run_stage_inner(stage, prompt, None, sid.as_deref(), None, None, None)
            .await
    }

    /// Run a single stage with an explicit model override (used by auto-pilot).
    pub(super) async fn run_stage_with_model(
        &self,
        stage: &str,
        prompt: &str,
        model: &str,
    ) -> Result<AgentRunResult, String> {
        let sid = self.get_workflow_session_id();
        self.run_stage_inner(stage, prompt, Some(model), sid.as_deref(), None, None, None)
            .await
    }

    /// Run a single stage with explicit model, timeout, and retry overrides.
    async fn run_stage_with_overrides(
        &self,
        stage: &str,
        prompt: &str,
        model: &str,
        timeout_secs: u64,
        max_retries: u32,
    ) -> Result<AgentRunResult, String> {
        let sid = self.get_workflow_session_id();
        self.run_stage_inner(
            stage,
            prompt,
            Some(model),
            sid.as_deref(),
            Some(timeout_secs),
            Some(max_retries),
            None,
        )
        .await
    }

    /// Run a single stage in a specific directory (instead of the primary repo_path).
    /// Used by the commit stage to run add-and-commit in each workspace project.
    ///
    /// Does not pass or capture the workflow session ID — secondary worktree
    /// runs must not overwrite the shared session, which would cause subsequent
    /// stages (e.g. code-review) to attempt --resume with a stale context.
    pub(super) async fn run_stage_in_dir(
        &self,
        stage: &str,
        prompt: &str,
        dir: &std::path::Path,
    ) -> Result<AgentRunResult, String> {
        self.run_stage_inner(stage, prompt, None, None, None, None, Some(dir))
            .await
    }

    fn get_workflow_session_id(&self) -> Option<String> {
        self.workflow_session_id
            .read()
            .ok()
            .and_then(|guard| guard.clone())
    }

    fn set_workflow_session_id(&self, session_id: &str) {
        if let Ok(mut guard) = self.workflow_session_id.write() {
            *guard = Some(session_id.to_string());
        }
        self.save_workflow_session_id(session_id);
    }

    /// Restore workflow session ID from run metadata into the in-memory field.
    pub(super) fn restore_workflow_session_id(&self) {
        if let Some(sid) = self.load_workflow_session_id() {
            tracing::info!("Restored workflow session id from metadata: {}", sid);
            if let Ok(mut guard) = self.workflow_session_id.write() {
                *guard = Some(sid);
            }
        }
    }

    async fn run_stage_inner(
        &self,
        stage: &str,
        prompt: &str,
        model_override: Option<&str>,
        session_id: Option<&str>,
        timeout_override: Option<u64>,
        retries_override: Option<u32>,
        dir_override: Option<&std::path::Path>,
    ) -> Result<AgentRunResult, String> {
        let max_attempts = retries_override.unwrap_or(self.stage_max_retries) + 1;
        let mut last_error = String::new();

        for attempt in 1..=max_attempts {
            if self.is_cancelled() {
                return Err("Workflow cancelled".to_string());
            }

            if attempt > 1 {
                let backoff_secs = 5 * attempt as u64;
                tracing::warn!(
                    "Stage '{}' retry {}/{} after {}s backoff",
                    stage,
                    attempt,
                    max_attempts,
                    backoff_secs
                );

                for _ in 0..backoff_secs {
                    if self.is_cancelled() {
                        tracing::info!("Stage '{}' backoff interrupted by cancellation", stage);
                        return Err("Workflow cancelled".to_string());
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }

            // Don't pass session_id on retries -- the session may be in a bad state.
            let attempt_session_id = if attempt == 1 { session_id } else { None };

            match self
                .run_stage_attempt(stage, prompt, attempt, max_attempts, model_override, attempt_session_id, timeout_override, dir_override)
                .await
            {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = e.clone();
                    if e.contains("cancelled") || e.contains("Cancelled") {
                        return Err(e);
                    }
                    if attempt < max_attempts {
                        tracing::warn!(
                            "Stage '{}' failed (attempt {}/{}): {}",
                            stage,
                            attempt,
                            max_attempts,
                            e
                        );
                        continue;
                    }
                }
            }
        }

        Err(format!("{} (after {} attempts)", last_error, max_attempts))
    }

    /// Run a single attempt of a stage.
    /// When `dir_override` is set, the agent runs in that directory instead of
    /// the orchestrator's primary `repo_path`.
    pub(super) async fn run_stage_attempt(
        &self,
        stage: &str,
        prompt: &str,
        attempt: u32,
        max_attempts: u32,
        model_override: Option<&str>,
        session_id: Option<&str>,
        timeout_override: Option<u64>,
        dir_override: Option<&std::path::Path>,
    ) -> Result<AgentRunResult, String> {
        let effective_repo_path = dir_override
            .map(|d| d.to_path_buf())
            .unwrap_or_else(|| self.repo_path.clone());

        tracing::info!(
            "Starting stage '{}' attempt {}/{} for parent run {}{}",
            stage,
            attempt,
            max_attempts,
            self.parent_run_id,
            dir_override.map_or(String::new(), |d| format!(" (in {})", d.display()))
        );

        if attempt == 1 {
            self.emit_stage_event(stage, "running", None, None);
        }

        let db_agent_type = self.agent_id.clone();

        let sub_run = self
            .db
            .create_run(&CreateRun {
                ticket_id: self.ticket.id.clone(),
                agent_type: db_agent_type,
                repo_path: effective_repo_path.to_string_lossy().to_string(),
                parent_run_id: Some(self.parent_run_id.clone()),
                stage: Some(stage.to_string()),
                ..Default::default()
            })
            .map_err(|e| format!("Failed to create sub-run: {}", e))?;

        self.db
            .update_run_status(&sub_run.id, RunStatus::Running, None, None)
            .map_err(|e| format!("Failed to update sub-run status: {}", e))?;

        let stage_model = model_override
            .map(|m| m.to_string())
            .unwrap_or_else(|| self.get_stage_model(stage));
        let config = AgentRunConfig {
            agent_id: self.agent_id.clone(),
            ticket_id: self.ticket.id.clone(),
            run_id: sub_run.id.clone(),
            repo_path: effective_repo_path,
            prompt: prompt.to_string(),
            timeout_secs: Some(timeout_override.unwrap_or(self.stage_timeout_secs)),
            model: Some(stage_model.clone()),
            agent_config: self.agent_config.clone(),
            session_id: session_id.map(|s| s.to_string()),
            workspace_file: self.workspace_file.clone(),
            workspace_paths: self.workspace_paths.clone(),
            debug_mode: self.debug_mode,
            allow_protected_branch: false,
        };

        // Create log callback
        let on_log = self.create_log_callback();

        // Set up cancel handle registration
        let cancel_handles = self.cancel_handles.clone();
        let sub_run_id_for_spawn = sub_run.id.clone();
        let sub_run_id_for_cleanup = sub_run.id.clone();
        let parent_run_id = self.parent_run_id.clone();
        let cancelled = self.cancelled.clone();

        let on_spawn: crate::agents::spawner::OnSpawnCallback = Box::new(move |cancel_handle| {
            tracing::info!(
                "Sub-run {} spawned for parent {}",
                sub_run_id_for_spawn,
                parent_run_id
            );
            let mut handles = cancel_handles
                .lock()
                .expect("cancel handles mutex poisoned");

            if let Some(prev_handle) = handles.get(&parent_run_id) {
                if prev_handle.is_cancelled() {
                    tracing::info!(
                        "Previous handle for parent {} was cancelled, propagating to new sub-run {}",
                        parent_run_id, sub_run_id_for_spawn
                    );
                    cancel_handle.cancel();
                }
            }

            handles.insert(sub_run_id_for_spawn.clone(), cancel_handle.clone());
            handles.insert(parent_run_id.clone(), cancel_handle);
        });

        if self.debug_mode {
            let command = crate::agents::build_debug_command_line(&*self.provider, &config);
            on_log(crate::agents::build_debug_log_line(stage, &command, session_id));
        }

        let provider = self.provider.clone();
        let runner = self.stage_runner.clone();
        let start_time = std::time::Instant::now();
        let spawn_result = tokio::task::spawn_blocking(move || {
            runner.run(&*provider, &config, Some(on_log), Some(on_spawn))
        })
        .await;

        let result = match spawn_result {
            Ok(Ok(r)) => r,
            err => {
                let duration_secs = start_time.elapsed().as_secs_f64();
                let err_msg = match err {
                    Err(e) => format!("Stage task failed: {}", e),
                    Ok(Err(e)) => format!("Stage execution failed: {}", e),
                    Ok(Ok(_)) => unreachable!(),
                };
                let _ = self.db.update_run_status(
                    &sub_run.id, RunStatus::Error, None, Some(&err_msg),
                );
                let _ = self.db.set_run_metadata(
                    &sub_run.id,
                    &serde_json::json!({ "duration_secs": duration_secs }),
                );
                {
                    let mut handles = self
                        .cancel_handles
                        .lock()
                        .expect("cancel handles mutex poisoned");
                    handles.remove(&sub_run_id_for_cleanup);
                }
                self.emit_stage_event(
                    stage,
                    RunStatus::Error.as_str(),
                    Some(sub_run.id.clone()),
                    Some(duration_secs),
                );
                return Err(err_msg);
            }
        };

        // Clean up cancel handles
        {
            let mut handles = self
                .cancel_handles
                .lock()
                .expect("cancel handles mutex poisoned");
            handles.remove(&sub_run_id_for_cleanup);
        }

        if result.status == RunOutcome::Cancelled {
            cancelled.store(true, Ordering::Relaxed);
        }

        let duration_secs = start_time.elapsed().as_secs_f64();

        let status = match result.status {
            RunOutcome::Success => RunStatus::Finished,
            RunOutcome::Error => RunStatus::Error,
            RunOutcome::Timeout => RunStatus::Error,
            RunOutcome::Cancelled => RunStatus::Aborted,
        };

        self.db
            .update_run_status(
                &sub_run.id,
                status.clone(),
                result.exit_code,
                result.summary.as_deref(),
            )
            .map_err(|e| format!("Failed to update sub-run status: {}", e))?;

        {
            let stdout = result.captured_stdout.as_deref().unwrap_or("");
            let cost_data = self.extract_cost(stdout, &stage_model, duration_secs);

            let mut metadata = serde_json::json!({
                "duration_secs": duration_secs,
                "stage_model": &stage_model,
            });

            if let Some(ref cost) = cost_data {
                metadata["cost"] = serde_json::to_value(cost).unwrap_or_default();
            }

            if !self.agent_config.is_empty() {
                metadata["agent_config"] =
                    serde_json::to_value(&self.agent_config).unwrap_or_default();
            }

            if result.status == RunOutcome::Success && !stdout.is_empty() {
                let extracted_output = self.extract_text(stdout);

                let truncated_output = if extracted_output.len() > 50_000 {
                    let safe_boundary = extracted_output
                        .char_indices()
                        .take_while(|(idx, _)| *idx < 50_000)
                        .last()
                        .map(|(idx, c)| (idx + c.len_utf8()).min(extracted_output.len()))
                        .unwrap_or(0);
                    format!("{}...[truncated]", &extracted_output[..safe_boundary])
                } else {
                    extracted_output
                };

                metadata["stage_output"] = serde_json::Value::String(truncated_output);
            }

            if let Err(e) = self.db.set_run_metadata(&sub_run.id, &metadata) {
                tracing::warn!("Failed to save stage metadata: {}", e);
            }
        }

        self.emit_stage_event(
            stage,
            status.as_str(),
            Some(sub_run.id.clone()),
            Some(duration_secs),
        );

        if result.status != RunOutcome::Success {
            return Err(format!(
                "Stage '{}' failed with status {:?}",
                stage, result.status
            ));
        }

        if dir_override.is_none() {
            if let Some(ref stdout) = result.captured_stdout {
                if let Some(sid) = self.provider.extract_session_id(stdout) {
                    let is_new = self
                        .get_workflow_session_id()
                        .as_ref()
                        .map(|old| *old != sid)
                        .unwrap_or(true);
                    if is_new {
                        tracing::info!(
                            "Captured workflow session id from '{}' stage: {}",
                            stage,
                            sid,
                        );
                        self.set_workflow_session_id(&sid);
                    }
                } else if session_id.is_none() {
                    tracing::warn!(
                        "No session_id found in agent output for stage '{}' — subsequent stages will not use --resume",
                        stage,
                    );
                }
            }
        }

        tracing::info!("Stage '{}' completed in {:.1}s", stage, duration_secs);
        Ok(result)
    }

    /// Create the log callback for a stage
    fn create_log_callback(&self) -> Arc<LogCallback> {
        let db_for_logs = self.db.clone();
        let window_for_logs = self.window.clone();
        let app_handle_for_logs = self.app_handle.clone();
        let parent_run_id_for_logs = self.parent_run_id.clone();
        let ticket_id_for_logs = self.ticket.id.clone();
        let db_agent_type = self.agent_id.clone();

        Arc::new(Box::new(move |log: LogLine| {
            let stream_name = match log.stream {
                LogStream::Stdout => "stdout",
                LogStream::Stderr => "stderr",
            };
            let normalized_event = NormalizedEvent {
                run_id: parent_run_id_for_logs.clone(),
                ticket_id: ticket_id_for_logs.clone(),
                agent_type: db_agent_type.clone(),
                event_type: EventType::Custom(format!("log_{}", stream_name)),
                payload: AgentEventPayload {
                    raw: Some(log.content.clone()),
                    structured: None,
                },
                timestamp: log.timestamp,
            };
            if let Err(e) = db_for_logs.create_event(&normalized_event) {
                tracing::error!("Failed to persist log event: {}", e);
            }

            #[derive(serde::Serialize, Clone)]
            #[serde(rename_all = "camelCase")]
            struct AgentLogEvent {
                run_id: String,
                stream: String,
                content: String,
                timestamp: String,
            }
            let event = AgentLogEvent {
                run_id: parent_run_id_for_logs.clone(),
                stream: stream_name.to_string(),
                content: log.content,
                timestamp: log.timestamp.to_rfc3339(),
            };

            if let Some(ref window) = window_for_logs {
                if let Err(e) = window.emit("agent-log", &event) {
                    tracing::error!("Failed to emit agent-log event via window: {}", e);
                }
            } else if let Some(ref app_handle) = app_handle_for_logs {
                if let Err(e) = app_handle.emit("agent-log", &event) {
                    tracing::error!("Failed to emit agent-log event via app_handle: {}", e);
                }
            }
        }))
    }

    /// Emit a stage event to the frontend
    pub(super) fn emit_stage_event(
        &self,
        stage: &str,
        status: &str,
        sub_run_id: Option<String>,
        duration_secs: Option<f64>,
    ) {
        self.emit_stage_event_with_progress(stage, status, sub_run_id, duration_secs, None);
    }

    pub(super) fn emit_stage_event_with_progress(
        &self,
        stage: &str,
        status: &str,
        sub_run_id: Option<String>,
        duration_secs: Option<f64>,
        implementation_progress: Option<super::config::ImplementationProgress>,
    ) {
        let event = StageEvent {
            parent_run_id: self.parent_run_id.clone(),
            stage: stage.to_string(),
            status: status.to_string(),
            sub_run_id,
            duration_secs,
            implementation_progress,
        };
        if let Err(e) = self.emit_event("agent-stage-update", &event) {
            tracing::warn!("Failed to emit stage event: {}", e);
        }
    }

    /// Run the iterative code review loop using the model from stage_configs.
    pub(super) async fn run_code_review_loop(&self) -> Result<(), String> {
        let model = self.get_stage_model("code-review");
        self.run_code_review_loop_inner(
            &model,
            self.code_review_max_iterations,
            None,
            None,
        )
        .await
    }

    /// Run the iterative code review loop with an explicit model override.
    pub(super) async fn run_code_review_loop_with_model(
        &self,
        model: &str,
    ) -> Result<(), String> {
        self.run_code_review_loop_inner(
            model,
            self.code_review_max_iterations,
            None,
            None,
        )
        .await
    }

    /// For workspace tickets, append the combined workspace diff context to a prompt.
    pub(super) fn append_workspace_context_to_prompt(&self, prompt: &str) -> String {
        match self.build_workspace_diff_context() {
            Some(ws_context) => format!(
                "{}\n\n{}\n\n\
                 IMPORTANT: The git commands above will only show changes for the primary \
                 project. This ticket spans multiple projects in a workspace. The full \
                 workspace diff is provided above — use it to see changes in ALL projects. \
                 You can also access other project directories via `--add-dir` paths.",
                prompt, ws_context
            ),
            None => prompt.to_string(),
        }
    }

    /// Build diff context using the orchestrator's known worktree paths
    /// (repo_path + workspace_paths) rather than re-resolving from the DB via
    /// get_ticket_working_dirs, which can fall back to main checkout paths.
    fn build_workspace_diff_context(&self) -> Option<String> {
        let workspace_id = self.ticket.workspace_id.as_ref()?;
        if self.workspace_paths.is_empty() {
            return None;
        }

        // Re-read from DB since self.ticket is a snapshot that won't reflect
        // branch names set during the branch stage of the current run.
        let current_branch = self.db.get_ticket(&self.ticket.id)
            .ok()
            .and_then(|t| t.branch_name)
            .or_else(|| self.worktree_branch.clone());
        let branch = match current_branch.as_deref() {
            Some(b) if !b.is_empty() => b,
            _ => return None,
        };

        let projects = self.db.get_workspace_projects(workspace_id).ok()?;
        if projects.len() < 2 {
            return None;
        }

        // Build (project_name, worktree_path) pairs from the orchestrator's
        // already-resolved paths. The primary project maps to repo_path;
        // secondary projects map to workspace_paths entries by matching
        // via resolve_working_dir_strict.
        let mut dir_pairs: Vec<(String, String)> = Vec::new();
        for project in &projects {
            let worktree_dir = match crate::commands::next_steps::resolve_working_dir_strict(&project.path, branch) {
                Ok(resolved) => resolved,
                Err(_) => {
                    tracing::warn!(
                        "No worktree found for project '{}', excluding from diff context",
                        project.name
                    );
                    continue;
                }
            };
            dir_pairs.push((project.name.clone(), worktree_dir));
        }

        if dir_pairs.is_empty() {
            return None;
        }

        let mut sections = Vec::new();
        for (project_name, working_dir) in &dir_pairs {
            let default_branch = crate::commands::next_steps::get_default_branch(working_dir)
                .unwrap_or_else(|_| "origin/main".to_string());

            let stat = std::process::Command::new("git")
                .args(["diff", "--stat", &format!("{}...{}", default_branch, branch)])
                .current_dir(working_dir)
                .output();

            let diff = std::process::Command::new("git")
                .args(["diff", &format!("{}...{}", default_branch, branch)])
                .current_dir(working_dir)
                .output();

            let stat_text = stat
                .as_ref()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                .unwrap_or_default();

            let diff_text = diff
                .as_ref()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                .unwrap_or_default();

            if !diff_text.trim().is_empty() {
                sections.push(format!(
                    "### Project: {}\n\nWorking directory: `{}`\n\n```\n{}\n```\n\nFull diff:\n```diff\n{}\n```",
                    project_name, working_dir, stat_text.trim(), diff_text.trim()
                ));
            } else {
                sections.push(format!(
                    "### Project: {}\n\nNo changes on this branch.",
                    project_name
                ));
            }
        }

        if sections.iter().all(|s| s.contains("No changes")) {
            return None;
        }

        Some(format!(
            "## Workspace Change Set\n\n\
             This ticket spans multiple projects in a workspace. Below is the combined \
             diff across all projects. You MUST review changes in ALL projects, not just \
             the primary working directory.\n\n{}",
            sections.join("\n\n---\n\n")
        ))
    }

    async fn run_code_review_loop_inner(
        &self,
        model: &str,
        max_iterations: usize,
        timeout_override: Option<u64>,
        retries_override: Option<u32>,
    ) -> Result<(), String> {
        if max_iterations == 0 {
            tracing::info!("Code review loop disabled (max_iterations = 0)");
            return Ok(());
        }

        let display_max = if max_iterations == usize::MAX {
            "unlimited".to_string()
        } else {
            max_iterations.to_string()
        };

        tracing::info!(
            "Starting code review loop for ticket {} (max {} iterations, model={})",
            self.ticket.id,
            display_max,
            model,
        );

        let timeout = timeout_override.unwrap_or(self.stage_timeout_secs);
        let retries = retries_override.unwrap_or(self.stage_max_retries);
        let custom_dir = self.custom_commands_dir();

        for iteration in 1..=max_iterations {
            if self.is_cancelled() {
                return Err("Workflow cancelled".to_string());
            }

            tracing::info!("Code review iteration {}/{}", iteration, display_max);

            let base_prompt = generate_command_prompt("code-review", custom_dir.as_deref());
            let review_prompt = self.append_workspace_context_to_prompt(&base_prompt);

            let review_result = self
                .run_stage_with_overrides("code-review", &review_prompt, model, timeout, retries)
                .await?;

            let sub_run_id = review_result.run_id.clone();
            let raw_output = review_result.captured_stdout.unwrap_or_default();
            let text = self.extract_text(&raw_output);
            let structured = parse_structured_review(&text);
            let issue_count = structured.as_ref().map(|o| o.issues_found)
                .or_else(|| parse_code_review_issues(&text));
            let issues_section = extract_issues_with_parsed(structured.as_ref(), &text);

            let mut iteration_meta = serde_json::json!({
                "code_review_iteration": iteration,
                "code_review_issues_found": issue_count,
                "code_review_issues_section": &issues_section,
            });
            if let Some(ref output) = structured {
                if let Ok(issues_json) = serde_json::to_value(&output.issues) {
                    iteration_meta["code_review_issues"] = issues_json;
                }
            }
            if let Err(e) = self.db.merge_run_metadata(&sub_run_id, &iteration_meta) {
                tracing::warn!("Failed to store code review iteration metadata: {}", e);
            }

            self.emit_code_review_iteration(
                iteration,
                issue_count,
                &sub_run_id,
                if issue_count == Some(0) { "finished" } else { "running" },
            );

            match issue_count {
                Some(0) => {
                    tracing::info!(
                        "Code review complete: no issues found (iteration {})",
                        iteration
                    );
                    return Ok(());
                }
                Some(count) => {
                    tracing::info!(
                        "Found {} issues in iteration {}, running fix phase",
                        count,
                        iteration
                    );

                    let base_fix_prompt =
                        generate_command_prompt("code-review-fix", custom_dir.as_deref());
                    let fix_prompt = format!(
                        "{}\n\n## Issues to Address\n\n{}",
                        base_fix_prompt, issues_section
                    );
                    self.run_stage_with_overrides("code-review-fix", &fix_prompt, model, timeout, retries)
                        .await?;
                }
                None => {
                    tracing::warn!(
                        "Could not parse ISSUES_FOUND from code review output (iteration {}), \
                         running fix phase with full review text",
                        iteration
                    );

                    let base_fix_prompt =
                        generate_command_prompt("code-review-fix", custom_dir.as_deref());
                    let fix_prompt = format!(
                        "{}\n\n## Issues to Address\n\n{}",
                        base_fix_prompt, text
                    );
                    self.run_stage_with_overrides("code-review-fix", &fix_prompt, model, timeout, retries)
                        .await?;
                }
            }
        }

        tracing::warn!(
            "Code review reached max iterations ({}) for ticket {} without resolving all issues",
            display_max,
            self.ticket.id
        );

        Ok(())
    }

    /// Emit a code-review iteration event to the frontend.
    fn emit_code_review_iteration(
        &self,
        iteration: usize,
        issues_found: Option<usize>,
        sub_run_id: &str,
        status: &str,
    ) {
        let event = super::config::CodeReviewIterationEvent {
            parent_run_id: self.parent_run_id.clone(),
            iteration,
            issues_found,
            sub_run_id: sub_run_id.to_string(),
            status: status.to_string(),
        };
        if let Err(e) = self.emit_event("agent-code-review-update", &event) {
            tracing::warn!("Failed to emit code review iteration event: {}", e);
        }
    }
}
