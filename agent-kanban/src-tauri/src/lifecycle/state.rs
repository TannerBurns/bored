use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TicketState {
    Backlog,
    Ready,
    InProgress,
    Blocked,
    Review,
    Done,
}

impl TicketState {
    pub fn from_column_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "backlog" => Some(Self::Backlog),
            "ready" => Some(Self::Ready),
            "in progress" | "in_progress" | "inprogress" => Some(Self::InProgress),
            "blocked" => Some(Self::Blocked),
            "review" => Some(Self::Review),
            "done" => Some(Self::Done),
            _ => None,
        }
    }

    pub fn to_column_name(&self) -> &'static str {
        match self {
            Self::Backlog => "Backlog",
            Self::Ready => "Ready",
            Self::InProgress => "In Progress",
            Self::Blocked => "Blocked",
            Self::Review => "Review",
            Self::Done => "Done",
        }
    }

    pub fn is_queueable(&self) -> bool {
        matches!(self, Self::Ready)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Done)
    }

    pub fn requires_lock(&self) -> bool {
        matches!(self, Self::InProgress)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleOutcome {
    Success,
    Error,
    Aborted,
    Timeout,
}

impl LifecycleOutcome {
    pub fn target_state(&self) -> TicketState {
        match self {
            Self::Success => TicketState::Review,
            Self::Error => TicketState::Blocked,
            Self::Aborted => TicketState::Ready,
            Self::Timeout => TicketState::Blocked,
        }
    }

    pub fn from_run_outcome(outcome: crate::agents::RunOutcome) -> Self {
        match outcome {
            crate::agents::RunOutcome::Success => Self::Success,
            crate::agents::RunOutcome::Error => Self::Error,
            crate::agents::RunOutcome::Cancelled => Self::Aborted,
            crate::agents::RunOutcome::Timeout => Self::Timeout,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_column_name() {
        assert_eq!(TicketState::from_column_name("Backlog"), Some(TicketState::Backlog));
        assert_eq!(TicketState::from_column_name("ready"), Some(TicketState::Ready));
        assert_eq!(TicketState::from_column_name("In Progress"), Some(TicketState::InProgress));
        assert_eq!(TicketState::from_column_name("in_progress"), Some(TicketState::InProgress));
        assert_eq!(TicketState::from_column_name("blocked"), Some(TicketState::Blocked));
        assert_eq!(TicketState::from_column_name("Review"), Some(TicketState::Review));
        assert_eq!(TicketState::from_column_name("done"), Some(TicketState::Done));
        assert_eq!(TicketState::from_column_name("unknown"), None);
    }

    #[test]
    fn test_to_column_name() {
        assert_eq!(TicketState::Backlog.to_column_name(), "Backlog");
        assert_eq!(TicketState::Ready.to_column_name(), "Ready");
        assert_eq!(TicketState::InProgress.to_column_name(), "In Progress");
        assert_eq!(TicketState::Blocked.to_column_name(), "Blocked");
        assert_eq!(TicketState::Review.to_column_name(), "Review");
        assert_eq!(TicketState::Done.to_column_name(), "Done");
    }

    #[test]
    fn test_is_queueable() {
        assert!(!TicketState::Backlog.is_queueable());
        assert!(TicketState::Ready.is_queueable());
        assert!(!TicketState::InProgress.is_queueable());
        assert!(!TicketState::Blocked.is_queueable());
        assert!(!TicketState::Review.is_queueable());
        assert!(!TicketState::Done.is_queueable());
    }

    #[test]
    fn test_is_terminal() {
        assert!(!TicketState::Backlog.is_terminal());
        assert!(!TicketState::Ready.is_terminal());
        assert!(!TicketState::InProgress.is_terminal());
        assert!(!TicketState::Blocked.is_terminal());
        assert!(!TicketState::Review.is_terminal());
        assert!(TicketState::Done.is_terminal());
    }

    #[test]
    fn test_requires_lock() {
        assert!(!TicketState::Backlog.requires_lock());
        assert!(!TicketState::Ready.requires_lock());
        assert!(TicketState::InProgress.requires_lock());
        assert!(!TicketState::Blocked.requires_lock());
        assert!(!TicketState::Review.requires_lock());
        assert!(!TicketState::Done.requires_lock());
    }

    #[test]
    fn test_lifecycle_outcome_target_state() {
        assert_eq!(LifecycleOutcome::Success.target_state(), TicketState::Review);
        assert_eq!(LifecycleOutcome::Error.target_state(), TicketState::Blocked);
        assert_eq!(LifecycleOutcome::Aborted.target_state(), TicketState::Ready);
        assert_eq!(LifecycleOutcome::Timeout.target_state(), TicketState::Blocked);
    }
}
