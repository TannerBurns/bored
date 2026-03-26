use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::agents::validation_agent::parsing::{
    parse_create_fix_tasks_from_response, parse_run_command_from_response,
    parse_start_app_from_response, parse_stop_app_from_response,
};
use crate::agents::validation_agent::prompts::{
    build_conversation_prompt, build_initial_prompt, build_resumption_prompt,
};
use crate::agents::validation_agent::{AppLogEventKind, AppProcessManager, StartResult};
use crate::api::state::LiveEvent;
use crate::commands::next_steps::get_branch_diff_sync;
use crate::db::models::{
    ChatMessage, ChatMessageRole, ChatStatus, ValidationMessage, ValidationMessageRole,
};
use crate::db::Database;

use super::config::ChatAgentError;
use super::review_tasks::{
    post_fix_tasks_completion_chat, process_fix_tasks_for_chat, wait_for_fix_tasks_chat,
};
use super::ChatAgent;

impl ChatAgent {
    pub(crate) async fn run_review(
        &self,
        messages: Vec<ChatMessage>,
        app_manager: &AppProcessManager,
    ) -> Result<ChatMessage, ChatAgentError> {
        let chat = self.db.get_chat(&self.config.chat_id)?;
        let ticket_id = chat
            .ticket_id
            .ok_or(ChatAgentError::MissingField("ticket_id"))?;
        let board_id = chat
            .board_id
            .ok_or(ChatAgentError::MissingField("board_id"))?;

        let ticket = self
            .db
            .get_ticket(&ticket_id)
            .map_err(|e| ChatAgentError::AgentFailed(e.to_string()))?;

        if let Some(ref chat_project_id) = chat.project_id {
            if ticket.project_id.as_deref() != Some(chat_project_id) {
                return Err(ChatAgentError::AgentFailed(
                    "Ticket does not belong to the chat's project".into(),
                ));
            }
        }

        let project_path = if let Some(ref project_id) = chat.project_id {
            let project = self.db.get_project(project_id)?
                .ok_or_else(|| ChatAgentError::AgentFailed(format!("Project not found: {}", project_id)))?;
            project.path
        } else if let Some(ref workspace_id) = chat.workspace_id {
            let projects = self.db.get_workspace_projects(workspace_id)
                .map_err(|e| ChatAgentError::AgentFailed(e.to_string()))?;
            projects.first()
                .ok_or_else(|| ChatAgentError::AgentFailed("Workspace has no projects".into()))?
                .path.clone()
        } else {
            return Err(ChatAgentError::AgentFailed("Chat has no project or workspace".into()));
        };

        let branch_diff = get_branch_diff_sync(&self.db, &ticket_id)
            .map_err(ChatAgentError::AgentFailed)?
            .diff;

        let (working_dir_path, worktree_path, repo_path_for_cleanup) =
            resolve_review_working_dir(&self.db, &ticket_id, &project_path, &self.config.chat_id)?;
        let working_dir = Path::new(&working_dir_path);

        let is_first_turn = !messages.iter().any(|m| m.role == ChatMessageRole::Assistant);
        let has_session = chat.agent_session_id.is_some();
        let val_messages = chat_to_validation_messages(&messages);
        let user_message = messages
            .iter()
            .rev()
            .find(|m| m.role == ChatMessageRole::User)
            .map(|m| m.content.as_str())
            .unwrap_or("");
        let prompt = if is_first_turn {
            build_initial_prompt(
                &ticket.title,
                &ticket.description_md,
                &branch_diff,
                None,
                user_message,
            )
        } else if has_session {
            let new_msgs = extract_new_messages(&val_messages);
            build_resumption_prompt(&new_msgs)
        } else {
            build_conversation_prompt(
                &ticket.title,
                &ticket.description_md,
                &branch_diff,
                None,
                &val_messages,
            )
        };

        let (response_text, stdout, ts_lines) = self.run_agent(&prompt).await?;

        let has_fix_task = parse_create_fix_tasks_from_response(&response_text).is_some();
        let metadata = if has_fix_task {
            Some(serde_json::json!({ "type": "fix_task_response" }))
        } else {
            None
        };
        let assistant_msg = self
            .save_assistant_message(&response_text, metadata.as_ref())
            .await?;
        self.persist_log_events(&ts_lines, &assistant_msg.id);
        self.extract_and_store_cost(&stdout, Some(&assistant_msg.id))
            .await?;

        let mut current_response = response_text;
        let mut last_assistant_msg = assistant_msg;
        let mut all_fix_task_ids: Vec<String> = Vec::new();
        let mut fix_tasks_already_extracted = false;
        const MAX_ROUNDS: usize = 10;

        for _round in 0..MAX_ROUNDS {
            // 1. Check for run_command
            if let Some(rc) = parse_run_command_from_response(&current_response) {
                tracing::info!("Running review command: {}", rc.command);

                let cmd_cwd = rc.cwd.as_deref().map(Path::new).unwrap_or(working_dir);
                let cmd_output = std::process::Command::new("sh")
                    .args(["-c", &rc.command])
                    .current_dir(cmd_cwd)
                    .output();

                let (exit_code, stdout_str, stderr_str) = match cmd_output {
                    Ok(output) => (
                        output.status.code().unwrap_or(-1),
                        String::from_utf8_lossy(&output.stdout).to_string(),
                        String::from_utf8_lossy(&output.stderr).to_string(),
                    ),
                    Err(e) => (-1, String::new(), format!("Failed to execute: {}", e)),
                };

                let combined = if stderr_str.is_empty() {
                    stdout_str.chars().take(3000).collect::<String>()
                } else {
                    let out: String = stdout_str.chars().take(1500).collect();
                    let err: String = stderr_str.chars().take(1500).collect();
                    format!("stdout:\n{}\nstderr:\n{}", out, err)
                };

                let status_label = if exit_code == 0 { "success" } else { "failed" };
                self.save_system_message(&format!(
                    "Ran `{}` (exit {}, {})",
                    rc.command, exit_code, status_label
                ))
                .await;
                self.save_user_message(&format!(
                    "Command `{}` finished with exit code {}.\n\nOutput:\n```\n{}\n```",
                    rc.command, exit_code, combined
                ))
                .await;

                if !fix_tasks_already_extracted {
                    let ids = process_fix_tasks_for_chat(
                        &current_response,
                        &self.db,
                        &self.config.chat_id,
                        &ticket_id,
                        &ticket.title,
                        &board_id,
                        &self.event_tx,
                    );
                    all_fix_task_ids.extend(ids);
                }

                let (next_response, next_stdout, next_msg) =
                    self.review_agent_followup(&ticket, &branch_diff).await?;
                let follow_fix_ids = process_fix_tasks_for_chat(
                    &next_response,
                    &self.db,
                    &self.config.chat_id,
                    &ticket_id,
                    &ticket.title,
                    &board_id,
                    &self.event_tx,
                );
                all_fix_task_ids.extend(follow_fix_ids);
                self.extract_and_store_cost(&next_stdout, Some(&next_msg.id))
                    .await?;
                current_response = next_response;
                last_assistant_msg = next_msg;
                fix_tasks_already_extracted = true;
                continue;
            }

            // 2. Check for stop_app
            if parse_stop_app_from_response(&current_response) {
                let was_running = app_manager.kill_process(&self.config.chat_id);
                let stop_label = if was_running {
                    "App stopped."
                } else {
                    "No app was running."
                };
                self.save_system_message(stop_label).await;
                self.save_user_message(stop_label).await;

                if !fix_tasks_already_extracted {
                    let ids = process_fix_tasks_for_chat(
                        &current_response,
                        &self.db,
                        &self.config.chat_id,
                        &ticket_id,
                        &ticket.title,
                        &board_id,
                        &self.event_tx,
                    );
                    all_fix_task_ids.extend(ids);
                }

                let (next_response, next_stdout, next_msg) =
                    self.review_agent_followup(&ticket, &branch_diff).await?;
                let follow_fix_ids = process_fix_tasks_for_chat(
                    &next_response,
                    &self.db,
                    &self.config.chat_id,
                    &ticket_id,
                    &ticket.title,
                    &board_id,
                    &self.event_tx,
                );
                all_fix_task_ids.extend(follow_fix_ids);
                self.extract_and_store_cost(&next_stdout, Some(&next_msg.id))
                    .await?;
                current_response = next_response;
                last_assistant_msg = next_msg;
                fix_tasks_already_extracted = true;
                continue;
            }

            // 3. Check for start_app
            if let Some(start_app) = parse_start_app_from_response(&current_response) {
                let start_result = app_manager.start(
                    self.config.chat_id.clone(),
                    start_app.command.clone(),
                    working_dir,
                    self.event_tx.clone(),
                    worktree_path.clone(),
                    repo_path_for_cleanup.clone(),
                    AppLogEventKind::Chat,
                );

                match start_result {
                    Ok(StartResult::ExitedEarly { exit_code, output }) => {
                        self.save_system_message(&format!(
                            "App failed to start: `{}` (exit {})",
                            start_app.command, exit_code
                        ))
                        .await;
                        self.save_user_message(&format!(
                            "The app failed to start. Command `{}` exited with code {}.\n\nLast output:\n```\n{}\n```\n\nPlease diagnose the issue and output a `run_command` to fix it, or a new `start_app` to try again.",
                            start_app.command,
                            exit_code,
                            output.chars().take(3000).collect::<String>()
                        )).await;

                        if !fix_tasks_already_extracted {
                            let ids = process_fix_tasks_for_chat(
                                &current_response,
                                &self.db,
                                &self.config.chat_id,
                                &ticket_id,
                                &ticket.title,
                                &board_id,
                                &self.event_tx,
                            );
                            all_fix_task_ids.extend(ids);
                        }

                        let (next_response, next_stdout, next_msg) =
                            self.review_agent_followup(&ticket, &branch_diff).await?;
                        let follow_fix_ids = process_fix_tasks_for_chat(
                            &next_response,
                            &self.db,
                            &self.config.chat_id,
                            &ticket_id,
                            &ticket.title,
                            &board_id,
                            &self.event_tx,
                        );
                        all_fix_task_ids.extend(follow_fix_ids);
                        self.extract_and_store_cost(&next_stdout, Some(&next_msg.id))
                            .await?;
                        current_response = next_response;
                        last_assistant_msg = next_msg;
                        fix_tasks_already_extracted = true;
                        continue;
                    }
                    Err(e) => {
                        tracing::error!("Failed to start app: {}", e);
                        self.save_system_message(&format!(
                            "App failed to start: `{}` (error: {})",
                            start_app.command, e
                        ))
                        .await;
                        self.save_user_message(&format!(
                            "The app failed to start. Command `{}` encountered an error: {}\n\nPlease diagnose the issue and output a `run_command` to fix it, or a new `start_app` to try again.",
                            start_app.command, e
                        )).await;

                        if !fix_tasks_already_extracted {
                            let ids = process_fix_tasks_for_chat(
                                &current_response,
                                &self.db,
                                &self.config.chat_id,
                                &ticket_id,
                                &ticket.title,
                                &board_id,
                                &self.event_tx,
                            );
                            all_fix_task_ids.extend(ids);
                        }

                        let (next_response, next_stdout, next_msg) =
                            self.review_agent_followup(&ticket, &branch_diff).await?;
                        let follow_fix_ids = process_fix_tasks_for_chat(
                            &next_response,
                            &self.db,
                            &self.config.chat_id,
                            &ticket_id,
                            &ticket.title,
                            &board_id,
                            &self.event_tx,
                        );
                        all_fix_task_ids.extend(follow_fix_ids);
                        self.extract_and_store_cost(&next_stdout, Some(&next_msg.id))
                            .await?;
                        current_response = next_response;
                        last_assistant_msg = next_msg;
                        fix_tasks_already_extracted = true;
                        continue;
                    }
                    Ok(StartResult::Running) => {}
                }

                if app_manager.is_running(&self.config.chat_id) {
                    self.db
                        .update_chat_status(&self.config.chat_id, ChatStatus::Active)
                        .ok();
                    self.broadcast(LiveEvent::ChatUpdated {
                        chat_id: self.config.chat_id.clone(),
                    });

                    let port_suffix = start_app
                        .port
                        .map(|p| format!(" on port {}", p))
                        .unwrap_or_default();
                    let log_file_path = working_dir.join(".validation-app.log");
                    let log_file_str = log_file_path.to_string_lossy();

                    self.save_system_message(&format!(
                        "App started: `{}`{}. Logs are streaming in the panel.",
                        start_app.command, port_suffix
                    ))
                    .await;
                    self.save_user_message(&format!(
                        "The application is now running{}. App logs are being written to `{}` — you can read that file to check for errors. Please provide testing instructions.",
                        port_suffix, log_file_str
                    )).await;

                    let (follow_up_response, follow_up_stdout, follow_up_msg) =
                        self.review_agent_followup(&ticket, &branch_diff).await?;

                    if !fix_tasks_already_extracted {
                        let ids = process_fix_tasks_for_chat(
                            &current_response,
                            &self.db,
                            &self.config.chat_id,
                            &ticket_id,
                            &ticket.title,
                            &board_id,
                            &self.event_tx,
                        );
                        all_fix_task_ids.extend(ids);
                    }
                    let follow_fix_ids = process_fix_tasks_for_chat(
                        &follow_up_response,
                        &self.db,
                        &self.config.chat_id,
                        &ticket_id,
                        &ticket.title,
                        &board_id,
                        &self.event_tx,
                    );
                    all_fix_task_ids.extend(follow_fix_ids);

                    self.extract_and_store_cost(&follow_up_stdout, Some(&follow_up_msg.id))
                        .await?;

                    if !all_fix_task_ids.is_empty() {
                        wait_for_fix_tasks_chat(
                            &all_fix_task_ids,
                            &self.db,
                            &self.event_tx,
                            &self.config.chat_id,
                        )
                        .await;
                        if let Some(msg) = post_fix_tasks_completion_chat(
                            &all_fix_task_ids,
                            &self.db,
                            &self.event_tx,
                            &self.config.chat_id,
                        ) {
                            return Ok(msg);
                        }
                    }

                    return Ok(follow_up_msg);
                }
            }
            break;
        }

        // Clean up worktree if no app is running
        if !app_manager.is_running(&self.config.chat_id) {
            if let (Some(wt), Some(repo)) = (worktree_path, repo_path_for_cleanup) {
                if let Err(e) = crate::agents::worktree::remove_worktree(&wt, &repo) {
                    tracing::warn!("Failed to remove review worktree after loop: {}", e);
                }
            }
        }

        if !fix_tasks_already_extracted {
            let ids = process_fix_tasks_for_chat(
                &current_response,
                &self.db,
                &self.config.chat_id,
                &ticket_id,
                &ticket.title,
                &board_id,
                &self.event_tx,
            );
            all_fix_task_ids.extend(ids);
        }

        if !all_fix_task_ids.is_empty() {
            wait_for_fix_tasks_chat(
                &all_fix_task_ids,
                &self.db,
                &self.event_tx,
                &self.config.chat_id,
            )
            .await;
            if let Some(msg) = post_fix_tasks_completion_chat(
                &all_fix_task_ids,
                &self.db,
                &self.event_tx,
                &self.config.chat_id,
            ) {
                return Ok(msg);
            }
        }

        Ok(last_assistant_msg)
    }

    /// Get a follow-up response from the agent using fresh messages from the DB.
    async fn review_agent_followup(
        &self,
        ticket: &crate::db::models::Ticket,
        branch_diff: &str,
    ) -> Result<(String, String, ChatMessage), ChatAgentError> {
        let messages = self.db.get_chat_messages(&self.config.chat_id)?;
        let val_messages = chat_to_validation_messages(&messages);

        let has_session = self
            .db
            .get_chat(&self.config.chat_id)
            .ok()
            .and_then(|c| c.agent_session_id)
            .is_some();

        let prompt = if has_session {
            let new_msgs = extract_new_messages(&val_messages);
            build_resumption_prompt(&new_msgs)
        } else {
            build_conversation_prompt(
                &ticket.title,
                &ticket.description_md,
                branch_diff,
                None,
                &val_messages,
            )
        };

        let (text, stdout, ts_lines) = self.run_agent(&prompt).await?;

        let has_fix = parse_create_fix_tasks_from_response(&text).is_some();
        let meta = if has_fix {
            Some(serde_json::json!({ "type": "fix_task_response" }))
        } else {
            None
        };
        let msg = self.save_assistant_message(&text, meta.as_ref()).await?;
        self.persist_log_events(&ts_lines, &msg.id);

        Ok((text, stdout, msg))
    }

    async fn save_system_message(&self, content: &str) {
        if let Ok(msg) = self.db.create_chat_message(
            &self.config.chat_id,
            ChatMessageRole::System,
            content,
            None,
        ) {
            self.broadcast(LiveEvent::ChatMessageAdded {
                chat_id: self.config.chat_id.clone(),
                message_id: msg.id,
                role: "system".to_string(),
            });
        }
    }

    async fn save_user_message(&self, content: &str) {
        if let Ok(msg) = self.db.create_chat_message(
            &self.config.chat_id,
            ChatMessageRole::User,
            content,
            None,
        ) {
            self.broadcast(LiveEvent::ChatMessageAdded {
                chat_id: self.config.chat_id.clone(),
                message_id: msg.id,
                role: "user".to_string(),
            });
        }
    }
}

/// Extract messages after the last assistant response.
fn extract_new_messages(messages: &[ValidationMessage]) -> Vec<ValidationMessage> {
    let last_assistant_idx = messages
        .iter()
        .rposition(|m| m.role == ValidationMessageRole::Assistant);
    match last_assistant_idx {
        Some(idx) => messages[idx + 1..].to_vec(),
        None => messages.to_vec(),
    }
}

fn chat_to_validation_messages(messages: &[ChatMessage]) -> Vec<ValidationMessage> {
    messages
        .iter()
        .map(|m| ValidationMessage {
            id: m.id.clone(),
            session_id: String::new(),
            role: match m.role {
                ChatMessageRole::User => ValidationMessageRole::User,
                ChatMessageRole::Assistant => ValidationMessageRole::Assistant,
                ChatMessageRole::System => ValidationMessageRole::System,
            },
            content: m.content.clone(),
            metadata: None,
            created_at: m.created_at,
        })
        .collect()
}

fn resolve_review_working_dir(
    db: &Arc<Database>,
    ticket_id: &str,
    project_path: &str,
    chat_id: &str,
) -> Result<(String, Option<PathBuf>, Option<PathBuf>), ChatAgentError> {
    match crate::commands::next_steps::get_ticket_working_dir(db, ticket_id) {
        Ok((wt_path, _branch)) => {
            if wt_path != project_path {
                Ok((wt_path, None, None))
            } else {
                let ticket = db
                    .get_ticket(ticket_id)
                    .map_err(|e| ChatAgentError::AgentFailed(e.to_string()))?;
                let branch = ticket.branch_name;
                if let Some(branch_name) = branch {
                    let repo = PathBuf::from(project_path);
                    let worktree_id = format!("review-{}", chat_id);

                    let expected_path =
                        crate::agents::worktree::get_default_worktree_base().join(&worktree_id);
                    if expected_path.exists() && expected_path.join(".git").exists() {
                        tracing::info!(
                            "Reusing existing review worktree at {} for branch {}",
                            expected_path.display(),
                            branch_name
                        );
                        return Ok((
                            expected_path.to_string_lossy().to_string(),
                            Some(expected_path),
                            Some(repo),
                        ));
                    }

                    match crate::agents::worktree::create_worktree_with_existing_branch(
                        &repo,
                        &branch_name,
                        &worktree_id,
                        None,
                    ) {
                        Ok(wt_info) => {
                            tracing::info!(
                                "Created review worktree at {} for branch {}",
                                wt_info.path.display(),
                                branch_name
                            );
                            let wt = wt_info.path.clone();
                            Ok((wt.to_string_lossy().to_string(), Some(wt), Some(repo)))
                        }
                        Err(e) => {
                            tracing::warn!("Failed to create review worktree: {}", e);
                            Ok((project_path.to_string(), None, None))
                        }
                    }
                } else {
                    Ok((project_path.to_string(), None, None))
                }
            }
        }
        Err(_) => Ok((project_path.to_string(), None, None)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_msg(id: &str, role: ValidationMessageRole, content: &str) -> ValidationMessage {
        ValidationMessage {
            id: id.into(),
            session_id: String::new(),
            role,
            content: content.into(),
            metadata: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn extract_new_messages_after_assistant() {
        let messages = vec![
            make_msg("1", ValidationMessageRole::User, "start"),
            make_msg("2", ValidationMessageRole::Assistant, "ok starting"),
            make_msg("3", ValidationMessageRole::System, "ran npm install"),
            make_msg("4", ValidationMessageRole::User, "command output"),
        ];
        let new = extract_new_messages(&messages);
        assert_eq!(new.len(), 2);
        assert_eq!(new[0].content, "ran npm install");
        assert_eq!(new[1].content, "command output");
    }

    #[test]
    fn extract_new_messages_no_assistant() {
        let messages = vec![
            make_msg("1", ValidationMessageRole::User, "hello"),
            make_msg("2", ValidationMessageRole::System, "info"),
        ];
        let new = extract_new_messages(&messages);
        assert_eq!(new.len(), 2);
    }

    #[test]
    fn extract_new_messages_empty() {
        let new = extract_new_messages(&[]);
        assert!(new.is_empty());
    }

    #[test]
    fn extract_new_messages_assistant_is_last() {
        let messages = vec![
            make_msg("1", ValidationMessageRole::User, "start"),
            make_msg("2", ValidationMessageRole::Assistant, "done"),
        ];
        let new = extract_new_messages(&messages);
        assert!(new.is_empty());
    }
}
