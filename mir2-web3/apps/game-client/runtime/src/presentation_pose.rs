//! Per-frame presentation poses shared by the Bevy renderer and DOM overlays.
//!
//! The buffer records the exact screen-space offsets used for Bevy transforms.
//! It is presentation-only and never changes authoritative movement state.

use std::cell::{Cell, RefCell};

use bevy::prelude::*;
use js_sys::Function;
use serde::Serialize;
use wasm_bindgen::prelude::*;

const PRESENTATION_POSE_VERSION: u8 = 1;
const MAX_ENTITY_POSES: usize = 256;
const LOCAL_COMMAND_PROMOTION_TOLERANCE_PX: f32 = 0.001;

thread_local! {
    static LATEST_JSON: RefCell<Option<String>> = const { RefCell::new(None) };
    static PENDING_ENABLED: Cell<Option<bool>> = const { Cell::new(None) };
    static PRESENTATION_POSE_SINK: RefCell<Option<Function>> = const { RefCell::new(None) };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum EntityPoseSource {
    LocalCommand,
    RemotePacket,
    SnapshotWindow,
    Static,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CameraPoseSource {
    LocalCommand,
    SelfWindow,
    Static,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EntityPresentationPose {
    pub(crate) object_id: String,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) source: EntityPoseSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) motion: Option<EntityPresentationMotion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EntityPresentationMotion {
    pub(crate) frame_index: u8,
    pub(crate) phase_count: u8,
    pub(crate) mode: String,
    pub(crate) direction: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CameraPresentationPose {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) source: CameraPoseSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PresentationGridCenter {
    pub(crate) x: i32,
    pub(crate) y: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PresentationPoseProvenance {
    pub(crate) map_center: Option<PresentationGridCenter>,
    pub(crate) entity_center: Option<PresentationGridCenter>,
    pub(crate) applied_map_revision: Option<u64>,
}

impl Default for CameraPresentationPose {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            source: CameraPoseSource::Static,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PresentationPoseSnapshot {
    pub(crate) ready: bool,
    pub(crate) version: u8,
    pub(crate) frame_id: u64,
    pub(crate) generated_at_ms: f64,
    pub(crate) bridge_enabled: bool,
    pub(crate) renderer_enabled: bool,
    pub(crate) provenance: PresentationPoseProvenance,
    pub(crate) camera: CameraPresentationPose,
    pub(crate) entities: Vec<EntityPresentationPose>,
    pub(crate) frame_overflow_count: u64,
    pub(crate) total_overflow_count: u64,
}

impl Default for PresentationPoseSnapshot {
    fn default() -> Self {
        Self {
            ready: false,
            version: PRESENTATION_POSE_VERSION,
            frame_id: 0,
            generated_at_ms: 0.0,
            bridge_enabled: false,
            renderer_enabled: false,
            provenance: PresentationPoseProvenance::default(),
            camera: CameraPresentationPose::default(),
            entities: Vec::new(),
            frame_overflow_count: 0,
            total_overflow_count: 0,
        }
    }
}

#[derive(Debug, Default, Resource)]
pub(crate) struct PresentationPoseBuffer {
    enabled: bool,
    frame_id: u64,
    generated_at_ms: f64,
    renderer_enabled: bool,
    provenance: PresentationPoseProvenance,
    camera: CameraPresentationPose,
    local_self_motion: Option<EntityPresentationMotion>,
    entities: Vec<EntityPresentationPose>,
    frame_overflow_count: u64,
    total_overflow_count: u64,
}

impl PresentationPoseBuffer {
    pub(crate) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.entities.clear();
        }
    }

    pub(crate) fn begin_frame(&mut self, generated_at_ms: f64, renderer_enabled: bool) {
        self.frame_id = self.frame_id.saturating_add(1);
        self.generated_at_ms = generated_at_ms;
        self.renderer_enabled = renderer_enabled;
        self.camera = CameraPresentationPose::default();
        self.local_self_motion = None;
        self.entities.clear();
        self.frame_overflow_count = 0;
    }

    pub(crate) fn set_applied_map_provenance(
        &mut self,
        center: Option<PresentationGridCenter>,
        revision: Option<u64>,
    ) {
        self.provenance.map_center = center;
        self.provenance.applied_map_revision = revision;
    }

    pub(crate) fn set_applied_entity_center(&mut self, center: Option<PresentationGridCenter>) {
        self.provenance.entity_center = center;
    }

    pub(crate) fn applied_map_center(&self) -> Option<PresentationGridCenter> {
        self.provenance.map_center
    }

    pub(crate) fn coherent_applied_center(&self) -> Option<PresentationGridCenter> {
        match (self.provenance.map_center, self.provenance.entity_center) {
            (Some(map_center), Some(entity_center)) if map_center == entity_center => {
                Some(map_center)
            }
            _ => None,
        }
    }

    pub(crate) fn record_entity(
        &mut self,
        object_id: &str,
        offset: Vec2,
        source: EntityPoseSource,
    ) {
        if !self.enabled || !self.renderer_enabled || object_id.is_empty() || !offset.is_finite() {
            return;
        }

        let motion = (source == EntityPoseSource::LocalCommand)
            .then(|| self.local_self_motion.clone())
            .flatten();
        if let Some(existing) = self
            .entities
            .iter_mut()
            .find(|pose| pose.object_id == object_id)
        {
            existing.x = offset.x;
            existing.y = offset.y;
            existing.source = source;
            existing.motion = motion;
            return;
        }

        if self.entities.len() >= MAX_ENTITY_POSES {
            self.frame_overflow_count = self.frame_overflow_count.saturating_add(1);
            self.total_overflow_count = self.total_overflow_count.saturating_add(1);
            return;
        }

        self.entities.push(EntityPresentationPose {
            object_id: object_id.to_owned(),
            x: offset.x,
            y: offset.y,
            source,
            motion,
        });
    }

    pub(crate) fn set_local_self_motion(&mut self, motion: Option<EntityPresentationMotion>) {
        self.local_self_motion = motion;
    }

    pub(crate) fn set_camera(&mut self, offset: Vec2, source: CameraPoseSource) {
        if !self.renderer_enabled || !offset.is_finite() {
            return;
        }
        self.camera = CameraPresentationPose {
            x: offset.x,
            y: offset.y,
            source,
        };
    }

    pub(crate) fn promote_matching_self_window_to_local_command(
        &mut self,
        entity_offset: Vec2,
        motion: Option<EntityPresentationMotion>,
    ) -> bool {
        if !self.renderer_enabled
            || self.camera.source != CameraPoseSource::SelfWindow
            || !entity_offset.is_finite()
        {
            return false;
        }

        let delta = self.self_entity_offset() - entity_offset;
        if delta.x.abs() > LOCAL_COMMAND_PROMOTION_TOLERANCE_PX
            || delta.y.abs() > LOCAL_COMMAND_PROMOTION_TOLERANCE_PX
        {
            return false;
        }

        self.set_local_self_motion(motion);
        self.set_camera(-entity_offset, CameraPoseSource::LocalCommand);
        true
    }

    /// Reconcile a local-command pose after map/entity layers commit a new center.
    ///
    /// `begin_frame` can only see the previous entity provenance while the map
    /// system is applying the next center. In that mixed-center tick it keeps a
    /// settled local/static camera. Once the entity system confirms the same
    /// center, replace that stale camera before transforms and the pose snapshot
    /// are published. A live TypeScript window still uses the stricter pixel-
    /// matching promotion guard so this cannot introduce a takeover jump.
    pub(crate) fn reconcile_local_command_for_applied_center(
        &mut self,
        entity_offset: Vec2,
        motion: Option<EntityPresentationMotion>,
    ) -> bool {
        if self.camera.source == CameraPoseSource::SelfWindow {
            return self.promote_matching_self_window_to_local_command(entity_offset, motion);
        }
        if !self.renderer_enabled || !entity_offset.is_finite() {
            return false;
        }

        self.set_local_self_motion(motion);
        self.set_camera(-entity_offset, CameraPoseSource::LocalCommand);
        true
    }

    pub(crate) fn camera_screen_offset(&self) -> Vec2 {
        Vec2::new(self.camera.x, self.camera.y)
    }

    pub(crate) fn self_entity_offset(&self) -> Vec2 {
        -self.camera_screen_offset()
    }

    pub(crate) fn camera_source(&self) -> CameraPoseSource {
        self.camera.source
    }

    pub(crate) fn publish(&self) -> String {
        let mut entities = self.entities.clone();
        entities.sort_by(|left, right| left.object_id.cmp(&right.object_id));
        let coherent_center = matches!(
            (self.provenance.map_center, self.provenance.entity_center),
            (Some(map), Some(entity)) if map == entity
        );
        let ready = self.enabled
            && self.renderer_enabled
            && self.provenance.applied_map_revision.is_some()
            && coherent_center
            && !entities.is_empty();
        let snapshot = PresentationPoseSnapshot {
            ready,
            version: PRESENTATION_POSE_VERSION,
            frame_id: self.frame_id,
            generated_at_ms: self.generated_at_ms,
            bridge_enabled: self.enabled,
            renderer_enabled: self.renderer_enabled,
            provenance: self.provenance,
            camera: self.camera,
            entities,
            frame_overflow_count: self.frame_overflow_count,
            total_overflow_count: self.total_overflow_count,
        };
        let json = serialize_snapshot(&snapshot);
        LATEST_JSON.with(|latest| {
            *latest.borrow_mut() = Some(json.clone());
        });
        json
    }

    pub(crate) fn publish_with<F>(&self, dispatch: F) -> String
    where
        F: FnOnce(&str),
    {
        let json = self.publish();
        dispatch(&json);
        json
    }
}

fn serialize_snapshot(snapshot: &PresentationPoseSnapshot) -> String {
    serde_json::to_string(snapshot).unwrap_or_else(|_| {
        r#"{"ready":false,"serializationError":"presentation pose was not finite"}"#.to_owned()
    })
}

pub(crate) fn get_presentation_pose_json() -> String {
    LATEST_JSON
        .with(|latest| latest.borrow().clone())
        .unwrap_or_else(|| serialize_snapshot(&PresentationPoseSnapshot::default()))
}

pub(crate) fn set_presentation_pose_sink(callback: Function) {
    PRESENTATION_POSE_SINK.with(|sink| {
        sink.borrow_mut().replace(callback);
    });
}

pub(crate) fn clear_presentation_pose_sink() {
    PRESENTATION_POSE_SINK.with(|sink| {
        sink.borrow_mut().take();
    });
}

pub(crate) fn push_presentation_pose_json(json: &str) {
    // Do not hold a RefCell borrow while invoking user code: the sink may clear itself.
    let callback = PRESENTATION_POSE_SINK.with(|sink| sink.borrow().clone());
    let Some(callback) = callback else {
        return;
    };
    let _ = callback.call1(&JsValue::NULL, &JsValue::from_str(json));
}

pub(crate) fn set_presentation_pose_enabled(enabled: bool) {
    PENDING_ENABLED.with(|pending| pending.set(Some(enabled)));
}

pub(crate) fn take_pending_enabled() -> Option<bool> {
    PENDING_ENABLED.with(|pending| pending.replace(None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_snapshot_is_not_ready() {
        LATEST_JSON.with(|latest| latest.borrow_mut().take());
        let value: serde_json::Value =
            serde_json::from_str(&get_presentation_pose_json()).expect("valid json");
        assert_eq!(value["ready"], false);
        assert_eq!(value["version"], PRESENTATION_POSE_VERSION);
        assert!(value["provenance"]["mapCenter"].is_null());
        assert_eq!(value["entities"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn publishes_exact_camera_and_entity_offsets() {
        let mut buffer = PresentationPoseBuffer::default();
        buffer.set_enabled(true);
        buffer
            .set_applied_map_provenance(Some(PresentationGridCenter { x: 330, y: 268 }), Some(17));
        buffer.set_applied_entity_center(Some(PresentationGridCenter { x: 330, y: 268 }));
        buffer.begin_frame(1_234.0, true);
        buffer.record_entity(
            "remote",
            Vec2::new(-40.0, 16.0),
            EntityPoseSource::RemotePacket,
        );
        buffer.record_entity(
            "self",
            Vec2::new(-40.0, 0.0),
            EntityPoseSource::SnapshotWindow,
        );
        buffer.set_camera(Vec2::new(40.0, 0.0), CameraPoseSource::SelfWindow);
        let published = buffer.publish();

        let value: serde_json::Value = serde_json::from_str(&published).expect("valid json");
        assert_eq!(published, get_presentation_pose_json());
        assert_eq!(value["ready"], true);
        assert_eq!(value["bridgeEnabled"], true);
        assert_eq!(value["rendererEnabled"], true);
        assert_eq!(value["generatedAtMs"], 1_234.0);
        assert_eq!(value["provenance"]["mapCenter"]["x"], 330);
        assert_eq!(value["provenance"]["entityCenter"]["x"], 330);
        assert_eq!(value["provenance"]["appliedMapRevision"], 17);
        assert_eq!(value["camera"]["x"], 40.0);
        assert_eq!(value["camera"]["source"], "selfWindow");
        assert_eq!(value["entities"][0]["objectId"], "remote");
        assert_eq!(value["entities"][0]["source"], "remotePacket");
        assert_eq!(value["entities"][1]["objectId"], "self");
    }

    #[test]
    fn mixed_or_incomplete_scene_frames_are_not_ready() {
        let mut buffer = PresentationPoseBuffer::default();
        buffer.set_enabled(true);
        buffer
            .set_applied_map_provenance(Some(PresentationGridCenter { x: 330, y: 268 }), Some(17));
        buffer.set_applied_entity_center(Some(PresentationGridCenter { x: 331, y: 268 }));
        buffer.begin_frame(1_234.0, true);
        buffer.record_entity("self", Vec2::ZERO, EntityPoseSource::SnapshotWindow);

        let mixed: serde_json::Value =
            serde_json::from_str(&buffer.publish()).expect("valid mixed pose json");
        assert_eq!(mixed["ready"], false);

        buffer.set_applied_entity_center(Some(PresentationGridCenter { x: 330, y: 268 }));
        buffer.begin_frame(1_250.0, true);
        let empty: serde_json::Value =
            serde_json::from_str(&buffer.publish()).expect("valid empty pose json");
        assert_eq!(empty["ready"], false);
    }

    #[test]
    fn coherent_applied_center_tracks_rendered_layers_not_requested_snapshot() {
        let mut buffer = PresentationPoseBuffer::default();
        assert_eq!(buffer.coherent_applied_center(), None);

        let rendered_center = PresentationGridCenter { x: 332, y: 275 };
        buffer.set_applied_map_provenance(Some(rendered_center), Some(17));
        assert_eq!(buffer.coherent_applied_center(), None);

        buffer.set_applied_entity_center(Some(PresentationGridCenter { x: 331, y: 275 }));
        assert_eq!(buffer.coherent_applied_center(), None);

        buffer.set_applied_entity_center(Some(rendered_center));
        assert_eq!(buffer.coherent_applied_center(), Some(rendered_center));
    }

    #[test]
    fn duplicate_entity_updates_in_place() {
        let mut buffer = PresentationPoseBuffer::default();
        buffer.set_enabled(true);
        buffer.begin_frame(10.0, true);
        buffer.record_entity("p1", Vec2::ZERO, EntityPoseSource::Static);
        buffer.record_entity("p1", Vec2::new(8.0, -4.0), EntityPoseSource::SnapshotWindow);
        assert_eq!(buffer.entities.len(), 1);
        assert_eq!(buffer.entities[0].x, 8.0);
        assert_eq!(buffer.entities[0].source, EntityPoseSource::SnapshotWindow);
    }

    #[test]
    fn local_command_pose_publishes_the_same_sprite_motion_phase() {
        let mut buffer = PresentationPoseBuffer::default();
        buffer.set_enabled(true);
        buffer.begin_frame(10.0, true);
        buffer.set_local_self_motion(Some(EntityPresentationMotion {
            frame_index: 3,
            phase_count: 6,
            mode: "walk".to_owned(),
            direction: "Left".to_owned(),
        }));
        buffer.record_entity("self", Vec2::new(16.0, 0.0), EntityPoseSource::LocalCommand);

        let value: serde_json::Value =
            serde_json::from_str(&buffer.publish()).expect("valid local pose json");
        assert_eq!(value["entities"][0]["motion"]["frameIndex"], 3);
        assert_eq!(value["entities"][0]["motion"]["phaseCount"], 6);
        assert_eq!(value["entities"][0]["motion"]["mode"], "walk");
        assert_eq!(value["entities"][0]["motion"]["direction"], "Left");

        buffer.begin_frame(20.0, true);
        buffer.record_entity("self", Vec2::ZERO, EntityPoseSource::Static);
        let value: serde_json::Value =
            serde_json::from_str(&buffer.publish()).expect("valid settled pose json");
        assert!(value["entities"][0].get("motion").is_none());
    }

    #[test]
    fn matching_self_window_promotes_source_without_moving_pixels() {
        let mut buffer = PresentationPoseBuffer::default();
        buffer.set_enabled(true);
        buffer.begin_frame(10.0, true);
        buffer.set_camera(Vec2::new(-40.0, 0.0), CameraPoseSource::SelfWindow);
        let camera_before = buffer.camera_screen_offset();
        let entity_before = buffer.self_entity_offset();
        let motion = EntityPresentationMotion {
            frame_index: 0,
            phase_count: 6,
            mode: "walk".to_owned(),
            direction: "Left".to_owned(),
        };

        assert!(!buffer.promote_matching_self_window_to_local_command(
            Vec2::new(39.0, 0.0),
            Some(motion.clone()),
        ));
        assert!(buffer
            .promote_matching_self_window_to_local_command(Vec2::new(40.0, 0.0), Some(motion),));
        assert_eq!(buffer.camera_source(), CameraPoseSource::LocalCommand);
        assert_eq!(buffer.camera_screen_offset(), camera_before);
        assert_eq!(buffer.self_entity_offset(), entity_before);

        buffer.record_entity("self", entity_before, EntityPoseSource::LocalCommand);
        assert_eq!(
            buffer.entities[0]
                .motion
                .as_ref()
                .map(|value| value.frame_index),
            Some(0)
        );
    }

    #[test]
    fn committed_center_reconciles_settled_camera_before_publish() {
        let mut buffer = PresentationPoseBuffer::default();
        buffer.set_enabled(true);
        buffer.begin_frame(10.0, true);
        // This is the transient state selected while map and entity provenance
        // still name different centers during a two-cell run handoff.
        buffer.set_camera(Vec2::ZERO, CameraPoseSource::LocalCommand);

        let motion = EntityPresentationMotion {
            frame_index: 0,
            phase_count: 6,
            mode: "run".to_owned(),
            direction: "Right".to_owned(),
        };
        assert!(
            buffer.reconcile_local_command_for_applied_center(Vec2::new(-80.0, 0.0), Some(motion),)
        );
        assert_eq!(buffer.camera_screen_offset(), Vec2::new(80.0, 0.0));
        assert_eq!(buffer.self_entity_offset(), Vec2::new(-80.0, 0.0));

        buffer.record_entity(
            "self",
            buffer.self_entity_offset(),
            EntityPoseSource::LocalCommand,
        );
        let value: serde_json::Value =
            serde_json::from_str(&buffer.publish()).expect("valid reconciled pose json");
        assert_eq!(value["camera"]["x"], 80.0);
        assert_eq!(value["camera"]["source"], "localCommand");
        assert_eq!(value["entities"][0]["x"], -80.0);
        assert_eq!(value["entities"][0]["motion"]["mode"], "run");
    }

    #[test]
    fn entity_buffer_is_bounded_and_disabled_frames_stay_empty() {
        let mut buffer = PresentationPoseBuffer::default();
        buffer.set_enabled(true);
        buffer.begin_frame(10.0, true);
        for index in 0..(MAX_ENTITY_POSES + 3) {
            buffer.record_entity(
                &format!("p{index}"),
                Vec2::new(index as f32, 0.0),
                EntityPoseSource::Static,
            );
        }
        assert_eq!(buffer.entities.len(), MAX_ENTITY_POSES);
        assert_eq!(buffer.frame_overflow_count, 3);
        assert_eq!(buffer.total_overflow_count, 3);

        buffer.begin_frame(20.0, false);
        buffer.record_entity("ignored", Vec2::ONE, EntityPoseSource::RemotePacket);
        buffer.set_camera(Vec2::ONE, CameraPoseSource::SelfWindow);
        assert!(buffer.entities.is_empty());
        assert_eq!(buffer.camera, CameraPresentationPose::default());
    }

    #[test]
    fn bridge_disable_does_not_change_bevy_camera_or_self_pose() {
        let mut buffer = PresentationPoseBuffer::default();
        buffer.set_enabled(false);
        buffer.begin_frame(30.0, true);
        buffer.set_camera(Vec2::new(40.0, 16.0), CameraPoseSource::SelfWindow);
        buffer.record_entity("ignored", Vec2::ONE, EntityPoseSource::SnapshotWindow);

        assert_eq!(buffer.camera_screen_offset(), Vec2::new(40.0, 16.0));
        assert_eq!(buffer.self_entity_offset(), Vec2::new(-40.0, -16.0));
        assert!(buffer.entities.is_empty());
    }

    #[test]
    fn frame_ids_are_monotonic_and_dispatch_receives_committed_json() {
        clear_presentation_pose_sink();
        push_presentation_pose_json("ignored without a sink");

        let mut buffer = PresentationPoseBuffer::default();
        buffer.begin_frame(10.0, true);
        let first = buffer.publish();
        buffer.begin_frame(20.0, true);

        let mut received = None;
        let second = buffer.publish_with(|json| received = Some(json.to_owned()));
        let first: serde_json::Value = serde_json::from_str(&first).expect("first frame json");
        let second_value: serde_json::Value =
            serde_json::from_str(&second).expect("second frame json");

        assert_eq!(first["frameId"], 1);
        assert_eq!(second_value["frameId"], 2);
        assert_eq!(received.as_deref(), Some(second.as_str()));
        assert_eq!(get_presentation_pose_json(), second);
    }
}
