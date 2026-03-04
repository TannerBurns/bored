use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::broadcast;

use crate::api::state::LiveEvent;
use crate::db::models::ChatMode;
use crate::db::{Database, UpdateSpec};

use super::super::registry::AgentRegistry;
use super::super::spawner;
use super::super::AgentRunConfig;

/// Spawn a background task that generates a title for the chat from the first message.
///
/// Runs concurrently so it doesn't block the main message processing path.
pub fn spawn_title_generation(
    db: Arc<Database>,
    chat_id: String,
    first_message: String,
    event_tx: broadcast::Sender<LiveEvent>,
    registry: Arc<AgentRegistry>,
    agent_id: String,
    repo_path: std::path::PathBuf,
    agent_config: HashMap<String, serde_json::Value>,
    model: Option<String>,
) {
    tokio::spawn(async move {
        let prompt = format!(
            "Generate a concise title (5 words or fewer) for a conversation that starts with \
             this message. Return ONLY the title text, nothing else.\n\n{}",
            first_message
        );

        let provider = match registry.get(&agent_id) {
            Some(p) => p,
            None => {
                tracing::warn!("Title generation: agent '{}' not found", agent_id);
                return;
            }
        };

        let run_config = AgentRunConfig {
            agent_id: agent_id.clone(),
            ticket_id: chat_id.clone(),
            run_id: format!("chat-title-{}", uuid::Uuid::new_v4()),
            repo_path,
            prompt,
            timeout_secs: Some(60),
            model,
            agent_config,
            session_id: None,
        };

        let provider_clone = provider.clone();
        let result = tokio::task::spawn_blocking(move || {
            spawner::run_agent_via_provider(&*provider_clone, &run_config, None)
        })
        .await;

        match result {
            Ok(Ok(run_result)) => {
                let stdout = run_result.captured_stdout.as_deref().unwrap_or("");
                let title = provider.extract_text(stdout).trim().to_string();
                if !title.is_empty() {
                    if let Err(e) = db.update_chat_title(&chat_id, &title) {
                        tracing::warn!("Failed to save chat title: {}", e);
                        return;
                    }

                    // For spec_builder chats, sync the title to the spec name
                    if let Ok(chat) = db.get_chat(&chat_id) {
                        if chat.mode == ChatMode::SpecBuilder {
                            if let Some(ref spec_id) = chat.spec_id {
                                let _ = db.update_spec(
                                    spec_id,
                                    &UpdateSpec {
                                        name: Some(title.clone()),
                                        ..Default::default()
                                    },
                                );
                                let _ = event_tx.send(LiveEvent::SpecUpdated {
                                    spec_id: spec_id.clone(),
                                });
                            }
                        }
                    }

                    let _ = event_tx.send(LiveEvent::ChatTitleGenerated {
                        chat_id,
                        title,
                    });
                }
            }
            Ok(Err(e)) => {
                tracing::warn!("Title generation agent failed: {}", e);
            }
            Err(e) => {
                tracing::warn!("Title generation task join error: {}", e);
            }
        }
    });
}
