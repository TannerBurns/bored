use super::{parse_datetime, Database, DbError};
use crate::db::models::{
    Chat, ChatEvent, ChatMessage, ChatMessageRole, ChatMode, ChatRun, ChatRunStatus, ChatStatus,
    CreateChat,
};
use rusqlite::params;
use uuid::Uuid;

impl Database {
    pub fn create_chat(&self, input: &CreateChat) -> Result<Chat, DbError> {
        match input.mode {
            ChatMode::TicketBuilder | ChatMode::SpecBuilder => {
                if input.board_id.is_none() {
                    return Err(DbError::Validation(format!(
                        "{} mode requires board_id",
                        input.mode.as_str(),
                    )));
                }
            }
            ChatMode::Review => {
                if input.board_id.is_none() || input.ticket_id.is_none() {
                    return Err(DbError::Validation(
                        "review mode requires board_id and ticket_id".to_string(),
                    ));
                }
            }
            _ => {}
        }

        self.with_conn(|conn| {
            let id = Uuid::new_v4().to_string();
            let now = chrono::Utc::now();
            let now_str = now.to_rfc3339();

            conn.execute(
                r#"INSERT INTO chats (id, agent_type, project_id, mode, board_id, ticket_id, spec_id, model, status, created_at, updated_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
                params![
                    id,
                    input.agent_type,
                    input.project_id,
                    input.mode.as_str(),
                    input.board_id,
                    input.ticket_id,
                    input.spec_id,
                    input.model,
                    ChatStatus::Active.as_str(),
                    now_str,
                    now_str,
                ],
            )?;

            Ok(Chat {
                id,
                title: None,
                agent_type: input.agent_type.clone(),
                project_id: input.project_id.clone(),
                mode: input.mode.clone(),
                board_id: input.board_id.clone(),
                ticket_id: input.ticket_id.clone(),
                spec_id: input.spec_id.clone(),
                model: input.model.clone(),
                status: ChatStatus::Active,
                agent_session_id: None,
                created_at: now,
                updated_at: now,
            })
        })
    }

    pub fn get_chat(&self, id: &str) -> Result<Chat, DbError> {
        self.with_conn(|conn| {
            conn.query_row(
                r#"SELECT id, title, agent_type, project_id, mode, board_id, ticket_id, spec_id, model, status, created_at, updated_at, agent_session_id
                   FROM chats
                   WHERE id = ?1"#,
                [id],
                |row| {
                    let mode_str: String = row.get(4)?;
                    let status_str: String = row.get(9)?;
                    Ok(Chat {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        agent_type: row.get(2)?,
                        project_id: row.get(3)?,
                        mode: ChatMode::parse(&mode_str).unwrap_or(ChatMode::General),
                        board_id: row.get(5)?,
                        ticket_id: row.get(6)?,
                        spec_id: row.get(7)?,
                        model: row.get(8)?,
                        status: ChatStatus::parse(&status_str).unwrap_or(ChatStatus::Active),
                        agent_session_id: row.get(12)?,
                        created_at: parse_datetime(row.get(10)?),
                        updated_at: parse_datetime(row.get(11)?),
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    DbError::NotFound(format!("Chat not found: {}", id))
                }
                _ => DbError::Sqlite(e),
            })
        })
    }

    pub fn get_chats(&self, limit: i64, offset: i64) -> Result<Vec<Chat>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, title, agent_type, project_id, mode, board_id, ticket_id, spec_id, model, status, created_at, updated_at, agent_session_id
                   FROM chats
                   ORDER BY created_at DESC
                   LIMIT ?1 OFFSET ?2"#,
            )?;

            let chats = stmt
                .query_map(params![limit, offset], |row| {
                    let mode_str: String = row.get(4)?;
                    let status_str: String = row.get(9)?;
                    Ok(Chat {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        agent_type: row.get(2)?,
                        project_id: row.get(3)?,
                        mode: ChatMode::parse(&mode_str).unwrap_or(ChatMode::General),
                        board_id: row.get(5)?,
                        ticket_id: row.get(6)?,
                        spec_id: row.get(7)?,
                        model: row.get(8)?,
                        status: ChatStatus::parse(&status_str).unwrap_or(ChatStatus::Active),
                        agent_session_id: row.get(12)?,
                        created_at: parse_datetime(row.get(10)?),
                        updated_at: parse_datetime(row.get(11)?),
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(chats)
        })
    }

    pub fn update_chat_title(&self, id: &str, title: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now_str = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "UPDATE chats SET title = ?1, updated_at = ?2 WHERE id = ?3",
                params![title, now_str, id],
            )?;
            Ok(())
        })
    }

    pub fn update_chat_status(&self, id: &str, status: ChatStatus) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now_str = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "UPDATE chats SET status = ?1, updated_at = ?2 WHERE id = ?3",
                params![status.as_str(), now_str, id],
            )?;
            Ok(())
        })
    }

    pub fn update_chat_agent_session_id(
        &self,
        chat_id: &str,
        agent_session_id: Option<&str>,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE chats SET agent_session_id = ?1 WHERE id = ?2",
                params![agent_session_id, chat_id],
            )?;
            Ok(())
        })
    }

    pub fn update_chat_spec_id(&self, chat_id: &str, spec_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now_str = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "UPDATE chats SET spec_id = ?1, updated_at = ?2 WHERE id = ?3",
                params![spec_id, now_str, chat_id],
            )?;
            Ok(())
        })
    }

    pub fn delete_chat(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            conn.execute("DELETE FROM chats WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    // --- Chat Messages ---

    pub fn create_chat_message(
        &self,
        chat_id: &str,
        role: ChatMessageRole,
        content: &str,
        metadata: Option<&serde_json::Value>,
    ) -> Result<ChatMessage, DbError> {
        self.with_conn(|conn| {
            let id = Uuid::new_v4().to_string();
            let created_at = chrono::Utc::now();
            let created_at_str = created_at.to_rfc3339();
            let metadata_str = metadata.map(|m| m.to_string());

            conn.execute(
                r#"INSERT INTO chat_messages (id, chat_id, role, content, metadata_json, created_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
                params![id, chat_id, role.as_str(), content, metadata_str, created_at_str],
            )?;

            Ok(ChatMessage {
                id,
                chat_id: chat_id.to_string(),
                role,
                content: content.to_string(),
                metadata: metadata.cloned(),
                created_at,
            })
        })
    }

    pub fn get_chat_messages(&self, chat_id: &str) -> Result<Vec<ChatMessage>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, chat_id, role, content, metadata_json, created_at
                   FROM chat_messages
                   WHERE chat_id = ?1
                   ORDER BY created_at ASC"#,
            )?;

            let messages = stmt
                .query_map([chat_id], |row| {
                    let role_str: String = row.get(2)?;
                    let metadata_str: Option<String> = row.get(4)?;
                    Ok(ChatMessage {
                        id: row.get(0)?,
                        chat_id: row.get(1)?,
                        role: ChatMessageRole::parse(&role_str)
                            .unwrap_or(ChatMessageRole::User),
                        content: row.get(3)?,
                        metadata: metadata_str.and_then(|s| serde_json::from_str(&s).ok()),
                        created_at: parse_datetime(row.get(5)?),
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(messages)
        })
    }

    // --- Chat Events ---

    pub fn create_chat_event(
        &self,
        chat_id: &str,
        message_id: Option<&str>,
        event_type: &str,
        payload: &serde_json::Value,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let id = Uuid::new_v4().to_string();
            let created_at_str = chrono::Utc::now().to_rfc3339();
            let payload_str = payload.to_string();

            conn.execute(
                r#"INSERT INTO chat_events (id, chat_id, message_id, event_type, payload_json, created_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
                params![id, chat_id, message_id, event_type, payload_str, created_at_str],
            )?;

            Ok(())
        })
    }

    pub fn get_chat_events(&self, chat_id: &str) -> Result<Vec<ChatEvent>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, chat_id, message_id, event_type, payload_json, created_at
                   FROM chat_events
                   WHERE chat_id = ?1
                   ORDER BY created_at ASC"#,
            )?;

            let events = stmt
                .query_map([chat_id], |row| {
                    let payload_str: String = row.get(4)?;
                    Ok(ChatEvent {
                        id: row.get(0)?,
                        chat_id: row.get(1)?,
                        message_id: row.get(2)?,
                        event_type: row.get(3)?,
                        payload: serde_json::from_str(&payload_str)
                            .unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
                        created_at: parse_datetime(row.get(5)?),
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(events)
        })
    }

    // --- Chat Runs ---

    pub fn create_chat_run(
        &self,
        chat_id: &str,
        chat_message_id: Option<&str>,
        agent_type: &str,
    ) -> Result<ChatRun, DbError> {
        self.with_conn(|conn| {
            let id = Uuid::new_v4().to_string();
            let now = chrono::Utc::now();
            let now_str = now.to_rfc3339();

            conn.execute(
                r#"INSERT INTO chat_runs (id, chat_id, chat_message_id, agent_type, status, created_at, updated_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
                params![
                    id,
                    chat_id,
                    chat_message_id,
                    agent_type,
                    ChatRunStatus::Running.as_str(),
                    now_str,
                    now_str,
                ],
            )?;

            Ok(ChatRun {
                id,
                chat_id: chat_id.to_string(),
                chat_message_id: chat_message_id.map(|s| s.to_string()),
                agent_type: agent_type.to_string(),
                status: ChatRunStatus::Running,
                metadata: None,
                created_at: now,
                updated_at: now,
            })
        })
    }

    pub fn update_chat_run_status(
        &self,
        id: &str,
        status: ChatRunStatus,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now_str = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "UPDATE chat_runs SET status = ?1, updated_at = ?2 WHERE id = ?3",
                params![status.as_str(), now_str, id],
            )?;
            Ok(())
        })
    }

    pub fn set_chat_run_metadata(
        &self,
        id: &str,
        metadata: &serde_json::Value,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now_str = chrono::Utc::now().to_rfc3339();
            let metadata_str = metadata.to_string();
            conn.execute(
                "UPDATE chat_runs SET metadata_json = ?1, updated_at = ?2 WHERE id = ?3",
                params![metadata_str, now_str, id],
            )?;
            Ok(())
        })
    }
}
