use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{Duration, Utc};
use serde::Deserialize;

use super::error::{ApiResult, AppError};
use super::state::{AppState, LiveEvent};
use super::types::*;
use crate::db::{
    AgentEvent, AgentEventPayload, AgentRun, Board, Column, Comment,
    CreateRun, CreateTicket, CreateComment, UpdateTicket, EventType,
    NormalizedEvent, RunStatus, Ticket, AuthorType,
};
use crate::lifecycle::{TicketState, TransitionPermission, can_transition};

pub async fn health() -> &'static str {
    "ok"
}

pub async fn health_detailed(
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let board_count = state.db.get_boards()
        .map(|b| b.len())
        .unwrap_or(0);

    Ok(Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "database": "connected",
        "boardCount": board_count
    })))
}

pub async fn list_boards(
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<Board>>> {
    let boards = state.db.get_boards()?;
    Ok(Json(boards))
}

pub async fn get_board(
    State(state): State<AppState>,
    Path(board_id): Path<String>,
) -> ApiResult<Json<BoardWithColumns>> {
    let board = state.db.get_board(&board_id)?
        .ok_or_else(|| AppError::not_found("Board"))?;
    let columns = state.db.get_columns(&board_id)?;
    
    Ok(Json(BoardWithColumns {
        id: board.id,
        name: board.name,
        default_project_id: board.default_project_id,
        created_at: board.created_at,
        updated_at: board.updated_at,
        columns,
    }))
}

pub async fn list_columns(
    State(state): State<AppState>,
    Path(board_id): Path<String>,
) -> ApiResult<Json<Vec<Column>>> {
    state.db.get_board(&board_id)?
        .ok_or_else(|| AppError::not_found("Board"))?;
    let columns = state.db.get_columns(&board_id)?;
    Ok(Json(columns))
}

#[derive(Debug, Deserialize)]
pub struct TicketQuery {
    pub column: Option<String>,
}

pub async fn list_tickets(
    State(state): State<AppState>,
    Path(board_id): Path<String>,
    Query(query): Query<TicketQuery>,
) -> ApiResult<Json<Vec<Ticket>>> {
    state.db.get_board(&board_id)?
        .ok_or_else(|| AppError::not_found("Board"))?;
    let tickets = state.db.get_tickets(&board_id, query.column.as_deref())?;
    Ok(Json(tickets))
}

pub async fn create_ticket(
    State(state): State<AppState>,
    Json(req): Json<CreateTicketRequest>,
) -> ApiResult<(StatusCode, Json<Ticket>)> {
    state.db.get_board(&req.board_id)?
        .ok_or_else(|| AppError::not_found("Board"))?;
    
    let columns = state.db.get_columns(&req.board_id)?;
    if !columns.iter().any(|c| c.id == req.column_id) {
        return Err(AppError::not_found("Column"));
    }

    if req.title.trim().is_empty() {
        return Err(AppError::validation("Title cannot be empty"));
    }

    let ticket = state.db.create_ticket(&CreateTicket {
        board_id: req.board_id.clone(),
        column_id: req.column_id,
        title: req.title,
        description_md: req.description_md,
        priority: req.priority,
        labels: req.labels,
        project_id: req.project_id,
        agent_pref: req.agent_pref,
        workflow_type: req.workflow_type.unwrap_or_default(),
        model: req.model,
        branch_name: req.branch_name,
    })?;

    state.broadcast(LiveEvent::TicketCreated {
        ticket_id: ticket.id.clone(),
        board_id: req.board_id,
    });

    Ok((StatusCode::CREATED, Json(ticket)))
}

pub async fn get_ticket(
    State(state): State<AppState>,
    Path(ticket_id): Path<String>,
) -> ApiResult<Json<Ticket>> {
    let ticket = state.db.get_ticket(&ticket_id)?;
    Ok(Json(ticket))
}

pub async fn update_ticket(
    State(state): State<AppState>,
    Path(ticket_id): Path<String>,
    Json(req): Json<UpdateTicketRequest>,
) -> ApiResult<Json<Ticket>> {
    if let Some(ref title) = req.title {
        if title.trim().is_empty() {
            return Err(AppError::validation("Title cannot be empty"));
        }
    }

    let ticket = state.db.update_ticket(&ticket_id, &UpdateTicket {
        title: req.title,
        description_md: req.description_md,
        priority: req.priority,
        labels: req.labels,
        project_id: req.project_id,
        agent_pref: req.agent_pref,
        workflow_type: req.workflow_type,
        model: req.model,
        branch_name: req.branch_name,
    })?;

    state.broadcast(LiveEvent::TicketUpdated {
        ticket_id: ticket.id.clone(),
    });

    Ok(Json(ticket))
}

pub async fn delete_ticket(
    State(state): State<AppState>,
    Path(ticket_id): Path<String>,
) -> ApiResult<Json<DeleteResponse>> {
    let ticket = state.db.get_ticket(&ticket_id)?;
    state.db.delete_ticket(&ticket_id)?;

    state.broadcast(LiveEvent::TicketDeleted {
        ticket_id: ticket_id.clone(),
        board_id: ticket.board_id,
    });

    Ok(Json(DeleteResponse {
        deleted: true,
        id: ticket_id,
    }))
}

pub async fn move_ticket(
    State(state): State<AppState>,
    Path(ticket_id): Path<String>,
    Json(req): Json<MoveTicketRequest>,
) -> ApiResult<Json<Ticket>> {
    let ticket = state.db.get_ticket(&ticket_id)?;
    let from_column_id = ticket.column_id.clone();

    let columns = state.db.get_columns(&ticket.board_id)?;
    
    let current_column = columns.iter()
        .find(|c| c.id == from_column_id)
        .ok_or_else(|| AppError::not_found("Current column"))?;
    
    let target_column = columns.iter()
        .find(|c| c.id == req.column_id)
        .ok_or_else(|| AppError::not_found("Target column"))?;

    let current_state = TicketState::from_column_name(&current_column.name)
        .ok_or_else(|| AppError::validation(format!(
            "Unknown column state: {}", current_column.name
        )))?;

    let target_state = TicketState::from_column_name(&target_column.name)
        .ok_or_else(|| AppError::validation(format!(
            "Unknown target state: {}", target_column.name
        )))?;

    let is_locked = ticket.locked_by_run_id.is_some() 
        && ticket.lock_expires_at.is_some_and(|exp| exp > Utc::now());
    match can_transition(current_state, target_state, is_locked, false) {
        TransitionPermission::Allowed => {}
        TransitionPermission::RequiresUnlock => {
            return Err(AppError::conflict("Ticket is locked by an active run"));
        }
        TransitionPermission::Denied(reason) => {
            return Err(AppError::validation(reason));
        }
    }

    state.db.move_ticket(&ticket_id, &req.column_id)?;
    let updated = state.db.get_ticket(&ticket_id)?;

    state.broadcast(LiveEvent::TicketMoved {
        ticket_id,
        from_column_id,
        to_column_id: req.column_id,
    });

    Ok(Json(updated))
}

pub async fn reserve_ticket(
    State(state): State<AppState>,
    Path(ticket_id): Path<String>,
    Json(req): Json<ReserveTicketRequest>,
) -> ApiResult<Json<ReservationResponse>> {
    let ticket = state.db.get_ticket(&ticket_id)?;

    if let Some(ref lock_expires) = ticket.lock_expires_at {
        if *lock_expires > Utc::now() {
            return Err(AppError::conflict("Ticket is already locked by another run"));
        }
    }

    let repo_path = req.repo_path
        .ok_or_else(|| AppError::validation("repo_path is required"))?;

    let run = state.db.create_run(&CreateRun {
        ticket_id: ticket_id.clone(),
        agent_type: req.agent_type,
        repo_path,
        parent_run_id: None,
        stage: None,
    })?;

    let lock_expires_at = Utc::now() + Duration::minutes(LOCK_DURATION_MINUTES);
    state.db.lock_ticket(&ticket_id, &run.id, lock_expires_at)?;

    let columns = state.db.get_columns(&ticket.board_id)?;
    if let Some(in_progress) = columns.iter().find(|c| c.name == "In Progress") {
        let _ = state.db.move_ticket(&ticket_id, &in_progress.id);
    }

    state.broadcast(LiveEvent::TicketLocked {
        ticket_id: ticket_id.clone(),
        run_id: run.id.clone(),
    });

    state.broadcast(LiveEvent::RunStarted {
        run_id: run.id.clone(),
        ticket_id: ticket_id.clone(),
        agent_type: req.agent_type.as_str().to_string(),
    });

    Ok(Json(ReservationResponse {
        run_id: run.id,
        ticket_id,
        lock_expires_at,
        heartbeat_interval_secs: HEARTBEAT_INTERVAL_SECS,
    }))
}

pub async fn create_run(
    State(state): State<AppState>,
    Json(req): Json<CreateRunRequest>,
) -> ApiResult<(StatusCode, Json<AgentRun>)> {
    let ticket = state.db.get_ticket(&req.ticket_id)?;

    if let Some(ref lock_expires) = ticket.lock_expires_at {
        if *lock_expires > Utc::now() {
            return Err(AppError::conflict("Ticket is already locked"));
        }
    }

    let run = state.db.create_run(&CreateRun {
        ticket_id: req.ticket_id.clone(),
        agent_type: req.agent_type,
        repo_path: req.repo_path,
        parent_run_id: None,
        stage: None,
    })?;

    // Lock the ticket to prevent concurrent runs
    let lock_expires_at = Utc::now() + Duration::minutes(LOCK_DURATION_MINUTES);
    state.db.lock_ticket(&req.ticket_id, &run.id, lock_expires_at)?;

    // Move to In Progress if that column exists
    let columns = state.db.get_columns(&ticket.board_id)?;
    if let Some(in_progress) = columns.iter().find(|c| c.name == "In Progress") {
        let _ = state.db.move_ticket(&req.ticket_id, &in_progress.id);
    }

    state.broadcast(LiveEvent::TicketLocked {
        ticket_id: req.ticket_id.clone(),
        run_id: run.id.clone(),
    });

    state.broadcast(LiveEvent::RunStarted {
        run_id: run.id.clone(),
        ticket_id: req.ticket_id,
        agent_type: req.agent_type.as_str().to_string(),
    });

    Ok((StatusCode::CREATED, Json(run)))
}

pub async fn get_run(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> ApiResult<Json<AgentRun>> {
    let run = state.db.get_run(&run_id)?;
    Ok(Json(run))
}

pub async fn list_runs(
    State(state): State<AppState>,
    Path(ticket_id): Path<String>,
) -> ApiResult<Json<Vec<AgentRun>>> {
    state.db.get_ticket(&ticket_id)?;
    let runs = state.db.get_runs(&ticket_id)?;
    Ok(Json(runs))
}

pub async fn update_run(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Json(req): Json<UpdateRunRequest>,
) -> ApiResult<Json<AgentRun>> {
    let existing = state.db.get_run(&run_id)?;

    let status = req.status
        .as_ref()
        .and_then(|s| RunStatus::parse(s))
        .unwrap_or(existing.status.clone());

    state.db.update_run_status(
        &run_id,
        status.clone(),
        req.exit_code,
        req.summary_md.as_deref(),
    )?;

    let updated = state.db.get_run(&run_id)?;

    if matches!(status, RunStatus::Finished | RunStatus::Error | RunStatus::Aborted) {
        if let Ok(ticket) = state.db.get_ticket(&existing.ticket_id) {
            if ticket.locked_by_run_id.as_ref() == Some(&run_id) {
                state.db.unlock_ticket(&existing.ticket_id)?;
                state.broadcast(LiveEvent::TicketUnlocked {
                    ticket_id: existing.ticket_id.clone(),
                });
            }
        }

        state.broadcast(LiveEvent::RunCompleted {
            run_id: run_id.clone(),
            ticket_id: existing.ticket_id,
            status: status.as_str().to_string(),
            exit_code: req.exit_code,
        });
    } else {
        state.broadcast(LiveEvent::RunUpdated {
            run_id,
            status: status.as_str().to_string(),
        });
    }

    Ok(Json(updated))
}

pub async fn heartbeat(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> ApiResult<Json<HeartbeatResponse>> {
    let run = state.db.get_run(&run_id)?;

    if !matches!(run.status, RunStatus::Queued | RunStatus::Running) {
        return Err(AppError::conflict(format!(
            "Run is not active (status: {})",
            run.status.as_str()
        )));
    }

    let new_expiry = Utc::now() + Duration::minutes(LOCK_DURATION_MINUTES);
    state.db.extend_lock(&run.ticket_id, &run_id, new_expiry)?;

    tracing::debug!("Heartbeat received for run {}, lock extended to {}", run_id, new_expiry);

    Ok(Json(HeartbeatResponse {
        run_id,
        lock_expires_at: new_expiry,
        ok: true,
    }))
}

pub async fn release_run(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> ApiResult<Json<AgentRun>> {
    let run = state.db.get_run(&run_id)?;

    state.db.update_run_status(&run_id, RunStatus::Aborted, None, None)?;

    if let Ok(ticket) = state.db.get_ticket(&run.ticket_id) {
        if ticket.locked_by_run_id.as_ref() == Some(&run_id) {
            state.db.unlock_ticket(&run.ticket_id)?;
            state.broadcast(LiveEvent::TicketUnlocked {
                ticket_id: run.ticket_id.clone(),
            });
        }
    }

    let updated = state.db.get_run(&run_id)?;

    state.broadcast(LiveEvent::RunCompleted {
        run_id: run_id.clone(),
        ticket_id: run.ticket_id,
        status: "aborted".to_string(),
        exit_code: None,
    });

    Ok(Json(updated))
}

pub async fn create_event(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Json(req): Json<CreateEventRequest>,
) -> ApiResult<(StatusCode, Json<AgentEvent>)> {
    let run = state.db.get_run(&run_id)?;

    let normalized = NormalizedEvent {
        run_id: run_id.clone(),
        ticket_id: run.ticket_id.clone(),
        agent_type: run.agent_type,
        event_type: EventType::parse(&req.event_type),
        payload: AgentEventPayload {
            raw: None,
            structured: Some(req.payload),
        },
        timestamp: req.timestamp,
    };

    let event = state.db.create_event(&normalized)?;

    state.broadcast(LiveEvent::EventReceived {
        run_id,
        event_id: event.id.clone(),
        event_type: req.event_type,
    });

    Ok((StatusCode::CREATED, Json(event)))
}

pub async fn list_events(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> ApiResult<Json<Vec<AgentEvent>>> {
    state.db.get_run(&run_id)?;
    let events = state.db.get_events(&run_id)?;
    Ok(Json(events))
}

pub async fn create_comment(
    State(state): State<AppState>,
    Path(ticket_id): Path<String>,
    Json(req): Json<CreateCommentRequest>,
) -> ApiResult<(StatusCode, Json<Comment>)> {
    state.db.get_ticket(&ticket_id)?;

    if req.body_md.trim().is_empty() {
        return Err(AppError::validation("Comment body cannot be empty"));
    }

    let author_type = match req.author_type.as_str() {
        "user" => AuthorType::User,
        "agent" => AuthorType::Agent,
        "system" => AuthorType::System,
        _ => return Err(AppError::validation("Invalid author_type")),
    };

    let comment = state.db.create_comment(&CreateComment {
        ticket_id: ticket_id.clone(),
        author_type,
        body_md: req.body_md,
        metadata: req.metadata,
    })?;

    state.broadcast(LiveEvent::CommentAdded {
        ticket_id,
        comment_id: comment.id.clone(),
    });

    Ok((StatusCode::CREATED, Json(comment)))
}

pub async fn list_comments(
    State(state): State<AppState>,
    Path(ticket_id): Path<String>,
) -> ApiResult<Json<Vec<Comment>>> {
    state.db.get_ticket(&ticket_id)?;
    let comments = state.db.get_comments(&ticket_id)?;
    Ok(Json(comments))
}

pub async fn queue_next(
    State(state): State<AppState>,
    Json(req): Json<QueueNextRequest>,
) -> ApiResult<Json<QueueNextResponse>> {
    let boards = match &req.board_id {
        Some(id) => {
            let board = state.db.get_board(id)?
                .ok_or_else(|| AppError::not_found("Board"))?;
            vec![board]
        }
        None => state.db.get_boards()?,
    };

    for board in boards {
        let columns = state.db.get_columns(&board.id)?;

        let ready_column = match columns.iter().find(|c| c.name == "Ready") {
            Some(c) => c,
            None => continue,
        };

        let tickets = state.db.get_tickets(&board.id, Some(&ready_column.id))?;

        for ticket in tickets {
            if let Some(ref lock_expires) = ticket.lock_expires_at {
                if *lock_expires > Utc::now() {
                    continue;
                }
            }

            if let Some(ref filter_path) = req.repo_path {
                // Look up project by filesystem path to get its ID
                let project = state.db.get_project_by_path(filter_path)?;
                let project_id = project.map(|p| p.id);
                if ticket.project_id != project_id {
                    continue;
                }
            }

            if let Some(ref pref) = ticket.agent_pref {
                use crate::db::AgentPref;
                match pref {
                    AgentPref::Cursor if req.agent_type != crate::db::AgentType::Cursor => continue,
                    AgentPref::Claude if req.agent_type != crate::db::AgentType::Claude => continue,
                    _ => {}
                }
            }

            // Use provided repo_path, or fall back to the ticket's project path
            let repo_path = req.repo_path.clone().or_else(|| {
                ticket.project_id.as_ref()
                    .and_then(|pid| state.db.get_project(pid).ok().flatten())
                    .map(|p| p.path)
            }).ok_or_else(|| AppError::validation(
                "repo_path is required when ticket has no associated project"
            ))?;

            let run = state.db.create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: req.agent_type,
                repo_path,
                parent_run_id: None,
                stage: None,
            })?;

            let lock_expires_at = Utc::now() + Duration::minutes(LOCK_DURATION_MINUTES);
            state.db.lock_ticket(&ticket.id, &run.id, lock_expires_at)?;

            if let Some(in_progress) = columns.iter().find(|c| c.name == "In Progress") {
                let _ = state.db.move_ticket(&ticket.id, &in_progress.id);
            }

            state.broadcast(LiveEvent::TicketLocked {
                ticket_id: ticket.id.clone(),
                run_id: run.id.clone(),
            });

            state.broadcast(LiveEvent::RunStarted {
                run_id: run.id.clone(),
                ticket_id: ticket.id.clone(),
                agent_type: req.agent_type.as_str().to_string(),
            });

            return Ok(Json(QueueNextResponse {
                ticket,
                run_id: run.id,
                lock_expires_at,
                heartbeat_interval_secs: HEARTBEAT_INTERVAL_SECS,
            }));
        }
    }

    Err(AppError::queue_empty())
}

pub async fn queue_status(
    State(state): State<AppState>,
) -> ApiResult<Json<QueueStatusResponse>> {
    let boards = state.db.get_boards()?;
    let mut total_ready = 0;
    let mut total_in_progress = 0;
    let mut board_statuses = Vec::new();

    for board in &boards {
        let columns = state.db.get_columns(&board.id)?;

        let ready_count = if let Some(ready_col) = columns.iter().find(|c| c.name == "Ready") {
            let tickets = state.db.get_tickets(&board.id, Some(&ready_col.id))?;
            tickets.iter().filter(|t| {
                t.lock_expires_at.is_none_or(|exp| exp <= Utc::now())
            }).count()
        } else {
            0
        };

        let in_progress_count = if let Some(ip_col) = columns.iter().find(|c| c.name == "In Progress") {
            state.db.get_tickets(&board.id, Some(&ip_col.id))?.len()
        } else {
            0
        };

        total_ready += ready_count;
        total_in_progress += in_progress_count;

        if ready_count > 0 {
            board_statuses.push(BoardQueueStatus {
                board_id: board.id.clone(),
                board_name: board.name.clone(),
                ready_count,
            });
        }
    }

    Ok(Json(QueueStatusResponse {
        ready_count: total_ready,
        in_progress_count: total_in_progress,
        boards: board_statuses,
    }))
}
