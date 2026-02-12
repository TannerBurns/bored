//! Database schema definitions and migrations

pub const SCHEMA_VERSION: i32 = 7;

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

-- Specs table (for spec/planning agent)
-- Note: Must be created before tickets table since tickets references specs(id)
-- Versioned fields (status, exploration_log, plan_*, work_started_at) are in spec_versions table
CREATE TABLE IF NOT EXISTS specs (
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

-- Spec versions table (versioned exploration/plan data for each spec)
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
    workflow_type TEXT NOT NULL DEFAULT 'multi_stage' CHECK(workflow_type IN ('multi_stage')),
    model TEXT,
    branch_name TEXT,
    -- Epic support: is_epic marks this ticket as an epic, epic_id references parent epic
    is_epic INTEGER NOT NULL DEFAULT 0,
    epic_id TEXT REFERENCES tickets(id) ON DELETE SET NULL,
    order_in_epic INTEGER,
    -- Cross-epic dependency: which epic must complete before this epic can start (primary dependency)
    depends_on_epic_id TEXT REFERENCES tickets(id) ON DELETE SET NULL,
    -- All epic dependencies as JSON array of IDs (for display purposes)
    depends_on_epic_ids_json TEXT,
    -- Link back to spec version that created this ticket
    spec_version_id TEXT REFERENCES spec_versions(id) ON DELETE SET NULL,
    -- Pause state for tickets
    paused_at TEXT,
    paused_at_stage TEXT,
    paused_run_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_tickets_board ON tickets(board_id);
CREATE INDEX IF NOT EXISTS idx_tickets_column ON tickets(column_id);
CREATE INDEX IF NOT EXISTS idx_tickets_locked ON tickets(locked_by_run_id) WHERE locked_by_run_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_tickets_project ON tickets(project_id);
CREATE INDEX IF NOT EXISTS idx_tickets_epic ON tickets(epic_id, order_in_epic) WHERE epic_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_tickets_depends_on ON tickets(depends_on_epic_id) WHERE depends_on_epic_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_tickets_spec_version ON tickets(spec_version_id) WHERE spec_version_id IS NOT NULL;

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
    status TEXT NOT NULL DEFAULT 'queued' CHECK(status IN ('queued', 'running', 'finished', 'error', 'aborted', 'paused')),
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    ended_at TEXT,
    exit_code INTEGER,
    summary_md TEXT,
    metadata_json TEXT,
    parent_run_id TEXT REFERENCES agent_runs(id) ON DELETE CASCADE,
    stage TEXT,
    -- For resumed runs: links to the run this is resuming from
    resumed_from_run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_runs_ticket ON agent_runs(ticket_id);
CREATE INDEX IF NOT EXISTS idx_runs_status ON agent_runs(status);
CREATE INDEX IF NOT EXISTS idx_runs_parent ON agent_runs(parent_run_id) WHERE parent_run_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_runs_resumed_from ON agent_runs(resumed_from_run_id) WHERE resumed_from_run_id IS NOT NULL;

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

-- Conversation messages table (for spec brainstorming)
CREATE TABLE IF NOT EXISTS conversation_messages (
    id TEXT PRIMARY KEY NOT NULL,
    spec_id TEXT NOT NULL REFERENCES specs(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('user', 'assistant', 'system')),
    content TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_conversation_messages_spec ON conversation_messages(spec_id);
CREATE INDEX IF NOT EXISTS idx_conversation_messages_created ON conversation_messages(spec_id, created_at);

-- Release notes table (populated at startup from embedded data)
CREATE TABLE IF NOT EXISTS release_notes (
    version TEXT PRIMARY KEY NOT NULL,
    published_at TEXT NOT NULL,
    summary TEXT,
    notes_json TEXT NOT NULL DEFAULT '[]'
);

-- Validation sessions table (post-completion validation chat)
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

-- Validation messages table (chat messages within a validation session)
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
