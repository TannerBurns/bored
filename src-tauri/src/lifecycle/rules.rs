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
    _from: TicketState,
    _to: TicketState,
    _is_locked: bool,
) -> TransitionPermission {
    // All user-initiated transitions are allowed - no restrictions
    // Users can move tickets freely between any columns
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
    fn test_in_progress_allowed_even_when_locked() {
        // All InProgress transitions are allowed, even when locked (no restrictions for users)
        assert_eq!(can_transition(InProgress, Ready, true, false), TransitionPermission::Allowed);
        assert_eq!(can_transition(InProgress, Blocked, true, false), TransitionPermission::Allowed);
        assert_eq!(can_transition(InProgress, Done, true, false), TransitionPermission::Allowed);
    }

    #[test]
    fn test_in_progress_allowed_when_not_locked() {
        // InProgress transitions are allowed when not locked
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
