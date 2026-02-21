mod boards;
mod comments;
mod conversations;
mod costs;
mod events;
pub mod models;
mod projects;
pub mod release_notes;
mod runs;
pub mod schema;
mod spec_versions;
mod specs;
pub mod tasks;
pub mod tickets;
mod validation;

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use thiserror::Error;

pub use models::*;
use schema::{CREATE_TABLES, SCHEMA_VERSION};
pub use tickets::IncompleteDependency;
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

    #[error("Migration error: {0}")]
    Migration(String),
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
        db.seed_release_notes()?;

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
        db.seed_release_notes()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), DbError> {
        let conn = self.conn.lock().map_err(|e| DbError::Lock(e.to_string()))?;

        // Query current schema version, handling the case where the table doesn't exist yet
        let current_version: i32 = conn
            .query_row(
                "SELECT version FROM schema_version ORDER BY version DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or_else(|e| {
                // Table might not exist yet (fresh database)
                tracing::debug!("Could not read schema_version (likely fresh db): {}", e);
                0
            });

        if current_version < SCHEMA_VERSION {
            tracing::info!(
                "Migrating database from version {} to {}",
                current_version,
                SCHEMA_VERSION
            );
            
            // CRITICAL: Disable foreign keys before migration transaction.
            // SQLite's DROP TABLE with foreign_keys=ON performs an implicit
            // DELETE FROM before dropping, which fires cascading actions
            // (ON DELETE SET NULL / CASCADE) and corrupts referencing tables.
            // This pragma must be set outside a transaction to take effect.
            conn.execute("PRAGMA foreign_keys = OFF", [])?;
            
            // Start a transaction for atomicity - if any migration step fails,
            // all changes will be rolled back automatically
            conn.execute("BEGIN EXCLUSIVE TRANSACTION", [])?;
            
            // Use a closure to ensure we can rollback on any error
            let migration_result = (|| -> Result<(), DbError> {

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
                // First check if we need to migrate (if table exists with the OLD format that has 'status' column)
                let specs_has_old_format: bool = conn
                    .query_row(
                        "SELECT COUNT(*) FROM pragma_table_info('specs') WHERE name='status'",
                        [],
                        |row| row.get::<_, i32>(0),
                    )
                    .unwrap_or(0) > 0;
                
                if specs_has_old_format {
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

            // Migration from version 2 to 3: Introduce spec versioning
            if current_version < 3 {
                // Check if migration is needed (specs table has old format with 'status' column)
                let specs_has_old_format: bool = conn
                    .query_row(
                        "SELECT COUNT(*) FROM pragma_table_info('specs') WHERE name='status'",
                        [],
                        |row| row.get::<_, i32>(0),
                    )
                    .unwrap_or(0) > 0;
                
                if specs_has_old_format {
                    tracing::info!("Running migration to version 3: spec versioning");
                
                // Step 1: Create spec_versions table
                conn.execute_batch(
                    r#"
                    CREATE TABLE IF NOT EXISTS spec_versions (
                        id TEXT PRIMARY KEY NOT NULL,
                        spec_id TEXT NOT NULL REFERENCES specs(id) ON DELETE CASCADE,
                        version_number INTEGER NOT NULL,
                        status TEXT NOT NULL DEFAULT 'conversing' CHECK(status IN ('conversing', 'exploring', 'planning', 'awaiting_approval', 'approved', 'executing', 'executed', 'working', 'paused', 'halted', 'completed', 'failed')),
                        exploration_log TEXT DEFAULT '[]',
                        plan_markdown TEXT,
                        plan_json TEXT,
                        work_started_at TEXT,
                        created_at TEXT NOT NULL DEFAULT (datetime('now')),
                        updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                        UNIQUE(spec_id, version_number)
                    );

                    CREATE INDEX IF NOT EXISTS idx_spec_versions_spec ON spec_versions(spec_id);
                    CREATE INDEX IF NOT EXISTS idx_spec_versions_status ON spec_versions(status);
                    "#
                )?;
                
                // Step 2: Migrate existing specs to spec_versions
                // For each spec, create a version 1 with the spec's current versioned data
                conn.execute_batch(
                    r#"
                    INSERT INTO spec_versions (id, spec_id, version_number, status, exploration_log, plan_markdown, plan_json, work_started_at, created_at, updated_at)
                    SELECT 
                        lower(hex(randomblob(16))),
                        id,
                        1,
                        CASE 
                            WHEN status = 'draft' THEN 'conversing'
                            ELSE status 
                        END,
                        COALESCE(exploration_log, '[]'),
                        plan_markdown,
                        plan_json,
                        work_started_at,
                        created_at,
                        updated_at
                    FROM specs;
                    "#
                )?;
                
                // Step 3: Add spec_version_id column to tickets and migrate data
                conn.execute_batch(
                    r#"
                    ALTER TABLE tickets ADD COLUMN spec_version_id TEXT REFERENCES spec_versions(id) ON DELETE SET NULL;
                    
                    UPDATE tickets 
                    SET spec_version_id = (
                        SELECT sv.id 
                        FROM spec_versions sv 
                        WHERE sv.spec_id = tickets.spec_id 
                        AND sv.version_number = 1
                    )
                    WHERE spec_id IS NOT NULL;
                    
                    CREATE INDEX IF NOT EXISTS idx_tickets_spec_version ON tickets(spec_version_id) WHERE spec_version_id IS NOT NULL;
                    "#
                )?;
                
                // Step 4: Recreate specs table without versioned fields
                conn.execute_batch(
                    r#"
                    CREATE TABLE specs_v3 (
                        id TEXT PRIMARY KEY NOT NULL,
                        board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
                        target_board_id TEXT REFERENCES boards(id) ON DELETE SET NULL,
                        project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                        name TEXT NOT NULL,
                        user_input TEXT NOT NULL,
                        agent_pref TEXT CHECK(agent_pref IS NULL OR agent_pref IN ('cursor', 'claude', 'any')),
                        model TEXT,
                        settings_json TEXT NOT NULL DEFAULT '{}',
                        created_at TEXT NOT NULL DEFAULT (datetime('now')),
                        updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                    );

                    INSERT INTO specs_v3 (id, board_id, target_board_id, project_id, name, user_input, agent_pref, model, settings_json, created_at, updated_at)
                    SELECT id, board_id, target_board_id, project_id, name, user_input, agent_pref, model, settings_json, created_at, updated_at FROM specs;

                    DROP TABLE specs;
                    ALTER TABLE specs_v3 RENAME TO specs;

                    CREATE INDEX IF NOT EXISTS idx_specs_board ON specs(board_id);
                    CREATE INDEX IF NOT EXISTS idx_specs_target_board ON specs(target_board_id);
                    CREATE INDEX IF NOT EXISTS idx_specs_project ON specs(project_id);
                    "#
                )?;
                
                // Step 5: Drop old spec_id column from tickets (SQLite doesn't support DROP COLUMN before 3.35)
                // We'll recreate the table without the old column
                conn.execute_batch(
                    r#"
                    CREATE TABLE tickets_v3 (
                        id TEXT PRIMARY KEY NOT NULL,
                        board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
                        column_id TEXT NOT NULL REFERENCES columns(id) ON DELETE RESTRICT,
                        title TEXT NOT NULL,
                        description_md TEXT NOT NULL DEFAULT '',
                        priority TEXT NOT NULL DEFAULT 'medium' CHECK(priority IN ('low', 'medium', 'high', 'urgent')),
                        labels_json TEXT NOT NULL DEFAULT '[]',
                        created_at TEXT NOT NULL DEFAULT (datetime('now')),
                        updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                        locked_by_run_id TEXT,
                        lock_expires_at TEXT,
                        project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
                        agent_pref TEXT CHECK(agent_pref IN ('cursor', 'claude', 'any')),
                        workflow_type TEXT NOT NULL DEFAULT 'multi_stage' CHECK(workflow_type IN ('multi_stage')),
                        model TEXT,
                        branch_name TEXT,
                        is_epic INTEGER NOT NULL DEFAULT 0,
                        epic_id TEXT REFERENCES tickets(id) ON DELETE SET NULL,
                        order_in_epic INTEGER,
                        depends_on_epic_id TEXT REFERENCES tickets(id) ON DELETE SET NULL,
                        depends_on_epic_ids_json TEXT,
                        spec_version_id TEXT REFERENCES spec_versions(id) ON DELETE SET NULL,
                        paused_at TEXT,
                        paused_at_stage TEXT,
                        paused_run_id TEXT
                    );

                    INSERT INTO tickets_v3 (id, board_id, column_id, title, description_md, priority, labels_json, created_at, updated_at, locked_by_run_id, lock_expires_at, project_id, agent_pref, workflow_type, model, branch_name, is_epic, epic_id, order_in_epic, depends_on_epic_id, depends_on_epic_ids_json, spec_version_id, paused_at, paused_at_stage, paused_run_id)
                    SELECT id, board_id, column_id, title, description_md, priority, labels_json, created_at, updated_at, locked_by_run_id, lock_expires_at, project_id, agent_pref, workflow_type, model, branch_name, is_epic, epic_id, order_in_epic, depends_on_epic_id, depends_on_epic_ids_json, spec_version_id, paused_at, paused_at_stage, paused_run_id FROM tickets;

                    DROP TABLE tickets;
                    ALTER TABLE tickets_v3 RENAME TO tickets;

                    CREATE INDEX IF NOT EXISTS idx_tickets_board ON tickets(board_id);
                    CREATE INDEX IF NOT EXISTS idx_tickets_column ON tickets(column_id);
                    CREATE INDEX IF NOT EXISTS idx_tickets_locked ON tickets(locked_by_run_id) WHERE locked_by_run_id IS NOT NULL;
                    CREATE INDEX IF NOT EXISTS idx_tickets_project ON tickets(project_id);
                    CREATE INDEX IF NOT EXISTS idx_tickets_epic ON tickets(epic_id, order_in_epic) WHERE epic_id IS NOT NULL;
                    CREATE INDEX IF NOT EXISTS idx_tickets_depends_on ON tickets(depends_on_epic_id) WHERE depends_on_epic_id IS NOT NULL;
                    CREATE INDEX IF NOT EXISTS idx_tickets_spec_version ON tickets(spec_version_id) WHERE spec_version_id IS NOT NULL;
                    "#
                )?;
                
                tracing::info!("Migration to version 3 complete: spec versioning enabled");
                } // end if specs_has_old_format
            }

            // Migration from version 3 to 4: Remove agent_pref/preferred_agent columns
            if current_version < 4 {
                tracing::info!("Running migration to version 4: remove agent preference columns");
                
                // SQLite doesn't support DROP COLUMN before version 3.35.0
                // We need to recreate tables without the columns
                
                // Check if projects table has preferred_agent column
                let projects_has_pref: bool = conn
                    .query_row(
                        "SELECT COUNT(*) FROM pragma_table_info('projects') WHERE name='preferred_agent'",
                        [],
                        |row| row.get::<_, i32>(0),
                    )
                    .unwrap_or(0) > 0;
                
                if projects_has_pref {
                    conn.execute_batch(
                        r#"
                        CREATE TABLE projects_v4 (
                            id TEXT PRIMARY KEY NOT NULL,
                            name TEXT NOT NULL,
                            path TEXT NOT NULL UNIQUE,
                            cursor_hooks_installed INTEGER NOT NULL DEFAULT 0,
                            claude_hooks_installed INTEGER NOT NULL DEFAULT 0,
                            allow_shell_commands INTEGER NOT NULL DEFAULT 1,
                            allow_file_writes INTEGER NOT NULL DEFAULT 1,
                            blocked_patterns_json TEXT NOT NULL DEFAULT '[]',
                            settings_json TEXT NOT NULL DEFAULT '{}',
                            requires_git INTEGER NOT NULL DEFAULT 1,
                            created_at TEXT NOT NULL DEFAULT (datetime('now')),
                            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                        );

                        INSERT INTO projects_v4 (id, name, path, cursor_hooks_installed, claude_hooks_installed,
                            allow_shell_commands, allow_file_writes, blocked_patterns_json, settings_json,
                            requires_git, created_at, updated_at)
                        SELECT id, name, path, cursor_hooks_installed, claude_hooks_installed,
                            allow_shell_commands, allow_file_writes, blocked_patterns_json, settings_json,
                            requires_git, created_at, updated_at FROM projects;

                        DROP TABLE projects;
                        ALTER TABLE projects_v4 RENAME TO projects;

                        CREATE INDEX IF NOT EXISTS idx_projects_path ON projects(path);
                        "#
                    )?;
                    tracing::info!("Removed preferred_agent from projects table");
                }
                
                // Check if tickets table has agent_pref column
                let tickets_has_pref: bool = conn
                    .query_row(
                        "SELECT COUNT(*) FROM pragma_table_info('tickets') WHERE name='agent_pref'",
                        [],
                        |row| row.get::<_, i32>(0),
                    )
                    .unwrap_or(0) > 0;
                
                if tickets_has_pref {
                    conn.execute_batch(
                        r#"
                        CREATE TABLE tickets_v4 (
                            id TEXT PRIMARY KEY NOT NULL,
                            board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
                            column_id TEXT NOT NULL REFERENCES columns(id) ON DELETE RESTRICT,
                            title TEXT NOT NULL,
                            description_md TEXT NOT NULL DEFAULT '',
                            priority TEXT NOT NULL DEFAULT 'medium' CHECK(priority IN ('low', 'medium', 'high', 'urgent')),
                            labels_json TEXT NOT NULL DEFAULT '[]',
                            created_at TEXT NOT NULL DEFAULT (datetime('now')),
                            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                            locked_by_run_id TEXT,
                            lock_expires_at TEXT,
                            project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
                            workflow_type TEXT NOT NULL DEFAULT 'multi_stage' CHECK(workflow_type IN ('multi_stage')),
                            model TEXT,
                            branch_name TEXT,
                            is_epic INTEGER NOT NULL DEFAULT 0,
                            epic_id TEXT REFERENCES tickets(id) ON DELETE SET NULL,
                            order_in_epic INTEGER,
                            depends_on_epic_id TEXT REFERENCES tickets(id) ON DELETE SET NULL,
                            depends_on_epic_ids_json TEXT,
                            spec_version_id TEXT REFERENCES spec_versions(id) ON DELETE SET NULL,
                            paused_at TEXT,
                            paused_at_stage TEXT,
                            paused_run_id TEXT
                        );

                        INSERT INTO tickets_v4 (id, board_id, column_id, title, description_md, priority, labels_json,
                            created_at, updated_at, locked_by_run_id, lock_expires_at, project_id, workflow_type,
                            model, branch_name, is_epic, epic_id, order_in_epic, depends_on_epic_id,
                            depends_on_epic_ids_json, spec_version_id, paused_at, paused_at_stage, paused_run_id)
                        SELECT id, board_id, column_id, title, description_md, priority, labels_json,
                            created_at, updated_at, locked_by_run_id, lock_expires_at, project_id, workflow_type,
                            model, branch_name, is_epic, epic_id, order_in_epic, depends_on_epic_id,
                            depends_on_epic_ids_json, spec_version_id, paused_at, paused_at_stage, paused_run_id
                        FROM tickets;

                        DROP TABLE tickets;
                        ALTER TABLE tickets_v4 RENAME TO tickets;

                        CREATE INDEX IF NOT EXISTS idx_tickets_board ON tickets(board_id);
                        CREATE INDEX IF NOT EXISTS idx_tickets_column ON tickets(column_id);
                        CREATE INDEX IF NOT EXISTS idx_tickets_locked ON tickets(locked_by_run_id) WHERE locked_by_run_id IS NOT NULL;
                        CREATE INDEX IF NOT EXISTS idx_tickets_project ON tickets(project_id);
                        CREATE INDEX IF NOT EXISTS idx_tickets_epic ON tickets(epic_id, order_in_epic) WHERE epic_id IS NOT NULL;
                        CREATE INDEX IF NOT EXISTS idx_tickets_depends_on ON tickets(depends_on_epic_id) WHERE depends_on_epic_id IS NOT NULL;
                        CREATE INDEX IF NOT EXISTS idx_tickets_spec_version ON tickets(spec_version_id) WHERE spec_version_id IS NOT NULL;
                        "#
                    )?;
                    tracing::info!("Removed agent_pref from tickets table");
                }
                
                // Check if specs table has agent_pref column
                let specs_has_pref: bool = conn
                    .query_row(
                        "SELECT COUNT(*) FROM pragma_table_info('specs') WHERE name='agent_pref'",
                        [],
                        |row| row.get::<_, i32>(0),
                    )
                    .unwrap_or(0) > 0;
                
                if specs_has_pref {
                    conn.execute_batch(
                        r#"
                        CREATE TABLE specs_v4 (
                            id TEXT PRIMARY KEY NOT NULL,
                            board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
                            target_board_id TEXT REFERENCES boards(id) ON DELETE SET NULL,
                            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                            name TEXT NOT NULL,
                            user_input TEXT NOT NULL,
                            model TEXT,
                            settings_json TEXT NOT NULL DEFAULT '{}',
                            created_at TEXT NOT NULL DEFAULT (datetime('now')),
                            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                        );

                        INSERT INTO specs_v4 (id, board_id, target_board_id, project_id, name, user_input,
                            model, settings_json, created_at, updated_at)
                        SELECT id, board_id, target_board_id, project_id, name, user_input,
                            model, settings_json, created_at, updated_at FROM specs;

                        DROP TABLE specs;
                        ALTER TABLE specs_v4 RENAME TO specs;

                        CREATE INDEX IF NOT EXISTS idx_specs_board ON specs(board_id);
                        CREATE INDEX IF NOT EXISTS idx_specs_target_board ON specs(target_board_id);
                        CREATE INDEX IF NOT EXISTS idx_specs_project ON specs(project_id);
                        "#
                    )?;
                    tracing::info!("Removed agent_pref from specs table");
                }
                
                tracing::info!("Migration to version 4 complete: agent preference columns removed");
            }

            // Migration from version 4 to 5: Add release_notes table
            if current_version < 5 {
                tracing::info!("Running migration to version 5: release_notes table");
                
                conn.execute_batch(
                    r#"
                    CREATE TABLE IF NOT EXISTS release_notes (
                        version TEXT PRIMARY KEY NOT NULL,
                        published_at TEXT NOT NULL,
                        summary TEXT,
                        notes_json TEXT NOT NULL DEFAULT '[]'
                    );
                    "#
                )?;
                
                tracing::info!("Migration to version 5 complete: release_notes table added");
            }

            // Migration from version 5 to 6: Add validation_sessions and validation_messages tables
            // Skip when current_version is 0 (fresh DB) — CREATE_TABLES already has validation tables
            if current_version > 0 && current_version < 6 {
                tracing::info!("Running migration to version 6: validation tables");

                conn.execute_batch(
                    r#"
                    CREATE TABLE IF NOT EXISTS validation_sessions (
                        id TEXT PRIMARY KEY NOT NULL,
                        ticket_id TEXT NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
                        project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
                        status TEXT NOT NULL DEFAULT 'created' CHECK(status IN ('created', 'chatting', 'app_running', 'passed', 'failed')),
                        app_command TEXT,
                        app_port INTEGER,
                        created_at TEXT NOT NULL DEFAULT (datetime('now')),
                        updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                    );

                    CREATE INDEX IF NOT EXISTS idx_validation_sessions_ticket ON validation_sessions(ticket_id);
                    CREATE INDEX IF NOT EXISTS idx_validation_sessions_status ON validation_sessions(status);

                    CREATE TABLE IF NOT EXISTS validation_messages (
                        id TEXT PRIMARY KEY NOT NULL,
                        session_id TEXT NOT NULL REFERENCES validation_sessions(id) ON DELETE CASCADE,
                        role TEXT NOT NULL CHECK(role IN ('user', 'assistant', 'system')),
                        content TEXT NOT NULL,
                        metadata_json TEXT,
                        created_at TEXT NOT NULL DEFAULT (datetime('now'))
                    );

                    CREATE INDEX IF NOT EXISTS idx_validation_messages_session ON validation_messages(session_id);
                    CREATE INDEX IF NOT EXISTS idx_validation_messages_created ON validation_messages(session_id, created_at);
                    "#
                )?;

                tracing::info!("Migration to version 6 complete: validation tables added");
            }

            // Migration from version 6 to 7: Add agent_type to validation_sessions
            // Skip when current_version is 0 (fresh DB) — CREATE_TABLES already has agent_type
            if current_version > 0 && current_version < 7 {
                tracing::info!("Running migration to version 7: validation_sessions.agent_type");
                conn.execute(
                    "ALTER TABLE validation_sessions ADD COLUMN agent_type TEXT",
                    [],
                )?;
                tracing::info!("Migration to version 7 complete: agent_type added");
            }

            // Migration from version 7 to 8: Remove app_command and app_port from validation_sessions
            // Skip when current_version is 0 (fresh DB) — CREATE_TABLES already has final schema
            if current_version > 0 && current_version < 8 {
                tracing::info!("Running migration to version 8: drop app_command and app_port from validation_sessions");
                conn.execute_batch(
                    r#"
                    CREATE TABLE validation_sessions_new (
                        id TEXT PRIMARY KEY NOT NULL,
                        ticket_id TEXT NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
                        project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
                        status TEXT NOT NULL DEFAULT 'created' CHECK(status IN ('created', 'chatting', 'app_running', 'passed', 'failed')),
                        agent_type TEXT,
                        created_at TEXT NOT NULL DEFAULT (datetime('now')),
                        updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                    );
                    INSERT INTO validation_sessions_new (id, ticket_id, project_id, status, agent_type, created_at, updated_at)
                    SELECT id, ticket_id, project_id, status, agent_type, created_at, updated_at FROM validation_sessions;
                    DROP TABLE validation_sessions;
                    ALTER TABLE validation_sessions_new RENAME TO validation_sessions;
                    CREATE INDEX IF NOT EXISTS idx_validation_sessions_ticket ON validation_sessions(ticket_id);
                    CREATE INDEX IF NOT EXISTS idx_validation_sessions_status ON validation_sessions(status);
                    "#
                )?;
                tracing::info!("Migration to version 8 complete: app_command and app_port removed");
            }

            // Migration from version 8 to 9: Replace per-agent hooks columns with hooks_installed_json
            // Skip when current_version is 0 (fresh DB) — CREATE_TABLES already has final schema
            if current_version > 0 && current_version < 9 {
                tracing::info!("Running migration to version 9: hooks_installed_json column");

                // Check if old columns exist
                let has_old_hooks: bool = conn
                    .query_row(
                        "SELECT COUNT(*) FROM pragma_table_info('projects') WHERE name='cursor_hooks_installed'",
                        [],
                        |row| row.get::<_, i32>(0),
                    )
                    .unwrap_or(0) > 0;

                if has_old_hooks {
                    conn.execute_batch(
                        r#"
                        CREATE TABLE projects_v9 (
                            id TEXT PRIMARY KEY NOT NULL,
                            name TEXT NOT NULL,
                            path TEXT NOT NULL UNIQUE,
                            hooks_installed_json TEXT NOT NULL DEFAULT '{}',
                            allow_shell_commands INTEGER NOT NULL DEFAULT 1,
                            allow_file_writes INTEGER NOT NULL DEFAULT 1,
                            blocked_patterns_json TEXT NOT NULL DEFAULT '[]',
                            settings_json TEXT NOT NULL DEFAULT '{}',
                            requires_git INTEGER NOT NULL DEFAULT 1,
                            created_at TEXT NOT NULL DEFAULT (datetime('now')),
                            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                        );

                        INSERT INTO projects_v9 (id, name, path, hooks_installed_json,
                            allow_shell_commands, allow_file_writes, blocked_patterns_json,
                            settings_json, requires_git, created_at, updated_at)
                        SELECT id, name, path,
                            json_object('cursor', CASE WHEN cursor_hooks_installed != 0 THEN json('true') ELSE json('false') END,
                                        'claude', CASE WHEN claude_hooks_installed != 0 THEN json('true') ELSE json('false') END),
                            allow_shell_commands, allow_file_writes, blocked_patterns_json,
                            settings_json, requires_git, created_at, updated_at
                        FROM projects;

                        DROP TABLE projects;
                        ALTER TABLE projects_v9 RENAME TO projects;

                        CREATE INDEX IF NOT EXISTS idx_projects_path ON projects(path);
                        "#
                    )?;
                }

                tracing::info!("Migration to version 9 complete: hooks_installed_json column added");
            }

            // Migration from version 9 to 10: Repair project associations
            // Previous migrations used DROP TABLE on the projects table while
            // PRAGMA foreign_keys was ON, which triggered implicit DELETE FROM
            // and cascaded ON DELETE SET NULL / CASCADE to referencing tables.
            // This repair attempts to restore ticket and board project associations.
            if current_version > 0 && current_version < 10 {
                tracing::info!("Running migration to version 10: repair project associations");

                let project_count: i32 = conn
                    .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
                    .unwrap_or(0);

                if project_count == 1 {
                    // Single project — safe to auto-assign all orphaned items
                    let project_id: String = conn.query_row(
                        "SELECT id FROM projects LIMIT 1",
                        [],
                        |row| row.get(0),
                    )?;

                    let repaired_tickets: usize = conn.execute(
                        "UPDATE tickets SET project_id = ?1 WHERE project_id IS NULL",
                        [&project_id],
                    )?;

                    let repaired_boards: usize = conn.execute(
                        "UPDATE boards SET default_project_id = ?1 WHERE default_project_id IS NULL",
                        [&project_id],
                    )?;

                    if repaired_tickets > 0 || repaired_boards > 0 {
                        tracing::info!(
                            "Repaired project associations: {} tickets, {} boards assigned to project '{}'",
                            repaired_tickets, repaired_boards, project_id
                        );
                    }
                } else if project_count > 1 {
                    // Multiple projects — assign from board defaults where available
                    let repaired_from_board: usize = conn.execute(
                        r#"UPDATE tickets SET project_id = (
                            SELECT default_project_id FROM boards WHERE boards.id = tickets.board_id
                        ) WHERE project_id IS NULL AND board_id IN (
                            SELECT id FROM boards WHERE default_project_id IS NOT NULL
                        )"#,
                        [],
                    )?;

                    let still_orphaned: i32 = conn
                        .query_row(
                            "SELECT COUNT(*) FROM tickets WHERE project_id IS NULL",
                            [],
                            |row| row.get(0),
                        )
                        .unwrap_or(0);

                    if repaired_from_board > 0 {
                        tracing::info!(
                            "Repaired {} tickets from board defaults, {} still without project",
                            repaired_from_board, still_orphaned
                        );
                    }

                    if still_orphaned > 0 {
                        tracing::warn!(
                            "{} tickets have no project assigned. Multiple projects exist; \
                             manual reassignment may be needed via the ticket settings.",
                            still_orphaned
                        );
                    }
                }

                tracing::info!("Migration to version 10 complete: project association repair");
            }

            // Migration from version 10 to 11: Improved project association repair
            // Uses agent_runs.repo_path (which survived the FK cascade since it
            // references tickets, not projects) to match tickets back to projects.
            if current_version > 0 && current_version < 11 {
                tracing::info!("Running migration to version 11: repo_path-based project repair");

                let orphaned_before: i32 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM tickets WHERE project_id IS NULL",
                        [],
                        |row| row.get(0),
                    )
                    .unwrap_or(0);

                if orphaned_before > 0 {
                    // Step 1: Match via agent_runs.repo_path = projects.path
                    let from_runs: usize = conn.execute(
                        r#"UPDATE tickets SET project_id = (
                            SELECT p.id FROM projects p
                            INNER JOIN agent_runs r ON r.ticket_id = tickets.id
                            WHERE r.repo_path = p.path
                            LIMIT 1
                        )
                        WHERE project_id IS NULL
                        AND EXISTS (
                            SELECT 1 FROM agent_runs r
                            INNER JOIN projects p ON p.path = r.repo_path
                            WHERE r.ticket_id = tickets.id
                        )"#,
                        [],
                    )?;

                    if from_runs > 0 {
                        tracing::info!("Repaired {} tickets from agent run repo_path", from_runs);
                    }

                    // Step 2: Board-level inference — if other tickets on the same
                    // board already have a project (from step 1), assign orphans
                    // on that board to the most common project.
                    let from_board_inference: usize = conn.execute(
                        r#"UPDATE tickets SET project_id = (
                            SELECT t2.project_id FROM tickets t2
                            WHERE t2.board_id = tickets.board_id
                              AND t2.project_id IS NOT NULL
                            GROUP BY t2.project_id
                            ORDER BY COUNT(*) DESC
                            LIMIT 1
                        )
                        WHERE project_id IS NULL
                        AND EXISTS (
                            SELECT 1 FROM tickets t2
                            WHERE t2.board_id = tickets.board_id
                              AND t2.project_id IS NOT NULL
                        )"#,
                        [],
                    )?;

                    if from_board_inference > 0 {
                        tracing::info!(
                            "Repaired {} tickets from board-level inference",
                            from_board_inference
                        );
                    }

                    // Step 3: Single-project fallback for anything still orphaned
                    let still_orphaned: i32 = conn
                        .query_row(
                            "SELECT COUNT(*) FROM tickets WHERE project_id IS NULL",
                            [],
                            |row| row.get(0),
                        )
                        .unwrap_or(0);

                    if still_orphaned > 0 {
                        let project_count: i32 = conn
                            .query_row(
                                "SELECT COUNT(*) FROM projects",
                                [],
                                |row| row.get(0),
                            )
                            .unwrap_or(0);

                        if project_count == 1 {
                            let project_id: String = conn.query_row(
                                "SELECT id FROM projects LIMIT 1",
                                [],
                                |row| row.get(0),
                            )?;
                            let assigned: usize = conn.execute(
                                "UPDATE tickets SET project_id = ?1 WHERE project_id IS NULL",
                                [&project_id],
                            )?;
                            if assigned > 0 {
                                tracing::info!(
                                    "Assigned {} remaining tickets to sole project",
                                    assigned
                                );
                            }
                        } else if still_orphaned > 0 {
                            tracing::warn!(
                                "{} tickets still orphaned across {} projects",
                                still_orphaned,
                                project_count
                            );
                        }
                    }

                    // Step 4: Recover board defaults from repaired ticket data
                    let boards_fixed: usize = conn.execute(
                        r#"UPDATE boards SET default_project_id = (
                            SELECT project_id FROM tickets
                            WHERE tickets.board_id = boards.id
                              AND project_id IS NOT NULL
                            GROUP BY project_id
                            ORDER BY COUNT(*) DESC
                            LIMIT 1
                        )
                        WHERE default_project_id IS NULL
                        AND EXISTS (
                            SELECT 1 FROM tickets
                            WHERE tickets.board_id = boards.id
                              AND project_id IS NOT NULL
                        )"#,
                        [],
                    )?;

                    if boards_fixed > 0 {
                        tracing::info!(
                            "Recovered {} board default_project_ids from ticket data",
                            boards_fixed
                        );
                    }
                }

                tracing::info!("Migration to version 11 complete");
            }

            // Migration from version 11 to 12: Remove agent_type CHECK constraint
            if current_version > 0 && current_version < 12 {
                tracing::info!("Running migration to version 12: remove agent_type CHECK constraint");

                conn.execute_batch(
                    r#"
                    CREATE TABLE agent_runs_v12 (
                        id TEXT PRIMARY KEY NOT NULL,
                        ticket_id TEXT NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
                        agent_type TEXT NOT NULL,
                        repo_path TEXT NOT NULL,
                        status TEXT NOT NULL DEFAULT 'queued' CHECK(status IN ('queued', 'running', 'finished', 'error', 'aborted', 'paused')),
                        started_at TEXT NOT NULL DEFAULT (datetime('now')),
                        ended_at TEXT,
                        exit_code INTEGER,
                        summary_md TEXT,
                        metadata_json TEXT,
                        parent_run_id TEXT REFERENCES agent_runs_v12(id) ON DELETE CASCADE,
                        stage TEXT,
                        resumed_from_run_id TEXT REFERENCES agent_runs_v12(id) ON DELETE SET NULL
                    );

                    INSERT INTO agent_runs_v12 (id, ticket_id, agent_type, repo_path, status,
                        started_at, ended_at, exit_code, summary_md, metadata_json,
                        parent_run_id, stage, resumed_from_run_id)
                    SELECT id, ticket_id, agent_type, repo_path, status,
                        started_at, ended_at, exit_code, summary_md, metadata_json,
                        parent_run_id, stage, resumed_from_run_id
                    FROM agent_runs;

                    DROP TABLE agent_runs;
                    ALTER TABLE agent_runs_v12 RENAME TO agent_runs;

                    CREATE INDEX IF NOT EXISTS idx_runs_ticket ON agent_runs(ticket_id);
                    CREATE INDEX IF NOT EXISTS idx_runs_status ON agent_runs(status);
                    CREATE INDEX IF NOT EXISTS idx_runs_parent ON agent_runs(parent_run_id) WHERE parent_run_id IS NOT NULL;
                    CREATE INDEX IF NOT EXISTS idx_runs_resumed_from ON agent_runs(resumed_from_run_id) WHERE resumed_from_run_id IS NOT NULL;
                    "#
                )?;

                tracing::info!("Migration to version 12 complete: agent_type CHECK constraint removed");
            }

            // Migration from version 12 to 13: Remove hooks_installed_json column
            // Hooks have been fully removed from the application; this column is no longer read.
            if current_version > 0 && current_version < 13 {
                tracing::info!("Running migration to version 13: remove hooks_installed_json column");

                let has_column: bool = conn
                    .query_row(
                        "SELECT COUNT(*) FROM pragma_table_info('projects') WHERE name='hooks_installed_json'",
                        [],
                        |row| row.get::<_, i32>(0),
                    )
                    .unwrap_or(0) > 0;

                if has_column {
                    conn.execute_batch(
                        r#"
                        CREATE TABLE projects_v13 (
                            id TEXT PRIMARY KEY NOT NULL,
                            name TEXT NOT NULL,
                            path TEXT NOT NULL UNIQUE,
                            allow_shell_commands INTEGER NOT NULL DEFAULT 1,
                            allow_file_writes INTEGER NOT NULL DEFAULT 1,
                            blocked_patterns_json TEXT NOT NULL DEFAULT '[]',
                            settings_json TEXT NOT NULL DEFAULT '{}',
                            requires_git INTEGER NOT NULL DEFAULT 1,
                            created_at TEXT NOT NULL DEFAULT (datetime('now')),
                            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                        );

                        INSERT INTO projects_v13 (id, name, path,
                            allow_shell_commands, allow_file_writes, blocked_patterns_json,
                            settings_json, requires_git, created_at, updated_at)
                        SELECT id, name, path,
                            allow_shell_commands, allow_file_writes, blocked_patterns_json,
                            settings_json, requires_git, created_at, updated_at
                        FROM projects;

                        DROP TABLE projects;
                        ALTER TABLE projects_v13 RENAME TO projects;

                        CREATE UNIQUE INDEX IF NOT EXISTS idx_projects_path ON projects(path);
                        "#
                    )?;
                }

                tracing::info!("Migration to version 13 complete: hooks_installed_json column removed");
            }

            // Migration from version 13 to 14: Add previous_versions_json to release_notes
            // Skip when current_version is 0 (fresh DB) — CREATE_TABLES already has the column
            if current_version > 0 && current_version < 14 {
                tracing::info!("Running migration to version 14: release_notes.previous_versions_json");
                conn.execute(
                    "ALTER TABLE release_notes ADD COLUMN previous_versions_json TEXT",
                    [],
                )?;
                tracing::info!("Migration to version 14 complete: previous_versions_json added");
            }

            conn.execute(
                "INSERT OR REPLACE INTO schema_version (version) VALUES (?)",
                [SCHEMA_VERSION],
            )?;

            Ok(())
            })(); // End of migration closure
            
            // Handle the migration result - commit on success, rollback on failure
            match migration_result {
                Ok(()) => {
                    conn.execute("COMMIT", [])?;
                    tracing::info!("Database migration complete - committed successfully");
                }
                Err(e) => {
                    tracing::error!("Migration failed, rolling back: {}", e);
                    // Attempt rollback - if this fails too, log it but return the original error
                    if let Err(rollback_err) = conn.execute("ROLLBACK", []) {
                        tracing::error!("Rollback also failed: {}", rollback_err);
                    }
                    // Re-enable foreign keys even on failure
                    let _ = conn.execute("PRAGMA foreign_keys = ON", []);
                    return Err(DbError::Migration(format!(
                        "Migration from version {} to {} failed: {}. Database has been rolled back to version {}.",
                        current_version, SCHEMA_VERSION, e, current_version
                    )));
                }
            }
            
            // Re-enable foreign key enforcement after migration
            conn.execute("PRAGMA foreign_keys = ON", [])?;
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

    /// Repair the specs table schema.
    /// This recreates the table with the correct schema (versioned fields moved to spec_versions).
    pub fn repair_specs_constraint(&self) -> Result<String, DbError> {
        self.with_conn(|conn| {
            tracing::warn!("Repairing specs table schema");
            
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
            
            tracing::info!("Specs table has {} rows", row_count);
            
            // Recreate the table with the new schema (no versioned fields)
            conn.execute_batch(
                r#"
                CREATE TABLE specs_repaired (
                    id TEXT PRIMARY KEY NOT NULL,
                    board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
                    target_board_id TEXT REFERENCES boards(id) ON DELETE SET NULL,
                    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                    name TEXT NOT NULL,
                    user_input TEXT NOT NULL,
                    model TEXT,
                    settings_json TEXT NOT NULL DEFAULT '{}',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );

                INSERT INTO specs_repaired (id, board_id, target_board_id, project_id, name, user_input, model, settings_json, created_at, updated_at)
                SELECT id, board_id, target_board_id, project_id, name, user_input, model, settings_json, created_at, updated_at FROM specs;

                DROP TABLE specs;
                ALTER TABLE specs_repaired RENAME TO specs;

                CREATE INDEX IF NOT EXISTS idx_specs_board ON specs(board_id);
                CREATE INDEX IF NOT EXISTS idx_specs_target_board ON specs(target_board_id);
                CREATE INDEX IF NOT EXISTS idx_specs_project ON specs(project_id);
                "#
            )?;
            
            tracing::info!("Specs table repaired successfully");
            Ok(format!("Repaired specs table with {} rows", row_count))
        })
    }

    /// Factory reset: delete all user data from the database.
    /// This clears all boards, tickets, projects, runs, specs, etc.
    /// and also repairs the table schemas to ensure correct constraints.
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
            
            // Tickets must be deleted before spec_versions (spec_version_id FK)
            // and before columns (column_id FK with RESTRICT)
            conn.execute("DELETE FROM tickets", [])?;
            
            // Spec versions (depends on specs)
            conn.execute("DELETE FROM spec_versions", [])?;
            
            // Conversation messages (depends on specs)
            conn.execute("DELETE FROM conversation_messages", [])?;
            
            // Now specs (depends on boards and projects)
            conn.execute("DELETE FROM specs", [])?;
            
            // Columns (depends on boards)
            conn.execute("DELETE FROM columns", [])?;
            
            // Boards (depends on projects via default_project_id)
            conn.execute("DELETE FROM boards", [])?;
            
            // Finally projects (root table)
            conn.execute("DELETE FROM projects", [])?;
            
            tracing::info!("Factory reset: all user data deleted");
            
            // Recreate tables with correct schema
            tracing::info!("Factory reset: recreating tables with correct schema");
            
            // Drop and recreate specs table
            conn.execute("DROP TABLE IF EXISTS spec_versions", [])?;
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
                    model TEXT,
                    settings_json TEXT NOT NULL DEFAULT '{}',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );

                CREATE INDEX IF NOT EXISTS idx_specs_board ON specs(board_id);
                CREATE INDEX IF NOT EXISTS idx_specs_target_board ON specs(target_board_id);
                CREATE INDEX IF NOT EXISTS idx_specs_project ON specs(project_id);

                CREATE TABLE spec_versions (
                    id TEXT PRIMARY KEY NOT NULL,
                    spec_id TEXT NOT NULL REFERENCES specs(id) ON DELETE CASCADE,
                    version_number INTEGER NOT NULL,
                    status TEXT NOT NULL DEFAULT 'conversing' CHECK(status IN ('conversing', 'exploring', 'planning', 'awaiting_approval', 'approved', 'executing', 'executed', 'working', 'paused', 'halted', 'completed', 'failed')),
                    exploration_log TEXT DEFAULT '[]',
                    plan_markdown TEXT,
                    plan_json TEXT,
                    work_started_at TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                    UNIQUE(spec_id, version_number)
                );

                CREATE INDEX IF NOT EXISTS idx_spec_versions_spec ON spec_versions(spec_id);
                CREATE INDEX IF NOT EXISTS idx_spec_versions_status ON spec_versions(status);
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
