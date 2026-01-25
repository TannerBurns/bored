//! Database module with connection management

pub mod schema;
pub mod models;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use rusqlite::Connection;
use thiserror::Error;

pub use models::*;
use schema::{CREATE_TABLES, SCHEMA_VERSION, DEFAULT_COLUMNS};

#[derive(Error, Debug)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    
    #[error("Database not initialized")]
    NotInitialized,
    
    #[error("Lock error: {0}")]
    Lock(String),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Validation error: {0}")]
    Validation(String),
}

/// Thread-safe database handle
#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// Open or create database at the given path
    pub fn open(db_path: PathBuf) -> Result<Self, DbError> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|_e| DbError::Validation(format!("Failed to create directory: {:?}", parent)))?;
        }

        let conn = Connection::open(&db_path)?;
        
        // Enable foreign keys
        conn.execute("PRAGMA foreign_keys = ON", [])?;
        
        // Enable WAL mode for better concurrent access
        conn.execute("PRAGMA journal_mode = WAL", [])?;
        
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        
        // Run migrations
        db.migrate()?;
        
        tracing::info!("Database opened at {:?}", db_path);
        Ok(db)
    }

    /// Open an in-memory database (useful for testing)
    pub fn open_in_memory() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        conn.execute("PRAGMA foreign_keys = ON", [])?;
        
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        
        db.migrate()?;
        Ok(db)
    }

    /// Run database migrations
    fn migrate(&self) -> Result<(), DbError> {
        let conn = self.conn.lock()
            .map_err(|e| DbError::Lock(e.to_string()))?;

        // Check current schema version
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

            // Run schema creation (idempotent with IF NOT EXISTS)
            conn.execute_batch(CREATE_TABLES)?;

            // Record new schema version
            conn.execute(
                "INSERT OR REPLACE INTO schema_version (version) VALUES (?)",
                [SCHEMA_VERSION],
            )?;

            tracing::info!("Database migration complete");
        }

        Ok(())
    }

    /// Execute a function with the database connection
    pub fn with_conn<F, T>(&self, f: F) -> Result<T, DbError>
    where
        F: FnOnce(&Connection) -> Result<T, DbError>,
    {
        let conn = self.conn.lock()
            .map_err(|e| DbError::Lock(e.to_string()))?;
        f(&conn)
    }

    /// Execute a function with a mutable database connection (for transactions)
    pub fn with_conn_mut<F, T>(&self, f: F) -> Result<T, DbError>
    where
        F: FnOnce(&mut Connection) -> Result<T, DbError>,
    {
        let mut conn = self.conn.lock()
            .map_err(|e| DbError::Lock(e.to_string()))?;
        f(&mut conn)
    }
}

// ============================================================================
// Project Operations
// ============================================================================

impl Database {
    /// Create a new project
    pub fn create_project(&self, input: &CreateProject) -> Result<Project, DbError> {
        // Validate path exists and is a directory
        let path = std::path::Path::new(&input.path);
        if !path.exists() {
            return Err(DbError::Validation(format!(
                "Path does not exist: {}",
                input.path
            )));
        }
        if !path.is_dir() {
            return Err(DbError::Validation(format!(
                "Path is not a directory: {}",
                input.path
            )));
        }

        // Canonicalize path to avoid duplicates
        let canonical_path = path
            .canonicalize()
            .map_err(|e| DbError::Validation(format!("Invalid path: {}", e)))?
            .to_string_lossy()
            .to_string();

        self.with_conn(|conn| {
            let project_id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now();

            conn.execute(
                r#"INSERT INTO projects 
                   (id, name, path, preferred_agent, created_at, updated_at)
                   VALUES (?, ?, ?, ?, ?, ?)"#,
                rusqlite::params![
                    project_id,
                    input.name,
                    canonical_path,
                    input.preferred_agent.as_ref().map(|p| p.as_str()),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )?;

            Ok(Project {
                id: project_id,
                name: input.name.clone(),
                path: canonical_path,
                cursor_hooks_installed: false,
                claude_hooks_installed: false,
                preferred_agent: input.preferred_agent.clone(),
                allow_shell_commands: true,
                allow_file_writes: true,
                blocked_patterns: vec![],
                settings: serde_json::json!({}),
                created_at: now,
                updated_at: now,
            })
        })
    }

    /// Get all projects
    pub fn get_projects(&self) -> Result<Vec<Project>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, name, path, cursor_hooks_installed, claude_hooks_installed,
                          preferred_agent, allow_shell_commands, allow_file_writes,
                          blocked_patterns_json, settings_json, created_at, updated_at
                   FROM projects ORDER BY name"#,
            )?;

            let projects = stmt
                .query_map([], |row| {
                    let blocked_json: String = row.get(8)?;
                    let settings_json: String = row.get(9)?;
                    let pref_str: Option<String> = row.get(5)?;

                    Ok(Project {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        path: row.get(2)?,
                        cursor_hooks_installed: row.get::<_, i32>(3)? != 0,
                        claude_hooks_installed: row.get::<_, i32>(4)? != 0,
                        preferred_agent: pref_str.and_then(|s| AgentPref::from_str(&s)),
                        allow_shell_commands: row.get::<_, i32>(6)? != 0,
                        allow_file_writes: row.get::<_, i32>(7)? != 0,
                        blocked_patterns: serde_json::from_str(&blocked_json).unwrap_or_default(),
                        settings: serde_json::from_str(&settings_json).unwrap_or(serde_json::json!({})),
                        created_at: parse_datetime(row.get(10)?),
                        updated_at: parse_datetime(row.get(11)?),
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(projects)
        })
    }

    /// Get a project by ID
    pub fn get_project(&self, project_id: &str) -> Result<Option<Project>, DbError> {
        self.get_projects().map(|projects| {
            projects.into_iter().find(|p| p.id == project_id)
        })
    }

    /// Get a project by path
    pub fn get_project_by_path(&self, path: &str) -> Result<Option<Project>, DbError> {
        // Canonicalize the input path for comparison
        let canonical = std::path::Path::new(path)
            .canonicalize()
            .ok()
            .map(|p| p.to_string_lossy().to_string());

        self.get_projects().map(|projects| {
            projects.into_iter().find(|p| {
                Some(&p.path) == canonical.as_ref()
            })
        })
    }

    /// Update a project
    pub fn update_project(&self, project_id: &str, input: &UpdateProject) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();

            if let Some(ref name) = input.name {
                conn.execute(
                    "UPDATE projects SET name = ?, updated_at = ? WHERE id = ?",
                    rusqlite::params![name, now, project_id],
                )?;
            }

            if let Some(ref pref) = input.preferred_agent {
                conn.execute(
                    "UPDATE projects SET preferred_agent = ?, updated_at = ? WHERE id = ?",
                    rusqlite::params![pref.as_str(), now, project_id],
                )?;
            }

            if let Some(allow) = input.allow_shell_commands {
                conn.execute(
                    "UPDATE projects SET allow_shell_commands = ?, updated_at = ? WHERE id = ?",
                    rusqlite::params![allow as i32, now, project_id],
                )?;
            }

            if let Some(allow) = input.allow_file_writes {
                conn.execute(
                    "UPDATE projects SET allow_file_writes = ?, updated_at = ? WHERE id = ?",
                    rusqlite::params![allow as i32, now, project_id],
                )?;
            }

            if let Some(ref patterns) = input.blocked_patterns {
                let json = serde_json::to_string(patterns).unwrap_or_else(|_| "[]".to_string());
                conn.execute(
                    "UPDATE projects SET blocked_patterns_json = ?, updated_at = ? WHERE id = ?",
                    rusqlite::params![json, now, project_id],
                )?;
            }

            Ok(())
        })
    }

    /// Update hook installation status
    pub fn update_project_hooks(
        &self,
        project_id: &str,
        cursor_installed: Option<bool>,
        claude_installed: Option<bool>,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();

            if let Some(installed) = cursor_installed {
                conn.execute(
                    "UPDATE projects SET cursor_hooks_installed = ?, updated_at = ? WHERE id = ?",
                    rusqlite::params![installed as i32, now, project_id],
                )?;
            }

            if let Some(installed) = claude_installed {
                conn.execute(
                    "UPDATE projects SET claude_hooks_installed = ?, updated_at = ? WHERE id = ?",
                    rusqlite::params![installed as i32, now, project_id],
                )?;
            }

            Ok(())
        })
    }

    /// Delete a project
    pub fn delete_project(&self, project_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            // Check if any boards use this as default
            let board_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM boards WHERE default_project_id = ?",
                [project_id],
                |row| row.get(0),
            )?;

            if board_count > 0 {
                return Err(DbError::Validation(format!(
                    "Cannot delete project: {} board(s) use it as default",
                    board_count
                )));
            }

            conn.execute("DELETE FROM projects WHERE id = ?", [project_id])?;
            Ok(())
        })
    }

    /// Set a board's default project
    pub fn set_board_project(
        &self,
        board_id: &str,
        project_id: Option<&str>,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "UPDATE boards SET default_project_id = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![project_id, now, board_id],
            )?;
            Ok(())
        })
    }

    /// Check if a ticket can be moved to Ready
    pub fn can_move_to_ready(&self, ticket_id: &str) -> Result<ReadinessCheck, DbError> {
        self.with_conn(|conn| {
            // Get ticket and its board
            let result: Result<(Option<String>, String), _> = conn.query_row(
                "SELECT project_id, board_id FROM tickets WHERE id = ?",
                [ticket_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            );

            let (ticket_project_id, board_id) = result
                .map_err(|_| DbError::NotFound(format!("Ticket {} not found", ticket_id)))?;

            // Get board's default project
            let board_project_id: Option<String> = conn.query_row(
                "SELECT default_project_id FROM boards WHERE id = ?",
                [&board_id],
                |row| row.get(0),
            ).ok().flatten();

            // Determine effective project
            let effective_project_id = ticket_project_id.or(board_project_id);

            match effective_project_id {
                Some(pid) => {
                    // Verify project exists and path is valid
                    let path: Option<String> = conn.query_row(
                        "SELECT path FROM projects WHERE id = ?",
                        [&pid],
                        |row| row.get(0),
                    ).ok();

                    if let Some(p) = path {
                        if std::path::Path::new(&p).exists() {
                            Ok(ReadinessCheck::Ready { project_id: pid })
                        } else {
                            Ok(ReadinessCheck::ProjectPathMissing { path: p })
                        }
                    } else {
                        Ok(ReadinessCheck::ProjectNotFound)
                    }
                }
                None => Ok(ReadinessCheck::NoProject),
            }
        })
    }

    /// Resolve the project for a ticket (ticket override or board default)
    pub fn resolve_project_for_ticket(&self, ticket_id: &str) -> Result<Option<Project>, DbError> {
        match self.can_move_to_ready(ticket_id)? {
            ReadinessCheck::Ready { project_id } => self.get_project(&project_id),
            _ => Ok(None),
        }
    }
}

// ============================================================================
// Board Operations
// ============================================================================

impl Database {
    /// Create a new board with default columns
    pub fn create_board(&self, name: &str) -> Result<Board, DbError> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            
            let board_id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now();
            
            tx.execute(
                "INSERT INTO boards (id, name, created_at, updated_at) VALUES (?, ?, ?, ?)",
                rusqlite::params![board_id, name, now.to_rfc3339(), now.to_rfc3339()],
            )?;

            // Create default columns
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

    /// Get all boards
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

    /// Get a board by ID
    pub fn get_board(&self, board_id: &str) -> Result<Option<Board>, DbError> {
        self.get_boards().map(|boards| {
            boards.into_iter().find(|b| b.id == board_id)
        })
    }

    /// Get columns for a board
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
}

// ============================================================================
// Ticket Operations
// ============================================================================

impl Database {
    /// Create a new ticket
    pub fn create_ticket(&self, ticket: &CreateTicket) -> Result<Ticket, DbError> {
        self.with_conn(|conn| {
            let ticket_id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now();
            let labels_json = serde_json::to_string(&ticket.labels).unwrap_or_else(|_| "[]".to_string());
            
            conn.execute(
                r#"INSERT INTO tickets 
                   (id, board_id, column_id, title, description_md, priority, labels_json, 
                    created_at, updated_at, project_id, agent_pref)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
                rusqlite::params![
                    ticket_id,
                    ticket.board_id,
                    ticket.column_id,
                    ticket.title,
                    ticket.description_md,
                    ticket.priority.as_str(),
                    labels_json,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                    ticket.project_id,
                    ticket.agent_pref.as_ref().map(|p| p.as_str()),
                ],
            )?;

            Ok(Ticket {
                id: ticket_id,
                board_id: ticket.board_id.clone(),
                column_id: ticket.column_id.clone(),
                title: ticket.title.clone(),
                description_md: ticket.description_md.clone(),
                priority: ticket.priority.clone(),
                labels: ticket.labels.clone(),
                created_at: now,
                updated_at: now,
                locked_by_run_id: None,
                lock_expires_at: None,
                project_id: ticket.project_id.clone(),
                agent_pref: ticket.agent_pref.clone(),
            })
        })
    }

    /// Get tickets for a board, optionally filtered by column
    pub fn get_tickets(&self, board_id: &str, column_id: Option<&str>) -> Result<Vec<Ticket>, DbError> {
        self.with_conn(|conn| {
            let sql = match column_id {
                Some(_) => {
                    "SELECT id, board_id, column_id, title, description_md, priority, 
                            labels_json, created_at, updated_at, locked_by_run_id, 
                            lock_expires_at, project_id, agent_pref
                     FROM tickets WHERE board_id = ? AND column_id = ? ORDER BY created_at"
                }
                None => {
                    "SELECT id, board_id, column_id, title, description_md, priority, 
                            labels_json, created_at, updated_at, locked_by_run_id, 
                            lock_expires_at, project_id, agent_pref
                     FROM tickets WHERE board_id = ? ORDER BY created_at"
                }
            };

            let mut stmt = conn.prepare(sql)?;
            
            let rows = match column_id {
                Some(col_id) => stmt.query_map(rusqlite::params![board_id, col_id], Self::map_ticket_row)?,
                None => stmt.query_map([board_id], Self::map_ticket_row)?,
            };

            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    /// Move a ticket to a different column
    pub fn move_ticket(&self, ticket_id: &str, column_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now();
            let affected = conn.execute(
                "UPDATE tickets SET column_id = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![column_id, now.to_rfc3339(), ticket_id],
            )?;
            
            if affected == 0 {
                return Err(DbError::NotFound(format!("Ticket {} not found", ticket_id)));
            }
            Ok(())
        })
    }

    /// Set ticket's project override
    pub fn set_ticket_project(&self, ticket_id: &str, project_id: Option<&str>) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "UPDATE tickets SET project_id = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![project_id, now, ticket_id],
            )?;
            Ok(())
        })
    }

    fn map_ticket_row(row: &rusqlite::Row) -> rusqlite::Result<Ticket> {
        let labels_json: String = row.get(6)?;
        let labels: Vec<String> = serde_json::from_str(&labels_json).unwrap_or_default();
        
        let priority_str: String = row.get(5)?;
        let priority = Priority::from_str(&priority_str).unwrap_or(Priority::Medium);
        
        let agent_pref_str: Option<String> = row.get(12)?;
        let agent_pref = agent_pref_str.and_then(|s| AgentPref::from_str(&s));

        Ok(Ticket {
            id: row.get(0)?,
            board_id: row.get(1)?,
            column_id: row.get(2)?,
            title: row.get(3)?,
            description_md: row.get(4)?,
            priority,
            labels,
            created_at: parse_datetime(row.get(7)?),
            updated_at: parse_datetime(row.get(8)?),
            locked_by_run_id: row.get(9)?,
            lock_expires_at: row.get::<_, Option<String>>(10)?.map(parse_datetime),
            project_id: row.get(11)?,
            agent_pref,
        })
    }
}

// ============================================================================
// Agent Run Operations
// ============================================================================

impl Database {
    /// Create a new agent run
    pub fn create_run(&self, run: &CreateRun) -> Result<AgentRun, DbError> {
        self.with_conn(|conn| {
            let run_id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now();
            
            conn.execute(
                r#"INSERT INTO agent_runs 
                   (id, ticket_id, agent_type, repo_path, status, started_at)
                   VALUES (?, ?, ?, ?, ?, ?)"#,
                rusqlite::params![
                    run_id,
                    run.ticket_id,
                    run.agent_type.as_str(),
                    run.repo_path,
                    RunStatus::Queued.as_str(),
                    now.to_rfc3339(),
                ],
            )?;

            Ok(AgentRun {
                id: run_id,
                ticket_id: run.ticket_id.clone(),
                agent_type: run.agent_type.clone(),
                repo_path: run.repo_path.clone(),
                status: RunStatus::Queued,
                started_at: now,
                ended_at: None,
                exit_code: None,
                summary_md: None,
                metadata: None,
            })
        })
    }

    /// Update run status
    pub fn update_run_status(
        &self,
        run_id: &str,
        status: RunStatus,
        exit_code: Option<i32>,
        summary_md: Option<&str>,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now();
            let ended_at = if matches!(status, RunStatus::Finished | RunStatus::Error | RunStatus::Aborted) {
                Some(now.to_rfc3339())
            } else {
                None
            };
            
            conn.execute(
                "UPDATE agent_runs SET status = ?, ended_at = ?, exit_code = ?, summary_md = ? WHERE id = ?",
                rusqlite::params![status.as_str(), ended_at, exit_code, summary_md, run_id],
            )?;
            Ok(())
        })
    }

    /// Get runs for a ticket
    pub fn get_runs(&self, ticket_id: &str) -> Result<Vec<AgentRun>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, ticket_id, agent_type, repo_path, status, 
                          started_at, ended_at, exit_code, summary_md, metadata_json
                   FROM agent_runs WHERE ticket_id = ? ORDER BY started_at DESC"#
            )?;
            
            let runs = stmt.query_map([ticket_id], |row| {
                let agent_type_str: String = row.get(2)?;
                let status_str: String = row.get(4)?;
                let metadata_json: Option<String> = row.get(9)?;
                
                Ok(AgentRun {
                    id: row.get(0)?,
                    ticket_id: row.get(1)?,
                    agent_type: match agent_type_str.as_str() {
                        "cursor" => AgentType::Cursor,
                        _ => AgentType::Claude,
                    },
                    repo_path: row.get(3)?,
                    status: RunStatus::from_str(&status_str).unwrap_or(RunStatus::Error),
                    started_at: parse_datetime(row.get(5)?),
                    ended_at: row.get::<_, Option<String>>(6)?.map(parse_datetime),
                    exit_code: row.get(7)?,
                    summary_md: row.get(8)?,
                    metadata: metadata_json.and_then(|s| serde_json::from_str(&s).ok()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
            
            Ok(runs)
        })
    }
}

// ============================================================================
// Agent Event Operations
// ============================================================================

impl Database {
    /// Record an agent event
    pub fn create_event(&self, event: &NormalizedEvent) -> Result<AgentEvent, DbError> {
        self.with_conn(|conn| {
            let event_id = uuid::Uuid::new_v4().to_string();
            let payload_json = serde_json::to_string(&event.payload)
                .unwrap_or_else(|_| "{}".to_string());
            
            conn.execute(
                r#"INSERT INTO agent_events 
                   (id, run_id, ticket_id, event_type, payload_json, created_at)
                   VALUES (?, ?, ?, ?, ?, ?)"#,
                rusqlite::params![
                    event_id,
                    event.run_id,
                    event.ticket_id,
                    event.event_type.as_str(),
                    payload_json,
                    event.timestamp.to_rfc3339(),
                ],
            )?;

            Ok(AgentEvent {
                id: event_id,
                run_id: event.run_id.clone(),
                ticket_id: event.ticket_id.clone(),
                event_type: event.event_type.clone(),
                payload: event.payload.clone(),
                created_at: event.timestamp,
            })
        })
    }

    /// Get events for a run
    pub fn get_events(&self, run_id: &str) -> Result<Vec<AgentEvent>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, run_id, ticket_id, event_type, payload_json, created_at
                   FROM agent_events WHERE run_id = ? ORDER BY created_at"#
            )?;
            
            let events = stmt.query_map([run_id], |row| {
                let event_type_str: String = row.get(3)?;
                let payload_json: String = row.get(4)?;
                let payload: AgentEventPayload = serde_json::from_str(&payload_json)
                    .unwrap_or(AgentEventPayload { raw: None, structured: None });
                
                Ok(AgentEvent {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    ticket_id: row.get(2)?,
                    event_type: EventType::from_str(&event_type_str),
                    payload,
                    created_at: parse_datetime(row.get(5)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
            
            Ok(events)
        })
    }
}

/// Helper to parse datetime strings
fn parse_datetime(s: String) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(&s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_board() {
        let db = Database::open_in_memory().unwrap();
        let board = db.create_board("Test Board").unwrap();
        
        assert_eq!(board.name, "Test Board");
        
        let columns = db.get_columns(&board.id).unwrap();
        assert_eq!(columns.len(), 6); // Default columns
        assert_eq!(columns[0].name, "Backlog");
        assert_eq!(columns[5].name, "Done");
    }

    #[test]
    fn test_create_ticket() {
        let db = Database::open_in_memory().unwrap();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Test Ticket".to_string(),
            description_md: "Description".to_string(),
            priority: Priority::High,
            labels: vec!["bug".to_string()],
            project_id: None,
            agent_pref: Some(AgentPref::Cursor),
        }).unwrap();
        
        assert_eq!(ticket.title, "Test Ticket");
        assert_eq!(ticket.priority, Priority::High);
        assert_eq!(ticket.labels, vec!["bug"]);
    }

    #[test]
    fn test_project_crud() {
        let db = Database::open_in_memory().unwrap();
        
        // Create project (use temp dir for testing)
        let temp_dir = std::env::temp_dir();
        let project = db.create_project(&CreateProject {
            name: "Test Project".to_string(),
            path: temp_dir.to_string_lossy().to_string(),
            preferred_agent: Some(AgentPref::Cursor),
        }).unwrap();
        
        assert_eq!(project.name, "Test Project");
        assert_eq!(project.preferred_agent, Some(AgentPref::Cursor));
        
        // Get projects
        let projects = db.get_projects().unwrap();
        assert_eq!(projects.len(), 1);
        
        // Update project
        db.update_project(&project.id, &UpdateProject {
            name: Some("Updated Project".to_string()),
            preferred_agent: None,
            allow_shell_commands: Some(false),
            allow_file_writes: None,
            blocked_patterns: None,
        }).unwrap();
        
        let updated = db.get_project(&project.id).unwrap().unwrap();
        assert_eq!(updated.name, "Updated Project");
        assert!(!updated.allow_shell_commands);
        
        // Delete project
        db.delete_project(&project.id).unwrap();
        let projects = db.get_projects().unwrap();
        assert_eq!(projects.len(), 0);
    }
}
