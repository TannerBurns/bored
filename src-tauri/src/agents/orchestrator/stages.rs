//! Stage execution and retry logic for the workflow orchestrator.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::Emitter;

use super::code_review::{extract_issues_section, parse_code_review_issues};
use super::config::StageEvent;
use super::WorkflowOrchestrator;
use crate::agents::prompt::generate_command_prompt;
use crate::agents::{AgentRunConfig, AgentRunResult};
use crate::agents::{LogCallback, LogLine, LogStream, RunOutcome};
use crate::db::{AgentEventPayload, CreateRun, EventType, NormalizedEvent, RunStatus};

impl WorkflowOrchestrator {
    /// Run a single stage of the workflow with retry support
    pub(super) async fn run_stage(
        &self,
        stage: &str,
        prompt: &str,
    ) -> Result<AgentRunResult, String> {
        self.run_stage_inner(stage, prompt, None, None).await
    }

    /// Run a single stage, resuming an existing agent session.
    pub(super) async fn run_stage_with_session(
        &self,
        stage: &str,
        prompt: &str,
        session_id: Option<&str>,
    ) -> Result<AgentRunResult, String> {
        self.run_stage_inner(stage, prompt, None, session_id).await
    }

    /// Run a single stage with an explicit model override (used by auto-pilot).
    pub(super) async fn run_stage_with_model(
        &self,
        stage: &str,
        prompt: &str,
        model: &str,
    ) -> Result<AgentRunResult, String> {
        self.run_stage_inner(stage, prompt, Some(model), None).await
    }

    async fn run_stage_inner(
        &self,
        stage: &str,
        prompt: &str,
        model_override: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<AgentRunResult, String> {
        let max_attempts = self.stage_max_retries + 1;
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
                .run_stage_attempt(stage, prompt, attempt, max_attempts, model_override, attempt_session_id)
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

    /// Run a single attempt of a stage
    pub(super) async fn run_stage_attempt(
        &self,
        stage: &str,
        prompt: &str,
        attempt: u32,
        max_attempts: u32,
        model_override: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<AgentRunResult, String> {
        tracing::info!(
            "Starting stage '{}' attempt {}/{} for parent run {}",
            stage,
            attempt,
            max_attempts,
            self.parent_run_id
        );

        if attempt == 1 {
            self.emit_stage_event(stage, "running", None, None);
        }

        let db_agent_type = self.agent_id.clone();

        // Create sub-run in database
        let sub_run = self
            .db
            .create_run(&CreateRun {
                ticket_id: self.ticket.id.clone(),
                agent_type: db_agent_type,
                repo_path: self.repo_path.to_string_lossy().to_string(),
                parent_run_id: Some(self.parent_run_id.clone()),
                stage: Some(stage.to_string()),
                ..Default::default()
            })
            .map_err(|e| format!("Failed to create sub-run: {}", e))?;

        // Update sub-run status to running
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
            repo_path: self.repo_path.clone(),
            prompt: prompt.to_string(),
            timeout_secs: Some(self.stage_timeout_secs),
            model: Some(stage_model.clone()),
            agent_config: self.agent_config.clone(),
            session_id: session_id.map(|s| s.to_string()),
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

    /// Run the iterative code review loop (find issues, then fix, repeat until clean).
    pub(super) async fn run_code_review_loop(&self) -> Result<(), String> {
        let max_iterations = self.code_review_max_iterations;

        if max_iterations == 0 {
            tracing::info!("Code review loop disabled (max_iterations = 0)");
            return Ok(());
        }

        tracing::info!(
            "Starting code review loop for ticket {} (max {} iterations)",
            self.ticket.id,
            max_iterations
        );

        let custom_dir = self.custom_commands_dir();

        for iteration in 1..=max_iterations {
            if self.is_cancelled() {
                return Err("Workflow cancelled".to_string());
            }

            tracing::info!("Code review iteration {}/{}", iteration, max_iterations);

            let review_prompt = generate_command_prompt("code-review", custom_dir.as_deref());
            let review_result = self.run_stage("code-review", &review_prompt).await?;
            let raw_output = review_result.captured_stdout.unwrap_or_default();
            let text = self.extract_text(&raw_output);
            let issue_count = parse_code_review_issues(&text);

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

                    let issues_context = extract_issues_section(&text);
                    let base_fix_prompt = generate_command_prompt("code-review-fix", custom_dir.as_deref());
                    let fix_prompt = format!(
                        "{}\n\n## Issues to Address\n\n{}",
                        base_fix_prompt, issues_context
                    );
                    self.run_stage("code-review-fix", &fix_prompt).await?;
                }
                None => {
                    tracing::warn!(
                        "Could not parse ISSUES_FOUND from code review output (iteration {}), \
                         running fix phase with full review text",
                        iteration
                    );

                    let base_fix_prompt = generate_command_prompt("code-review-fix", custom_dir.as_deref());
                    let fix_prompt = format!(
                        "{}\n\n## Issues to Address\n\n{}",
                        base_fix_prompt, text
                    );
                    self.run_stage("code-review-fix", &fix_prompt).await?;
                }
            }
        }

        tracing::warn!(
            "Code review reached max iterations ({}) for ticket {} without resolving all issues",
            max_iterations,
            self.ticket.id
        );

        Ok(())
    }
}
