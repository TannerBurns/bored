//! Tauri commands for spec conversation (brainstorming) operations

use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::broadcast;

use crate::agents::planner::{PlannerAgent, PlannerConfig};
use crate::agents::{AgentKind, ClaudeApiConfig};
use crate::api::state::LiveEvent;
use crate::commands::claude::ClaudeApiSettingsState;
use crate::commands::ApiConnState;
use crate::db::{
    ConversationMessage, ConversationRole, CreateConversationMessage, Database,
    SpecVersionStatus, StructuredSpec, UpdateSpec, UpdateSpecVersion,
};

/// Get all conversation messages for a spec
#[tauri::command]
pub async fn get_conversation_messages(
    spec_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<ConversationMessage>, String> {
    db.get_conversation_messages(&spec_id)
        .map_err(|e| e.to_string())
}

/// Send a user message in a conversation and trigger brainstorm agent response
#[tauri::command]
pub async fn send_conversation_message(
    spec_id: String,
    content: String,
    timeout_minutes: Option<u32>,
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
    api_conn: State<'_, ApiConnState>,
    claude_api_state: State<'_, ClaudeApiSettingsState>,
) -> Result<ConversationMessage, String> {
    tracing::info!("Sending conversation message for spec {}", spec_id);

    let mut spec = db.get_spec(&spec_id).map_err(|e| e.to_string())?;
    let mut version = db
        .get_latest_spec_version(&spec_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No version found for spec".to_string())?;

    // If the current version is not in conversing state, create a new version to continue the conversation
    if version.status != SpecVersionStatus::Conversing {
        tracing::info!(
            "Creating new version for spec {} to continue conversation (current version {} is in '{}' status)",
            spec_id,
            version.version_number,
            version.status.as_str()
        );
        
        // Create a new version for the continued conversation
        version = db.create_new_spec_version(&spec_id).map_err(|e| e.to_string())?;
        
        // Strip refined requirements appended by the previous version's brainstorm
        // so version 2+ generates fresh ones from the new conversation
        if let Some(sep_idx) = spec.user_input.find("\n\n---\n") {
            let original_input = spec.user_input[..sep_idx].to_string();
            db.update_spec(
                &spec_id,
                &UpdateSpec {
                    user_input: Some(original_input.clone()),
                    ..Default::default()
                },
            )
            .map_err(|e| e.to_string())?;
            spec.user_input = original_input;
        }
        
        // Emit event for the new version
        let _ = event_tx.send(LiveEvent::SpecUpdated {
            spec_id: spec_id.clone(),
        });
        
        // Add a system message indicating new version started
        let _ = db.create_conversation_message(&CreateConversationMessage {
            spec_id: spec_id.clone(),
            role: ConversationRole::System,
            content: format!("Starting new version {} based on continued conversation...", version.version_number),
        });
        
        let _ = event_tx.send(LiveEvent::ConversationMessageAdded {
            spec_id: spec_id.clone(),
            message_id: format!("new-version-{}", version.version_number),
            role: "system".to_string(),
            content: format!("Starting new version {} based on continued conversation...", version.version_number),
        });
    }

    let user_msg = db
        .create_conversation_message(&CreateConversationMessage {
            spec_id: spec_id.clone(),
            role: ConversationRole::User,
            content: content.clone(),
        })
        .map_err(|e| e.to_string())?;

    let _ = event_tx.send(LiveEvent::ConversationMessageAdded {
        spec_id: spec_id.clone(),
        message_id: user_msg.id.clone(),
        role: "user".to_string(),
        content: content.clone(),
    });

    let project = db
        .get_project(&spec.project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project '{}' not found", spec.project_id))?;

    let claude_api_config = Some(ClaudeApiConfig::from(claude_api_state.get()));

    let messages = db
        .get_conversation_messages(&spec_id)
        .map_err(|e| e.to_string())?;

    // Default to Claude for conversation agent
    let agent_kind = AgentKind::Claude;

    // Before brainstorm_config takes ownership of shared values
    let plan_trigger = PlanTriggerConfig {
        spec_id: spec_id.clone(),
        exploration_context: String::new(),
        repo_path: std::path::PathBuf::from(&project.path),
        api_url: api_conn.url.clone(),
        api_token: api_conn.token.clone(),
        claude_api_config: claude_api_config.clone(),
        agent_kind,
        model: spec.model.clone(),
    };

    let brainstorm_config = crate::agents::brainstorm::BrainstormConfig {
        spec_id: spec_id.clone(),
        user_input: spec.user_input.clone(),
        repo_path: std::path::PathBuf::from(&project.path),
        api_url: api_conn.url.clone(),
        api_token: api_conn.token.clone(),
        claude_api_config,
        agent_kind,
        model: spec.model.clone(),
        timeout_secs: timeout_minutes.map(|m| m as u64 * 60).unwrap_or(600),
    };

    let brainstorm_agent = crate::agents::brainstorm::BrainstormAgent::new(
        db.inner().clone(),
        brainstorm_config,
        event_tx.inner().clone(),
    );

    match brainstorm_agent.process_message(&messages).await {
        Ok(response) => {
            if response.is_complete {
                // Notify UI that spec generation (and plan generation) is starting
                // so the generating indicator shows for the direct completion case.
                let _ = event_tx.send(LiveEvent::BrainstormGeneratingSpec {
                    spec_id: spec_id.clone(),
                    version_number: version.version_number,
                });
                let trigger = response.structured_spec.as_ref()
                    .map(|s| trigger_from_spec(&plan_trigger, s));
                handle_spec_completion(
                    &db, &event_tx, &spec_id, &version.id, &spec.user_input,
                    response.structured_spec.as_ref(),
                    trigger,
                )?;
            } else if !response.has_questions {
                let ctx = AutoCompleteCtx {
                    spec_id: &spec_id, version_id: &version.id,
                    version_number: version.version_number,
                    user_input: &spec.user_input, plan_trigger: &plan_trigger,
                };
                request_auto_completion(&brainstorm_agent, &db, &event_tx, &ctx).await;
            }
        }
        Err(e) => {
            tracing::error!("Brainstorm agent error: {}", e);
            emit_conversation_error(&db, &event_tx, &spec_id, &format!("Error: {}", e));
            return Err(format!("Brainstorm agent error: {}", e));
        }
    }

    Ok(user_msg)
}

/// Start a conversation for a spec (version should already be in conversing state)
#[tauri::command]
pub async fn start_conversation(
    spec_id: String,
    timeout_minutes: Option<u32>,
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
    api_conn: State<'_, ApiConnState>,
    claude_api_state: State<'_, ClaudeApiSettingsState>,
) -> Result<ConversationMessage, String> {
    tracing::info!("Starting conversation for spec {}", spec_id);

    let spec = db.get_spec(&spec_id).map_err(|e| e.to_string())?;
    let version = db
        .get_latest_spec_version(&spec_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No version found for spec".to_string())?;

    if version.status != SpecVersionStatus::Conversing {
        return Err(format!(
            "Cannot start conversation: spec version is in '{}' status, expected 'conversing'",
            version.status.as_str()
        ));
    }

    let system_msg = db
        .create_conversation_message(&CreateConversationMessage {
            spec_id: spec_id.clone(),
            role: ConversationRole::System,
            content: "Starting brainstorming session...".to_string(),
        })
        .map_err(|e| e.to_string())?;

    let _ = event_tx.send(LiveEvent::ConversationMessageAdded {
        spec_id: spec_id.clone(),
        message_id: system_msg.id.clone(),
        role: "system".to_string(),
        content: system_msg.content.clone(),
    });

    let project = db
        .get_project(&spec.project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project '{}' not found", spec.project_id))?;

    let claude_api_config = Some(ClaudeApiConfig::from(claude_api_state.get()));

    let plan_trigger = PlanTriggerConfig {
        spec_id: spec_id.clone(),
        exploration_context: String::new(),
        repo_path: std::path::PathBuf::from(&project.path),
        api_url: api_conn.url.clone(),
        api_token: api_conn.token.clone(),
        claude_api_config: claude_api_config.clone(),
        agent_kind: AgentKind::Claude,
        model: spec.model.clone(),
    };

    let brainstorm_config = crate::agents::brainstorm::BrainstormConfig {
        spec_id: spec_id.clone(),
        user_input: spec.user_input.clone(),
        repo_path: std::path::PathBuf::from(&project.path),
        api_url: api_conn.url.clone(),
        api_token: api_conn.token.clone(),
        claude_api_config,
        agent_kind: AgentKind::Claude,
        model: spec.model.clone(),
        timeout_secs: timeout_minutes.map(|m| m as u64 * 60).unwrap_or(600),
    };

    let brainstorm_agent = crate::agents::brainstorm::BrainstormAgent::new(
        db.inner().clone(),
        brainstorm_config,
        event_tx.inner().clone(),
    );

    match brainstorm_agent.start_conversation().await {
        Ok(response) => {
            if response.is_complete {
                // Notify UI that spec generation (and plan generation) is starting
                // so the generating indicator shows for the direct completion case.
                let _ = event_tx.send(LiveEvent::BrainstormGeneratingSpec {
                    spec_id: spec_id.clone(),
                    version_number: version.version_number,
                });
                let trigger = response.structured_spec.as_ref()
                    .map(|s| trigger_from_spec(&plan_trigger, s));
                handle_spec_completion(
                    &db, &event_tx, &spec_id, &version.id, &spec.user_input,
                    response.structured_spec.as_ref(),
                    trigger,
                )?;
            } else if !response.has_questions {
                let ctx = AutoCompleteCtx {
                    spec_id: &spec_id, version_id: &version.id,
                    version_number: version.version_number,
                    user_input: &spec.user_input, plan_trigger: &plan_trigger,
                };
                request_auto_completion(&brainstorm_agent, &db, &event_tx, &ctx).await;
            }
        }
        Err(e) => {
            tracing::error!("Failed to start conversation: {}", e);
            emit_conversation_error(&db, &event_tx, &spec_id, &format!("Error starting conversation: {}", e));
            return Err(format!("Failed to start conversation: {}", e));
        }
    }

    let _ = event_tx.send(LiveEvent::SpecUpdated {
        spec_id: spec_id.clone(),
    });

    Ok(system_msg)
}

#[derive(Clone)]
struct PlanTriggerConfig {
    spec_id: String,
    exploration_context: String,
    repo_path: PathBuf,
    api_url: String,
    api_token: String,
    claude_api_config: Option<ClaudeApiConfig>,
    agent_kind: AgentKind,
    model: Option<String>,
}

const COMPLETION_PROMPT: &str = "Based on your observations and the conversation so far, you have enough information. \
    Please produce the final specification JSON block now:\n\
    ```json\n{\n  \"spec_complete\": true,\n  \"structured_spec\": {\n    \
    \"requirements\": \"...\",\n    \"decisions\": [...],\n    \
    \"constraints\": [...],\n    \"technical_notes\": \"...\"\n  }\n}\n```";

/// Create a system error message, emit it via SSE, and signal conversation complete.
fn emit_conversation_error(
    db: &Arc<Database>,
    event_tx: &broadcast::Sender<LiveEvent>,
    spec_id: &str,
    error_message: &str,
) {
    let error_msg = db.create_conversation_message(&CreateConversationMessage {
        spec_id: spec_id.to_string(),
        role: ConversationRole::System,
        content: error_message.to_string(),
    });
    if let Ok(msg) = &error_msg {
        let _ = event_tx.send(LiveEvent::ConversationMessageAdded {
            spec_id: spec_id.to_string(),
            message_id: msg.id.clone(),
            role: "system".to_string(),
            content: error_message.to_string(),
        });
    }
    let _ = event_tx.send(LiveEvent::ConversationComplete {
        spec_id: spec_id.to_string(),
        structured_spec: serde_json::Value::Null,
    });
}

fn trigger_from_spec(base: &PlanTriggerConfig, spec: &StructuredSpec) -> PlanTriggerConfig {
    let mut cfg = base.clone();
    cfg.exploration_context = spec.technical_notes.clone().unwrap_or_default();
    cfg
}

/// Context needed to drive auto-completion after a no-questions response.
struct AutoCompleteCtx<'a> {
    spec_id: &'a str,
    version_id: &'a str,
    version_number: i32,
    user_input: &'a str,
    plan_trigger: &'a PlanTriggerConfig,
}

/// When the agent returns only observations (no questions), automatically
/// request spec completion in a follow-up call.
async fn request_auto_completion(
    agent: &crate::agents::brainstorm::BrainstormAgent,
    db: &Arc<Database>,
    event_tx: &broadcast::Sender<LiveEvent>,
    ctx: &AutoCompleteCtx<'_>,
) {
    tracing::info!("Agent has no questions, requesting spec completion for {}", ctx.spec_id);

    let _ = event_tx.send(LiveEvent::BrainstormGeneratingSpec {
        spec_id: ctx.spec_id.to_string(),
        version_number: ctx.version_number,
    });

    let messages = match db.get_conversation_messages(ctx.spec_id) {
        Ok(m) => m,
        Err(e) => {
            emit_conversation_error(db, event_tx, ctx.spec_id, &format!("Failed to fetch messages: {}", e));
            return;
        }
    };

    let completion_messages: Vec<_> = messages.into_iter()
        .chain(std::iter::once(crate::db::ConversationMessage {
            id: "completion-request".to_string(),
            spec_id: ctx.spec_id.to_string(),
            role: ConversationRole::User,
            content: COMPLETION_PROMPT.to_string(),
            created_at: chrono::Utc::now(),
        }))
        .collect();

    match agent.process_message(&completion_messages).await {
        Ok(response) => {
            if response.is_complete {
                let trigger = response.structured_spec.as_ref()
                    .map(|s| trigger_from_spec(ctx.plan_trigger, s));
                let _ = handle_spec_completion(
                    db, event_tx, ctx.spec_id, ctx.version_id, ctx.user_input,
                    response.structured_spec.as_ref(),
                    trigger,
                );
            } else {
                tracing::warn!("Auto-completion response was not complete for spec {}", ctx.spec_id);
                let _ = event_tx.send(LiveEvent::ConversationComplete {
                    spec_id: ctx.spec_id.to_string(),
                    structured_spec: serde_json::Value::Null,
                });
            }
        }
        Err(e) => {
            tracing::error!("Failed to get spec completion: {}", e);
            emit_conversation_error(db, event_tx, ctx.spec_id, &format!("Failed to generate spec: {}", e));
        }
    }
}

/// Helper function to handle spec completion
fn handle_spec_completion(
    db: &Arc<Database>,
    event_tx: &broadcast::Sender<LiveEvent>,
    spec_id: &str,
    version_id: &str,
    original_user_input: &str,
    structured_spec: Option<&StructuredSpec>,
    plan_trigger: Option<PlanTriggerConfig>,
) -> Result<(), String> {
    if let Some(structured) = structured_spec {
        let enhanced_input = format!(
            "{}\n\n---\n## Refined Requirements\n{}\n\n## Key Decisions\n{}\n\n## Constraints\n{}{}",
            original_user_input,
            structured.requirements,
            structured.decisions.iter().map(|d| format!("- {}", d)).collect::<Vec<_>>().join("\n"),
            structured.constraints.iter().map(|c| format!("- {}", c)).collect::<Vec<_>>().join("\n"),
            structured.technical_notes.as_ref().map(|n| format!("\n\n## Technical Notes (from codebase exploration)\n{}", n)).unwrap_or_default()
        );

        let exploration_entry = crate::db::Exploration {
            query: "Codebase exploration during spec discovery".to_string(),
            response: structured.technical_notes.clone().unwrap_or_else(|| 
                "Exploration completed during conversational spec discovery.".to_string()
            ),
            timestamp: chrono::Utc::now(),
        };

        db.update_spec(
            spec_id,
            &UpdateSpec {
                user_input: Some(enhanced_input),
                ..Default::default()
            },
        )
        .map_err(|e| e.to_string())?;

        // Set status to Planning (skipping Exploring since exploration happened during conversation)
        db.update_spec_version(
            version_id,
            &UpdateSpecVersion {
                exploration_log: Some(vec![exploration_entry]),
                status: Some(SpecVersionStatus::Planning),
                ..Default::default()
            },
        )
        .map_err(|e| e.to_string())?;

        let _ = event_tx.send(LiveEvent::ConversationComplete {
            spec_id: spec_id.to_string(),
            structured_spec: serde_json::to_value(structured).unwrap_or_default(),
        });

        let _ = event_tx.send(LiveEvent::SpecUpdated {
            spec_id: spec_id.to_string(),
        });

        // Spawn plan generation in background
        if let Some(config) = plan_trigger {
            let db_clone = db.clone();
            let event_tx_clone = event_tx.clone();
            
            tokio::spawn(async move {
                run_plan_generation(db_clone, event_tx_clone, config).await;
            });
        }
    }
    
    Ok(())
}

/// Run plan generation in background after spec completion
async fn run_plan_generation(
    db: Arc<Database>,
    event_tx: broadcast::Sender<LiveEvent>,
    config: PlanTriggerConfig,
) {
    tracing::info!("Starting plan generation for spec {} after conversation complete", config.spec_id);
    
    let planner_config = PlannerConfig {
        spec_id: config.spec_id.clone(),
        max_explorations: 0,
        auto_approve: false,
        model: config.model,
        agent_kind: config.agent_kind,
        repo_path: config.repo_path,
        api_url: config.api_url,
        api_token: config.api_token,
        claude_api_config: config.claude_api_config,
        timeout_secs: 300,
        max_retries: 2,
    };
    
    let agent = PlannerAgent::with_events(db.clone(), planner_config, event_tx.clone());
    
    match agent.run_plan_only(&config.exploration_context).await {
        Ok(result) => {
            tracing::info!(
                "Plan generation completed for spec {}: status={:?}",
                config.spec_id,
                result.status
            );
            
            if let Ok(Some(version)) = db.get_latest_spec_version(&config.spec_id) {
                let _ = db.create_conversation_message(&CreateConversationMessage {
                    spec_id: config.spec_id.clone(),
                    role: ConversationRole::System,
                    content: format!("VERSION_CREATED:{}", version.version_number),
                });
                let _ = event_tx.send(LiveEvent::ConversationMessageAdded {
                    spec_id: config.spec_id.clone(),
                    message_id: format!("version-{}", version.version_number),
                    role: "system".to_string(),
                    content: format!("VERSION_CREATED:{}", version.version_number),
                });
            }
        }
        Err(e) => {
            tracing::error!("Plan generation failed for spec {}: {}", config.spec_id, e);
            
            let error_content = format!("Plan generation failed: {}", e);
            let error_msg = db.create_conversation_message(&CreateConversationMessage {
                spec_id: config.spec_id.clone(),
                role: ConversationRole::System,
                content: error_content.clone(),
            });
            if let Ok(msg) = &error_msg {
                let _ = event_tx.send(LiveEvent::ConversationMessageAdded {
                    spec_id: config.spec_id.clone(),
                    message_id: msg.id.clone(),
                    role: "system".to_string(),
                    content: error_content,
                });
            }
            let _ = event_tx.send(LiveEvent::SpecUpdated {
                spec_id: config.spec_id.clone(),
            });
        }
    }
}
