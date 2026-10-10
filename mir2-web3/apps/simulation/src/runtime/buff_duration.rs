//! Monotonic online duration, or non-ticking original Infinite ExpireTime
//! metadata. Timed persistence remains a scalar millisecond value; no Instant
//! is persisted. Infinite metadata has a distinct validated object shape.
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::time::Instant;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use rand_core::{OsRng,RngCore};
static NEXT_BUFF_DURATION_INSTANCE:OnceLock<AtomicU64>=OnceLock::new();
fn next_duration_instance()->u64 {
    NEXT_BUFF_DURATION_INSTANCE.get_or_init(||AtomicU64::new((OsRng.next_u64() & (u64::MAX >> 1)) | 1))
        .fetch_update(Ordering::Relaxed,Ordering::Relaxed,|value|value.checked_add(1))
        .expect("buff duration instance counter exhausted")
}

#[derive(Debug, Clone)]
pub(in super::super) struct RealTimeBuffDuration {
    remaining_at_anchor_ms: u64,
    anchor: Instant,
    paused: bool,
    instance_id:u64,
    source_infinite_duration_ms: Option<u64>,
}
impl RealTimeBuffDuration {
    pub(in super::super) fn new(remaining_ms: u64) -> Self {
        Self {
            remaining_at_anchor_ms: remaining_ms,
            anchor: Instant::now(),
            paused:false,
            instance_id:next_duration_instance(),
            source_infinite_duration_ms: None,
        }
    }
    pub(in super::super) fn new_infinite(source_duration_ms: u64) -> Self {
        let mut duration = Self::new(source_duration_ms);
        duration.source_infinite_duration_ms = Some(source_duration_ms);
        duration
    }
    pub(in super::super) fn is_infinite(&self) -> bool { self.source_infinite_duration_ms.is_some() }
    pub(in super::super) fn remaining_ms(&self) -> u64 {
        self.remaining_at(Instant::now())
    }
    fn remaining_at(&self, now: Instant) -> u64 {
        if let Some(duration_ms) = self.source_infinite_duration_ms { return duration_ms; }
        if self.paused { return self.remaining_at_anchor_ms; }
        self.remaining_at_anchor_ms.saturating_sub(
            now.saturating_duration_since(self.anchor)
                .as_millis()
                .min(u128::from(u64::MAX)) as u64,
        )
    }
    pub(in super::super) fn is_paused(&self)->bool{self.paused}
    pub(in super::super) fn instance_id(&self)->u64{self.instance_id}
    pub(in super::super) fn set_remaining_and_paused(&mut self,remaining_ms:u64,paused:bool) {
        if self.is_infinite() { return; }
        self.remaining_at_anchor_ms=remaining_ms;
        self.anchor=Instant::now();
        self.paused=paused;
    }
    pub(in super::super) fn set_paused(&mut self,paused:bool) {
        if self.is_infinite() { return; }
        if self.paused==paused{return;}
        self.set_remaining_and_paused(self.remaining_ms(),paused);
    }
    #[cfg(test)]
    pub(in super::super) fn elapse_for_test(&mut self, ms: u64) {
        self.remaining_at_anchor_ms = self.remaining_ms().saturating_sub(ms);
        self.anchor = Instant::now();
    }
}
impl Serialize for RealTimeBuffDuration {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if let Some(duration_ms) = self.source_infinite_duration_ms {
            InfiniteDurationMetadata { source_infinite_duration_ms: duration_ms }.serialize(serializer)
        } else { serializer.serialize_u64(self.remaining_ms()) }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InfiniteDurationMetadata { source_infinite_duration_ms: u64 }
#[derive(Deserialize)]
#[serde(untagged)]
enum PersistedDuration { Timed(u64), Infinite(InfiniteDurationMetadata) }
impl<'de> Deserialize<'de> for RealTimeBuffDuration {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        PersistedDuration::deserialize(deserializer).map(|duration| match duration {
            PersistedDuration::Timed(ms) => Self::new(ms),
            PersistedDuration::Infinite(metadata) => Self::new_infinite(metadata.source_infinite_duration_ms),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn source_infinite_expire_time_is_metadata_and_cold_round_trip_cannot_start_a_countdown() {
        for ms in [0, 3_600_000] {
            let mut duration = RealTimeBuffDuration::new_infinite(ms);
            assert!(duration.is_infinite());
            assert_eq!(duration.remaining_at(duration.anchor + Duration::from_secs(86_400)), ms);
            duration.set_paused(true);
            duration.set_remaining_and_paused(9, true);
            assert!(!duration.is_paused());
            assert_eq!(duration.remaining_ms(), ms);
            let encoded = serde_json::to_value(&duration).unwrap();
            assert_eq!(encoded, serde_json::json!({"sourceInfiniteDurationMs":ms}));
            let restored: RealTimeBuffDuration = serde_json::from_value(encoded).unwrap();
            assert!(restored.is_infinite());
            assert_eq!(restored.remaining_at(restored.anchor + Duration::from_secs(86_400)), ms);
        }
        for invalid in ["{}", "{\"sourceInfiniteDurationMs\":-1}", "{\"sourceInfiniteDurationMs\":1,\"paused\":true}", "-1", "null"] {
            assert!(serde_json::from_str::<RealTimeBuffDuration>(invalid).is_err(), "{invalid}");
        }
    }
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
