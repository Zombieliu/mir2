//! Renderer-neutral Windows producer for the native Crystal light buffer.
//!
//! This module intentionally lives under `map_parser` until the conflicted
//! gateway/main integration round is available. It consumes authoritative
//! world/map packet data plus explicit presentation-motion offsets and emits
//! the JSON contract accepted by `push_native_lighting_render_state`.


use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex, OnceLock};

use mir2_client_bevy::native_lighting_environment::NativeLightingEnvironment;
use serde_json::{json, Value};

use super::{native_map_light_cells, MapViewport, ParsedMap};
use crate::effects::{native_effect_light_snapshots, NativeEffectLightSnapshot};

pub const STAGE_WIDTH: f32 = 1024.0;
pub const STAGE_HEIGHT: f32 = 768.0;
pub const CELL_WIDTH: f32 = 48.0;
pub const CELL_HEIGHT: f32 = 32.0;
pub const ENTITY_ORIGIN_X: f32 = 480.0;
pub const ENTITY_ORIGIN_Y: f32 = 352.0;
pub const MAX_NATIVE_LIGHTS: usize = 200;
const LIGHT_TEXTURE_COUNT: usize = 10;


// Set once by native startup, before the WebSocket lighting producer starts.
// Rendering never reads a file or changes the server's time/map state.
static FORCE_DAYLIGHT: AtomicBool = AtomicBool::new(false);

pub(crate) fn configure_force_daylight(enabled: bool) {
    FORCE_DAYLIGHT.store(enabled, AtomicOrdering::Relaxed);
}

pub(crate) fn force_daylight_enabled() -> bool {
    FORCE_DAYLIGHT.load(AtomicOrdering::Relaxed)
}

pub(crate) fn presentation_light_setting(setting: Option<u8>, force_daylight: bool) -> Option<u8> {
    if force_daylight {
        Some(2)
    } else {
        setting
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NativeLightAssets {
    ranges: [bool; LIGHT_TEXTURE_COUNT],
}

impl NativeLightAssets {
    pub fn from_asset_root(asset_root: &Path) -> Self {
        let lighting_root = asset_root.join("original-effects").join("Lighting");
        Self {
            ranges: std::array::from_fn(|range| {
                lighting_root.join(format!("{range}.png")).is_file()
            }),
        }
    }

    pub fn complete(&self) -> bool {
        self.ranges.iter().all(|present| *present)
    }

    fn contains(&self, range: usize) -> bool {
        self.ranges.get(range).copied().unwrap_or(false)
    }

    #[cfg(test)]
    pub(crate) fn complete_fixture() -> Self {
        Self {
            ranges: [true; LIGHT_TEXTURE_COUNT],
        }
    }
}

/// Display-Hz presentation offsets supplied by the host. The camera offset is
/// applied to map and entity anchors; an object-specific offset is then applied
/// to that entity only. This keeps lighting on the same sub-cell motion path as
/// map/entity sprites instead of deriving motion from wall-clock time twice.
pub use mir2_client_bevy::native_lighting_sources::NativeLightingMotion;
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NativeLightingBridge {
    force_daylight: bool,
    generation: Option<u64>,
    current_map_file_name: Option<String>,
    time_of_day_light_setting: Option<i32>,
    map_light_setting: Option<i32>,
    map_dark_light: i32,
}


// Both the immutable world context and its parsed map are shared. Cloning a
// Bichon map here copied 700 * 700 * 24 bytes on every effect-light frame.
#[derive(Debug, Clone)]
struct EffectLightingContext {
    generation: Option<u64>,
    bridge: NativeLightingBridge,
    payload: Value,
    map: Option<Arc<ParsedMap>>,
    map_frame_offsets: HashMap<(i32, i32), (i32, i32)>,
    motion: NativeLightingMotion,
    assets: NativeLightAssets,
}

static EFFECT_LIGHTING_CONTEXT: OnceLock<Mutex<Option<Arc<EffectLightingContext>>>> =
    OnceLock::new();

fn effect_lighting_context() -> &'static Mutex<Option<Arc<EffectLightingContext>>> {
    EFFECT_LIGHTING_CONTEXT.get_or_init(|| Mutex::new(None))
}

/// Returns the effective Crystal lighting state for a capture only when the
/// latest lighting bridge context belongs to the requested map. This avoids
/// labeling a screenshot with a stale override from the previous map.
pub(crate) fn capture_light_state_for_map(map_file_name: &str) -> Option<String> {
    let current = effect_lighting_context()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    capture_light_state_slug(
        current.as_ref().map(|context| &context.bridge),
        map_file_name,
    )
}

fn capture_light_state_slug(
    bridge: Option<&NativeLightingBridge>,
    map_file_name: &str,
) -> Option<String> {
    let bridge = bridge?;
    let current_map = bridge.current_map_file_name.as_deref()?;
    if !same_map_file_name(current_map, map_file_name) {
        return None;
    }
    if bridge.force_daylight {
        return Some("setting=2;mapDarkLight=0;forceDaylight=true".to_owned());
    }
    let setting = bridge
        .map_light_setting
        .or(bridge.time_of_day_light_setting)?;
    Some(format!(
        "setting={setting};mapDarkLight={}",
        bridge.map_dark_light
    ))
}

fn remember_effect_lighting_context(
    bridge: &NativeLightingBridge,
    payload: &Value,
    map: Option<&Arc<ParsedMap>>,
    map_frame_offsets: &HashMap<(i32, i32), (i32, i32)>,
    motion: &NativeLightingMotion,
    assets: &NativeLightAssets,
) {
    let context = EffectLightingContext {
        generation: bridge.generation,
        bridge: bridge.clone(),
        payload: payload.clone(),
        map: map.cloned(),
        map_frame_offsets: map_frame_offsets.clone(),
        motion: motion.clone(),
        assets: assets.clone(),
    };
    let mut current = effect_lighting_context()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *current = Some(Arc::new(context));
}

/// Rebuild and enqueue the current lighting state from the latest gateway
/// context plus the per-frame effect snapshot. The generation check prevents a
/// reconnect from combining the old lighting context with a new effect list.
pub(crate) fn publish_effect_lighting_frame(
    generation: u64,
    effect_lights: Vec<NativeEffectLightSnapshot>,
) {
    let Some(state) = render_effect_lighting_frame(generation, &effect_lights) else {
        return;
    };
    let Ok(json) = serde_json::to_string(&state) else {
        return;
    };
    let _ = mir2_bevy_runtime::native_ingest::push_native_lighting_render_state(json);
}

fn render_effect_lighting_frame(
    generation: u64,
    effect_lights: &[NativeEffectLightSnapshot],
) -> Option<Value> {
    let context = effect_lighting_context()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let Some(context) = context else {
        return None;
    };
    if context.generation != Some(generation) {
        return None;
    }
    Some(context.bridge.build_render_state_with_effects(
        &context.payload,
        context.map.as_deref(),
        &context.map_frame_offsets,
        &context.motion,
        &context.assets,
        &effect_lights,
    ))
}

impl NativeLightingBridge {
    pub(crate) fn set_force_daylight(&mut self, enabled: bool) {
        self.force_daylight = enabled;
    }

    pub fn set_generation(&mut self, generation: u64) {
        if self.generation != Some(generation) {
            self.reset_session();
            self.generation = Some(generation);
        }
    }

    // Keep generation, source selection and force-daylight rules unchanged.
    // Only the environment metadata reducer is shared with Android.
    fn with_shared_environment(&mut self, update: impl FnOnce(&mut NativeLightingEnvironment)) {
        let mut environment = NativeLightingEnvironment {
            current_map_file_name: self.current_map_file_name.take(),
            time_of_day_light_setting: self.time_of_day_light_setting,
            map_light_setting: self.map_light_setting,
            map_dark_light: self.map_dark_light,
        };
        update(&mut environment);
        self.current_map_file_name = environment.current_map_file_name;
        self.time_of_day_light_setting = environment.time_of_day_light_setting;
        self.map_light_setting = environment.map_light_setting;
        self.map_dark_light = environment.map_dark_light;
    }

    pub fn reset_session(&mut self) {
        self.with_shared_environment(|environment| environment.reset_session());
    }

    /// A map transition invalidates all map-specific light state immediately.
    /// The time-of-day setting belongs to the connection and is intentionally
    /// retained until the next authoritative snapshot or session reset.
    pub fn reset_scene(&mut self) {
        self.with_shared_environment(|environment| environment.reset_scene());
    }

    pub fn observe_world_snapshot(&mut self, payload: &Value) {
        self.with_shared_environment(|environment| environment.observe_world_snapshot(payload));
    }

    /// Observe only packet-authoritative light lifecycle data. Unknown packets
    /// are ignored; logout and reconnect generation changes fail closed.
    pub fn observe_packet(&mut self, packet: &str, payload: &Value) {
        self.with_shared_environment(|environment| environment.observe_packet(packet, payload));
    }

    pub fn build_render_state(
        &self,
        payload: &Value,
        map: Option<&Arc<ParsedMap>>,
        map_frame_offsets: &HashMap<(i32, i32), (i32, i32)>,
        motion: &NativeLightingMotion,
        assets: &NativeLightAssets,
    ) -> Value {
        let effect_lights = if self.generation.is_some() {
            native_effect_light_snapshots()
        } else {
            Vec::new()
        };
        let state = self.build_render_state_with_effects(
            payload,
            map.map(Arc::as_ref),
            map_frame_offsets,
            motion,
            assets,
            &effect_lights,
        );
        remember_effect_lighting_context(self, payload, map, map_frame_offsets, motion, assets);
        state
    }

    fn build_render_state_with_effects(
        &self,
        payload: &Value,
        map: Option<&ParsedMap>,
        map_frame_offsets: &HashMap<(i32, i32), (i32, i32)>,
        motion: &NativeLightingMotion,
        assets: &NativeLightAssets,
        effect_lights: &[NativeEffectLightSnapshot],
    ) -> Value {
        let environment = NativeLightingEnvironment {
            current_map_file_name: self.current_map_file_name.clone(),
            time_of_day_light_setting: self.time_of_day_light_setting,
            map_light_setting: self.map_light_setting,
            map_dark_light: self.map_dark_light,
        };
        let viewport = MapViewport::from_gateway_payload(payload);
        let map_lights = map.into_iter().flat_map(|map| native_map_light_cells(map, map_frame_offsets));
        mir2_client_bevy::native_lighting_sources::build_native_lighting_render_state(
            &environment, self.generation, self.force_daylight, payload,
            (viewport.center_x, viewport.center_y), map_lights, motion,
            &mir2_client_bevy::native_lighting_sources::NativeLightAssets::from_presence(assets.ranges),
            effect_lights,
        )
    }
}


fn same_map_file_name(left: &str, right: &str) -> bool {
    mir2_client_bevy::native_lighting_environment::same_map_file_name(left, right)
}
fn object_id_string(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => Some(number.to_string()),
        Value::String(value) => Some(value.clone()),
        _ => None,
    }
}





#[cfg(test)]
mod tests {
    use super::*;
    use crate::map_parser::MapCell;

    fn cell(light: u8, animated: bool) -> MapCell {
        MapCell {
            back_index: 0,
            back_image: 0,
            middle_index: -1,
            middle_image: 0,
            front_index: 0,
            front_image: 1,
            front_animation_frame: u8::from(animated),
            front_animation_tick: 0,
            middle_animation_frame: 0,
            middle_animation_tick: 0,
            tile_animation_image: 0,
            tile_animation_offset: 0,
            tile_animation_frames: 0,
            light,
        }
    }

    fn world() -> Value {
        json!({
            "mapFileName": "0",
            "lightSetting": 4,
            "playerObjectId": 1000,
            "sceneView": {"center": {"x": 10, "y": 20}, "width": 19, "height": 15},
            "entities": [
                {"objectId": 1000, "kind": "selfPlayer", "x": 10, "y": 20},
                {"objectId": 2000, "kind": "monster", "x": 11, "y": 19, "light": 1, "dead": false}
            ]
        })
    }

    #[test]
    fn daylight_preference_survives_map_logout_and_generation_without_overwriting_source_lights() {
        let mut bridge = NativeLightingBridge::default();
        bridge.set_force_daylight(true);
        let mut payload = world();
        for generation in [1, 1, 2] {
            bridge.reset_scene();
            if generation == 2 {
                bridge.observe_packet("LogOutSuccess", &Value::Null);
            }
            bridge.set_generation(generation);
            payload["mapFileName"] = json!(if generation == 1 { "0" } else { "D401" });
            bridge.observe_world_snapshot(&payload);
            bridge.observe_packet("TimeOfDay", &json!({"lights": 4}));
            bridge.observe_packet(
                "MapInformation",
                &json!({
                    "fileName": payload["mapFileName"], "lights": 4, "mapDarkLight": 2
                }),
            );
            let render = bridge.build_render_state_with_effects(
                &payload,
                None,
                &HashMap::new(),
                &NativeLightingMotion::default(),
                &NativeLightAssets::default(),
                &[],
            );
            assert_eq!(render["enabled"], true);
            assert_eq!(render["timeOfDayLightSetting"], 2);
            assert_eq!(render["mapLightSetting"], 2);
            assert_eq!(render["mapDarkLight"], 0);
            assert_eq!(render["entityLights"], json!([]));
            assert_eq!(bridge.time_of_day_light_setting, Some(4));
            assert_eq!(bridge.map_light_setting, Some(4));
            assert_eq!(bridge.map_dark_light, 2);
            assert_eq!(payload["lightSetting"], 4);
            assert_eq!(
                capture_light_state_slug(Some(&bridge), payload["mapFileName"].as_str().unwrap()),
                Some("setting=2;mapDarkLight=0;forceDaylight=true".to_owned())
            );
        }
        bridge.set_force_daylight(false);
        let restored = bridge.build_render_state_with_effects(
            &payload,
            None,
            &HashMap::new(),
            &NativeLightingMotion::default(),
            &NativeLightAssets::complete_fixture(),
            &[],
        );
        assert_eq!(restored["timeOfDayLightSetting"], 4);
        assert_eq!(restored["mapLightSetting"], 4);
        assert_eq!(restored["mapDarkLight"], 2);
        assert_eq!(presentation_light_setting(Some(4), true), Some(2));
        assert_eq!(presentation_light_setting(Some(4), false), Some(4));
        assert_eq!(presentation_light_setting(None, false), None);
    }

    #[test]
    fn capture_light_state_requires_matching_map_and_preserves_dark_override() {
        let bridge = NativeLightingBridge {
            current_map_file_name: Some("maps/0.map".to_owned()),
            time_of_day_light_setting: Some(4),
            map_light_setting: Some(3),
            map_dark_light: 2,
            ..Default::default()
        };
        assert_eq!(
            capture_light_state_slug(Some(&bridge), "0"),
            Some("setting=3;mapDarkLight=2".to_owned())
        );
        assert_eq!(capture_light_state_slug(Some(&bridge), "1"), None);
        assert_eq!(capture_light_state_slug(None, "0"), None);
    }

    #[test]
    fn packet_and_generation_lifecycle_clear_stale_map_light() {
        let mut bridge = NativeLightingBridge::default();
        bridge.set_generation(1);
        bridge.observe_world_snapshot(&world());
        bridge.observe_packet(
            "MapInformation",
            &json!({"lights": 3, "mapDarkLight": 2, "fileName": "0"}),
        );
        assert_eq!(bridge.map_light_setting, Some(3));
        assert_eq!(bridge.map_dark_light, 2);
        bridge.set_generation(2);
        assert_eq!(bridge.map_light_setting, None);
        assert_eq!(bridge.time_of_day_light_setting, None);
        bridge.observe_packet("LogOutSuccess", &Value::Null);
        assert_eq!(
            bridge,
            NativeLightingBridge {
                generation: Some(2),
                ..Default::default()
            }
        );
    }

    #[test]
    fn partial_snapshot_preserves_time_and_same_map_packet_preserves_map_override() {
        let mut bridge = NativeLightingBridge::default();
        bridge.observe_world_snapshot(&world());
        bridge.observe_packet(
            "MapInformation",
            &json!({"lights": 3, "mapDarkLight": 2, "fileName": "maps/0.map"}),
        );

        bridge.observe_world_snapshot(&json!({"mapFileName": "0"}));
        bridge.observe_packet("MapInformation", &json!({"fileName": "0"}));
        assert_eq!(bridge.time_of_day_light_setting, Some(4));
        assert_eq!(bridge.map_light_setting, Some(3));
        assert_eq!(bridge.map_dark_light, 2);

        bridge.observe_packet("MapChanged", &json!({"fileName": "1"}));
        assert_eq!(bridge.map_light_setting, None);
        assert_eq!(bridge.map_dark_light, 0);
        assert_eq!(bridge.current_map_file_name.as_deref(), Some("1"));
    }

    #[test]
    fn builder_uses_crystal_stage_anchors_motion_and_animated_map_offset() {
        let mut bridge = NativeLightingBridge::default();
        bridge.observe_world_snapshot(&world());
        bridge.observe_packet(
            "MapInformation",
            &json!({"lights": 4, "mapDarkLight": 1, "fileName": "0"}),
        );
        let map = Arc::new(ParsedMap {
            width: 1,
            height: 1,
            cells: vec![cell(1, true)],
        });
        let motion = NativeLightingMotion {
            camera_offset_x: 8.0,
            camera_offset_y: -4.0,
            entity_offsets: HashMap::from([("2000".to_owned(), (3.0, 5.0))]),
        };
        let state = bridge.build_render_state(
            &world(),
            Some(&map),
            &HashMap::from([((0, 0), (-50, -100))]),
            &motion,
            &NativeLightAssets::complete_fixture(),
        );
        assert_eq!(state["enabled"], true);
        assert_eq!(state["entityLights"][0]["drawX"], 488.0);
        assert_eq!(state["entityLights"][0]["drawY"], 348.0);
        assert_eq!(state["entityLights"][1]["drawX"], 539.0);
        assert_eq!(state["entityLights"][1]["drawY"], 321.0);
        assert_eq!(state["mapLights"][0]["drawX"], 8.0);
        assert_eq!(state["mapLights"][0]["drawY"], -292.0);
        assert_eq!(state["mapLights"][0]["offsetX"], -50);
        assert_eq!(state["mapLights"][0]["offsetY"], -100);
    }

    #[test]
    fn effect_context_arc_reuses_map_and_releases_old_map_on_replacement() {
        assert_eq!(std::mem::size_of::<super::super::MapCell>(), 24);
        let map = Arc::new(ParsedMap {
            width: 1,
            height: 1,
            cells: vec![cell(1, true)],
        });
        let old_map = Arc::downgrade(&map);
        let mut bridge = NativeLightingBridge::default();
        bridge.set_generation(81);
        bridge.observe_world_snapshot(&world());
        let make_context = |bridge: NativeLightingBridge, map: Option<Arc<ParsedMap>>, payload| {
            Arc::new(EffectLightingContext {
                generation: bridge.generation,
                bridge,
                payload,
                map,
                map_frame_offsets: HashMap::from([((0, 0), (-50, -100))]),
                motion: NativeLightingMotion::default(),
                assets: NativeLightAssets::complete_fixture(),
            })
        };
        let render = |context: &EffectLightingContext| {
            context.bridge.build_render_state_with_effects(
                &context.payload,
                context.map.as_deref(),
                &context.map_frame_offsets,
                &context.motion,
                &context.assets,
                &[],
            )
        };
        let mut retained = make_context(bridge.clone(), Some(Arc::clone(&map)), world());
        let expected = render(&retained);
        assert_eq!(expected["mapLights"][0]["offsetX"], -50);
        assert_eq!(expected["mapLights"][0]["offsetY"], -100);
        for _ in 0..120 {
            let frame = Arc::clone(&retained);
            assert!(Arc::ptr_eq(&retained, &frame));
            assert!(Arc::ptr_eq(frame.map.as_ref().unwrap(), &map));
            assert_eq!(render(&frame), expected);
        }
        assert_eq!(Arc::strong_count(&map), 2);
        drop(map);

        // A new map can have identical dimensions. Its own cells, rather than
        // dimensions or a stale context, determine the next light frame.
        let replacement = Arc::new(ParsedMap {
            width: 1,
            height: 1,
            cells: vec![cell(0, true)],
        });
        let replacement_weak = Arc::downgrade(&replacement);
        bridge.observe_packet("MapChanged", &json!({"fileName": "1"}));
        let mut next_world = world();
        next_world["mapFileName"] = json!("1");
        bridge.observe_world_snapshot(&next_world);
        retained = make_context(bridge.clone(), Some(replacement), next_world);
        assert!(old_map.upgrade().is_none());
        assert!(render(&retained)["mapLights"]
            .as_array()
            .unwrap()
            .is_empty());

        bridge.reset_scene();
        retained = make_context(bridge, None, Value::Null);
        assert!(replacement_weak.upgrade().is_none());
        assert_eq!(render(&retained)["enabled"], false);
    }

    #[test]
    fn missing_assets_and_nonfinite_motion_fail_closed() {
        let mut bridge = NativeLightingBridge::default();
        bridge.observe_world_snapshot(&world());
        let missing = NativeLightAssets {
            ranges: [false; LIGHT_TEXTURE_COUNT],
        };
        assert_eq!(
            bridge.build_render_state(
                &world(),
                None,
                &HashMap::new(),
                &NativeLightingMotion::default(),
                &missing,
            )["enabled"],
            false
        );

        let mut missing_map = world();
        missing_map.as_object_mut().unwrap().remove("mapFileName");
        assert_eq!(
            bridge.build_render_state(
                &missing_map,
                None,
                &HashMap::new(),
                &NativeLightingMotion::default(),
                &NativeLightAssets::complete_fixture(),
            )["enabled"],
            false
        );
        let motion = NativeLightingMotion {
            camera_offset_x: f32::NAN,
            ..Default::default()
        };
        assert_eq!(
            bridge.build_render_state(
                &world(),
                None,
                &HashMap::new(),
                &motion,
                &NativeLightAssets::complete_fixture(),
            )["enabled"],
            false
        );
    }

    #[test]
    fn object_lights_keep_priority_at_the_two_hundred_cap() {
        let mut payload = world();
        payload["entities"] = Value::Array(
            (0..250)
                .map(|index| {
                    json!({
                        "objectId": index + 1,
                        "kind": "monster",
                        "x": 10,
                        "y": 20,
                        "light": 1,
                    })
                })
                .collect(),
        );
        let mut bridge = NativeLightingBridge::default();
        bridge.observe_world_snapshot(&payload);
        let state = bridge.build_render_state(
            &payload,
            None,
            &HashMap::new(),
            &NativeLightingMotion::default(),
            &NativeLightAssets::complete_fixture(),
        );
        assert_eq!(
            state["entityLights"].as_array().unwrap().len(),
            MAX_NATIVE_LIGHTS
        );
        assert!(state["mapLights"].as_array().unwrap().is_empty());
    }

    #[test]
    fn entity_candidates_are_bounded_and_order_independent() {
        let mut payload = world();
        let mut entities: Vec<Value> = (0..500)
            .map(|index| {
                json!({
                    "objectId": index + 1,
                    "kind": "monster",
                    "x": 10,
                    "y": 20,
                    "light": 1,
                })
            })
            .collect();
        entities.push(json!({
            "objectId": 800_000,
            "kind": "npc",
            "x": 10,
            "y": 20,
        }));
        entities.push(json!({
            "objectId": 900_000,
            "kind": "selfPlayer",
            "x": 10,
            "y": 20,
        }));
        payload["playerObjectId"] = json!(900_000);

        let mut first = payload.clone();
        first["entities"] = Value::Array(entities.clone());
        let mut reversed = payload;
        entities.reverse();
        reversed["entities"] = Value::Array(entities);

        let mut bridge = NativeLightingBridge::default();
        bridge.observe_world_snapshot(&first);
        let state_first = bridge.build_render_state(
            &first,
            None,
            &HashMap::new(),
            &NativeLightingMotion::default(),
            &NativeLightAssets::complete_fixture(),
        );
        let state_reversed = bridge.build_render_state(
            &reversed,
            None,
            &HashMap::new(),
            &NativeLightingMotion::default(),
            &NativeLightAssets::complete_fixture(),
        );
        let keys = |state: &Value| {
            state["entityLights"]
                .as_array()
                .unwrap()
                .iter()
                .map(|light| light["key"].as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        };
        let first_keys = keys(&state_first);
        assert_eq!(first_keys.len(), MAX_NATIVE_LIGHTS);
        assert!(first_keys.iter().any(|key| key == "800000"));
        assert!(first_keys.iter().any(|key| key == "900000"));
        assert_eq!(first_keys, keys(&state_reversed));
    }

    #[test]
    fn dead_object_flood_cannot_suppress_the_self_light() {
        let mut payload = world();
        let mut entities: Vec<Value> = (0..250)
            .map(|index| {
                json!({
                    "objectId": index + 1,
                    "kind": "monster",
                    "x": 10,
                    "y": 20,
                    "light": 1,
                    "dead": true,
                })
            })
            .collect();
        entities.push(json!({
            "objectId": 1000,
            "kind": "selfPlayer",
            "x": 10,
            "y": 20,
            "dead": false,
        }));
        payload["entities"] = Value::Array(entities);
        let mut bridge = NativeLightingBridge::default();
        bridge.observe_world_snapshot(&payload);
        let state = bridge.build_render_state(
            &payload,
            None,
            &HashMap::new(),
            &NativeLightingMotion::default(),
            &NativeLightAssets::complete_fixture(),
        );
        assert_eq!(state["entityLights"].as_array().unwrap().len(), 1);
        assert_eq!(state["entityLights"][0]["key"], "1000");
        assert_eq!(state["entityLights"][0]["light"], 3);
    }

    #[test]
    fn effect_lights_merge_after_entities_and_follow_fractional_tile() {
        let mut bridge = NativeLightingBridge::default();
        bridge.observe_world_snapshot(&world());
        let effects = vec![NativeEffectLightSnapshot {
            generation: 0,
            key: "fx-proj-1".to_owned(),
            tile_x: 12.5,
            tile_y: 20.0,
            light: 6,
        }];
        let state = bridge.build_render_state_with_effects(
            &world(),
            None,
            &HashMap::new(),
            &NativeLightingMotion::default(),
            &NativeLightAssets::complete_fixture(),
            &effects,
        );
        let lights = state["entityLights"].as_array().unwrap();
        assert_eq!(lights.len(), 3);
        assert_eq!(lights[2]["key"], "effect:fx-proj-1");
        assert_eq!(lights[2]["drawX"], 600.0);
        assert_eq!(lights[2]["drawY"], 352.0);
    }

    #[test]
    fn effect_lighting_frame_is_generation_bound_and_rebuilt_per_frame() {
        let mut bridge = NativeLightingBridge::default();
        bridge.set_generation(44);
        bridge.observe_world_snapshot(&world());
        let _ = bridge.build_render_state(
            &world(),
            None,
            &HashMap::new(),
            &NativeLightingMotion::default(),
            &NativeLightAssets::complete_fixture(),
        );

        let frame = |tile_x: f32| NativeEffectLightSnapshot {
            generation: 44,
            key: "fx-proj-frame".to_owned(),
            tile_x,
            tile_y: 20.0,
            light: 6,
        };
        let first = render_effect_lighting_frame(44, &[frame(10.5)]).expect("first frame");
        let second = render_effect_lighting_frame(44, &[frame(11.5)]).expect("second frame");
        let first_light = first["entityLights"]
            .as_array()
            .unwrap()
            .iter()
            .find(|light| light["key"] == "effect:fx-proj-frame")
            .unwrap();
        let second_light = second["entityLights"]
            .as_array()
            .unwrap()
            .iter()
            .find(|light| light["key"] == "effect:fx-proj-frame")
            .unwrap();
        assert_eq!(first_light["drawX"], 504.0);
        assert_eq!(second_light["drawX"], 552.0);
        assert!(render_effect_lighting_frame(43, &[frame(12.0)]).is_none());
    }

    #[test]
    fn effect_lights_respect_assets_cap_and_entity_priority() {
        let mut payload = world();
        payload["entities"] = Value::Array(
            (0..199)
                .map(|index| {
                    json!({
                        "objectId": index + 1,
                        "kind": "monster",
                        "x": 10,
                        "y": 20,
                        "light": 1,
                    })
                })
                .collect(),
        );
        let mut bridge = NativeLightingBridge::default();
        bridge.observe_world_snapshot(&payload);
        let effects = vec![
            NativeEffectLightSnapshot {
                generation: 0,
                key: "fx-a".to_owned(),
                tile_x: 10.0,
                tile_y: 20.0,
                light: 6,
            },
            NativeEffectLightSnapshot {
                generation: 0,
                key: "fx-invalid-range".to_owned(),
                tile_x: 10.0,
                tile_y: 20.0,
                light: 6,
            },
        ];
        let state = bridge.build_render_state_with_effects(
            &payload,
            None,
            &HashMap::new(),
            &NativeLightingMotion::default(),
            &NativeLightAssets::complete_fixture(),
            &effects,
        );
        let lights = state["entityLights"].as_array().unwrap();
        assert_eq!(lights.len(), MAX_NATIVE_LIGHTS);
        assert_eq!(lights[0]["key"], "1");
        assert_eq!(lights.last().unwrap()["key"], "effect:fx-a");
        assert!(lights
            .iter()
            .all(|value| value["key"] != "effect:fx-invalid-range"));

        let missing = NativeLightAssets {
            ranges: [
                true, false, false, false, false, false, false, false, false, false,
            ],
        };
        let state = bridge.build_render_state_with_effects(
            &world(),
            None,
            &HashMap::new(),
            &NativeLightingMotion::default(),
            &missing,
            &[NativeEffectLightSnapshot {
                generation: 0,
                key: "fx-no-range".to_owned(),
                tile_x: 10.0,
                tile_y: 20.0,
                light: 6,
            }],
        );
        assert!(state["entityLights"]
            .as_array()
            .unwrap()
            .iter()
            .all(|value| value["kind"] != "effect"));
    }

    #[test]
    fn zero_or_negative_effect_light_is_ignored() {
        let mut bridge = NativeLightingBridge::default();
        bridge.observe_world_snapshot(&world());
        let state = bridge.build_render_state_with_effects(
            &world(),
            None,
            &HashMap::new(),
            &NativeLightingMotion::default(),
            &NativeLightAssets::complete_fixture(),
            &[
                NativeEffectLightSnapshot {
                    generation: 0,
                    key: "fx-zero".to_owned(),
                    tile_x: 10.0,
                    tile_y: 20.0,
                    light: 0,
                },
                NativeEffectLightSnapshot {
                    generation: 0,
                    key: "fx-negative".to_owned(),
                    tile_x: 10.0,
                    tile_y: 20.0,
                    light: -1,
                },
            ],
        );
        assert!(state["entityLights"]
            .as_array()
            .unwrap()
            .iter()
            .all(|value| value["kind"] != "effect"));
    }
}
