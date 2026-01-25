use super::TicketState;

/// Transition permission
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionPermission {
    /// Transition is allowed
    Allowed,
    /// Transition requires unlocked ticket
    RequiresUnlock,
    /// Transition is not allowed
    Denied(String),
}

/// Check if a transition is allowed
pub fn can_transition(
    from: TicketState,
    to: TicketState,
    is_locked: bool,
    is_system: bool,
) -> TransitionPermission {
    // Same state is always allowed (no-op)
    if from == to {
        return TransitionPermission::Allowed;
    }

    // System transitions have different rules
    if is_system {
        return check_system_transition(from, to);
    }

    // User transitions
    check_user_transition(from, to, is_locked)
}

fn check_user_transition(
    from: TicketState,
    to: TicketState,
    is_locked: bool,
) -> TransitionPermission {
    use TicketState::*;

    // Define allowed user transitions
    let allowed: &[(TicketState, TicketState)] = &[
        // From Backlog
        (Backlog, Ready),
        
        // From Ready
        (Ready, Backlog),
        
        // From In Progress (only if unlocked)
        (InProgress, Ready),
        (InProgress, Blocked),
        
        // From Blocked
        (Blocked, Ready),
        (Blocked, Backlog),
        
        // From Review
        (Review, Done),
        (Review, Blocked),
        (Review, Ready),
        (Review, InProgress), // Retry
        
        // From Done
        (Done, Review), // Reopen
    ];

    if allowed.contains(&(from, to)) {
        // Check lock requirement
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

fn check_system_transition(
    from: TicketState,
    to: TicketState,
) -> TransitionPermission {
    use TicketState::*;

    match (from, to) {
        // Agent reservation
        (Ready, InProgress) => TransitionPermission::Allowed,
        
        // Run completion
        (InProgress, Review) => TransitionPermission::Allowed,
        (InProgress, Blocked) => TransitionPermission::Allowed,
        
        // Run cancellation / lock expiry
        (InProgress, Ready) => TransitionPermission::Allowed,
        
        _ => TransitionPermission::Denied(format!(
            "System cannot transition from {} to {}",
            from.to_column_name(),
            to.to_column_name()
        ))
    }
}

/// Get valid target states from current state for user transitions
pub fn valid_targets(from: TicketState, is_locked: bool) -> Vec<TicketState> {
    use TicketState::*;

    let mut targets = Vec::new();

    // Always can stay in same state
    targets.push(from);

    match from {
        Backlog => {
            targets.push(Ready);
        }
        Ready => {
            targets.push(Backlog);
            // InProgress only via agent reservation
        }
        InProgress => {
            if !is_locked {
                targets.push(Ready);
                targets.push(Blocked);
            }
            // Review/Blocked via system only
        }
        Blocked => {
            targets.push(Ready);
            targets.push(Backlog);
        }
        Review => {
            targets.push(Done);
            targets.push(Blocked);
            targets.push(Ready);
            targets.push(InProgress); // Retry with agent
        }
        Done => {
            targets.push(Review);
        }
    }

    targets
}

#[cfg(test)]
mod tests {
    use super::*;
    use TicketState::*;

    #[test]
    fn test_same_state_always_allowed() {
        for state in [Backlog, Ready, InProgress, Blocked, Review, Done] {
            assert_eq!(
                can_transition(state, state, false, false),
                TransitionPermission::Allowed
            );
            assert_eq!(
                can_transition(state, state, true, false),
                TransitionPermission::Allowed
            );
        }
    }

    #[test]
    fn test_backlog_to_ready_allowed() {
        assert_eq!(
            can_transition(Backlog, Ready, false, false),
            TransitionPermission::Allowed
        );
    }

    #[test]
    fn test_ready_to_backlog_allowed() {
        assert_eq!(
            can_transition(Ready, Backlog, false, false),
            TransitionPermission::Allowed
        );
    }

    #[test]
    fn test_in_progress_requires_unlock() {
        assert_eq!(
            can_transition(InProgress, Ready, true, false),
            TransitionPermission::RequiresUnlock
        );
        assert_eq!(
            can_transition(InProgress, Ready, false, false),
            TransitionPermission::Allowed
        );
    }

    #[test]
    fn test_review_to_done_allowed() {
        assert_eq!(
            can_transition(Review, Done, false, false),
            TransitionPermission::Allowed
        );
    }

    #[test]
    fn test_done_to_review_allowed() {
        assert_eq!(
            can_transition(Review, Done, false, false),
            TransitionPermission::Allowed
        );
    }

    #[test]
    fn test_system_transitions() {
        // Agent reservation
        assert_eq!(
            can_transition(Ready, InProgress, false, true),
            TransitionPermission::Allowed
        );

        // Run completion
        assert_eq!(
            can_transition(InProgress, Review, false, true),
            TransitionPermission::Allowed
        );
        assert_eq!(
            can_transition(InProgress, Blocked, false, true),
            TransitionPermission::Allowed
        );

        // Lock expiry / cancellation
        assert_eq!(
            can_transition(InProgress, Ready, false, true),
            TransitionPermission::Allowed
        );
    }

    #[test]
    fn test_invalid_transitions_denied() {
        // Cannot go from Backlog directly to InProgress
        assert!(matches!(
            can_transition(Backlog, InProgress, false, false),
            TransitionPermission::Denied(_)
        ));

        // Cannot go from Done to Ready directly
        assert!(matches!(
            can_transition(Done, Ready, false, false),
            TransitionPermission::Denied(_)
        ));
    }

    #[test]
    fn test_valid_targets() {
        let targets = valid_targets(Backlog, false);
        assert!(targets.contains(&Backlog));
        assert!(targets.contains(&Ready));
        assert!(!targets.contains(&InProgress));

        let targets = valid_targets(InProgress, true);
        assert!(targets.contains(&InProgress));
        assert!(!targets.contains(&Ready)); // Locked

        let targets = valid_targets(InProgress, false);
        assert!(targets.contains(&InProgress));
        assert!(targets.contains(&Ready)); // Unlocked
        assert!(targets.contains(&Blocked));

        let targets = valid_targets(Review, false);
        assert!(targets.contains(&Done));
        assert!(targets.contains(&Blocked));
        assert!(targets.contains(&Ready));
        assert!(targets.contains(&InProgress));
    }
}
