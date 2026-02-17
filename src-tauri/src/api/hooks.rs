//! Hook event handler — receives raw events from agent hook scripts,
//! normalizes them via the `AgentProvider` trait, and returns an action.

use axum::{extract::State, Json};

use super::error::{ApiResult, AppError};
use super::state::{AppState, LiveEvent};
use super::types::{HookEventRequest, HookEventResponse};
use crate::db;

pub async fn hook_event(
    State(state): State<AppState>,
    Json(req): Json<HookEventRequest>,
) -> ApiResult<Json<HookEventResponse>> {
    let registry = state
        .agent_registry
        .as_ref()
        .ok_or_else(|| AppError::internal("Agent registry not configured"))?;

    let provider = registry.get(&req.agent_type).ok_or_else(|| {
        AppError::bad_request(format!("Unknown agent type: {}", req.agent_type))
    })?;

    let normalized = provider.normalize_hook_event(&req.raw_event_type, &req.raw_payload);

    let ticket_id = req
        .ticket_id
        .clone()
        .or_else(|| {
            state
                .db
                .get_run(&req.run_id)
                .ok()
                .map(|run| run.ticket_id)
        });

    let db_event = db::NormalizedEvent {
        run_id: req.run_id.clone(),
        ticket_id: ticket_id.clone().unwrap_or_default(),
        agent_type: req.agent_type.clone(),
        event_type: db::EventType::parse(&normalized.event_type),
        payload: db::AgentEventPayload {
            raw: Some(req.raw_payload.to_string()),
            structured: Some(normalized.structured),
        },
        timestamp: req.timestamp,
    };

    if let Ok(event) = state.db.create_event(&db_event) {
        state.broadcast(LiveEvent::EventReceived {
            run_id: req.run_id.clone(),
            event_id: event.id,
            event_type: normalized.event_type.clone(),
        });
    }

    // Handle stop events: update run status, unlock ticket, broadcast
    let is_stop_event = normalized.event_type == "run_stopped";
    let stop_result = if is_stop_event {
        let result = provider.normalize_stop_event(&req.raw_payload);
        update_run_on_stop(&state, &req.run_id, &result, ticket_id.as_deref());
        Some(result)
    } else {
        None
    };

    // Lifecycle events need no action; all others go through the provider.
    let action = if is_stop_event || normalized.event_type == "run_started" {
        crate::agents::HookAction::NoAction
    } else {
        provider.hook_action(
            &req.raw_event_type,
            &req.raw_payload,
            ticket_id.as_deref(),
            Some(&req.run_id),
        )
    };

    Ok(Json(HookEventResponse {
        action,
        stop_result,
    }))
}

/// Update run status in the database when a stop event is received.
/// Unlocks the ticket and broadcasts completion events for terminal statuses.
fn update_run_on_stop(
    state: &AppState,
    run_id: &str,
    result: &crate::agents::StopEventResult,
    ticket_id: Option<&str>,
) {
    let Some(status) = db::RunStatus::parse(&result.status) else {
        tracing::warn!(
            "Hook stop event had unrecognized status '{}' for run {}",
            result.status,
            run_id,
        );
        return;
    };

    if let Err(e) = state.db.update_run_status(
        run_id,
        status.clone(),
        Some(result.exit_code),
        Some(&result.summary),
    ) {
        tracing::warn!("Failed to update run status for {}: {}", run_id, e);
        return;
    }

    let is_terminal = matches!(
        status,
        db::RunStatus::Finished | db::RunStatus::Error | db::RunStatus::Aborted
    );

    if !is_terminal {
        return;
    }

    if let Some(tid) = ticket_id {
        if let Ok(ticket) = state.db.get_ticket(tid) {
            if ticket.locked_by_run_id.as_deref() == Some(run_id) {
                if let Err(e) = state.db.unlock_ticket(tid) {
                    tracing::warn!("Failed to unlock ticket {}: {}", tid, e);
                }
                state.broadcast(LiveEvent::TicketUnlocked {
                    ticket_id: tid.to_string(),
                });
            }
        }
    }

    state.broadcast(LiveEvent::RunCompleted {
        run_id: run_id.to_string(),
        ticket_id: ticket_id.unwrap_or_default().to_string(),
        status: result.status.clone(),
        exit_code: Some(result.exit_code),
    });
}
