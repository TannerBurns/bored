pub mod schema;
pub mod models;
mod projects;
mod boards;
mod tickets;
mod runs;
mod events;
mod comments;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use rusqlite::Connection;
use thiserror::Error;

pub use models::*;
use schema::{CREATE_TABLES, SCHEMA_VERSION};

#[derive(Error, Debug)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    
    #[error("Lock error: {0}")]
    Lock(String),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Validation error: {0}")]
    Validation(String),
}

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn open(db_path: PathBuf) -> Result<Self, DbError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|_e| DbError::Validation(format!("Failed to create directory: {:?}", parent)))?;
        }

        let conn = Connection::open(&db_path)?;
        conn.execute("PRAGMA foreign_keys = ON", [])?;
        let _: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
        
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.migrate()?;
        
        tracing::info!("Database opened at {:?}", db_path);
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        conn.execute("PRAGMA foreign_keys = ON", [])?;
        
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), DbError> {
        let conn = self.conn.lock()
            .map_err(|e| DbError::Lock(e.to_string()))?;

        let current_version: i32 = conn
            .query_row(
                "SELECT version FROM schema_version ORDER BY version DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if current_version < SCHEMA_VERSION {
            tracing::info!(
                "Migrating database from version {} to {}",
                current_version,
                SCHEMA_VERSION
            );

            // For fresh databases (version 0), create all tables
            if current_version == 0 {
                conn.execute_batch(CREATE_TABLES)?;
            }
            
            // Apply incremental migrations
            if current_version < 3 && current_version > 0 {
                tracing::info!("Applying migration v3: repo_locks and requires_git");
                // Split migration to handle potential errors gracefully
                // Add requires_git column if it doesn't exist
                let _ = conn.execute(
                    "ALTER TABLE projects ADD COLUMN requires_git INTEGER NOT NULL DEFAULT 1",
                    [],
                );
                // Create repo_locks table
                conn.execute_batch(
                    r#"
                    CREATE TABLE IF NOT EXISTS repo_locks (
                        project_id TEXT PRIMARY KEY NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                        locked_by_run_id TEXT NOT NULL,
                        lock_expires_at TEXT NOT NULL,
                        locked_at TEXT NOT NULL
                    );
                    CREATE INDEX IF NOT EXISTS idx_repo_locks_expires ON repo_locks(lock_expires_at);
                    "#
                )?;
            }
            
            if current_version < 4 && current_version > 0 {
                tracing::info!("Applying migration v4: workflow_type, parent_run_id, stage");
                // Add columns one at a time to handle potential errors gracefully
                let _ = conn.execute(
                    "ALTER TABLE tickets ADD COLUMN workflow_type TEXT NOT NULL DEFAULT 'basic'",
                    [],
                );
                let _ = conn.execute(
                    "ALTER TABLE agent_runs ADD COLUMN parent_run_id TEXT REFERENCES agent_runs(id) ON DELETE CASCADE",
                    [],
                );
                let _ = conn.execute(
                    "ALTER TABLE agent_runs ADD COLUMN stage TEXT",
                    [],
                );
                let _ = conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_runs_parent ON agent_runs(parent_run_id) WHERE parent_run_id IS NOT NULL",
                    [],
                );
            }
            
            if current_version < 5 && current_version > 0 {
                tracing::info!("Applying migration v5: model column for tickets");
                let _ = conn.execute(
                    "ALTER TABLE tickets ADD COLUMN model TEXT",
                    [],
                );
            }
            
            conn.execute(
                "INSERT OR REPLACE INTO schema_version (version) VALUES (?)",
                [SCHEMA_VERSION],
            )?;

            tracing::info!("Database migration complete");
        }

        Ok(())
    }

    pub fn with_conn<F, T>(&self, f: F) -> Result<T, DbError>
    where
        F: FnOnce(&Connection) -> Result<T, DbError>,
    {
        let conn = self.conn.lock()
            .map_err(|e| DbError::Lock(e.to_string()))?;
        f(&conn)
    }

    pub fn with_conn_mut<F, T>(&self, f: F) -> Result<T, DbError>
    where
        F: FnOnce(&mut Connection) -> Result<T, DbError>,
    {
        let mut conn = self.conn.lock()
            .map_err(|e| DbError::Lock(e.to_string()))?;
        f(&mut conn)
    }
}

pub(crate) fn parse_datetime(s: String) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(&s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunArtifacts {
    pub commit_hash: Option<String>,
    pub files_changed: Vec<String>,
    pub diff_path: Option<String>,
    pub transcript_path: Option<String>,
    pub log_path: Option<String>,
}

impl Database {
    /// Attempt to acquire a repository-level lock.
    /// 
    /// Returns true if the lock was acquired, false if another worker holds a valid lock.
    /// Uses INSERT...ON CONFLICT to atomically acquire or fail.
    pub fn acquire_repo_lock(
        &self,
        project_id: &str,
        run_id: &str,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<bool, DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            let expires_str = expires_at.to_rfc3339();
            let affected = conn.execute(
                r#"INSERT INTO repo_locks (project_id, locked_by_run_id, lock_expires_at, locked_at)
                   VALUES (?1, ?2, ?3, ?4)
                   ON CONFLICT(project_id) DO UPDATE 
                   SET locked_by_run_id = ?2, lock_expires_at = ?3, locked_at = ?4
                   WHERE lock_expires_at < ?4"#,
                rusqlite::params![project_id, run_id, expires_str, now],
            )?;
            
            Ok(affected > 0)
        })
    }
    
    /// Release a repository-level lock.
    /// Only releases if the lock is held by the specified run_id.
    pub fn release_repo_lock(&self, project_id: &str, run_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            conn.execute(
                "DELETE FROM repo_locks WHERE project_id = ? AND locked_by_run_id = ?",
                rusqlite::params![project_id, run_id],
            )?;
            Ok(())
        })
    }
    
    /// Update the run_id that owns a repository lock.
    /// Used when a temporary run_id is replaced with the actual run ID after creation.
    /// Only updates if the lock is currently held by old_run_id.
    pub fn update_repo_lock_owner(
        &self,
        project_id: &str,
        old_run_id: &str,
        new_run_id: &str,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let affected = conn.execute(
                "UPDATE repo_locks SET locked_by_run_id = ? WHERE project_id = ? AND locked_by_run_id = ?",
                rusqlite::params![new_run_id, project_id, old_run_id],
            )?;
            
            if affected == 0 {
                return Err(DbError::NotFound("Repo lock not found or not owned by this run".to_string()));
            }
            Ok(())
        })
    }
    
    /// Extend an existing repository lock.
    /// Only extends if the lock is held by the specified run_id.
    pub fn extend_repo_lock(
        &self,
        project_id: &str,
        run_id: &str,
        new_expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let affected = conn.execute(
                "UPDATE repo_locks SET lock_expires_at = ? WHERE project_id = ? AND locked_by_run_id = ?",
                rusqlite::params![new_expires_at.to_rfc3339(), project_id, run_id],
            )?;
            
            if affected == 0 {
                return Err(DbError::NotFound("Repo lock not found or not owned by this run".to_string()));
            }
            Ok(())
        })
    }
    
    /// Clean up expired repository locks.
    /// Returns the number of locks that were cleaned up.
    pub fn cleanup_expired_repo_locks(&self) -> Result<usize, DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            let affected = conn.execute(
                "DELETE FROM repo_locks WHERE lock_expires_at < ?",
                [&now],
            )?;
            Ok(affected)
        })
    }

    pub fn update_run_artifacts(&self, run_id: &str, artifacts: &RunArtifacts) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let metadata = serde_json::to_string(artifacts).unwrap_or_else(|_| "{}".to_string());
            conn.execute(
                "UPDATE agent_runs SET metadata_json = ? WHERE id = ?",
                rusqlite::params![metadata, run_id],
            )?;
            Ok(())
        })
    }

    pub fn get_run_artifacts(&self, run_id: &str) -> Result<Option<RunArtifacts>, DbError> {
        self.with_conn(|conn| {
            let metadata: Option<String> = conn.query_row(
                "SELECT metadata_json FROM agent_runs WHERE id = ?",
                [run_id],
                |row| row.get(0),
            ).ok();
            Ok(metadata.and_then(|m| serde_json::from_str(&m).ok()))
        })
    }

    pub fn release_lock(&self, ticket_id: &str, run_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE tickets SET locked_by_run_id = NULL, lock_expires_at = NULL 
                 WHERE id = ? AND locked_by_run_id = ?",
                rusqlite::params![ticket_id, run_id],
            )?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    fn create_test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn temp_dir_path() -> String {
        std::env::temp_dir().to_string_lossy().to_string()
    }

    mod repo_lock_tests {
        use super::*;
        use crate::db::models::CreateProject;

        fn setup_project(db: &Database) -> String {
            let project = db.create_project(&CreateProject {
                name: "Test".to_string(),
                path: temp_dir_path(),
                preferred_agent: None,
                requires_git: true,
            }).unwrap();
            project.id
        }

        #[test]
        fn acquire_repo_lock_success() {
            let db = create_test_db();
            let project_id = setup_project(&db);
            
            let expires = Utc::now() + Duration::minutes(30);
            let acquired = db.acquire_repo_lock(&project_id, "run-1", expires).unwrap();
            
            assert!(acquired);
        }

        #[test]
        fn acquire_repo_lock_fails_when_held() {
            let db = create_test_db();
            let project_id = setup_project(&db);
            
            let expires = Utc::now() + Duration::minutes(30);
            
            // First acquisition should succeed
            let first = db.acquire_repo_lock(&project_id, "run-1", expires).unwrap();
            assert!(first);
            
            // Second acquisition should fail (lock not expired)
            let second = db.acquire_repo_lock(&project_id, "run-2", expires).unwrap();
            assert!(!second);
        }

        #[test]
        fn acquire_repo_lock_succeeds_when_expired() {
            let db = create_test_db();
            let project_id = setup_project(&db);
            
            // Acquire with expired lock
            let expired = Utc::now() - Duration::minutes(5);
            db.acquire_repo_lock(&project_id, "run-1", expired).unwrap();
            
            // New acquisition should succeed since lock is expired
            let new_expires = Utc::now() + Duration::minutes(30);
            let acquired = db.acquire_repo_lock(&project_id, "run-2", new_expires).unwrap();
            
            assert!(acquired);
        }

        #[test]
        fn release_repo_lock_success() {
            let db = create_test_db();
            let project_id = setup_project(&db);
            
            let expires = Utc::now() + Duration::minutes(30);
            db.acquire_repo_lock(&project_id, "run-1", expires).unwrap();
            
            // Release the lock
            db.release_repo_lock(&project_id, "run-1").unwrap();
            
            // Now another run should be able to acquire
            let acquired = db.acquire_repo_lock(&project_id, "run-2", expires).unwrap();
            assert!(acquired);
        }

        #[test]
        fn release_repo_lock_wrong_run_no_effect() {
            let db = create_test_db();
            let project_id = setup_project(&db);
            
            let expires = Utc::now() + Duration::minutes(30);
            db.acquire_repo_lock(&project_id, "run-1", expires).unwrap();
            
            // Try to release with wrong run_id
            db.release_repo_lock(&project_id, "run-wrong").unwrap();
            
            // Lock should still be held, so new acquisition should fail
            let acquired = db.acquire_repo_lock(&project_id, "run-2", expires).unwrap();
            assert!(!acquired);
        }

        #[test]
        fn extend_repo_lock_success() {
            let db = create_test_db();
            let project_id = setup_project(&db);
            
            let initial_expires = Utc::now() + Duration::minutes(30);
            db.acquire_repo_lock(&project_id, "run-1", initial_expires).unwrap();
            
            let new_expires = Utc::now() + Duration::minutes(60);
            let result = db.extend_repo_lock(&project_id, "run-1", new_expires);
            
            assert!(result.is_ok());
        }

        #[test]
        fn extend_repo_lock_wrong_run_fails() {
            let db = create_test_db();
            let project_id = setup_project(&db);
            
            let expires = Utc::now() + Duration::minutes(30);
            db.acquire_repo_lock(&project_id, "run-1", expires).unwrap();
            
            // Try to extend with wrong run_id
            let result = db.extend_repo_lock(&project_id, "run-wrong", expires);
            
            assert!(matches!(result, Err(DbError::NotFound(_))));
        }

        #[test]
        fn update_repo_lock_owner_success() {
            let db = create_test_db();
            let project_id = setup_project(&db);
            
            let expires = Utc::now() + Duration::minutes(30);
            db.acquire_repo_lock(&project_id, "temp-run-id", expires).unwrap();
            
            // Update owner from temp to actual run id
            let result = db.update_repo_lock_owner(&project_id, "temp-run-id", "actual-run-id");
            assert!(result.is_ok());
            
            // Now extend should work with new run id
            let new_expires = Utc::now() + Duration::minutes(60);
            let extend_result = db.extend_repo_lock(&project_id, "actual-run-id", new_expires);
            assert!(extend_result.is_ok());
            
            // And release should work with new run id
            db.release_repo_lock(&project_id, "actual-run-id").unwrap();
            
            // Lock should now be released
            let acquired = db.acquire_repo_lock(&project_id, "run-3", expires).unwrap();
            assert!(acquired);
        }

        #[test]
        fn update_repo_lock_owner_wrong_old_id_fails() {
            let db = create_test_db();
            let project_id = setup_project(&db);
            
            let expires = Utc::now() + Duration::minutes(30);
            db.acquire_repo_lock(&project_id, "run-1", expires).unwrap();
            
            // Try to update with wrong old_run_id
            let result = db.update_repo_lock_owner(&project_id, "wrong-id", "new-id");
            
            assert!(matches!(result, Err(DbError::NotFound(_))));
        }

        #[test]
        fn cleanup_expired_repo_locks() {
            let db = create_test_db();
            let project_id = setup_project(&db);
            
            // Create expired lock
            let expired = Utc::now() - Duration::minutes(5);
            db.acquire_repo_lock(&project_id, "run-1", expired).unwrap();
            
            // Cleanup should remove it
            let count = db.cleanup_expired_repo_locks().unwrap();
            assert_eq!(count, 1);
            
            // Now new acquisition should succeed
            let new_expires = Utc::now() + Duration::minutes(30);
            let acquired = db.acquire_repo_lock(&project_id, "run-2", new_expires).unwrap();
            assert!(acquired);
        }

        #[test]
        fn cleanup_does_not_remove_valid_locks() {
            let db = create_test_db();
            let project_id = setup_project(&db);
            
            // Create valid lock
            let expires = Utc::now() + Duration::minutes(30);
            db.acquire_repo_lock(&project_id, "run-1", expires).unwrap();
            
            // Cleanup should not remove it
            let count = db.cleanup_expired_repo_locks().unwrap();
            assert_eq!(count, 0);
            
            // Lock should still be held
            let acquired = db.acquire_repo_lock(&project_id, "run-2", expires).unwrap();
            assert!(!acquired);
        }
    }
}
