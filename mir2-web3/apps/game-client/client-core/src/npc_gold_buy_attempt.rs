//! Platform-neutral local NPC gold-buy transport attempts; never purchase ACKs.
use std::collections::VecDeque;

pub const NPC_GOLD_BUY_AUTHORITY_MAX_BYTES: usize = 1024 * 1024;
pub const NPC_GOLD_BUY_ATTEMPT_HISTORY_LIMIT: usize = 32;

/// Trusted producing connection only; owner, model and UI clocks are independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NpcGoldBuyConnectionEpoch { pub run: u64, pub connection: u64 }
impl NpcGoldBuyConnectionEpoch {
    pub fn is_valid(self) -> bool { self.run > 0 && self.connection > 0 }
}

/// Opaque, nonzero, checked monotonic identity. A slot never resets its allocator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NpcGoldBuyAttemptToken(u64);
impl NpcGoldBuyAttemptToken {
    pub fn value(self) -> u64 { self.0 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcGoldBuyAttemptPhase {
    Queued, Bound, Entered, Flushed, Unknown, DefinitelyUnsent,
}

/// Transport observations only; neither Flushed nor Unknown means purchase success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcGoldBuyAttemptOutcome { DefinitelyUnsent, Flushed, Unknown }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpcGoldBuyAttempt<T: Clone + Eq> {
    pub token: NpcGoldBuyAttemptToken,
    pub authority_revision: u64,
    pub ticket: Option<T>,
    pub phase: NpcGoldBuyAttemptPhase,
}

/// The host serializes the complete owner/service/catalog/inventory authority.
/// Selection, quantity, pagination and layout are not authority changes. Hosts
/// must hold their transport gate across begin_entry and the actual start_send.
#[derive(Debug)]
pub struct NpcGoldBuyAttemptSlot<T: Clone + Eq> {
    connection: Option<NpcGoldBuyConnectionEpoch>,
    authority: Option<String>,
    authority_revision: u64,
    authority_observed: bool,
    available: bool,
    last_token: u64,
    exhausted: bool,
    flight: Option<NpcGoldBuyAttempt<T>>,
    history: VecDeque<NpcGoldBuyAttempt<T>>,
}

impl<T: Clone + Eq> Default for NpcGoldBuyAttemptSlot<T> {
    fn default() -> Self {
        Self { connection: None, authority: None, authority_revision: 0, authority_observed: false,
            available: false, last_token: 0, exhausted: false,
            flight: None, history: VecDeque::new() }
    }
}

impl<T: Clone + Eq> NpcGoldBuyAttemptSlot<T> {
    pub fn new() -> Self { Self::default() }
    pub fn authority_revision(&self) -> u64 { self.authority_revision }
    pub fn connection_epoch(&self) -> Option<NpcGoldBuyConnectionEpoch> { self.connection }

    /// Only a trusted real new connection can retire unresolved transport work.
    /// This is Unknown retirement, never proof of economic success or rejection.
    pub fn observe_connection(&mut self, epoch: NpcGoldBuyConnectionEpoch) -> bool {
        if self.exhausted || !epoch.is_valid() { return false; }
        if let Some(current) = self.connection {
            if epoch == current { return true; }
            if epoch < current { return false; }
            let Some(revision) = self.authority_revision.checked_add(1) else {
                self.exhaust(); return false;
            };
            if let Some(mut attempt) = self.flight.take() {
                attempt.phase = match attempt.phase {
                    NpcGoldBuyAttemptPhase::Queued | NpcGoldBuyAttemptPhase::Bound
                        => NpcGoldBuyAttemptPhase::DefinitelyUnsent,
                    _ => NpcGoldBuyAttemptPhase::Unknown,
                };
                self.archive(attempt);
            }
            self.authority_revision = revision;
        } else {
            // Legacy unresolved work has no trustworthy producing connection.
            // Assigning its first epoch must never manufacture a reset boundary.
            if self.flight.as_ref().is_some_and(|attempt| !matches!(attempt.phase,
                NpcGoldBuyAttemptPhase::Queued | NpcGoldBuyAttemptPhase::Bound)) { return false; }
            self.retire_for_authority_change();
        }
        self.connection = Some(epoch);
        self.authority = None;
        self.authority_observed = false;
        self.available = false;
        true
    }
    pub fn flight(&self) -> Option<&NpcGoldBuyAttempt<T>> { self.flight.as_ref() }
    pub fn history(&self) -> &VecDeque<NpcGoldBuyAttempt<T>> { &self.history }
    pub fn can_reserve(&self) -> bool {
        self.connection.is_some() && self.available && self.authority_observed && self.authority.is_some()
            && !self.exhausted && self.last_token < u64::MAX && self.flight.is_none()
    }

    fn archive(&mut self, attempt: NpcGoldBuyAttempt<T>) {
        if self.history.len() == NPC_GOLD_BUY_ATTEMPT_HISTORY_LIMIT {
            self.history.pop_front();
        }
        self.history.push_back(attempt);
    }

    fn retire_for_authority_change(&mut self) {
        if self.flight.as_ref().is_some_and(|attempt| matches!(attempt.phase,
            NpcGoldBuyAttemptPhase::Queued | NpcGoldBuyAttemptPhase::Bound)) {
            let mut attempt = self.flight.take().expect("checked pre-entry flight");
            attempt.phase = NpcGoldBuyAttemptPhase::DefinitelyUnsent;
            self.archive(attempt);
        }
    }

    fn exhaust(&mut self) {
        self.exhausted = true;
        self.set_available(false);
    }

    /// Observe the full current authority, without implicitly opening the UI.
    /// Invalid observations withdraw pre-entry work but preserve entered barriers.
    pub fn observe_authority(&mut self, authority: &str) -> bool {
        if self.exhausted { return false; }
        if authority.is_empty() || authority.len() > NPC_GOLD_BUY_AUTHORITY_MAX_BYTES {
            self.authority_observed = false;
            self.set_available(false);
            return false;
        }
        if self.authority.as_deref() == Some(authority) {
            self.authority_observed = true;
            return true;
        }
        let Some(revision) = self.authority_revision.checked_add(1) else {
            self.exhaust();
            return false;
        };
        self.retire_for_authority_change();
        self.authority = Some(authority.to_owned());
        self.authority_revision = revision;
        self.authority_observed = true;
        true
    }

    /// Layout/visibility loss cancels only work proven not to have entered.
    /// Reopening the same authority cannot release an entered/flushed/unknown flight.
    pub fn set_available(&mut self, available: bool) {
        self.available = available && !self.exhausted;
        if !self.available && self.flight.as_ref().is_some_and(|attempt| matches!(
            attempt.phase, NpcGoldBuyAttemptPhase::Queued | NpcGoldBuyAttemptPhase::Bound
        )) {
            let mut attempt = self.flight.take().expect("checked flight");
            attempt.phase = NpcGoldBuyAttemptPhase::DefinitelyUnsent;
            self.archive(attempt);
        }
    }

    pub fn reserve(&mut self) -> Option<NpcGoldBuyAttemptToken> {
        if self.connection.is_none() || !self.available || !self.authority_observed || self.authority.is_none()
            || self.exhausted || self.flight.is_some() { return None; }
        let Some(value) = self.last_token.checked_add(1) else {
            self.exhaust();
            return None;
        };
        let token = NpcGoldBuyAttemptToken(value);
        self.last_token = value;
        self.flight = Some(NpcGoldBuyAttempt { token,
            authority_revision: self.authority_revision, ticket: None,
            phase: NpcGoldBuyAttemptPhase::Queued });
        Some(token)
    }

    pub fn bind(&mut self, token: NpcGoldBuyAttemptToken, ticket: T) -> bool {
        let Some(attempt) = self.flight.as_mut() else { return false; };
        if attempt.token != token || attempt.phase != NpcGoldBuyAttemptPhase::Queued
            || attempt.authority_revision != self.authority_revision
            || self.connection.is_none() || !self.available || !self.authority_observed || self.exhausted { return false; }
        attempt.ticket = Some(ticket);
        attempt.phase = NpcGoldBuyAttemptPhase::Bound;
        true
    }

    pub fn reject_unpublished(&mut self, token: NpcGoldBuyAttemptToken) -> bool {
        if !self.flight.as_ref().is_some_and(|attempt|
            attempt.token == token && attempt.phase == NpcGoldBuyAttemptPhase::Queued
        ) { return false; }
        let mut attempt = self.flight.take().expect("checked queued flight");
        attempt.phase = NpcGoldBuyAttemptPhase::DefinitelyUnsent;
        self.archive(attempt);
        true
    }

    /// Read-only form of the exact final entry predicate; never reserves or enters.
    pub fn can_begin_entry(&self, token: NpcGoldBuyAttemptToken, ticket: &T) -> bool {
        self.flight.as_ref().is_some_and(|attempt|
            attempt.token == token && attempt.ticket.as_ref() == Some(ticket)
            && attempt.phase == NpcGoldBuyAttemptPhase::Bound
            && attempt.authority_revision == self.authority_revision
            && self.connection.is_some() && self.available && self.authority_observed && !self.exhausted
        )
    }

    pub fn begin_entry(&mut self, token: NpcGoldBuyAttemptToken, ticket: &T) -> bool {
        if !self.can_begin_entry(token, ticket) { return false; }
        self.flight.as_mut().expect("checked bound flight").phase = NpcGoldBuyAttemptPhase::Entered;
        true
    }

    /// Called only by a host which retained this original transport proof and
    /// observed an exact durable settlement after the complete owner checkpoint
    /// was applied. Ordinary transport receipts never call this method.
    pub fn retire_durably_settled(&mut self, token: NpcGoldBuyAttemptToken, ticket: &T) -> bool {
        if !self.flight.as_ref().is_some_and(|attempt| attempt.token == token
            && attempt.ticket.as_ref() == Some(ticket)
            && matches!(attempt.phase, NpcGoldBuyAttemptPhase::Entered
                | NpcGoldBuyAttemptPhase::Flushed | NpcGoldBuyAttemptPhase::Unknown)) {
            return false;
        }
        let attempt = self.flight.take().expect("checked exact durable settlement");
        self.archive(attempt);
        true
    }

    fn receipt_phase(phase: NpcGoldBuyAttemptPhase, outcome: NpcGoldBuyAttemptOutcome)
        -> Option<NpcGoldBuyAttemptPhase> {
        use NpcGoldBuyAttemptOutcome as Outcome;
        use NpcGoldBuyAttemptPhase as Phase;
        match (phase, outcome) {
            (Phase::Bound, Outcome::DefinitelyUnsent) => Some(Phase::DefinitelyUnsent),
            (Phase::Entered, Outcome::Flushed) => Some(Phase::Flushed),
            (Phase::Entered, Outcome::Unknown) => Some(Phase::Unknown),
            _ => None,
        }
    }

    /// Exact token AND ticket correlation; late receipts never release a new flight.
    /// DefinitelyUnsent is admissible only before entry. Terminal observations are
    /// immutable and duplicates are rejected rather than treated as new evidence.
    pub fn apply_receipt(&mut self, token: NpcGoldBuyAttemptToken, ticket: &T,
        outcome: NpcGoldBuyAttemptOutcome) -> bool {
        if let Some(attempt) = self.flight.as_mut().filter(|attempt|
            attempt.token == token && attempt.ticket.as_ref() == Some(ticket)
        ) {
            let Some(phase) = Self::receipt_phase(attempt.phase, outcome) else { return false; };
            attempt.phase = phase;
            if phase == NpcGoldBuyAttemptPhase::DefinitelyUnsent {
                let attempt = self.flight.take().expect("checked receipt flight");
                self.archive(attempt);
            }
            return true;
        }
        if let Some(attempt) = self.history.iter_mut().find(|attempt|
            attempt.token == token && attempt.ticket.as_ref() == Some(ticket)
        ) {
            let Some(phase) = Self::receipt_phase(attempt.phase, outcome) else { return false; };
            attempt.phase = phase;
            return true;
        }
        false
    }
}

#[cfg(test)]
#[path = "npc_gold_buy_attempt_tests.rs"]
mod tests;

#[cfg(test)]
mod durable_settlement_tests {
    use super::*;
    fn ready() -> NpcGoldBuyAttemptSlot<u64> {
        let mut slot = NpcGoldBuyAttemptSlot::new();
        assert!(slot.observe_connection(NpcGoldBuyConnectionEpoch {run:1,connection:1}));
        assert!(slot.observe_authority("full original source")); slot.set_available(true); slot
    }
    #[test]
    fn native_npc_durable_settlement_rejects_unentered_wrong_ticket_and_duplicate() {
        let mut slot = ready(); let token = slot.reserve().unwrap();
        assert!(!slot.retire_durably_settled(token,&8));
        assert!(slot.bind(token,8)); assert!(!slot.retire_durably_settled(token,&8));
        assert!(slot.begin_entry(token,&8)); assert!(!slot.retire_durably_settled(token,&9));
        assert!(slot.retire_durably_settled(token,&8)); assert!(!slot.retire_durably_settled(token,&8));
        let next = slot.reserve().unwrap(); assert!(next.value() > token.value());
        assert!(slot.bind(next,9)); assert!(slot.begin_entry(next,&9));
        assert!(!slot.retire_durably_settled(token,&8)); assert!(slot.flight().is_some());
    }
    #[test]
    fn native_npc_durable_settlement_preserves_legacy_flush_unknown_and_model_barriers() {
        for outcome in [NpcGoldBuyAttemptOutcome::Flushed,NpcGoldBuyAttemptOutcome::Unknown] {
            let mut slot = ready(); let token = slot.reserve().unwrap();
            assert!(slot.bind(token,8)); assert!(slot.begin_entry(token,&8));
            assert!(slot.apply_receipt(token,&8,outcome));
            assert!(slot.observe_authority("changed complete source")); slot.set_available(true);
            assert!(slot.reserve().is_none()); assert!(!slot.retire_durably_settled(token,&9));
            assert!(slot.retire_durably_settled(token,&8)); assert!(slot.reserve().is_some());
        }
    }
}
