use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::Stream;
use std::{collections::HashSet, convert::Infallible, time::Duration};
use tokio_stream::StreamExt;

use super::state::{AppState, LiveEvent};

pub async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.subscribe();

    let stream =
        tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(|result| match result {
            Ok(event) => match serde_json::to_string(&event) {
                Ok(json) => Some(Ok(Event::default().data(json))),
                Err(e) => {
                    tracing::error!("Failed to serialize SSE event: {}", e);
                    None
                }
            },
            Err(e) => {
                tracing::warn!("SSE broadcast lag: {}", e);
                None
            }
        });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("ping"),
    )
}

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

    let type_filter: Option<HashSet<String>> = filter
        .types
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect());

    let ticket_filter = filter.ticket_id;
    let run_filter = filter.run_id;

    let stream =
        tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(move |result| match result {
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
        });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("ping"),
    )
}

fn event_matches_filter(
    event: &LiveEvent,
    type_filter: &Option<HashSet<String>>,
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
            LiveEvent::SpecCreated { .. } => "spec_created",
            LiveEvent::SpecUpdated { .. } => "spec_updated",
            LiveEvent::SpecDeleted { .. } => "spec_deleted",
            LiveEvent::ExplorationProgress { .. } => "exploration_progress",
            LiveEvent::PlanGenerated { .. } => "plan_generated",
            LiveEvent::PlanApproved { .. } => "plan_approved",
            LiveEvent::PlanExecutionStarted { .. } => "plan_execution_started",
            LiveEvent::PlanExecutionCompleted { .. } => "plan_execution_completed",
            LiveEvent::PlannerLogEntry { .. } => "planner_log_entry",
            LiveEvent::ConversationMessageAdded { .. } => "conversation_message_added",
            LiveEvent::SpecDiscoveryLogEntry { .. } => "spec_discovery_log_entry",
            LiveEvent::ValidationLogEntry { .. } => "validation_log_entry",
            LiveEvent::ValidationAppLog { .. } => "validation_app_log",
            LiveEvent::ChatCreated { .. } => "chat_created",
            LiveEvent::ChatUpdated { .. } => "chat_updated",
            LiveEvent::ChatMessageAdded { .. } => "chat_message_added",
            LiveEvent::ChatTitleGenerated { .. } => "chat_title_generated",
            LiveEvent::ChatLogEntry { .. } => "chat_log_entry",
            LiveEvent::ChatCostUpdated { .. } => "chat_cost_updated",
            LiveEvent::ChatAppLog { .. } => "chat_app_log",
        };

        if !types.contains(event_type) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chat_created(chat_id: &str) -> LiveEvent {
        LiveEvent::ChatCreated {
            chat_id: chat_id.to_string(),
        }
    }

    fn make_ticket_created(ticket_id: &str) -> LiveEvent {
        LiveEvent::TicketCreated {
            ticket_id: ticket_id.to_string(),
            board_id: "board-1".to_string(),
        }
    }

    fn make_run_started(ticket_id: &str, run_id: &str) -> LiveEvent {
        LiveEvent::RunStarted {
            ticket_id: ticket_id.to_string(),
            run_id: run_id.to_string(),
            agent_type: "claude".to_string(),
        }
    }

    #[test]
    fn no_filters_matches_everything() {
        let event = make_chat_created("c1");
        assert!(event_matches_filter(&event, &None, &None, &None));
    }

    #[test]
    fn type_filter_hashset_matches() {
        let filter: HashSet<String> =
            ["chat_created", "chat_updated"].iter().map(|s| s.to_string()).collect();

        let matching = make_chat_created("c1");
        assert!(event_matches_filter(&matching, &Some(filter.clone()), &None, &None));

        let non_matching = make_ticket_created("t1");
        assert!(!event_matches_filter(&non_matching, &Some(filter), &None, &None));
    }

    #[test]
    fn type_filter_single_entry() {
        let filter: HashSet<String> = ["ticket_created"].iter().map(|s| s.to_string()).collect();
        let event = make_ticket_created("t1");
        assert!(event_matches_filter(&event, &Some(filter), &None, &None));
    }

    #[test]
    fn ticket_filter_matches() {
        let event = make_ticket_created("t1");
        assert!(event_matches_filter(
            &event,
            &None,
            &Some("t1".to_string()),
            &None,
        ));
    }

    #[test]
    fn ticket_filter_rejects_mismatch() {
        let event = make_ticket_created("t1");
        assert!(!event_matches_filter(
            &event,
            &None,
            &Some("t-other".to_string()),
            &None,
        ));
    }

    #[test]
    fn run_filter_matches() {
        let event = make_run_started("t1", "r1");
        assert!(event_matches_filter(
            &event,
            &None,
            &None,
            &Some("r1".to_string()),
        ));
    }

    #[test]
    fn run_filter_rejects_mismatch() {
        let event = make_run_started("t1", "r1");
        assert!(!event_matches_filter(
            &event,
            &None,
            &None,
            &Some("r-other".to_string()),
        ));
    }

    #[test]
    fn combined_type_and_ticket_filter() {
        let type_filter: HashSet<String> =
            ["ticket_created"].iter().map(|s| s.to_string()).collect();
        let event = make_ticket_created("t1");
        assert!(event_matches_filter(
            &event,
            &Some(type_filter.clone()),
            &Some("t1".to_string()),
            &None,
        ));
        assert!(!event_matches_filter(
            &event,
            &Some(type_filter),
            &Some("t-other".to_string()),
            &None,
        ));
    }

    #[test]
    fn event_without_ticket_field_skips_ticket_filter() {
        let event = make_chat_created("c1");
        assert!(!event_matches_filter(
            &event,
            &None,
            &Some("t1".to_string()),
            &None,
        ));
    }
}
