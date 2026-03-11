//! Workflow execution logic for agent runs.

use tauri::{Emitter, Window};

use super::config::RunnerConfig;
use super::events::{AgentCompleteEvent, AgentErrorEvent};
use crate::agents::orchestrator::{OrchestratorConfig, WorkflowOrchestrator};
use crate::db::{AuthorType, CreateComment, Database, RunStatus, Ticket};

pub(super) async fn execute_multi_stage_workflow(config: &RunnerConfig) -> Result<(), String> {
    tracing::info!("Starting multi-stage workflow for run {}", config.run_id);

    let workflow_settings = config
        .workflow_settings
        .clone()
        .unwrap_or_else(|| {
            tracing::warn!("No shared WorkflowSettings on RunnerConfig — using empty defaults");
            std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()))
        });

    let orchestrator = WorkflowOrchestrator::new(OrchestratorConfig {
        db: config.db.clone(),
        window: config.window.clone(),
        app_handle: config.app_handle.clone(),
        parent_run_id: config.run_id.clone(),
        ticket: config.ticket.clone(),
        task: config.task.clone(),
        repo_path: config.repo_path.clone(),
        agent_id: config.agent_id.clone(),
        provider: config.provider.clone(),
        cancel_handles: config.cancel_handles.clone(),
        worktree_branch: config.worktree_branch.clone(),
        branch_already_created: config.branch_already_created,
        is_temp_branch: config.is_temp_branch,
        target_branch: config.target_branch.clone(),
        agent_config: config.agent_config.clone(),
        resume_from_stage: config.resume_from_stage.clone(),
        previous_run_id: config.previous_run_id.clone(),
        workflow_settings,
        stage_configs: config.stage_configs.clone(),
        code_review_max_iterations: config.code_review_max_iterations,
        stage_timeout_secs: config.stage_timeout_secs,
        stage_max_retries: config.stage_max_retries,
    });

    orchestrator.execute().await
}

pub(super) fn move_ticket_to_column(
    db: &Database,
    ticket: &Ticket,
    column_name: &str,
    window: Option<&Window>,
) {
    match db.find_column_by_name(&ticket.board_id, column_name) {
        Ok(Some(column)) => {
            if let Err(e) = db.move_ticket(&ticket.id, &column.id) {
                tracing::error!(
                    "Failed to move ticket {} to '{}': {}",
                    ticket.id,
                    column_name,
                    e
                );
            } else {
                tracing::info!("Moved ticket {} to column '{}'", ticket.id, column_name);
                if let Some(window) = window {
                    let _ = window.emit(
                        "ticket-moved",
                        serde_json::json!({
                            "ticketId": ticket.id,
                            "columnName": column_name,
                            "columnId": column.id,
                        }),
                    );
                }
            }
        }
        Ok(None) => {
            tracing::warn!(
                "Column '{}' not found for board {}",
                column_name,
                ticket.board_id
            );
        }
        Err(e) => {
            tracing::error!("Error finding column '{}': {}", column_name, e);
        }
    }
}

pub(super) fn handle_workflow_success(
    config: &RunnerConfig,
    duration_secs: f64,
) -> Result<super::config::RunnerResult, String> {
    tracing::info!(
        "Agent run {} completed successfully in {:.1}s",
        config.run_id,
        duration_secs
    );

    config
        .db
        .update_run_status(
            &config.run_id,
            RunStatus::Finished,
            Some(0),
            Some("Workflow completed successfully"),
        )
        .map_err(|e| format!("Failed to update run status: {}", e))?;

    if let Some(ref window) = config.window {
        let event = AgentCompleteEvent {
            run_id: config.run_id.clone(),
            status: "finished".to_string(),
            exit_code: Some(0),
            duration_secs,
        };
        if let Err(e) = window.emit("agent-complete", &event) {
            tracing::error!("Failed to emit agent-complete event: {}", e);
        }
    }

    Ok(super::config::RunnerResult {
        status: RunStatus::Finished,
        exit_code: Some(0),
        summary: Some("Workflow completed successfully".to_string()),
        duration_secs,
    })
}

pub(super) fn handle_workflow_error(
    config: &RunnerConfig,
    error: String,
    duration_secs: f64,
) -> Result<super::config::RunnerResult, String> {
    if error.starts_with("Plan requires user clarification:")
        || error.starts_with("Task deleted by auto-clarification:")
    {
        return handle_clarification_stop(config, duration_secs);
    }

    tracing::error!("Agent run {} failed: {}", config.run_id, error);

    config
        .db
        .update_run_status(
            &config.run_id,
            RunStatus::Error,
            None,
            Some(&format!("Workflow failed: {}", error)),
        )
        .map_err(|db_err| format!("Failed to update run status: {}", db_err))?;

    // Post error comment before the move to prevent stale clarification banner.
    let _ = config.db.create_comment(&CreateComment {
        ticket_id: config.ticket.id.clone(),
        author_type: AuthorType::System,
        body_md: format!("## Blocked: Workflow Error\n\n{}", error),
        metadata: Some(serde_json::json!({ "type": "error" })),
    });

    move_ticket_to_column(
        &config.db,
        &config.ticket,
        "Blocked",
        config.window.as_ref(),
    );

    if let Some(ref window) = config.window {
        let event = AgentErrorEvent {
            run_id: config.run_id.clone(),
            error: error.clone(),
        };
        if let Err(emit_err) = window.emit("agent-error", &event) {
            tracing::error!("Failed to emit agent-error event: {}", emit_err);
        }
    }

    Ok(super::config::RunnerResult {
        status: RunStatus::Error,
        exit_code: None,
        summary: Some(format!("Workflow failed: {}", error)),
        duration_secs,
    })
}

/// The orchestrator already posted the clarification comment and moved the
/// ticket to Blocked. Treat this as a successful stop — don't post an error
/// comment that would hide the clarification banner.
fn handle_clarification_stop(
    config: &RunnerConfig,
    duration_secs: f64,
) -> Result<super::config::RunnerResult, String> {
    tracing::info!(
        "Agent run {} stopped for user clarification in {:.1}s",
        config.run_id,
        duration_secs
    );

    config
        .db
        .update_run_status(
            &config.run_id,
            RunStatus::Finished,
            Some(0),
            Some("Waiting for user clarification"),
        )
        .map_err(|e| format!("Failed to update run status: {}", e))?;

    if let Some(ref t) = config.task {
        if let Err(e) = config.db.fail_task(&t.id) {
            tracing::warn!(
                "Failed to mark task {} as failed for clarification stop: {}",
                t.id, e
            );
        }
    }

    if let Some(ref window) = config.window {
        let event = AgentCompleteEvent {
            run_id: config.run_id.clone(),
            status: "finished".to_string(),
            exit_code: Some(0),
            duration_secs,
        };
        if let Err(e) = window.emit("agent-complete", &event) {
            tracing::error!("Failed to emit agent-complete event: {}", e);
        }
    }

    Ok(super::config::RunnerResult {
        status: RunStatus::Finished,
        exit_code: Some(0),
        summary: Some("Waiting for user clarification".to_string()),
        duration_secs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::orchestrator::CancelHandlesMap;
    use crate::agents::provider::AgentProvider;
    use crate::agents::AgentRunConfig;
    use crate::db::models::{CreateTicket, Priority, WorkflowType};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    #[derive(Debug)]
    struct StubProvider;

    impl AgentProvider for StubProvider {
        fn id(&self) -> &str { "stub" }
        fn display_name(&self) -> &str { "Stub" }
        fn build_command(&self, _: &AgentRunConfig) -> (String, Vec<String>) {
            ("stub".into(), vec![])
        }
        fn build_env_vars(&self, _: &AgentRunConfig) -> Vec<(String, String)> { vec![] }
        fn extract_text(&self, o: &str) -> String { o.into() }
        fn extract_cost(&self, _: &str, _: &str, _: f64) -> Option<crate::agents::cost::RunCostData> { None }
        fn is_available(&self) -> bool { false }
        fn get_version(&self) -> Option<String> { None }
        fn config_dir_name(&self) -> &str { ".stub" }
        fn command_instructions_subdir(&self) -> &str { "commands" }
        fn format_command_reference(&self, c: &str) -> String { format!("/{c}") }
        fn extract_session_id(&self, _output: &str) -> Option<String> { None }
    }

    fn make_test_config() -> RunnerConfig {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id,
            column_id: columns[0].id.clone(),
            title: "Test Ticket".to_string(),
            description_md: "test".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        }).unwrap();

        let run = db.create_run(&crate::db::CreateRun {
            ticket_id: ticket.id.clone(),
            agent_type: "stub".to_string(),
            repo_path: "/tmp/test".to_string(),
            parent_run_id: None,
            stage: None,
            resumed_from_run_id: None,
        }).unwrap();

        RunnerConfig {
            db,
            window: None,
            app_handle: None,
            ticket,
            task: None,
            run_id: run.id,
            repo_path: PathBuf::from("/tmp/test"),
            agent_id: "stub".to_string(),
            provider: Arc::new(StubProvider),
            cancel_handles: Arc::new(Mutex::new(HashMap::new())) as CancelHandlesMap,
            worktree_branch: None,
            branch_already_created: false,
            is_temp_branch: false,
            target_branch: None,
            timeout_secs: 3600,
            agent_config: HashMap::new(),
            code_review_max_iterations: 3,
            stage_timeout_secs: 3600,
            stage_max_retries: 2,
            resume_from_stage: None,
            previous_run_id: None,
            stage_configs: HashMap::new(),
            workflow_settings: None,
        }
    }

    fn make_test_config_with_task() -> RunnerConfig {
        use crate::db::models::{CreateTask, TaskType, TaskStatus};

        let mut config = make_test_config();
        let task = config.db.create_task(&CreateTask {
            ticket_id: config.ticket.id.clone(),
            task_type: TaskType::Custom,
            title: Some("Test Task".to_string()),
            content: None,
        }).unwrap();
        config.db.start_task(&task.id, &config.run_id).unwrap();
        let started = config.db.get_task(&task.id).unwrap();
        assert_eq!(started.status, TaskStatus::InProgress);
        config.task = Some(started);
        config
    }

    #[test]
    fn handle_workflow_error_routes_clarification_to_finished() {
        let config = make_test_config();
        let result = handle_workflow_error(
            &config,
            "Plan requires user clarification: unclear requirements".to_string(),
            1.5,
        );
        let result = result.unwrap();
        assert_eq!(result.status, RunStatus::Finished);
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(
            result.summary.as_deref(),
            Some("Waiting for user clarification")
        );
    }

    #[test]
    fn clarification_stop_fails_task_when_present() {
        use crate::db::models::TaskStatus;

        let config = make_test_config_with_task();
        let task_id = config.task.as_ref().unwrap().id.clone();

        let result = handle_workflow_error(
            &config,
            "Plan requires user clarification: unclear requirements".to_string(),
            1.5,
        );
        assert!(result.is_ok());

        let task = config.db.get_task(&task_id).unwrap();
        assert_eq!(
            task.status,
            TaskStatus::Failed,
            "task must be failed so resolve_clarification can reset it to pending"
        );
    }

    #[test]
    fn auto_clarification_delete_does_not_panic_on_missing_task() {
        let config = make_test_config_with_task();
        let task_id = config.task.as_ref().unwrap().id.clone();
        config.db.delete_task(&task_id).unwrap();

        let result = handle_workflow_error(
            &config,
            "Task deleted by auto-clarification: already completed".to_string(),
            2.0,
        );
        assert!(result.is_ok(), "should not panic when task is already deleted");
    }

    #[test]
    fn handle_workflow_error_routes_auto_clarification_delete_to_finished() {
        let config = make_test_config();
        let result = handle_workflow_error(
            &config,
            "Task deleted by auto-clarification: already completed".to_string(),
            2.0,
        );
        let result = result.unwrap();
        assert_eq!(result.status, RunStatus::Finished);
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(
            result.summary.as_deref(),
            Some("Waiting for user clarification")
        );
    }

    #[test]
    fn handle_workflow_error_routes_normal_error_to_error_status() {
        let config = make_test_config();
        let result = handle_workflow_error(
            &config,
            "Stage 'implement' failed with status Error".to_string(),
            3.0,
        );
        let result = result.unwrap();
        assert_eq!(result.status, RunStatus::Error);
        assert!(result.exit_code.is_none());
        assert!(result.summary.unwrap().contains("Workflow failed"));
    }

    #[test]
    fn handle_workflow_error_does_not_match_partial_prefix() {
        let config = make_test_config();
        let result = handle_workflow_error(
            &config,
            "Something about Plan requires user clarification but not at start".to_string(),
            1.0,
        );
        let result = result.unwrap();
        assert_eq!(
            result.status,
            RunStatus::Error,
            "should not match clarification when prefix is not at the start"
        );
    }
}
