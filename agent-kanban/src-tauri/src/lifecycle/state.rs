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
}
