//! Platform-neutral intent metadata.
//!
//! Concrete movement, combat and inventory payloads are intentionally not
//! defined here until the existing wire protocol has been mapped explicitly.

use std::error::Error;
use std::fmt;

use crate::clock::Clock;

/// A client-local sequence assigned before an intent reaches a wire adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IntentSequence(u64);

impl IntentSequence {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Metadata shared by every normalized client intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentEnvelope<T> {
    pub sequence: IntentSequence,
    pub issued_at_ms: u64,
    pub payload: T,
}

/// Returned after the entire sequence space has been issued once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SequenceExhausted;

impl fmt::Display for SequenceExhausted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("client intent sequence exhausted")
    }
}

impl Error for SequenceExhausted {}

/// Issues strictly increasing client-local intent sequence numbers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentSequencer {
    next: Option<IntentSequence>,
}

impl Default for IntentSequencer {
    fn default() -> Self {
        Self::with_next(0)
    }
}

impl IntentSequencer {
    pub const fn with_next(next: u64) -> Self {
        Self {
            next: Some(IntentSequence::new(next)),
        }
    }

    pub fn issue<C, T>(
        &mut self,
        clock: &C,
        payload: T,
    ) -> Result<IntentEnvelope<T>, SequenceExhausted>
    where
        C: Clock,
    {
        let sequence = self.next.ok_or(SequenceExhausted)?;
        self.next = sequence.get().checked_add(1).map(IntentSequence::new);

        Ok(IntentEnvelope {
            sequence,
            issued_at_ms: clock.now_ms(),
            payload,
        })
    }

    pub fn next_sequence(&self) -> Option<IntentSequence> {
        self.next
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::ManualClock;

    #[test]
    fn envelopes_are_monotonic_and_capture_issue_time() {
        let mut clock = ManualClock::new(100);
        let mut sequencer = IntentSequencer::default();

        let first = sequencer.issue(&clock, "walk").expect("first intent");
        clock.advance_ms(600);
        let second = sequencer.issue(&clock, "attack").expect("second intent");

        assert_eq!(first.sequence.get(), 0);
        assert_eq!(first.issued_at_ms, 100);
        assert_eq!(first.payload, "walk");
        assert_eq!(second.sequence.get(), 1);
        assert_eq!(second.issued_at_ms, 700);
    }

    #[test]
    fn final_sequence_is_issued_once_without_wraparound() {
        let clock = ManualClock::default();
        let mut sequencer = IntentSequencer::with_next(u64::MAX);

        assert_eq!(
            sequencer
                .issue(&clock, ())
                .expect("last valid intent")
                .sequence
                .get(),
            u64::MAX
        );
        assert_eq!(sequencer.issue(&clock, ()), Err(SequenceExhausted));
        assert_eq!(sequencer.next_sequence(), None);
    }
}
