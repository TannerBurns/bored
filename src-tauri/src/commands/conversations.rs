//! Tauri commands for spec conversation (brainstorming) operations

use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::broadcast;

use crate::agents::planner::{PlannerAgent, PlannerConfig};
use crate::agents::AgentRegistry;
use crate::api::state::LiveEvent;
use crate::commands::agent_settings::AgentSettingsManager;
use crate::commands::ApiConnState;
use crate::db::{
    ConversationMessage, ConversationRole, CreateConversationMessage, Database,
    SpecVersionStatus, StructuredSpec, UpdateSpec, UpdateSpecVersion,
};

/// Input for starting or continuing a conversation.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationInput {
    pub spec_id: String,
    #[serde(default)]
    pub content: Option<String>,
    pub timeout_minutes: Option<u32>,
    pub agent_type: Option<String>,
}

fn resolve_agent_id(
    agent_type: Option<&str>,
    settings: Option<&serde_json::Map<String, serde_json::Value>>,
    registry: &AgentRegistry,
) -> String {
    if let Some(t) = agent_type {
        return t.to_string();
    }
    if let Some(settings) = settings {
        if let Some(serde_json::Value::String(s)) = settings.get("agentType") {
            return s.clone();
        }
    }
    registry.default_agent_id()
}

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
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn send_conversation_message(
    input: ConversationInput,
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
    _api_conn: State<'_, ApiConnState>,
    agent_settings: State<'_, AgentSettingsManager>,
    registry: State<'_, AgentRegistry>,
) -> Result<ConversationMessage, String> {
    let spec_id = input.spec_id;
    let content = input.content.unwrap_or_default();
    let timeout_minutes = input.timeout_minutes;
    let agent_type = input.agent_type;
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

    let messages = db
        .get_conversation_messages(&spec_id)
        .map_err(|e| e.to_string())?;

    let agent_id = resolve_agent_id(agent_type.as_deref(), spec.settings.as_object(), &registry);
    let provider = registry
        .get(&agent_id)
        .ok_or_else(|| format!("Unknown agent: {}", agent_id))?;

    let agent_config = agent_settings.agent_config_for(&agent_id);

    // Before brainstorm_config takes ownership of shared values
    let plan_trigger = PlanTriggerConfig {
        spec_id: spec_id.clone(),
        exploration_context: String::new(),
        repo_path: std::path::PathBuf::from(&project.path),
        agent_config: agent_config.clone(),
        agent_id: agent_id.clone(),
        provider: provider.clone(),
        model: spec.model.clone(),
    };

    let brainstorm_config = crate::agents::brainstorm::BrainstormConfig {
        spec_id: spec_id.clone(),
        user_input: spec.user_input.clone(),
        repo_path: std::path::PathBuf::from(&project.path),
        agent_config,
        agent_id,
        provider,
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
    input: ConversationInput,
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
    _api_conn: State<'_, ApiConnState>,
    agent_settings: State<'_, AgentSettingsManager>,
    registry: State<'_, AgentRegistry>,
) -> Result<ConversationMessage, String> {
    let spec_id = input.spec_id;
    let timeout_minutes = input.timeout_minutes;
    let agent_type = input.agent_type;
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

    let agent_id = resolve_agent_id(agent_type.as_deref(), spec.settings.as_object(), &registry);
    let provider = registry
        .get(&agent_id)
        .ok_or_else(|| format!("Unknown agent: {}", agent_id))?;

    let agent_config = agent_settings.agent_config_for(&agent_id);

    let plan_trigger = PlanTriggerConfig {
        spec_id: spec_id.clone(),
        exploration_context: String::new(),
        repo_path: std::path::PathBuf::from(&project.path),
        agent_config: agent_config.clone(),
        agent_id: agent_id.clone(),
        provider: provider.clone(),
        model: spec.model.clone(),
    };

    let brainstorm_config = crate::agents::brainstorm::BrainstormConfig {
        spec_id: spec_id.clone(),
        user_input: spec.user_input.clone(),
        repo_path: std::path::PathBuf::from(&project.path),
        agent_config,
        agent_id,
        provider,
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
    agent_config: std::collections::HashMap<String, serde_json::Value>,
    agent_id: String,
    provider: std::sync::Arc<dyn crate::agents::AgentProvider>,
    model: Option<String>,
}

const COMPLETION_PROMPT: &str = "Based on your observations and the conversation so far, you have enough information. \
    Please produce the final specification JSON block now. \
    The spec is the ONLY document implementing agents will see — capture EVERY detail from the conversation.\n\
    ```json\n{\n  \"spec_complete\": true,\n  \"observations\": \"<comprehensive final summary>\",\n  \"structured_spec\": {\n    \
    \"requirements\": [\"Requirement 1: <specific, self-contained requirement>\", \"Requirement 2: <another requirement>\"],\n    \
    \"decisions\": [\"Decision: WHAT — WHY — HOW it affects implementation\"],\n    \
    \"constraints\": [\"Constraint with context and codebase evidence\"],\n    \
    \"technical_notes\": [\"Create/Modify <path> — <details>\", \"Follow pattern in <path> — <what to replicate>\"]\n  }\n}\n```\n\
    IMPORTANT: requirements and technical_notes MUST be JSON arrays of strings, not single strings. \
    Each array item should be one concrete, actionable statement. Do NOT embed code fences inside array values.";

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
    cfg.exploration_context = spec.technical_notes.join("\n");
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

/// Format a slice of strings as a markdown bullet list (`- item\n- item`).
fn bullet_list(items: &[String]) -> String {
    items.iter().map(|s| format!("- {}", s)).collect::<Vec<_>>().join("\n")
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
        let observations_section = extract_latest_observations(db, spec_id);

        let enhanced_input = format!(
            "{}\n\n---\n## Refined Requirements\n{}\n\n## Key Decisions\n{}\n\n## Constraints\n{}{}{}",
            original_user_input,
            bullet_list(&structured.requirements),
            bullet_list(&structured.decisions),
            bullet_list(&structured.constraints),
            if structured.technical_notes.is_empty() {
                String::new()
            } else {
                format!(
                    "\n\n## Technical Notes (from codebase exploration)\n{}",
                    bullet_list(&structured.technical_notes)
                )
            },
            observations_section
        );

        let exploration_entry = crate::db::Exploration {
            query: "Codebase exploration during spec discovery".to_string(),
            response: if structured.technical_notes.is_empty() {
                "Exploration completed during conversational spec discovery.".to_string()
            } else {
                structured.technical_notes.join("\n")
            },
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

/// Extract the observations from the most recent assistant message in the conversation.
/// Returns a formatted section string to append to the enhanced input, or empty string if none found.
fn extract_latest_observations(db: &Arc<Database>, spec_id: &str) -> String {
    let messages = match db.get_conversation_messages(spec_id) {
        Ok(m) => m,
        Err(_) => return String::new(),
    };

    for msg in messages.iter().rev() {
        if msg.role == ConversationRole::Assistant {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&msg.content) {
                if let Some(obs) = parsed.get("observations").and_then(|v| v.as_str()) {
                    let trimmed = obs.trim();
                    if !trimmed.is_empty() {
                        return format!("\n\n## Codebase Observations (from discovery)\n{}", trimmed);
                    }
                }
            }
        }
    }

    String::new()
}

/// Build a conversation summary from the brainstorm Q&A to use as exploration context for the planner.
/// This preserves the full back-and-forth between the user and brainstorm agent so the planner
/// has access to all clarifications, decisions, and context discussed during discovery.
fn build_conversation_context(db: &Arc<Database>, spec_id: &str, technical_notes: &str) -> String {
    let messages = match db.get_conversation_messages(spec_id) {
        Ok(m) => m,
        Err(_) => return technical_notes.to_string(),
    };

    // Filter to meaningful conversation messages (skip system messages like "Starting brainstorming session...")
    let conversation_entries: Vec<String> = messages
        .iter()
        .filter(|msg| msg.role != ConversationRole::System)
        .map(|msg| {
            let role_label = match msg.role {
                ConversationRole::User => "User",
                ConversationRole::Assistant => "Assistant",
                ConversationRole::System => "System",
            };
            // For assistant messages, try to extract the readable observations/questions
            // instead of raw JSON
            if msg.role == ConversationRole::Assistant {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&msg.content) {
                    let mut parts = Vec::new();
                    if let Some(obs) = parsed.get("observations").and_then(|v| v.as_str()) {
                        if !obs.trim().is_empty() {
                            parts.push(format!("**Observations:** {}", obs.trim()));
                        }
                    }
                    if let Some(qs) = parsed.get("questions").and_then(|v| v.as_str()) {
                        if !qs.trim().is_empty() {
                            parts.push(format!("**Questions:** {}", qs.trim()));
                        }
                    }
                    if !parts.is_empty() {
                        return format!("**{}:**\n{}", role_label, parts.join("\n\n"));
                    }
                }
            }
            format!("**{}:** {}", role_label, msg.content)
        })
        .collect();

    if conversation_entries.is_empty() {
        return technical_notes.to_string();
    }

    let mut context = String::new();

    if !technical_notes.is_empty() {
        context.push_str("## Technical Notes from Codebase Exploration\n\n");
        context.push_str(technical_notes);
        context.push_str("\n\n");
    }

    context.push_str("## Discovery Conversation History\n\n");
    context.push_str("The following is the complete Q&A from the spec discovery session. ");
    context.push_str("Use the decisions, clarifications, and context discussed here to inform the work plan.\n\n");
    context.push_str(&conversation_entries.join("\n\n---\n\n"));

    context
}

/// Run plan generation in background after spec completion
async fn run_plan_generation(
    db: Arc<Database>,
    event_tx: broadcast::Sender<LiveEvent>,
    config: PlanTriggerConfig,
) {
    tracing::info!("Starting plan generation for spec {} after conversation complete", config.spec_id);

    let exploration_context = build_conversation_context(
        &db,
        &config.spec_id,
        &config.exploration_context,
    );
    
    let planner_config = PlannerConfig {
        spec_id: config.spec_id.clone(),
        max_explorations: 0,
        auto_approve: false,
        model: config.model,
        agent_id: config.agent_id,
        provider: config.provider,
        repo_path: config.repo_path,
        agent_config: config.agent_config,
        timeout_secs: 300,
        max_retries: 2,
    };
    
    let agent = PlannerAgent::with_events(db.clone(), planner_config, event_tx.clone());
    
    match agent.run_plan_only(&exploration_context).await {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{CreateConversationMessage, CreateProject, CreateSpec};
    use std::sync::Arc;

    fn create_test_db() -> Arc<Database> {
        Arc::new(Database::open_in_memory().unwrap())
    }

    fn setup_spec(db: &Database) -> String {
        let project = db
            .create_project(&CreateProject {
                name: "Test".to_string(),
                path: std::env::temp_dir().to_string_lossy().to_string(),
                requires_git: true,
            })
            .unwrap();
        let board = db.create_board("Board").unwrap();
        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: None,
                project_id: project.id,
                name: "Spec".to_string(),
                user_input: "Build something".to_string(),
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();
        spec.id
    }

    fn add_msg(db: &Database, spec_id: &str, role: ConversationRole, content: &str) {
        db.create_conversation_message(&CreateConversationMessage {
            spec_id: spec_id.to_string(),
            role,
            content: content.to_string(),
        })
        .unwrap();
    }

    // ====================================================================
    // extract_latest_observations
    // ====================================================================

    #[test]
    fn extract_observations_empty_conversation() {
        let db = create_test_db();
        let spec_id = setup_spec(&db);
        assert_eq!(extract_latest_observations(&db, &spec_id), "");
    }

    #[test]
    fn extract_observations_no_assistant_messages() {
        let db = create_test_db();
        let spec_id = setup_spec(&db);
        add_msg(&db, &spec_id, ConversationRole::User, "hello");
        add_msg(&db, &spec_id, ConversationRole::System, "started");
        assert_eq!(extract_latest_observations(&db, &spec_id), "");
    }

    #[test]
    fn extract_observations_assistant_non_json() {
        let db = create_test_db();
        let spec_id = setup_spec(&db);
        add_msg(&db, &spec_id, ConversationRole::Assistant, "plain text, not JSON");
        assert_eq!(extract_latest_observations(&db, &spec_id), "");
    }

    #[test]
    fn extract_observations_json_without_observations_key() {
        let db = create_test_db();
        let spec_id = setup_spec(&db);
        add_msg(
            &db,
            &spec_id,
            ConversationRole::Assistant,
            r#"{"questions": "What color?"}"#,
        );
        assert_eq!(extract_latest_observations(&db, &spec_id), "");
    }

    #[test]
    fn extract_observations_empty_observations_value() {
        let db = create_test_db();
        let spec_id = setup_spec(&db);
        add_msg(
            &db,
            &spec_id,
            ConversationRole::Assistant,
            r#"{"observations": "   "}"#,
        );
        assert_eq!(extract_latest_observations(&db, &spec_id), "");
    }

    #[test]
    fn extract_observations_valid() {
        let db = create_test_db();
        let spec_id = setup_spec(&db);
        add_msg(
            &db,
            &spec_id,
            ConversationRole::Assistant,
            r#"{"observations": "Found auth module in src/auth/"}"#,
        );
        let result = extract_latest_observations(&db, &spec_id);
        assert!(result.contains("## Codebase Observations (from discovery)"));
        assert!(result.contains("Found auth module in src/auth/"));
    }

    #[test]
    fn extract_observations_uses_most_recent_assistant() {
        let db = create_test_db();
        let spec_id = setup_spec(&db);
        add_msg(
            &db,
            &spec_id,
            ConversationRole::Assistant,
            r#"{"observations": "Old finding"}"#,
        );
        add_msg(&db, &spec_id, ConversationRole::User, "thanks");
        add_msg(
            &db,
            &spec_id,
            ConversationRole::Assistant,
            r#"{"observations": "Latest finding"}"#,
        );
        let result = extract_latest_observations(&db, &spec_id);
        assert!(result.contains("Latest finding"));
        assert!(!result.contains("Old finding"));
    }

    #[test]
    fn extract_observations_skips_non_json_assistant_to_find_earlier_json() {
        let db = create_test_db();
        let spec_id = setup_spec(&db);
        add_msg(
            &db,
            &spec_id,
            ConversationRole::Assistant,
            r#"{"observations": "Good finding"}"#,
        );
        // A later assistant message that isn't JSON
        add_msg(&db, &spec_id, ConversationRole::Assistant, "plain text");
        // extract walks backward: hits plain text first (skip), then JSON (match)
        let result = extract_latest_observations(&db, &spec_id);
        assert!(result.contains("Good finding"));
    }

    // ====================================================================
    // build_conversation_context
    // ====================================================================

    #[test]
    fn context_empty_conversation_returns_tech_notes() {
        let db = create_test_db();
        let spec_id = setup_spec(&db);
        let result = build_conversation_context(&db, &spec_id, "Some notes");
        assert_eq!(result, "Some notes");
    }

    #[test]
    fn context_only_system_messages_returns_tech_notes() {
        let db = create_test_db();
        let spec_id = setup_spec(&db);
        add_msg(&db, &spec_id, ConversationRole::System, "Starting session...");
        let result = build_conversation_context(&db, &spec_id, "Tech notes");
        assert_eq!(result, "Tech notes");
    }

    #[test]
    fn context_includes_user_messages() {
        let db = create_test_db();
        let spec_id = setup_spec(&db);
        add_msg(&db, &spec_id, ConversationRole::User, "Build a login page");
        let result = build_conversation_context(&db, &spec_id, "");
        assert!(result.contains("**User:** Build a login page"));
        assert!(result.contains("## Discovery Conversation History"));
    }

    #[test]
    fn context_prepends_tech_notes_when_present() {
        let db = create_test_db();
        let spec_id = setup_spec(&db);
        add_msg(&db, &spec_id, ConversationRole::User, "hello");
        let result = build_conversation_context(&db, &spec_id, "Existing patterns...");
        assert!(result.starts_with("## Technical Notes from Codebase Exploration"));
        assert!(result.contains("Existing patterns..."));
        assert!(result.contains("## Discovery Conversation History"));
    }

    #[test]
    fn context_omits_tech_notes_section_when_empty() {
        let db = create_test_db();
        let spec_id = setup_spec(&db);
        add_msg(&db, &spec_id, ConversationRole::User, "hello");
        let result = build_conversation_context(&db, &spec_id, "");
        assert!(!result.contains("## Technical Notes"));
        assert!(result.contains("## Discovery Conversation History"));
    }

    #[test]
    fn context_extracts_assistant_json_observations_and_questions() {
        let db = create_test_db();
        let spec_id = setup_spec(&db);
        add_msg(
            &db,
            &spec_id,
            ConversationRole::Assistant,
            r#"{"observations": "Found Zustand stores", "questions": "1. Which auth provider?"}"#,
        );
        let result = build_conversation_context(&db, &spec_id, "");
        assert!(result.contains("**Observations:** Found Zustand stores"));
        assert!(result.contains("**Questions:** 1. Which auth provider?"));
    }

    #[test]
    fn context_falls_back_to_raw_content_for_non_json_assistant() {
        let db = create_test_db();
        let spec_id = setup_spec(&db);
        add_msg(&db, &spec_id, ConversationRole::Assistant, "I explored the codebase.");
        let result = build_conversation_context(&db, &spec_id, "");
        assert!(result.contains("**Assistant:** I explored the codebase."));
    }

    #[test]
    fn context_filters_out_system_messages() {
        let db = create_test_db();
        let spec_id = setup_spec(&db);
        add_msg(&db, &spec_id, ConversationRole::System, "Session started");
        add_msg(&db, &spec_id, ConversationRole::User, "Build auth");
        add_msg(&db, &spec_id, ConversationRole::Assistant, "OK, looking...");
        let result = build_conversation_context(&db, &spec_id, "");
        assert!(!result.contains("Session started"));
        assert!(result.contains("Build auth"));
        assert!(result.contains("OK, looking..."));
    }

    mod resolve_agent_id_tests {
        use super::*;
        use crate::agents::cost::RunCostData;
        use crate::agents::provider::{AgentProvider, AgentRunConfig};
        use crate::agents::registry::AgentRegistry;

        #[derive(Debug)]
        struct FakeProvider {
            name: String,
            available: bool,
        }

        impl AgentProvider for FakeProvider {
            fn id(&self) -> &str { &self.name }
            fn display_name(&self) -> &str { &self.name }
            fn build_command(&self, _: &AgentRunConfig) -> (String, Vec<String>) { (self.name.clone(), vec![]) }
            fn build_env_vars(&self, _: &AgentRunConfig) -> Vec<(String, String)> { vec![] }
            fn extract_text(&self, o: &str) -> String { o.to_string() }
            fn extract_cost(&self, _: &str, _: &str, _: f64) -> Option<RunCostData> { None }
            fn is_available(&self) -> bool { self.available }
            fn get_version(&self) -> Option<String> { None }
            fn config_dir_name(&self) -> &str { ".fake" }
            fn command_instructions_subdir(&self) -> &str { "commands" }
            fn format_command_reference(&self, c: &str) -> String { format!("/{}", c) }
        }

        fn make_registry(providers: Vec<(&str, bool)>) -> AgentRegistry {
            let mut reg = AgentRegistry::new();
            for (name, available) in providers {
                reg.register(std::sync::Arc::new(FakeProvider {
                    name: name.to_string(),
                    available,
                }));
            }
            reg
        }

        #[test]
        fn explicit_agent_type_takes_priority() {
            let reg = make_registry(vec![("cursor", true), ("claude", true)]);
            let result = resolve_agent_id(Some("cursor"), None, &reg);
            assert_eq!(result, "cursor");
        }

        #[test]
        fn explicit_agent_type_overrides_settings() {
            let reg = make_registry(vec![("cursor", true), ("claude", true)]);
            let mut settings = serde_json::Map::new();
            settings.insert("agentType".to_string(), serde_json::json!("claude"));
            let result = resolve_agent_id(Some("cursor"), Some(&settings), &reg);
            assert_eq!(result, "cursor");
        }

        #[test]
        fn settings_agent_type_used_when_no_explicit() {
            let reg = make_registry(vec![("cursor", true), ("claude", true)]);
            let mut settings = serde_json::Map::new();
            settings.insert("agentType".to_string(), serde_json::json!("claude"));
            let result = resolve_agent_id(None, Some(&settings), &reg);
            assert_eq!(result, "claude");
        }

        #[test]
        fn falls_back_to_first_available_agent() {
            let reg = make_registry(vec![("offline", false), ("online", true)]);
            let result = resolve_agent_id(None, None, &reg);
            assert_eq!(result, "online");
        }

        #[test]
        fn falls_back_to_first_registered_when_none_available() {
            let reg = make_registry(vec![("offline1", false), ("offline2", false)]);
            let result = resolve_agent_id(None, None, &reg);
            // Falls back to first registered agent when none are available
            assert!(!result.is_empty());
        }

        #[test]
        fn falls_back_to_empty_when_registry_empty() {
            let reg = AgentRegistry::new();
            let result = resolve_agent_id(None, None, &reg);
            assert_eq!(result, "");
        }

        #[test]
        fn unknown_explicit_agent_is_passed_through() {
            let reg = make_registry(vec![("cursor", true)]);
            let result = resolve_agent_id(Some("unknown-agent"), None, &reg);
            assert_eq!(result, "unknown-agent");
        }

        #[test]
        fn settings_with_non_string_agent_type_ignored() {
            let reg = make_registry(vec![("cursor", true)]);
            let mut settings = serde_json::Map::new();
            settings.insert("agentType".to_string(), serde_json::json!(42));
            let result = resolve_agent_id(None, Some(&settings), &reg);
            assert_eq!(result, "cursor");
        }
    }
}
