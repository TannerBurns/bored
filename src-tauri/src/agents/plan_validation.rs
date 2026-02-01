//! Plan clarification detection for agent workflows.
//!
//! Analyzes implementation plans to detect when user input is needed
//! before proceeding with implementation.

use std::path::PathBuf;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

use crate::db::{Database, AgentType, CreateRun, RunStatus};
use super::{AgentKind, AgentRunConfig, ClaudeApiConfig, extract_text_from_stream_json, extract_agent_text};
use super::spawner;

/// Result of plan clarification validation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanValidationResult {
    /// Whether the plan requires user clarification before implementation
    pub needs_clarification: bool,
    /// Brief explanation of why clarification is needed (or not)
    pub reason: String,
}

impl Default for PlanValidationResult {
    fn default() -> Self {
        Self {
            needs_clarification: false,
            reason: "Plan appears ready for implementation".to_string(),
        }
    }
}

/// Configuration for plan validation operations
#[derive(Clone)]
pub struct PlanValidationConfig {
    pub db: Arc<Database>,
    pub parent_run_id: String,
    pub ticket_id: String,
    pub repo_path: PathBuf,
    pub api_url: String,
    pub api_token: String,
    pub model: Option<String>,
    /// The agent type to use for validation (Claude or Cursor)
    pub agent_kind: AgentKind,
    /// Claude API configuration (auth token, api key, base url, model override)
    pub claude_api_config: Option<ClaudeApiConfig>,
}

/// Error type for plan validation operations
#[derive(Debug, thiserror::Error)]
pub enum PlanValidationError {
    #[error("Failed to create validation run: {0}")]
    RunCreationFailed(String),
    
    #[error("Failed to spawn validation agent: {0}")]
    SpawnFailed(String),
    
    #[error("Failed to parse validation response: {0}")]
    ParseFailed(String),
    
    #[error("Database error: {0}")]
    DatabaseError(#[from] crate::db::DbError),
}

/// Build the prompt for the validation agent to analyze a plan
pub fn build_plan_validation_prompt(plan: &str) -> String {
    format!(
        r#"Analyze this plan and determine if it requires user clarification before implementation.

## Plan
{plan}

## Decision Criteria
A plan needs clarification if it:
- Asks questions to the user
- Presents multiple options and asks which to choose
- States it cannot proceed without more information
- Expresses uncertainty about core requirements

A plan does NOT need clarification if it:
- Has a clear implementation path
- Makes reasonable assumptions (even if noted)
- Has complete, actionable steps

## Response Format
Respond with ONLY a JSON object:
{{"needs_clarification": true/false, "reason": "brief explanation"}}
"#
    )
}

/// Build the prompt for generating a user-facing clarification message
pub fn build_clarification_message_prompt(plan: &str) -> String {
    format!(
        r#"Based on this implementation plan, craft a clear message asking the user for the specific information needed to proceed.

## Plan
{plan}

## Instructions
- Extract the specific questions or ambiguities from the plan
- Write a concise, friendly message to the user
- Clearly state what information is needed
- If there are options, list them clearly
- Do NOT include implementation details - focus only on what the user needs to answer

Write ONLY the clarification message, no preamble.
"#
    )
}

/// Parse the validation agent's response to extract the structured result
pub fn parse_validation_response(output: &str) -> Result<PlanValidationResult, PlanValidationError> {
    let text_content = extract_text_from_stream_json(output)
        .unwrap_or_else(|| output.to_string());
    let trimmed = text_content.trim();
    
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            let json_str = &trimmed[start..=end];
            
            if let Ok(result) = serde_json::from_str::<PlanValidationResult>(json_str) {
                return Ok(result);
            }
            
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) {
                let needs_clarification = value.get("needs_clarification")
                    .or_else(|| value.get("needsClarification"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                
                let reason = value.get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("No reason provided")
                    .to_string();
                
                return Ok(PlanValidationResult {
                    needs_clarification,
                    reason,
                });
            }
        }
    }
    
    Err(PlanValidationError::ParseFailed(format!(
        "Could not parse validation response: {}",
        if trimmed.len() > 200 { &trimmed[..200] } else { trimmed }
    )))
}

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
        AgentKind::Cursor => AgentType::Cursor,
        AgentKind::Claude => AgentType::Claude,
    };
    
    let run = config.db.create_run(&CreateRun {
        ticket_id: config.ticket_id.clone(),
        agent_type,
        repo_path: config.repo_path.to_string_lossy().to_string(),
        parent_run_id: Some(config.parent_run_id.clone()),
        stage: Some("plan-validation".to_string()),
    }).map_err(|e| PlanValidationError::RunCreationFailed(e.to_string()))?;
    
    if let Err(e) = config.db.update_run_status(&run.id, RunStatus::Running, None, None) {
        tracing::warn!("Failed to update validation run status: {}", e);
    }
    
    let prompt = build_plan_validation_prompt(plan);
    
    let agent_config = AgentRunConfig {
        kind: config.agent_kind,
        ticket_id: config.ticket_id.clone(),
        run_id: run_id.clone(),
        repo_path: config.repo_path.clone(),
        prompt,
        timeout_secs: Some(120),
        api_url: config.api_url.clone(),
        api_token: config.api_token.clone(),
        model: config.model.clone(),
        claude_api_config: config.claude_api_config.clone(),
    };
    
    let db = config.db.clone();
    
    let result = tokio::task::spawn_blocking(move || {
        spawner::run_agent(agent_config, None)
    }).await;
    
    match result {
        Ok(Ok(agent_result)) => {
            let exit_code = agent_result.exit_code;
            let status = if exit_code == Some(0) { RunStatus::Finished } else { RunStatus::Error };
            
            let validation_result = agent_result.captured_stdout
                .as_ref()
                .and_then(|output| parse_validation_response(output).ok());
            
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
            let _ = db.update_run_status(&run.id, RunStatus::Error, None, Some(&spawn_error.to_string()));
            Ok(PlanValidationResult::default())
        }
        Err(join_error) => {
            tracing::error!("Validation agent task panicked: {}", join_error);
            let _ = db.update_run_status(&run.id, RunStatus::Error, None, Some(&join_error.to_string()));
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
        AgentKind::Cursor => AgentType::Cursor,
        AgentKind::Claude => AgentType::Claude,
    };
    
    let run = config.db.create_run(&CreateRun {
        ticket_id: config.ticket_id.clone(),
        agent_type,
        repo_path: config.repo_path.to_string_lossy().to_string(),
        parent_run_id: Some(config.parent_run_id.clone()),
        stage: Some("clarification-gen".to_string()),
    }).map_err(|e| PlanValidationError::RunCreationFailed(e.to_string()))?;
    
    if let Err(e) = config.db.update_run_status(&run.id, RunStatus::Running, None, None) {
        tracing::warn!("Failed to update clarification run status: {}", e);
    }
    
    let prompt = build_clarification_message_prompt(plan);
    
    let agent_config = AgentRunConfig {
        kind: config.agent_kind,
        ticket_id: config.ticket_id.clone(),
        run_id: run_id.clone(),
        repo_path: config.repo_path.clone(),
        prompt,
        timeout_secs: Some(120),
        api_url: config.api_url.clone(),
        api_token: config.api_token.clone(),
        model: config.model.clone(),
        claude_api_config: config.claude_api_config.clone(),
    };
    
    let db = config.db.clone();
    
    let result = tokio::task::spawn_blocking(move || {
        spawner::run_agent(agent_config, None)
    }).await;
    
    match result {
        Ok(Ok(agent_result)) => {
            let exit_code = agent_result.exit_code;
            let status = if exit_code == Some(0) { RunStatus::Finished } else { RunStatus::Error };
            
            let message = agent_result.captured_stdout
                .as_ref()
                .map(|output| extract_agent_text(output))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            
            let _ = db.update_run_status(&run.id, status, exit_code, message.as_deref());
            
            message.ok_or_else(|| PlanValidationError::SpawnFailed(
                "Clarification agent produced no output".to_string()
            ))
        }
        Ok(Err(spawn_error)) => {
            tracing::error!("Clarification agent spawn failed: {}", spawn_error);
            let _ = db.update_run_status(&run.id, RunStatus::Error, None, Some(&spawn_error.to_string()));
            Err(PlanValidationError::SpawnFailed(spawn_error.to_string()))
        }
        Err(join_error) => {
            tracing::error!("Clarification agent task panicked: {}", join_error);
            let _ = db.update_run_status(&run.id, RunStatus::Error, None, Some(&join_error.to_string()));
            Err(PlanValidationError::SpawnFailed(join_error.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_validation_response_valid_json_needs_clarification() {
        let response = r#"{"needs_clarification": true, "reason": "Plan asks which framework to use"}"#;
        let result = parse_validation_response(response).unwrap();
        assert!(result.needs_clarification);
        assert_eq!(result.reason, "Plan asks which framework to use");
    }
    
    #[test]
    fn parse_validation_response_valid_json_no_clarification() {
        let response = r#"{"needs_clarification": false, "reason": "Plan has clear implementation path"}"#;
        let result = parse_validation_response(response).unwrap();
        assert!(!result.needs_clarification);
        assert_eq!(result.reason, "Plan has clear implementation path");
    }
    
    #[test]
    fn parse_validation_response_with_preamble() {
        let response = r#"Here is my analysis:
{"needs_clarification": true, "reason": "Multiple options presented"}
That's my assessment."#;
        let result = parse_validation_response(response).unwrap();
        assert!(result.needs_clarification);
        assert_eq!(result.reason, "Multiple options presented");
    }
    
    #[test]
    fn parse_validation_response_camel_case() {
        let response = r#"{"needsClarification": true, "reason": "Question asked"}"#;
        let result = parse_validation_response(response).unwrap();
        assert!(result.needs_clarification);
    }
    
    #[test]
    fn parse_validation_response_no_whitespace() {
        let response = r#"{"needs_clarification":true,"reason":"Compact JSON"}"#;
        let result = parse_validation_response(response).unwrap();
        assert!(result.needs_clarification);
    }
    
    #[test]
    fn parse_validation_response_extra_fields() {
        let response = r#"{"needs_clarification": false, "reason": "Ready", "confidence": 0.95}"#;
        let result = parse_validation_response(response).unwrap();
        assert!(!result.needs_clarification);
        assert_eq!(result.reason, "Ready");
    }
    
    #[test]
    fn parse_validation_response_missing_reason() {
        let response = r#"{"needs_clarification": true}"#;
        let result = parse_validation_response(response).unwrap();
        assert!(result.needs_clarification);
        assert_eq!(result.reason, "No reason provided");
    }
    
    #[test]
    fn parse_validation_response_invalid_json_fails() {
        let response = "This is not valid JSON at all";
        let result = parse_validation_response(response);
        assert!(result.is_err());
    }
    
    #[test]
    fn plan_validation_result_default() {
        let result = PlanValidationResult::default();
        assert!(!result.needs_clarification);
        assert!(!result.reason.is_empty());
    }
    
    #[test]
    fn plan_validation_result_serializes() {
        let result = PlanValidationResult {
            needs_clarification: true,
            reason: "Test reason".to_string(),
        };
        
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"needsClarification\":true"));
        assert!(json.contains("\"reason\":\"Test reason\""));
    }
    
    #[test]
    fn build_plan_validation_prompt_contains_plan() {
        let plan = "1. Do step A\n2. Do step B";
        let prompt = build_plan_validation_prompt(plan);
        
        assert!(prompt.contains("1. Do step A"));
        assert!(prompt.contains("2. Do step B"));
        assert!(prompt.contains("Decision Criteria"));
        assert!(prompt.contains("needs_clarification"));
    }
    
    #[test]
    fn build_clarification_message_prompt_contains_plan() {
        let plan = "Should we use React or Vue?";
        let prompt = build_clarification_message_prompt(plan);
        
        assert!(prompt.contains("Should we use React or Vue?"));
        assert!(prompt.contains("clarification message"));
    }
    
    #[test]
    fn plan_validation_error_display() {
        let error = PlanValidationError::RunCreationFailed("db error".to_string());
        assert!(error.to_string().contains("Failed to create validation run"));
        assert!(error.to_string().contains("db error"));
        
        let error = PlanValidationError::SpawnFailed("spawn error".to_string());
        assert!(error.to_string().contains("Failed to spawn validation agent"));
        
        let error = PlanValidationError::ParseFailed("bad json".to_string());
        assert!(error.to_string().contains("Failed to parse validation response"));
    }
    
    #[test]
    fn plan_validation_config_is_clone() {
        fn assert_clone<T: Clone>() {}
        assert_clone::<PlanValidationConfig>();
    }
}
