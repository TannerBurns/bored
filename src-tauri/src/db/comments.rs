use crate::db::{Database, DbError, parse_datetime};
use crate::db::models::{Comment, CreateComment, AuthorType};

impl Database {
    pub fn create_comment(&self, comment: &CreateComment) -> Result<Comment, DbError> {
        self.with_conn(|conn| {
            let comment_id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now();
            let metadata_json = comment.metadata
                .as_ref()
                .and_then(|m| serde_json::to_string(m).ok());
            
            conn.execute(
                r#"INSERT INTO comments 
                   (id, ticket_id, author_type, body_md, created_at, metadata_json)
                   VALUES (?, ?, ?, ?, ?, ?)"#,
                rusqlite::params![
                    comment_id,
                    comment.ticket_id,
                    comment.author_type.as_str(),
                    comment.body_md,
                    now.to_rfc3339(),
                    metadata_json,
                ],
            )?;

            Ok(Comment {
                id: comment_id,
                ticket_id: comment.ticket_id.clone(),
                author_type: comment.author_type.clone(),
                body_md: comment.body_md.clone(),
                created_at: now,
                metadata: comment.metadata.clone(),
            })
        })
    }

    pub fn get_comments(&self, ticket_id: &str) -> Result<Vec<Comment>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, ticket_id, author_type, body_md, created_at, metadata_json
                   FROM comments WHERE ticket_id = ? ORDER BY created_at"#
            )?;
            
            let comments = stmt.query_map([ticket_id], |row| {
                let author_type_str: String = row.get(2)?;
                let metadata_json: Option<String> = row.get(5)?;
                
                Ok(Comment {
                    id: row.get(0)?,
                    ticket_id: row.get(1)?,
                    author_type: match author_type_str.as_str() {
                        "user" => AuthorType::User,
                        "system" => AuthorType::System,
                        _ => AuthorType::Agent,
                    },
                    body_md: row.get(3)?,
                    created_at: parse_datetime(row.get(4)?),
                    metadata: metadata_json.and_then(|s| serde_json::from_str(&s).ok()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
            
            Ok(comments)
        })
    }

    pub fn update_comment(&self, comment_id: &str, body_md: &str) -> Result<Comment, DbError> {
        self.with_conn(|conn| {
            // First check if comment exists
            let mut stmt = conn.prepare(
                r#"SELECT id, ticket_id, author_type, body_md, created_at, metadata_json
                   FROM comments WHERE id = ?"#
            )?;
            
            let comment = stmt.query_row([comment_id], |row| {
                let author_type_str: String = row.get(2)?;
                let metadata_json: Option<String> = row.get(5)?;
                
                Ok(Comment {
                    id: row.get(0)?,
                    ticket_id: row.get(1)?,
                    author_type: match author_type_str.as_str() {
                        "user" => AuthorType::User,
                        "system" => AuthorType::System,
                        _ => AuthorType::Agent,
                    },
                    body_md: row.get(3)?,
                    created_at: parse_datetime(row.get(4)?),
                    metadata: metadata_json.and_then(|s| serde_json::from_str(&s).ok()),
                })
            }).map_err(|_| DbError::NotFound(format!("Comment {} not found", comment_id)))?;

            // Update the comment body
            conn.execute(
                "UPDATE comments SET body_md = ? WHERE id = ?",
                rusqlite::params![body_md, comment_id],
            )?;

            Ok(Comment {
                body_md: body_md.to_string(),
                ..comment
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{CreateTicket, Priority, WorkflowType};

    fn create_test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn create_and_get_comments() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Low,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
        }).unwrap();
        
        let comment = db.create_comment(&CreateComment {
            ticket_id: ticket.id.clone(),
            author_type: AuthorType::Agent,
            body_md: "Test comment".to_string(),
            metadata: Some(serde_json::json!({"key": "value"})),
        }).unwrap();
        
        assert_eq!(comment.author_type, AuthorType::Agent);
        assert_eq!(comment.body_md, "Test comment");
        
        let comments = db.get_comments(&ticket.id).unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].id, comment.id);
    }

    #[test]
    fn update_comment_success() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Low,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
        }).unwrap();
        
        let comment = db.create_comment(&CreateComment {
            ticket_id: ticket.id.clone(),
            author_type: AuthorType::User,
            body_md: "Original comment".to_string(),
            metadata: None,
        }).unwrap();
        
        let updated = db.update_comment(&comment.id, "Updated comment").unwrap();
        
        assert_eq!(updated.id, comment.id);
        assert_eq!(updated.body_md, "Updated comment");
        assert_eq!(updated.ticket_id, ticket.id);
        assert_eq!(updated.author_type, AuthorType::User);
    }

    #[test]
    fn update_comment_not_found() {
        let db = create_test_db();
        let result = db.update_comment("nonexistent-id", "New body");
        assert!(matches!(result, Err(DbError::NotFound(_))));
    }

    #[test]
    fn update_comment_preserves_metadata() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Low,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
        }).unwrap();
        
        let metadata = serde_json::json!({"stage": "plan", "key": "value"});
        let comment = db.create_comment(&CreateComment {
            ticket_id: ticket.id.clone(),
            author_type: AuthorType::Agent,
            body_md: "Original".to_string(),
            metadata: Some(metadata.clone()),
        }).unwrap();
        
        let updated = db.update_comment(&comment.id, "Updated body").unwrap();
        
        // The update only changes body_md, metadata should be preserved from fetch
        assert_eq!(updated.body_md, "Updated body");
        assert_eq!(updated.metadata, Some(metadata));
    }
}
