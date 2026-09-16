//! Monotonic online duration. Persistence stores only time remaining, never an Instant.
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::time::Instant;

#[derive(Debug, Clone)]
pub(in super::super) struct RealTimeBuffDuration {
    remaining_at_anchor_ms: u64,
    anchor: Instant,
}
impl RealTimeBuffDuration {
    pub(in super::super) fn new(remaining_ms: u64) -> Self {
        Self {
            remaining_at_anchor_ms: remaining_ms,
            anchor: Instant::now(),
        }
    }
    pub(in super::super) fn remaining_ms(&self) -> u64 {
        self.remaining_at(Instant::now())
    }
    fn remaining_at(&self, now: Instant) -> u64 {
        self.remaining_at_anchor_ms.saturating_sub(
            now.saturating_duration_since(self.anchor)
                .as_millis()
                .min(u128::from(u64::MAX)) as u64,
        )
    }
    #[cfg(test)]
    pub(in super::super) fn elapse_for_test(&mut self, ms: u64) {
        self.remaining_at_anchor_ms = self.remaining_ms().saturating_sub(ms);
        self.anchor = Instant::now();
    }
}
impl Serialize for RealTimeBuffDuration {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(self.remaining_ms())
    }
}
impl<'de> Deserialize<'de> for RealTimeBuffDuration {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        u64::deserialize(deserializer).map(Self::new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn creature_duration_clone_preserves_anchor_and_monotonic_expiry() {
        let duration = RealTimeBuffDuration::new(60_000);
        let clone = duration.clone();
        assert_eq!(duration.anchor, clone.anchor);
        assert_eq!(
            clone.remaining_at(duration.anchor + Duration::from_millis(59_999)),
            1
        );
        assert_eq!(
            clone.remaining_at(duration.anchor + Duration::from_millis(60_000)),
            0
        );
    }
    #[test]
    fn creature_duration_serialization_preserves_remaining_and_legacy_absence() {
        let mut duration = RealTimeBuffDuration::new(60_000);
        duration.elapse_for_test(20_000);
        let encoded = serde_json::to_string(&duration).unwrap();
        let stored: u64 = serde_json::from_str(&encoded).unwrap();
        assert!((39_000..=40_000).contains(&stored));
        let restored: RealTimeBuffDuration = serde_json::from_str(&encoded).unwrap();
        assert!(restored.remaining_ms() <= stored);
        assert!(restored.remaining_ms() >= stored - 1000);
        assert!(stored <= duration.remaining_at_anchor_ms);
        let anchor = duration.anchor;
        for _ in 0..1000 {
            let _ = serde_json::to_string(&duration).unwrap();
        }
        assert_eq!(duration.anchor, anchor);
    }
}
