//! Plan clarification detection for agent workflows.

use crate::agents::spawner;
use crate::agents::{extract_agent_text, AgentRunConfig};
use crate::db::{AgentType, CreateRun, RunStatus};

mod config;
mod parsing;
mod prompts;

pub use config::{PlanValidationConfig, PlanValidationError, PlanValidationResult};
pub use parsing::parse_validation_response;
pub use prompts::{build_clarification_message_prompt, build_plan_validation_prompt};

/// Run a validation agent to check if a plan needs clarification (fail-open).
pub async fn validate_plan_for_clarification(
    config: &PlanValidationConfig,
    plan: &str,
) -> Result<PlanValidationResult, PlanValidationError> {
    let run_id = uuid::Uuid::new_v4().to_string();

    tracing::info!(
        "Starting plan validation for ticket {}, plan length: {} chars",
        config.ticket_id,
        plan.len()
    );

    let agent_type = match config.agent_kind {
        crate::agents::AgentKind::Cursor => AgentType::Cursor,
        crate::agents::AgentKind::Claude => AgentType::Claude,
    };

    let run = config
        .db
        .create_run(&CreateRun {
            ticket_id: config.ticket_id.clone(),
            agent_type,
            repo_path: config.repo_path.to_string_lossy().to_string(),
            parent_run_id: Some(config.parent_run_id.clone()),
            stage: Some("plan-validation".to_string()),
            ..Default::default()
        })
        .map_err(|e| PlanValidationError::RunCreationFailed(e.to_string()))?;

    if let Err(e) = config
        .db
        .update_run_status(&run.id, RunStatus::Running, None, None)
    {
        tracing::warn!("Failed to update validation run status: {}", e);
    }

    let prompt = build_plan_validation_prompt(plan);

    let agent_config = AgentRunConfig {
        kind: config.agent_kind,
        ticket_id: config.ticket_id.clone(),
        run_id: run_id.clone(),
        repo_path: config.repo_path.clone(),
        prompt,
        timeout_secs: Some(config.timeout_secs),
        api_url: config.api_url.clone(),
        api_token: config.api_token.clone(),
        model: config.model.clone(),
        claude_api_config: config.claude_api_config.clone(),
    };

    let db = config.db.clone();

    let result = tokio::task::spawn_blocking(move || spawner::run_agent(agent_config, None)).await;

    match result {
        Ok(Ok(agent_result)) => {
            let exit_code = agent_result.exit_code;
            let status = if exit_code == Some(0) {
                RunStatus::Finished
            } else {
                RunStatus::Error
            };

            tracing::debug!(
                "Plan validation agent completed: exit_code={:?}, stdout_len={:?}",
                exit_code,
                agent_result.captured_stdout.as_ref().map(|s| s.len())
            );

            let validation_result = agent_result.captured_stdout.as_ref().and_then(|output| {
                match parse_validation_response(output) {
                    Ok(result) => Some(result),
                    Err(e) => {
                        tracing::warn!("Failed to parse plan validation response: {}", e);
                        None
                    }
                }
            });

            if let Err(e) = db.update_run_status(
                &run.id,
                status.clone(),
                exit_code,
                validation_result.as_ref().map(|r| r.reason.as_str()),
            ) {
                tracing::warn!("Failed to update validation run status: {}", e);
            }

            tracing::info!(
                "Plan validation completed: exit_code={:?}, needs_clarification={:?}",
                exit_code,
                validation_result.as_ref().map(|r| r.needs_clarification)
            );

            if validation_result.is_none() {
                tracing::warn!(
                    "No valid validation result parsed, using default (needs_clarification=false). \
                    stdout_present={}, status={:?}",
                    agent_result.captured_stdout.is_some(),
                    status
                );
            }

            Ok(validation_result.unwrap_or_default())
        }
        Ok(Err(spawn_error)) => {
            tracing::error!("Validation agent spawn failed: {}", spawn_error);
            let _ = db.update_run_status(
                &run.id,
                RunStatus::Error,
                None,
                Some(&spawn_error.to_string()),
            );
            Ok(PlanValidationResult::default())
        }
        Err(join_error) => {
            tracing::error!("Validation agent task panicked: {}", join_error);
            let _ = db.update_run_status(
                &run.id,
                RunStatus::Error,
                None,
                Some(&join_error.to_string()),
            );
            Ok(PlanValidationResult::default())
        }
    }
}

/// Generate a clarification message asking the user for needed information.
pub async fn generate_clarification_message(
    config: &PlanValidationConfig,
    plan: &str,
) -> Result<String, PlanValidationError> {
    let run_id = uuid::Uuid::new_v4().to_string();

    tracing::info!(
        "Generating clarification message for ticket {}, plan length: {} chars",
        config.ticket_id,
        plan.len()
    );

    let agent_type = match config.agent_kind {
        crate::agents::AgentKind::Cursor => AgentType::Cursor,
        crate::agents::AgentKind::Claude => AgentType::Claude,
    };

    let run = config
        .db
        .create_run(&CreateRun {
            ticket_id: config.ticket_id.clone(),
            agent_type,
            repo_path: config.repo_path.to_string_lossy().to_string(),
            parent_run_id: Some(config.parent_run_id.clone()),
            stage: Some("clarification-gen".to_string()),
            ..Default::default()
        })
        .map_err(|e| PlanValidationError::RunCreationFailed(e.to_string()))?;

    if let Err(e) = config
        .db
        .update_run_status(&run.id, RunStatus::Running, None, None)
    {
        tracing::warn!("Failed to update clarification run status: {}", e);
    }

    let prompt = build_clarification_message_prompt(plan);

    let agent_config = AgentRunConfig {
        kind: config.agent_kind,
        ticket_id: config.ticket_id.clone(),
        run_id: run_id.clone(),
        repo_path: config.repo_path.clone(),
        prompt,
        timeout_secs: Some(config.timeout_secs),
        api_url: config.api_url.clone(),
        api_token: config.api_token.clone(),
        model: config.model.clone(),
        claude_api_config: config.claude_api_config.clone(),
    };

    let db = config.db.clone();

    let result = tokio::task::spawn_blocking(move || spawner::run_agent(agent_config, None)).await;

    match result {
        Ok(Ok(agent_result)) => {
            let exit_code = agent_result.exit_code;
            let status = if exit_code == Some(0) {
                RunStatus::Finished
            } else {
                RunStatus::Error
            };

            tracing::debug!(
                "Clarification agent completed: exit_code={:?}, stdout_len={:?}",
                exit_code,
                agent_result.captured_stdout.as_ref().map(|s| s.len())
            );

            let message = agent_result
                .captured_stdout
                .as_ref()
                .map(|output| extract_agent_text(output))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            let _ = db.update_run_status(&run.id, status, exit_code, message.as_deref());

            message.ok_or_else(|| {
                PlanValidationError::SpawnFailed(
                    "Clarification agent produced no output".to_string(),
                )
            })
        }
        Ok(Err(spawn_error)) => {
            tracing::error!("Clarification agent spawn failed: {}", spawn_error);
            let _ = db.update_run_status(
                &run.id,
                RunStatus::Error,
                None,
                Some(&spawn_error.to_string()),
            );
            Err(PlanValidationError::SpawnFailed(spawn_error.to_string()))
        }
        Err(join_error) => {
            tracing::error!("Clarification agent task panicked: {}", join_error);
            let _ = db.update_run_status(
                &run.id,
                RunStatus::Error,
                None,
                Some(&join_error.to_string()),
            );
            Err(PlanValidationError::SpawnFailed(join_error.to_string()))
        }
    }
}
