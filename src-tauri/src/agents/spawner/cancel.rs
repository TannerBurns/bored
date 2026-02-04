//! Cancel handle for running processes.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct CancelHandle {
    cancelled: Arc<AtomicBool>,
}

impl CancelHandle {
    pub fn new(cancelled: Arc<AtomicBool>) -> Self {
        Self { cancelled }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_handle_sets_flag() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let handle = CancelHandle::new(cancelled.clone());

        assert!(!cancelled.load(Ordering::Relaxed));
        handle.cancel();
        assert!(cancelled.load(Ordering::Relaxed));
    }

    #[test]
    fn cancel_handle_is_cancelled_reflects_state() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let handle = CancelHandle::new(cancelled.clone());

        assert!(!handle.is_cancelled());
        handle.cancel();
        assert!(handle.is_cancelled());
    }

    #[test]
    fn cancel_handle_clone_shares_state() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let handle1 = CancelHandle::new(cancelled.clone());
        let handle2 = handle1.clone();

        assert!(!handle1.is_cancelled());
        assert!(!handle2.is_cancelled());

        // Cancel via handle1
        handle1.cancel();

        // Both handles should reflect the cancellation
        assert!(handle1.is_cancelled());
        assert!(handle2.is_cancelled());
    }
}
