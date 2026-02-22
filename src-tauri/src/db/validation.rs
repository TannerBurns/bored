//! Database operations for validation sessions and messages

use super::{parse_datetime, Database, DbError};
use crate::db::models::{
    CreateValidationMessage, CreateValidationSession, ValidationMessage,
    ValidationMessageRole, ValidationSession, ValidationSessionStatus,
};
use rusqlite::params;
use uuid::Uuid;

impl Database {
    // --- Validation Sessions ---

    pub fn create_validation_session(
        &self,
        input: &CreateValidationSession,
    ) -> Result<ValidationSession, DbError> {
        self.with_conn(|conn| {
            let id = Uuid::new_v4().to_string();
            let now = chrono::Utc::now();
            let now_str = now.to_rfc3339();

            conn.execute(
                r#"INSERT INTO validation_sessions (id, ticket_id, project_id, status, agent_type, created_at, updated_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
                params![
                    id,
                    input.ticket_id,
                    input.project_id,
                    ValidationSessionStatus::Created.as_str(),
                    input.agent_type,
                    now_str,
                    now_str,
                ],
            )?;

            Ok(ValidationSession {
                id,
                ticket_id: input.ticket_id.clone(),
                project_id: input.project_id.clone(),
                status: ValidationSessionStatus::Created,
                agent_type: input.agent_type.clone(),
                created_at: now,
                updated_at: now,
            })
        })
    }

    pub fn get_validation_session(&self, id: &str) -> Result<ValidationSession, DbError> {
        self.with_conn(|conn| {
            conn.query_row(
                r#"SELECT id, ticket_id, project_id, status, agent_type, created_at, updated_at
                   FROM validation_sessions
                   WHERE id = ?1"#,
                [id],
                |row| {
                    let status_str: String = row.get(3)?;
                    Ok(ValidationSession {
                        id: row.get(0)?,
                        ticket_id: row.get(1)?,
                        project_id: row.get(2)?,
                        status: ValidationSessionStatus::parse(&status_str)
                            .unwrap_or(ValidationSessionStatus::Created),
                        agent_type: row.get(4)?,
                        created_at: parse_datetime(row.get(5)?),
                        updated_at: parse_datetime(row.get(6)?),
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    DbError::NotFound(format!("Validation session not found: {}", id))
                }
                _ => DbError::Sqlite(e),
            })
        })
    }

    pub fn get_validation_sessions(
        &self,
        ticket_id: &str,
    ) -> Result<Vec<ValidationSession>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, ticket_id, project_id, status, agent_type, created_at, updated_at
                   FROM validation_sessions
                   WHERE ticket_id = ?1
                   ORDER BY created_at DESC"#,
            )?;

            let sessions = stmt
                .query_map([ticket_id], |row| {
                    let status_str: String = row.get(3)?;
                    Ok(ValidationSession {
                        id: row.get(0)?,
                        ticket_id: row.get(1)?,
                        project_id: row.get(2)?,
                        status: ValidationSessionStatus::parse(&status_str)
                            .unwrap_or(ValidationSessionStatus::Created),
                        agent_type: row.get(4)?,
                        created_at: parse_datetime(row.get(5)?),
                        updated_at: parse_datetime(row.get(6)?),
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(sessions)
        })
    }

    pub fn update_validation_session_status(
        &self,
        id: &str,
        status: &ValidationSessionStatus,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now_str = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "UPDATE validation_sessions SET status = ?1, updated_at = ?2 WHERE id = ?3",
                params![status.as_str(), now_str, id],
            )?;
            Ok(())
        })
    }

    pub fn delete_validation_session(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            conn.execute("DELETE FROM validation_sessions WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    // --- Validation Messages ---

    pub fn create_validation_message(
        &self,
        input: &CreateValidationMessage,
    ) -> Result<ValidationMessage, DbError> {
        self.with_conn(|conn| {
            let id = Uuid::new_v4().to_string();
            let created_at = chrono::Utc::now();
            let created_at_str = created_at.to_rfc3339();
            let metadata_str = input.metadata.as_ref().map(|m| m.to_string());

            conn.execute(
                r#"INSERT INTO validation_messages (id, session_id, role, content, metadata_json, created_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
                params![
                    id,
                    input.session_id,
                    input.role.as_str(),
                    input.content,
                    metadata_str,
                    created_at_str,
                ],
            )?;

            Ok(ValidationMessage {
                id,
                session_id: input.session_id.clone(),
                role: input.role.clone(),
                content: input.content.clone(),
                metadata: input.metadata.clone(),
                created_at,
            })
        })
    }

    pub fn get_validation_messages(
        &self,
        session_id: &str,
    ) -> Result<Vec<ValidationMessage>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, session_id, role, content, metadata_json, created_at
                   FROM validation_messages
                   WHERE session_id = ?1
                   ORDER BY created_at ASC"#,
            )?;

            let messages = stmt
                .query_map([session_id], |row| {
                    let role_str: String = row.get(2)?;
                    let metadata_str: Option<String> = row.get(4)?;
                    Ok(ValidationMessage {
                        id: row.get(0)?,
                        session_id: row.get(1)?,
                        role: ValidationMessageRole::parse(&role_str)
                            .unwrap_or(ValidationMessageRole::User),
                        content: row.get(3)?,
                        metadata: metadata_str
                            .and_then(|s| serde_json::from_str(&s).ok()),
                        created_at: parse_datetime(row.get(5)?),
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(messages)
        })
    }
}
