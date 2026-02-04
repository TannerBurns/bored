//! ETA calculation for spec work estimation.

use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;

use crate::db::{Database, SpecEta};

mod timing;

pub use timing::{calculate_remaining_time, calculate_timing_stats};

pub fn calculate_eta(db: &Arc<Database>, spec_id: &str) -> Result<SpecEta, String> {
    let spec = db.get_spec(spec_id).map_err(|e| e.to_string())?;
    let version = db
        .get_latest_spec_version(spec_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No version found for spec".to_string())?;

    let tickets = db
        .get_spec_version_tickets(&version.id)
        .map_err(|e| e.to_string())?;

    let child_tickets: Vec<_> = tickets.iter().filter(|t| !t.is_epic).collect();

    let total_tickets = child_tickets.len();

    let columns = if let Some(ref target_board_id) = spec.target_board_id {
        db.get_columns(target_board_id).ok()
    } else {
        db.get_columns(&spec.board_id).ok()
    }
    .unwrap_or_default();

    let column_names: HashMap<String, String> = columns
        .iter()
        .map(|c| (c.id.clone(), c.name.clone()))
        .collect();

    let mut completed_tickets = 0;
    let mut in_progress_tickets = 0;
    let mut paused_tickets = 0;

    for ticket in &child_tickets {
        let column_name = column_names
            .get(&ticket.column_id)
            .map(|s| s.as_str())
            .unwrap_or("Unknown");

        if column_name == "Done" {
            completed_tickets += 1;
        } else if ticket.paused_at.is_some() {
            paused_tickets += 1;
        } else if column_name == "In Progress" || column_name == "Review" {
            in_progress_tickets += 1;
        }
    }

    let now = Utc::now();
    let elapsed_seconds = if let Some(ref started) = version.work_started_at {
        (now - *started).num_seconds()
    } else {
        0
    };

    let (avg_seconds_per_ticket, avg_seconds_per_stage, confidence) =
        calculate_timing_stats(db, &version.id);

    let (estimated_seconds_remaining, estimated_completion_time) = calculate_remaining_time(
        total_tickets,
        completed_tickets,
        in_progress_tickets,
        paused_tickets,
        avg_seconds_per_ticket,
    );

    Ok(SpecEta {
        spec_id: spec_id.to_string(),
        work_started_at: version.work_started_at,
        total_tickets,
        completed_tickets,
        in_progress_tickets,
        paused_tickets,
        elapsed_seconds,
        avg_seconds_per_ticket,
        avg_seconds_per_stage,
        estimated_seconds_remaining,
        estimated_completion_time,
        confidence,
    })
}
