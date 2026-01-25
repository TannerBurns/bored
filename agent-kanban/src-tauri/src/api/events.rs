use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::Stream;
use std::{convert::Infallible, time::Duration};
use tokio_stream::StreamExt;

use super::state::{AppState, LiveEvent};

/// Server-Sent Events endpoint for real-time updates
pub async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.subscribe();

    let stream = tokio_stream::wrappers::BroadcastStream::new(rx)
        .filter_map(|result| {
            match result {
                Ok(event) => {
                    match serde_json::to_string(&event) {
                        Ok(json) => Some(Ok(Event::default().data(json))),
                        Err(e) => {
                            tracing::error!("Failed to serialize SSE event: {}", e);
                            None
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("SSE broadcast lag: {}", e);
                    None
                }
            }
        });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("ping"),
    )
}

/// SSE endpoint with filtering
#[derive(Debug, serde::Deserialize)]
pub struct SseFilter {
    #[serde(default)]
    pub types: Option<String>,
    #[serde(default)]
    pub ticket_id: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
}

pub async fn sse_filtered(
    State(state): State<AppState>,
    axum::extract::Query(filter): axum::extract::Query<SseFilter>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.subscribe();

    let type_filter: Option<Vec<String>> = filter.types.map(|t| {
        t.split(',').map(|s| s.trim().to_string()).collect()
    });

    let ticket_filter = filter.ticket_id;
    let run_filter = filter.run_id;

    let stream = tokio_stream::wrappers::BroadcastStream::new(rx)
        .filter_map(move |result| {
            match result {
                Ok(event) => {
                    if !event_matches_filter(&event, &type_filter, &ticket_filter, &run_filter) {
                        return None;
                    }

                    match serde_json::to_string(&event) {
                        Ok(json) => Some(Ok(Event::default().data(json))),
                        Err(_) => None,
                    }
                }
                Err(_) => None,
            }
        });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("ping"),
    )
}

fn event_matches_filter(
    event: &LiveEvent,
    type_filter: &Option<Vec<String>>,
    ticket_filter: &Option<String>,
    run_filter: &Option<String>,
) -> bool {
    if let Some(ref types) = type_filter {
        let event_type = match event {
            LiveEvent::TicketCreated { .. } => "ticket_created",
            LiveEvent::TicketUpdated { .. } => "ticket_updated",
            LiveEvent::TicketMoved { .. } => "ticket_moved",
            LiveEvent::TicketDeleted { .. } => "ticket_deleted",
            LiveEvent::CommentAdded { .. } => "comment_added",
            LiveEvent::RunStarted { .. } => "run_started",
            LiveEvent::RunUpdated { .. } => "run_updated",
            LiveEvent::RunCompleted { .. } => "run_completed",
            LiveEvent::EventReceived { .. } => "event_received",
            LiveEvent::TicketLocked { .. } => "ticket_locked",
            LiveEvent::TicketUnlocked { .. } => "ticket_unlocked",
        };

        if !types.iter().any(|t| t == event_type) {
            return false;
        }
    }

    if let Some(ref ticket_id) = ticket_filter {
        let event_ticket = match event {
            LiveEvent::TicketCreated { ticket_id, .. } => Some(ticket_id),
            LiveEvent::TicketUpdated { ticket_id } => Some(ticket_id),
            LiveEvent::TicketMoved { ticket_id, .. } => Some(ticket_id),
            LiveEvent::TicketDeleted { ticket_id, .. } => Some(ticket_id),
            LiveEvent::CommentAdded { ticket_id, .. } => Some(ticket_id),
            LiveEvent::RunStarted { ticket_id, .. } => Some(ticket_id),
            LiveEvent::RunCompleted { ticket_id, .. } => Some(ticket_id),
            LiveEvent::TicketLocked { ticket_id, .. } => Some(ticket_id),
            LiveEvent::TicketUnlocked { ticket_id } => Some(ticket_id),
            _ => None,
        };

        if event_ticket != Some(ticket_id) {
            return false;
        }
    }

    if let Some(ref run_id) = run_filter {
        let event_run = match event {
            LiveEvent::RunStarted { run_id, .. } => Some(run_id),
            LiveEvent::RunUpdated { run_id, .. } => Some(run_id),
            LiveEvent::RunCompleted { run_id, .. } => Some(run_id),
            LiveEvent::EventReceived { run_id, .. } => Some(run_id),
            LiveEvent::TicketLocked { run_id, .. } => Some(run_id),
            _ => None,
        };

        if event_run != Some(run_id) {
            return false;
        }
    }

    true
}
