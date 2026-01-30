use crate::db::{Database, DbError, parse_datetime};
use crate::db::models::{AgentEvent, NormalizedEvent, EventType, AgentEventPayload};

impl Database {
    pub fn create_event(&self, event: &NormalizedEvent) -> Result<AgentEvent, DbError> {
        self.with_conn(|conn| {
            let event_id = uuid::Uuid::new_v4().to_string();
            let payload_json = serde_json::to_string(&event.payload)
                .unwrap_or_else(|_| "{}".to_string());
            
            conn.execute(
                r#"INSERT INTO agent_events 
                   (id, run_id, ticket_id, event_type, payload_json, created_at)
                   VALUES (?, ?, ?, ?, ?, ?)"#,
                rusqlite::params![
                    event_id,
                    event.run_id,
                    event.ticket_id,
                    event.event_type.as_str(),
                    payload_json,
                    event.timestamp.to_rfc3339(),
                ],
            )?;

            Ok(AgentEvent {
                id: event_id,
                run_id: event.run_id.clone(),
                ticket_id: event.ticket_id.clone(),
                event_type: event.event_type.clone(),
                payload: event.payload.clone(),
                created_at: event.timestamp,
            })
        })
    }

    pub fn get_events(&self, run_id: &str) -> Result<Vec<AgentEvent>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, run_id, ticket_id, event_type, payload_json, created_at
                   FROM agent_events WHERE run_id = ? ORDER BY created_at"#
            )?;
            
            let events = stmt.query_map([run_id], |row| {
                let event_type_str: String = row.get(3)?;
                let payload_json: String = row.get(4)?;
                let payload: AgentEventPayload = serde_json::from_str(&payload_json)
                    .unwrap_or(AgentEventPayload { raw: None, structured: None });
                
                Ok(AgentEvent {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    ticket_id: row.get(2)?,
                    event_type: EventType::parse(&event_type_str),
                    payload,
                    created_at: parse_datetime(row.get(5)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
            
            Ok(events)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{CreateTicket, CreateRun, Priority, AgentType, WorkflowType};

    fn create_test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn create_and_get_events() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Low,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let run = db.create_run(&CreateRun {
            ticket_id: ticket.id.clone(),
            agent_type: AgentType::Cursor,
            repo_path: "/tmp".to_string(),
            parent_run_id: None,
            stage: None,
        }).unwrap();
        
        let event = db.create_event(&NormalizedEvent {
            run_id: run.id.clone(),
            ticket_id: ticket.id.clone(),
            agent_type: AgentType::Cursor,
            event_type: EventType::FileEdited,
            payload: AgentEventPayload {
                raw: Some("edited file.txt".to_string()),
                structured: None,
            },
            timestamp: chrono::Utc::now(),
        }).unwrap();
        
        assert_eq!(event.event_type, EventType::FileEdited);
        
        let events = db.get_events(&run.id).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, event.id);
        assert_eq!(events[0].payload.raw, Some("edited file.txt".to_string()));
    }
}
