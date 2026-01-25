use std::sync::Arc;
use tokio::sync::broadcast;
use crate::db::Database;

/// Event sent to connected clients via SSE
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LiveEvent {
    TicketCreated {
        ticket_id: String,
        board_id: String,
    },
    TicketUpdated {
        ticket_id: String,
    },
    TicketMoved {
        ticket_id: String,
        from_column_id: String,
        to_column_id: String,
    },
    TicketDeleted {
        ticket_id: String,
        board_id: String,
    },
    CommentAdded {
        ticket_id: String,
        comment_id: String,
    },
    RunStarted {
        run_id: String,
        ticket_id: String,
        agent_type: String,
    },
    RunUpdated {
        run_id: String,
        status: String,
    },
    RunCompleted {
        run_id: String,
        ticket_id: String,
        status: String,
        exit_code: Option<i32>,
    },
    EventReceived {
        run_id: String,
        event_id: String,
        event_type: String,
    },
    TicketLocked {
        ticket_id: String,
        run_id: String,
    },
    TicketUnlocked {
        ticket_id: String,
    },
}

/// Shared application state for the API server
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub api_token: String,
    pub event_tx: broadcast::Sender<LiveEvent>,
}

impl AppState {
    pub fn new(db: Arc<Database>, api_token: String) -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self { db, api_token, event_tx }
    }

    pub fn broadcast(&self, event: LiveEvent) {
        tracing::debug!("Broadcasting event: {:?}", event);
        let _ = self.event_tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LiveEvent> {
        self.event_tx.subscribe()
    }
}
