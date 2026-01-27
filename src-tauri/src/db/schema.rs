//! Database schema definitions and migrations

pub const SCHEMA_VERSION: i32 = 5;

/// Initial schema creation SQL
pub const CREATE_TABLES: &str = r#"
-- Projects table (registered repositories for agent work)
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    path TEXT NOT NULL UNIQUE,
    
    -- Hook installation status
    cursor_hooks_installed INTEGER NOT NULL DEFAULT 0,
    claude_hooks_installed INTEGER NOT NULL DEFAULT 0,
    
    -- Agent preferences for this project
    preferred_agent TEXT CHECK(preferred_agent IN ('cursor', 'claude', 'any')),
    
    -- Safety settings
    allow_shell_commands INTEGER NOT NULL DEFAULT 1,
    allow_file_writes INTEGER NOT NULL DEFAULT 1,
    blocked_patterns_json TEXT NOT NULL DEFAULT '[]',
    
    -- General settings
    settings_json TEXT NOT NULL DEFAULT '{}',
    
    -- Whether this project requires git (default true for backward compatibility)
    requires_git INTEGER NOT NULL DEFAULT 1,
    
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_projects_path ON projects(path);

-- Boards table
CREATE TABLE IF NOT EXISTS boards (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    default_project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Columns table (kanban columns within a board)
CREATE TABLE IF NOT EXISTS columns (
    id TEXT PRIMARY KEY NOT NULL,
    board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    wip_limit INTEGER,
    UNIQUE(board_id, position)
);

CREATE INDEX IF NOT EXISTS idx_columns_board ON columns(board_id);

-- Tickets table
-- Note: locked_by_run_id intentionally omits FK constraint to avoid circular
-- dependency with agent_runs table. Referential integrity is maintained at
-- the application level.
CREATE TABLE IF NOT EXISTS tickets (
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
    workflow_type TEXT NOT NULL DEFAULT 'basic' CHECK(workflow_type IN ('basic', 'multi_stage')),
    model TEXT
);

CREATE INDEX IF NOT EXISTS idx_tickets_board ON tickets(board_id);
CREATE INDEX IF NOT EXISTS idx_tickets_column ON tickets(column_id);
CREATE INDEX IF NOT EXISTS idx_tickets_locked ON tickets(locked_by_run_id) WHERE locked_by_run_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_tickets_project ON tickets(project_id);

-- Comments table
CREATE TABLE IF NOT EXISTS comments (
    id TEXT PRIMARY KEY NOT NULL,
    ticket_id TEXT NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
    author_type TEXT NOT NULL CHECK(author_type IN ('user', 'agent', 'system')),
    body_md TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    metadata_json TEXT
);

CREATE INDEX IF NOT EXISTS idx_comments_ticket ON comments(ticket_id);

-- Agent runs table
CREATE TABLE IF NOT EXISTS agent_runs (
    id TEXT PRIMARY KEY NOT NULL,
    ticket_id TEXT NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
    agent_type TEXT NOT NULL CHECK(agent_type IN ('cursor', 'claude')),
    repo_path TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued' CHECK(status IN ('queued', 'running', 'finished', 'error', 'aborted')),
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    ended_at TEXT,
    exit_code INTEGER,
    summary_md TEXT,
    metadata_json TEXT,
    parent_run_id TEXT REFERENCES agent_runs(id) ON DELETE CASCADE,
    stage TEXT
);

CREATE INDEX IF NOT EXISTS idx_runs_ticket ON agent_runs(ticket_id);
CREATE INDEX IF NOT EXISTS idx_runs_status ON agent_runs(status);
CREATE INDEX IF NOT EXISTS idx_runs_parent ON agent_runs(parent_run_id) WHERE parent_run_id IS NOT NULL;

-- Agent events table (audit trail for hook events)
CREATE TABLE IF NOT EXISTS agent_events (
    id TEXT PRIMARY KEY NOT NULL,
    run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    ticket_id TEXT NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    payload_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_events_run ON agent_events(run_id);
CREATE INDEX IF NOT EXISTS idx_events_ticket ON agent_events(ticket_id);
CREATE INDEX IF NOT EXISTS idx_events_type ON agent_events(event_type);

-- Schema version tracking
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Repository-level locks to prevent multiple workers processing same repo
CREATE TABLE IF NOT EXISTS repo_locks (
    project_id TEXT PRIMARY KEY NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    locked_by_run_id TEXT NOT NULL,
    lock_expires_at TEXT NOT NULL,
    locked_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_repo_locks_expires ON repo_locks(lock_expires_at);
"#;

/// Migration SQL for schema version 3
/// Adds repo_locks table and requires_git column to projects
pub const MIGRATION_V3: &str = r#"
-- Add requires_git column to projects (defaults to true for backward compatibility)
ALTER TABLE projects ADD COLUMN requires_git INTEGER NOT NULL DEFAULT 1;

-- Repository-level locks to prevent multiple workers processing same repo
CREATE TABLE IF NOT EXISTS repo_locks (
    project_id TEXT PRIMARY KEY NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    locked_by_run_id TEXT NOT NULL,
    lock_expires_at TEXT NOT NULL,
    locked_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_repo_locks_expires ON repo_locks(lock_expires_at);
"#;

/// Migration SQL for schema version 4
/// Adds workflow_type to tickets, parent_run_id and stage to agent_runs
pub const MIGRATION_V4: &str = r#"
-- Add workflow_type column to tickets (defaults to 'basic' for backward compatibility)
ALTER TABLE tickets ADD COLUMN workflow_type TEXT NOT NULL DEFAULT 'basic';

-- Add parent_run_id column to agent_runs for sub-run tracking
ALTER TABLE agent_runs ADD COLUMN parent_run_id TEXT REFERENCES agent_runs(id) ON DELETE CASCADE;

-- Add stage column to agent_runs for tracking workflow stages
ALTER TABLE agent_runs ADD COLUMN stage TEXT;

-- Index for efficient sub-run queries
CREATE INDEX IF NOT EXISTS idx_runs_parent ON agent_runs(parent_run_id) WHERE parent_run_id IS NOT NULL;
"#;

/// Migration SQL for schema version 5
/// Adds model column to tickets for per-ticket AI model selection
pub const MIGRATION_V5: &str = r#"
-- Add model column to tickets for AI model selection (e.g., 'claude-opus-4-5')
ALTER TABLE tickets ADD COLUMN model TEXT;
"#;

/// Default columns for a new board
pub const DEFAULT_COLUMNS: &[&str] = &[
    "Backlog",
    "Ready",
    "In Progress",
    "Blocked",
    "Review",
    "Done",
];
