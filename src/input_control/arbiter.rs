// Peer values are opaque runtime source IDs supplied by the input provider.
pub struct ArbiterInput {
    pub peer: u64,
    pub priority: i64,
    // Whether this source would hold any key or mouse button after this input.
    pub key_hold: bool,
}

#[derive(Default)]
pub struct ArbiterAction {
    pub accept: bool,
    // Clear the old source's held input before applying an accepted replacement.
    pub reset_key_hold: bool,
}

struct Owner {
    input: ArbiterInput,
    last_input_ms: u64,
    hold_deadline_ms: u64,
}

// Select one active input source. Higher-priority input can preempt even a hold;
// rejected sources are discarded without refreshing the current source's activity.
// The caller owns actual events and releases held input when requested.
pub struct InputArbiter {
    idle_ms: u64,
    hold_timeout_ms: u64,
    owner: Option<Owner>,
}

impl InputArbiter {
    pub fn new(idle_ms: u64, hold_timeout_ms: u64) -> Self {
        Self {
            idle_ms,
            hold_timeout_ms,
            owner: None,
        }
    }

    pub fn set_idle_ms(&mut self, idle_ms: u64) {
        self.idle_ms = idle_ms;
    }

    pub(crate) fn owner(&self) -> Option<u64> {
        self.owner.as_ref().map(|owner| owner.input.peer)
    }

    // None forgets the current source; Some forgets only that source if it owns input.
    pub fn forget(&mut self, peer: Option<u64>) -> bool {
        if self.owner.is_some() && peer.is_none_or(|peer| self.owner() == Some(peer)) {
            self.owner = None;
            true
        } else {
            false
        }
    }

    pub fn observe(&mut self, input: ArbiterInput, now: u64) -> ArbiterAction {
        let mut action = ArbiterAction {
            reset_key_hold: self.check_hold_expired(now),
            ..Default::default()
        };
        if let Some(owner) = &self.owner {
            let active =
                owner.input.key_hold || now.saturating_sub(owner.last_input_ms) < self.idle_ms;
            if owner.input.peer != input.peer && active && input.priority <= owner.input.priority {
                return action;
            }
            action.reset_key_hold |= owner.input.peer != input.peer;
        }
        self.owner = Some(Owner {
            input,
            last_input_ms: now,
            hold_deadline_ms: now.saturating_add(self.hold_timeout_ms),
        });
        action.accept = true;
        action
    }

    // Heartbeats extend only a live hold; they never claim ownership or count as new input.
    pub fn renew_hold(&mut self, peer: u64, received_at: u64, now: u64) -> ArbiterAction {
        let action = ArbiterAction {
            reset_key_hold: self.check_hold_expired(now),
            ..Default::default()
        };
        if let Some(owner) = self
            .owner
            .as_mut()
            .filter(|owner| owner.input.peer == peer && owner.input.key_hold)
        {
            owner.hold_deadline_ms = owner
                .hold_deadline_ms
                .max(received_at.saturating_add(self.hold_timeout_ms));
        }
        action
    }

    pub fn check_hold_expired(&mut self, now: u64) -> bool {
        let expired = self
            .owner
            .as_ref()
            .is_some_and(|owner| owner.input.key_hold && now >= owner.hold_deadline_ms);
        if expired {
            self.owner = None;
        }
        expired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(peer: u64, priority: i64, key_hold: bool) -> ArbiterInput {
        ArbiterInput {
            peer,
            priority,
            key_hold,
        }
    }

    #[test]
    fn rejected_input_does_not_keep_owner_active() {
        let mut arbiter = InputArbiter::new(50, 100);
        arbiter.observe(input(1, 10, false), 0);
        assert!(!arbiter.observe(input(2, 0, false), 30).accept);
        assert!(arbiter.observe(input(2, 0, false), 50).accept);
    }

    #[test]
    fn heartbeat_cannot_reacquire_expired_hold() {
        let mut arbiter = InputArbiter::new(50, 100);
        arbiter.observe(input(1, 10, true), 0);
        let action = arbiter.renew_hold(1, 100, 100);
        assert!(action.reset_key_hold);
        assert!(!action.accept);
        assert!(arbiter.observe(input(2, 0, true), 100).accept);
    }

    #[test]
    fn old_source_heartbeat_does_not_extend_replacement_hold() {
        let mut arbiter = InputArbiter::new(50, 100);
        arbiter.observe(input(1, 0, true), 0);
        let action = arbiter.observe(input(2, 10, true), 10);
        assert!(action.accept && action.reset_key_hold);
        arbiter.renew_hold(1, 100, 100);
        assert!(arbiter.check_hold_expired(110));
    }
}
