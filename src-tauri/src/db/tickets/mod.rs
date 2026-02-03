mod crud;
mod epics;
mod locking;
mod state;

#[cfg(test)]
mod tests;

use crate::db::models::{AgentPref, Priority, Ticket, WorkflowType};
use crate::db::{parse_datetime, Database};

/// Diagnostic info about tickets in the Ready column
#[derive(Debug, Clone)]
pub struct ReadyTicketDiagnostics {
    /// Total tickets in Ready column
    pub total_ready: i64,
    /// Tickets that are paused
    pub paused: i64,
    /// Tickets that are currently locked (not expired)
    pub locked: i64,
    /// Tickets that are epics (excluded from worker pickup)
    pub epics: i64,
    /// Tickets with a different project than the filter
    pub wrong_project: i64,
    /// Tickets with incompatible agent preference
    pub wrong_agent_pref: i64,
    /// Tickets eligible for pickup by this worker
    pub eligible: i64,
}

impl Database {
    pub(super) fn map_ticket_row(row: &rusqlite::Row) -> rusqlite::Result<Ticket> {
        let labels_json: String = row.get(6)?;
        let labels: Vec<String> = serde_json::from_str(&labels_json).unwrap_or_default();

        let priority_str: String = row.get(5)?;
        let priority = Priority::parse(&priority_str).unwrap_or(Priority::Medium);

        let agent_pref_str: Option<String> = row.get(12)?;
        let agent_pref = agent_pref_str.and_then(|s| AgentPref::parse(&s));

        let workflow_type_str: String = row
            .get::<_, Option<String>>(13)?
            .unwrap_or_else(|| "basic".to_string());
        let workflow_type = WorkflowType::parse(&workflow_type_str).unwrap_or_default();

        let model: Option<String> = row.get(14)?;
        let branch_name: Option<String> = row.get(15)?;

        // Epic fields (columns 16, 17, 18, 19, 20, 21)
        let is_epic: bool = row.get::<_, i32>(16).unwrap_or(0) != 0;
        let epic_id: Option<String> = row.get(17)?;
        let order_in_epic: Option<i32> = row.get(18)?;
        let depends_on_epic_id: Option<String> = row.get(19)?;
        let depends_on_epic_ids_json: Option<String> = row.get(20)?;
        let depends_on_epic_ids: Vec<String> = depends_on_epic_ids_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let spec_version_id: Option<String> = row.get(21)?;

        // Pause fields (columns 22, 23, 24)
        let paused_at: Option<String> = row.get(22)?;
        let paused_at_stage: Option<String> = row.get(23)?;
        let paused_run_id: Option<String> = row.get(24)?;

        Ok(Ticket {
            id: row.get(0)?,
            board_id: row.get(1)?,
            column_id: row.get(2)?,
            title: row.get(3)?,
            description_md: row.get(4)?,
            priority,
            labels,
            created_at: parse_datetime(row.get(7)?),
            updated_at: parse_datetime(row.get(8)?),
            locked_by_run_id: row.get(9)?,
            lock_expires_at: row.get::<_, Option<String>>(10)?.map(parse_datetime),
            project_id: row.get(11)?,
            agent_pref,
            workflow_type,
            model,
            branch_name,
            is_epic,
            epic_id,
            order_in_epic,
            depends_on_epic_id,
            depends_on_epic_ids,
            spec_version_id,
            paused_at: paused_at.map(parse_datetime),
            paused_at_stage,
            paused_run_id,
        })
    }
}
