//! Shared renderer-neutral native lighting producer, extracted from Windows.
//! Presentation only; Crystal palette/material/range placement still belongs
//! to the existing runtime. Hosts supply validated scene and motion.
use crate::native_lighting_environment::{NativeLightingEnvironment, same_map_file_name};
use serde::Serialize;
use serde_json::{Value, json};
use std::{cmp::Ordering, collections::HashMap, path::Path};

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

    /// Presence comes from the host's actual packaged assets, not metadata.
    pub fn from_presence(ranges: [bool; LIGHT_TEXTURE_COUNT]) -> Self {
        Self { ranges }
    }

    pub fn complete(&self) -> bool {
        self.ranges.iter().all(|present| *present)
    }

    fn contains(&self, range: usize) -> bool {
        self.ranges.get(range).copied().unwrap_or(false)
    }

    #[doc(hidden)]
    pub fn complete_fixture() -> Self {
        Self {
            ranges: [true; LIGHT_TEXTURE_COUNT],
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NativeLightingMotion {
    pub camera_offset_x: f32,
    pub camera_offset_y: f32,
    pub entity_offsets: HashMap<String, (f32, f32)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeEffectLightSnapshot {
    pub generation: u64,
    pub key: String,
    pub tile_x: f32,
    pub tile_y: f32,
    pub light: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeMapLightCell {
    pub key: String,
    pub x: i32,
    pub y: i32,
    pub light: u8,
    pub offset_x: i32,
    pub offset_y: i32,
}

/// Only the source fields needed by the original Crystal cell-light hook.
#[derive(Clone, Copy)]
pub struct NativeMapLightInput {
    pub front_index: i16,
    pub front_image: i16,
    pub front_animation_frame: u8,
    pub light: u8,
}

/// Original x-major extraction, including nonanimated zero offset.
pub fn native_map_light_cells(
    width: u16,
    height: u16,
    cells: impl IntoIterator<Item = NativeMapLightInput>,
    frame_offsets: &HashMap<(i32, i32), (i32, i32)>,
) -> Vec<NativeMapLightCell> {
    if width == 0 || height == 0 {
        return Vec::new();
    }
    let grid_height = i32::from(height);
    cells
        .into_iter()
        .take(usize::from(width) * usize::from(height))
        .enumerate()
        .filter_map(|(index, cell)| {
            let front_image_index = (cell.front_image & 0x7fff) - 1;
            if !(1..10).contains(&cell.light) || cell.front_index == -1 || front_image_index == -1 {
                return None;
            }
            let index = i32::try_from(index).ok()?;
            let x = index / grid_height;
            let y = index % grid_height;
            let (offset_x, offset_y) = if cell.front_animation_frame > 0 {
                frame_offsets.get(&(x, y)).copied().unwrap_or((0, 0))
            } else {
                (0, 0)
            };
            Some(NativeMapLightCell {
                key: format!("{x}:{y}:{}", cell.light),
                x,
                y,
                light: cell.light,
                offset_x,
                offset_y,
            })
        })
        .collect()
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

/// Hosts must match the payload to their validated scene and current generation.
#[allow(clippy::too_many_arguments)]
pub fn build_native_lighting_render_state(
    environment: &NativeLightingEnvironment,
    generation: Option<u64>,
    force_daylight: bool,
    payload: &Value,
    center: (i32, i32),
    map_lights_input: impl IntoIterator<Item = NativeMapLightCell>,
    motion: &NativeLightingMotion,
    assets: &NativeLightAssets,
    effect_lights: &[NativeEffectLightSnapshot],
) -> Value {
    let payload_map = payload.get("mapFileName").and_then(Value::as_str);
    let map_matches = payload_map.is_some_and(|payload| {
        environment
            .current_map_file_name
            .as_deref()
            .is_none_or(|current| same_map_file_name(current, payload))
    });
    if force_daylight && map_matches && motion_is_finite(motion) {
        return json!({
            "enabled": true,
            "mapFileName": environment.current_map_file_name,
            "stageWidth": STAGE_WIDTH,
            "stageHeight": STAGE_HEIGHT,
            "timeOfDayLightSetting": 2,
            "mapLightSetting": 2,
            "mapDarkLight": 0,
            "mapLights": [],
            "entityLights": [],
        });
    }
    let enabled = assets.complete()
        && environment
            .map_light_setting
            .or(environment.time_of_day_light_setting)
            .is_some();

    if !enabled || !map_matches || !motion_is_finite(motion) {
        return disabled_state();
    }

    // Entity lights are authoritative and always outrank transient effect
    // lights. Keep only the best 200 candidates while scanning, so a large
    // gateway snapshot cannot create an unbounded clone/sort buffer.
    let mut entity_candidates: Vec<EntityLightCandidate> = Vec::with_capacity(MAX_NATIVE_LIGHTS);
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
            if entity.get("dead").and_then(Value::as_bool).unwrap_or(false) && !is_self && !is_spell
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
                + (x - i64::from(center.0)) as f32 * CELL_WIDTH
                + motion.camera_offset_x
                + entity_offset_x;
            let draw_y = ENTITY_ORIGIN_Y
                + (y - i64::from(center.1)) as f32 * CELL_HEIGHT
                + motion.camera_offset_y
                + entity_offset_y;
            if !draw_x.is_finite() || !draw_y.is_finite() {
                continue;
            }
            let priority = if is_self {
                0
            } else if kind.eq_ignore_ascii_case("npc") || kind.eq_ignore_ascii_case("merchant") {
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
                && generation.map_or(true, |generation| effect.generation == generation)
                && assets.contains(entity_light_range(effect.light))
        })
        .collect::<Vec<_>>();
    effect_candidates.sort_by(|left, right| left.key.cmp(&right.key));
    let effect_capacity = MAX_NATIVE_LIGHTS.saturating_sub(entity_lights.len());
    for effect in effect_candidates.into_iter().take(effect_capacity) {
        let dx = effect.tile_x - center.0 as f32;
        let dy = effect.tile_y - center.1 as f32;
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
    {
        for source in map_lights_input {
            if map_lights.len() == remaining {
                break;
            }
            let dx = source.x - center.0;
            let dy = source.y - center.1;
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
        "mapFileName": environment.current_map_file_name,
        "stageWidth": STAGE_WIDTH,
        "stageHeight": STAGE_HEIGHT,
        "timeOfDayLightSetting": environment.time_of_day_light_setting,
        "mapLightSetting": environment.map_light_setting,
        "mapDarkLight": environment.map_dark_light,
        "mapLights": map_lights,
        "entityLights": entity_lights,
    })
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
#[path = "native_lighting_sources_tests.rs"]
mod tests;
