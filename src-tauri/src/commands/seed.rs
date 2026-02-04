//! Demo data seeding for screenshots and testing

use std::sync::Arc;
use tauri::State;

use crate::db::models::{
    AgentEventPayload, AgentType, CreateProject, CreateRun, CreateTicket, EventType,
    NormalizedEvent, Priority, RunStatus, WorkflowType,
};
use crate::db::Database;

/// Seed the database with realistic demo data for screenshots.
/// This creates a complete demo environment with:
/// - A demo project
/// - A board with tickets across all columns
/// - Various labels and priorities
/// - A completed run with events
/// - An in-progress run with live events
#[tauri::command]
pub async fn seed_demo_data(db: State<'_, Arc<Database>>) -> Result<String, String> {
    tracing::info!("Seeding demo data for screenshots...");

    // Create demo project
    let project = db
        .create_project(&CreateProject {
            name: "My Web App".to_string(),
            path: "/Users/demo/my-web-app".to_string(),
            requires_git: false, // Skip git checks for demo
        })
        .map_err(|e| format!("Failed to create project: {}", e))?;

    tracing::info!("Created demo project: {}", project.name);

    // Create board
    let board = db
        .create_board("Development Sprint")
        .map_err(|e| format!("Failed to create board: {}", e))?;

    tracing::info!("Created demo board: {}", board.name);

    // Set board's default project
    db.set_board_project(&board.id, Some(&project.id))
        .map_err(|e| format!("Failed to set board project: {}", e))?;

    // Get columns
    let columns = db
        .get_columns(&board.id)
        .map_err(|e| format!("Failed to get columns: {}", e))?;

    // Find column IDs by name
    let get_column_id = |name: &str| -> Result<String, String> {
        columns
            .iter()
            .find(|c| c.name.to_lowercase() == name.to_lowercase())
            .map(|c| c.id.clone())
            .ok_or_else(|| format!("Column '{}' not found", name))
    };

    let backlog_id = get_column_id("Backlog")?;
    let ready_id = get_column_id("Ready")?;
    let in_progress_id = get_column_id("In Progress")?;
    let blocked_id = get_column_id("Blocked")?;
    let review_id = get_column_id("Review")?;
    let done_id = get_column_id("Done")?;

    // Helper to create tickets
    let create_ticket = |title: &str,
                         description: &str,
                         column_id: &str,
                         priority: Priority,
                         labels: Vec<&str>|
     -> Result<crate::db::models::Ticket, String> {
        db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: column_id.to_string(),
            title: title.to_string(),
            description_md: description.to_string(),
            priority,
            labels: labels.into_iter().map(String::from).collect(),
            project_id: Some(project.id.clone()),
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .map_err(|e| format!("Failed to create ticket '{}': {}", title, e))
    };

    // === BACKLOG (2 tickets) ===
    create_ticket(
        "Add dark mode support",
        r#"## Overview
Implement a dark mode theme toggle for the application.

## Requirements
- Add a theme toggle in the settings panel
- Store preference in localStorage
- Use CSS custom properties for theming
- Respect system preference by default

## Acceptance Criteria
- [ ] Toggle switch works correctly
- [ ] Theme persists across sessions
- [ ] All components render correctly in both themes"#,
        &backlog_id,
        Priority::Medium,
        vec!["frontend", "feature"],
    )?;

    create_ticket(
        "Implement search functionality",
        r#"## Overview
Add global search across all boards and tickets.

## Requirements
- Search by ticket title and description
- Filter by labels, priority, and status
- Show search results in a dropdown
- Keyboard navigation support (Cmd/Ctrl+K)

## Technical Notes
- Use Fuse.js for fuzzy search
- Debounce search input (300ms)"#,
        &backlog_id,
        Priority::Low,
        vec!["frontend", "feature"],
    )?;

    // === READY (2 tickets) ===
    create_ticket(
        "Create user settings page",
        r#"## Overview
Build a comprehensive settings page for user preferences.

## Sections
1. **General** - Theme, language, timezone
2. **Notifications** - Email, push, in-app
3. **Privacy** - Data sharing, analytics opt-out
4. **Account** - Profile, password, 2FA

## Design
Follow the existing modal patterns. Use tabs for section navigation."#,
        &ready_id,
        Priority::Medium,
        vec!["frontend", "feature"],
    )?;

    create_ticket(
        "Add notification system",
        r#"## Overview
Implement a real-time notification system.

## Requirements
- Toast notifications for immediate feedback
- Notification center for history
- Mark as read/unread
- Sound toggle option

## Technical
- Use React Context for state
- WebSocket for real-time updates"#,
        &ready_id,
        Priority::High,
        vec!["frontend", "backend", "feature"],
    )?;

    // === IN PROGRESS (1 ticket with active run) ===
    let in_progress_ticket = create_ticket(
        "Implement authentication flow",
        r#"## Overview
Build a complete authentication system with login, registration, and password reset.

## Requirements
- Email/password authentication
- OAuth with Google and GitHub
- JWT token management
- Secure password hashing (bcrypt)
- Rate limiting on auth endpoints

## API Endpoints
- POST /api/auth/login
- POST /api/auth/register
- POST /api/auth/logout
- POST /api/auth/forgot-password
- POST /api/auth/reset-password

## Security
- HTTP-only cookies for tokens
- CSRF protection
- Input validation and sanitization"#,
        &in_progress_id,
        Priority::Urgent,
        vec!["backend", "auth", "feature"],
    )?;

    // Create an active run for the in-progress ticket
    let active_run = db
        .create_run(&CreateRun {
            ticket_id: in_progress_ticket.id.clone(),
            agent_type: AgentType::Cursor,
            repo_path: project.path.clone(),
            parent_run_id: None,
            stage: None,
            resumed_from_run_id: None,
        })
        .map_err(|e| format!("Failed to create run: {}", e))?;

    // Update to running status
    db.update_run_status(&active_run.id, RunStatus::Running, None, None)
        .map_err(|e| format!("Failed to update run status: {}", e))?;

    // Add some events to the active run
    let now = chrono::Utc::now();
    let events = vec![
        (
            EventType::RunStarted,
            "Agent started working on authentication implementation",
        ),
        (
            EventType::FileRead,
            "Reading existing auth configuration...",
        ),
        (
            EventType::FileEdited,
            "Created src/lib/auth/index.ts with base authentication module",
        ),
        (
            EventType::FileEdited,
            "Added JWT token utilities in src/lib/auth/jwt.ts",
        ),
        (
            EventType::CommandExecuted,
            "pnpm add bcryptjs jsonwebtoken",
        ),
        (
            EventType::FileEdited,
            "Implementing login endpoint in src/api/auth/login.ts",
        ),
    ];

    for (i, (event_type, message)) in events.into_iter().enumerate() {
        db.create_event(&NormalizedEvent {
            run_id: active_run.id.clone(),
            ticket_id: in_progress_ticket.id.clone(),
            agent_type: AgentType::Cursor,
            event_type,
            payload: AgentEventPayload {
                raw: Some(message.to_string()),
                structured: None,
            },
            timestamp: now + chrono::Duration::seconds(i as i64 * 30),
        })
        .map_err(|e| format!("Failed to create event: {}", e))?;
    }

    // === BLOCKED (1 ticket with error) ===
    let blocked_ticket = create_ticket(
        "Fix database connection issue",
        r#"## Problem
Production database connections are timing out intermittently.

## Error
```
Error: Connection timed out after 30000ms
    at PostgresClient.connect (pg-pool/index.js:123)
```

## Investigation
- Connection pool size: 10
- Max connections on DB: 100
- Current connections: ~85

## Possible Causes
- Connection leak in transaction handling
- Missing connection release in error paths
- Pool exhaustion under load"#,
        &blocked_id,
        Priority::Urgent,
        vec!["backend", "bugfix", "database"],
    )?;

    // Create a failed run for the blocked ticket
    let failed_run = db
        .create_run(&CreateRun {
            ticket_id: blocked_ticket.id.clone(),
            agent_type: AgentType::Claude,
            repo_path: project.path.clone(),
            parent_run_id: None,
            stage: None,
            resumed_from_run_id: None,
        })
        .map_err(|e| format!("Failed to create run: {}", e))?;

    db.update_run_status(
        &failed_run.id,
        RunStatus::Error,
        Some(1),
        Some("Failed to identify root cause - needs human review"),
    )
    .map_err(|e| format!("Failed to update run status: {}", e))?;

    // Add error event
    db.create_event(&NormalizedEvent {
        run_id: failed_run.id.clone(),
        ticket_id: blocked_ticket.id.clone(),
        agent_type: AgentType::Claude,
        event_type: EventType::Error,
        payload: AgentEventPayload {
            raw: Some(
                "Unable to reproduce connection timeout in local environment. \
                 The issue may be related to production-specific configuration."
                    .to_string(),
            ),
            structured: None,
        },
        timestamp: chrono::Utc::now(),
    })
    .map_err(|e| format!("Failed to create event: {}", e))?;

    // === REVIEW (2 tickets) ===
    let review_ticket1 = create_ticket(
        "Add unit tests for API endpoints",
        r#"## Overview
Comprehensive test coverage for all REST API endpoints.

## Test Categories
- Authentication endpoints
- User CRUD operations  
- Error handling and validation
- Rate limiting behavior

## Coverage Target
- Line coverage: 80%+
- Branch coverage: 70%+"#,
        &review_id,
        Priority::Medium,
        vec!["backend", "testing"],
    )?;

    // Create a finished run for review ticket
    let review_run = db
        .create_run(&CreateRun {
            ticket_id: review_ticket1.id.clone(),
            agent_type: AgentType::Cursor,
            repo_path: project.path.clone(),
            parent_run_id: None,
            stage: None,
            resumed_from_run_id: None,
        })
        .map_err(|e| format!("Failed to create run: {}", e))?;

    db.update_run_status(
        &review_run.id,
        RunStatus::Finished,
        Some(0),
        Some("Added 47 unit tests covering all API endpoints. Coverage: 84% lines, 72% branches."),
    )
    .map_err(|e| format!("Failed to update run status: {}", e))?;

    create_ticket(
        "Update API documentation",
        r#"## Overview
Update OpenAPI/Swagger documentation for all endpoints.

## Tasks
- [ ] Document new authentication endpoints
- [ ] Add request/response examples
- [ ] Update error code descriptions
- [ ] Add rate limiting documentation

## Output
Generate updated swagger.json and host on /api/docs"#,
        &review_id,
        Priority::Low,
        vec!["docs", "backend"],
    )?;

    // === DONE (3 tickets) ===
    create_ticket(
        "Setup project structure",
        r#"## Completed
- Initialized Next.js 14 with App Router
- Configured TypeScript strict mode
- Set up Tailwind CSS with custom theme
- Added ESLint and Prettier
- Created folder structure following best practices"#,
        &done_id,
        Priority::High,
        vec!["setup", "frontend"],
    )?;

    create_ticket(
        "Configure CI/CD pipeline",
        r#"## Completed
- GitHub Actions workflow for PR checks
- Automated testing on push
- Docker build and push to registry
- Deployment to staging on merge to main
- Production deployment with manual approval"#,
        &done_id,
        Priority::High,
        vec!["devops", "setup"],
    )?;

    create_ticket(
        "Add structured logging",
        r#"## Completed
- Integrated Pino for JSON logging
- Added request ID correlation
- Configured log levels per environment
- Set up log aggregation with Datadog
- Added performance timing logs"#,
        &done_id,
        Priority::Medium,
        vec!["backend", "observability"],
    )?;

    // === Create an Epic ===
    let epic = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready_id.clone(),
            title: "User Management System".to_string(),
            description_md: r#"## Epic: User Management System

A comprehensive user management system including:
- User profiles with avatars
- Role-based access control (RBAC)
- Team and organization support
- Activity logging and audit trail

### Child Tickets
1. User profile CRUD
2. Role and permission system
3. Team management
4. Activity audit log"#
                .to_string(),
            priority: Priority::High,
            labels: vec!["epic".to_string(), "backend".to_string(), "auth".to_string()],
            project_id: Some(project.id.clone()),
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: true,
            epic_id: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .map_err(|e| format!("Failed to create epic: {}", e))?;

    // Create child tickets for the epic
    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: ready_id.clone(),
        title: "User profile CRUD operations".to_string(),
        description_md: "Implement create, read, update, delete for user profiles.".to_string(),
        priority: Priority::Medium,
        labels: vec!["backend".to_string()],
        project_id: Some(project.id.clone()),
        workflow_type: WorkflowType::default(),
        model: None,
        branch_name: None,
        is_epic: false,
        epic_id: Some(epic.id.clone()),
        depends_on_epic_id: None,
        depends_on_epic_ids: vec![],
        spec_version_id: None,
    })
    .map_err(|e| format!("Failed to create epic child: {}", e))?;

    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: backlog_id.clone(),
        title: "Role and permission system".to_string(),
        description_md: "Implement RBAC with predefined roles and custom permissions.".to_string(),
        priority: Priority::Medium,
        labels: vec!["backend".to_string(), "auth".to_string()],
        project_id: Some(project.id.clone()),
        workflow_type: WorkflowType::default(),
        model: None,
        branch_name: None,
        is_epic: false,
        epic_id: Some(epic.id.clone()),
        depends_on_epic_id: None,
        depends_on_epic_ids: vec![],
        spec_version_id: None,
    })
    .map_err(|e| format!("Failed to create epic child: {}", e))?;

    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: backlog_id.clone(),
        title: "Team management".to_string(),
        description_md: "Allow users to create and manage teams with shared access.".to_string(),
        priority: Priority::Low,
        labels: vec!["backend".to_string(), "feature".to_string()],
        project_id: Some(project.id.clone()),
        workflow_type: WorkflowType::default(),
        model: None,
        branch_name: None,
        is_epic: false,
        epic_id: Some(epic.id.clone()),
        depends_on_epic_id: None,
        depends_on_epic_ids: vec![],
        spec_version_id: None,
    })
    .map_err(|e| format!("Failed to create epic child: {}", e))?;

    tracing::info!("Demo data seeded successfully!");

    Ok(format!(
        "Demo data created successfully!\n\
         - Board: {} (ID: {})\n\
         - Project: {} (ID: {})\n\
         - 11 tickets across all columns\n\
         - 1 epic with 3 child tickets\n\
         - 1 active run with events\n\
         - 1 completed run\n\
         - 1 failed run",
        board.name, board.id, project.name, project.id
    ))
}

/// Clear all demo data (factory reset)
#[tauri::command]
pub async fn clear_demo_data(db: State<'_, Arc<Database>>) -> Result<String, String> {
    tracing::warn!("Clearing all demo data (factory reset)...");
    db.factory_reset()
        .map_err(|e| format!("Failed to clear data: {}", e))?;
    Ok("All data cleared successfully".to_string())
}
