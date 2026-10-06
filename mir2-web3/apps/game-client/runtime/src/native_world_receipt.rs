//! Native world-data receipts. These do not acknowledge asset loading, rendering,
//! authentication, or server commands. Hosts must match the exact local request.
use bevy::prelude::Resource;
use serde::Deserialize;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_REQUEST: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldApplyOutcome {
    Applied,
    DecodeRejected,
}

/// Receipt for the last message consumed by the runtime (None if untracked).
/// It may belong to an
/// old session; callers must never infer readiness without an exact ID match.
#[derive(Resource, Default, Debug)]
pub struct NativeWorldReceipt {
    pub last: Option<(u64, WorldApplyOutcome)>,
}

/// Overwrite any wire-supplied tracking field with a process-unique local ID.
/// Returning a ticket does not enqueue or apply the snapshot.
pub fn tag_world_request(json: &mut serde_json::Value) -> Option<u64> {
    let object = json.as_object_mut()?;
    let id = NEXT_REQUEST
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .ok()?;
    object.insert("_nativeWorldRequest".into(), serde_json::json!(id));
    Some(id)
}

#[derive(Deserialize)]
struct Header {
    #[serde(default, rename = "_nativeWorldRequest")]
    id: Option<u64>,
}

pub(crate) fn apply(
    json: &str,
    state: &mut super::RuntimeWorldState,
    snapshots: &mut super::interpolation::SnapshotBuffer,
    receipt_secs: f64,
    receipt: Option<&mut NativeWorldReceipt>,
) -> bool {
    // Header parsing ignores the large scene fields; no second Value tree is
    // retained. Legacy untracked producers continue through the same decoder.
    let id = serde_json::from_str::<Header>(json)
        .ok()
        .and_then(|h| h.id)
        .filter(|id| *id != 0);
    let applied = match serde_json::from_str::<super::WorldSnapshot>(json) {
        Ok(snapshot) => {
            super::apply_world_snapshot(state, snapshots, receipt_secs, snapshot);
            true
        }
        Err(_) => false,
    };
    if let Some(receipt) = receipt {
        receipt.last = id.map(|id| {
            (
                id,
                if applied {
                    WorldApplyOutcome::Applied
                } else {
                    WorldApplyOutcome::DecodeRejected
                },
            )
        });
    }
    applied
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn production_queue_only_receipts_the_consumed_snapshot_after_update() {
        use super::super::{ingest_pending_world_state, native_ingest, RuntimeWorldState};
        use bevy::prelude::*;
        let _guard = native_ingest::native_queue_test_guard();
        let mut app = App::new();
        app.init_resource::<RuntimeWorldState>()
            .init_resource::<super::super::interpolation::SnapshotBuffer>()
            .init_resource::<Time>()
            .init_resource::<NativeWorldReceipt>()
            .insert_resource(native_ingest::NativeInbound::new())
            .add_systems(Update, ingest_pending_world_state);
        let mut wire = json!({"playerObjectId":"42", "entities":[{
            "objectId":"42","kind":"selfPlayer","name":"Fixture","x":302,"y":634}]});
        let old = tag_world_request(&mut wire).unwrap();
        assert!(native_ingest::push_native_world_state(wire.to_string()));
        wire["entities"][0]["x"] = json!(303);
        let current = tag_world_request(&mut wire).unwrap();
        assert_ne!(old, current);
        assert!(native_ingest::push_native_world_state(wire.to_string()));
        assert!(app.world().resource::<NativeWorldReceipt>().last.is_none());
        app.update();
        assert_eq!(
            app.world().resource::<NativeWorldReceipt>().last,
            Some((current, WorldApplyOutcome::Applied))
        );
        assert_eq!(
            app.world()
                .resource::<RuntimeWorldState>()
                .snapshot
                .as_ref()
                .unwrap()
                .entities[0]
                .x,
            303
        );

        wire["entities"][0]["x"] = json!("bad coordinate");
        let rejected = tag_world_request(&mut wire).unwrap();
        assert!(native_ingest::push_native_world_state(wire.to_string()));
        app.update();
        assert_eq!(
            app.world().resource::<NativeWorldReceipt>().last,
            Some((rejected, WorldApplyOutcome::DecodeRejected))
        );
        assert_eq!(
            app.world()
                .resource::<RuntimeWorldState>()
                .snapshot
                .as_ref()
                .unwrap()
                .entities[0]
                .x,
            303
        );
    }

    #[test]
    fn receipt_is_published_after_shared_state_application_not_tagging() {
        let mut wire = json!({"_nativeWorldRequest":999, "playerObjectId":"42",
            "entities":[{"objectId":"42","kind":"selfPlayer","name":"Fixture","x":302,"y":634}]});
        let id = tag_world_request(&mut wire).unwrap();
        let mut receipt = NativeWorldReceipt::default();
        let mut state = super::super::RuntimeWorldState::default();
        let mut snapshots = super::super::interpolation::SnapshotBuffer::default();
        assert!(receipt.last.is_none());
        assert!(state.snapshot.is_none());
        assert!(apply(
            &wire.to_string(),
            &mut state,
            &mut snapshots,
            1.0,
            Some(&mut receipt)
        ));
        assert_eq!(receipt.last, Some((id, WorldApplyOutcome::Applied)));
        let snapshot = state.snapshot.as_ref().unwrap();
        assert_eq!(snapshot.player_object_id.as_deref(), Some("42"));
        assert_eq!((snapshot.entities[0].x, snapshot.entities[0].y), (302, 634));

        wire["entities"][0]["x"] = json!("not an integer");
        let rejected = tag_world_request(&mut wire).unwrap();
        assert_ne!(id, rejected);
        assert!(!apply(
            &wire.to_string(),
            &mut state,
            &mut snapshots,
            2.0,
            Some(&mut receipt)
        ));
        assert_eq!(
            receipt.last,
            Some((rejected, WorldApplyOutcome::DecodeRejected))
        );
        assert_eq!(state.snapshot.as_ref().unwrap().entities[0].x, 302);
    }

    #[test]
    fn untracked_legacy_world_does_not_create_or_reuse_a_receipt() {
        let mut receipt = NativeWorldReceipt {
            last: Some((1, WorldApplyOutcome::Applied)),
        };
        let mut state = super::super::RuntimeWorldState::default();
        let mut snapshots = super::super::interpolation::SnapshotBuffer::default();
        assert!(apply(
            "{}",
            &mut state,
            &mut snapshots,
            0.0,
            Some(&mut receipt)
        ));
        assert!(receipt.last.is_none());
        assert!(!apply(
            "broken JSON",
            &mut state,
            &mut snapshots,
            0.0,
            Some(&mut receipt)
        ));
        assert!(receipt.last.is_none());
        assert!(tag_world_request(&mut json!([])).is_none());
    }
}
