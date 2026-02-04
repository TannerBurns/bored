//! Worker manager for coordinating multiple workers.

use std::sync::Arc;

use crate::db::Database;

use super::config::{WorkerConfig, WorkerStatus};
use super::runner::CancelHandlesMap;
use super::Worker;

pub struct WorkerManager {
    workers: std::sync::Mutex<Vec<Arc<Worker>>>,
    handles: std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>,
}

impl WorkerManager {
    pub fn new() -> Self {
        Self {
            workers: std::sync::Mutex::new(Vec::new()),
            handles: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn start_worker(
        &self,
        config: WorkerConfig,
        db: Arc<Database>,
        cancel_handles: Option<CancelHandlesMap>,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let worker = Arc::new(Worker::new(id.clone(), config, db, cancel_handles));
        let worker_clone = worker.clone();

        let handle = tokio::spawn(async move {
            worker_clone.run().await;
        });

        self.workers
            .lock()
            .expect("workers mutex poisoned")
            .push(worker);
        self.handles
            .lock()
            .expect("handles mutex poisoned")
            .push(handle);

        id
    }

    pub fn stop_worker(&self, worker_id: &str) -> bool {
        let mut workers = self.workers.lock().expect("workers mutex poisoned");
        let mut handles = self.handles.lock().expect("handles mutex poisoned");

        let index = workers.iter().position(|w| w.id == worker_id);

        if let Some(idx) = index {
            workers[idx].stop();
            workers.remove(idx);
            if idx < handles.len() {
                let handle = handles.remove(idx);
                handle.abort();
            }
            return true;
        }
        false
    }

    pub async fn stop_all(&self) {
        {
            let workers = self.workers.lock().expect("workers mutex poisoned");
            for worker in workers.iter() {
                worker.stop();
            }
        }

        // Abort all handles instead of awaiting them - this ensures idle workers
        // that are sleeping during poll intervals are terminated immediately
        let handles: Vec<_> = self
            .handles
            .lock()
            .expect("handles mutex poisoned")
            .drain(..)
            .collect();
        for handle in handles {
            handle.abort();
        }

        self.workers.lock().expect("workers mutex poisoned").clear();
    }

    pub fn get_all_status(&self) -> Vec<WorkerStatus> {
        self.workers
            .lock()
            .expect("workers mutex poisoned")
            .iter()
            .map(|w| w.get_status())
            .collect()
    }

    pub fn worker_count(&self) -> usize {
        self.workers.lock().expect("workers mutex poisoned").len()
    }
}

impl Default for WorkerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_manager_new_is_empty() {
        let manager = WorkerManager::new();
        assert_eq!(manager.worker_count(), 0);
        assert!(manager.get_all_status().is_empty());
    }

    #[test]
    fn worker_manager_stop_unknown_returns_false() {
        let manager = WorkerManager::new();
        assert!(!manager.stop_worker("nonexistent-id"));
    }

    #[test]
    fn worker_manager_default_is_new() {
        let manager = WorkerManager::default();
        assert_eq!(manager.worker_count(), 0);
    }
}
