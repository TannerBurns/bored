//! Diagnostic agent for analyzing worktree and git failures.

use std::sync::Arc;
use tauri::AppHandle;

use crate::agents::provider::AgentProvider;
use crate::agents::spawner;
use crate::agents::AgentRunConfig;
use crate::db::{AgentType, AuthorType, CreateComment, CreateRun, Database, RunStatus};

mod context;
mod fallback;
mod prompts;

pub use context::{classify_worktree_error, DiagnosticContext, DiagnosticError};
pub use fallback::create_fallback_diagnostic_comment;
pub use prompts::build_diagnostic_prompt;

/// Run a diagnostic agent to analyze an error and post troubleshooting as a ticket comment.
#[allow(clippy::too_many_arguments)]
pub async fn run_diagnostic_agent(
    db: Arc<Database>,
    _app_handle: Option<AppHandle>,
    ticket_id: &str,
    context: DiagnosticContext,
    api_url: &str,
    api_token: &str,
    model: Option<String>,
    provider: Arc<dyn AgentProvider>,
    agent_config: std::collections::HashMap<String, serde_json::Value>,
) -> Result<(), DiagnosticError> {
    let run_id = uuid::Uuid::new_v4().to_string();
    let ticket_id_owned = ticket_id.to_string();
    let agent_id = provider.id().to_string();

    tracing::info!(
        "Starting diagnostic agent for ticket {}: error_type={:?}, operation={}",
        ticket_id,
        context.error_type,
        context.operation
    );

    let db_agent_type = AgentType::parse_agent(&agent_id);

    let run = db
        .create_run(&CreateRun {
            ticket_id: ticket_id.to_string(),
            agent_type: db_agent_type,
            repo_path: context.repo_path.to_string_lossy().to_string(),
            parent_run_id: None,
            stage: Some("diagnostic".to_string()),
            ..Default::default()
        })
        .map_err(|e| DiagnosticError::RunCreationFailed(e.to_string()))?;

    if let Err(e) = db.update_run_status(&run.id, RunStatus::Running, None, None) {
        tracing::warn!("Failed to update diagnostic run status: {}", e);
    }

    let prompt = build_diagnostic_prompt(&context);
    let config = AgentRunConfig {
        agent_id,
        ticket_id: ticket_id.to_string(),
        run_id: run_id.clone(),
        repo_path: context.repo_path.clone(),
        prompt,
        timeout_secs: Some(300),
        api_url: api_url.to_string(),
        api_token: api_token.to_string(),
        model,
        agent_config,
    };

    let provider_for_extract = provider.clone();
    let result = tokio::task::spawn_blocking(move || {
        spawner::run_agent_via_provider(&*provider, &config, None)
    }).await;

    match result {
        Ok(Ok(agent_result)) => {
            let exit_code = agent_result.exit_code;
            let status = if exit_code == Some(0) {
                RunStatus::Finished
            } else {
                RunStatus::Error
            };

            let extracted_text = agent_result
                .captured_stdout
                .as_ref()
                .map(|output| provider_for_extract.extract_text(output))
                .filter(|s| !s.is_empty());

            if let Err(e) = db.update_run_status(
                &run.id,
                status.clone(),
                exit_code,
                extracted_text.as_deref(),
            ) {
                tracing::warn!("Failed to update diagnostic run status: {}", e);
            }

            tracing::info!(
                "Diagnostic agent completed for ticket {}: exit_code={:?}, has_output={}",
                ticket_id_owned,
                exit_code,
                extracted_text.is_some()
            );

            if let Some(ref comment_text) = extracted_text {
                if !comment_text.trim().is_empty() {
                    tracing::info!(
                        "Posting diagnostic comment for ticket {} ({} chars)",
                        ticket_id_owned,
                        comment_text.len()
                    );

                    if let Err(e) = db.create_comment(&CreateComment {
                        ticket_id: ticket_id_owned.clone(),
                        author_type: AuthorType::System,
                        body_md: comment_text.clone(),
                        metadata: None,
                    }) {
                        tracing::error!(
                            "Failed to create diagnostic comment for ticket {}: {}",
                            ticket_id_owned,
                            e
                        );
                        return Err(DiagnosticError::SpawnFailed(format!(
                            "Failed to post diagnostic comment: {}",
                            e
                        )));
                    }

                    return Ok(());
                }
            }

            let error_msg = format!(
                "Diagnostic agent produced no usable output (exit_code={:?})",
                exit_code
            );
            tracing::warn!("{}", error_msg);
            Err(DiagnosticError::SpawnFailed(error_msg))
        }
        Ok(Err(spawn_error)) => {
            let error_msg = format!("Diagnostic agent spawn failed: {}", spawn_error);
            tracing::error!("{}", error_msg);

            if let Err(e) = db.update_run_status(&run.id, RunStatus::Error, None, Some(&error_msg))
            {
                tracing::warn!("Failed to update diagnostic run status: {}", e);
            }

            Err(DiagnosticError::SpawnFailed(error_msg))
        }
        Err(join_error) => {
            let error_msg = format!("Diagnostic agent task panicked: {}", join_error);
            tracing::error!("{}", error_msg);

            if let Err(e) = db.update_run_status(&run.id, RunStatus::Error, None, Some(&error_msg))
            {
                tracing::warn!("Failed to update diagnostic run status: {}", e);
            }

            Err(DiagnosticError::SpawnFailed(error_msg))
        }
    }
}
