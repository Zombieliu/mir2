//! Platform-neutral intent metadata and source-mapped inventory plans.
//!
//! The Bag-to-Belt plan below maps the existing Crystal raw inventory endpoints.
//! It grants no ownership, reserves no transport, and never changes inventory.

use std::error::Error;
use std::fmt;

use crate::clock::Clock;
use crate::equipment_pending::InventoryPlacement;

/// Crystal's unified inventory length includes six belt cells. The first
/// expansion adds eight cells; subsequent expansions add four, up to eighty
/// bag cells. Preserve Native's legacy invalid-value fallback to length 46.
pub fn canonical_inventory_capacity(value: u16) -> u16 {
    if inventory_bag_capacity(value).is_some() {
        value
    } else {
        46
    }
}

/// Validate a concrete unified inventory length and return its bag-cell count.
/// Unknown or noncanonical values cannot prove that Bag2 exists.
pub fn inventory_bag_capacity(inventory_capacity: u16) -> Option<u16> {
    if inventory_capacity == 46
        || ((54..=86).contains(&inventory_capacity) && (inventory_capacity - 54) % 4 == 0)
    {
        inventory_capacity.checked_sub(6)
    } else {
        None
    }
}

/// Endpoints for the existing MoveItem packet with its grid fixed to `belt`.
/// Source is Crystal's unified inventory index; target is belt index 0 through 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BagToBeltMovePlan {
    pub unique_id: u64,
    pub from: i32,
    pub to: i32,
}

/// Plan addresses from a current host-supplied placement. The host still owns
/// live source custody, gesture, parcel locks, pending state, and final sending.
/// This intentionally imposes no invented item-type or empty-target restriction.
pub fn plan_bag_to_belt_move(
    inventory_capacity: u16,
    source: InventoryPlacement,
    target_slot: u8,
) -> Option<BagToBeltMovePlan> {
    let bag_capacity = inventory_bag_capacity(inventory_capacity)?;
    if source.container != 0 || source.slot >= u32::from(bag_capacity) || target_slot >= 6 {
        return None;
    }
    Some(BagToBeltMovePlan {
        unique_id: source.unique_id?,
        from: i32::try_from(source.slot.checked_add(6)?).ok()?,
        to: i32::from(target_slot),
    })
}

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

    #[test]
    fn crystal_inventory_capacity_keeps_native_fallback_and_excludes_six_belt_cells() {
        for (capacity, bag_cells) in [
            (46, 40), (54, 48), (58, 52), (62, 56), (66, 60),
            (70, 64), (74, 68), (78, 72), (82, 76), (86, 80),
        ] {
            assert_eq!(canonical_inventory_capacity(capacity), capacity);
            assert_eq!(inventory_bag_capacity(capacity), Some(bag_cells));
        }
        for capacity in [0, 45, 47, 50, 53, 55, 87, 100, u16::MAX] {
            assert_eq!(canonical_inventory_capacity(capacity), 46);
            assert_eq!(inventory_bag_capacity(capacity), None);
        }
    }

    #[test]
    fn bag_to_belt_preserves_raw_endpoint_goldens_and_zero_identity() {
        for (capacity, slot, unique_id, target, from) in [
            (46, 0, 0, 0, 6), (46, 2, 7001, 0, 8), (46, 39, 42, 5, 45),
            (54, 40, 43, 0, 46), (54, 47, 44, 5, 53), (86, 79, u64::MAX, 5, 85),
        ] {
            let source = InventoryPlacement { unique_id: Some(unique_id), container: 0, slot };
            assert_eq!(plan_bag_to_belt_move(capacity, source, target),
                Some(BagToBeltMovePlan { unique_id, from, to: i32::from(target) }));
            assert_eq!(source.slot, slot, "planning cannot rewrite a placement");
        }
    }

    #[test]
    fn bag_to_belt_rejects_locked_expansion_and_unrepresentable_endpoints() {
        let place = |slot| InventoryPlacement { unique_id: Some(0), container: 0, slot };
        assert_eq!(plan_bag_to_belt_move(46, place(40), 0), None);
        assert_eq!(plan_bag_to_belt_move(47, place(40), 0), None);
        assert_eq!(plan_bag_to_belt_move(54, place(48), 0), None);
        assert_eq!(plan_bag_to_belt_move(86, place(80), 0), None);
        assert_eq!(plan_bag_to_belt_move(86, place(u32::MAX), 0), None);
        assert_eq!(plan_bag_to_belt_move(46, place(0), 6), None);
        assert_eq!(plan_bag_to_belt_move(46, place(0), u8::MAX), None);
        assert_eq!(plan_bag_to_belt_move(46, InventoryPlacement { unique_id: None, ..place(0) }, 0), None);
        for container in [1, 2, 3, 4, u8::MAX] {
            assert_eq!(plan_bag_to_belt_move(46, InventoryPlacement { container, ..place(0) }, 0), None);
        }
    }
}
