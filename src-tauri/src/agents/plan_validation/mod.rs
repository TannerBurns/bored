//! Plan clarification detection for agent workflows.

use crate::agents::spawner;
use crate::agents::AgentRunConfig;
use crate::db::{CreateRun, RunStatus};

mod config;
mod parsing;
mod prompts;

pub use config::{
    AutoClarificationAction, AutoClarificationResult, PlanValidationConfig, PlanValidationError,
    PlanValidationResult,
};
pub use parsing::{parse_auto_clarification_response, parse_validation_response};
pub use prompts::{
    build_auto_clarification_prompt, build_clarification_message_prompt,
    build_plan_validation_prompt, build_spec_rewrite_prompt,
};

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

    let agent_type = config.agent_id.clone();

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
        agent_id: config.agent_id.clone(),
        ticket_id: config.ticket_id.clone(),
        run_id: run_id.clone(),
        repo_path: config.repo_path.clone(),
        prompt,
        timeout_secs: Some(config.timeout_secs),
        model: config.model.clone(),
        agent_config: config.agent_config.clone(),
        session_id: None,
        workspace_file: None,
        workspace_paths: vec![],
        debug_mode: false,
    };

    let db = config.db.clone();
    let provider = config.provider.clone();
    let model_for_cost = config.model.clone().unwrap_or_default();
    let start_time = std::time::Instant::now();

    let result = tokio::task::spawn_blocking(move || {
        spawner::run_agent_via_provider(&*provider, &agent_config, None)
    }).await;

    match result {
        Ok(Ok(agent_result)) => {
            let duration_secs = start_time.elapsed().as_secs_f64();
            let exit_code = agent_result.exit_code;
            let status = if exit_code == Some(0) {
                RunStatus::Finished
            } else {
                RunStatus::Error
            };

            let stdout = agent_result.captured_stdout.as_deref().unwrap_or("");
            let cost_data = crate::agents::provider::extract_cost_with_overrides(
                &*config.provider,
                stdout,
                &model_for_cost,
                &config.agent_config,
                duration_secs,
            );
            let mut metadata = serde_json::json!({ "duration_secs": duration_secs });
            if let Some(ref cost) = cost_data {
                metadata["cost"] = serde_json::to_value(cost).unwrap_or_default();
            }
            if let Err(e) = db.set_run_metadata(&run.id, &metadata) {
                tracing::warn!("Failed to save plan-validation cost metadata: {}", e);
            }

            let validation_result = agent_result.captured_stdout.as_ref().and_then(|output| {
                let text = config.provider.extract_text(output);
                match parse_validation_response(&text) {
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

    let agent_type = config.agent_id.clone();

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
        agent_id: config.agent_id.clone(),
        ticket_id: config.ticket_id.clone(),
        run_id: run_id.clone(),
        repo_path: config.repo_path.clone(),
        prompt,
        timeout_secs: Some(config.timeout_secs),
        model: config.model.clone(),
        agent_config: config.agent_config.clone(),
        session_id: None,
        workspace_file: None,
        workspace_paths: vec![],
        debug_mode: false,
    };

    let db = config.db.clone();
    let provider = config.provider.clone();
    let model_for_cost = config.model.clone().unwrap_or_default();
    let start_time = std::time::Instant::now();

    let result = tokio::task::spawn_blocking(move || {
        spawner::run_agent_via_provider(&*provider, &agent_config, None)
    }).await;

    match result {
        Ok(Ok(agent_result)) => {
            let duration_secs = start_time.elapsed().as_secs_f64();
            let exit_code = agent_result.exit_code;
            let status = if exit_code == Some(0) {
                RunStatus::Finished
            } else {
                RunStatus::Error
            };

            let stdout = agent_result.captured_stdout.as_deref().unwrap_or("");
            let cost_data = crate::agents::provider::extract_cost_with_overrides(
                &*config.provider,
                stdout,
                &model_for_cost,
                &config.agent_config,
                duration_secs,
            );
            let mut metadata = serde_json::json!({ "duration_secs": duration_secs });
            if let Some(ref cost) = cost_data {
                metadata["cost"] = serde_json::to_value(cost).unwrap_or_default();
            }
            if let Err(e) = db.set_run_metadata(&run.id, &metadata) {
                tracing::warn!("Failed to save clarification-gen cost metadata: {}", e);
            }

            let message = agent_result
                .captured_stdout
                .as_ref()
                .map(|output| config.provider.extract_text(output))
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

/// Rewrite a task specification by combining the original description with
/// the user's answers to clarification questions.
pub async fn rewrite_task_with_clarification(
    config: &PlanValidationConfig,
    original_description: &str,
    clarification_questions: &str,
    user_answers: &str,
) -> Result<String, PlanValidationError> {
    let run_id = uuid::Uuid::new_v4().to_string();

    tracing::info!(
        "Rewriting task spec for ticket {} (desc={} chars, questions={} chars, answers={} chars)",
        config.ticket_id,
        original_description.len(),
        clarification_questions.len(),
        user_answers.len(),
    );

    let agent_type = config.agent_id.clone();

    let run = config
        .db
        .create_run(&CreateRun {
            ticket_id: config.ticket_id.clone(),
            agent_type,
            repo_path: config.repo_path.to_string_lossy().to_string(),
            parent_run_id: Some(config.parent_run_id.clone()),
            stage: Some("spec-rewrite".to_string()),
            ..Default::default()
        })
        .map_err(|e| PlanValidationError::RunCreationFailed(e.to_string()))?;

    if let Err(e) = config
        .db
        .update_run_status(&run.id, RunStatus::Running, None, None)
    {
        tracing::warn!("Failed to update spec-rewrite run status: {}", e);
    }

    let prompt =
        build_spec_rewrite_prompt(original_description, clarification_questions, user_answers);

    let agent_config = AgentRunConfig {
        agent_id: config.agent_id.clone(),
        ticket_id: config.ticket_id.clone(),
        run_id: run_id.clone(),
        repo_path: config.repo_path.clone(),
        prompt,
        timeout_secs: Some(config.timeout_secs),
        model: config.model.clone(),
        agent_config: config.agent_config.clone(),
        session_id: None,
        workspace_file: None,
        workspace_paths: vec![],
        debug_mode: false,
    };

    let db = config.db.clone();
    let provider = config.provider.clone();
    let model_for_cost = config.model.clone().unwrap_or_default();
    let start_time = std::time::Instant::now();

    let result = tokio::task::spawn_blocking(move || {
        spawner::run_agent_via_provider(&*provider, &agent_config, None)
    })
    .await;

    match result {
        Ok(Ok(agent_result)) => {
            let duration_secs = start_time.elapsed().as_secs_f64();
            let exit_code = agent_result.exit_code;
            let status = if exit_code == Some(0) {
                RunStatus::Finished
            } else {
                RunStatus::Error
            };

            let stdout = agent_result.captured_stdout.as_deref().unwrap_or("");
            let cost_data = crate::agents::provider::extract_cost_with_overrides(
                &*config.provider,
                stdout,
                &model_for_cost,
                &config.agent_config,
                duration_secs,
            );
            let mut metadata = serde_json::json!({ "duration_secs": duration_secs });
            if let Some(ref cost) = cost_data {
                metadata["cost"] = serde_json::to_value(cost).unwrap_or_default();
            }
            if let Err(e) = db.set_run_metadata(&run.id, &metadata) {
                tracing::warn!("Failed to save spec-rewrite cost metadata: {}", e);
            }

            let message = agent_result
                .captured_stdout
                .as_ref()
                .map(|output| config.provider.extract_text(output))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            let _ = db.update_run_status(&run.id, status, exit_code, message.as_deref());

            message.ok_or_else(|| {
                PlanValidationError::SpawnFailed(
                    "Spec rewrite agent produced no output".to_string(),
                )
            })
        }
        Ok(Err(spawn_error)) => {
            tracing::error!("Spec rewrite agent spawn failed: {}", spawn_error);
            let _ = db.update_run_status(
                &run.id,
                RunStatus::Error,
                None,
                Some(&spawn_error.to_string()),
            );
            Err(PlanValidationError::SpawnFailed(spawn_error.to_string()))
        }
        Err(join_error) => {
            tracing::error!("Spec rewrite agent task panicked: {}", join_error);
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

/// Attempt to resolve a clarification autonomously using an agent.
pub async fn auto_resolve_clarification(
    config: &PlanValidationConfig,
    plan: &str,
    clarification_reason: &str,
    ticket_description: &str,
    task_content: &str,
    completed_task_summaries: &str,
) -> Result<AutoClarificationResult, PlanValidationError> {
    let run_id = uuid::Uuid::new_v4().to_string();

    tracing::info!(
        "Running auto-clarification for ticket {} (reason: {})",
        config.ticket_id,
        clarification_reason,
    );

    let agent_type = config.agent_id.clone();

    let run = config
        .db
        .create_run(&CreateRun {
            ticket_id: config.ticket_id.clone(),
            agent_type,
            repo_path: config.repo_path.to_string_lossy().to_string(),
            parent_run_id: Some(config.parent_run_id.clone()),
            stage: Some("auto-clarification".to_string()),
            ..Default::default()
        })
        .map_err(|e| PlanValidationError::RunCreationFailed(e.to_string()))?;

    if let Err(e) = config
        .db
        .update_run_status(&run.id, RunStatus::Running, None, None)
    {
        tracing::warn!("Failed to update auto-clarification run status: {}", e);
    }

    let prompt = build_auto_clarification_prompt(
        plan,
        clarification_reason,
        ticket_description,
        task_content,
        completed_task_summaries,
    );

    let agent_config = AgentRunConfig {
        agent_id: config.agent_id.clone(),
        ticket_id: config.ticket_id.clone(),
        run_id: run_id.clone(),
        repo_path: config.repo_path.clone(),
        prompt,
        timeout_secs: Some(config.timeout_secs),
        model: config.model.clone(),
        agent_config: config.agent_config.clone(),
        session_id: None,
        workspace_file: None,
        workspace_paths: vec![],
        debug_mode: false,
    };

    let db = config.db.clone();
    let provider = config.provider.clone();
    let model_for_cost = config.model.clone().unwrap_or_default();
    let start_time = std::time::Instant::now();

    let result = tokio::task::spawn_blocking(move || {
        spawner::run_agent_via_provider(&*provider, &agent_config, None)
    })
    .await;

    match result {
        Ok(Ok(agent_result)) => {
            let duration_secs = start_time.elapsed().as_secs_f64();
            let exit_code = agent_result.exit_code;
            let status = if exit_code == Some(0) {
                RunStatus::Finished
            } else {
                RunStatus::Error
            };

            let stdout = agent_result.captured_stdout.as_deref().unwrap_or("");
            let cost_data = crate::agents::provider::extract_cost_with_overrides(
                &*config.provider,
                stdout,
                &model_for_cost,
                &config.agent_config,
                duration_secs,
            );
            let mut metadata = serde_json::json!({ "duration_secs": duration_secs });
            if let Some(ref cost) = cost_data {
                metadata["cost"] = serde_json::to_value(cost).unwrap_or_default();
            }
            if let Err(e) = db.set_run_metadata(&run.id, &metadata) {
                tracing::warn!("Failed to save auto-clarification cost metadata: {}", e);
            }

            let text = agent_result
                .captured_stdout
                .as_ref()
                .map(|output| config.provider.extract_text(output))
                .unwrap_or_default();

            let parsed = parse_auto_clarification_response(&text);

            let output_summary = parsed
                .as_ref()
                .ok()
                .map(|r| r.reason.as_str())
                .unwrap_or("parse failed");
            let _ = db.update_run_status(&run.id, status, exit_code, Some(output_summary));

            tracing::info!(
                "Auto-clarification completed: exit_code={:?}, action={:?}",
                exit_code,
                parsed
                    .as_ref()
                    .ok()
                    .map(|r| std::mem::discriminant(&r.action)),
            );

            parsed
        }
        Ok(Err(spawn_error)) => {
            tracing::error!("Auto-clarification agent spawn failed: {}", spawn_error);
            let _ = db.update_run_status(
                &run.id,
                RunStatus::Error,
                None,
                Some(&spawn_error.to_string()),
            );
            Err(PlanValidationError::SpawnFailed(spawn_error.to_string()))
        }
        Err(join_error) => {
            tracing::error!("Auto-clarification agent task panicked: {}", join_error);
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
