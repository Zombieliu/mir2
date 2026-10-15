//! Pure reservation arithmetic. Not registered in production yet.
//!
//! A source adapter must commit `ReservationPlan` with exact version/high-water
//! CAS before calling `CommittedItemIds::from_committed_source`. The constructor
//! is crate-private and must never be exposed through a client packet or save.
//! A lease is deliberately neither Clone nor serializable. Dropping it retires
//! unused IDs; restoring a World checkpoint must not restore its cursor.

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ReservationError {
    Empty,
    TooLarge,
    Exhausted,
    InvalidSourceVersion,
    ReceiptMismatch,
}

/// A bounded allocation amortizes source writes without a huge accidental burn.
pub(crate) const MAX_RESERVATION_IDS: u32 = 4096;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ReservationPlan {
    expected_version: i64,
    old_high: u64,
    new_high: u64,
    count: u32,
}

impl ReservationPlan {
    pub(crate) fn prepare(
        expected_version: i64,
        old_high: u64,
        count: u32,
    ) -> Result<Self, ReservationError> {
        if expected_version <= 0 || expected_version == i64::MAX {
            return Err(ReservationError::InvalidSourceVersion);
        }
        if count == 0 {
            return Err(ReservationError::Empty);
        }
        if count > MAX_RESERVATION_IDS {
            return Err(ReservationError::TooLarge);
        }
        let new_high = old_high
            .checked_add(u64::from(count))
            .ok_or(ReservationError::Exhausted)?;
        Ok(Self {
            expected_version,
            old_high,
            new_high,
            count,
        })
    }

    pub(crate) fn source_cas(&self) -> (i64, u64, u64) {
        (self.expected_version, self.old_high, self.new_high)
    }
}

/// One live single-writer cursor backed by a previously committed source range.
///
/// Never put this inside a Clone World resource/checkpoint or reconstruct a
/// cursor from a persisted receipt on process startup. Restart reserves anew.
#[derive(Debug)]
pub(crate) struct CommittedItemIds {
    next: u64,
    remaining: u32,
}

impl CommittedItemIds {
    /// Source adapter only: its actual CAS receipt must match both fields. This
    /// pure module checks arithmetic, not database authenticity or owner fencing.
    pub(crate) fn from_committed_source(
        plan: ReservationPlan,
        committed_version: i64,
        committed_high: u64,
    ) -> Result<Self, ReservationError> {
        if committed_version != plan.expected_version + 1 || committed_high != plan.new_high {
            return Err(ReservationError::ReceiptMismatch);
        }
        Ok(Self {
            next: plan.old_high + 1,
            remaining: plan.count,
        })
    }

    /// Consume the full request or consume nothing. IDs returned by a preview
    /// or aborted business plan stay consumed: caller must never rewind them.
    pub(crate) fn take(&mut self, count: u32) -> Result<Vec<u64>, ReservationError> {
        if count == 0 {
            return Err(ReservationError::Empty);
        }
        if count > self.remaining {
            return Err(ReservationError::Exhausted);
        }
        let last = self
            .next
            .checked_add(u64::from(count) - 1)
            .ok_or(ReservationError::Exhausted)?;
        let ids = (self.next..=last).collect();
        self.remaining -= count;
        // u64::MAX is a valid last identity, but must never wrap into zero.
        self.next = if self.remaining == 0 { last } else { last + 1 };
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn committed(high: u64, count: u32) -> CommittedItemIds {
        let plan = ReservationPlan::prepare(7, high, count).unwrap();
        let (_, _, desired) = plan.source_cas();
        CommittedItemIds::from_committed_source(plan, 8, desired).unwrap()
    }

    #[test]
    fn item_reservation_aborted_preview_burns_and_restart_skips_unused_range() {
        let mut live = committed(100, 4);
        let discarded_preview = live.take(2).unwrap();
        assert_eq!(discarded_preview, [101, 102]);
        assert_eq!(live.take(1).unwrap(), [103]);
        drop(live); // 104 is retired with the process, not replayed.
        let mut restarted = committed(104, 2);
        assert_eq!(restarted.take(2).unwrap(), [105, 106]);
    }

    #[test]
    fn item_reservation_bulk_failure_does_not_partially_consume() {
        let mut live = committed(0, 2);
        assert_eq!(live.take(3), Err(ReservationError::Exhausted));
        assert_eq!(live.take(0), Err(ReservationError::Empty));
        assert_eq!(live.take(2).unwrap(), [1, 2]);
        assert_eq!(live.take(1), Err(ReservationError::Exhausted));
    }

    #[test]
    fn item_reservation_full_u64_boundary_never_wraps() {
        let mut live = committed(u64::MAX - 1, 1);
        assert_eq!(live.take(1).unwrap(), [u64::MAX]);
        assert_eq!(live.take(1), Err(ReservationError::Exhausted));
        assert_eq!(
            ReservationPlan::prepare(7, u64::MAX, 1),
            Err(ReservationError::Exhausted)
        );
    }

    #[test]
    fn item_reservation_rejects_missing_or_mismatched_source_receipt() {
        assert!(matches!(
            CommittedItemIds::from_committed_source(
                ReservationPlan::prepare(7, 100, 3).unwrap(),
                7,
                103
            ),
            Err(ReservationError::ReceiptMismatch)
        ));
        assert!(matches!(
            CommittedItemIds::from_committed_source(
                ReservationPlan::prepare(7, 100, 3).unwrap(),
                8,
                102
            ),
            Err(ReservationError::ReceiptMismatch)
        ));
        assert_eq!(
            ReservationPlan::prepare(0, 0, 1),
            Err(ReservationError::InvalidSourceVersion)
        );
        assert_eq!(
            ReservationPlan::prepare(i64::MAX, 0, 1),
            Err(ReservationError::InvalidSourceVersion)
        );
        assert_eq!(
            ReservationPlan::prepare(1, 0, MAX_RESERVATION_IDS + 1),
            Err(ReservationError::TooLarge)
        );
    }
}
