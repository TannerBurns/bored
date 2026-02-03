//! Heartbeat management for keeping ticket locks alive.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::db::Database;

/// Start a heartbeat task that periodically extends the ticket lock.
///
/// Returns a handle that can be used to abort the heartbeat when the work is complete.
pub fn start_heartbeat(
    db: Arc<Database>,
    ticket_id: String,
    run_id: String,
    interval_secs: u64,
    lock_duration_mins: i64,
    running: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));

        while running.load(Ordering::Relaxed) {
            ticker.tick().await;

            let new_expires = chrono::Utc::now() + chrono::Duration::minutes(lock_duration_mins);

            if let Err(e) = db.extend_lock(&ticket_id, &run_id, new_expires) {
                tracing::error!("Heartbeat failed for ticket {}: {}", ticket_id, e);
                break;
            }
        }
    })
}
