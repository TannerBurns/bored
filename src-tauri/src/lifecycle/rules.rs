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
    _to: TicketState,
    is_locked: bool,
) -> TransitionPermission {
    use TicketState::*;

    // Only restriction: locked tickets in InProgress require unlock first
    if from == InProgress && is_locked {
        return TransitionPermission::RequiresUnlock;
    }

    // All other transitions are allowed
    TransitionPermission::Allowed
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
    fn test_all_transitions_allowed() {
        // All transitions between different states should be allowed (when not locked)
        let states = [Backlog, Ready, InProgress, Blocked, Review, Done];
        for from in states {
            for to in states {
                if from != to && from != InProgress {
                    assert_eq!(
                        can_transition(from, to, false, false), 
                        TransitionPermission::Allowed,
                        "Expected {:?} -> {:?} to be allowed",
                        from, to
                    );
                }
            }
        }
    }

    #[test]
    fn test_in_progress_requires_unlock_when_locked() {
        // When locked, InProgress transitions require unlock
        assert_eq!(can_transition(InProgress, Ready, true, false), TransitionPermission::RequiresUnlock);
        assert_eq!(can_transition(InProgress, Blocked, true, false), TransitionPermission::RequiresUnlock);
        assert_eq!(can_transition(InProgress, Done, true, false), TransitionPermission::RequiresUnlock);
    }

    #[test]
    fn test_in_progress_allowed_when_not_locked() {
        // When not locked, InProgress transitions are allowed
        assert_eq!(can_transition(InProgress, Ready, false, false), TransitionPermission::Allowed);
        assert_eq!(can_transition(InProgress, Blocked, false, false), TransitionPermission::Allowed);
        assert_eq!(can_transition(InProgress, Done, false, false), TransitionPermission::Allowed);
        assert_eq!(can_transition(InProgress, Backlog, false, false), TransitionPermission::Allowed);
    }

    #[test]
    fn test_system_transitions() {
        assert_eq!(can_transition(Ready, InProgress, false, true), TransitionPermission::Allowed);
        assert_eq!(can_transition(InProgress, Review, false, true), TransitionPermission::Allowed);
        assert_eq!(can_transition(InProgress, Blocked, false, true), TransitionPermission::Allowed);
        assert_eq!(can_transition(InProgress, Ready, false, true), TransitionPermission::Allowed);
    }
}
