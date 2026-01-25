use super::TicketState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionPermission {
    Allowed,
    RequiresUnlock,
    Denied(String),
}

pub fn can_transition(
    from: TicketState,
    to: TicketState,
    is_locked: bool,
    is_system: bool,
) -> TransitionPermission {
    if from == to {
        return TransitionPermission::Allowed;
    }

    if is_system {
        return check_system_transition(from, to);
    }

    check_user_transition(from, to, is_locked)
}

fn check_user_transition(
    from: TicketState,
    to: TicketState,
    is_locked: bool,
) -> TransitionPermission {
    use TicketState::*;

    let allowed: &[(TicketState, TicketState)] = &[
        (Backlog, Ready),
        (Ready, Backlog),
        (InProgress, Ready),
        (InProgress, Blocked),
        (Blocked, Ready),
        (Blocked, Backlog),
        (Review, Done),
        (Review, Blocked),
        (Review, Ready),
        (Review, InProgress),
        (Done, Review),
    ];

    if allowed.contains(&(from, to)) {
        if from == InProgress && is_locked {
            return TransitionPermission::RequiresUnlock;
        }
        TransitionPermission::Allowed
    } else {
        TransitionPermission::Denied(format!(
            "Cannot move ticket from {} to {}",
            from.to_column_name(),
            to.to_column_name()
        ))
    }
}

fn check_system_transition(from: TicketState, to: TicketState) -> TransitionPermission {
    use TicketState::*;

    match (from, to) {
        (Ready, InProgress) => TransitionPermission::Allowed,
        (InProgress, Review) => TransitionPermission::Allowed,
        (InProgress, Blocked) => TransitionPermission::Allowed,
        (InProgress, Ready) => TransitionPermission::Allowed,
        _ => TransitionPermission::Denied(format!(
            "System cannot transition from {} to {}",
            from.to_column_name(),
            to.to_column_name()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TicketState::*;

    #[test]
    fn test_same_state_always_allowed() {
        for state in [Backlog, Ready, InProgress, Blocked, Review, Done] {
            assert_eq!(can_transition(state, state, false, false), TransitionPermission::Allowed);
            assert_eq!(can_transition(state, state, true, false), TransitionPermission::Allowed);
        }
    }

    #[test]
    fn test_backlog_to_ready_allowed() {
        assert_eq!(can_transition(Backlog, Ready, false, false), TransitionPermission::Allowed);
    }

    #[test]
    fn test_ready_to_backlog_allowed() {
        assert_eq!(can_transition(Ready, Backlog, false, false), TransitionPermission::Allowed);
    }

    #[test]
    fn test_in_progress_requires_unlock() {
        assert_eq!(can_transition(InProgress, Ready, true, false), TransitionPermission::RequiresUnlock);
        assert_eq!(can_transition(InProgress, Ready, false, false), TransitionPermission::Allowed);
    }

    #[test]
    fn test_review_to_done_allowed() {
        assert_eq!(can_transition(Review, Done, false, false), TransitionPermission::Allowed);
    }

    #[test]
    fn test_done_to_review_allowed() {
        assert_eq!(can_transition(Done, Review, false, false), TransitionPermission::Allowed);
    }

    #[test]
    fn test_system_transitions() {
        assert_eq!(can_transition(Ready, InProgress, false, true), TransitionPermission::Allowed);
        assert_eq!(can_transition(InProgress, Review, false, true), TransitionPermission::Allowed);
        assert_eq!(can_transition(InProgress, Blocked, false, true), TransitionPermission::Allowed);
        assert_eq!(can_transition(InProgress, Ready, false, true), TransitionPermission::Allowed);
    }

    #[test]
    fn test_invalid_transitions_denied() {
        assert!(matches!(can_transition(Backlog, InProgress, false, false), TransitionPermission::Denied(_)));
        assert!(matches!(can_transition(Done, Ready, false, false), TransitionPermission::Denied(_)));
    }
}
