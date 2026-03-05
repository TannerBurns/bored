//! Spec Builder mode for the chat agent.
//!
//! Reuses spec discovery prompt-building and response-parsing utilities while running
//! the agent through ChatAgent's own execution pipeline (run_agent), so that
//! ChatLogEntry events fire and messages are stored in chat_messages.
//!
//! Plan generation also runs in-session via run_agent, so the agent retains
//! full conversation context (via session ID) and the user sees progress in
//! the chat timeline.

use std::sync::Arc;

use crate::agents::spec_discovery::{
    build_conversation_prompt, build_initial_prompt, bullet_list, parse_response,
    COMPLETION_PROMPT,
};
use crate::agents::planner::{generate_plan_markdown, generate_planning_prompt, parse_project_plan};
use crate::api::state::LiveEvent;
use crate::db::models::{ChatMessage, ChatMessageRole};
use crate::db::{
    ConversationMessage, ConversationRole, CreateSpec, Database, Exploration,
    SpecVersionStatus, StructuredSpec, UpdateSpec, UpdateSpecVersion,
};

use super::config::ChatAgentError;
use super::ChatAgent;

impl ChatAgent {
    pub(crate) async fn run_spec_builder(
        &self,
        messages: Vec<ChatMessage>,
    ) -> Result<ChatMessage, ChatAgentError> {
        let chat = self.db.get_chat(&self.config.chat_id)?;

        let spec_id = self.ensure_spec_for_chat(&chat, &messages).await?;

        let mut spec = self
            .db
            .get_spec(&spec_id)
            .map_err(|e| ChatAgentError::AgentFailed(format!("Failed to load spec: {}", e)))?;

        let mut version = self
            .db
            .get_latest_spec_version(&spec_id)
            .map_err(|e| {
                ChatAgentError::AgentFailed(format!("Failed to load spec version: {}", e))
            })?
            .ok_or(ChatAgentError::MissingField("spec_version"))?;

        // If the latest version is no longer in conversing state (e.g. already
        // planned/completed), start a fresh version so the user can iterate.
        if version.status != SpecVersionStatus::Conversing {
            tracing::info!(
                "Creating new version for spec {} (current v{} is '{}')",
                spec_id,
                version.version_number,
                version.status.as_str(),
            );

            version = self
                .db
                .create_new_spec_version(&spec_id)
                .map_err(|e| {
                    ChatAgentError::AgentFailed(format!("Failed to create new version: {}", e))
                })?;

            // Strip refined requirements appended by the previous spec discovery
            if let Some(sep_idx) = spec.user_input.find("\n\n---\n") {
                let original = spec.user_input[..sep_idx].to_string();
                let _ = self.db.update_spec(
                    &spec_id,
                    &UpdateSpec {
                        user_input: Some(original.clone()),
                        ..Default::default()
                    },
                );
                spec.user_input = original;
            }

            let _ = self.event_tx.send(LiveEvent::SpecUpdated {
                spec_id: spec_id.clone(),
            });

            self.send_system_message(&format!(
                "Starting new version {} based on continued conversation...",
                version.version_number,
            ))
            .await;
        }

        let has_assistant_response = messages.iter().any(|m| m.role == ChatMessageRole::Assistant);

        let prompt = if !has_assistant_response {
            build_initial_prompt(&spec.user_input)
        } else {
            let conv_messages: Vec<_> = Self::convert_to_conv_messages(&messages, &spec_id)
                .into_iter()
                .skip(1) // skip the first user message; it's already in spec.user_input
                .collect();
            build_conversation_prompt(&spec.user_input, &conv_messages)
        };

        let (text, stdout, ts_lines) = self.run_agent(&prompt).await?;

        let parsed = parse_response(&text)
            .map_err(|e| ChatAgentError::AgentFailed(format!("Failed to parse response: {}", e)))?;

        let metadata = if parsed.is_complete {
            Some(serde_json::json!({ "spec_complete": true }))
        } else {
            None
        };
        let assistant_msg = self
            .save_assistant_message(&parsed.message, metadata.as_ref())
            .await?;
        self.persist_log_events(&ts_lines, &assistant_msg.id);

        self.extract_and_store_cost(&stdout, Some(&assistant_msg.id))
            .await?;

        if parsed.is_complete {
            self.handle_spec_discovery_completion(
                &spec_id,
                &version.id,
                &spec.user_input,
                parsed.structured_spec.as_ref(),
            )
            .await?;
        } else if !parsed.has_questions {
            self.handle_spec_discovery_auto_completion(
                &spec_id,
                &version.id,
                version.version_number,
                &spec.user_input,
            )
            .await?;
        }

        Ok(assistant_msg)
    }

    /// If the chat has no spec_id, creates a Spec + SpecVersion from the first
    /// user message and links it to the chat.
    async fn ensure_spec_for_chat(
        &self,
        chat: &crate::db::models::Chat,
        messages: &[ChatMessage],
    ) -> Result<String, ChatAgentError> {
        if let Some(ref spec_id) = chat.spec_id {
            return Ok(spec_id.clone());
        }

        let user_input = messages
            .iter()
            .find(|m| m.role == ChatMessageRole::User)
            .map(|m| m.content.clone())
            .unwrap_or_default();

        let board_id = chat
            .board_id
            .clone()
            .ok_or(ChatAgentError::MissingField("board_id"))?;

        let spec = self
            .db
            .create_spec(&CreateSpec {
                board_id,
                target_board_id: None,
                project_id: chat.project_id.clone(),
                name: truncate_for_name(&user_input),
                user_input,
                model: self.config.model.clone(),
                settings: serde_json::json!({}),
            })
            .map_err(|e| ChatAgentError::AgentFailed(format!("Failed to create spec: {}", e)))?;

        self.db
            .update_chat_spec_id(&self.config.chat_id, &spec.id)?;

        self.broadcast(LiveEvent::ChatUpdated {
            chat_id: self.config.chat_id.clone(),
        });

        Ok(spec.id)
    }

    /// Convert chat messages to conversation messages for the spec discovery prompt builder.
    fn convert_to_conv_messages(
        messages: &[ChatMessage],
        spec_id: &str,
    ) -> Vec<ConversationMessage> {
        messages
            .iter()
            .filter(|m| m.role != ChatMessageRole::System)
            .map(|m| ConversationMessage {
                id: m.id.clone(),
                spec_id: spec_id.to_string(),
                role: match m.role {
                    ChatMessageRole::User => ConversationRole::User,
                    ChatMessageRole::Assistant => ConversationRole::Assistant,
                    ChatMessageRole::System => ConversationRole::System,
                },
                content: m.content.clone(),
                created_at: m.created_at,
            })
            .collect()
    }

    /// Handle completed spec discovery: update spec with refined requirements,
    /// set version to Planning, and generate the plan in-session.
    async fn handle_spec_discovery_completion(
        &self,
        spec_id: &str,
        version_id: &str,
        original_user_input: &str,
        structured_spec: Option<&StructuredSpec>,
    ) -> Result<(), ChatAgentError> {
        let structured = match structured_spec {
            Some(s) => s,
            None => return Ok(()),
        };

        let observations_section =
            extract_latest_observations_from_chat(&self.db, &self.config.chat_id);

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
            observations_section,
        );

        let exploration_entry = Exploration {
            query: "Codebase exploration during spec discovery".to_string(),
            response: if structured.technical_notes.is_empty() {
                "Exploration completed during conversational spec discovery.".to_string()
            } else {
                structured.technical_notes.join("\n")
            },
            timestamp: chrono::Utc::now(),
        };

        self.db
            .update_spec(
                spec_id,
                &UpdateSpec {
                    user_input: Some(enhanced_input.clone()),
                    ..Default::default()
                },
            )
            .map_err(|e| ChatAgentError::AgentFailed(format!("Failed to update spec: {}", e)))?;

        self.db
            .update_spec_version(
                version_id,
                &UpdateSpecVersion {
                    exploration_log: Some(vec![exploration_entry]),
                    status: Some(SpecVersionStatus::Planning),
                    ..Default::default()
                },
            )
            .map_err(|e| {
                ChatAgentError::AgentFailed(format!("Failed to update spec version: {}", e))
            })?;

        let _ = self.event_tx.send(LiveEvent::SpecUpdated {
            spec_id: spec_id.to_string(),
        });

        self.send_system_message_with_metadata(
            "Spec finalized. Generating plan...",
            &serde_json::json!({
                "type": "spec_finalized",
                "spec_id": spec_id,
                "requirements": structured.requirements,
                "decisions": structured.decisions,
                "constraints": structured.constraints,
                "technical_notes": structured.technical_notes,
            }),
        )
        .await;

        self.generate_plan_in_session(spec_id, version_id, &enhanced_input)
            .await;

        Ok(())
    }

    /// When the agent returns only observations (no questions), automatically
    /// request spec completion by re-running with the COMPLETION_PROMPT appended.
    async fn handle_spec_discovery_auto_completion(
        &self,
        spec_id: &str,
        version_id: &str,
        version_number: i32,
        original_user_input: &str,
    ) -> Result<(), ChatAgentError> {
        tracing::info!(
            "Spec builder: no questions returned, requesting auto-completion for spec {}",
            spec_id
        );

        self.send_system_message(&format!(
            "Generating spec... (Version {})",
            version_number
        ))
        .await;

        let fresh_messages = self.db.get_chat_messages(&self.config.chat_id)?;
        let mut conv_messages: Vec<_> = Self::convert_to_conv_messages(&fresh_messages, spec_id)
            .into_iter()
            .skip(1)
            .collect();
        conv_messages.push(ConversationMessage {
            id: "completion-request".to_string(),
            spec_id: spec_id.to_string(),
            role: ConversationRole::User,
            content: COMPLETION_PROMPT.to_string(),
            created_at: chrono::Utc::now(),
        });

        let prompt = build_conversation_prompt(original_user_input, &conv_messages);

        match self.run_agent(&prompt).await {
            Ok((text, stdout, ts_lines)) => {
                match parse_response(&text) {
                    Ok(response) => {
                        let metadata = if response.is_complete {
                            Some(serde_json::json!({ "spec_complete": true }))
                        } else {
                            None
                        };
                        let msg = self
                            .save_assistant_message(&response.message, metadata.as_ref())
                            .await?;
                        self.persist_log_events(&ts_lines, &msg.id);
                        self.extract_and_store_cost(&stdout, Some(&msg.id)).await?;

                        if response.is_complete {
                            self.handle_spec_discovery_completion(
                                spec_id,
                                version_id,
                                original_user_input,
                                response.structured_spec.as_ref(),
                            )
                            .await?;
                        } else {
                            tracing::warn!(
                                "Auto-completion response was not complete for spec {}",
                                spec_id
                            );
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse auto-completion response: {}", e);
                        self.send_system_message(&format!("Failed to generate spec: {}", e))
                            .await;
                    }
                }
            }
            Err(e) => {
                tracing::error!("Auto-completion agent call failed: {}", e);
                self.send_system_message(&format!("Failed to generate spec: {}", e))
                    .await;
            }
        }

        Ok(())
    }

    /// Insert a system message into the chat and broadcast the event.
    async fn send_system_message(&self, content: &str) {
        match self.db.create_chat_message(
            &self.config.chat_id,
            ChatMessageRole::System,
            content,
            None,
        ) {
            Ok(msg) => {
                self.broadcast(LiveEvent::ChatMessageAdded {
                    chat_id: self.config.chat_id.clone(),
                    message_id: msg.id,
                    role: "system".to_string(),
                });
            }
            Err(e) => {
                tracing::warn!("Failed to save system message: {}", e);
            }
        }
    }

    /// Generate the plan in-session using the same chat agent session, so the
    /// agent retains full conversation context and the user sees progress.
    async fn generate_plan_in_session(
        &self,
        spec_id: &str,
        version_id: &str,
        enhanced_user_input: &str,
    ) {
        let exploration_note = "You already explored the codebase during our conversation above. \
            Use everything you learned to create the plan — do NOT re-explore.";

        let prompt = generate_planning_prompt(enhanced_user_input, exploration_note);

        match self.run_agent(&prompt).await {
            Ok((text, stdout, ts_lines)) => {
                let plan_msg = self
                    .save_assistant_message(
                        &text,
                        Some(&serde_json::json!({ "plan_response": true })),
                    )
                    .await;
                if let Ok(ref msg) = plan_msg {
                    self.persist_log_events(&ts_lines, &msg.id);
                    let _ = self.extract_and_store_cost(&stdout, Some(&msg.id)).await;
                }

                match parse_project_plan(&text) {
                    Ok(plan) => {
                        let markdown = generate_plan_markdown(&plan);
                        let plan_json = serde_json::to_value(&plan).ok();

                        if let Err(e) = self.db.set_spec_version_plan(
                            version_id,
                            &markdown,
                            plan_json.as_ref(),
                        ) {
                            tracing::error!("Failed to save plan: {}", e);
                            self.send_system_message(&format!("Failed to save plan: {}", e))
                                .await;
                            return;
                        }

                        if let Err(e) = self.db.set_spec_version_status(
                            version_id,
                            SpecVersionStatus::AwaitingApproval,
                        ) {
                            tracing::error!("Failed to update spec version status: {}", e);
                        }

                        self.broadcast(LiveEvent::PlanGenerated {
                            spec_id: spec_id.to_string(),
                        });
                        self.broadcast(LiveEvent::SpecUpdated {
                            spec_id: spec_id.to_string(),
                        });

                        self.send_system_message_with_metadata(
                            "Plan generated. View your spec to review and approve the plan.",
                            &serde_json::json!({
                                "action": "view_plan",
                                "spec_id": spec_id,
                            }),
                        ).await;

                        tracing::info!(
                            "Plan generated in-session for spec {}: {} epics, {} tickets",
                            spec_id,
                            plan.epics.len(),
                            plan.epics.iter().map(|e| e.tickets.len()).sum::<usize>(),
                        );
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse plan JSON for spec {}: {}", spec_id, e);
                        let _ = self.db.set_spec_version_status(
                            version_id,
                            SpecVersionStatus::Failed,
                        );
                        self.broadcast(LiveEvent::SpecUpdated {
                            spec_id: spec_id.to_string(),
                        });
                        self.send_system_message(&format!(
                            "Plan generation produced invalid output: {}",
                            e,
                        )).await;
                    }
                }
            }
            Err(e) => {
                tracing::error!("Plan generation agent call failed for spec {}: {}", spec_id, e);
                let _ = self.db.set_spec_version_status(
                    version_id,
                    SpecVersionStatus::Failed,
                );
                self.broadcast(LiveEvent::SpecUpdated {
                    spec_id: spec_id.to_string(),
                });
                self.send_system_message(&format!("Plan generation failed: {}", e))
                    .await;
            }
        }
    }

    /// Insert a system message with metadata into the chat and broadcast.
    async fn send_system_message_with_metadata(
        &self,
        content: &str,
        metadata: &serde_json::Value,
    ) {
        match self.db.create_chat_message(
            &self.config.chat_id,
            ChatMessageRole::System,
            content,
            Some(metadata),
        ) {
            Ok(msg) => {
                self.broadcast(LiveEvent::ChatMessageAdded {
                    chat_id: self.config.chat_id.clone(),
                    message_id: msg.id,
                    role: "system".to_string(),
                });
            }
            Err(e) => {
                tracing::warn!("Failed to save system message: {}", e);
            }
        }
    }
}

/// Extract observations from the latest assistant message in the chat.
fn extract_latest_observations_from_chat(db: &Arc<Database>, chat_id: &str) -> String {
    let messages = match db.get_chat_messages(chat_id) {
        Ok(m) => m,
        Err(_) => return String::new(),
    };

    for msg in messages.iter().rev() {
        if msg.role == ChatMessageRole::Assistant {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&msg.content) {
                if let Some(obs) = parsed.get("observations").and_then(|v| v.as_str()) {
                    let trimmed = obs.trim();
                    if !trimmed.is_empty() {
                        return format!(
                            "\n\n## Codebase Observations (from discovery)\n{}",
                            trimmed
                        );
                    }
                }
            }
        }
    }

    String::new()
}

/// Derive a short spec name from user input (first ~60 chars of the first line).
fn truncate_for_name(input: &str) -> String {
    let first_line = input.lines().next().unwrap_or(input);
    if first_line.len() <= 60 {
        first_line.to_string()
    } else {
        let boundary = first_line
            .char_indices()
            .take_while(|(i, _)| *i < 57)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(57);
        format!("{}...", &first_line[..boundary])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_for_name_short() {
        assert_eq!(truncate_for_name("Build a login page"), "Build a login page");
    }

    #[test]
    fn truncate_for_name_long() {
        let long = "Build a comprehensive user authentication system with OAuth2 support and session management";
        let result = truncate_for_name(long);
        assert!(result.len() <= 63);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn truncate_for_name_multiline() {
        let input = "First line is the name\nSecond line has details";
        assert_eq!(truncate_for_name(input), "First line is the name");
    }

    #[test]
    fn convert_to_conv_messages_filters_system() {
        let messages = vec![
            ChatMessage {
                id: "1".into(),
                chat_id: "c".into(),
                role: ChatMessageRole::System,
                content: "system msg".into(),
                metadata: None,
                created_at: chrono::Utc::now(),
            },
            ChatMessage {
                id: "2".into(),
                chat_id: "c".into(),
                role: ChatMessageRole::User,
                content: "user msg".into(),
                metadata: None,
                created_at: chrono::Utc::now(),
            },
        ];

        let conv = ChatAgent::convert_to_conv_messages(&messages, "spec-1");
        assert_eq!(conv.len(), 1);
        assert_eq!(conv[0].role, ConversationRole::User);
        assert_eq!(conv[0].spec_id, "spec-1");
    }
}
