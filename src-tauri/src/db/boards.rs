use crate::db::{Database, DbError, parse_datetime};
use crate::db::models::{Board, Column};
use crate::db::schema::DEFAULT_COLUMNS;

impl Database {
    pub fn create_board(&self, name: &str) -> Result<Board, DbError> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            
            let board_id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now();
            
            tx.execute(
                "INSERT INTO boards (id, name, created_at, updated_at) VALUES (?, ?, ?, ?)",
                rusqlite::params![board_id, name, now.to_rfc3339(), now.to_rfc3339()],
            )?;

            for (position, col_name) in DEFAULT_COLUMNS.iter().enumerate() {
                let col_id = uuid::Uuid::new_v4().to_string();
                tx.execute(
                    "INSERT INTO columns (id, board_id, name, position) VALUES (?, ?, ?, ?)",
                    rusqlite::params![col_id, board_id, col_name, position as i32],
                )?;
            }

            tx.commit()?;

            Ok(Board {
                id: board_id,
                name: name.to_string(),
                default_project_id: None,
                created_at: now,
                updated_at: now,
            })
        })
    }

    pub fn get_boards(&self) -> Result<Vec<Board>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, default_project_id, created_at, updated_at FROM boards ORDER BY created_at DESC"
            )?;
            
            let boards = stmt.query_map([], |row| {
                Ok(Board {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    default_project_id: row.get(2)?,
                    created_at: parse_datetime(row.get(3)?),
                    updated_at: parse_datetime(row.get(4)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
            
            Ok(boards)
        })
    }

    pub fn get_board(&self, board_id: &str) -> Result<Option<Board>, DbError> {
        self.get_boards().map(|boards| {
            boards.into_iter().find(|b| b.id == board_id)
        })
    }

    pub fn get_columns(&self, board_id: &str) -> Result<Vec<Column>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, board_id, name, position, wip_limit 
                 FROM columns WHERE board_id = ? ORDER BY position"
            )?;
            
            let columns = stmt.query_map([board_id], |row| {
                Ok(Column {
                    id: row.get(0)?,
                    board_id: row.get(1)?,
                    name: row.get(2)?,
                    position: row.get(3)?,
                    wip_limit: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
            
            Ok(columns)
        })
    }

    pub fn update_board(&self, board_id: &str, name: &str) -> Result<Board, DbError> {
        self.with_conn_mut(|conn| {
            let now = chrono::Utc::now();
            
            let affected = conn.execute(
                "UPDATE boards SET name = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![name, now.to_rfc3339(), board_id],
            )?;
            
            if affected == 0 {
                return Err(DbError::NotFound(format!("Board {}", board_id)));
            }
            
            let mut stmt = conn.prepare(
                "SELECT id, name, default_project_id, created_at, updated_at FROM boards WHERE id = ?"
            )?;
            
            let board = stmt.query_row([board_id], |row| {
                Ok(Board {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    default_project_id: row.get(2)?,
                    created_at: parse_datetime(row.get(3)?),
                    updated_at: parse_datetime(row.get(4)?),
                })
            })?;
            
            Ok(board)
        })
    }

    pub fn delete_board(&self, board_id: &str) -> Result<(), DbError> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM boards WHERE id = ?)",
                [board_id],
                |row| row.get(0),
            )?;
            
            if !exists {
                return Err(DbError::NotFound(format!("Board {}", board_id)));
            }
            
            tx.execute("DELETE FROM tickets WHERE board_id = ?", [board_id])?;
            tx.execute("DELETE FROM columns WHERE board_id = ?", [board_id])?;
            tx.execute("DELETE FROM boards WHERE id = ?", [board_id])?;
            tx.commit()?;
            
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn create_board_with_default_columns() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        
        assert_eq!(board.name, "Test Board");
        assert!(board.default_project_id.is_none());
        
        let columns = db.get_columns(&board.id).unwrap();
        assert_eq!(columns.len(), 6);
        assert_eq!(columns[0].name, "Backlog");
        assert_eq!(columns[5].name, "Done");
    }

    #[test]
    fn get_boards_returns_all() {
        let db = create_test_db();
        db.create_board("Board 1").unwrap();
        db.create_board("Board 2").unwrap();
        
        let boards = db.get_boards().unwrap();
        assert_eq!(boards.len(), 2);
    }

    #[test]
    fn get_board_by_id() {
        let db = create_test_db();
        let board = db.create_board("Test").unwrap();
        
        let found = db.get_board(&board.id).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Test");
        
        let not_found = db.get_board("nonexistent").unwrap();
        assert!(not_found.is_none());
    }

    #[test]
    fn update_board_name() {
        let db = create_test_db();
        let board = db.create_board("Original Name").unwrap();
        
        let updated = db.update_board(&board.id, "New Name").unwrap();
        assert_eq!(updated.name, "New Name");
        assert_eq!(updated.id, board.id);
        
        // Verify persistence
        let fetched = db.get_board(&board.id).unwrap().unwrap();
        assert_eq!(fetched.name, "New Name");
    }

    #[test]
    fn update_nonexistent_board_fails() {
        let db = create_test_db();
        
        let result = db.update_board("nonexistent", "New Name");
        assert!(result.is_err());
    }

    #[test]
    fn delete_board_removes_board_and_columns() {
        let db = create_test_db();
        let board = db.create_board("To Delete").unwrap();
        
        // Verify columns exist
        let columns = db.get_columns(&board.id).unwrap();
        assert_eq!(columns.len(), 6);
        
        // Delete the board
        db.delete_board(&board.id).unwrap();
        
        // Verify board is gone
        let found = db.get_board(&board.id).unwrap();
        assert!(found.is_none());
        
        // Verify columns are gone
        let columns = db.get_columns(&board.id).unwrap();
        assert_eq!(columns.len(), 0);
    }

    #[test]
    fn delete_nonexistent_board_fails() {
        let db = create_test_db();
        
        let result = db.delete_board("nonexistent");
        assert!(result.is_err());
    }
}
