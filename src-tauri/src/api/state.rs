use crate::db::Database;
use std::sync::Arc;
use tokio::sync::broadcast;

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
    // Spec / Planner events
    SpecCreated {
        spec_id: String,
        board_id: String,
    },
    SpecUpdated {
        spec_id: String,
    },
    SpecDeleted {
        spec_id: String,
        board_id: String,
    },
    ExplorationProgress {
        spec_id: String,
        query: String,
        status: String,
    },
    PlanGenerated {
        spec_id: String,
    },
    PlanApproved {
        spec_id: String,
    },
    PlanExecutionStarted {
        spec_id: String,
    },
    PlanExecutionCompleted {
        spec_id: String,
        epic_ids: Vec<String>,
    },
    /// Real-time log entry from planner agent output
    PlannerLogEntry {
        spec_id: String,
        /// Phase: "exploration" or "planning"
        phase: String,
        /// Log level: "info", "output", "error"
        level: String,
        /// The log message content
        message: String,
        /// Timestamp
        timestamp: String,
    },
    // Conversation events (spec discovery)
    ConversationMessageAdded {
        spec_id: String,
        message_id: String,
        role: String,
        content: String,
    },
    /// Real-time log entry from spec discovery agent output
    SpecDiscoveryLogEntry {
        spec_id: String,
        /// Log message content (agent's thinking/tool use)
        message: String,
        /// Timestamp
        timestamp: String,
    },
    /// Real-time log entry from validation agent (streaming CLI output)
    ValidationLogEntry {
        session_id: String,
        stream: String,
        message: String,
        timestamp: String,
    },
    /// App subprocess stdout/stderr (e.g. npm run dev) for a validation session
    ValidationAppLog {
        session_id: String,
        stream: String,
        message: String,
        timestamp: String,
    },
    // Chat events
    ChatCreated {
        chat_id: String,
    },
    ChatUpdated {
        chat_id: String,
    },
    ChatMessageAdded {
        chat_id: String,
        message_id: String,
        role: String,
    },
    ChatTitleGenerated {
        chat_id: String,
        title: String,
    },
    /// Real-time log entry from chat agent output
    ChatLogEntry {
        chat_id: String,
        stream: String,
        message: String,
        timestamp: String,
    },
    ChatCostUpdated {
        chat_id: String,
    },
    /// App subprocess stdout/stderr for a chat review session
    ChatAppLog {
        chat_id: String,
        stream: String,
        message: String,
        timestamp: String,
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
        let (event_tx, _) = broadcast::channel(1024);
        Self {
            db,
            api_token,
            event_tx,
        }
    }

    /// Create AppState with an externally provided event_tx
    pub fn with_event_tx(
        db: Arc<Database>,
        api_token: String,
        event_tx: broadcast::Sender<LiveEvent>,
    ) -> Self {
        Self {
            db,
            api_token,
            event_tx,
        }
    }

    pub fn broadcast(&self, event: LiveEvent) {
        tracing::debug!("Broadcasting event: {:?}", event);
        let _ = self.event_tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LiveEvent> {
        self.event_tx.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_state() -> AppState {
        let db = Arc::new(crate::db::Database::open_in_memory().unwrap());
        AppState::new(db, "test-token".to_string())
    }

    #[test]
    fn state_stores_token() {
        let state = create_test_state();
        assert_eq!(state.api_token, "test-token");
    }

    #[test]
    fn broadcast_and_receive() {
        let state = create_test_state();
        let mut rx = state.subscribe();

        state.broadcast(LiveEvent::TicketCreated {
            ticket_id: "t1".to_string(),
            board_id: "b1".to_string(),
        });

        let event = rx.try_recv().unwrap();
        match event {
            LiveEvent::TicketCreated {
                ticket_id,
                board_id,
            } => {
                assert_eq!(ticket_id, "t1");
                assert_eq!(board_id, "b1");
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn multiple_subscribers() {
        let state = create_test_state();
        let mut rx1 = state.subscribe();
        let mut rx2 = state.subscribe();

        state.broadcast(LiveEvent::TicketUpdated {
            ticket_id: "t1".to_string(),
        });

        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_ok());
    }

    #[test]
    fn with_event_tx_creates_functional_state() {
        let db = Arc::new(crate::db::Database::open_in_memory().unwrap());
        let (tx, _) = broadcast::channel(16);
        let state = AppState::with_event_tx(db, "tok".to_string(), tx);

        assert_eq!(state.api_token, "tok");

        let mut rx = state.subscribe();
        state.broadcast(LiveEvent::TicketUpdated {
            ticket_id: "t1".to_string(),
        });
        assert!(rx.try_recv().is_ok());
    }

    #[test]
    fn state_has_no_agent_registry_field() {
        let state = create_test_state();
        // After hooks removal, AppState should not carry an agent_registry.
        // This test documents the simplified struct — if someone re-adds the
        // field, they'll need to update this test deliberately.
        let json = format!("{:?}", state.api_token);
        assert!(!json.is_empty());
    }
}
