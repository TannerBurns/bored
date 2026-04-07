use std::sync::Arc;

use tokio::sync::broadcast;

use super::apply_output::apply_ticket_builder_output;
use super::apply_updates::apply_ticket_updates;
use crate::agents::chat::{
    TicketBuilderEpic, TicketBuilderOutput, TicketBuilderTask, TicketBuilderTicket,
    TicketBuilderUpdate,
};
use crate::db::models::{CreateTask, CreateTicket, UpdateTicket, WorkflowType};

fn unique_path(suffix: &str) -> String {
    let p = std::env::temp_dir().join(format!(
        "test-chat-tb-{}-{}",
        suffix,
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p.to_string_lossy().to_string()
}

#[test]
fn apply_ticket_updates_skips_done_column_tickets() {
    let db = Arc::new(crate::db::Database::open_in_memory().unwrap());
    let (event_tx, _) = broadcast::channel(16);

    let project = db
        .create_project(&crate::db::models::CreateProject {
            name: "P".into(),
            path: unique_path("tb-skip-done"),
            requires_git: false,
        })
        .unwrap();
    let board = db.create_board("B").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog_id = columns.iter().find(|c| c.name == "Backlog").unwrap().id.clone();
    let done_id = columns.iter().find(|c| c.name == "Done").unwrap().id.clone();

    let ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: backlog_id,
            title: "Done ticket".into(),
            description_md: "orig".into(),
            priority: crate::db::models::Priority::Medium,
            labels: vec![],
            project_id: Some(project.id.clone()),
            workspace_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

    db.update_ticket(
        &ticket.id,
        &UpdateTicket {
            column_id: Some(done_id),
            ..Default::default()
        },
    )
    .unwrap();

    let updates = vec![TicketBuilderUpdate {
        ticket_id: ticket.id.clone(),
        title: Some("Renamed".into()),
        description: None,
        priority: None,
        tasks: None,
        epic_id: None,
    }];

    let mut updated_ids = Vec::new();
    let mut summary_lines = Vec::new();
    apply_ticket_updates(
        &db,
        &event_tx,
        &updates,
        &mut updated_ids,
        &mut summary_lines,
    )
    .unwrap();

    assert!(updated_ids.is_empty());
    assert_eq!(summary_lines.len(), 1);
    assert!(summary_lines[0].contains("Skipped"));
    let t = db.get_ticket(&ticket.id).unwrap();
    assert_eq!(t.title, "Done ticket");
}

#[test]
fn apply_ticket_builder_creates_epic_and_children() {
    let db = Arc::new(crate::db::Database::open_in_memory().unwrap());
    let (event_tx, _) = broadcast::channel(16);

    let project = db
        .create_project(&crate::db::models::CreateProject {
            name: "P".into(),
            path: unique_path("tb-epic-create"),
            requires_git: false,
        })
        .unwrap();
    let board = db.create_board("B").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog_id = columns.iter().find(|c| c.name == "Backlog").unwrap().id.clone();

    let chat = db
        .create_chat(&crate::db::models::CreateChat {
            agent_type: "claude".into(),
            project_id: Some(project.id.clone()),
            workspace_id: None,
            mode: crate::db::models::ChatMode::TicketBuilder,
            board_id: Some(board.id.clone()),
            ticket_id: None,
            spec_id: None,
            model: None,
        })
        .unwrap();

    let output = TicketBuilderOutput {
        tickets: vec![],
        epics: vec![TicketBuilderEpic {
            id: None,
            name: "Auth".into(),
            description: None,
            tickets: vec![TicketBuilderTicket {
                id: None,
                title: "Login".into(),
                description: "Build login".into(),
                priority: Some("high".into()),
                tasks: None,
            }],
        }],
        updates: vec![],
    };

    let ids = apply_ticket_builder_output(
        &db,
        &event_tx,
        &chat.id,
        &chat,
        &board.id,
        &backlog_id,
        output,
    )
    .unwrap();

    assert_eq!(ids.len(), 2);
    let epic = db.get_ticket(&ids[0]).unwrap();
    assert!(epic.is_epic);
    let child = db.get_ticket(&ids[1]).unwrap();
    assert_eq!(child.epic_id.as_deref(), Some(epic.id.as_str()));
}

#[test]
fn apply_ticket_builder_links_existing_ticket_under_epic() {
    let db = Arc::new(crate::db::Database::open_in_memory().unwrap());
    let (event_tx, _) = broadcast::channel(16);

    let project = db
        .create_project(&crate::db::models::CreateProject {
            name: "P".into(),
            path: unique_path("tb-link"),
            requires_git: false,
        })
        .unwrap();
    let board = db.create_board("B").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog_id = columns.iter().find(|c| c.name == "Backlog").unwrap().id.clone();

    let epic = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: backlog_id.clone(),
            title: "Epic".into(),
            description_md: "".into(),
            priority: crate::db::models::Priority::Medium,
            labels: vec![],
            project_id: Some(project.id.clone()),
            workspace_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: true,
            epic_id: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

    let loose = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: backlog_id.clone(),
            title: "Loose".into(),
            description_md: "d".into(),
            priority: crate::db::models::Priority::Medium,
            labels: vec![],
            project_id: Some(project.id.clone()),
            workspace_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

    let chat = db
        .create_chat(&crate::db::models::CreateChat {
            agent_type: "claude".into(),
            project_id: Some(project.id.clone()),
            workspace_id: None,
            mode: crate::db::models::ChatMode::TicketBuilder,
            board_id: Some(board.id.clone()),
            ticket_id: None,
            spec_id: None,
            model: None,
        })
        .unwrap();

    let output = TicketBuilderOutput {
        tickets: vec![],
        epics: vec![TicketBuilderEpic {
            id: Some(epic.id.clone()),
            name: String::new(),
            description: None,
            tickets: vec![TicketBuilderTicket {
                id: Some(loose.id.clone()),
                title: String::new(),
                description: String::new(),
                priority: None,
                tasks: None,
            }],
        }],
        updates: vec![],
    };

    apply_ticket_builder_output(
        &db,
        &event_tx,
        &chat.id,
        &chat,
        &board.id,
        &backlog_id,
        output,
    )
    .unwrap();

    let t = db.get_ticket(&loose.id).unwrap();
    assert_eq!(t.epic_id.as_deref(), Some(epic.id.as_str()));
}

#[test]
fn apply_ticket_builder_rejects_ticket_id_on_standalone_list() {
    let db = Arc::new(crate::db::Database::open_in_memory().unwrap());
    let (event_tx, _) = broadcast::channel(16);

    let project = db
        .create_project(&crate::db::models::CreateProject {
            name: "P".into(),
            path: unique_path("tb-standalone-id"),
            requires_git: false,
        })
        .unwrap();
    let board = db.create_board("B").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog_id = columns.iter().find(|c| c.name == "Backlog").unwrap().id.clone();

    let chat = db
        .create_chat(&crate::db::models::CreateChat {
            agent_type: "claude".into(),
            project_id: Some(project.id.clone()),
            workspace_id: None,
            mode: crate::db::models::ChatMode::TicketBuilder,
            board_id: Some(board.id.clone()),
            ticket_id: None,
            spec_id: None,
            model: None,
        })
        .unwrap();

    let output = TicketBuilderOutput {
        tickets: vec![TicketBuilderTicket {
            id: Some("any-id".into()),
            title: "T".into(),
            description: "D".into(),
            priority: None,
            tasks: None,
        }],
        epics: vec![],
        updates: vec![],
    };

    let err = apply_ticket_builder_output(
        &db,
        &event_tx,
        &chat.id,
        &chat,
        &board.id,
        &backlog_id,
        output,
    )
    .unwrap_err();

    assert!(err.contains("nested under an epic"));
}

#[test]
fn apply_ticket_updates_replaces_tasks() {
    let db = Arc::new(crate::db::Database::open_in_memory().unwrap());
    let (event_tx, _) = broadcast::channel(16);

    let project = db
        .create_project(&crate::db::models::CreateProject {
            name: "P".into(),
            path: unique_path("tb-replace-tasks"),
            requires_git: false,
        })
        .unwrap();
    let board = db.create_board("B").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog_id = columns.iter().find(|c| c.name == "Backlog").unwrap().id.clone();

    let ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: backlog_id,
            title: "T".into(),
            description_md: "d".into(),
            priority: crate::db::models::Priority::Medium,
            labels: vec![],
            project_id: Some(project.id.clone()),
            workspace_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

    db.create_task(&CreateTask {
        ticket_id: ticket.id.clone(),
        task_type: Default::default(),
        title: Some("old1".into()),
        content: None,
    })
    .unwrap();
    db.create_task(&CreateTask {
        ticket_id: ticket.id.clone(),
        task_type: Default::default(),
        title: Some("old2".into()),
        content: None,
    })
    .unwrap();

    let updates = vec![TicketBuilderUpdate {
        ticket_id: ticket.id.clone(),
        title: None,
        description: None,
        priority: None,
        tasks: Some(vec![TicketBuilderTask {
            title: "only".into(),
            content: Some("new body".into()),
        }]),
        epic_id: None,
    }];

    let mut updated_ids = Vec::new();
    let mut summary_lines = Vec::new();
    apply_ticket_updates(
        &db,
        &event_tx,
        &updates,
        &mut updated_ids,
        &mut summary_lines,
    )
    .unwrap();

    assert_eq!(updated_ids.len(), 1);
    let tasks = db.get_tasks_for_ticket(&ticket.id).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].title.as_deref(), Some("only"));
}
