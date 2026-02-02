//! ETA calculation module for scratchpad work estimation.
//!
//! This module provides timing data aggregation and ETA calculation for
//! estimating when scratchpad work will complete.

use std::collections::HashMap;
use std::sync::Arc;
use chrono::{DateTime, Utc};

use crate::db::{Database, ScratchpadEta, EtaConfidence};

/// Calculate ETA for a scratchpad based on timing data from completed tickets
pub fn calculate_eta(db: &Arc<Database>, scratchpad_id: &str) -> Result<ScratchpadEta, String> {
    let scratchpad = db.get_scratchpad(scratchpad_id)
        .map_err(|e| e.to_string())?;
    
    let tickets = db.get_scratchpad_tickets(scratchpad_id)
        .map_err(|e| e.to_string())?;
    
    let child_tickets: Vec<_> = tickets.iter()
        .filter(|t| !t.is_epic)
        .collect();
    
    let total_tickets = child_tickets.len();
    
    let columns = if let Some(ref target_board_id) = scratchpad.target_board_id {
        db.get_columns(target_board_id).ok()
    } else {
        db.get_columns(&scratchpad.board_id).ok()
    }.unwrap_or_default();
    
    let column_names: HashMap<String, String> = columns.iter()
        .map(|c| (c.id.clone(), c.name.clone()))
        .collect();
    
    let mut completed_tickets = 0;
    let mut in_progress_tickets = 0;
    let mut paused_tickets = 0;
    
    for ticket in &child_tickets {
        let column_name = column_names.get(&ticket.column_id)
            .map(|s| s.as_str())
            .unwrap_or("Unknown");
        
        if ticket.paused_at.is_some() {
            paused_tickets += 1;
        } else if column_name == "Done" {
            completed_tickets += 1;
        } else if column_name == "In Progress" || column_name == "Review" {
            in_progress_tickets += 1;
        }
    }
    
    let now = Utc::now();
    let elapsed_seconds = if let Some(ref started) = scratchpad.work_started_at {
        (now - *started).num_seconds()
    } else {
        0
    };
    
    let (avg_seconds_per_ticket, avg_seconds_per_stage, confidence) = 
        calculate_timing_stats(db, scratchpad_id);
    
    let (estimated_seconds_remaining, estimated_completion_time) = 
        calculate_remaining_time(
            total_tickets,
            completed_tickets,
            in_progress_tickets,
            paused_tickets,
            avg_seconds_per_ticket,
        );
    
    Ok(ScratchpadEta {
        scratchpad_id: scratchpad_id.to_string(),
        work_started_at: scratchpad.work_started_at,
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

fn calculate_timing_stats(
    db: &Arc<Database>,
    scratchpad_id: &str,
) -> (Option<f64>, HashMap<String, f64>, EtaConfidence) {
    let tickets = match db.get_scratchpad_tickets(scratchpad_id) {
        Ok(t) => t,
        Err(_) => return (None, HashMap::new(), EtaConfidence::Low),
    };
    
    let child_ticket_ids: Vec<_> = tickets.iter()
        .filter(|t| !t.is_epic)
        .map(|t| t.id.clone())
        .collect();
    
    if child_ticket_ids.is_empty() {
        return (None, HashMap::new(), EtaConfidence::Low);
    }
    
    let mut total_duration_secs: f64 = 0.0;
    let mut run_count = 0;
    let mut stage_durations: HashMap<String, Vec<f64>> = HashMap::new();
    
    for ticket_id in &child_ticket_ids {
        if let Ok(runs) = db.get_runs(ticket_id) {
            for run in runs {
                // Only count finished runs
                if run.status.as_str() != "finished" {
                    continue;
                }
                
                if let Some(ended) = run.ended_at {
                    let started = run.started_at;
                    let duration = (ended - started).num_seconds() as f64;
                    if duration > 0.0 {
                        // For parent runs (no parent_run_id), count toward total ticket time
                        if run.parent_run_id.is_none() {
                            total_duration_secs += duration;
                            run_count += 1;
                        }
                        
                        // Track stage-specific durations
                        if let Some(stage) = run.stage {
                            stage_durations
                                .entry(stage)
                                .or_default()
                                .push(duration);
                        }
                    }
                }
            }
        }
    }
    
    let avg_per_ticket = if run_count > 0 {
        Some(total_duration_secs / run_count as f64)
    } else {
        None
    };
    
    let avg_per_stage: HashMap<String, f64> = stage_durations.into_iter()
        .map(|(stage, durations)| {
            let avg = durations.iter().sum::<f64>() / durations.len() as f64;
            (stage, avg)
        })
        .collect();
    
    // Confidence is based on the number of completed runs we have timing data for,
    // not the number of tickets in "Done" column (which may not reflect actual run data)
    let confidence = match run_count {
        0 => EtaConfidence::Low,
        1..=2 => EtaConfidence::Low,
        3..=5 => EtaConfidence::Medium,
        _ => EtaConfidence::High,
    };
    
    (avg_per_ticket, avg_per_stage, confidence)
}

/// Calculate estimated remaining time
fn calculate_remaining_time(
    total: usize,
    completed: usize,
    in_progress: usize,
    paused: usize,
    avg_seconds_per_ticket: Option<f64>,
) -> (Option<i64>, Option<DateTime<Utc>>) {
    let avg = match avg_seconds_per_ticket {
        Some(avg) if avg > 0.0 => avg,
        _ => return (None, None),
    };
    
    // Remaining = (total - completed - in_progress - paused) * avg + in_progress * (avg / 2)
    // Assuming in-progress tickets are on average halfway done
    // Paused tickets are excluded from remaining work calculation
    let remaining_count = total
        .saturating_sub(completed)
        .saturating_sub(in_progress)
        .saturating_sub(paused);
    let estimated_remaining = (remaining_count as f64 * avg) + (in_progress as f64 * avg / 2.0);
    
    let remaining_secs = estimated_remaining as i64;
    let completion_time = Utc::now() + chrono::Duration::seconds(remaining_secs);
    
    (Some(remaining_secs), Some(completion_time))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_calculate_remaining_time_no_avg() {
        let (remaining, completion) = calculate_remaining_time(10, 5, 2, 0, None);
        assert!(remaining.is_none());
        assert!(completion.is_none());
    }
    
    #[test]
    fn test_calculate_remaining_time_zero_avg() {
        let (remaining, completion) = calculate_remaining_time(10, 5, 2, 0, Some(0.0));
        assert!(remaining.is_none());
        assert!(completion.is_none());
    }
    
    #[test]
    fn test_calculate_remaining_time_with_avg() {
        // 10 total, 5 completed, 2 in progress, avg 60 seconds
        // Remaining = (10 - 5 - 2) * 60 + 2 * 30 = 3 * 60 + 60 = 240 seconds
        let (remaining, completion) = calculate_remaining_time(10, 5, 2, 0, Some(60.0));
        assert_eq!(remaining, Some(240));
        assert!(completion.is_some());
    }
    
    #[test]
    fn test_calculate_remaining_time_all_completed() {
        let (remaining, completion) = calculate_remaining_time(10, 10, 0, 0, Some(60.0));
        assert_eq!(remaining, Some(0));
        assert!(completion.is_some());
    }
    
    #[test]
    fn test_calculate_remaining_time_negative_avg() {
        let (remaining, completion) = calculate_remaining_time(10, 5, 2, 0, Some(-5.0));
        assert!(remaining.is_none());
        assert!(completion.is_none());
    }
    
    #[test]
    fn test_calculate_remaining_time_only_in_progress() {
        // 5 total, 0 completed, 5 in progress, avg 120 seconds
        // Remaining = 0 + 5 * 60 = 300 seconds (in-progress estimated as half done)
        let (remaining, _) = calculate_remaining_time(5, 0, 5, 0, Some(120.0));
        assert_eq!(remaining, Some(300));
    }
    
    #[test]
    fn test_calculate_remaining_time_handles_saturating_sub() {
        // Edge case: more completed than total (shouldn't happen but should not panic)
        let (remaining, _) = calculate_remaining_time(5, 10, 0, 0, Some(60.0));
        assert_eq!(remaining, Some(0));
    }
    
    #[test]
    fn test_calculate_remaining_time_with_paused_tickets() {
        // 10 total, 3 completed, 2 in progress, 2 paused, avg 60 seconds
        // Remaining = (10 - 3 - 2 - 2) * 60 + 2 * 30 = 3 * 60 + 60 = 240 seconds
        // Paused tickets are excluded from remaining work
        let (remaining, completion) = calculate_remaining_time(10, 3, 2, 2, Some(60.0));
        assert_eq!(remaining, Some(240));
        assert!(completion.is_some());
    }
    
    #[test]
    fn test_calculate_remaining_time_all_paused() {
        // 5 total, 0 completed, 0 in progress, 5 paused, avg 60 seconds
        // All tickets are paused, so remaining = 0
        let (remaining, _) = calculate_remaining_time(5, 0, 0, 5, Some(60.0));
        assert_eq!(remaining, Some(0));
    }
    
    #[test]
    fn test_eta_confidence_low_for_zero_completed() {
        assert_eq!(
            match 0 {
                0 => EtaConfidence::Low,
                1..=2 => EtaConfidence::Low,
                3..=5 => EtaConfidence::Medium,
                _ => EtaConfidence::High,
            },
            EtaConfidence::Low
        );
    }
    
    #[test]
    fn test_eta_confidence_medium_for_few_completed() {
        for n in 3..=5 {
            let confidence = match n {
                0 => EtaConfidence::Low,
                1..=2 => EtaConfidence::Low,
                3..=5 => EtaConfidence::Medium,
                _ => EtaConfidence::High,
            };
            assert_eq!(confidence, EtaConfidence::Medium);
        }
    }
    
    #[test]
    fn test_eta_confidence_high_for_many_completed() {
        for n in [6, 10, 100] {
            let confidence = match n {
                0 => EtaConfidence::Low,
                1..=2 => EtaConfidence::Low,
                3..=5 => EtaConfidence::Medium,
                _ => EtaConfidence::High,
            };
            assert_eq!(confidence, EtaConfidence::High);
        }
    }
}
