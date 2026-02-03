//! Database operations for conversation messages (spec brainstorming)

use super::{parse_datetime, Database, DbError};
use crate::db::models::{ConversationMessage, ConversationRole, CreateConversationMessage};
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

impl Database {
    pub fn create_conversation_message(
        &self,
        input: &CreateConversationMessage,
    ) -> Result<ConversationMessage, DbError> {
        self.with_conn(|conn| {
            let id = Uuid::new_v4().to_string();
            let created_at = chrono::Utc::now();
            let created_at_str = created_at.to_rfc3339();

            conn.execute(
                r#"INSERT INTO conversation_messages (id, spec_id, role, content, created_at)
                   VALUES (?1, ?2, ?3, ?4, ?5)"#,
                params![id, input.spec_id, input.role.as_str(), input.content, created_at_str],
            )?;

            Ok(ConversationMessage {
                id,
                spec_id: input.spec_id.clone(),
                role: input.role.clone(),
                content: input.content.clone(),
                created_at,
            })
        })
    }

    /// Returns messages ordered by creation time (ascending).
    pub fn get_conversation_messages(
        &self,
        spec_id: &str,
    ) -> Result<Vec<ConversationMessage>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, spec_id, role, content, created_at
                   FROM conversation_messages
                   WHERE spec_id = ?1
                   ORDER BY created_at ASC"#,
            )?;

            let messages = stmt
                .query_map([spec_id], |row| {
                    let role_str: String = row.get(2)?;
                    Ok(ConversationMessage {
                        id: row.get(0)?,
                        spec_id: row.get(1)?,
                        role: ConversationRole::parse(&role_str).unwrap_or(ConversationRole::User),
                        content: row.get(3)?,
                        created_at: parse_datetime(row.get(4)?),
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(messages)
        })
    }

    pub fn get_conversation_message(&self, id: &str) -> Result<ConversationMessage, DbError> {
        self.with_conn(|conn| {
            conn.query_row(
                r#"SELECT id, spec_id, role, content, created_at
                   FROM conversation_messages
                   WHERE id = ?1"#,
                [id],
                |row| {
                    let role_str: String = row.get(2)?;
                    Ok(ConversationMessage {
                        id: row.get(0)?,
                        spec_id: row.get(1)?,
                        role: ConversationRole::parse(&role_str).unwrap_or(ConversationRole::User),
                        content: row.get(3)?,
                        created_at: parse_datetime(row.get(4)?),
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    DbError::NotFound(format!("Conversation message not found: {}", id))
                }
                _ => DbError::Sqlite(e),
            })
        })
    }

    pub fn delete_conversation_messages(&self, spec_id: &str) -> Result<usize, DbError> {
        self.with_conn(|conn| {
            let deleted = conn.execute(
                "DELETE FROM conversation_messages WHERE spec_id = ?1",
                [spec_id],
            )?;
            Ok(deleted)
        })
    }

    pub fn get_conversation_message_count(&self, spec_id: &str) -> Result<i32, DbError> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM conversation_messages WHERE spec_id = ?1",
                [spec_id],
                |row| row.get(0),
            )
            .map_err(DbError::Sqlite)
        })
    }

    pub fn get_last_conversation_message(
        &self,
        spec_id: &str,
    ) -> Result<Option<ConversationMessage>, DbError> {
        self.with_conn(|conn| {
            conn.query_row(
                r#"SELECT id, spec_id, role, content, created_at
                   FROM conversation_messages
                   WHERE spec_id = ?1
                   ORDER BY created_at DESC
                   LIMIT 1"#,
                [spec_id],
                |row| {
                    let role_str: String = row.get(2)?;
                    Ok(ConversationMessage {
                        id: row.get(0)?,
                        spec_id: row.get(1)?,
                        role: ConversationRole::parse(&role_str).unwrap_or(ConversationRole::User),
                        content: row.get(3)?,
                        created_at: parse_datetime(row.get(4)?),
                    })
                },
            )
            .optional()
            .map_err(DbError::Sqlite)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{CreateProject, CreateSpec};

    fn create_test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn temp_dir_path() -> String {
        std::env::temp_dir().to_string_lossy().to_string()
    }

    fn setup_spec(db: &Database) -> (String, String) {
        // Create a project first
        let project = db
            .create_project(&CreateProject {
                name: "Test Project".to_string(),
                path: temp_dir_path(),
                preferred_agent: None,
                requires_git: true,
            })
            .unwrap();

        // Create a board
        let board = db.create_board("Test Board").unwrap();

        // Create a spec
        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: None,
                project_id: project.id.clone(),
                name: "Test Spec".to_string(),
                user_input: "Build a feature".to_string(),
                agent_pref: None,
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        (spec.id, board.id)
    }

    #[test]
    fn create_and_get_message() {
        let db = create_test_db();
        let (spec_id, _) = setup_spec(&db);

        let msg = db
            .create_conversation_message(&CreateConversationMessage {
                spec_id: spec_id.clone(),
                role: ConversationRole::User,
                content: "I want to build a chat feature".to_string(),
            })
            .unwrap();

        assert_eq!(msg.spec_id, spec_id);
        assert_eq!(msg.role, ConversationRole::User);
        assert_eq!(msg.content, "I want to build a chat feature");

        let fetched = db.get_conversation_message(&msg.id).unwrap();
        assert_eq!(fetched.id, msg.id);
        assert_eq!(fetched.content, msg.content);
    }

    #[test]
    fn get_messages_ordered() {
        let db = create_test_db();
        let (spec_id, _) = setup_spec(&db);

        db.create_conversation_message(&CreateConversationMessage {
            spec_id: spec_id.clone(),
            role: ConversationRole::User,
            content: "First message".to_string(),
        })
        .unwrap();

        db.create_conversation_message(&CreateConversationMessage {
            spec_id: spec_id.clone(),
            role: ConversationRole::Assistant,
            content: "Second message".to_string(),
        })
        .unwrap();

        db.create_conversation_message(&CreateConversationMessage {
            spec_id: spec_id.clone(),
            role: ConversationRole::User,
            content: "Third message".to_string(),
        })
        .unwrap();

        let messages = db.get_conversation_messages(&spec_id).unwrap();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].content, "First message");
        assert_eq!(messages[1].content, "Second message");
        assert_eq!(messages[2].content, "Third message");
    }

    #[test]
    fn delete_messages() {
        let db = create_test_db();
        let (spec_id, _) = setup_spec(&db);

        db.create_conversation_message(&CreateConversationMessage {
            spec_id: spec_id.clone(),
            role: ConversationRole::User,
            content: "Test".to_string(),
        })
        .unwrap();

        db.create_conversation_message(&CreateConversationMessage {
            spec_id: spec_id.clone(),
            role: ConversationRole::Assistant,
            content: "Response".to_string(),
        })
        .unwrap();

        let count = db.delete_conversation_messages(&spec_id).unwrap();
        assert_eq!(count, 2);

        let messages = db.get_conversation_messages(&spec_id).unwrap();
        assert!(messages.is_empty());
    }

    #[test]
    fn get_message_count() {
        let db = create_test_db();
        let (spec_id, _) = setup_spec(&db);

        assert_eq!(db.get_conversation_message_count(&spec_id).unwrap(), 0);

        db.create_conversation_message(&CreateConversationMessage {
            spec_id: spec_id.clone(),
            role: ConversationRole::User,
            content: "Test".to_string(),
        })
        .unwrap();

        assert_eq!(db.get_conversation_message_count(&spec_id).unwrap(), 1);
    }

    #[test]
    fn get_last_message() {
        let db = create_test_db();
        let (spec_id, _) = setup_spec(&db);

        assert!(db.get_last_conversation_message(&spec_id).unwrap().is_none());

        db.create_conversation_message(&CreateConversationMessage {
            spec_id: spec_id.clone(),
            role: ConversationRole::User,
            content: "First".to_string(),
        })
        .unwrap();

        db.create_conversation_message(&CreateConversationMessage {
            spec_id: spec_id.clone(),
            role: ConversationRole::Assistant,
            content: "Last".to_string(),
        })
        .unwrap();

        let last = db.get_last_conversation_message(&spec_id).unwrap().unwrap();
        assert_eq!(last.content, "Last");
        assert_eq!(last.role, ConversationRole::Assistant);
    }

    #[test]
    fn get_message_not_found_returns_error() {
        let db = create_test_db();
        let result = db.get_conversation_message("nonexistent-id");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, super::DbError::NotFound(_)));
    }

    #[test]
    fn get_messages_empty_spec_returns_empty_vec() {
        let db = create_test_db();
        let (spec_id, _) = setup_spec(&db);
        let messages = db.get_conversation_messages(&spec_id).unwrap();
        assert!(messages.is_empty());
    }

    #[test]
    fn create_message_with_system_role() {
        let db = create_test_db();
        let (spec_id, _) = setup_spec(&db);

        let msg = db
            .create_conversation_message(&CreateConversationMessage {
                spec_id: spec_id.clone(),
                role: ConversationRole::System,
                content: "Starting brainstorming session...".to_string(),
            })
            .unwrap();

        assert_eq!(msg.role, ConversationRole::System);
        let fetched = db.get_conversation_message(&msg.id).unwrap();
        assert_eq!(fetched.role, ConversationRole::System);
    }

    #[test]
    fn delete_messages_returns_zero_for_empty_conversation() {
        let db = create_test_db();
        let (spec_id, _) = setup_spec(&db);
        let count = db.delete_conversation_messages(&spec_id).unwrap();
        assert_eq!(count, 0);
    }
}
