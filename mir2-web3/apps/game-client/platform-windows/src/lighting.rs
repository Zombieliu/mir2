//! Renderer-neutral Windows producer for the native Crystal light buffer.
//!
//! This module intentionally lives under `map_parser` until the conflicted
//! gateway/main integration round is available. It consumes authoritative
//! world/map packet data plus explicit presentation-motion offsets and emits
//! the JSON contract accepted by `push_native_lighting_render_state`.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

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
const MAP_LIGHT_RANGE_X: i32 = 40;
const MAP_LIGHT_RANGE_Y: i32 = 41;

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
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NativeLightingMotion {
    pub camera_offset_x: f32,
    pub camera_offset_y: f32,
    pub entity_offsets: HashMap<String, (f32, f32)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NativeLightingBridge {
    generation: Option<u64>,
    current_map_file_name: Option<String>,
    time_of_day_light_setting: Option<i32>,
    map_light_setting: Option<i32>,
    map_dark_light: i32,
}

#[derive(Debug, Clone)]
struct EntityLightCandidate {
    priority: u8,
    key: String,
    draw_x: f32,
    draw_y: f32,
    kind: String,
    light: i32,
    dead: bool,
    is_self: bool,
}

fn entity_candidate_cmp(left: &EntityLightCandidate, right: &EntityLightCandidate) -> Ordering {
    left.priority
        .cmp(&right.priority)
        .then_with(|| left.key.cmp(&right.key))
        .then_with(|| left.draw_x.total_cmp(&right.draw_x))
        .then_with(|| left.draw_y.total_cmp(&right.draw_y))
        .then_with(|| left.kind.cmp(&right.kind))
        .then_with(|| left.light.cmp(&right.light))
        .then_with(|| left.dead.cmp(&right.dead))
        .then_with(|| left.is_self.cmp(&right.is_self))
}

fn retain_best_entity_candidate(
    candidates: &mut Vec<EntityLightCandidate>,
    candidate: EntityLightCandidate,
) {
    if candidates.len() < MAX_NATIVE_LIGHTS {
        candidates.push(candidate);
        return;
    }
    let Some((worst_index, worst)) = candidates
        .iter()
        .enumerate()
        .max_by(|left, right| entity_candidate_cmp(left.1, right.1))
    else {
        return;
    };
    if entity_candidate_cmp(&candidate, worst) == Ordering::Less {
        candidates[worst_index] = candidate;
    }
}

#[derive(Debug, Clone)]
struct EffectLightingContext {
    generation: Option<u64>,
    bridge: NativeLightingBridge,
    payload: Value,
    map: Option<ParsedMap>,
    map_frame_offsets: HashMap<(i32, i32), (i32, i32)>,
    motion: NativeLightingMotion,
    assets: NativeLightAssets,
}

static EFFECT_LIGHTING_CONTEXT: OnceLock<Mutex<Option<EffectLightingContext>>> = OnceLock::new();

fn effect_lighting_context() -> &'static Mutex<Option<EffectLightingContext>> {
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
    map: Option<&ParsedMap>,
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
    *current = Some(context);
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
        context.map.as_ref(),
        &context.map_frame_offsets,
        &context.motion,
        &context.assets,
        &effect_lights,
    ))
}

impl NativeLightingBridge {
    pub fn set_generation(&mut self, generation: u64) {
        if self.generation != Some(generation) {
            self.reset_session();
            self.generation = Some(generation);
        }
    }

    pub fn reset_session(&mut self) {
        self.current_map_file_name = None;
        self.time_of_day_light_setting = None;
        self.map_light_setting = None;
        self.map_dark_light = 0;
    }

    /// A map transition invalidates all map-specific light state immediately.
    /// The time-of-day setting belongs to the connection and is intentionally
    /// retained until the next authoritative snapshot or session reset.
    pub fn reset_scene(&mut self) {
        self.current_map_file_name = None;
        self.map_light_setting = None;
        self.map_dark_light = 0;
    }

    pub fn observe_world_snapshot(&mut self, payload: &Value) {
        let next_map = payload
            .get("mapFileName")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        if self.current_map_file_name.is_some()
            && next_map.is_some()
            && !same_map_file_name(
                self.current_map_file_name.as_deref().unwrap_or_default(),
                next_map.as_deref().unwrap_or_default(),
            )
        {
            self.map_light_setting = None;
            self.map_dark_light = 0;
        }
        if next_map.is_some() {
            self.current_map_file_name = next_map;
        }
        if let Some(setting) = light_setting(payload.get("lightSetting")) {
            self.time_of_day_light_setting = Some(setting);
        }
    }

    /// Observe only packet-authoritative light lifecycle data. Unknown packets
    /// are ignored; logout and reconnect generation changes fail closed.
    pub fn observe_packet(&mut self, packet: &str, payload: &Value) {
        let body = packet_body(payload);
        match packet {
            "MapInformation" | "MapChanged" | "NewMapInfo" => {
                let next_map_file_name = body
                    .get("fileName")
                    .or_else(|| body.get("mapFileName"))
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty());
                let map_changed = self
                    .current_map_file_name
                    .as_deref()
                    .zip(next_map_file_name)
                    .is_some_and(|(current, next)| !same_map_file_name(current, next));
                if let Some(setting) = map_light_setting(body.get("lights")) {
                    self.map_light_setting = Some(setting);
                } else if map_changed {
                    self.map_light_setting = None;
                }
                if let Some(darkness) = body
                    .get("mapDarkLight")
                    .and_then(Value::as_i64)
                    .filter(|value| (0..=4).contains(value))
                {
                    self.map_dark_light = darkness as i32;
                } else if map_changed {
                    self.map_dark_light = 0;
                }
                if let Some(map_file_name) = next_map_file_name {
                    self.current_map_file_name = Some(map_file_name.to_owned());
                }
            }
            "TimeOfDay" => {
                self.time_of_day_light_setting = light_setting(body.get("lights"));
            }
            "LogOutSuccess" => self.reset_session(),
            _ => {}
        }
    }

    pub fn build_render_state(
        &self,
        payload: &Value,
        map: Option<&ParsedMap>,
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
            map,
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
        let viewport = MapViewport::from_gateway_payload(payload);
        let payload_map = payload.get("mapFileName").and_then(Value::as_str);
        let map_matches = payload_map.is_some_and(|payload| {
            self.current_map_file_name
                .as_deref()
                .is_none_or(|current| same_map_file_name(current, payload))
        });
        let enabled = assets.complete()
            && self
                .map_light_setting
                .or(self.time_of_day_light_setting)
                .is_some();

        if !enabled || !map_matches || !motion_is_finite(motion) {
            return disabled_state();
        }

        // Entity lights are authoritative and always outrank transient effect
        // lights. Keep only the best 200 candidates while scanning, so a large
        // gateway snapshot cannot create an unbounded clone/sort buffer.
        let mut entity_candidates: Vec<EntityLightCandidate> =
            Vec::with_capacity(MAX_NATIVE_LIGHTS);
        let player_object_id = payload.get("playerObjectId").and_then(object_id_string);
        if let Some(entities) = payload.get("entities").and_then(Value::as_array) {
            for entity in entities {
                let Some(key) = entity.get("objectId").and_then(object_id_string) else {
                    continue;
                };
                if key.is_empty() || key.len() > 128 || key.chars().any(char::is_control) {
                    continue;
                }
                let kind = entity
                    .get("kind")
                    .and_then(Value::as_str)
                    .unwrap_or("monster");
                let is_self = kind.eq_ignore_ascii_case("selfPlayer")
                    || player_object_id.as_deref() == Some(key.as_str());
                let is_spell = kind.eq_ignore_ascii_case("spell");
                if entity.get("dead").and_then(Value::as_bool).unwrap_or(false)
                    && !is_self
                    && !is_spell
                {
                    continue;
                }
                let raw_light =
                    if kind.eq_ignore_ascii_case("npc") || kind.eq_ignore_ascii_case("merchant") {
                        10
                    } else {
                        entity
                            .get("light")
                            .and_then(Value::as_i64)
                            .and_then(|value| i32::try_from(value).ok())
                            .unwrap_or(if is_self { 3 } else { 0 })
                    };
                if raw_light <= 0 || !assets.contains(entity_light_range(raw_light)) {
                    continue;
                }
                let (Some(x), Some(y)) = (
                    entity.get("x").and_then(Value::as_i64),
                    entity.get("y").and_then(Value::as_i64),
                ) else {
                    continue;
                };
                let (entity_offset_x, entity_offset_y) =
                    motion.entity_offsets.get(&key).copied().unwrap_or_default();
                let draw_x = ENTITY_ORIGIN_X
                    + (x - i64::from(viewport.center_x)) as f32 * CELL_WIDTH
                    + motion.camera_offset_x
                    + entity_offset_x;
                let draw_y = ENTITY_ORIGIN_Y
                    + (y - i64::from(viewport.center_y)) as f32 * CELL_HEIGHT
                    + motion.camera_offset_y
                    + entity_offset_y;
                if !draw_x.is_finite() || !draw_y.is_finite() {
                    continue;
                }
                let priority = if is_self {
                    0
                } else if kind.eq_ignore_ascii_case("npc") || kind.eq_ignore_ascii_case("merchant")
                {
                    1
                } else {
                    2
                };
                retain_best_entity_candidate(
                    &mut entity_candidates,
                    EntityLightCandidate {
                        priority,
                        key,
                        draw_x,
                        draw_y,
                        kind: kind.to_owned(),
                        light: raw_light,
                        dead: entity.get("dead").and_then(Value::as_bool).unwrap_or(false),
                        is_self,
                    },
                );
            }
        }

        entity_candidates.sort_by(entity_candidate_cmp);
        let mut entity_lights = entity_candidates
            .into_iter()
            .map(|candidate| {
                json!({
                    "key": candidate.key,
                    "drawX": candidate.draw_x,
                    "drawY": candidate.draw_y,
                    "kind": candidate.kind,
                    "light": candidate.light,
                    "dead": candidate.dead,
                    "isSelf": candidate.is_self,
                })
            })
            .collect::<Vec<_>>();

        // Effect snapshots are published by NativeEffects after every event
        // and animation tick. They are tile anchored or fractional projectile
        // positions, so they share the exact viewport/camera transform as map
        // and entity lights. Their stable key and explicit sort make the
        // result deterministic even when snapshots arrive in another order.
        let mut effect_candidates = effect_lights
            .iter()
            .cloned()
            .into_iter()
            .filter(|effect| {
                effect.light > 0
                    && effect.key.len() <= 128
                    && !effect.key.chars().any(char::is_control)
                    && effect.tile_x.is_finite()
                    && effect.tile_y.is_finite()
                    && self
                        .generation
                        .map_or(true, |generation| effect.generation == generation)
                    && assets.contains(entity_light_range(effect.light))
            })
            .collect::<Vec<_>>();
        effect_candidates.sort_by(|left, right| left.key.cmp(&right.key));
        let effect_capacity = MAX_NATIVE_LIGHTS.saturating_sub(entity_lights.len());
        for effect in effect_candidates.into_iter().take(effect_capacity) {
            let dx = effect.tile_x - viewport.center_x as f32;
            let dy = effect.tile_y - viewport.center_y as f32;
            let draw_x = ENTITY_ORIGIN_X + dx * CELL_WIDTH + motion.camera_offset_x;
            let draw_y = ENTITY_ORIGIN_Y + dy * CELL_HEIGHT + motion.camera_offset_y;
            if !draw_x.is_finite() || !draw_y.is_finite() {
                continue;
            }
            entity_lights.push(json!({
                "key": format!("effect:{}", effect.key),
                "drawX": draw_x,
                "drawY": draw_y,
                "kind": "effect",
                "light": effect.light,
                "dead": false,
                "isSelf": false,
            }));
        }

        let remaining = MAX_NATIVE_LIGHTS.saturating_sub(entity_lights.len());
        let mut map_lights = Vec::new();
        if let Some(map) = map {
            for source in native_map_light_cells(map, map_frame_offsets) {
                if map_lights.len() == remaining {
                    break;
                }
                let dx = source.x - viewport.center_x;
                let dy = source.y - viewport.center_y;
                if dx.abs() > MAP_LIGHT_RANGE_X || dy.abs() > MAP_LIGHT_RANGE_Y {
                    continue;
                }
                let range = ((i32::from(source.light) % 10) * 3).min(9) as usize;
                if !assets.contains(range) {
                    continue;
                }
                map_lights.push(json!({
                    "key": source.key,
                    "drawX": ENTITY_ORIGIN_X + dx as f32 * CELL_WIDTH + motion.camera_offset_x,
                    "drawY": ENTITY_ORIGIN_Y + dy as f32 * CELL_HEIGHT + motion.camera_offset_y,
                    "light": source.light,
                    "offsetX": source.offset_x,
                    "offsetY": source.offset_y,
                }));
            }
        }

        json!({
            "enabled": true,
            "stageWidth": STAGE_WIDTH,
            "stageHeight": STAGE_HEIGHT,
            "timeOfDayLightSetting": self.time_of_day_light_setting,
            "mapLightSetting": self.map_light_setting,
            "mapDarkLight": self.map_dark_light,
            "mapLights": map_lights,
            "entityLights": entity_lights,
        })
    }
}

fn disabled_state() -> Value {
    json!({
        "enabled": false,
        "stageWidth": STAGE_WIDTH,
        "stageHeight": STAGE_HEIGHT,
        "mapLights": [],
        "entityLights": [],
    })
}

fn light_setting(value: Option<&Value>) -> Option<i32> {
    value
        .and_then(Value::as_i64)
        .filter(|value| (1..=4).contains(value))
        .map(|value| value as i32)
}

fn map_light_setting(value: Option<&Value>) -> Option<i32> {
    light_setting(value)
}

fn packet_body(payload: &Value) -> &Value {
    payload.get("payload").unwrap_or(payload)
}

fn same_map_file_name(left: &str, right: &str) -> bool {
    normalize_map_file_name(left) == normalize_map_file_name(right)
}

fn normalize_map_file_name(value: &str) -> String {
    let normalized = value.trim().replace('\\', "/");
    let file_name = normalized.rsplit('/').next().unwrap_or_default();
    let lower = file_name.to_ascii_lowercase();
    lower.strip_suffix(".map").unwrap_or(&lower).to_owned()
}

fn object_id_string(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => Some(number.to_string()),
        Value::String(value) => Some(value.clone()),
        _ => None,
    }
}

fn entity_light_range(light: i32) -> usize {
    (light.rem_euclid(15) as usize).min(LIGHT_TEXTURE_COUNT - 1)
}

fn motion_is_finite(motion: &NativeLightingMotion) -> bool {
    motion.camera_offset_x.is_finite()
        && motion.camera_offset_y.is_finite()
        && motion
            .entity_offsets
            .values()
            .all(|(x, y)| x.is_finite() && y.is_finite())
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
        let map = ParsedMap {
            width: 1,
            height: 1,
            cells: vec![cell(1, true)],
        };
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
