//! Ordering gate for authoritative snapshot delivery.
//!
//! This module only rejects duplicate or stale presentation updates. It does
//! not validate gameplay state or make any server-authoritative decision.

/// A monotonic delivery revision assigned by the protocol adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SnapshotRevision(u64);

impl SnapshotRevision {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Result of comparing an incoming snapshot with the last applied revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevisionDecision {
    Initialize {
        revision: SnapshotRevision,
    },
    Advance {
        previous: SnapshotRevision,
        revision: SnapshotRevision,
    },
    Duplicate {
        revision: SnapshotRevision,
    },
    Stale {
        last_applied: SnapshotRevision,
        received: SnapshotRevision,
    },
}

impl RevisionDecision {
    pub const fn should_apply(self) -> bool {
        matches!(self, Self::Initialize { .. } | Self::Advance { .. })
    }
}

/// Remembers only the newest applied authoritative delivery revision.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RevisionGate {
    last_applied: Option<SnapshotRevision>,
}

impl RevisionGate {
    pub const fn last_applied(&self) -> Option<SnapshotRevision> {
        self.last_applied
    }

    pub fn observe(&mut self, received: SnapshotRevision) -> RevisionDecision {
        let decision = match self.last_applied {
            None => RevisionDecision::Initialize { revision: received },
            Some(previous) if received > previous => RevisionDecision::Advance {
                previous,
                revision: received,
            },
            Some(previous) if received == previous => {
                RevisionDecision::Duplicate { revision: received }
            }
            Some(last_applied) => RevisionDecision::Stale {
                last_applied,
                received,
            },
        };

        if decision.should_apply() {
            self.last_applied = Some(received);
        }

        decision
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initializes_then_advances_monotonically() {
        let mut gate = RevisionGate::default();

        assert_eq!(
            gate.observe(SnapshotRevision::new(7)),
            RevisionDecision::Initialize {
                revision: SnapshotRevision::new(7)
            }
        );
        assert_eq!(
            gate.observe(SnapshotRevision::new(9)),
            RevisionDecision::Advance {
                previous: SnapshotRevision::new(7),
                revision: SnapshotRevision::new(9)
            }
        );
        assert_eq!(gate.last_applied(), Some(SnapshotRevision::new(9)));
    }

    #[test]
    fn duplicate_and_stale_revisions_do_not_move_the_gate() {
        let mut gate = RevisionGate::default();
        gate.observe(SnapshotRevision::new(9));

        assert_eq!(
            gate.observe(SnapshotRevision::new(9)),
            RevisionDecision::Duplicate {
                revision: SnapshotRevision::new(9)
            }
        );
        assert_eq!(
            gate.observe(SnapshotRevision::new(3)),
            RevisionDecision::Stale {
                last_applied: SnapshotRevision::new(9),
                received: SnapshotRevision::new(3)
            }
        );
        assert_eq!(gate.last_applied(), Some(SnapshotRevision::new(9)));
    }
}
