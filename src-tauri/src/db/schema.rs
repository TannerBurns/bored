//! Database schema definitions and migrations

pub const SCHEMA_VERSION: i32 = 10;

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
    workflow_type TEXT NOT NULL DEFAULT 'multi_stage' CHECK(workflow_type IN ('multi_stage')),
    model TEXT,
    branch_name TEXT,
    -- Epic support: is_epic marks this ticket as an epic, epic_id references parent epic
    is_epic INTEGER NOT NULL DEFAULT 0,
    epic_id TEXT REFERENCES tickets(id) ON DELETE SET NULL,
    order_in_epic INTEGER,
    -- Cross-epic dependency: which epic must complete before this epic can start
    depends_on_epic_id TEXT REFERENCES tickets(id) ON DELETE SET NULL,
    -- Link back to scratchpad that created this ticket
    scratchpad_id TEXT REFERENCES scratchpads(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_tickets_board ON tickets(board_id);
CREATE INDEX IF NOT EXISTS idx_tickets_column ON tickets(column_id);
CREATE INDEX IF NOT EXISTS idx_tickets_locked ON tickets(locked_by_run_id) WHERE locked_by_run_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_tickets_project ON tickets(project_id);
CREATE INDEX IF NOT EXISTS idx_tickets_epic ON tickets(epic_id, order_in_epic) WHERE epic_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_tickets_depends_on ON tickets(depends_on_epic_id) WHERE depends_on_epic_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_tickets_scratchpad ON tickets(scratchpad_id) WHERE scratchpad_id IS NOT NULL;

-- Scratchpads table (for planner agent)
CREATE TABLE IF NOT EXISTS scratchpads (
    id TEXT PRIMARY KEY NOT NULL,
    board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    user_input TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft', 'exploring', 'planning', 'awaiting_approval', 'approved', 'executing', 'completed', 'failed')),
    exploration_log TEXT,
    plan_markdown TEXT,
    plan_json TEXT,
    settings_json TEXT NOT NULL DEFAULT '{}',
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_scratchpads_board ON scratchpads(board_id);
CREATE INDEX IF NOT EXISTS idx_scratchpads_status ON scratchpads(status);

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

-- Tasks table (task queue for tickets)
CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY NOT NULL,
    ticket_id TEXT NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
    order_index INTEGER NOT NULL,
    task_type TEXT NOT NULL DEFAULT 'custom' CHECK(task_type IN ('custom', 'sync_with_main', 'add_tests', 'review_polish', 'fix_lint')),
    title TEXT,
    content TEXT,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'in_progress', 'completed', 'failed')),
    run_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    started_at TEXT,
    completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_tasks_ticket ON tasks(ticket_id);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_order ON tasks(ticket_id, order_index);
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

/// Migration SQL for schema version 6
/// Removes single-shot (basic) workflow - all tickets use multi_stage
pub const MIGRATION_V6: &str = r#"
-- Convert all 'basic' workflow types to 'multi_stage'
UPDATE tickets SET workflow_type = 'multi_stage' WHERE workflow_type = 'basic';
"#;

/// Migration SQL for schema version 7
/// Adds branch_name column to tickets for storing agent-generated branch names
pub const MIGRATION_V7: &str = r#"
-- Add branch_name column to tickets for persistent branch tracking
ALTER TABLE tickets ADD COLUMN branch_name TEXT;
"#;

/// Migration SQL for schema version 8
/// Adds tasks table for task queue system
pub const MIGRATION_V8: &str = r#"
-- Tasks table (task queue for tickets)
CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY NOT NULL,
    ticket_id TEXT NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
    order_index INTEGER NOT NULL,
    task_type TEXT NOT NULL DEFAULT 'custom' CHECK(task_type IN ('custom', 'sync_with_main', 'add_tests', 'review_polish', 'fix_lint')),
    title TEXT,
    content TEXT,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'in_progress', 'completed', 'failed')),
    run_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    started_at TEXT,
    completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_tasks_ticket ON tasks(ticket_id);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_order ON tasks(ticket_id, order_index);
"#;

/// Migration SQL for schema version 9
/// Adds epic support: is_epic, epic_id, order_in_epic columns
pub const MIGRATION_V9: &str = r#"
-- Add epic columns to tickets
ALTER TABLE tickets ADD COLUMN is_epic INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tickets ADD COLUMN epic_id TEXT REFERENCES tickets(id) ON DELETE SET NULL;
ALTER TABLE tickets ADD COLUMN order_in_epic INTEGER;

-- Index for efficient epic children queries
CREATE INDEX IF NOT EXISTS idx_tickets_epic ON tickets(epic_id, order_in_epic) WHERE epic_id IS NOT NULL;
"#;

/// Migration SQL for schema version 10
/// Adds scratchpads table and epic dependency columns
pub const MIGRATION_V10: &str = r#"
-- Add depends_on_epic_id column to tickets for cross-epic dependencies
ALTER TABLE tickets ADD COLUMN depends_on_epic_id TEXT REFERENCES tickets(id) ON DELETE SET NULL;

-- Add scratchpad_id column to tickets to link back to generating scratchpad
ALTER TABLE tickets ADD COLUMN scratchpad_id TEXT REFERENCES scratchpads(id) ON DELETE SET NULL;

-- Create scratchpads table
CREATE TABLE IF NOT EXISTS scratchpads (
    id TEXT PRIMARY KEY NOT NULL,
    board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    user_input TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft', 'exploring', 'planning', 'awaiting_approval', 'approved', 'executing', 'completed', 'failed')),
    exploration_log TEXT,
    plan_markdown TEXT,
    plan_json TEXT,
    settings_json TEXT NOT NULL DEFAULT '{}',
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Indexes for scratchpads
CREATE INDEX IF NOT EXISTS idx_scratchpads_board ON scratchpads(board_id);
CREATE INDEX IF NOT EXISTS idx_scratchpads_status ON scratchpads(status);

-- Indexes for new ticket columns
CREATE INDEX IF NOT EXISTS idx_tickets_depends_on ON tickets(depends_on_epic_id) WHERE depends_on_epic_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_tickets_scratchpad ON tickets(scratchpad_id) WHERE scratchpad_id IS NOT NULL;
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
