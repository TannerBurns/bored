pub mod schema;
pub mod models;
mod projects;
mod boards;
mod tickets;
mod runs;
mod events;

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

            conn.execute_batch(CREATE_TABLES)?;
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
