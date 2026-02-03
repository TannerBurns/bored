mod boards;
mod comments;
mod conversations;
mod events;
pub mod models;
mod projects;
mod runs;
pub mod schema;
mod specs;
pub mod tasks;
mod tickets;

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use thiserror::Error;

pub use models::*;
use schema::{CREATE_TABLES, SCHEMA_VERSION};
pub use tickets::ReadyTicketDiagnostics;

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
            std::fs::create_dir_all(parent).map_err(|_e| {
                DbError::Validation(format!("Failed to create directory: {:?}", parent))
            })?;
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
        let conn = self.conn.lock().map_err(|e| DbError::Lock(e.to_string()))?;

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

            // Migration from version 1 to 2: Add conversation_messages table and 'conversing' status
            if current_version < 2 {
                tracing::info!("Running migration to version 2: conversation_messages table");
                
                // Create conversation_messages table
                conn.execute_batch(
                    r#"
                    CREATE TABLE IF NOT EXISTS conversation_messages (
                        id TEXT PRIMARY KEY NOT NULL,
                        spec_id TEXT NOT NULL REFERENCES specs(id) ON DELETE CASCADE,
                        role TEXT NOT NULL CHECK(role IN ('user', 'assistant', 'system')),
                        content TEXT NOT NULL,
                        created_at TEXT NOT NULL DEFAULT (datetime('now'))
                    );

                    CREATE INDEX IF NOT EXISTS idx_conversation_messages_spec ON conversation_messages(spec_id);
                    CREATE INDEX IF NOT EXISTS idx_conversation_messages_created ON conversation_messages(spec_id, created_at);
                    "#
                )?;
                
                // Recreate specs table with 'conversing' status in CHECK constraint
                // First check if we need to migrate (if table exists and doesn't have conversing status)
                let specs_exists: bool = conn
                    .query_row(
                        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='specs'",
                        [],
                        |row| row.get::<_, i32>(0),
                    )
                    .unwrap_or(0) > 0;
                
                if specs_exists {
                    conn.execute_batch(
                        r#"
                        CREATE TABLE specs_v2 (
                            id TEXT PRIMARY KEY NOT NULL,
                            board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
                            target_board_id TEXT REFERENCES boards(id) ON DELETE SET NULL,
                            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                            name TEXT NOT NULL,
                            user_input TEXT NOT NULL,
                            status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft', 'conversing', 'exploring', 'planning', 'awaiting_approval', 'approved', 'executing', 'executed', 'working', 'paused', 'halted', 'completed', 'failed')),
                            agent_pref TEXT CHECK(agent_pref IS NULL OR agent_pref IN ('cursor', 'claude', 'any')),
                            model TEXT,
                            exploration_log TEXT,
                            plan_markdown TEXT,
                            plan_json TEXT,
                            settings_json TEXT NOT NULL DEFAULT '{}',
                            work_started_at TEXT,
                            created_at TEXT NOT NULL DEFAULT (datetime('now')),
                            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                        );

                        INSERT INTO specs_v2 SELECT * FROM specs;
                        DROP TABLE specs;
                        ALTER TABLE specs_v2 RENAME TO specs;

                        CREATE INDEX IF NOT EXISTS idx_specs_board ON specs(board_id);
                        CREATE INDEX IF NOT EXISTS idx_specs_target_board ON specs(target_board_id);
                        CREATE INDEX IF NOT EXISTS idx_specs_project ON specs(project_id);
                        CREATE INDEX IF NOT EXISTS idx_specs_status ON specs(status);
                        "#
                    )?;
                }
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
        let conn = self.conn.lock().map_err(|e| DbError::Lock(e.to_string()))?;
        f(&conn)
    }

    pub fn with_conn_mut<F, T>(&self, f: F) -> Result<T, DbError>
    where
        F: FnOnce(&mut Connection) -> Result<T, DbError>,
    {
        let mut conn = self.conn.lock().map_err(|e| DbError::Lock(e.to_string()))?;
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
            let affected =
                conn.execute("DELETE FROM repo_locks WHERE lock_expires_at < ?", [&now])?;
            Ok(affected)
        })
    }

    pub fn update_run_artifacts(
        &self,
        run_id: &str,
        artifacts: &RunArtifacts,
    ) -> Result<(), DbError> {
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
            let metadata: Option<String> = conn
                .query_row(
                    "SELECT metadata_json FROM agent_runs WHERE id = ?",
                    [run_id],
                    |row| row.get(0),
                )
                .ok();
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

    /// Repair the specs table CHECK constraint.
    /// This recreates the table with the correct constraint including 'executed' and 'working' status values.
    pub fn repair_specs_constraint(&self) -> Result<String, DbError> {
        self.with_conn(|conn| {
            tracing::warn!("Repairing specs table CHECK constraint");
            
            // Check if the table exists
            let table_exists: bool = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='specs'",
                    [],
                    |row| row.get::<_, i32>(0),
                )
                .unwrap_or(0) > 0;
            
            if !table_exists {
                return Ok("Specs table does not exist, nothing to repair".to_string());
            }
            
            // Get current row count
            let row_count: i32 = conn
                .query_row("SELECT COUNT(*) FROM specs", [], |row| row.get(0))
                .unwrap_or(0);
            
            // Check if target_board_id column exists
            let has_target_board: bool = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('specs') WHERE name = 'target_board_id'",
                    [],
                    |row| row.get::<_, i32>(0),
                )
                .unwrap_or(0) > 0;
            
            tracing::info!("Specs table has {} rows, target_board_id={}", row_count, has_target_board);
            
            // Recreate the table with the correct constraint
            if has_target_board {
                conn.execute_batch(
                    r#"
                    CREATE TABLE specs_repaired (
                        id TEXT PRIMARY KEY NOT NULL,
                        board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
                        target_board_id TEXT REFERENCES boards(id) ON DELETE SET NULL,
                        project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                        name TEXT NOT NULL,
                        user_input TEXT NOT NULL,
                        status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft', 'exploring', 'planning', 'awaiting_approval', 'approved', 'executing', 'executed', 'working', 'completed', 'failed')),
                        agent_pref TEXT CHECK(agent_pref IS NULL OR agent_pref IN ('cursor', 'claude', 'any')),
                        model TEXT,
                        exploration_log TEXT,
                        plan_markdown TEXT,
                        plan_json TEXT,
                        settings_json TEXT NOT NULL DEFAULT '{}',
                        created_at TEXT NOT NULL DEFAULT (datetime('now')),
                        updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                    );

                    INSERT INTO specs_repaired (id, board_id, target_board_id, project_id, name, user_input, status, agent_pref, model, exploration_log, plan_markdown, plan_json, settings_json, created_at, updated_at)
                    SELECT id, board_id, target_board_id, project_id, name, user_input, status, agent_pref, model, exploration_log, plan_markdown, plan_json, settings_json, created_at, updated_at FROM specs;

                    DROP TABLE specs;
                    ALTER TABLE specs_repaired RENAME TO specs;

                    CREATE INDEX IF NOT EXISTS idx_specs_board ON specs(board_id);
                    CREATE INDEX IF NOT EXISTS idx_specs_target_board ON specs(target_board_id);
                    CREATE INDEX IF NOT EXISTS idx_specs_project ON specs(project_id);
                    CREATE INDEX IF NOT EXISTS idx_specs_status ON specs(status);
                    "#
                )?;
            } else {
                conn.execute_batch(
                    r#"
                    CREATE TABLE specs_repaired (
                        id TEXT PRIMARY KEY NOT NULL,
                        board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
                        target_board_id TEXT REFERENCES boards(id) ON DELETE SET NULL,
                        project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                        name TEXT NOT NULL,
                        user_input TEXT NOT NULL,
                        status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft', 'exploring', 'planning', 'awaiting_approval', 'approved', 'executing', 'executed', 'working', 'completed', 'failed')),
                        agent_pref TEXT CHECK(agent_pref IS NULL OR agent_pref IN ('cursor', 'claude', 'any')),
                        model TEXT,
                        exploration_log TEXT,
                        plan_markdown TEXT,
                        plan_json TEXT,
                        settings_json TEXT NOT NULL DEFAULT '{}',
                        created_at TEXT NOT NULL DEFAULT (datetime('now')),
                        updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                    );

                    INSERT INTO specs_repaired (id, board_id, project_id, name, user_input, status, agent_pref, model, exploration_log, plan_markdown, plan_json, settings_json, created_at, updated_at)
                    SELECT id, board_id, project_id, name, user_input, status, agent_pref, model, exploration_log, plan_markdown, plan_json, settings_json, created_at, updated_at FROM specs;

                    DROP TABLE specs;
                    ALTER TABLE specs_repaired RENAME TO specs;

                    CREATE INDEX IF NOT EXISTS idx_specs_board ON specs(board_id);
                    CREATE INDEX IF NOT EXISTS idx_specs_target_board ON specs(target_board_id);
                    CREATE INDEX IF NOT EXISTS idx_specs_project ON specs(project_id);
                    CREATE INDEX IF NOT EXISTS idx_specs_status ON specs(status);
                    "#
                )?;
            }
            
            tracing::info!("Specs table repaired successfully");
            Ok(format!("Repaired specs table with {} rows", row_count))
        })
    }

    /// Factory reset: delete all user data from the database.
    /// This clears all boards, tickets, projects, runs, specs, etc.
    /// and also repairs the specs table schema to ensure correct constraints.
    pub fn factory_reset(&self) -> Result<(), DbError> {
        self.with_conn(|conn| {
            tracing::warn!("Factory reset: deleting all user data from database");
            
            // Delete in dependency order to respect foreign key constraints
            // First: tables with no dependents or only CASCADE dependents
            conn.execute("DELETE FROM agent_events", [])?;
            conn.execute("DELETE FROM comments", [])?;
            conn.execute("DELETE FROM tasks", [])?;
            conn.execute("DELETE FROM agent_runs", [])?;
            conn.execute("DELETE FROM repo_locks", [])?;
            
            // Tickets must be deleted before specs (spec_id FK)
            // and before columns (column_id FK with RESTRICT)
            conn.execute("DELETE FROM tickets", [])?;
            
            // Now specs (depends on boards and projects)
            conn.execute("DELETE FROM specs", [])?;
            
            // Columns (depends on boards)
            conn.execute("DELETE FROM columns", [])?;
            
            // Boards (depends on projects via default_project_id)
            conn.execute("DELETE FROM boards", [])?;
            
            // Finally projects (root table)
            conn.execute("DELETE FROM projects", [])?;
            
            tracing::info!("Factory reset: all user data deleted");
            
            // Now recreate the specs table with the correct schema
            // This ensures the CHECK constraint is correct
            tracing::info!("Factory reset: recreating specs table with correct schema");
            
            // Drop and recreate specs table with correct constraint
            conn.execute("DROP TABLE IF EXISTS specs", [])?;
            conn.execute_batch(
                r#"
                CREATE TABLE specs (
                    id TEXT PRIMARY KEY NOT NULL,
                    board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
                    target_board_id TEXT REFERENCES boards(id) ON DELETE SET NULL,
                    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                    name TEXT NOT NULL,
                    user_input TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft', 'exploring', 'planning', 'awaiting_approval', 'approved', 'executing', 'executed', 'working', 'completed', 'failed')),
                    agent_pref TEXT CHECK(agent_pref IS NULL OR agent_pref IN ('cursor', 'claude', 'any')),
                    model TEXT,
                    exploration_log TEXT,
                    plan_markdown TEXT,
                    plan_json TEXT,
                    settings_json TEXT NOT NULL DEFAULT '{}',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );

                CREATE INDEX IF NOT EXISTS idx_specs_board ON specs(board_id);
                CREATE INDEX IF NOT EXISTS idx_specs_target_board ON specs(target_board_id);
                CREATE INDEX IF NOT EXISTS idx_specs_project ON specs(project_id);
                CREATE INDEX IF NOT EXISTS idx_specs_status ON specs(status);
                "#
            )?;
            
            tracing::info!("Factory reset complete: database ready for fresh start");
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
            let project = db
                .create_project(&CreateProject {
                    name: "Test".to_string(),
                    path: temp_dir_path(),
                    preferred_agent: None,
                    requires_git: true,
                })
                .unwrap();
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
            let acquired = db
                .acquire_repo_lock(&project_id, "run-2", new_expires)
                .unwrap();

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
            db.acquire_repo_lock(&project_id, "run-1", initial_expires)
                .unwrap();

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
            db.acquire_repo_lock(&project_id, "temp-run-id", expires)
                .unwrap();

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
            let acquired = db
                .acquire_repo_lock(&project_id, "run-2", new_expires)
                .unwrap();
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
