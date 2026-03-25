use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::broadcast;

use crate::api::state::LiveEvent;
use crate::db::models::ChatMode;
use crate::db::{Database, UpdateSpec};

use super::super::registry::AgentRegistry;
use super::super::spawner;
use super::super::AgentRunConfig;

/// Clean up the raw title: strip surrounding quotes, collapse whitespace, cap length.
fn sanitize_title(raw: &str) -> String {
    let trimmed = raw
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim_start_matches("Title:")
        .trim_start_matches("title:")
        .trim();

    let single_line: String = trimmed
        .lines()
        .next()
        .unwrap_or(trimmed)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if single_line.len() > 80 {
        let boundary = single_line
            .char_indices()
            .take_while(|(i, _)| *i < 77)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(77);
        format!("{}...", &single_line[..boundary])
    } else {
        single_line
    }
}

pub struct TitleGenParams {
    pub db: Arc<Database>,
    pub chat_id: String,
    pub first_message: String,
    pub event_tx: broadcast::Sender<LiveEvent>,
    pub registry: Arc<AgentRegistry>,
    pub agent_id: String,
    pub repo_path: std::path::PathBuf,
    pub agent_config: HashMap<String, serde_json::Value>,
    pub model: Option<String>,
    pub workspace_file: Option<std::path::PathBuf>,
    pub workspace_paths: Vec<std::path::PathBuf>,
}

/// Spawn a background task that generates a title for the chat from the first message.
///
/// Runs concurrently so it doesn't block the main message processing path.
/// Uses a lightweight agent config (no thinking, max 1 turn) to keep it fast.
pub fn spawn_title_generation(params: TitleGenParams) {
    let TitleGenParams {
        db, chat_id, first_message, event_tx, registry,
        agent_id, repo_path, agent_config, model,
        workspace_file, workspace_paths,
    } = params;
    tokio::spawn(async move {
        let truncated_input: String = first_message.chars().take(500).collect();
        let prompt = format!(
            "Generate a concise title (5 words or fewer) for a conversation that starts with \
             this message. Return ONLY the title text, nothing else. \
             Do not use any tools.\n\n{}",
            truncated_input
        );

        let provider = match registry.get(&agent_id) {
            Some(p) => p,
            None => {
                tracing::warn!("Title generation: agent '{}' not found", agent_id);
                return;
            }
        };

        let title_config = provider.lightweight_agent_config(&agent_config);

        let run_config = AgentRunConfig {
            agent_id: agent_id.clone(),
            ticket_id: chat_id.clone(),
            run_id: format!("chat-title-{}", uuid::Uuid::new_v4()),
            repo_path,
            prompt,
            timeout_secs: Some(120),
            model,
            agent_config: title_config,
            session_id: None,
            workspace_file,
            workspace_paths,
        };

        let provider_clone = provider.clone();
        let result = tokio::task::spawn_blocking(move || {
            spawner::run_agent_via_provider(&*provider_clone, &run_config, None)
        })
        .await;

        match result {
            Ok(Ok(run_result)) => {
                let stdout = run_result.captured_stdout.as_deref().unwrap_or("");
                let raw_title = provider.extract_text(stdout);
                let title = sanitize_title(&raw_title);

                if title.is_empty() {
                    tracing::warn!(
                        "Title generation returned empty text for chat {} (status={:?}, stdout_len={})",
                        chat_id,
                        run_result.status,
                        stdout.len(),
                    );
                    return;
                }

                tracing::info!("Generated title for chat {}: {:?}", chat_id, title);

                if let Err(e) = db.update_chat_title(&chat_id, &title) {
                    tracing::warn!("Failed to save chat title: {}", e);
                    return;
                }

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
            Ok(Err(e)) => {
                tracing::warn!("Title generation agent failed for chat {}: {}", chat_id, e);
            }
            Err(e) => {
                tracing::warn!("Title generation task join error for chat {}: {}", chat_id, e);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_quotes() {
        assert_eq!(sanitize_title("\"My Title\""), "My Title");
        assert_eq!(sanitize_title("'My Title'"), "My Title");
    }

    #[test]
    fn sanitize_strips_prefix() {
        assert_eq!(sanitize_title("Title: My Title"), "My Title");
        assert_eq!(sanitize_title("title: My Title"), "My Title");
    }

    #[test]
    fn sanitize_takes_first_line() {
        assert_eq!(sanitize_title("First\nSecond\nThird"), "First");
    }

    #[test]
    fn sanitize_collapses_whitespace() {
        assert_eq!(sanitize_title("  Too   Many   Spaces  "), "Too Many Spaces");
    }

    #[test]
    fn sanitize_truncates_long_titles() {
        let long = "a ".repeat(50);
        let result = sanitize_title(&long);
        assert!(result.len() <= 83); // 80 + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn sanitize_empty_returns_empty() {
        assert_eq!(sanitize_title(""), "");
        assert_eq!(sanitize_title("  "), "");
    }
}
