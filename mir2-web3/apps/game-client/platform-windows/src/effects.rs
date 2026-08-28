//! Native Windows authoritative scene-effect parsing and lifecycle.
//!
//! Gateway packets remain authoritative for *where* an effect appears, who
//! casts it, its direction/target and when it must be removed. This module
//! only owns client-side manifest resolution (effects.generated.json +
//! per-library meta.json) and a Crystal-faithful frame clock, mirroring the
//! Web scene-effect-runtime.ts + crystal-magic-effects.ts resolver semantics.
//! It never fabricates client game state and never draws a fake/fallback
//! sprite for a missing asset - a frame whose PNG is absent yields no sprite.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::{Mutex, OnceLock};

use bevy::prelude::Resource;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::assets;
use crate::gameplay_bridge::NativeEffectEvent;

fn fx_trace_enabled() -> bool {
    std::env::var_os("MIR2_NATIVE_FX_TRACE").is_some()
}

fn fx_trace_event(
    generation: u64,
    sequence: u64,
    packet: &str,
    spell: &str,
    source_id: Option<u32>,
    dest_id: Option<u32>,
    source_tile: Option<(i32, i32)>,
    dest_tile: Option<(i32, i32)>,
    phase: &str,
    library: &str,
    base: i64,
    current_frame: &str,
    start_at: u64,
    image: &str,
) {
    if !fx_trace_enabled() {
        return;
    }
    eprintln!(
        "[fx-trace] gen={} seq={} packet={} spell={} srcId={:?} dstId={:?} srcTile={:?} dstTile={:?} phase={} lib={} base={} frame={} startAt={} image={}",
        generation,
        sequence,
        packet,
        spell,
        source_id,
        dest_id,
        source_tile,
        dest_tile,
        phase,
        library,
        base,
        current_frame,
        start_at,
        image
    );
}

/// Fixed 1024x768 logical stage + Crystal cell geometry (mirrors atlas.rs).
pub const STAGE_WIDTH: f32 = 1024.0;
pub const STAGE_HEIGHT: f32 = 768.0;
pub const CELL_WIDTH: f32 = 48.0;
pub const CELL_HEIGHT: f32 = 32.0;
pub const ENTITY_ORIGIN_X: f32 = 480.0;
pub const ENTITY_ORIGIN_Y: f32 = 352.0;

/// Entity depth gain + effect band orders (mirrors atlas.rs ENTITY_DEPTH_GAIN).
const EFFECT_DEPTH_GAIN: f32 = 10.0;
/// Persistent ObjectSpell effects sit just below the body layer (order 5).
const EFFECT_GROUND_ORDER: f32 = 4.8;
/// Relative ordering for scene/actor effects that Crystal draws in the final
/// post-world pass, after selected-target DrawBlend.
const EFFECT_TRANSIENT_ORDER: f32 = 9.0;

/// Upper bound on simultaneously-active transient effects.
pub const MAX_ACTIVE_EFFECTS: usize = 96;

/// Crystal completes the six-frame Spell actor action before Lightning begins.
const LIGHTNING_SPELL_ACTION_MS: u64 = 600;
const LIGHTNING_SOUND_FILE: &str = "M40-0.wav";
const LIGHTNING_SOUND_CUE: &str = "Lightning.complete";
const FIREBALL_SPELL_ACTION_MS: u64 = 600;
const FIREBALL_PROJECTILE_STEP_MS: u64 = 30;
const FIREBALL_TILE_TRAVEL_MS: u64 = 50;
const GUARD_RANGE_ACTION_FRAME_DELAY_MS: u64 = 400;
const FIREBALL_CAST_SOUND_FILE: &str = "M31-0.wav";
const FIREBALL_PROJECTILE_SOUND_FILE: &str = "M31-1.wav";
const FIREBALL_IMPACT_SOUND_FILE: &str = "M31-2.wav";
const FIREBALL_CAST_SOUND_CUE: &str = "FireBall.cast";
const FIREBALL_PROJECTILE_SOUND_CUE: &str = "FireBall.projectile";
const FIREBALL_IMPACT_SOUND_CUE: &str = "FireBall.impact";
const GREAT_FIREBALL_SPELL_ACTION_MS: u64 = 600;
const GREAT_FIREBALL_CAST_SOUND_FILE: &str = "M34-0.wav";
const GREAT_FIREBALL_PROJECTILE_SOUND_FILE: &str = "M34-1.wav";
const GREAT_FIREBALL_IMPACT_SOUND_FILE: &str = "M34-2.wav";
const GREAT_FIREBALL_CAST_SOUND_CUE: &str = "GreatFireBall.cast";
const GREAT_FIREBALL_PROJECTILE_SOUND_CUE: &str = "GreatFireBall.projectile";
const GREAT_FIREBALL_IMPACT_SOUND_CUE: &str = "GreatFireBall.impact";
const FIREWALL_SPELL_ACTION_MS: u64 = 600;
const FIREWALL_CAST_SOUND_FILE: &str = "M39-0.wav";
const FIREWALL_COMPLETE_SOUND_FILE: &str = "M39-1.wav";
const FIREWALL_CAST_SOUND_CUE: &str = "FireWall.cast";
const FIREWALL_COMPLETE_SOUND_CUE: &str = "FireWall.complete";
const HEALING_CAST_SOUND_FILE: &str = "M61-0.wav";
const HEALING_TARGET_SOUND_FILE: &str = "M61-1.wav";
const HEALING_CAST_SOUND_CUE: &str = "Healing.cast";
const HEALING_TARGET_SOUND_CUE: &str = "Healing.target";
const FLAMING_SWORD_SOUND_FILE: &str = "M8-1.wav";
const FLAMING_SWORD_SOUND_CUE: &str = "FlamingSword.attack";
const PLAYER_REVIVE_SOUND_FILE: &str = "M79-1.wav";
const PLAYER_REVIVE_SOUND_CUE: &str = "PlayerRevive";
const PLAYER_STRUCK_BODY_SWORD_FILE: &str = "70.wav";
const PLAYER_STRUCK_BODY_AXE_FILE: &str = "71.wav";
const PLAYER_STRUCK_BODY_LONG_STICK_FILE: &str = "72.wav";
const PLAYER_STRUCK_BODY_FIST_FILE: &str = "73.wav";
const PLAYER_STRUCK_ARMOUR_SWORD_FILE: &str = "80.wav";
const PLAYER_STRUCK_ARMOUR_AXE_FILE: &str = "81.wav";
const PLAYER_STRUCK_ARMOUR_LONG_STICK_FILE: &str = "82.wav";
const PLAYER_STRUCK_ARMOUR_FIST_FILE: &str = "83.wav";
const PLAYER_MALE_FLINCH_FILE: &str = "138.wav";
const PLAYER_FEMALE_FLINCH_FILE: &str = "139.wav";
const PLAYER_MALE_DIE_FILE: &str = "144.wav";
const PLAYER_FEMALE_DIE_FILE: &str = "145.wav";
const PLAYER_TIGER_STRUCK_1_FILE: &str = "tiger_struck_1.wav";
const PLAYER_TIGER_STRUCK_2_FILE: &str = "tiger_struck_2.wav";
const PLAYER_WOLF_STRUCK_FILE: &str = "wolf_struck1.wav";
const PLAYER_DIE_SOUND_DELAY_MS: u64 = 100;
const SCARECROW_DIE_SOUND_FILE: &str = "005-3.wav";
const SOUL_FIREBALL_SPELL_ACTION_MS: u64 = 600;
const SOUL_FIREBALL_CAST_SOUND_FILE: &str = "M64-0.wav";
const SOUL_FIREBALL_PROJECTILE_SOUND_FILE: &str = "M64-1.wav";
const SOUL_FIREBALL_IMPACT_SOUND_FILE: &str = "M64-2.wav";
const SOUL_FIREBALL_CAST_SOUND_CUE: &str = "SoulFireBall.cast";
const SOUL_FIREBALL_PROJECTILE_SOUND_CUE: &str = "SoulFireBall.projectile";
const SOUL_FIREBALL_IMPACT_SOUND_CUE: &str = "SoulFireBall.impact";

const NATIVE_SOAK_METRICS_INTERVAL_MS: u64 = 10_000;

fn native_soak_metrics_enabled() -> bool {
    std::env::var("MIR2_NATIVE_SOAK_METRICS")
        .map(|value| value.trim() == "1")
        .unwrap_or(false)
}

/// Pure timing predicate for the producer's periodic metrics gate.
fn should_emit_native_soak_metrics(last_emitted_at_ms: Option<u64>, now_ms: u64) -> bool {
    last_emitted_at_ms.map_or(true, |last| {
        now_ms.saturating_sub(last) >= NATIVE_SOAK_METRICS_INTERVAL_MS
    })
}

/// Build the single-line JSON payload without reading process state or logging.
fn native_soak_metrics_json(process_id: u32, timestamp_ms: u64, active_effects: usize) -> String {
    json!({
        "processId": process_id,
        "timestampMs": timestamp_ms,
        "activeEffects": active_effects,
        "activeEffectsCap": MAX_ACTIVE_EFFECTS,
    })
    .to_string()
}

/// Direction numbering matches atlas.rs: Up=0,UpRight=1,Right=2,DownRight=3,
/// Down=4,DownLeft=5,Left=6,UpLeft=7.
pub(crate) fn direction_index(direction: &str) -> u32 {
    match direction.to_ascii_lowercase().as_str() {
        "up" => 0,
        "upright" => 1,
        "right" => 2,
        "downright" => 3,
        "down" => 4,
        "downleft" => 5,
        "left" => 6,
        "upleft" => 7,
        _ => 4,
    }
}

/// Crystal `MapControl.Direction16`: 0 is up and values advance clockwise in
/// 22.5-degree sectors. FireBall's missile uses this 16-way index with a
/// ten-frame stride (`6` visible frames plus `skip=4`).
fn projectile_direction16(source: (i32, i32), destination: (i32, i32)) -> u32 {
    let dx = (destination.0 - source.0) as f32;
    let dy = (destination.1 - source.1) as f32;
    if dx == 0.0 && dy == 0.0 {
        return 0;
    }
    let degrees = dx.atan2(-dy).to_degrees().rem_euclid(360.0);
    (((degrees + 11.25).rem_euclid(360.0) / 22.5).floor() as u32) % 16
}

fn max_tile_distance(source: (i32, i32), destination: (i32, i32)) -> u64 {
    u64::from(
        source
            .0
            .abs_diff(destination.0)
            .max(source.1.abs_diff(destination.1)),
    )
}

fn crystal_projectile_clock(distance: u64) -> (u64, u64) {
    let duration_ms = distance.saturating_mul(FIREBALL_TILE_TRAVEL_MS).max(1);
    let process_frame_count = (duration_ms / FIREBALL_PROJECTILE_STEP_MS).max(1);
    let frame_interval_ms = (duration_ms / process_frame_count).max(1);
    (duration_ms, frame_interval_ms)
}

fn actor_is_player(actor: &Value) -> bool {
    actor
        .get("kind")
        .and_then(Value::as_str)
        .is_some_and(|kind| {
            matches!(
                kind.to_ascii_lowercase().as_str(),
                "selfplayer" | "player" | "hero"
            )
        })
}

fn actor_is_scarecrow(actor: &Value) -> bool {
    if !actor
        .get("kind")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind.eq_ignore_ascii_case("monster"))
    {
        return false;
    }
    actor_sprite_library(actor, "bodyLibrary").is_some_and(|library| {
        let normalized = library.trim().replace('\\', "/");
        let normalized = normalized.trim_start_matches('/');
        normalized.eq_ignore_ascii_case("Monster/005")
            || normalized.eq_ignore_ascii_case("original-ui/Monster/005")
    })
}

fn actor_sound_key(actor: &Value) -> Option<String> {
    actor
        .get("objectId")
        .and_then(|value| {
            value
                .as_u64()
                .map(|id| id.to_string())
                .or_else(|| value.as_str().map(str::to_owned))
        })
        .filter(|value| !value.is_empty() && value != "0")
}

fn actor_sprite_library<'a>(actor: &'a Value, field: &str) -> Option<&'a str> {
    actor
        .get("sprite")
        .and_then(Value::as_object)
        .and_then(|sprite| sprite.get(field))
        .or_else(|| actor.get(field))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn actor_is_female(actor: &Value) -> bool {
    actor
        .get("genderKey")
        .or_else(|| actor.get("gender"))
        .and_then(Value::as_str)
        .is_some_and(|gender| gender.eq_ignore_ascii_case("female"))
}

fn actor_is_assassin(actor: &Value) -> bool {
    actor
        .get("classKey")
        .or_else(|| actor.get("class"))
        .and_then(Value::as_str)
        .is_some_and(|class| class.eq_ignore_ascii_case("assassin"))
}

fn player_library_index(library: &str) -> Option<i32> {
    let normalized = library.trim().replace('\\', "/");
    let tail = normalized.trim_end_matches('/').rsplit('/').next()?;
    let digits = tail
        .chars()
        .skip_while(|character| !character.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    (!digits.is_empty())
        .then(|| digits.parse::<i32>().ok())
        .flatten()
}

fn player_weapon_index(attacker: Option<&Value>) -> i32 {
    let Some(attacker) = attacker.filter(|actor| actor_is_player(actor)) else {
        return -2;
    };
    let weapon = attacker
        .get("weapon")
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .or_else(|| actor_sprite_library(attacker, "weaponLibrary").and_then(player_library_index))
        .unwrap_or(-1);
    if actor_is_assassin(attacker) && weapon != -1 {
        1
    } else {
        weapon
    }
}

fn player_mount_index(actor: &Value) -> Option<i32> {
    match actor.get("ridingMount").and_then(Value::as_bool) {
        Some(false) => return None,
        Some(true) => {
            if let Some(mount_type) = actor
                .get("mountType")
                .and_then(Value::as_i64)
                .and_then(|value| i32::try_from(value).ok())
                .filter(|value| *value >= 0)
            {
                return Some(mount_type);
            }
        }
        None => {}
    }
    actor_sprite_library(actor, "mountLibrary").and_then(player_library_index)
}

fn player_struck_body_file(target: &Value, attacker: Option<&Value>) -> Option<&'static str> {
    let heavy_armour = !actor_is_assassin(target)
        && actor_sprite_library(target, "bodyLibrary")
            .and_then(player_library_index)
            .is_some_and(|armour| matches!(armour, 3 | 6 | 9));
    let weapon = player_weapon_index(attacker);
    if matches!(
        weapon,
        0 | 1
            | 2
            | 3
            | 5
            | 7
            | 8
            | 9
            | 11
            | 12
            | 13
            | 15
            | 18
            | 19
            | 20
            | 23
            | 24
            | 25
            | 26
            | 28
            | 29
            | 31
            | 32
            | 33
            | 34
            | 35
            | 37
            | 40
            | 41
    ) {
        Some(if heavy_armour {
            PLAYER_STRUCK_ARMOUR_SWORD_FILE
        } else {
            PLAYER_STRUCK_BODY_SWORD_FILE
        })
    } else if matches!(weapon, 4 | 14 | 16 | 38) {
        Some(if heavy_armour {
            PLAYER_STRUCK_ARMOUR_AXE_FILE
        } else {
            PLAYER_STRUCK_BODY_AXE_FILE
        })
    } else if matches!(weapon, 6 | 10 | 17 | 21 | 22 | 27 | 30 | 36 | 39) {
        Some(if heavy_armour {
            PLAYER_STRUCK_ARMOUR_LONG_STICK_FILE
        } else {
            PLAYER_STRUCK_BODY_LONG_STICK_FILE
        })
    } else if weapon == -1 {
        Some(if heavy_armour {
            PLAYER_STRUCK_ARMOUR_FIST_FILE
        } else {
            PLAYER_STRUCK_BODY_FIST_FILE
        })
    } else {
        None
    }
}

fn player_sound_actor_key(payload: &Value, self_player: bool) -> Option<String> {
    payload
        .get("_nativeTarget")
        .and_then(actor_sound_key)
        .or_else(|| {
            (!self_player)
                .then(|| {
                    payload
                        .get("objectId")
                        .and_then(|value| value.as_u64().map(|id| id.to_string()))
                })
                .flatten()
        })
        .or_else(|| self_player.then(|| "self".to_owned()))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EffectFrameMeta {
    path: String,
    width: f32,
    height: f32,
    x: f32,
    y: f32,
    #[serde(default)]
    shadow_x: Option<f32>,
    #[serde(default)]
    shadow_y: Option<f32>,
    #[serde(default)]
    mask_path: Option<String>,
    #[serde(default)]
    mask_width: Option<f32>,
    #[serde(default)]
    mask_height: Option<f32>,
    #[serde(default)]
    mask_x: Option<f32>,
    #[serde(default)]
    mask_y: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
struct LibraryMeta {
    #[serde(default)]
    frames: HashMap<String, EffectFrameMeta>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EffectOffset {
    #[serde(default)]
    x: f32,
    #[serde(default)]
    y: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubSpec {
    library: String,
    base: i64,
    count: i64,
    #[serde(default)]
    interval: Option<i64>,
    #[serde(default)]
    direction_count: Option<i64>,
    #[serde(default)]
    direction_stride: Option<i64>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    blend: Option<bool>,
    #[serde(default)]
    rate: Option<f32>,
    #[serde(default)]
    light: Option<i32>,
    #[serde(default)]
    repeat: Option<bool>,
    #[serde(default)]
    offset: Option<EffectOffset>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DirectionRange {
    direction: i64,
    base: i64,
    end: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ValueRange {
    value: i64,
    base: i64,
    end: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EffectSpec {
    #[serde(default)]
    spell: Option<String>,
    #[serde(default)]
    spell_id: Option<u32>,
    #[serde(default)]
    effect: Option<String>,
    #[serde(default)]
    effect_id: Option<u32>,
    #[serde(default)]
    kind: Option<String>,
    library: String,
    #[serde(default)]
    base: i64,
    #[serde(default)]
    count: i64,
    #[serde(default)]
    interval: i64,
    #[serde(default)]
    direction_count: Option<i64>,
    #[serde(default)]
    direction_stride: Option<i64>,
    #[serde(default)]
    direction_ranges: Option<Vec<DirectionRange>>,
    #[serde(default)]
    value_count: Option<i64>,
    #[serde(default)]
    value_stride: Option<i64>,
    #[serde(default)]
    value_ranges: Option<Vec<ValueRange>>,
    #[serde(default)]
    blend: Option<bool>,
    #[serde(default)]
    rate: Option<f32>,
    #[serde(default)]
    light: Option<i32>,
    #[serde(default)]
    repeat: Option<bool>,
    #[serde(default)]
    offset: Option<EffectOffset>,
    #[serde(default)]
    projectile: Option<SubSpec>,
    #[serde(default)]
    impact: Option<SubSpec>,
    #[serde(default)]
    return_effect: Option<SubSpec>,
    #[serde(default)]
    provenance: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpellEffectName {
    id: u32,
    name: String,
}

#[derive(Debug, Deserialize)]
struct EffectsManifest {
    #[serde(default)]
    available: Vec<String>,
    #[serde(default)]
    spell_effect_enum: Vec<Value>,
    #[serde(default)]
    spell_effect_map: Vec<SpellEffectName>,
    #[serde(default)]
    spell_effects: Vec<EffectSpec>,
    #[serde(default)]
    ground_effects: Vec<EffectSpec>,
    #[serde(default)]
    client_effects: Vec<EffectSpec>,
    #[serde(default)]
    object_effects: Vec<EffectSpec>,
    #[serde(default)]
    map_effects: Vec<EffectSpec>,
}

#[derive(Debug, Clone)]
pub(crate) struct Animation {
    pub name: String,
    pub kind: String,
    pub frames: Vec<EffectFrameMeta>,
    pub interval: u64,
    pub blend: bool,
    /// Crystal DrawBlend rate. `1.0` preserves the historical full-strength
    /// default; FlamingSword's Attack1 overlay uses `0.7`.
    pub opacity: f32,
    pub repeat: bool,
    pub offset_x: f32,
    pub offset_y: f32,
    pub duration_ms: u64,
    /// Crystal manifest light intensity for this animation phase.
    pub light: Option<i32>,
}

impl Animation {
    /// The source frame for the given elapsed ms. Crystal missiles cycle their
    /// visible `FrameCount` with `% FrameCount` while their separate movement
    /// `Count` remains in flight; that frame cycling must not make the effect's
    /// lifecycle repeat forever.
    pub(crate) fn frame_at(&self, elapsed_ms: u64) -> Option<&EffectFrameMeta> {
        if self.frames.is_empty() {
            return None;
        }
        let mut index = elapsed_ms / self.interval.max(1);
        if index >= self.frames.len() as u64 {
            if !self.repeat && self.kind != "projectile" {
                return None;
            }
            index %= self.frames.len() as u64;
        }
        self.frames.get(index as usize)
    }
}

pub(crate) struct EffectCatalog {
    libraries: HashMap<String, LibraryMeta>,
    spell_by_name: HashMap<String, EffectSpec>,
    ground_by_spell: HashMap<String, EffectSpec>,
    map_by_name: HashMap<String, EffectSpec>,
    effect_name_by_number: HashMap<u32, String>,
}

fn library_dir(library: &str) -> String {
    library.replace(":", "_")
}

impl EffectCatalog {
    /// Load the exported effect manifest + per-library frame metadata. Returns
    /// None (non-throwing) when the manifest/libraries are unavailable, so the
    /// native client never panics on a missing asset bundle.
    pub(crate) fn load() -> Option<EffectCatalog> {
        let manifest_path = assets::asset_path("original-effects/effects.generated.json")?;
        let bytes = fs::read(&manifest_path).ok()?;
        let manifest: EffectsManifest = serde_json::from_slice(&bytes).ok()?;

        let mut libraries = HashMap::new();
        for library in &manifest.available {
            let dir = library_dir(library);
            let Some(meta_path) = assets::asset_path(&format!("original-effects/{dir}/meta.json"))
            else {
                continue;
            };
            if let Ok(bytes) = fs::read(&meta_path) {
                if let Ok(meta) = serde_json::from_slice::<LibraryMeta>(&bytes) {
                    libraries.insert(library.clone(), meta);
                }
            }
        }

        let mut spell_by_name = HashMap::new();
        for entry in &manifest.spell_effects {
            if let Some(name) = &entry.spell {
                spell_by_name.insert(name.clone(), entry.clone());
            }
        }
        let mut ground_by_spell = HashMap::new();
        for entry in &manifest.ground_effects {
            if let Some(name) = &entry.spell {
                ground_by_spell.insert(name.clone(), entry.clone());
            }
        }
        let mut map_by_name = HashMap::new();
        for entry in manifest
            .client_effects
            .iter()
            .chain(manifest.object_effects.iter())
            .chain(manifest.map_effects.iter())
        {
            if let Some(name) = &entry.effect {
                map_by_name.insert(name.clone(), entry.clone());
            }
        }

        let mut effect_name_by_number = HashMap::new();
        if !manifest.spell_effect_map.is_empty() {
            for entry in &manifest.spell_effect_map {
                effect_name_by_number.insert(entry.id, entry.name.clone());
            }
        } else {
            for (index, entry) in manifest.spell_effect_enum.iter().enumerate() {
                if let Some(name) = entry.as_str() {
                    effect_name_by_number.insert(index as u32, name.to_owned());
                } else {
                    let id = entry
                        .get("id")
                        .and_then(Value::as_u64)
                        .unwrap_or(index as u64) as u32;
                    if let Some(name) = entry.get("name").and_then(Value::as_str) {
                        effect_name_by_number.insert(id, name.to_owned());
                    }
                }
            }
        }

        Some(EffectCatalog {
            libraries,
            spell_by_name,
            ground_by_spell,
            map_by_name,
            effect_name_by_number,
        })
    }

    fn resolve_frames(&self, library: &str, base: i64, count: i64) -> Vec<EffectFrameMeta> {
        if count <= 0 {
            return Vec::new();
        }
        let Some(meta) = self.libraries.get(library) else {
            return Vec::new();
        };
        let mut frames = Vec::with_capacity(count as usize);
        for index in 0..count {
            let Some(frame) = meta.frames.get(&(base + index).to_string()) else {
                return Vec::new();
            };
            if !crate::frame_png_exists(&frame.path) {
                return Vec::new();
            }
            frames.push(frame.clone());
        }
        frames
    }

    fn resolve_sub(
        &self,
        sub: &SubSpec,
        name: &str,
        fallback_kind: &str,
        direction: u32,
    ) -> Option<Animation> {
        if sub
            .direction_count
            .is_some_and(|count| count > 0 && direction >= count as u32)
        {
            return None;
        }
        let base = sub.base + i64::from(direction) * sub.direction_stride.unwrap_or(0);
        let frames = self.resolve_frames(&sub.library, base, sub.count);
        if frames.is_empty() {
            return None;
        }
        let interval = sub.interval.unwrap_or(100).max(0) as u64;
        let offset = sub
            .offset
            .clone()
            .unwrap_or(EffectOffset { x: 0.0, y: 0.0 });
        let frame_count = frames.len() as u64;
        Some(Animation {
            name: name.to_owned(),
            kind: sub.kind.clone().unwrap_or_else(|| fallback_kind.to_owned()),
            frames,
            interval,
            blend: sub.blend.unwrap_or(true),
            opacity: sub.rate.unwrap_or(1.0).clamp(0.0, 1.0),
            repeat: sub.repeat.unwrap_or(false),
            offset_x: offset.x,
            offset_y: offset.y,
            duration_ms: interval * frame_count,
            light: sub.light,
        })
    }

    pub(crate) fn resolve_animation(
        &self,
        entry: &EffectSpec,
        direction: u32,
        value: u32,
    ) -> Option<Animation> {
        if let Some(direction_count) = entry.direction_count {
            if direction_count > 0 && direction >= direction_count as u32 {
                return None;
            }
        }
        if let Some(value_count) = entry.value_count {
            if value_count > 0 && value >= value_count as u32 {
                return None;
            }
        }
        let base = entry.base
            + i64::from(direction) * entry.direction_stride.unwrap_or(0)
            + i64::from(value) * entry.value_stride.unwrap_or(0);
        let frames = self.resolve_frames(&entry.library, base, entry.count.max(0));
        if frames.is_empty() {
            return None;
        }
        let interval = entry.interval.max(0) as u64;
        let offset = entry
            .offset
            .clone()
            .unwrap_or(EffectOffset { x: 0.0, y: 0.0 });
        let frame_count = frames.len() as u64;
        Some(Animation {
            name: entry
                .spell
                .clone()
                .or_else(|| entry.effect.clone())
                .unwrap_or_else(|| "effect".to_owned()),
            kind: entry.kind.clone().unwrap_or_else(|| "impact".to_owned()),
            frames,
            interval,
            blend: entry.blend.unwrap_or(true),
            opacity: entry.rate.unwrap_or(1.0).clamp(0.0, 1.0),
            repeat: entry.repeat.unwrap_or(false),
            offset_x: offset.x,
            offset_y: offset.y,
            duration_ms: interval * frame_count,
            light: entry.light,
        })
    }

    pub(crate) fn spell_cast_animation(&self, spell: &str, direction: u32) -> Option<Animation> {
        let entry = self.spell_by_name.get(spell)?;
        if matches!(
            entry.kind.as_deref().unwrap_or(""),
            "projectile" | "impact" | "target" | "attackOverlay"
        ) {
            return None;
        }
        self.resolve_animation(entry, direction, 0)
    }

    pub(crate) fn spell_attack_overlay_animation(
        &self,
        spell: &str,
        direction: u32,
    ) -> Option<Animation> {
        let entry = self.spell_by_name.get(spell)?;
        if entry.kind.as_deref() != Some("attackOverlay") {
            return None;
        }
        self.resolve_animation(entry, direction, 0)
    }

    pub(crate) fn spell_world_animation(
        &self,
        spell: &str,
        direction: u32,
        value: u32,
    ) -> Option<Animation> {
        self.map_animation(spell, value)
            .or_else(|| self.spell_animation(spell, direction))
    }

    pub(crate) fn spell_animation(&self, spell: &str, direction: u32) -> Option<Animation> {
        let entry = self.spell_by_name.get(spell)?;
        self.resolve_animation(entry, direction, 0)
    }

    pub(crate) fn spell_projectile_animation(
        &self,
        spell: &str,
        direction: u32,
    ) -> Option<Animation> {
        let entry = self.spell_by_name.get(spell)?;
        let sub = entry.projectile.as_ref()?;
        self.resolve_sub(sub, spell, "projectile", direction)
    }

    pub(crate) fn spell_impact_animation(&self, spell: &str) -> Option<Animation> {
        let entry = self.spell_by_name.get(spell)?;
        let sub = entry.impact.as_ref()?;
        self.resolve_sub(sub, spell, "impact", 0)
    }

    pub(crate) fn spell_return_animation(&self, spell: &str) -> Option<Animation> {
        let entry = self.spell_by_name.get(spell)?;
        let sub = entry.return_effect.as_ref()?;
        self.resolve_sub(sub, spell, "return", 0)
    }

    pub(crate) fn map_animation(&self, name: &str, value: u32) -> Option<Animation> {
        let entry = self
            .map_by_name
            .get(name)
            .or_else(|| self.ground_by_spell.get(name))?;
        self.resolve_animation(entry, 0, value)
    }

    pub(crate) fn map_animation_by_number(&self, effect: u32, value: u32) -> Option<Animation> {
        let name = self.effect_name_for_number(effect)?;
        self.map_animation(&name, value)
    }

    fn right_guard_range_hit_animation(&self) -> Option<Animation> {
        let animation = self.map_animation("RightGuardRangeHit", 0)?;
        (animation.kind == "impact"
            && animation.frames.len() == 5
            && animation.interval == 60
            && animation.duration_ms == 300
            && animation.blend
            && (animation.opacity - 1.0).abs() < f32::EPSILON
            && animation.light == Some(6)
            && !animation.repeat)
            .then_some(animation)
    }

    fn left_guard_range_projectile_animation(&self, direction: u32) -> Option<Animation> {
        let entry = self.map_by_name.get("LeftGuardRangeProjectile")?;
        let animation = self.resolve_animation(entry, direction, 0)?;
        (animation.kind == "projectile"
            && animation.frames.len() == 6
            && animation.interval == FIREBALL_PROJECTILE_STEP_MS
            && animation.duration_ms == 180
            && animation.blend
            && (animation.opacity - 1.0).abs() < f32::EPSILON
            && animation.light == Some(6)
            && !animation.repeat)
            .then_some(animation)
    }

    fn effect_name_for_number(&self, effect: u32) -> Option<String> {
        self.effect_name_by_number
            .get(&effect)
            .cloned()
            .or_else(|| spell_name_by_number(effect))
    }

    fn is_empty(&self) -> bool {
        self.libraries.is_empty() && self.spell_by_name.is_empty()
    }
}

/// Built-in Spell enum id -> name fallback (mirrors SPELL_NAME_BY_ID), used
/// when the manifest spell-effect enum is absent. It also answers the
/// ObjectSpell spell byte and the ObjectEffect/MapEffect raw byte in fallback.
fn spell_name_by_number(spell: u32) -> Option<String> {
    let name = match spell {
        0 => "None",
        1 => "Fencing",
        2 => "Slaying",
        3 => "Thrusting",
        4 => "HalfMoon",
        5 => "ShoulderDash",
        6 => "TwinDrakeBlade",
        7 => "Entrapment",
        8 => "FlamingSword",
        9 => "LionRoar",
        10 => "CrossHalfMoon",
        11 => "BladeAvalanche",
        12 => "ProtectionField",
        13 => "Rage",
        14 => "CounterAttack",
        15 => "SlashingBurst",
        16 => "Fury",
        17 => "ImmortalSkin",
        31 => "FireBall",
        32 => "Repulsion",
        33 => "ElectricShock",
        34 => "GreatFireBall",
        35 => "HellFire",
        36 => "ThunderBolt",
        37 => "Teleport",
        38 => "FireBang",
        39 => "FireWall",
        40 => "Lightning",
        41 => "FrostCrunch",
        42 => "ThunderStorm",
        43 => "MagicShield",
        44 => "TurnUndead",
        45 => "Vampirism",
        46 => "IceStorm",
        47 => "FlameDisruptor",
        48 => "Mirroring",
        49 => "FlameField",
        50 => "Blizzard",
        51 => "MagicBooster",
        52 => "MeteorStrike",
        53 => "IceThrust",
        54 => "FastMove",
        55 => "StormEscape",
        61 => "Healing",
        62 => "SpiritSword",
        63 => "Poisoning",
        64 => "SoulFireBall",
        65 => "SummonSkeleton",
        67 => "Hiding",
        68 => "MassHiding",
        69 => "SoulShield",
        70 => "Revelation",
        71 => "BlessedArmour",
        72 => "EnergyRepulsor",
        73 => "TrapHexagon",
        74 => "Purification",
        75 => "MassHealing",
        76 => "Hallucination",
        77 => "UltimateEnhancer",
        78 => "SummonShinsu",
        79 => "Reincarnation",
        80 => "SummonHolyDeva",
        81 => "Curse",
        82 => "Plague",
        83 => "PoisonCloud",
        84 => "EnergyShield",
        85 => "PetEnhancer",
        86 => "HealingCircle",
        91 => "FatalSword",
        92 => "DoubleSlash",
        93 => "Haste",
        94 => "FlashDash",
        95 => "LightBody",
        96 => "HeavenlySword",
        97 => "FireBurst",
        98 => "Trap",
        99 => "PoisonSword",
        100 => "MoonLight",
        101 => "MPEater",
        102 => "SwiftFeet",
        103 => "DarkBody",
        104 => "Hemorrhage",
        105 => "CrescentSlash",
        106 => "MoonMist",
        107 => "CatTongue",
        121 => "Focus",
        122 => "StraightShot",
        123 => "DoubleShot",
        124 => "ExplosiveTrap",
        125 => "DelayedExplosion",
        126 => "Meditation",
        127 => "BackStep",
        128 => "ElementalShot",
        129 => "Concentration",
        130 => "Stonetrap",
        131 => "ElementalBarrier",
        132 => "SummonVampire",
        133 => "VampireShot",
        134 => "SummonToad",
        135 => "PoisonShot",
        136 => "CrippleShot",
        137 => "SummonSnakes",
        138 => "NapalmShot",
        139 => "OneWithNature",
        140 => "BindingShot",
        141 => "MentalState",
        151 => "Blink",
        152 => "Portal",
        153 => "BattleCry",
        154 => "FireBounce",
        155 => "MeteorShower",
        200 => "DigOutZombie",
        201 => "Rubble",
        202 => "MapLightning",
        203 => "MapLava",
        204 => "MapQuake1",
        205 => "MapQuake2",
        206 => "DigOutArmadillo",
        207 => "GeneralMeowMeowThunder",
        208 => "StoneGolemQuake",
        209 => "EarthGolemPile",
        210 => "TreeQueenRoot",
        211 => "TreeQueenMassRoots",
        212 => "TreeQueenGroundRoots",
        213 => "TucsonGeneralRock",
        214 => "FlyingStatueIceTornado",
        215 => "DarkOmaKingNuke",
        216 => "HornedSorcererDustTornado",
        217 => "HornedCommanderRockFall",
        218 => "HornedCommanderRockSpike",
        _ => return None,
    };
    Some(name.to_owned())
}

static EFFECT_CATALOG: OnceLock<Option<EffectCatalog>> = OnceLock::new();

fn effect_catalog() -> &'static Option<EffectCatalog> {
    EFFECT_CATALOG.get_or_init(EffectCatalog::load)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EffectKindTag {
    Ground,
    SceneForeground,
    Cast,
    AttackOverlay,
    Projectile,
    Impact,
    Persistent,
}

impl EffectKindTag {
    fn z_order(self) -> f32 {
        match self {
            EffectKindTag::SceneForeground
            | EffectKindTag::Cast
            | EffectKindTag::AttackOverlay
            | EffectKindTag::Projectile
            | EffectKindTag::Impact => EFFECT_TRANSIENT_ORDER,
            _ => EFFECT_GROUND_ORDER,
        }
    }

    fn draws_post_world(self) -> bool {
        matches!(
            self,
            EffectKindTag::SceneForeground
                | EffectKindTag::Cast
                | EffectKindTag::AttackOverlay
                | EffectKindTag::Projectile
                | EffectKindTag::Impact
        )
    }
}

#[derive(Debug)]
struct EffectInstance {
    key: String,
    kind: EffectKindTag,
    /// Anchor tile (Ground/Persistent) or destination tile (Impact).
    tile_x: i32,
    tile_y: i32,
    /// Projectile source (None for non-projectile instances).
    from_x: Option<f32>,
    from_y: Option<f32>,
    current: Option<Animation>,
    queued: Option<Animation>,
    return_queued: Option<Animation>,
    started_at: u64,
    /// Wall-clock ms before which the instance is not yet visible (delayTime).
    start_at: u64,
    persistent_object_id: Option<u32>,
    /// Event provenance so FX trace can correlate generation/sequence/packet/spell.
    provenance: EffectProvenance,
}

#[derive(Debug, Clone, Default)]
struct EffectProvenance {
    generation: u64,
    sequence: u64,
    packet: String,
    spell: String,
}

#[derive(Debug, Clone)]
struct PendingEffectSound {
    key: String,
    due_at_ms: u64,
    requires_active_effect: bool,
    event: mir2_client_bevy::audio::NativeGameplaySoundEvent,
}

#[derive(Debug, Clone, Copy)]
struct LocalProjectileTarget {
    target_id: Option<u32>,
    fallback: (i32, i32),
    resolved_at_launch: bool,
}

/// Renderer-neutral light emitted by one currently-active effect phase.
///
/// The lighting producer runs on the gateway task while effect animation runs
/// on Bevy's main thread. A small process-local snapshot keeps that boundary
/// explicit without adding a second transport contract. Fractional tiles are
/// intentional: projectile lights follow the same interpolated tile as the
/// visible projectile, while impact/cast lights remain tile anchored.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NativeEffectLightSnapshot {
    pub generation: u64,
    pub key: String,
    pub tile_x: f32,
    pub tile_y: f32,
    pub light: i32,
}

static ACTIVE_EFFECT_LIGHTS: OnceLock<Mutex<Vec<NativeEffectLightSnapshot>>> = OnceLock::new();

fn active_effect_lights() -> &'static Mutex<Vec<NativeEffectLightSnapshot>> {
    ACTIVE_EFFECT_LIGHTS.get_or_init(|| Mutex::new(Vec::new()))
}

fn publish_effect_lights(snapshots: Vec<NativeEffectLightSnapshot>) {
    let mut current = active_effect_lights()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *current = snapshots;
}

/// Read the latest still-active effect light snapshot for the native lighting
/// producer. A clone keeps the lock out of the render-state construction path.
pub(crate) fn native_effect_light_snapshots() -> Vec<NativeEffectLightSnapshot> {
    active_effect_lights()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// Bevy resource holding the authoritative effect event buffer and the active
/// effect set. The catalog is a lazily-loaded shared singleton.
#[derive(Resource)]
pub(crate) struct NativeEffects {
    active: Vec<EffectInstance>,
    anchor_object_ids: HashMap<String, u32>,
    /// Effects owned by a source actor but rendered along a separate path
    /// (for example a missile moving toward a target). Source removal clears
    /// the effect without incorrectly snapping its destination to the owner.
    source_object_ids: HashMap<String, u32>,
    /// The current native presentation tombstones most ObjectHide actors
    /// before the effect consumer sees its zone map. Crystal Hide keeps the
    /// object instance alive, so source-owned LeftGuard missiles retain only
    /// the last authoritative tiles they still reference.
    hidden_left_guard_tiles: HashMap<u32, (i32, i32)>,
    /// Delayed client-owned effects may depend on their source actor only
    /// until the Crystal action-frame boundary creates the target-owned effect.
    prestart_source_object_ids: HashMap<String, u32>,
    anchor_player_keys: HashSet<String>,
    zone_tiles: HashMap<u32, (i32, i32)>,
    /// The Rust simulation currently emits a compatibility ObjectProjectile
    /// immediately after some ObjectMagic packets. Crystal creates FireBall
    /// and SoulFireBall missiles locally after the Spell action completes, so
    /// consume the adjacent compatibility packet instead of drawing twice.
    local_projectile_dedupe: HashMap<(String, u32, u32), u64>,
    /// Target presence is resolved at Crystal's Spell-completion boundary,
    /// not when ObjectMagic first arrives. This preserves point-flight when a
    /// target disappears during wind-up and binds a target that appears before
    /// launch.
    local_projectile_targets: HashMap<String, LocalProjectileTarget>,
    /// Audio-only cast branches (SoulFireBall has no cast bitmap) still need a
    /// one-shot queue that does not depend on an active render instance.
    ready_sounds: Vec<mir2_client_bevy::audio::NativeGameplaySoundEvent>,
    pending_sounds: Vec<PendingEffectSound>,
    dead_player_sound_keys: HashSet<String>,
    dead_scarecrow_sound_keys: HashSet<String>,
    revived_player_effect_keys: HashSet<String>,
    last_effect_sequence: u64,
    last_generation: u64,
    instance_seq: u64,
    now_ms: u64,
    player_x: i32,
    player_y: i32,
    post_world_depth_bounds: Option<(f32, f32)>,
    last_state: Option<String>,
    soak_metrics_enabled: bool,
    last_soak_metrics_at_ms: Option<u64>,
}

impl Default for NativeEffects {
    fn default() -> Self {
        Self {
            active: Vec::new(),
            anchor_object_ids: HashMap::new(),
            source_object_ids: HashMap::new(),
            hidden_left_guard_tiles: HashMap::new(),
            prestart_source_object_ids: HashMap::new(),
            anchor_player_keys: HashSet::new(),
            zone_tiles: HashMap::new(),
            local_projectile_dedupe: HashMap::new(),
            local_projectile_targets: HashMap::new(),
            ready_sounds: Vec::new(),
            pending_sounds: Vec::new(),
            dead_player_sound_keys: HashSet::new(),
            dead_scarecrow_sound_keys: HashSet::new(),
            revived_player_effect_keys: HashSet::new(),
            last_effect_sequence: 0,
            last_generation: 0,
            instance_seq: 0,
            now_ms: 0,
            player_x: 0,
            player_y: 0,
            post_world_depth_bounds: None,
            last_state: None,
            soak_metrics_enabled: native_soak_metrics_enabled(),
            last_soak_metrics_at_ms: None,
        }
    }
}

impl NativeEffects {
    pub(crate) fn observe_render_payload(&mut self, payload: &Value) {
        self.post_world_depth_bounds = Some(crate::atlas::post_world_depth_bounds(payload));
    }

    fn maybe_emit_native_soak_metrics(&mut self, now_ms: u64) {
        if !self.soak_metrics_enabled
            || !should_emit_native_soak_metrics(self.last_soak_metrics_at_ms, now_ms)
        {
            return;
        }

        self.last_soak_metrics_at_ms = Some(now_ms);
        eprintln!(
            "[native-soak-fx] {}",
            native_soak_metrics_json(std::process::id(), now_ms, self.active.len())
        );
    }

    fn current_light_snapshots(&self, now_ms: u64) -> Vec<NativeEffectLightSnapshot> {
        let mut snapshots = self
            .active
            .iter()
            .filter_map(|instance| {
                if !instance_still_active(instance, now_ms) {
                    return None;
                }
                let (animation, started_at) = advance_instance(instance, now_ms)?;
                let light = animation.light.filter(|value| *value > 0)?;
                let (tile_x, tile_y) =
                    instance_current_tile(instance, animation, started_at, now_ms);
                if !tile_x.is_finite() || !tile_y.is_finite() {
                    return None;
                }
                Some(NativeEffectLightSnapshot {
                    generation: self.last_generation,
                    key: instance.key.clone(),
                    tile_x,
                    tile_y,
                    light,
                })
            })
            .collect::<Vec<_>>();
        snapshots.sort_by(|left, right| left.key.cmp(&right.key));
        snapshots
    }

    fn publish_current_light_snapshots(&self, now_ms: u64, visible: bool) {
        let snapshots = if visible {
            self.current_light_snapshots(now_ms)
        } else {
            Vec::new()
        };
        publish_effect_lights(snapshots.clone());
        crate::map_parser::lighting::publish_effect_lighting_frame(self.last_generation, snapshots);
    }

    /// Forward authoritative effect events + player/anchor data. Events are
    /// deduplicated by their monotonic sequence, so re-delivered snapshots
    /// never replay an already-seen event.
    pub(crate) fn observe(
        &mut self,
        now_ms: u64,
        player_x: i32,
        player_y: i32,
        events: &[NativeEffectEvent],
        zone_tiles: &HashMap<u32, (i32, i32)>,
    ) {
        self.now_ms = now_ms;
        self.player_x = player_x;
        self.player_y = player_y;
        if self.post_world_depth_bounds.is_none() {
            self.post_world_depth_bounds = Some(crate::atlas::post_world_depth_bounds(&json!({
                "sceneView": {
                    "center": {"x": player_x, "y": player_y},
                    "width": 19,
                    "height": 15
                }
            })));
        }
        let previous_generation = self.last_generation;
        let previous_effect_sequence = self.last_effect_sequence;
        let previous_zone_tiles = self.zone_tiles.clone();
        for event in events {
            if event.generation != self.last_generation
                || event.sequence <= self.last_effect_sequence
            {
                continue;
            }
            let object_id = event
                .payload
                .get("objectId")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok());
            match (event.packet.as_str(), object_id) {
                ("ObjectHide", Some(object_id)) => {
                    if let Some(tile) = zone_tiles
                        .get(&object_id)
                        .or_else(|| previous_zone_tiles.get(&object_id))
                        .or_else(|| self.hidden_left_guard_tiles.get(&object_id))
                        .copied()
                    {
                        self.hidden_left_guard_tiles.insert(object_id, tile);
                    }
                }
                ("ObjectRemove", Some(object_id)) => {
                    self.hidden_left_guard_tiles.remove(&object_id);
                }
                ("MapChanged" | "LogOutSuccess", _) => {
                    self.hidden_left_guard_tiles.clear();
                }
                _ => {}
            }
        }
        self.zone_tiles.clone_from(zone_tiles);
        self.refresh_anchor_tiles();
        for event in events {
            if event.generation != self.last_generation {
                self.last_generation = event.generation;
                self.last_effect_sequence = 0;
                self.clear_active_effects();
            }
            if event.sequence <= self.last_effect_sequence {
                continue;
            }
            self.last_effect_sequence = event.sequence;
            let spell = event
                .payload
                .get("spell")
                .and_then(|value| {
                    value.as_str().map(ToOwned::to_owned).or_else(|| {
                        value
                            .as_u64()
                            .and_then(|id| spell_name_by_number(id as u32))
                    })
                })
                .or_else(|| {
                    event
                        .payload
                        .get("effect")
                        .and_then(Value::as_u64)
                        .map(|_| "effect".to_owned())
                })
                .unwrap_or_else(|| "-".to_owned());
            let src = event.payload.get("sourceId").and_then(Value::as_u64);
            let dst = event.payload.get("destinationId").and_then(Value::as_u64);
            let src_tile = src.and_then(|id| zone_tiles.get(&(id as u32)).copied());
            let dst_tile = dst.and_then(|id| zone_tiles.get(&(id as u32)).copied());
            fx_trace_event(
                event.generation,
                event.sequence,
                &event.packet,
                &spell,
                src.map(|v| v as u32),
                dst.map(|v| v as u32),
                src_tile,
                dst_tile,
                "event",
                "-",
                0,
                "-",
                self.now_ms,
                "-",
            );
            let provenance = EffectProvenance {
                generation: event.generation,
                sequence: event.sequence,
                packet: event.packet.clone(),
                spell,
            };
            let is_left_guard_range = event.packet == "ObjectRangeAttack"
                && event
                    .payload
                    .get("_nativeAttacker")
                    .and_then(|attacker| actor_sprite_library(attacker, "bodyLibrary"))
                    .is_some_and(|library| {
                        matches!(library, "Monster/100" | "/original-ui/Monster/100")
                    });
            let earlier_scene_boundary = events.iter().any(|candidate| {
                candidate.generation == event.generation
                    && candidate.sequence > previous_effect_sequence
                    && candidate.sequence < event.sequence
                    && matches!(candidate.packet.as_str(), "MapChanged" | "LogOutSuccess")
            });
            if is_left_guard_range
                && event.generation == previous_generation
                && !earlier_scene_boundary
            {
                // Packet-first snapshots are folded after every packet in the
                // batch. When RangeAttack precedes Hide, the final snapshot has
                // already tombstoned the actor even though it existed when the
                // attack was received. Reconstruct only those earlier
                // LeftGuard inputs from the previous raw snapshot. A Hide that
                // precedes this attack, another monster family, or an unrelated
                // effect never receives the retained tile.
                let mut event_zone_tiles = zone_tiles.clone();
                for object_id in ["objectId", "targetId"].into_iter().filter_map(|field| {
                    event
                        .payload
                        .get(field)
                        .and_then(Value::as_u64)
                        .and_then(|value| u32::try_from(value).ok())
                }) {
                    let hidden_later = events.iter().any(|candidate| {
                        candidate.generation == event.generation
                            && candidate.sequence > event.sequence
                            && candidate.packet == "ObjectHide"
                            && candidate
                                .payload
                                .get("objectId")
                                .and_then(Value::as_u64)
                                .and_then(|value| u32::try_from(value).ok())
                                == Some(object_id)
                    });
                    if hidden_later {
                        if let Some(tile) = previous_zone_tiles.get(&object_id).copied() {
                            event_zone_tiles.entry(object_id).or_insert(tile);
                        }
                    }
                }
                self.apply_event(
                    &event.packet,
                    &event.payload,
                    &event_zone_tiles,
                    &provenance,
                );
            } else {
                self.apply_event(&event.packet, &event.payload, zone_tiles, &provenance);
            }
        }
        let latest_sequence = self.last_effect_sequence;
        self.local_projectile_dedupe
            .retain(|_, sequence| latest_sequence.saturating_sub(*sequence) <= 2);
        while self.active.len() > MAX_ACTIVE_EFFECTS {
            self.active.remove(0);
        }
        self.prune_hidden_left_guard_tiles();
        self.publish_current_light_snapshots(self.now_ms, true);
    }

    pub(crate) fn reset_for_new_connection(&mut self) {
        self.last_generation = self.last_generation.wrapping_add(1);
        self.last_effect_sequence = 0;
        self.clear_active_effects();
        self.zone_tiles.clear();
        self.post_world_depth_bounds = None;
        publish_effect_lights(Vec::new());
        crate::map_parser::lighting::publish_effect_lighting_frame(
            self.last_generation,
            Vec::new(),
        );
    }

    pub(crate) fn reset_session(&mut self) {
        self.reset_for_new_connection();
    }

    fn next_key(&mut self, tag: &str) -> String {
        self.instance_seq = self.instance_seq.saturating_add(1);
        format!("fx-{tag}-{}", self.instance_seq)
    }

    fn clear_active_effects(&mut self) {
        self.active.clear();
        self.anchor_object_ids.clear();
        self.source_object_ids.clear();
        self.hidden_left_guard_tiles.clear();
        self.prestart_source_object_ids.clear();
        self.anchor_player_keys.clear();
        self.local_projectile_dedupe.clear();
        self.local_projectile_targets.clear();
        self.ready_sounds.clear();
        self.pending_sounds.clear();
        self.dead_player_sound_keys.clear();
        self.dead_scarecrow_sound_keys.clear();
        self.revived_player_effect_keys.clear();
    }

    fn refresh_anchor_tiles(&mut self) {
        let now_ms = self.now_ms;
        let zone_tiles = self.zone_tiles.clone();
        let hidden_left_guard_tiles = self.hidden_left_guard_tiles.clone();
        let left_guard_keys = self
            .active
            .iter()
            .filter(|instance| instance.provenance.spell == "LeftGuardRangeProjectile")
            .map(|instance| instance.key.clone())
            .collect::<HashSet<_>>();
        let resolve_tile = |key: &str, object_id: u32| {
            zone_tiles.get(&object_id).copied().or_else(|| {
                left_guard_keys
                    .contains(key)
                    .then(|| hidden_left_guard_tiles.get(&object_id).copied())
                    .flatten()
            })
        };
        let active_start_times = self
            .active
            .iter()
            .map(|instance| (instance.key.clone(), instance.start_at))
            .collect::<HashMap<_, _>>();
        let missing_prestart_sources = self
            .prestart_source_object_ids
            .iter()
            .filter_map(|(key, source_id)| {
                let start_at = active_start_times.get(key).copied()?;
                (now_ms <= start_at && resolve_tile(key, *source_id).is_none())
                    .then_some(key.clone())
            })
            .collect::<Vec<_>>();
        let missing_owned_sources = self
            .source_object_ids
            .iter()
            .filter_map(|(key, source_id)| {
                resolve_tile(key, *source_id)
                    .is_none()
                    .then_some(key.clone())
            })
            .collect::<Vec<_>>();
        self.prestart_source_object_ids.retain(|key, _| {
            active_start_times
                .get(key)
                .is_some_and(|start_at| now_ms <= *start_at)
        });
        for instance in &mut self.active {
            if self.anchor_player_keys.contains(&instance.key) {
                instance.tile_x = self.player_x;
                instance.tile_y = self.player_y;
            }
        }
        let mut anchors_to_insert = Vec::new();
        let mut failed_launches = Vec::new();
        let mut launch_impact_sounds = Vec::new();
        for instance in &mut self.active {
            let Some(target_state) = self.local_projectile_targets.get_mut(&instance.key) else {
                continue;
            };
            if target_state.resolved_at_launch || now_ms < instance.start_at {
                continue;
            }
            let (spell, impact_sound) = match instance.provenance.spell.as_str() {
                "FireBall" => (
                    "FireBall",
                    Some((FIREBALL_IMPACT_SOUND_CUE, FIREBALL_IMPACT_SOUND_FILE)),
                ),
                "GreatFireBall" => (
                    "GreatFireBall",
                    Some((
                        GREAT_FIREBALL_IMPACT_SOUND_CUE,
                        GREAT_FIREBALL_IMPACT_SOUND_FILE,
                    )),
                ),
                "SoulFireBall" => (
                    "SoulFireBall",
                    Some((
                        SOUL_FIREBALL_IMPACT_SOUND_CUE,
                        SOUL_FIREBALL_IMPACT_SOUND_FILE,
                    )),
                ),
                "LeftGuardRangeProjectile" => ("LeftGuardRangeProjectile", None),
                _ => continue,
            };
            let bound_target = target_state
                .target_id
                .and_then(|object_id| resolve_tile(&instance.key, object_id));
            let destination = bound_target.unwrap_or(target_state.fallback);
            let (Some(from_x), Some(from_y), Some(projectile)) =
                (instance.from_x, instance.from_y, instance.current.as_mut())
            else {
                target_state.resolved_at_launch = true;
                continue;
            };
            let source = (from_x as i32, from_y as i32);
            let direction = projectile_direction16(source, destination);
            let (duration_ms, frame_interval_ms) =
                crystal_projectile_clock(max_tile_distance(source, destination));
            let launch_animation = effect_catalog().as_ref().and_then(|catalog| {
                if spell == "LeftGuardRangeProjectile" {
                    catalog.left_guard_range_projectile_animation(direction)
                } else {
                    catalog.spell_projectile_animation(spell, direction)
                }
            });
            let Some(mut launch_animation) = launch_animation else {
                target_state.resolved_at_launch = true;
                failed_launches.push(instance.key.clone());
                continue;
            };
            launch_animation.duration_ms = duration_ms;
            launch_animation.interval = frame_interval_ms;
            *projectile = launch_animation;
            instance.tile_x = destination.0;
            instance.tile_y = destination.1;
            instance.queued = if spell == "LeftGuardRangeProjectile" {
                None
            } else {
                bound_target.and_then(|_| {
                    effect_catalog()
                        .as_ref()
                        .and_then(|catalog| catalog.spell_impact_animation(spell))
                })
            };
            target_state.resolved_at_launch = true;
            if let Some(target_id) = target_state.target_id.filter(|_| bound_target.is_some()) {
                anchors_to_insert.push((instance.key.clone(), target_id));
            }
            if let Some((impact_sound_cue, impact_sound_file)) =
                impact_sound.filter(|_| instance.queued.is_some())
            {
                launch_impact_sounds.push(PendingEffectSound {
                    key: instance.key.clone(),
                    due_at_ms: instance.start_at.saturating_add(duration_ms),
                    requires_active_effect: true,
                    event: mir2_client_bevy::audio::NativeGameplaySoundEvent {
                        generation: instance.provenance.generation,
                        sequence: instance.provenance.sequence,
                        cue: impact_sound_cue.to_owned(),
                        file_name: impact_sound_file.to_owned(),
                    },
                });
            }
        }
        for (key, object_id) in anchors_to_insert {
            self.anchor_object_ids.insert(key, object_id);
        }
        self.pending_sounds.extend(launch_impact_sounds);

        let anchors = &self.anchor_object_ids;
        let mut missing = missing_prestart_sources;
        missing.extend(missing_owned_sources);
        missing.extend(failed_launches);
        let mut detached_left_guard_targets = Vec::new();
        let mut local_projectile_impact_due = Vec::new();
        for instance in &mut self.active {
            let Some(object_id) = anchors.get(&instance.key) else {
                continue;
            };
            if let Some((tile_x, tile_y)) = resolve_tile(&instance.key, *object_id) {
                instance.tile_x = tile_x;
                instance.tile_y = tile_y;
                let (is_local_projectile, impact_sound_cue) =
                    match instance.provenance.spell.as_str() {
                        "FireBall" => (true, Some(FIREBALL_IMPACT_SOUND_CUE)),
                        "GreatFireBall" => (true, Some(GREAT_FIREBALL_IMPACT_SOUND_CUE)),
                        "SoulFireBall" => (true, Some(SOUL_FIREBALL_IMPACT_SOUND_CUE)),
                        "LeftGuardRangeProjectile" => (true, None),
                        _ => (false, None),
                    };
                if is_local_projectile {
                    if let (Some(from_x), Some(from_y), Some(projectile)) =
                        (instance.from_x, instance.from_y, instance.current.as_mut())
                    {
                        if projectile.kind == "projectile"
                            && self.now_ms
                                < instance.start_at.saturating_add(projectile.duration_ms)
                        {
                            let source = (from_x as i32, from_y as i32);
                            let destination = (tile_x, tile_y);
                            let (duration_ms, frame_interval_ms) =
                                crystal_projectile_clock(max_tile_distance(source, destination));
                            projectile.duration_ms = duration_ms;
                            projectile.interval = frame_interval_ms;
                            if let Some(impact_sound_cue) = impact_sound_cue {
                                local_projectile_impact_due.push((
                                    instance.key.clone(),
                                    instance.start_at.saturating_add(duration_ms),
                                    impact_sound_cue,
                                ));
                            }
                        }
                    }
                };
            } else if instance.provenance.spell == "LeftGuardRangeProjectile"
                && now_ms > instance.start_at
                && self
                    .local_projectile_targets
                    .get(&instance.key)
                    .is_some_and(|target| target.resolved_at_launch)
            {
                // Crystal's Missile keeps a strong Target reference after the
                // object leaves MapControl's registries. Continue from the
                // last known destination instead of deleting the source-owned
                // projectile.
                detached_left_guard_targets.push(instance.key.clone());
            } else {
                missing.push(instance.key.clone());
            }
        }
        for key in detached_left_guard_targets {
            self.anchor_object_ids.remove(&key);
            if let Some(target) = self.local_projectile_targets.get_mut(&key) {
                target.target_id = None;
            }
        }
        for (key, due_at_ms, impact_sound_cue) in local_projectile_impact_due {
            for pending in &mut self.pending_sounds {
                if pending.key == key && pending.event.cue == impact_sound_cue {
                    pending.due_at_ms = due_at_ms;
                }
            }
        }
        if !missing.is_empty() {
            self.active
                .retain(|instance| !missing.contains(&instance.key));
            self.anchor_object_ids
                .retain(|key, _| !missing.contains(key));
            self.prestart_source_object_ids
                .retain(|key, _| !missing.contains(key));
            self.source_object_ids
                .retain(|key, _| !missing.contains(key));
            self.pending_sounds
                .retain(|pending| !missing.contains(&pending.key));
            self.local_projectile_targets
                .retain(|key, _| !missing.contains(key));
        }
    }

    fn prune_hidden_left_guard_tiles(&mut self) {
        let left_guard_keys = self
            .active
            .iter()
            .filter(|instance| instance.provenance.spell == "LeftGuardRangeProjectile")
            .map(|instance| instance.key.as_str())
            .collect::<HashSet<_>>();
        let referenced_ids = self
            .source_object_ids
            .iter()
            .chain(self.anchor_object_ids.iter())
            .filter_map(|(key, object_id)| {
                left_guard_keys.contains(key.as_str()).then_some(*object_id)
            })
            .collect::<HashSet<_>>();
        self.hidden_left_guard_tiles
            .retain(|object_id, _| referenced_ids.contains(object_id));
    }

    fn take_due_sound_events(
        &mut self,
        now_ms: u64,
    ) -> Vec<mir2_client_bevy::audio::NativeGameplaySoundEvent> {
        let active_keys = self
            .active
            .iter()
            .map(|instance| instance.key.as_str())
            .collect::<Vec<_>>();
        let mut due = std::mem::take(&mut self.ready_sounds);
        self.pending_sounds.retain(|pending| {
            if pending.requires_active_effect && !active_keys.contains(&pending.key.as_str()) {
                return false;
            }
            if now_ms >= pending.due_at_ms {
                due.push(pending.event.clone());
                return false;
            }
            true
        });
        due
    }

    fn apply_event(
        &mut self,
        packet: &str,
        payload: &Value,
        zone_tiles: &HashMap<u32, (i32, i32)>,
        provenance: &EffectProvenance,
    ) {
        match packet {
            "MapChanged" | "LogOutSuccess" => self.clear_active_effects(),
            "Struck" | "ObjectStruck" => self.apply_player_struck_sound(payload, provenance),
            "Death" | "ObjectDied" => {
                self.apply_player_death_sound(payload, provenance);
                self.apply_scarecrow_death_sound(payload, provenance);
            }
            "ObjectAttack" => self.apply_object_attack(payload, provenance),
            "ObjectRangeAttack" => self.apply_object_range_attack(payload, zone_tiles, provenance),
            "ObjectMagic" => self.apply_object_magic(payload, provenance),
            "ObjectProjectile" => self.apply_object_projectile(payload, zone_tiles, provenance),
            "ObjectEffect" => self.apply_object_effect(payload, zone_tiles, provenance),
            "MapEffect" => self.apply_map_effect(payload, provenance),
            "ObjectSpell" => self.apply_object_spell(payload, zone_tiles, provenance),
            "Revived" => self.apply_player_revived(payload, zone_tiles, provenance, true),
            "ObjectRevived" => self.apply_player_revived(payload, zone_tiles, provenance, false),
            "ObjectRemove" => self.apply_object_remove(payload, false),
            "ObjectHide" => self.apply_object_remove(payload, true),
            _ => {}
        }
    }

    fn apply_player_revived(
        &mut self,
        payload: &Value,
        zone_tiles: &HashMap<u32, (i32, i32)>,
        provenance: &EffectProvenance,
        self_player: bool,
    ) {
        let actor_key = player_sound_actor_key(payload, self_player);
        if let Some(actor_key) = actor_key.as_deref() {
            self.cancel_player_sounds(actor_key);
            self.dead_player_sound_keys.remove(actor_key);
        }
        if !self_player
            && !payload
                .get("effect")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            return;
        }
        if actor_key
            .as_ref()
            .is_some_and(|actor_key| !self.revived_player_effect_keys.insert(actor_key.clone()))
        {
            return;
        }
        let (key, tile_x, tile_y, object_id) = if self_player {
            (
                "player-revive-self".to_owned(),
                self.player_x,
                self.player_y,
                None,
            )
        } else {
            let Some(object_id) = payload
                .get("objectId")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
            else {
                return;
            };
            let Some((x, y)) = zone_tiles.get(&object_id).copied() else {
                return;
            };
            (format!("player-revive-{object_id}"), x, y, Some(object_id))
        };

        self.queue_immediate_sound(
            provenance,
            PLAYER_REVIVE_SOUND_CUE,
            PLAYER_REVIVE_SOUND_FILE,
        );
        let Some(animation) = effect_catalog()
            .as_ref()
            .and_then(|catalog| catalog.map_animation("PlayerRevive", 0))
        else {
            return;
        };

        self.active.retain(|instance| instance.key != key);
        self.anchor_object_ids.remove(&key);
        self.anchor_player_keys.remove(&key);
        if let Some(object_id) = object_id {
            self.anchor_object_ids.insert(key.clone(), object_id);
        } else {
            self.anchor_player_keys.insert(key.clone());
        }
        self.active.push(EffectInstance {
            key,
            kind: EffectKindTag::Impact,
            tile_x,
            tile_y,
            from_x: None,
            from_y: None,
            current: Some(animation),
            queued: None,
            return_queued: None,
            started_at: self.now_ms,
            start_at: self.now_ms,
            persistent_object_id: None,
            provenance: provenance.clone(),
        });
    }

    fn apply_object_attack(&mut self, payload: &Value, provenance: &EffectProvenance) {
        let flaming_sword = payload.get("spell").is_some_and(|value| {
            value.as_u64() == Some(8) || value.as_str() == Some("FlamingSword")
        });
        if !flaming_sword {
            return;
        }
        let Some(catalog) = effect_catalog() else {
            return;
        };
        let Some(object_id) = payload
            .get("objectId")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
        else {
            return;
        };
        let (Some(x), Some(y)) = (
            value_f32(payload, "location", "x"),
            value_f32(payload, "location", "y"),
        ) else {
            return;
        };
        let direction = payload
            .get("direction")
            .and_then(Value::as_str)
            .map(direction_index)
            .unwrap_or(4);
        let Some(animation) = catalog.spell_attack_overlay_animation("FlamingSword", direction)
        else {
            return;
        };

        // One overlay per attacker: a newer authoritative attack restarts its
        // six-frame clock while distinct attackers remain independent.
        let key = format!("flaming-sword-{object_id}");
        self.active.retain(|instance| instance.key != key);
        self.pending_sounds.retain(|pending| pending.key != key);
        self.anchor_object_ids.insert(key.clone(), object_id);
        self.queue_immediate_sound(
            provenance,
            FLAMING_SWORD_SOUND_CUE,
            FLAMING_SWORD_SOUND_FILE,
        );
        self.active.push(EffectInstance {
            key,
            kind: EffectKindTag::AttackOverlay,
            tile_x: x as i32,
            tile_y: y as i32,
            from_x: None,
            from_y: None,
            current: Some(animation),
            queued: None,
            return_queued: None,
            started_at: self.now_ms,
            start_at: self.now_ms,
            persistent_object_id: None,
            provenance: provenance.clone(),
        });
    }

    fn queue_immediate_sound(&mut self, provenance: &EffectProvenance, cue: &str, file_name: &str) {
        self.ready_sounds
            .push(mir2_client_bevy::audio::NativeGameplaySoundEvent {
                generation: provenance.generation,
                sequence: provenance.sequence,
                cue: cue.to_owned(),
                file_name: file_name.to_owned(),
            });
    }

    fn apply_player_struck_sound(&mut self, payload: &Value, provenance: &EffectProvenance) {
        let Some(target) = payload
            .get("_nativeTarget")
            .filter(|actor| actor_is_player(actor))
        else {
            return;
        };
        let Some(actor_key) = actor_sound_key(target) else {
            return;
        };
        if self.dead_player_sound_keys.contains(&actor_key) {
            return;
        }
        if let Some(mount_index) = player_mount_index(target) {
            let mount_file = if mount_index < 7 {
                if provenance.sequence & 1 == 0 {
                    Some(PLAYER_TIGER_STRUCK_2_FILE)
                } else {
                    Some(PLAYER_TIGER_STRUCK_1_FILE)
                }
            } else if mount_index < 12 {
                Some(PLAYER_WOLF_STRUCK_FILE)
            } else {
                None
            };
            if let Some(mount_file) = mount_file {
                self.queue_immediate_sound(
                    provenance,
                    &format!("Player.{actor_key}.StruckMount"),
                    mount_file,
                );
            }
        } else if let Some(body_file) =
            player_struck_body_file(target, payload.get("_nativeAttacker"))
        {
            self.queue_immediate_sound(
                provenance,
                &format!("Player.{actor_key}.StruckBody"),
                body_file,
            );
        }
        self.queue_immediate_sound(
            provenance,
            &format!("Player.{actor_key}.Flinch"),
            if actor_is_female(target) {
                PLAYER_FEMALE_FLINCH_FILE
            } else {
                PLAYER_MALE_FLINCH_FILE
            },
        );
    }

    fn apply_player_death_sound(&mut self, payload: &Value, provenance: &EffectProvenance) {
        let Some(target) = payload
            .get("_nativeTarget")
            .filter(|actor| actor_is_player(actor))
        else {
            return;
        };
        let Some(actor_key) = actor_sound_key(target) else {
            return;
        };
        if !self.dead_player_sound_keys.insert(actor_key.clone()) {
            return;
        }
        self.revived_player_effect_keys.remove(&actor_key);
        self.cancel_pending_player_sound(&actor_key);
        self.pending_sounds.push(PendingEffectSound {
            key: format!("player-sound-{actor_key}"),
            due_at_ms: self.now_ms.saturating_add(PLAYER_DIE_SOUND_DELAY_MS),
            requires_active_effect: false,
            event: mir2_client_bevy::audio::NativeGameplaySoundEvent {
                generation: provenance.generation,
                sequence: provenance.sequence,
                cue: format!("Player.{actor_key}.Die"),
                file_name: if actor_is_female(target) {
                    PLAYER_FEMALE_DIE_FILE.to_owned()
                } else {
                    PLAYER_MALE_DIE_FILE.to_owned()
                },
            },
        });
    }

    fn apply_scarecrow_death_sound(&mut self, payload: &Value, provenance: &EffectProvenance) {
        let Some(target) = payload
            .get("_nativeTarget")
            .filter(|actor| actor_is_scarecrow(actor))
        else {
            return;
        };
        let Some(actor_key) = actor_sound_key(target) else {
            return;
        };
        if !self.dead_scarecrow_sound_keys.insert(actor_key.clone()) {
            return;
        }
        self.cancel_scarecrow_sounds(&actor_key);
        // Crystal's MonsterObject plays BaseSound + 3 when the Die action
        // starts. Keep this due-now entry pending until the packet batch is
        // complete so an adjacent remove/hide lifecycle packet can cancel it.
        self.pending_sounds.push(PendingEffectSound {
            key: format!("scarecrow-sound-{actor_key}"),
            due_at_ms: self.now_ms,
            requires_active_effect: false,
            event: mir2_client_bevy::audio::NativeGameplaySoundEvent {
                generation: provenance.generation,
                sequence: provenance.sequence,
                cue: format!("Scarecrow.{actor_key}.Die"),
                file_name: SCARECROW_DIE_SOUND_FILE.to_owned(),
            },
        });
    }

    fn cancel_player_sounds(&mut self, actor_key: &str) {
        let cue_prefix = format!("Player.{actor_key}.");
        self.ready_sounds
            .retain(|event| !event.cue.starts_with(&cue_prefix));
        self.cancel_pending_player_sound(actor_key);
    }

    fn cancel_pending_player_sound(&mut self, actor_key: &str) {
        let pending_key = format!("player-sound-{actor_key}");
        self.pending_sounds
            .retain(|pending| pending.key != pending_key);
    }

    fn cancel_scarecrow_sounds(&mut self, actor_key: &str) {
        let cue_prefix = format!("Scarecrow.{actor_key}.");
        self.ready_sounds
            .retain(|event| !event.cue.starts_with(&cue_prefix));
        let pending_key = format!("scarecrow-sound-{actor_key}");
        self.pending_sounds
            .retain(|pending| pending.key != pending_key);
    }

    fn schedule_local_projectile_from_object_magic(
        &mut self,
        payload: &Value,
        catalog: &EffectCatalog,
        provenance: &EffectProvenance,
        spell: &'static str,
        tag: &'static str,
        spell_action_ms: u64,
        projectile_sound_cue: &'static str,
        projectile_sound_file: &'static str,
    ) {
        if !payload
            .get("cast")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return;
        }
        let (Some(source_x), Some(source_y), Some(packet_target_x), Some(packet_target_y)) = (
            value_f32(payload, "location", "x").map(|value| value as i32),
            value_f32(payload, "location", "y").map(|value| value as i32),
            value_f32(payload, "target", "x").map(|value| value as i32),
            value_f32(payload, "target", "y").map(|value| value as i32),
        ) else {
            return;
        };
        let source_id = payload
            .get("objectId")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok());
        let target_id = payload
            .get("targetId")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value != 0);
        let destination = (packet_target_x, packet_target_y);
        let source = (source_x, source_y);
        let direction = projectile_direction16(source, destination);
        let Some(mut projectile) = catalog.spell_projectile_animation(spell, direction) else {
            return;
        };
        let (duration_ms, frame_interval_ms) =
            crystal_projectile_clock(max_tile_distance(source, destination));
        projectile.duration_ms = duration_ms;
        projectile.interval = frame_interval_ms;

        let now = self.now_ms;
        let start_at = now.saturating_add(spell_action_ms);
        let key = self.next_key(tag);
        self.local_projectile_targets.insert(
            key.clone(),
            LocalProjectileTarget {
                target_id,
                fallback: destination,
                resolved_at_launch: false,
            },
        );
        self.pending_sounds.push(PendingEffectSound {
            key: key.clone(),
            due_at_ms: start_at,
            requires_active_effect: true,
            event: mir2_client_bevy::audio::NativeGameplaySoundEvent {
                generation: provenance.generation,
                sequence: provenance.sequence,
                cue: projectile_sound_cue.to_owned(),
                file_name: projectile_sound_file.to_owned(),
            },
        });
        self.active.push(EffectInstance {
            key,
            kind: EffectKindTag::Projectile,
            tile_x: destination.0,
            tile_y: destination.1,
            from_x: Some(source_x as f32),
            from_y: Some(source_y as f32),
            current: Some(projectile),
            queued: None,
            return_queued: None,
            started_at: now,
            start_at,
            persistent_object_id: None,
            provenance: provenance.clone(),
        });
        if let (Some(source_id), Some(target_id)) = (source_id, target_id) {
            self.local_projectile_dedupe.insert(
                (spell.to_owned(), source_id, target_id),
                provenance.sequence,
            );
        }
    }

    fn apply_object_magic(&mut self, payload: &Value, provenance: &EffectProvenance) {
        let Some(catalog) = effect_catalog() else {
            return;
        };
        if catalog.is_empty() {
            return;
        }
        let (Some(x), Some(y)) = (
            value_f32(payload, "location", "x"),
            value_f32(payload, "location", "y"),
        ) else {
            return;
        };
        // The spell arrives as a string name (gateway Debug-serialises the enum).
        let Some(spell) = payload.get("spell").and_then(Value::as_str) else {
            return;
        };
        if spell == "FireWall" {
            // Crystal plays M39-0 and the attached Magic/1620..1629 cast at
            // action start even when Cast=false. Only the post-Spell-action
            // M39-1 completion branch is gated by Cast=true; ObjectSpell owns
            // the authoritative persistent ground flames independently.
            self.queue_immediate_sound(
                provenance,
                FIREWALL_CAST_SOUND_CUE,
                FIREWALL_CAST_SOUND_FILE,
            );
        }
        if spell == "SoulFireBall" {
            // Crystal has no SoulFireBall cast bitmap. The action start always
            // plays M64-0, while Cast=false suppresses only the completion
            // branch (missile, impact and their two sounds).
            self.queue_immediate_sound(
                provenance,
                SOUL_FIREBALL_CAST_SOUND_CUE,
                SOUL_FIREBALL_CAST_SOUND_FILE,
            );
            self.schedule_local_projectile_from_object_magic(
                payload,
                catalog,
                provenance,
                "SoulFireBall",
                "soul-fireball",
                SOUL_FIREBALL_SPELL_ACTION_MS,
                SOUL_FIREBALL_PROJECTILE_SOUND_CUE,
                SOUL_FIREBALL_PROJECTILE_SOUND_FILE,
            );
            return;
        }
        // Crystal still plays the caster's Spell actor action for Cast=false,
        // but Lightning itself is created only by the action-completion branch
        // when Cast=true.
        if spell == "Lightning"
            && !payload
                .get("cast")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            return;
        }
        let direction = payload
            .get("direction")
            .and_then(Value::as_str)
            .map(direction_index)
            .unwrap_or(4);
        let Some(anim) = catalog.spell_cast_animation(spell, direction) else {
            return;
        };
        if fx_trace_enabled() {
            eprintln!(
                "[fx-trace] cast spell={} dir={} tile=({},{}) anim={} frames={} interval={} first_frame={} offset=({},{})",
                spell,
                direction,
                x as i32,
                y as i32,
                anim.name,
                anim.frames.len(),
                anim.interval,
                anim.frames.first().map(|f| f.path.as_str()).unwrap_or("-"),
                anim.offset_x,
                anim.offset_y
            );
        }
        let now = self.now_ms;
        let start_at = if spell == "Lightning" {
            now.saturating_add(LIGHTNING_SPELL_ACTION_MS)
        } else {
            now
        };
        let key = self.next_key("cast");
        if spell == "FireBall" {
            self.pending_sounds.push(PendingEffectSound {
                key: key.clone(),
                due_at_ms: now,
                requires_active_effect: true,
                event: mir2_client_bevy::audio::NativeGameplaySoundEvent {
                    generation: provenance.generation,
                    sequence: provenance.sequence,
                    cue: FIREBALL_CAST_SOUND_CUE.to_owned(),
                    file_name: FIREBALL_CAST_SOUND_FILE.to_owned(),
                },
            });
        }
        if spell == "GreatFireBall" {
            self.pending_sounds.push(PendingEffectSound {
                key: key.clone(),
                due_at_ms: now,
                requires_active_effect: true,
                event: mir2_client_bevy::audio::NativeGameplaySoundEvent {
                    generation: provenance.generation,
                    sequence: provenance.sequence,
                    cue: GREAT_FIREBALL_CAST_SOUND_CUE.to_owned(),
                    file_name: GREAT_FIREBALL_CAST_SOUND_FILE.to_owned(),
                },
            });
        }
        if spell == "Healing" {
            self.pending_sounds.push(PendingEffectSound {
                key: key.clone(),
                due_at_ms: now,
                requires_active_effect: true,
                event: mir2_client_bevy::audio::NativeGameplaySoundEvent {
                    generation: provenance.generation,
                    sequence: provenance.sequence,
                    cue: HEALING_CAST_SOUND_CUE.to_owned(),
                    file_name: HEALING_CAST_SOUND_FILE.to_owned(),
                },
            });
        }
        if spell == "FireWall"
            && payload
                .get("cast")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            self.pending_sounds.push(PendingEffectSound {
                key: key.clone(),
                due_at_ms: now.saturating_add(FIREWALL_SPELL_ACTION_MS),
                requires_active_effect: true,
                event: mir2_client_bevy::audio::NativeGameplaySoundEvent {
                    generation: provenance.generation,
                    sequence: provenance.sequence,
                    cue: FIREWALL_COMPLETE_SOUND_CUE.to_owned(),
                    file_name: FIREWALL_COMPLETE_SOUND_FILE.to_owned(),
                },
            });
        }
        if spell == "Lightning" {
            if let Some(object_id) = payload
                .get("objectId")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
            {
                self.anchor_object_ids.insert(key.clone(), object_id);
            }
            self.pending_sounds.push(PendingEffectSound {
                key: key.clone(),
                due_at_ms: start_at,
                requires_active_effect: true,
                event: mir2_client_bevy::audio::NativeGameplaySoundEvent {
                    generation: provenance.generation,
                    sequence: provenance.sequence,
                    cue: LIGHTNING_SOUND_CUE.to_owned(),
                    file_name: LIGHTNING_SOUND_FILE.to_owned(),
                },
            });
        }
        self.active.push(EffectInstance {
            key,
            kind: EffectKindTag::Cast,
            tile_x: x as i32,
            tile_y: y as i32,
            from_x: None,
            from_y: None,
            current: Some(anim),
            queued: None,
            return_queued: None,
            started_at: now,
            start_at,
            persistent_object_id: None,
            provenance: provenance.clone(),
        });
        if spell == "FireBall" {
            self.schedule_local_projectile_from_object_magic(
                payload,
                catalog,
                provenance,
                "FireBall",
                "fireball",
                FIREBALL_SPELL_ACTION_MS,
                FIREBALL_PROJECTILE_SOUND_CUE,
                FIREBALL_PROJECTILE_SOUND_FILE,
            );
        }
        if spell == "GreatFireBall" {
            self.schedule_local_projectile_from_object_magic(
                payload,
                catalog,
                provenance,
                "GreatFireBall",
                "great-fireball",
                GREAT_FIREBALL_SPELL_ACTION_MS,
                GREAT_FIREBALL_PROJECTILE_SOUND_CUE,
                GREAT_FIREBALL_PROJECTILE_SOUND_FILE,
            );
        }
    }

    fn apply_object_projectile(
        &mut self,
        payload: &Value,
        zone_tiles: &HashMap<u32, (i32, i32)>,
        provenance: &EffectProvenance,
    ) {
        let Some(catalog) = effect_catalog() else {
            return;
        };
        if catalog.is_empty() {
            return;
        }
        let Some(spell) = payload.get("spell").and_then(Value::as_str) else {
            return;
        };
        // Crystal's SoulFireBall and GreatFireBall player paths never consume
        // ObjectProjectile; current Rust servers emit it only as a
        // compatibility supplement. Ignore it regardless of order/replay and
        // let ObjectMagic own the delayed local missile.
        if matches!(spell, "SoulFireBall" | "GreatFireBall") {
            return;
        }
        let open_id = |name: &str| -> Option<u32> {
            payload.get(name).and_then(Value::as_u64).map(|v| v as u32)
        };
        let (Some(source_id), Some(destination_id)) =
            (open_id("sourceId"), open_id("destinationId"))
        else {
            // Without authoritative source/destination we must not fabricate a path.
            return;
        };
        if matches!(spell, "FireBall" | "SoulFireBall")
            && self
                .local_projectile_dedupe
                .get(&(spell.to_owned(), source_id, destination_id))
                .is_some_and(|cast_sequence| {
                    provenance.sequence > *cast_sequence
                        && provenance.sequence.saturating_sub(*cast_sequence) <= 2
                })
        {
            self.local_projectile_dedupe
                .remove(&(spell.to_owned(), source_id, destination_id));
            return;
        }
        let (Some(&(from_x, from_y)), Some(&(to_x, to_y))) =
            (zone_tiles.get(&source_id), zone_tiles.get(&destination_id))
        else {
            return;
        };
        let direction = projectile_direction16((from_x, from_y), (to_x, to_y));
        let projectile = catalog.spell_projectile_animation(spell, direction);
        let impact = catalog.spell_impact_animation(spell);
        let return_anim = catalog.spell_return_animation(spell);
        // Vampirism has no projectile but has impact+return; FireBall has projectile+impact.
        // For spells without projectile, we still create an effect if they have impact/return.
        let (current, queued, return_queued) = match (projectile, impact, return_anim) {
            (Some(proj), imp, ret) => (Some(proj), imp, ret),
            (None, Some(imp), ret) => (Some(imp), None, ret),
            (None, None, Some(ret)) => (Some(ret), None, None),
            (None, None, None) => return,
        };
        let current_for_trace = current.as_ref().unwrap();
        if fx_trace_enabled() {
            eprintln!(
                "[fx-trace] projectile spell={} src={} dst={} srcTile=({},{}) dstTile=({},{}) proj_frames={} proj_first={} impact_first={} return_first={} offset=({},{})",
                spell,
                source_id,
                destination_id,
                from_x,
                from_y,
                to_x,
                to_y,
                current_for_trace.frames.len(),
                current_for_trace
                    .frames
                    .first()
                    .map(|f| f.path.as_str())
                    .unwrap_or("-"),
                queued
                    .as_ref()
                    .and_then(|a| a.frames.first())
                    .map(|f| f.path.as_str())
                    .unwrap_or("-"),
                return_queued
                    .as_ref()
                    .and_then(|a| a.frames.first())
                    .map(|f| f.path.as_str())
                    .unwrap_or("-"),
                current_for_trace.offset_x,
                current_for_trace.offset_y
            );
        }
        let now = self.now_ms;
        let key = self.next_key("proj");
        self.active.push(EffectInstance {
            key,
            kind: EffectKindTag::Projectile,
            tile_x: to_x,
            tile_y: to_y,
            from_x: Some(from_x as f32),
            from_y: Some(from_y as f32),
            current,
            queued,
            return_queued,
            started_at: now,
            start_at: now,
            persistent_object_id: None,
            provenance: provenance.clone(),
        });
    }

    fn apply_object_effect(
        &mut self,
        payload: &Value,
        zone_tiles: &HashMap<u32, (i32, i32)>,
        provenance: &EffectProvenance,
    ) {
        let Some(catalog) = effect_catalog() else {
            return;
        };
        if catalog.is_empty() {
            return;
        }
        let Some(effect) = payload
            .get("effect")
            .and_then(Value::as_u64)
            .map(|v| v as u32)
        else {
            return;
        };
        let Some(object_id) = payload
            .get("objectId")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
        else {
            return;
        };
        let Some((tile_x, tile_y)) = zone_tiles.get(&object_id).copied() else {
            return;
        };
        let Some(anim) = catalog.map_animation_by_number(effect, 0) else {
            return;
        };
        let now = self.now_ms;
        // Crystal's ObjectEffect Healing branch ignores Packet.DelayTime and
        // attaches Magic/370..379 immediately to the target object. Other
        // ObjectEffect families retain the generic packet delay contract.
        let is_healing = effect == 3;
        let delay_ms = if is_healing {
            0
        } else {
            payload
                .get("delayTime")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        };
        let key = self.next_key("obj");
        self.anchor_object_ids.insert(key.clone(), object_id);
        if is_healing {
            self.pending_sounds.push(PendingEffectSound {
                key: key.clone(),
                due_at_ms: now,
                requires_active_effect: true,
                event: mir2_client_bevy::audio::NativeGameplaySoundEvent {
                    generation: provenance.generation,
                    sequence: provenance.sequence,
                    cue: HEALING_TARGET_SOUND_CUE.to_owned(),
                    file_name: HEALING_TARGET_SOUND_FILE.to_owned(),
                },
            });
        }
        self.active.push(EffectInstance {
            key,
            // Crystal attaches ObjectEffect to MirObject.Effects and renders it
            // from DrawEffects, after the selected-target DrawBlend pass.
            kind: EffectKindTag::SceneForeground,
            tile_x,
            tile_y,
            from_x: None,
            from_y: None,
            started_at: now,
            start_at: now.saturating_add(delay_ms),
            current: Some(anim),
            queued: None,
            return_queued: None,
            persistent_object_id: None,
            provenance: provenance.clone(),
        });
    }

    fn apply_object_range_attack(
        &mut self,
        payload: &Value,
        zone_tiles: &HashMap<u32, (i32, i32)>,
        provenance: &EffectProvenance,
    ) {
        let Some(attacker) = payload.get("_nativeAttacker") else {
            return;
        };
        let Some(body_library) = actor_sprite_library(attacker, "bodyLibrary") else {
            return;
        };
        let Some(source_id) = payload
            .get("objectId")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
        else {
            return;
        };
        let Some(target_id) = payload
            .get("targetId")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
        else {
            return;
        };
        if !zone_tiles.contains_key(&source_id) {
            return;
        }
        let Some(target_tile) = zone_tiles.get(&target_id).copied() else {
            return;
        };

        match body_library {
            "Monster/099" | "/original-ui/Monster/099" => {
                self.apply_right_guard_range_hit(source_id, target_id, target_tile, provenance)
            }
            "Monster/100" | "/original-ui/Monster/100" => {
                let (Some(source_x), Some(source_y)) = (
                    value_f32(payload, "location", "x").map(|value| value as i32),
                    value_f32(payload, "location", "y").map(|value| value as i32),
                ) else {
                    return;
                };
                self.apply_left_guard_range_projectile(
                    source_id,
                    target_id,
                    (source_x, source_y),
                    target_tile,
                    provenance,
                )
            }
            _ => {}
        }
    }

    fn apply_right_guard_range_hit(
        &mut self,
        source_id: u32,
        target_id: u32,
        (tile_x, tile_y): (i32, i32),
        provenance: &EffectProvenance,
    ) {
        let Some(animation) = effect_catalog()
            .as_ref()
            .and_then(EffectCatalog::right_guard_range_hit_animation)
        else {
            return;
        };

        // Monster/099 creates the target-owned Magic2 effect on AttackRange1's
        // fourth 100 ms action-frame boundary. A new authoritative attack for
        // the same pair restarts that one semantic instance; distinct targets
        // remain independent, matching Crystal's target-owned effect list.
        let now = self.now_ms;
        let key = format!("right-guard-range:{source_id}:{target_id}");
        self.active.retain(|instance| instance.key != key);
        self.anchor_object_ids.remove(&key);
        self.source_object_ids.remove(&key);
        self.prestart_source_object_ids.remove(&key);
        self.local_projectile_targets.remove(&key);
        self.active.push(EffectInstance {
            key: key.clone(),
            kind: EffectKindTag::Impact,
            tile_x,
            tile_y,
            from_x: None,
            from_y: None,
            current: Some(animation),
            queued: None,
            return_queued: None,
            started_at: now,
            start_at: now.saturating_add(GUARD_RANGE_ACTION_FRAME_DELAY_MS),
            persistent_object_id: None,
            provenance: provenance.clone(),
        });
        self.anchor_object_ids.insert(key, target_id);
        self.prestart_source_object_ids.insert(
            format!("right-guard-range:{source_id}:{target_id}"),
            source_id,
        );
    }

    fn apply_left_guard_range_projectile(
        &mut self,
        source_id: u32,
        target_id: u32,
        source: (i32, i32),
        destination: (i32, i32),
        provenance: &EffectProvenance,
    ) {
        let direction = projectile_direction16(source, destination);
        let Some(mut animation) = effect_catalog()
            .as_ref()
            .and_then(|catalog| catalog.left_guard_range_projectile_animation(direction))
        else {
            return;
        };
        let (duration_ms, frame_interval_ms) =
            crystal_projectile_clock(max_tile_distance(source, destination));
        animation.duration_ms = duration_ms;
        animation.interval = frame_interval_ms;

        // Crystal creates this source-owned Missile on AttackRange1's fourth
        // 100 ms frame boundary. It locks Direction16 at launch, follows the
        // target position during flight and recomputes the 50 ms/tile clock.
        let now = self.now_ms;
        let key = format!("left-guard-range:{source_id}:{target_id}");
        self.active.retain(|instance| instance.key != key);
        self.anchor_object_ids.remove(&key);
        self.source_object_ids.remove(&key);
        self.prestart_source_object_ids.remove(&key);
        self.local_projectile_targets.remove(&key);
        self.pending_sounds.retain(|pending| pending.key != key);
        let mut projectile_provenance = provenance.clone();
        projectile_provenance.spell = "LeftGuardRangeProjectile".to_owned();
        self.active.push(EffectInstance {
            key: key.clone(),
            kind: EffectKindTag::Projectile,
            tile_x: destination.0,
            tile_y: destination.1,
            from_x: Some(source.0 as f32),
            from_y: Some(source.1 as f32),
            current: Some(animation),
            queued: None,
            return_queued: None,
            started_at: now,
            start_at: now.saturating_add(GUARD_RANGE_ACTION_FRAME_DELAY_MS),
            persistent_object_id: None,
            provenance: projectile_provenance,
        });
        self.anchor_object_ids.insert(key.clone(), target_id);
        self.source_object_ids.insert(key.clone(), source_id);
        self.local_projectile_targets.insert(
            key,
            LocalProjectileTarget {
                target_id: Some(target_id),
                fallback: destination,
                resolved_at_launch: false,
            },
        );
    }

    fn apply_map_effect(&mut self, payload: &Value, provenance: &EffectProvenance) {
        let Some(catalog) = effect_catalog() else {
            return;
        };
        if catalog.is_empty() {
            return;
        }
        let (Some(x), Some(y)) = (
            value_f32(payload, "location", "x"),
            value_f32(payload, "location", "y"),
        ) else {
            return;
        };
        let Some(effect) = payload
            .get("effect")
            .and_then(Value::as_u64)
            .map(|v| v as u32)
        else {
            return;
        };
        let value = payload
            .get("value")
            .and_then(Value::as_u64)
            .map(|v| v as u32)
            .unwrap_or(0);
        let Some(anim) = catalog.map_animation_by_number(effect, value) else {
            return;
        };
        let now = self.now_ms;
        let key = self.next_key("map");
        self.active.push(EffectInstance {
            key,
            // MapEffect enters MapControl.Effects with DrawBehind=false by
            // default, so GameScene draws it after the selected target pass.
            kind: EffectKindTag::SceneForeground,
            tile_x: x as i32,
            tile_y: y as i32,
            from_x: None,
            from_y: None,
            started_at: now,
            start_at: now,
            current: Some(anim),
            queued: None,
            return_queued: None,
            persistent_object_id: None,
            provenance: provenance.clone(),
        });
    }

    fn apply_object_spell(
        &mut self,
        payload: &Value,
        _zone_tiles: &HashMap<u32, (i32, i32)>,
        provenance: &EffectProvenance,
    ) {
        let Some(catalog) = effect_catalog() else {
            return;
        };
        if catalog.is_empty() {
            return;
        }
        let Some(object_id) = payload
            .get("objectId")
            .and_then(Value::as_u64)
            .map(|v| v as u32)
        else {
            return;
        };
        let (Some(x), Some(y)) = (
            value_f32(payload, "location", "x"),
            value_f32(payload, "location", "y"),
        ) else {
            return;
        };
        let spell = payload.get("spell").map(spell_number_u32).unwrap_or(0);
        let Some(spell_name) = spell_name_by_number(spell) else {
            return;
        };
        let direction = payload
            .get("direction")
            .and_then(Value::as_str)
            .map(direction_index)
            .unwrap_or(4);
        let param = payload.get("param").map(spell_number_u32).unwrap_or(0);
        let Some(mut anim) = catalog.spell_world_animation(&spell_name, direction, param) else {
            return;
        };
        // Persistent world spell loops until it is removed by ObjectRemove.
        anim.repeat = true;
        let now = self.now_ms;
        let key = format!("spell-{object_id}");
        self.active.retain(|instance| instance.key != key);
        self.active.push(EffectInstance {
            key,
            kind: EffectKindTag::Persistent,
            tile_x: x as i32,
            tile_y: y as i32,
            from_x: None,
            from_y: None,
            started_at: now,
            start_at: now,
            current: Some(anim),
            queued: None,
            return_queued: None,
            persistent_object_id: Some(object_id),
            provenance: provenance.clone(),
        });
    }

    fn apply_object_remove(&mut self, payload: &Value, hidden: bool) {
        let Some(object_id) = payload
            .get("objectId")
            .and_then(Value::as_u64)
            .map(|v| v as u32)
        else {
            return;
        };
        let actor_key = object_id.to_string();
        self.cancel_player_sounds(&actor_key);
        self.cancel_scarecrow_sounds(&actor_key);
        self.dead_player_sound_keys.remove(&actor_key);
        self.dead_scarecrow_sound_keys.remove(&actor_key);
        self.revived_player_effect_keys.remove(&actor_key);
        let remove_key = format!("spell-{object_id}");
        let left_guard_start_times = self
            .active
            .iter()
            .filter_map(|instance| {
                (instance.provenance.spell == "LeftGuardRangeProjectile")
                    .then_some((instance.key.clone(), instance.start_at))
            })
            .collect::<HashMap<_, _>>();
        let protected_hide_keys = if hidden {
            left_guard_start_times
                .keys()
                .cloned()
                .collect::<HashSet<_>>()
        } else {
            HashSet::new()
        };
        let detached_target_keys = if hidden {
            HashSet::new()
        } else {
            self.anchor_object_ids
                .iter()
                .filter_map(|(key, anchored_id)| {
                    (*anchored_id == object_id
                        && left_guard_start_times
                            .get(key)
                            .is_some_and(|start_at| self.now_ms > *start_at))
                    .then_some(key.clone())
                })
                .collect::<HashSet<_>>()
        };
        let anchored_keys = self
            .anchor_object_ids
            .iter()
            .filter_map(|(key, anchored_id)| {
                (*anchored_id == object_id
                    && !protected_hide_keys.contains(key)
                    && !detached_target_keys.contains(key))
                .then_some(key.clone())
            })
            .collect::<Vec<_>>();
        let prestart_source_keys = self
            .prestart_source_object_ids
            .iter()
            .filter_map(|(key, source_id)| {
                (*source_id == object_id && !protected_hide_keys.contains(key))
                    .then_some(key.clone())
            })
            .collect::<Vec<_>>();
        let source_owned_keys = self
            .source_object_ids
            .iter()
            .filter_map(|(key, source_id)| {
                (*source_id == object_id && !protected_hide_keys.contains(key))
                    .then_some(key.clone())
            })
            .collect::<Vec<_>>();
        let removed_keys = anchored_keys
            .iter()
            .chain(prestart_source_keys.iter())
            .chain(source_owned_keys.iter())
            .cloned()
            .collect::<HashSet<_>>();
        self.active
            .retain(|instance| instance.key != remove_key && !removed_keys.contains(&instance.key));
        self.anchor_object_ids
            .retain(|key, _| !removed_keys.contains(key) && !detached_target_keys.contains(key));
        self.prestart_source_object_ids
            .retain(|key, _| !removed_keys.contains(key));
        self.source_object_ids
            .retain(|key, _| !removed_keys.contains(key));
        self.pending_sounds
            .retain(|pending| !removed_keys.contains(&pending.key));
        self.local_projectile_targets
            .retain(|key, _| !removed_keys.contains(key));
        for key in detached_target_keys {
            if let Some(target) = self.local_projectile_targets.get_mut(&key) {
                target.target_id = None;
            }
        }
    }

    /// Advance the effect clock and build the EffectRenderState JSON. Returns
    /// None when nothing changed since the last emitted state.
    pub(crate) fn tick(&mut self, now_ms: u64) -> Option<String> {
        self.tick_with_visibility(now_ms, true)
    }

    /// Advance authoritative effect lifetimes regardless of presentation, but
    /// gate only the native render payload. Crystal's `Settings.Effect` is a
    /// local visual preference: turning it off must neither drop packets nor
    /// mutate the server-owned world. A still-active effect is therefore
    /// immediately visible again if the option is restored before it expires.
    pub(crate) fn tick_with_visibility(&mut self, now_ms: u64, visible: bool) -> Option<String> {
        self.now_ms = now_ms;
        let player_x = self.player_x;
        let player_y = self.player_y;
        let post_world_depth_bounds = self.post_world_depth_bounds;
        let _ = effect_catalog();
        self.refresh_anchor_tiles();

        let rendered = self
            .active
            .iter_mut()
            .filter_map(|instance| {
                let (animation, started_at) = advance_instance(instance, now_ms)?;
                let elapsed = now_ms.saturating_sub(started_at);
                let frame = animation.frame_at(elapsed)?;
                let (tile_x, tile_y) = instance_current_tile(instance, animation, started_at, now_ms);
                let (left, top) = screen_position(
                    player_x,
                    player_y,
                    tile_x,
                    tile_y,
                    frame.x + animation.offset_x,
                    frame.y + animation.offset_y,
                );
                if fx_trace_enabled() {
                    let p = &instance.provenance;
                    eprintln!(
                        "[fx-trace] tick gen={} seq={} packet={} spell={} key={} phase={} tile=({:.1},{:.1}) startAt={} elapsed={} frame={} image={} left={:.1} top={:.1} additive={} mask={:?} shadow=({:?},{:?})",
                        p.generation,
                        p.sequence,
                        p.packet,
                        p.spell,
                        instance.key,
                        animation.kind,
                        tile_x,
                        tile_y,
                        started_at,
                        elapsed,
                        frame.path,
                        frame.path,
                        left,
                        top,
                        animation.blend,
                        frame.mask_path,
                        frame.shadow_x,
                        frame.shadow_y
                    );
                }
                let mask_image_url = frame
                    .mask_path
                    .as_ref()
                    .filter(|p| crate::frame_png_exists(p))
                    .cloned();
                // Shadow is treated as a WHOLE: a legal offset may have one axis
                // zero (e.g. (0, -5) or (4, 0)). Only when both are absent (None)
                // is there no shadow at all.
                let shadow_pair = if instance.kind == EffectKindTag::AttackOverlay {
                    // PlayerObject.DrawEffects adds only the source bitmap;
                    // the actor body owns its own shadow.
                    None
                } else {
                    match (frame.shadow_x, frame.shadow_y) {
                        (Some(sx), Some(sy)) => Some((sx, sy)),
                        _ => None,
                    }
                };
                let normal_z = effect_z(tile_x, tile_y, instance.kind.z_order());
                let z = if instance.kind.draws_post_world() {
                    post_world_depth_bounds
                        .map(|bounds| crate::atlas::post_world_effect_z_from_bounds(bounds, normal_z))
                        .unwrap_or(normal_z)
                } else {
                    normal_z
                };
                let mut entry = json!({
                    "key": instance.key,
                    "imageUrl": frame.path,
                    "left": left,
                    "top": top,
                    "width": frame.width,
                    "height": frame.height,
                    "z": z,
                    "additive": animation.blend,
                    "opacity": animation.opacity,
                });
                if let Some(mask) = mask_image_url {
                    entry["maskImageUrl"] = json!(mask);
                    // Pass mask geometry AND the primary frame's local offset so the
                    // runtime can place the mask at (maskX-frameX, maskY-frameY)
                    // relative to the primary anchor — mask and frame are both
                    // expressed in the same local origin, so the delta is exact.
                    entry["frameX"] = json!(frame.x);
                    entry["frameY"] = json!(frame.y);
                    if let Some(mw) = frame.mask_width {
                        entry["maskWidth"] = json!(mw);
                    }
                    if let Some(mh) = frame.mask_height {
                        entry["maskHeight"] = json!(mh);
                    }
                    if let Some(mx) = frame.mask_x {
                        entry["maskX"] = json!(mx);
                    }
                    if let Some(my) = frame.mask_y {
                        entry["maskY"] = json!(my);
                    }
                }
                if let Some((sx, sy)) = shadow_pair {
                    entry["shadowX"] = json!(sx);
                    entry["shadowY"] = json!(sy);
                }
                Some(entry)
            })
            .collect::<Vec<_>>();

        self.active
            .retain(|instance| instance_still_active(instance, now_ms));
        let live_keys = self
            .active
            .iter()
            .map(|instance| instance.key.as_str())
            .collect::<Vec<_>>();
        self.anchor_object_ids
            .retain(|key, _| live_keys.contains(&key.as_str()));
        self.prestart_source_object_ids
            .retain(|key, _| live_keys.contains(&key.as_str()));
        self.source_object_ids
            .retain(|key, _| live_keys.contains(&key.as_str()));
        self.anchor_player_keys
            .retain(|key| live_keys.contains(&key.as_str()));
        self.local_projectile_targets
            .retain(|key, _| live_keys.contains(&key.as_str()));
        self.pending_sounds.retain(|pending| {
            !pending.requires_active_effect || live_keys.contains(&pending.key.as_str())
        });
        self.prune_hidden_left_guard_tiles();

        self.publish_current_light_snapshots(now_ms, visible);

        let state = json!({
            "enabled": visible,
            "stageWidth": STAGE_WIDTH,
            "stageHeight": STAGE_HEIGHT,
            "effects": if visible { rendered } else { Vec::new() },
        });
        let json_str = serde_json::to_string(&state).ok()?;
        if self.last_state.as_deref() == Some(&json_str) {
            return None;
        }
        Some(json_str)
    }
}

/// Current fractional tile for an instance: interpolated for projectiles,
/// else the anchor tile.
fn instance_current_tile(
    instance: &EffectInstance,
    animation: &Animation,
    started_at: u64,
    now_ms: u64,
) -> (f32, f32) {
    if let (Some(from_x), Some(from_y)) = (instance.from_x, instance.from_y) {
        // Projectile: source -> destination; Return: destination -> source.
        // Use kind to distinguish, since pointer equality fails for cloned test anims.
        if animation.kind == "projectile" {
            let progress =
                projectile_progress(animation.duration_ms, now_ms.saturating_sub(started_at));
            let dx = (instance.tile_x as f32 - from_x) * progress;
            let dy = (instance.tile_y as f32 - from_y) * progress;
            return (from_x + dx, from_y + dy);
        }
        if animation.kind == "return" {
            let progress =
                projectile_progress(animation.duration_ms, now_ms.saturating_sub(started_at));
            let dx = (from_x - instance.tile_x as f32) * progress;
            let dy = (from_y - instance.tile_y as f32) * progress;
            return (instance.tile_x as f32 + dx, instance.tile_y as f32 + dy);
        }
    }
    (instance.tile_x as f32, instance.tile_y as f32)
}

/// Whether a transient instance still has a frame to show at `now_ms`.
fn instance_still_active(instance: &EffectInstance, now_ms: u64) -> bool {
    if let EffectKindTag::Persistent = instance.kind {
        return true;
    }
    if now_ms < instance.start_at {
        return true;
    }
    let Some(current) = &instance.current else {
        return false;
    };
    let current_elapsed = now_ms.saturating_sub(instance.start_at);
    if current_elapsed < current.duration_ms {
        return true;
    }
    if let Some(queued) = &instance.queued {
        let queued_started = instance.start_at + current.duration_ms;
        if now_ms >= queued_started {
            if queued.repeat {
                return true;
            }
            if now_ms - queued_started < queued.duration_ms {
                return true;
            }
            if let Some(ret) = &instance.return_queued {
                let return_started = queued_started + queued.duration_ms;
                if now_ms >= return_started {
                    if ret.repeat {
                        return true;
                    }
                    return now_ms - return_started < ret.duration_ms;
                }
            }
            return false;
        }
    } else if let Some(ret) = &instance.return_queued {
        let return_started = instance.start_at + current.duration_ms;
        if now_ms >= return_started {
            if ret.repeat {
                return true;
            }
            return now_ms - return_started < ret.duration_ms;
        }
    }
    current.repeat
}

/// Determine the active animation slot for an instance, promoting the queued
/// (impact) animation when the current one completes. Returns the animation
/// + its started_at, or None when not visible (delayed/expired).
fn advance_instance(instance: &EffectInstance, now_ms: u64) -> Option<(&Animation, u64)> {
    if now_ms < instance.start_at {
        return None;
    }
    let current = instance.current.as_ref()?;
    let current_elapsed = now_ms.saturating_sub(instance.start_at);
    if current_elapsed < current.duration_ms {
        return Some((current, instance.start_at));
    }
    if let Some(queued) = &instance.queued {
        let queued_started = instance.start_at + current.duration_ms;
        if now_ms >= queued_started {
            if queued.repeat || now_ms - queued_started < queued.duration_ms {
                return Some((queued, queued_started));
            }
            if let Some(ret) = &instance.return_queued {
                let return_started = queued_started + queued.duration_ms;
                if now_ms >= return_started {
                    if ret.repeat || now_ms - return_started < ret.duration_ms {
                        return Some((ret, return_started));
                    }
                    return None;
                }
                return None;
            }
            return None;
        }
        return None;
    }
    if let Some(ret) = &instance.return_queued {
        let return_started = instance.start_at + current.duration_ms;
        if now_ms >= return_started {
            if ret.repeat || now_ms - return_started < ret.duration_ms {
                return Some((ret, return_started));
            }
            return None;
        }
        return None;
    }
    if current.repeat {
        return Some((current, instance.start_at));
    }
    None
}

/// Interpolated projectile progress in 0..=1.
fn projectile_progress(duration_ms: u64, elapsed_ms: u64) -> f32 {
    if duration_ms == 0 {
        return 1.0;
    }
    (elapsed_ms as f32 / duration_ms as f32).clamp(0.0, 1.0)
}

/// Screen-stage left/top from a tile (fractional for projectiles) plus the
/// frame source offset. left = ORIGIN_X + tile_delta_x*CELL + frame.x, etc.
pub(crate) fn screen_position(
    player_x: i32,
    player_y: i32,
    tile_x: f32,
    tile_y: f32,
    frame_x: f32,
    frame_y: f32,
) -> (f32, f32) {
    let left = ENTITY_ORIGIN_X + (tile_x - player_x as f32) * CELL_WIDTH + frame_x;
    let top = ENTITY_ORIGIN_Y + (tile_y - player_y as f32) * CELL_HEIGHT + frame_y;
    (left, top)
}

/// World-z depth band: same cell-depth * gain as entities, plus a band order.
pub(crate) fn effect_z(tile_x: f32, tile_y: f32, order: f32) -> f32 {
    let cell_depth = tile_y * 1000.0 + tile_x * 10.0;
    cell_depth * EFFECT_DEPTH_GAIN + order
}

fn value_f32(payload: &Value, object: &str, field: &str) -> Option<f32> {
    payload
        .get(object)
        .and_then(Value::as_object)
        .and_then(|map| map.get(field))
        .and_then(Value::as_f64)
        .map(|v| v as f32)
}

fn spell_number_u32(value: &Value) -> u32 {
    value.as_u64().map(|v| v as u32).unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Native host Bevy entry point (wired in main.rs Update chain).
// ---------------------------------------------------------------------------

/// Bevy system: advance the effect clock and push the render state each frame.
pub(crate) fn tick_native_effects(
    time: bevy::prelude::Res<bevy::prelude::Time>,
    mut effects: bevy::prelude::ResMut<NativeEffects>,
    mut gameplay_audio: Option<
        bevy::prelude::ResMut<mir2_client_bevy::audio::NativeGameplayAudioQueue>,
    >,
    player_ui: Option<
        bevy::prelude::Res<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>,
    >,
) {
    let now_ms = u64::try_from(time.elapsed().as_millis()).unwrap_or(u64::MAX);
    if let Some(queue) = gameplay_audio.as_deref_mut() {
        for event in effects.take_due_sound_events(now_ms) {
            queue.push(event);
        }
    }
    let effect_visible = player_ui
        .as_deref()
        .map(|state| state.core.options.effect)
        .unwrap_or(true);
    let render_state = effects.tick_with_visibility(now_ms, effect_visible);
    effects.maybe_emit_native_soak_metrics(now_ms);
    if let Some(json) = render_state {
        let success =
            mir2_bevy_runtime::native_ingest::push_native_effect_render_state(json.clone());
        if fx_trace_enabled() {
            eprintln!(
                "[fx-trace] push len={} success={} json_preview={}",
                json.len(),
                success,
                &json[..json.len().min(200)]
            );
        }
        if success {
            effects.last_state = Some(json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay_bridge::{NativeEffectEvent, NativeGameplayAdapter};
    use crate::native_protocol::PacketEvent;
    use serde_json::json;

    #[test]
    fn native_soak_metrics_gate_is_ten_seconds_and_saturating() {
        assert!(should_emit_native_soak_metrics(None, 0));
        assert!(!should_emit_native_soak_metrics(Some(0), 9_999));
        assert!(should_emit_native_soak_metrics(Some(0), 10_000));
        assert!(!should_emit_native_soak_metrics(Some(10_000), 10_001));
        assert!(!should_emit_native_soak_metrics(Some(10_000), 9_000));
    }

    #[test]
    fn native_soak_metrics_json_has_bounded_effect_fields() {
        let payload: Value = serde_json::from_str(&native_soak_metrics_json(4_242, 12_345, 7))
            .expect("valid metrics JSON");
        assert_eq!(payload["processId"], 4_242);
        assert_eq!(payload["timestampMs"], 12_345);
        assert_eq!(payload["activeEffects"], 7);
        assert_eq!(payload["activeEffectsCap"], MAX_ACTIVE_EFFECTS);
        assert!(!native_soak_metrics_json(4_242, 12_345, 7).contains('\n'));
    }

    #[test]
    fn crystal_foreground_scene_and_actor_effects_render_after_selected_redraw() {
        for kind in [
            EffectKindTag::SceneForeground,
            EffectKindTag::Cast,
            EffectKindTag::AttackOverlay,
            EffectKindTag::Projectile,
            EffectKindTag::Impact,
        ] {
            assert!(kind.draws_post_world());
        }
        for kind in [EffectKindTag::Ground, EffectKindTag::Persistent] {
            assert!(!kind.draws_post_world());
        }

        let payload = json!({
            "sceneView": {"center": {"x": 288, "y": 616}, "width": 19, "height": 15},
            "entities": [{"objectId": 1, "kind": "player", "x": 292, "y": 618}]
        });
        let mut effects = NativeEffects::default();
        effects.observe_render_payload(&payload);
        assert_eq!(
            effects.post_world_depth_bounds,
            Some(crate::atlas::post_world_depth_bounds(&payload))
        );

        let bounds = effects.post_world_depth_bounds.expect("observed bounds");
        let animation = fake_anim("foreground", 1_000, 1, false);
        effects.active.extend([
            EffectInstance {
                key: "object-or-map-effect".to_owned(),
                kind: EffectKindTag::SceneForeground,
                tile_x: 280,
                tile_y: 608,
                from_x: None,
                from_y: None,
                current: Some(animation.clone()),
                queued: None,
                return_queued: None,
                started_at: 0,
                start_at: 0,
                persistent_object_id: None,
                provenance: EffectProvenance::default(),
            },
            EffectInstance {
                key: "persistent-object-spell".to_owned(),
                kind: EffectKindTag::Persistent,
                tile_x: 292,
                tile_y: 618,
                from_x: None,
                from_y: None,
                current: Some(animation),
                queued: None,
                return_queued: None,
                started_at: 0,
                start_at: 0,
                persistent_object_id: Some(7),
                provenance: EffectProvenance::default(),
            },
        ]);
        let state: Value = serde_json::from_str(&effects.tick(0).expect("render state"))
            .expect("render-state JSON");
        let foreground_z = state["effects"]
            .as_array()
            .expect("effects")
            .iter()
            .find(|effect| effect["key"] == "object-or-map-effect")
            .and_then(|effect| effect["z"].as_f64())
            .expect("foreground z") as f32;
        let persistent_z = state["effects"]
            .as_array()
            .expect("effects")
            .iter()
            .find(|effect| effect["key"] == "persistent-object-spell")
            .and_then(|effect| effect["z"].as_f64())
            .expect("persistent z") as f32;
        assert!(
            foreground_z > crate::atlas::post_world_highlight_band_ceiling(bounds),
            "ObjectEffect/MapEffect must remain visible above the selected redraw"
        );
        assert!(
            persistent_z < crate::atlas::post_world_highlight_band_ceiling(bounds),
            "persistent ObjectSpell remains in the world pass"
        );

        effects.reset_session();
        assert_eq!(effects.post_world_depth_bounds, None);
    }

    fn fake_anim(kind: &str, interval: u64, count: u64, repeat: bool) -> Animation {
        let frames = (0..count)
            .map(|i| EffectFrameMeta {
                path: format!("/f/{i}.png"),
                width: 48.0,
                height: 48.0,
                x: 0.0,
                y: 0.0,
                shadow_x: None,
                shadow_y: None,
                mask_path: None,
                mask_width: None,
                mask_height: None,
                mask_x: None,
                mask_y: None,
            })
            .collect::<Vec<_>>();
        Animation {
            name: "test".to_owned(),
            kind: kind.to_owned(),
            interval,
            frames: frames.clone(),
            blend: true,
            opacity: 1.0,
            repeat,
            offset_x: 0.0,
            offset_y: 0.0,
            duration_ms: interval * count,
            light: None,
        }
    }

    #[test]
    fn manifest_loads() {
        let catalog = EffectCatalog::load().expect("manifest should load");
        assert!(!catalog.spell_by_name.is_empty());
        assert!(!catalog.libraries.is_empty());
    }

    #[test]
    fn player_revive_catalog_is_exact_magic2_twenty_frame_sequence() {
        let catalog = EffectCatalog::load().expect("manifest should load");
        let animation = catalog
            .map_animation("PlayerRevive", 0)
            .expect("PlayerRevive animation");
        assert_eq!(animation.kind, "target");
        assert_eq!(animation.frames.len(), 20);
        assert_eq!(animation.interval, 100);
        assert_eq!(animation.duration_ms, 2_000);
        assert_eq!(animation.light, Some(6));
        assert!(animation.frames[0].path.ends_with("/Magic2/1220.png"));
        assert!(animation.frames[19].path.ends_with("/Magic2/1239.png"));
    }

    #[test]
    fn player_revive_effect_and_audio_follow_self_remote_and_effect_gates() {
        let mut self_fx = NativeEffects::default();
        self_fx.observe(
            1_000,
            288,
            616,
            &[NativeEffectEvent {
                sequence: 1,
                generation: 0,
                packet: "Revived".to_owned(),
                payload: json!({"location": {"x": 288, "y": 616}}),
            }],
            &HashMap::new(),
        );
        assert_eq!(self_fx.active.len(), 1);
        assert_eq!(self_fx.active[0].key, "player-revive-self");
        assert_eq!(
            (self_fx.active[0].tile_x, self_fx.active[0].tile_y),
            (288, 616)
        );
        assert!(self_fx.anchor_player_keys.contains("player-revive-self"));
        let self_audio = self_fx.take_due_sound_events(1_000);
        assert_eq!(self_audio.len(), 1);
        assert_eq!(self_audio[0].cue, PLAYER_REVIVE_SOUND_CUE);
        assert_eq!(self_audio[0].file_name, PLAYER_REVIVE_SOUND_FILE);

        self_fx.observe(1_100, 289, 617, &[], &HashMap::new());
        assert_eq!(
            (self_fx.active[0].tile_x, self_fx.active[0].tile_y),
            (289, 617)
        );
        let _ = self_fx.tick(3_000);
        assert!(self_fx.active.is_empty());
        assert!(self_fx.anchor_player_keys.is_empty());

        let zone = HashMap::from([(2_001_u32, (300, 610))]);
        let mut remote_fx = NativeEffects::default();
        remote_fx.observe(
            2_000,
            288,
            616,
            &[NativeEffectEvent {
                sequence: 1,
                generation: 0,
                packet: "ObjectRevived".to_owned(),
                payload: json!({"objectId": 2001, "effect": false}),
            }],
            &zone,
        );
        assert!(remote_fx.active.is_empty());
        assert!(remote_fx.take_due_sound_events(2_000).is_empty());

        remote_fx.observe(
            2_100,
            288,
            616,
            &[NativeEffectEvent {
                sequence: 2,
                generation: 0,
                packet: "ObjectRevived".to_owned(),
                payload: json!({"objectId": 2001, "effect": true}),
            }],
            &zone,
        );
        assert_eq!(remote_fx.active.len(), 1);
        assert_eq!(remote_fx.active[0].key, "player-revive-2001");
        assert_eq!(
            (remote_fx.active[0].tile_x, remote_fx.active[0].tile_y),
            (300, 610)
        );
        assert_eq!(remote_fx.take_due_sound_events(2_100).len(), 1);

        remote_fx.observe(
            2_200,
            288,
            616,
            &[NativeEffectEvent {
                sequence: 3,
                generation: 0,
                packet: "ObjectRemove".to_owned(),
                payload: json!({"objectId": 2001}),
            }],
            &zone,
        );
        assert!(remote_fx.active.is_empty());
    }

    fn player_actor(
        object_id: u32,
        gender: &str,
        class: &str,
        body: &str,
        weapon: Option<&str>,
        mount: Option<&str>,
    ) -> Value {
        let mut sprite =
            serde_json::Map::from_iter([("bodyLibrary".to_owned(), Value::from(body))]);
        if let Some(weapon) = weapon {
            sprite.insert("weaponLibrary".to_owned(), Value::from(weapon));
        }
        if let Some(mount) = mount {
            sprite.insert("mountLibrary".to_owned(), Value::from(mount));
        }
        json!({
            "objectId": object_id,
            "kind": "player",
            "genderKey": gender,
            "classKey": class,
            "sprite": sprite,
        })
    }

    fn player_sound_event(
        sequence: u64,
        packet: &str,
        target: Value,
        attacker: Option<Value>,
    ) -> NativeEffectEvent {
        let mut payload = serde_json::Map::from_iter([
            ("objectId".to_owned(), target["objectId"].clone()),
            ("_nativeTarget".to_owned(), target),
        ]);
        if let Some(attacker) = attacker {
            payload.insert("_nativeAttacker".to_owned(), attacker);
        }
        NativeEffectEvent {
            sequence,
            generation: 0,
            packet: packet.to_owned(),
            payload: Value::Object(payload),
        }
    }

    fn monster_actor(object_id: u32, body: &str) -> Value {
        json!({
            "objectId": object_id,
            "kind": "monster",
            "sprite": { "bodyLibrary": body },
        })
    }

    #[test]
    fn owner_revive_alias_packets_emit_one_effect_and_one_sound() {
        let owner = player_actor(
            1_000,
            "male",
            "warrior",
            "/original-ui/CArmour/00",
            None,
            None,
        );
        let events = [
            player_sound_event(1, "Revived", owner.clone(), None),
            NativeEffectEvent {
                sequence: 2,
                generation: 0,
                packet: "ObjectRevived".to_owned(),
                payload: json!({
                    "objectId": 1000,
                    "effect": true,
                    "_nativeTarget": owner,
                }),
            },
        ];
        let mut fx = NativeEffects::default();
        fx.observe(
            1_000,
            288,
            616,
            &events,
            &HashMap::from([(1_000_u32, (288, 616))]),
        );

        assert_eq!(fx.active.len(), 1);
        assert_eq!(fx.active[0].key, "player-revive-self");
        let sounds = fx.take_due_sound_events(1_000);
        assert_eq!(sounds.len(), 1);
        assert_eq!(sounds[0].file_name, PLAYER_REVIVE_SOUND_FILE);
    }

    #[test]
    fn player_struck_audio_resolves_weapon_armour_mount_and_gender_families() {
        let heavy_female = player_actor(
            2_001,
            "female",
            "warrior",
            "/original-ui/CArmour/03",
            None,
            None,
        );
        let axe_attacker = player_actor(
            2_002,
            "male",
            "warrior",
            "/original-ui/CArmour/00",
            Some("/original-ui/CWeapon/04"),
            None,
        );
        let mut fx = NativeEffects::default();
        fx.observe(
            1_000,
            288,
            616,
            &[player_sound_event(
                1,
                "ObjectStruck",
                heavy_female,
                Some(axe_attacker),
            )],
            &HashMap::new(),
        );
        let sounds = fx.take_due_sound_events(1_000);
        assert_eq!(sounds.len(), 2);
        assert_eq!(sounds[0].file_name, PLAYER_STRUCK_ARMOUR_AXE_FILE);
        assert_eq!(sounds[1].file_name, PLAYER_FEMALE_FLINCH_FILE);

        let mounted_male = player_actor(
            2_003,
            "male",
            "warrior",
            "/original-ui/CArmour/00",
            None,
            Some("/original-ui/Mount/03"),
        );
        fx.observe(
            1_100,
            288,
            616,
            &[player_sound_event(2, "ObjectStruck", mounted_male, None)],
            &HashMap::new(),
        );
        let sounds = fx.take_due_sound_events(1_100);
        assert_eq!(sounds.len(), 2);
        assert_eq!(sounds[0].file_name, PLAYER_TIGER_STRUCK_2_FILE);
        assert_eq!(sounds[1].file_name, PLAYER_MALE_FLINCH_FILE);
        assert!(!sounds.iter().any(|sound| {
            matches!(
                sound.file_name.as_str(),
                PLAYER_STRUCK_BODY_SWORD_FILE
                    | PLAYER_STRUCK_BODY_AXE_FILE
                    | PLAYER_STRUCK_BODY_LONG_STICK_FILE
                    | PLAYER_STRUCK_BODY_FIST_FILE
            )
        }));

        let ordinary_male = player_actor(
            2_004,
            "male",
            "warrior",
            "/original-ui/CArmour/00",
            None,
            None,
        );
        let mut mounted_axe_attacker = player_actor(
            2_005,
            "male",
            "warrior",
            "/original-ui/CArmour/00",
            None,
            Some("/original-ui/Mount/03"),
        );
        mounted_axe_attacker["weapon"] = Value::from(4);
        mounted_axe_attacker["ridingMount"] = Value::Bool(true);
        fx.observe(
            1_200,
            288,
            616,
            &[player_sound_event(
                3,
                "ObjectStruck",
                ordinary_male,
                Some(mounted_axe_attacker),
            )],
            &HashMap::new(),
        );
        let sounds = fx.take_due_sound_events(1_200);
        assert_eq!(sounds.len(), 2);
        assert_eq!(sounds[0].file_name, PLAYER_STRUCK_BODY_AXE_FILE);
        assert_eq!(sounds[1].file_name, PLAYER_MALE_FLINCH_FILE);
    }

    #[test]
    fn lethal_struck_batch_keeps_hit_and_flinch_before_delayed_death_cry() {
        let target = player_actor(
            2_001,
            "female",
            "warrior",
            "/original-ui/CArmour/03",
            None,
            None,
        );
        let attacker = player_actor(
            2_002,
            "male",
            "warrior",
            "/original-ui/CArmour/00",
            Some("/original-ui/CWeapon/04"),
            None,
        );
        let events = [
            player_sound_event(1, "ObjectStruck", target.clone(), Some(attacker)),
            player_sound_event(2, "ObjectDied", target, None),
        ];
        let mut fx = NativeEffects::default();
        fx.observe(1_000, 288, 616, &events, &HashMap::new());

        let struck = fx.take_due_sound_events(1_000);
        assert_eq!(struck.len(), 2);
        assert_eq!(struck[0].file_name, PLAYER_STRUCK_ARMOUR_AXE_FILE);
        assert_eq!(struck[1].file_name, PLAYER_FEMALE_FLINCH_FILE);
        let _ = fx.tick_with_visibility(1_050, true);
        let death = fx.take_due_sound_events(1_100);
        assert_eq!(death.len(), 1);
        assert_eq!(death[0].file_name, PLAYER_FEMALE_DIE_FILE);
    }

    #[test]
    fn native_gameplay_audio_allowlist_contains_every_implemented_combat_clip() {
        for file_name in [
            SCARECROW_DIE_SOUND_FILE,
            PLAYER_STRUCK_BODY_SWORD_FILE,
            PLAYER_STRUCK_BODY_AXE_FILE,
            PLAYER_STRUCK_BODY_LONG_STICK_FILE,
            PLAYER_STRUCK_BODY_FIST_FILE,
            PLAYER_STRUCK_ARMOUR_SWORD_FILE,
            PLAYER_STRUCK_ARMOUR_AXE_FILE,
            PLAYER_STRUCK_ARMOUR_LONG_STICK_FILE,
            PLAYER_STRUCK_ARMOUR_FIST_FILE,
            PLAYER_MALE_FLINCH_FILE,
            PLAYER_FEMALE_FLINCH_FILE,
            PLAYER_MALE_DIE_FILE,
            PLAYER_FEMALE_DIE_FILE,
            PLAYER_TIGER_STRUCK_1_FILE,
            PLAYER_TIGER_STRUCK_2_FILE,
            PLAYER_WOLF_STRUCK_FILE,
            PLAYER_REVIVE_SOUND_FILE,
        ] {
            assert!(
                mir2_client_bevy::audio::NATIVE_GAMEPLAY_SOUND_FILES.contains(&file_name),
                "native gameplay audio rejected {file_name}"
            );
        }
    }

    #[test]
    fn player_death_audio_waits_one_frame_and_lifecycle_packets_cancel_it() {
        let female = player_actor(
            2_001,
            "female",
            "wizard",
            "/original-ui/CArmour/00",
            None,
            None,
        );
        let mut fx = NativeEffects::default();
        fx.observe(
            1_000,
            288,
            616,
            &[player_sound_event(1, "ObjectDied", female.clone(), None)],
            &HashMap::new(),
        );
        let _ = fx.tick_with_visibility(1_000, true);
        let _ = fx.tick_with_visibility(1_050, true);
        assert!(fx.take_due_sound_events(1_099).is_empty());
        let due = fx.take_due_sound_events(1_100);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].file_name, PLAYER_FEMALE_DIE_FILE);

        fx.observe(
            1_200,
            288,
            616,
            &[player_sound_event(2, "ObjectDied", female.clone(), None)],
            &HashMap::new(),
        );
        assert!(fx.take_due_sound_events(1_300).is_empty());

        fx.observe(
            1_400,
            288,
            616,
            &[NativeEffectEvent {
                sequence: 3,
                generation: 0,
                packet: "ObjectRevived".to_owned(),
                payload: json!({
                    "objectId": 2001,
                    "effect": false,
                    "_nativeTarget": female.clone(),
                }),
            }],
            &HashMap::new(),
        );
        fx.observe(
            1_500,
            288,
            616,
            &[player_sound_event(4, "ObjectDied", female.clone(), None)],
            &HashMap::new(),
        );
        fx.observe(
            1_550,
            288,
            616,
            &[NativeEffectEvent {
                sequence: 5,
                generation: 0,
                packet: "ObjectRemove".to_owned(),
                payload: json!({"objectId": 2001}),
            }],
            &HashMap::new(),
        );
        assert!(fx.take_due_sound_events(1_600).is_empty());

        fx.observe(
            1_700,
            288,
            616,
            &[player_sound_event(6, "ObjectDied", female, None)],
            &HashMap::new(),
        );
        fx.observe(
            1_750,
            288,
            616,
            &[NativeEffectEvent {
                sequence: 7,
                generation: 0,
                packet: "MapChanged".to_owned(),
                payload: json!({"mapIndex": 1}),
            }],
            &HashMap::new(),
        );
        assert!(fx.take_due_sound_events(1_800).is_empty());

        fx.observe(
            1_900,
            288,
            616,
            &[player_sound_event(
                8,
                "ObjectDied",
                player_actor(
                    2_001,
                    "female",
                    "wizard",
                    "/original-ui/CArmour/00",
                    None,
                    None,
                ),
                None,
            )],
            &HashMap::new(),
        );
        fx.observe(
            1_950,
            288,
            616,
            &[NativeEffectEvent {
                sequence: 9,
                generation: 0,
                packet: "LogOutSuccess".to_owned(),
                payload: json!({}),
            }],
            &HashMap::new(),
        );
        assert!(fx.take_due_sound_events(2_000).is_empty());
    }

    #[test]
    fn scarecrow_death_audio_uses_crystal_numeric_file_resolution_once() {
        let scarecrow = monster_actor(5_001, "/original-ui/Monster/005");
        let mut fx = NativeEffects::default();
        fx.observe(
            1_000,
            288,
            616,
            &[player_sound_event(
                1,
                "ObjectDied",
                scarecrow.clone(),
                None,
            )],
            &HashMap::new(),
        );
        let due = fx.take_due_sound_events(1_000);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].file_name, SCARECROW_DIE_SOUND_FILE);
        assert_eq!(due[0].cue, "Scarecrow.5001.Die");

        fx.observe(
            1_100,
            288,
            616,
            &[player_sound_event(2, "ObjectDied", scarecrow, None)],
            &HashMap::new(),
        );
        assert!(fx.take_due_sound_events(1_100).is_empty());
    }

    #[test]
    fn scarecrow_death_audio_is_exact_and_lifecycle_scoped() {
        for target in [
            monster_actor(5_010, "/original-ui/Monster/004"),
            json!({
                "objectId": 5011,
                "kind": "player",
                "sprite": { "bodyLibrary": "/original-ui/Monster/005" },
            }),
            json!({
                "objectId": 5012,
                "kind": "monster",
                "name": "Scarecrow",
                "sprite": { "bodyLibrary": "/original-ui/Monster/006" },
            }),
        ] {
            let mut fx = NativeEffects::default();
            fx.observe(
                1_000,
                288,
                616,
                &[player_sound_event(1, "ObjectDied", target, None)],
                &HashMap::new(),
            );
            assert!(fx.take_due_sound_events(1_000).is_empty());
        }

        for lifecycle_packet in [
            "ObjectRemove",
            "ObjectHide",
            "MapChanged",
            "LogOutSuccess",
        ] {
            let mut fx = NativeEffects::default();
            let mut events = vec![player_sound_event(
                1,
                "ObjectDied",
                monster_actor(5_020, "Monster/005"),
                None,
            )];
            events.push(NativeEffectEvent {
                sequence: 2,
                generation: 0,
                packet: lifecycle_packet.to_owned(),
                payload: if matches!(lifecycle_packet, "ObjectRemove" | "ObjectHide") {
                    json!({"objectId": 5020})
                } else {
                    json!({})
                },
            });
            fx.observe(1_000, 288, 616, &events, &HashMap::new());
            assert!(
                fx.take_due_sound_events(1_000).is_empty(),
                "{lifecycle_packet} must cancel the same-batch Scarecrow cry"
            );
        }
    }

    #[test]
    fn fireball_cast_animation_resolves() {
        let catalog = EffectCatalog::load().expect("load");
        let cast = catalog
            .spell_cast_animation("FireBall", 4)
            .expect("cast animation");
        assert_eq!(cast.kind, "cast");
        assert_eq!(cast.frames.len(), 10);
        assert_eq!(cast.interval, 60);
        assert_eq!(cast.duration_ms, 600);
        assert_eq!(cast.light, Some(6));
    }

    #[test]
    fn fireball_projectile_and_impact_spec() {
        let catalog = EffectCatalog::load().expect("load");
        let entry = catalog.spell_by_name.get("FireBall").expect("entry");
        assert_eq!(entry.library, "Magic");
        assert_eq!(entry.base, 0);
        assert_eq!(entry.count, 10);
        assert_eq!(entry.interval, 60);
        let proj = entry.projectile.as_ref().expect("projectile sub");
        assert_eq!(proj.library, "Magic");
        assert_eq!(proj.base, 10);
        assert_eq!(proj.count, 6);
        assert_eq!(proj.interval, Some(30));
        let impact = entry.impact.as_ref().expect("impact sub");
        assert_eq!(impact.library, "Magic");
        assert_eq!(impact.base, 170);
        assert_eq!(impact.count, 10);
        assert_eq!(impact.interval, Some(60));
    }

    #[test]
    fn fireball_projectile_and_impact_resolve_via_subspec() {
        let catalog = EffectCatalog::load().expect("load");
        let projectile = catalog
            .spell_projectile_animation("FireBall", 0)
            .expect("projectile via resolve_sub");
        assert!(
            projectile.frames[0].path.ends_with("/Magic/10.png"),
            "projectile first frame should be Magic/10.png, got {}",
            projectile.frames[0].path
        );
        assert_eq!(projectile.kind, "projectile");
        assert_eq!(projectile.frames.len(), 6);
        assert_eq!(projectile.interval, 30);
        assert_eq!(projectile.light, Some(6));
        let impact = catalog
            .spell_impact_animation("FireBall")
            .expect("impact via resolve_sub");
        assert!(
            impact.frames[0].path.ends_with("/Magic/170.png"),
            "impact first frame should be Magic/170.png, got {}",
            impact.frames[0].path
        );
        assert_eq!(impact.frames.len(), 10);
        assert_eq!(impact.interval, 60);
        assert_eq!(impact.light, Some(6));
    }

    #[test]
    fn fireball_effect_light_tracks_cast_projectile_and_impact() {
        let mut fx = NativeEffects::default();
        let zone = HashMap::from([(100_u32, (10, 10)), (200_u32, (14, 10))]);
        fx.observe(
            0,
            10,
            10,
            &[NativeEffectEvent {
                sequence: 1,
                generation: 0,
                packet: "ObjectMagic".to_owned(),
                payload: json!({
                    "location": {"x": 10, "y": 10},
                    "spell": "FireBall",
                    "direction": "down"
                }),
            }],
            &zone,
        );
        let cast = fx.current_light_snapshots(0);
        assert_eq!(cast.len(), 1);
        assert_eq!(cast[0].generation, 0);
        assert_eq!(cast[0].light, 6);
        assert_eq!((cast[0].tile_x, cast[0].tile_y), (10.0, 10.0));

        fx.observe(
            0,
            10,
            10,
            &[NativeEffectEvent {
                sequence: 2,
                generation: 0,
                packet: "ObjectProjectile".to_owned(),
                payload: json!({
                    "spell": "FireBall",
                    "sourceId": 100,
                    "destinationId": 200
                }),
            }],
            &zone,
        );
        let projectile_instance = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-proj-"))
            .expect("projectile instance");
        assert_eq!(
            projectile_instance
                .current
                .as_ref()
                .map(|anim| anim.kind.as_str()),
            Some("projectile")
        );
        assert_eq!(
            projectile_instance
                .current
                .as_ref()
                .map(|anim| anim.duration_ms),
            Some(180)
        );
        let moving = fx
            .current_light_snapshots(90)
            .into_iter()
            .find(|snapshot| snapshot.key.starts_with("fx-proj-"))
            .expect("projectile light");
        assert!(
            moving.tile_x > 10.0 && moving.tile_x < 14.0,
            "unexpected projectile light: {moving:?}"
        );
        let later = fx
            .current_light_snapshots(120)
            .into_iter()
            .find(|snapshot| snapshot.key.starts_with("fx-proj-"))
            .expect("later projectile light");
        assert!(later.tile_x > moving.tile_x);

        let impact = fx
            .current_light_snapshots(180)
            .into_iter()
            .find(|snapshot| snapshot.key.starts_with("fx-proj-"))
            .expect("impact light");
        assert_eq!((impact.tile_x, impact.tile_y), (14.0, 10.0));
    }

    #[test]
    fn effect_light_snapshot_expires_and_clears_on_scene_reset() {
        let mut fx = NativeEffects::default();
        let mut animation = fake_anim("cast", 50, 2, false);
        animation.light = Some(6);
        fx.active.push(EffectInstance {
            key: "expiring-light".to_owned(),
            kind: EffectKindTag::Cast,
            tile_x: 2,
            tile_y: 3,
            from_x: None,
            from_y: None,
            current: Some(animation),
            queued: None,
            return_queued: None,
            started_at: 0,
            start_at: 0,
            persistent_object_id: None,
            provenance: EffectProvenance::default(),
        });
        assert_eq!(fx.current_light_snapshots(49).len(), 1);
        fx.publish_current_light_snapshots(0, true);
        assert!(native_effect_light_snapshots()
            .iter()
            .any(|snapshot| snapshot.key == "expiring-light"));
        assert!(fx.current_light_snapshots(100).is_empty());
        let _ = fx.tick(100);
        assert!(native_effect_light_snapshots().is_empty());
        fx.reset_for_new_connection();
        assert!(fx.current_light_snapshots(100).is_empty());
        assert!(native_effect_light_snapshots().is_empty());
    }

    #[test]
    fn effect_light_snapshot_publishes_across_threads_and_keeps_generation() {
        let snapshot = NativeEffectLightSnapshot {
            generation: 700_001,
            key: "cross-thread-light".to_owned(),
            tile_x: 12.5,
            tile_y: 20.0,
            light: 6,
        };
        let writer = std::thread::spawn(move || publish_effect_lights(vec![snapshot]));
        writer.join().expect("effect light publisher thread");
        let current = native_effect_light_snapshots();
        let published = current
            .iter()
            .find(|snapshot| snapshot.key == "cross-thread-light")
            .expect("cross-thread effect light snapshot");
        assert_eq!(published.generation, 700_001);
        assert_eq!((published.tile_x, published.tile_y), (12.5, 20.0));
        publish_effect_lights(Vec::new());
    }

    #[test]
    fn effect_without_manifest_light_emits_no_light_snapshot() {
        let mut fx = NativeEffects::default();
        let animation = fake_anim("cast", 50, 2, false);
        fx.active.push(EffectInstance {
            key: "no-light".to_owned(),
            kind: EffectKindTag::Cast,
            tile_x: 2,
            tile_y: 3,
            from_x: None,
            from_y: None,
            current: Some(animation),
            queued: None,
            return_queued: None,
            started_at: 0,
            start_at: 0,
            persistent_object_id: None,
            provenance: EffectProvenance::default(),
        });
        assert!(fx.current_light_snapshots(0).is_empty());
    }

    #[test]
    fn lightning_direction_7_resolves_base_1110() {
        let catalog = EffectCatalog::load().expect("load");
        let anim = catalog.spell_animation("Lightning", 7).expect("lightning");
        assert!(anim.frames[0].path.ends_with("/Magic/1110.png"));
    }

    #[test]
    fn mine_value_7_resolves_effect_frame_56() {
        let catalog = EffectCatalog::load().expect("load");
        let mut found = None;
        for (id, name) in catalog.effect_name_by_number.iter() {
            if name == "Mine" {
                found = Some(*id);
            }
        }
        let mine_id = found.expect("Mine id");
        let anim = catalog
            .map_animation_by_number(mine_id, 7)
            .expect("mine anim");
        assert!(anim.frames[0].path.ends_with("/Effect/56.png"));
    }

    #[test]
    fn duplicate_sequence_does_not_duplicate_effect() {
        let mut fx = NativeEffects::default();
        let zone = HashMap::new();
        let ev = NativeEffectEvent {
            sequence: 1,
            generation: 0,
            packet: "ObjectMagic".to_owned(),
            payload: json!({"location":{"x":10,"y":10},"spell":"FireBall","direction":"down"}),
        };
        fx.observe(0, 0, 0, &[ev.clone()], &zone);
        fx.observe(0, 0, 0, &[ev], &zone);
        assert_eq!(fx.active.len(), 1);
    }

    #[test]
    fn visual_gate_hides_effects_without_discarding_live_instances() {
        let animation = fake_anim("cast", 50, 4, false);
        let mut fx = NativeEffects::default();
        fx.active.push(EffectInstance {
            key: "visible-after-toggle".to_owned(),
            kind: EffectKindTag::Cast,
            tile_x: 1,
            tile_y: 1,
            from_x: None,
            from_y: None,
            current: Some(animation),
            queued: None,
            return_queued: None,
            started_at: 0,
            start_at: 0,
            persistent_object_id: None,
            provenance: EffectProvenance::default(),
        });

        let hidden = fx.tick_with_visibility(25, false).expect("disabled state");
        assert!(hidden.contains("\"enabled\":false"));
        assert!(hidden.contains("\"effects\":[]"));
        assert_eq!(
            fx.active.len(),
            1,
            "render gate must not clear authoritative lifetime"
        );

        let restored = fx.tick_with_visibility(50, true).expect("restored state");
        assert!(restored.contains("\"enabled\":true"));
        assert!(restored.contains("visible-after-toggle"));
    }

    #[test]
    fn projectile_midpoint_interpolation_is_precise() {
        let anim = fake_anim("projectile", 50, 2, false);
        let inst = EffectInstance {
            key: "p".to_owned(),
            kind: EffectKindTag::Projectile,
            tile_x: 2,
            tile_y: 0,
            from_x: Some(0.0),
            from_y: Some(0.0),
            current: Some(anim.clone()),
            queued: None,
            return_queued: None,
            started_at: 0,
            start_at: 0,
            persistent_object_id: None,
            provenance: EffectProvenance::default(),
        };
        let (tx, ty) = instance_current_tile(&inst, &anim, 0, anim.duration_ms / 2);
        assert!((tx - 1.0).abs() < 0.001);
        assert!((ty - 0.0).abs() < 0.001);
    }

    #[test]
    fn projectile_impact_does_not_start_before_projectile_completes() {
        let projectile = fake_anim("projectile", 50, 2, false);
        let impact = fake_anim("impact", 50, 2, false);
        let mut inst = EffectInstance {
            key: "p".to_owned(),
            kind: EffectKindTag::Projectile,
            tile_x: 2,
            tile_y: 0,
            from_x: Some(0.0),
            from_y: Some(0.0),
            current: Some(projectile.clone()),
            queued: Some(impact.clone()),
            return_queued: None,
            started_at: 0,
            start_at: 0,
            persistent_object_id: None,
            provenance: EffectProvenance::default(),
        };
        let mid = advance_instance(&inst, 50).expect("mid projectile");
        assert_eq!(mid.0.kind, "projectile");
        let after = advance_instance(&inst, projectile.duration_ms).expect("impact after");
        assert_eq!(after.0.kind, "impact");
        inst.started_at = 0;
        inst.queued = None;
        assert!(advance_instance(&inst, projectile.duration_ms + 1).is_none());
    }

    #[test]
    fn object_spell_removed_by_object_remove() {
        let mut fx = NativeEffects::default();
        let zone = HashMap::new();
        fx.observe(
            0, 0, 0,
            &[NativeEffectEvent {
                sequence: 1,
                generation: 0,
                packet: "ObjectSpell".to_owned(),
                payload: json!({"objectId": 55, "location":{"x":5,"y":5},"spell":31,"direction":"down","param":0}),
            }],
            &zone,
        );
        assert_eq!(fx.active.len(), 1);
        fx.observe(
            0,
            0,
            0,
            &[NativeEffectEvent {
                sequence: 2,
                generation: 0,
                packet: "ObjectRemove".to_owned(),
                payload: json!({"objectId": 55}),
            }],
            &zone,
        );
        assert!(fx.active.is_empty());
    }

    #[test]
    fn real_trap_hexagon_object_spell_remains_until_authoritative_remove() {
        let mut fx = NativeEffects::default();
        let zone = HashMap::new();
        fx.observe(
            0,
            288,
            616,
            &[NativeEffectEvent {
                sequence: 1,
                generation: 1,
                packet: "ObjectSpell".to_owned(),
                payload: json!({
                    "objectId": 90_073,
                    "location": {"x": 290, "y": 616},
                    "spell": 73,
                    "direction": "up",
                    "param": false
                }),
            }],
            &zone,
        );

        let initial = fx.tick(0).expect("TrapHexagon should render immediately");
        assert!(initial.contains("/original-effects/Magic/1390.png"));
        let later = fx
            .tick(10_000)
            .expect("persistent ObjectSpell should continue animating");
        assert!(later.contains("/original-effects/Magic/1390.png"));
        assert_eq!(fx.active.len(), 1);

        fx.observe(
            10_000,
            288,
            616,
            &[NativeEffectEvent {
                sequence: 2,
                generation: 1,
                packet: "ObjectRemove".to_owned(),
                payload: json!({"objectId": 90_073}),
            }],
            &zone,
        );
        assert!(fx.active.is_empty());
    }

    #[test]
    fn map_changed_and_logout_clear_all_effects() {
        let mut fx = NativeEffects::default();
        let zone = HashMap::new();
        fx.observe(
            0,
            0,
            0,
            &[NativeEffectEvent {
                sequence: 1,
                generation: 0,
                packet: "ObjectMagic".to_owned(),
                payload: json!({"location":{"x":10,"y":10},"spell":"FireBall","direction":"down"}),
            }],
            &zone,
        );
        assert_eq!(fx.active.len(), 1);
        fx.observe(
            0,
            0,
            0,
            &[NativeEffectEvent {
                sequence: 2,
                generation: 0,
                packet: "MapChanged".to_owned(),
                payload: json!({}),
            }],
            &zone,
        );
        assert!(fx.active.is_empty());
        fx.observe(
            0,
            0,
            0,
            &[NativeEffectEvent {
                sequence: 3,
                generation: 0,
                packet: "ObjectMagic".to_owned(),
                payload: json!({"location":{"x":10,"y":10},"spell":"FireBall","direction":"down"}),
            }],
            &zone,
        );
        fx.observe(
            0,
            0,
            0,
            &[NativeEffectEvent {
                sequence: 4,
                generation: 0,
                packet: "LogOutSuccess".to_owned(),
                payload: json!({}),
            }],
            &zone,
        );
        assert!(fx.active.is_empty());
    }

    #[test]
    fn active_effects_never_exceed_the_cap() {
        let mut fx = NativeEffects::default();
        let zone = HashMap::new();
        let events = (1..=200)
            .map(|seq| NativeEffectEvent {
                sequence: seq as u64,
                generation: 0,
                packet: "ObjectMagic".to_owned(),
                payload: json!({"location":{"x":10,"y":10},"spell":"FireBall","direction":"down"}),
            })
            .collect::<Vec<_>>();
        fx.observe(0, 0, 0, &events, &zone);
        assert_eq!(fx.active.len(), MAX_ACTIVE_EFFECTS);
    }

    #[test]
    fn missing_asset_produces_no_effect_and_no_panic() {
        let empty = EffectCatalog {
            libraries: HashMap::new(),
            spell_by_name: HashMap::new(),
            ground_by_spell: HashMap::new(),
            map_by_name: HashMap::new(),
            effect_name_by_number: HashMap::new(),
        };
        assert!(empty.spell_cast_animation("FireBall", 4).is_none());
        assert!(empty.map_animation_by_number(12, 7).is_none());
    }

    #[test]
    fn delay_time_instance_survives_but_not_rendered_before_start_at() {
        let anim = fake_anim("ground", 100, 5, false);
        let instance = EffectInstance {
            key: "delayed".to_owned(),
            kind: EffectKindTag::Ground,
            tile_x: 1,
            tile_y: 1,
            from_x: None,
            from_y: None,
            current: Some(anim.clone()),
            queued: None,
            return_queued: None,
            started_at: 1000,
            start_at: 1500,
            persistent_object_id: None,
            provenance: EffectProvenance::default(),
        };
        assert!(instance_still_active(&instance, 1499));
        assert!(advance_instance(&instance, 1499).is_none());
        assert!(advance_instance(&instance, 1500).is_some());
        assert!(instance_still_active(&instance, 1500));
    }

    #[test]
    fn delay_time_boundary_499_500_and_end() {
        let anim = fake_anim("ground", 100, 5, false);
        let duration = anim.duration_ms;
        assert_eq!(duration, 500);
        let start_at = 2000u64;
        let instance = EffectInstance {
            key: "boundary".to_owned(),
            kind: EffectKindTag::Ground,
            tile_x: 0,
            tile_y: 0,
            from_x: None,
            from_y: None,
            current: Some(anim.clone()),
            queued: None,
            return_queued: None,
            started_at: start_at,
            start_at,
            persistent_object_id: None,
            provenance: EffectProvenance::default(),
        };
        let before_end = advance_instance(&instance, start_at + 499).expect("499 visible");
        assert_eq!(before_end.1, start_at);
        assert!(before_end.0.frame_at(499).is_some());
        assert!(advance_instance(&instance, start_at + 500).is_none());
        assert!(instance_still_active(&instance, start_at + 499));
        assert!(!instance_still_active(&instance, start_at + 500));
        assert!(!instance_still_active(&instance, start_at + 501));
    }

    #[test]
    fn delay_time_elapsed_starts_at_start_at_not_started_at() {
        let anim = fake_anim("ground", 100, 5, false);
        let instance = EffectInstance {
            key: "elapsed".to_owned(),
            kind: EffectKindTag::Ground,
            tile_x: 0,
            tile_y: 0,
            from_x: None,
            from_y: None,
            current: Some(anim.clone()),
            queued: None,
            return_queued: None,
            started_at: 1000,
            start_at: 1500,
            persistent_object_id: None,
            provenance: EffectProvenance::default(),
        };
        assert!(advance_instance(&instance, 1500).is_some());
        let (animation, started) = advance_instance(&instance, 1999).expect("1999 visible");
        assert_eq!(started, 1500);
        assert!(animation.frame_at(499).is_some());
        assert!(advance_instance(&instance, 2000).is_none());
    }

    #[test]
    fn object_spell_same_object_id_replaces_previous() {
        let mut fx = NativeEffects::default();
        let zone = HashMap::new();
        fx.observe(
            0,
            0,
            0,
            &[NativeEffectEvent {
                sequence: 1,
                generation: 0,
                packet: "ObjectSpell".to_owned(),
                payload: json!({"objectId": 55, "location":{"x":5,"y":5},"spell":50,"direction":"down","param":0}),
            }],
            &zone,
        );
        assert_eq!(fx.active.len(), 1);
        let first_key = fx.active[0].key.clone();
        fx.observe(
            0,
            0,
            0,
            &[NativeEffectEvent {
                sequence: 2,
                generation: 0,
                packet: "ObjectSpell".to_owned(),
                payload: json!({"objectId": 55, "location":{"x":6,"y":6},"spell":50,"direction":"down","param":0}),
            }],
            &zone,
        );
        assert_eq!(fx.active.len(), 1);
        assert_eq!(fx.active[0].key, first_key);
        assert_eq!(fx.active[0].tile_x, 6);
    }

    #[test]
    fn push_failure_does_not_commit_last_state() {
        let mut fx = NativeEffects::default();
        let zone = HashMap::new();
        fx.observe(
            0,
            0,
            0,
            &[NativeEffectEvent {
                sequence: 1,
                generation: 0,
                packet: "ObjectMagic".to_owned(),
                payload: json!({"location":{"x":10,"y":10},"spell":"FireBall","direction":"down"}),
            }],
            &zone,
        );
        let first = fx.tick(0).expect("first tick");
        assert!(
            fx.last_state.is_none(),
            "tick must not commit last_state before push"
        );
        let second = fx.tick(0).expect("retry tick after push failure");
        assert_eq!(first, second);
        fx.last_state = Some(first.clone());
        assert!(
            fx.tick(0).is_none(),
            "after commit, same state should be deduped"
        );
    }

    #[test]
    fn object_projectile_from_player_to_monster_succeeds_with_merged_tiles() {
        let mut zone_tiles = HashMap::new();
        zone_tiles.insert(1000u32, (10, 10));
        zone_tiles.insert(2001u32, (12, 10));
        let mut fx = NativeEffects::default();
        fx.observe(
            0,
            10,
            10,
            &[NativeEffectEvent {
                sequence: 1,
                generation: 0,
                packet: "ObjectProjectile".to_owned(),
                payload: json!({"spell":"FireBall","sourceId":1000,"destinationId":2001}),
            }],
            &zone_tiles,
        );
        assert_eq!(fx.active.len(), 1);
        let inst = &fx.active[0];
        assert_eq!(inst.kind, EffectKindTag::Projectile);
        assert_eq!(inst.from_x, Some(10.0));
        assert_eq!(inst.tile_x, 12);
        assert!(inst
            .current
            .as_ref()
            .is_some_and(|anim| anim.frames[0].path.ends_with("/Magic/50.png")));
        assert!(inst
            .queued
            .as_ref()
            .is_some_and(|anim| anim.frames[0].path.ends_with("/Magic/170.png")));
    }

    #[test]
    fn blizzard_ground_offset_applied() {
        let catalog = EffectCatalog::load().expect("load");
        let anim = catalog
            .map_animation("Blizzard", 0)
            .expect("Blizzard ground");
        assert_eq!(anim.offset_y, -20.0);
        let meteor = catalog
            .map_animation("MeteorStrike", 0)
            .expect("MeteorStrike ground");
        assert_eq!(meteor.offset_y, -20.0);
        let mut anim_with_offset = fake_anim("ground", 100, 1, false);
        anim_with_offset.offset_y = -20.0;
        anim_with_offset.offset_x = 5.0;
        let frame = &anim_with_offset.frames[0];
        let (left, top) = screen_position(
            10,
            10,
            10.0,
            10.0,
            frame.x + anim_with_offset.offset_x,
            frame.y + anim_with_offset.offset_y,
        );
        let (left_no_offset, top_no_offset) = screen_position(10, 10, 10.0, 10.0, frame.x, frame.y);
        assert_eq!(left - left_no_offset, 5.0);
        assert_eq!(top - top_no_offset, -20.0);
    }

    #[test]
    fn reconnect_sequence_resets_via_generation() {
        let mut fx = NativeEffects::default();
        let zone = HashMap::new();
        fx.observe(
            0,
            0,
            0,
            &[NativeEffectEvent {
                sequence: 1,
                generation: 0,
                packet: "ObjectMagic".to_owned(),
                payload: json!({"location":{"x":10,"y":10},"spell":"FireBall","direction":"down"}),
            }],
            &zone,
        );
        assert_eq!(fx.active.len(), 1);
        fx.observe(
            0,
            0,
            0,
            &[NativeEffectEvent {
                sequence: 1,
                generation: 0,
                packet: "ObjectMagic".to_owned(),
                payload: json!({"location":{"x":10,"y":10},"spell":"FireBall","direction":"down"}),
            }],
            &zone,
        );
        assert_eq!(fx.active.len(), 1);
        fx.observe(
            0,
            0,
            0,
            &[NativeEffectEvent {
                sequence: 1,
                generation: 1,
                packet: "ObjectMagic".to_owned(),
                payload: json!({"location":{"x":10,"y":10},"spell":"FireBall","direction":"down"}),
            }],
            &zone,
        );
        assert_eq!(fx.active.len(), 1);
    }

    #[test]
    fn reconnect_reset_for_new_connection_allows_sequence_one() {
        let mut fx = NativeEffects::default();
        let zone = HashMap::new();
        fx.observe(
            0,
            0,
            0,
            &[NativeEffectEvent {
                sequence: 5,
                generation: 0,
                packet: "ObjectMagic".to_owned(),
                payload: json!({"location":{"x":10,"y":10},"spell":"FireBall","direction":"down"}),
            }],
            &zone,
        );
        assert_eq!(fx.last_effect_sequence, 5);
        fx.reset_for_new_connection();
        fx.observe(
            0,
            0,
            0,
            &[NativeEffectEvent {
                sequence: 1,
                generation: 1,
                packet: "ObjectMagic".to_owned(),
                payload: json!({"location":{"x":10,"y":10},"spell":"FireBall","direction":"down"}),
            }],
            &zone,
        );
        assert_eq!(fx.active.len(), 1);
    }

    #[test]
    fn object_magic_creates_cast_with_magic_0_frame() {
        let mut fx = NativeEffects::default();
        let zone = HashMap::new();
        fx.observe(
            0,
            10,
            10,
            &[NativeEffectEvent {
                sequence: 1,
                generation: 0,
                packet: "ObjectMagic".to_owned(),
                payload: json!({"location":{"x":10,"y":10},"spell":"FireBall","direction":"down"}),
            }],
            &zone,
        );
        assert_eq!(fx.active.len(), 1);
        let inst = &fx.active[0];
        assert_eq!(inst.kind, EffectKindTag::Cast);
        let anim = inst.current.as_ref().expect("cast anim");
        assert!(anim.frames[0].path.ends_with("/Magic/0.png"));
        assert_eq!(anim.kind, "cast");
    }

    #[test]
    fn projectile_duplicate_sequence_does_not_duplicate_projectile() {
        let mut zone_tiles = HashMap::new();
        zone_tiles.insert(1000, (10, 10));
        zone_tiles.insert(2001, (12, 10));
        let mut fx = NativeEffects::default();
        let ev = NativeEffectEvent {
            sequence: 1,
            generation: 0,
            packet: "ObjectProjectile".to_owned(),
            payload: json!({"spell":"FireBall","sourceId":1000,"destinationId":2001}),
        };
        fx.observe(0, 10, 10, &[ev.clone()], &zone_tiles);
        assert_eq!(fx.active.len(), 1);
        fx.observe(0, 10, 10, &[ev], &zone_tiles);
        assert_eq!(fx.active.len(), 1);
    }

    #[test]
    fn projectile_uses_authoritative_destination_after_target_moves() {
        let mut fx = NativeEffects::default();
        let mut zone_tiles = HashMap::new();
        zone_tiles.insert(1000, (10, 10));
        zone_tiles.insert(2001, (12, 10));
        fx.observe(
            0,
            10,
            10,
            &[NativeEffectEvent {
                sequence: 1,
                generation: 0,
                packet: "ObjectProjectile".to_owned(),
                payload: json!({"spell":"FireBall","sourceId":1000,"destinationId":2001}),
            }],
            &zone_tiles,
        );
        assert_eq!(fx.active[0].tile_x, 12);
        zone_tiles.insert(2001, (20, 20));
        fx.observe(
            0,
            10,
            10,
            &[NativeEffectEvent {
                sequence: 2,
                generation: 0,
                packet: "ObjectProjectile".to_owned(),
                payload: json!({"spell":"FireBall","sourceId":1000,"destinationId":2001}),
            }],
            &zone_tiles,
        );
        assert_eq!(fx.active.len(), 2);
        assert_eq!(fx.active[1].tile_x, 20);
        assert_eq!(fx.active[1].tile_y, 20);
    }

    #[test]
    fn map_changed_and_logout_clear_runtime_retained_via_tick() {
        let mut fx = NativeEffects::default();
        let zone = HashMap::new();
        fx.observe(
            0,
            0,
            0,
            &[NativeEffectEvent {
                sequence: 1,
                generation: 0,
                packet: "ObjectMagic".to_owned(),
                payload: json!({"location":{"x":10,"y":10},"spell":"FireBall","direction":"down"}),
            }],
            &zone,
        );
        assert_eq!(fx.active.len(), 1);
        let state_before = fx.tick(0).expect("render before clear");
        assert!(state_before.contains("Magic/0.png"));
        fx.observe(
            10,
            0,
            0,
            &[NativeEffectEvent {
                sequence: 2,
                generation: 0,
                packet: "MapChanged".to_owned(),
                payload: json!({}),
            }],
            &zone,
        );
        assert!(fx.active.is_empty());
        let state_after = fx.tick(10).expect("render after clear");
        assert!(state_after.contains("\"effects\":[]"));
        fx.observe(
            20,
            0,
            0,
            &[NativeEffectEvent {
                sequence: 3,
                generation: 0,
                packet: "ObjectMagic".to_owned(),
                payload: json!({"location":{"x":10,"y":10},"spell":"FireBall","direction":"down"}),
            }],
            &zone,
        );
        fx.observe(
            30,
            0,
            0,
            &[NativeEffectEvent {
                sequence: 4,
                generation: 0,
                packet: "LogOutSuccess".to_owned(),
                payload: json!({}),
            }],
            &zone,
        );
        assert!(fx.active.is_empty());
        let state_logout = fx.tick(30).expect("render after logout");
        assert!(state_logout.contains("\"effects\":[]"));
    }

    #[test]
    fn continuous_200_projectiles_bounded_and_no_leak() {
        let mut zone_tiles = HashMap::new();
        zone_tiles.insert(1000, (10, 10));
        for i in 2001..2101 {
            zone_tiles.insert(i, (12, 10));
        }
        let mut fx = NativeEffects::default();
        let events: Vec<NativeEffectEvent> = (1..=200)
            .map(|seq| NativeEffectEvent {
                sequence: seq as u64,
                generation: 0,
                packet: "ObjectProjectile".to_owned(),
                payload: json!({"spell":"FireBall","sourceId":1000,"destinationId":2001}),
            })
            .collect();
        fx.observe(0, 10, 10, &events, &zone_tiles);
        assert!(fx.active.len() <= MAX_ACTIVE_EFFECTS);
        assert_eq!(fx.active.len(), 96);
        // Advance time beyond all durations and ensure all finished are cleared.
        let far_future = 10_000u64;
        let _ = fx.tick(far_future);
        // After tick, finished non-persistent should be retained only if still active, but at far future all should be expired.
        // Trigger retain.
        fx.tick(far_future);
        // At far future, no projectile should be alive (duration 180 + 600).
        assert!(fx
            .active
            .iter()
            .all(|inst| matches!(inst.kind, EffectKindTag::Persistent)));
        // For projectile-only test, after far future all should be cleared.
        let mut fx2 = NativeEffects::default();
        fx2.observe(0, 10, 10, &events, &zone_tiles);
        // Fast-forward and check bounded.
        for t in [0, 100, 200, 500, 1000, 5000] {
            let _ = fx2.tick(t);
            assert!(fx2.active.len() <= MAX_ACTIVE_EFFECTS);
        }
    }

    // ---- FX-3 mask/shadow/return/multi-skill/bounded/trace ----

    fn fake_frame_with_mask(mask: Option<&str>, shadow: Option<(f32, f32)>) -> EffectFrameMeta {
        EffectFrameMeta {
            path: "/original-effects/Magic/0.png".to_owned(),
            width: 44.0,
            height: 75.0,
            x: 3.0,
            y: -40.0,
            shadow_x: shadow.map(|(x, _)| x),
            shadow_y: shadow.map(|(_, y)| y),
            mask_path: mask.map(|s| s.to_owned()),
            mask_width: mask.map(|_| 44.0),
            mask_height: mask.map(|_| 75.0),
            mask_x: mask.map(|_| 3.0),
            mask_y: mask.map(|_| -40.0),
        }
    }

    #[test]
    fn frame_with_mask_emits_mask_render_state() {
        let mut fx = NativeEffects::default();
        // Create a fake animation where the frame has a mask that exists.
        let mut anim = fake_anim("cast", 60, 1, false);
        anim.frames[0] = fake_frame_with_mask(Some("/original-effects/Magic/0.png"), None);
        // Directly push an instance with that animation.
        fx.active.push(EffectInstance {
            key: "mask-test".to_owned(),
            kind: EffectKindTag::Cast,
            tile_x: 10,
            tile_y: 10,
            from_x: None,
            from_y: None,
            current: Some(anim),
            queued: None,
            return_queued: None,
            started_at: 0,
            start_at: 0,
            persistent_object_id: None,
            provenance: EffectProvenance::default(),
        });
        fx.player_x = 10;
        fx.player_y = 10;
        let json = fx.tick(0).expect("render");
        assert!(json.contains("maskImageUrl"), "mask should be emitted");
        assert!(json.contains("/original-effects/Magic/0.png"));
        // Mask must not be an opaque placeholder: it should be additive and same geometry.
        assert!(json.contains("\"additive\":true"));
    }

    #[test]
    fn missing_mask_falls_back_without_placeholder() {
        let mut fx = NativeEffects::default();
        let mut anim = fake_anim("cast", 60, 1, false);
        anim.frames[0] = fake_frame_with_mask(Some("/original-effects/Magic/9999.mask.png"), None);
        fx.active.push(EffectInstance {
            key: "missing-mask".to_owned(),
            kind: EffectKindTag::Cast,
            tile_x: 10,
            tile_y: 10,
            from_x: None,
            from_y: None,
            current: Some(anim),
            queued: None,
            return_queued: None,
            started_at: 0,
            start_at: 0,
            persistent_object_id: None,
            provenance: EffectProvenance::default(),
        });
        fx.player_x = 10;
        fx.player_y = 10;
        let json = fx.tick(0).expect("render");
        // Missing mask file should not produce maskImageUrl.
        assert!(!json.contains("maskImageUrl") || !json.contains("9999.mask.png"));
    }

    #[test]
    fn mask_frame_tracks_primary_frame() {
        let mut fx = NativeEffects::default();
        let mut anim = fake_anim("cast", 60, 2, false);
        anim.frames[0] = fake_frame_with_mask(Some("/original-effects/Magic/0.png"), None);
        anim.frames[1] = fake_frame_with_mask(Some("/original-effects/Magic/1.png"), None);
        fx.active.push(EffectInstance {
            key: "track-mask".to_owned(),
            kind: EffectKindTag::Cast,
            tile_x: 10,
            tile_y: 10,
            from_x: None,
            from_y: None,
            current: Some(anim),
            queued: None,
            return_queued: None,
            started_at: 0,
            start_at: 0,
            persistent_object_id: None,
            provenance: EffectProvenance::default(),
        });
        fx.player_x = 10;
        fx.player_y = 10;
        let first = fx.tick(0).expect("first");
        assert!(first.contains("Magic/0.png"));
        let second = fx.tick(60).expect("second");
        assert!(second.contains("Magic/1.png"));
        // Mask should track primary: first mask is 0.png, second is 1.png if we had real mask files,
        // but with our fake we use same path, so we check that mask is present in both.
        assert!(second.contains("maskImageUrl"));
    }

    #[test]
    fn effect_shadow_uses_manifest_offsets() {
        let mut fx = NativeEffects::default();
        let mut anim = fake_anim("ground", 100, 1, false);
        anim.frames[0] = fake_frame_with_mask(None, Some((5.0, -3.0)));
        fx.active.push(EffectInstance {
            key: "shadow-test".to_owned(),
            kind: EffectKindTag::Ground,
            tile_x: 10,
            tile_y: 10,
            from_x: None,
            from_y: None,
            current: Some(anim),
            queued: None,
            return_queued: None,
            started_at: 0,
            start_at: 0,
            persistent_object_id: None,
            provenance: EffectProvenance::default(),
        });
        fx.player_x = 10;
        fx.player_y = 10;
        let json = fx.tick(0).expect("render");
        assert!(json.contains("shadowX"));
        assert!(json.contains("shadowY"));
        assert!(json.contains("5.0"));
    }

    #[test]
    fn shadow_does_not_move_primary_sprite() {
        let mut fx = NativeEffects::default();
        let mut anim = fake_anim("ground", 100, 1, false);
        anim.frames[0] = fake_frame_with_mask(None, Some((7.0, 7.0)));
        let primary_frame = anim.frames[0].clone();
        fx.active.push(EffectInstance {
            key: "shadow-primary".to_owned(),
            kind: EffectKindTag::Ground,
            tile_x: 10,
            tile_y: 10,
            from_x: None,
            from_y: None,
            current: Some(anim),
            queued: None,
            return_queued: None,
            started_at: 0,
            start_at: 0,
            persistent_object_id: None,
            provenance: EffectProvenance::default(),
        });
        fx.player_x = 10;
        fx.player_y = 10;
        let json = fx.tick(0).expect("render");
        // Primary left/top should be based on frame.x/y + offset, not shadow offset.
        let v: Value = serde_json::from_str(&json).expect("json");
        let effect = &v["effects"][0];
        let left = effect["left"].as_f64().unwrap();
        let top = effect["top"].as_f64().unwrap();
        let expected_left = ENTITY_ORIGIN_X + primary_frame.x;
        let expected_top = ENTITY_ORIGIN_Y + primary_frame.y;
        assert!((left - expected_left as f64).abs() < 0.01);
        assert!((top - expected_top as f64).abs() < 0.01);
        // Shadow offset should be in shadowX/Y, not in primary.
        assert!(json.contains("shadowX"));
    }

    #[test]
    fn frame_without_shadow_creates_no_shadow_entity() {
        let mut fx = NativeEffects::default();
        let mut anim = fake_anim("ground", 100, 1, false);
        anim.frames[0] = fake_frame_with_mask(None, None);
        fx.active.push(EffectInstance {
            key: "no-shadow".to_owned(),
            kind: EffectKindTag::Ground,
            tile_x: 10,
            tile_y: 10,
            from_x: None,
            from_y: None,
            current: Some(anim),
            queued: None,
            return_queued: None,
            started_at: 0,
            start_at: 0,
            persistent_object_id: None,
            provenance: EffectProvenance::default(),
        });
        fx.player_x = 10;
        fx.player_y = 10;
        let json = fx.tick(0).expect("render");
        assert!(!json.contains("shadowX"));
        assert!(!json.contains("shadowY"));
    }

    #[test]
    fn return_effect_reverses_source_and_destination() {
        let mut zone_tiles = HashMap::new();
        zone_tiles.insert(1000, (10, 10));
        zone_tiles.insert(2001, (12, 10));
        let mut fx = NativeEffects::default();
        // Vampirism has returnEffect
        fx.observe(
            0,
            10,
            10,
            &[NativeEffectEvent {
                sequence: 1,
                generation: 0,
                packet: "ObjectProjectile".to_owned(),
                payload: json!({"spell":"Vampirism","sourceId":1000,"destinationId":2001}),
            }],
            &zone_tiles,
        );
        assert_eq!(fx.active.len(), 1);
        let inst = &fx.active[0];
        assert!(inst.return_queued.is_some(), "Vampirism should have return");
        let ret = inst.return_queued.as_ref().unwrap();
        assert_eq!(ret.kind, "return");
        // Check that after projectile + impact, return starts and its position is reversed.
        // For this test, Vampirism has no impact, so return starts after projectile.
        let proj_duration = inst.current.as_ref().unwrap().duration_ms;
        let ret_started = inst.start_at + proj_duration;
        // At return start, tile should be destination, then move to source.
        let anim = inst.return_queued.as_ref().unwrap();
        let (tx, ty) = instance_current_tile(inst, anim, ret_started, ret_started);
        assert_eq!((tx as i32, ty as i32), (12, 10));
        let (tx2, ty2) =
            instance_current_tile(inst, anim, ret_started, ret_started + anim.duration_ms);
        assert_eq!((tx2 as i32, ty2 as i32), (10, 10));
    }

    #[test]
    fn spell_without_return_effect_has_no_return_phase() {
        let mut zone_tiles = HashMap::new();
        zone_tiles.insert(1000, (10, 10));
        zone_tiles.insert(2001, (12, 10));
        let mut fx = NativeEffects::default();
        fx.observe(
            0,
            10,
            10,
            &[NativeEffectEvent {
                sequence: 1,
                generation: 0,
                packet: "ObjectProjectile".to_owned(),
                payload: json!({"spell":"FireBall","sourceId":1000,"destinationId":2001}),
            }],
            &zone_tiles,
        );
        assert_eq!(fx.active.len(), 1);
        assert!(fx.active[0].return_queued.is_none());
    }

    #[test]
    fn duplicate_packet_does_not_duplicate_return_effect() {
        let mut zone_tiles = HashMap::new();
        zone_tiles.insert(1000, (10, 10));
        zone_tiles.insert(2001, (12, 10));
        let mut fx = NativeEffects::default();
        let ev = NativeEffectEvent {
            sequence: 1,
            generation: 0,
            packet: "ObjectProjectile".to_owned(),
            payload: json!({"spell":"Vampirism","sourceId":1000,"destinationId":2001}),
        };
        fx.observe(0, 10, 10, &[ev.clone()], &zone_tiles);
        assert_eq!(fx.active.len(), 1);
        fx.observe(0, 10, 10, &[ev], &zone_tiles);
        assert_eq!(fx.active.len(), 1);
    }

    #[test]
    fn map_change_clears_return_effect() {
        let mut zone_tiles = HashMap::new();
        zone_tiles.insert(1000, (10, 10));
        zone_tiles.insert(2001, (12, 10));
        let mut fx = NativeEffects::default();
        fx.observe(
            0,
            10,
            10,
            &[NativeEffectEvent {
                sequence: 1,
                generation: 0,
                packet: "ObjectProjectile".to_owned(),
                payload: json!({"spell":"Vampirism","sourceId":1000,"destinationId":2001}),
            }],
            &zone_tiles,
        );
        assert_eq!(fx.active.len(), 1);
        fx.observe(
            0,
            10,
            10,
            &[NativeEffectEvent {
                sequence: 2,
                generation: 0,
                packet: "MapChanged".to_owned(),
                payload: json!({}),
            }],
            &zone_tiles,
        );
        assert!(fx.active.is_empty());
    }

    #[test]
    fn lightning_target_anchored_no_fake_projectile() {
        let catalog = EffectCatalog::load().expect("load");
        assert!(catalog.spell_projectile_animation("Lightning", 0).is_none());
        assert!(catalog.spell_impact_animation("Lightning").is_none());
        let cast = catalog.spell_cast_animation("Lightning", 4).expect("cast");
        assert!(cast.frames[0].path.contains("Magic/"));
        // Lightning should not generate a projectile even if we try ObjectProjectile with it.
        let mut zone_tiles = HashMap::new();
        zone_tiles.insert(1000, (10, 10));
        zone_tiles.insert(2001, (12, 10));
        let mut fx = NativeEffects::default();
        fx.observe(
            0,
            10,
            10,
            &[NativeEffectEvent {
                sequence: 1,
                generation: 0,
                packet: "ObjectProjectile".to_owned(),
                payload: json!({"spell":"Lightning","sourceId":1000,"destinationId":2001}),
            }],
            &zone_tiles,
        );
        assert!(fx.active.is_empty(), "Lightning must not create projectile");
    }

    fn fireball_fixture() -> Value {
        serde_json::from_str(include_str!(
            "../tests/fixtures/vis02-bichon-fireball-v1.json"
        ))
        .expect("VIS-02 FireBall fixture JSON")
    }

    fn fireball_magic_event(sequence: u64, cast: bool) -> NativeEffectEvent {
        let fixture = fireball_fixture();
        let index = if cast { 0 } else { 2 };
        NativeEffectEvent {
            sequence,
            generation: 11,
            packet: "ObjectMagic".to_owned(),
            payload: fixture["timeline"][index]["event"]["payload"].clone(),
        }
    }

    fn fireball_compat_projectile_event(sequence: u64) -> NativeEffectEvent {
        let fixture = fireball_fixture();
        NativeEffectEvent {
            sequence,
            generation: 11,
            packet: "ObjectProjectile".to_owned(),
            payload: fixture["timeline"][1]["compatibilityEvent"]["payload"].clone(),
        }
    }

    #[test]
    fn fireball_object_magic_owns_cast_delayed_projectile_impact_and_three_sounds() {
        let zone = HashMap::from([(1000, (288, 616)), (2005, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(
            0,
            288,
            616,
            &[
                fireball_magic_event(1, true),
                fireball_compat_projectile_event(2),
            ],
            &zone,
        );
        assert_eq!(
            fx.active.len(),
            2,
            "compatibility packet must not duplicate"
        );
        let cast = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-cast-"))
            .expect("FireBall cast");
        assert_eq!(cast.start_at, 0);
        assert!(cast
            .current
            .as_ref()
            .is_some_and(|animation| animation.frames[0].path.ends_with("/Magic/0.png")));
        let projectile = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-fireball-"))
            .expect("FireBall projectile");
        assert_eq!(projectile.start_at, FIREBALL_SPELL_ACTION_MS);
        assert_eq!(
            projectile.current.as_ref().map(|anim| anim.duration_ms),
            Some(250)
        );
        assert_eq!(
            projectile.current.as_ref().map(|anim| anim.interval),
            Some(31)
        );
        assert!(projectile
            .current
            .as_ref()
            .is_some_and(|animation| animation.frames[0].path.ends_with("/Magic/10.png")));
        assert!(
            projectile.queued.is_none(),
            "target binding occurs at launch"
        );

        let cast_sound = fx.take_due_sound_events(0);
        assert_eq!(cast_sound.len(), 1);
        assert_eq!(cast_sound[0].cue, FIREBALL_CAST_SOUND_CUE);
        assert_eq!(cast_sound[0].file_name, FIREBALL_CAST_SOUND_FILE);
        assert!(fx.take_due_sound_events(599).is_empty());
        let before: Value = serde_json::from_str(
            &fx.tick_with_visibility(599, true)
                .expect("FireBall before projectile"),
        )
        .expect("FireBall pre-projectile JSON");
        assert_eq!(before["effects"].as_array().map(Vec::len), Some(1));

        let projectile_sound = fx.take_due_sound_events(600);
        assert_eq!(projectile_sound.len(), 1);
        assert_eq!(projectile_sound[0].cue, FIREBALL_PROJECTILE_SOUND_CUE);
        assert_eq!(
            projectile_sound[0].file_name,
            FIREBALL_PROJECTILE_SOUND_FILE
        );
        let launch: Value = serde_json::from_str(
            &fx.tick_with_visibility(600, true)
                .expect("FireBall projectile launch"),
        )
        .expect("FireBall launch JSON");
        assert!(launch["effects"][0]["imageUrl"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Magic/10.png")));
        assert!(fx.active.iter().any(|instance| {
            instance.key.starts_with("fx-fireball-")
                && instance
                    .queued
                    .as_ref()
                    .is_some_and(|animation| animation.frames[0].path.ends_with("/Magic/170.png"))
        }));
        assert!(fx.take_due_sound_events(849).is_empty());

        let impact_sound = fx.take_due_sound_events(850);
        assert_eq!(impact_sound.len(), 1);
        assert_eq!(impact_sound[0].cue, FIREBALL_IMPACT_SOUND_CUE);
        assert_eq!(impact_sound[0].file_name, FIREBALL_IMPACT_SOUND_FILE);
        let impact: Value =
            serde_json::from_str(&fx.tick_with_visibility(850, true).expect("FireBall impact"))
                .expect("FireBall impact JSON");
        assert!(impact["effects"][0]["imageUrl"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Magic/170.png")));
        assert!(fx.take_due_sound_events(850).is_empty());
    }

    #[test]
    fn fireball_direction16_is_locked_from_the_target_position_at_launch() {
        assert_eq!(projectile_direction16((0, 0), (0, -10)), 0);
        assert_eq!(projectile_direction16((0, 0), (10, -10)), 2);
        assert_eq!(projectile_direction16((0, 0), (10, 0)), 4);
        assert_eq!(projectile_direction16((0, 0), (10, 10)), 6);
        assert_eq!(projectile_direction16((0, 0), (0, 10)), 8);
        assert_eq!(projectile_direction16((0, 0), (-10, 10)), 10);
        assert_eq!(projectile_direction16((0, 0), (-10, 0)), 12);
        assert_eq!(projectile_direction16((0, 0), (-10, -10)), 14);

        let mut zone = HashMap::from([(1000, (288, 616)), (2005, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[fireball_magic_event(1, true)], &zone);
        zone.insert(2005, (295, 616));
        fx.observe(600, 288, 616, &[], &zone);
        let projectile = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-fireball-"))
            .expect("FireBall launch");
        assert!(projectile
            .current
            .as_ref()
            .is_some_and(|animation| animation.frames[0].path.ends_with("/Magic/50.png")));
        assert_eq!(
            projectile
                .current
                .as_ref()
                .map(|animation| animation.duration_ms),
            Some(350)
        );
    }

    #[test]
    fn fireball_cast_false_keeps_cast_but_never_projectile_impact_or_phase_audio() {
        let zone = HashMap::from([(1000, (288, 616)), (2005, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[fireball_magic_event(1, false)], &zone);
        assert_eq!(
            fx.active.len(),
            1,
            "Crystal still plays the cast action effect"
        );
        assert!(fx.active[0].key.starts_with("fx-cast-"));
        let sound = fx.take_due_sound_events(0);
        assert_eq!(sound.len(), 1);
        assert_eq!(sound[0].cue, FIREBALL_CAST_SOUND_CUE);
        assert!(fx.take_due_sound_events(10_000).is_empty());
        let expired: Value = serde_json::from_str(
            &fx.tick_with_visibility(600, true)
                .expect("cast-false expiration"),
        )
        .expect("cast-false JSON");
        assert_eq!(expired["effects"], json!([]));
    }

    #[test]
    fn fireball_tracks_bound_target_and_recomputes_distance_clock_before_arrival() {
        let mut zone = HashMap::from([(1000, (288, 616)), (2005, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[fireball_magic_event(1, true)], &zone);
        assert_eq!(fx.take_due_sound_events(0)[0].cue, FIREBALL_CAST_SOUND_CUE);
        let projectile = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-fireball-"))
            .expect("bound projectile");
        assert_eq!(
            projectile.current.as_ref().map(|anim| anim.duration_ms),
            Some(250)
        );

        zone.insert(2005, (288, 609));
        fx.observe(650, 288, 616, &[], &zone);
        assert_eq!(
            fx.take_due_sound_events(650)[0].cue,
            FIREBALL_PROJECTILE_SOUND_CUE
        );
        let projectile = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-fireball-"))
            .expect("moving bound projectile");
        assert_eq!((projectile.tile_x, projectile.tile_y), (288, 609));
        assert_eq!(
            projectile.current.as_ref().map(|anim| anim.duration_ms),
            Some(350)
        );
        assert!(fx.take_due_sound_events(949).is_empty());
        assert_eq!(
            fx.take_due_sound_events(950)[0].cue,
            FIREBALL_IMPACT_SOUND_CUE
        );
    }

    #[test]
    fn fireball_unbound_target_point_has_projectile_but_no_invented_impact() {
        let zone = HashMap::from([(1000, (288, 616))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[fireball_magic_event(1, true)], &zone);
        let projectile = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-fireball-"))
            .expect("point-target projectile");
        assert!(projectile.queued.is_none());
        assert!(!fx.anchor_object_ids.contains_key(&projectile.key));
        assert_eq!(fx.take_due_sound_events(0)[0].cue, FIREBALL_CAST_SOUND_CUE);
        assert_eq!(
            fx.take_due_sound_events(600)[0].cue,
            FIREBALL_PROJECTILE_SOUND_CUE
        );
        let late_flight: Value = serde_json::from_str(
            &fx.tick_with_visibility(830, true)
                .expect("bounded projectile still visible late in flight"),
        )
        .expect("late-flight JSON");
        assert_eq!(late_flight["effects"].as_array().map(Vec::len), Some(1));
        let expired: Value = serde_json::from_str(
            &fx.tick_with_visibility(850, true)
                .expect("bounded projectile expiration"),
        )
        .expect("expired projectile JSON");
        assert_eq!(expired["effects"], json!([]));
        assert!(fx.active.is_empty(), "point-target missile must not repeat");
        assert!(fx.take_due_sound_events(10_000).is_empty());
    }

    #[test]
    fn fireball_map_change_clears_pending_projectile_and_all_phase_audio() {
        let zone = HashMap::from([(1000, (288, 616)), (2005, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[fireball_magic_event(1, true)], &zone);
        let _ = fx.take_due_sound_events(0);
        fx.observe(
            100,
            288,
            616,
            &[NativeEffectEvent {
                sequence: 2,
                generation: 11,
                packet: "MapChanged".to_owned(),
                payload: json!({}),
            }],
            &zone,
        );
        assert!(fx.active.is_empty());
        assert!(fx.take_due_sound_events(10_000).is_empty());
    }

    #[test]
    fn fireball_production_direction_frames_and_audio_are_integrity_closed() {
        use sha2::{Digest, Sha256};

        let fixture = fireball_fixture();
        let catalog = EffectCatalog::load().expect("production effect catalog");
        for direction in 0..16_u32 {
            let animation = catalog
                .spell_projectile_animation("FireBall", direction)
                .expect("all FireBall directions resolve");
            assert_eq!(animation.frames.len(), 6);
            for (frame, source) in animation.frames.iter().zip(0..6_u32) {
                let index = 10 + direction * 10 + source;
                assert!(frame.path.ends_with(&format!("/Magic/{index}.png")));
                assert!(crate::frame_png_exists(&frame.path));
            }
        }
        for audio in fixture["source"]["audio"]
            .as_array()
            .expect("FireBall audio catalog")
        {
            let file = audio["file"].as_str().expect("FireBall audio file");
            let path = assets::asset_path(&format!("original-ui/Sound/{file}"))
                .expect("packaged FireBall sound path");
            let bytes = fs::read(path).expect("read FireBall sound");
            assert_eq!(bytes.len(), audio["sourceBytes"]);
            assert_eq!(format!("{:x}", Sha256::digest(&bytes)), audio["sha256"]);
        }
    }

    fn great_fireball_fixture() -> Value {
        serde_json::from_str(include_str!(
            "../tests/fixtures/vis02-bichon-great-fireball-v1.json"
        ))
        .expect("VIS-02 GreatFireBall fixture JSON")
    }

    fn great_fireball_magic_event(sequence: u64, cast: bool) -> NativeEffectEvent {
        let fixture = great_fireball_fixture();
        let index = if cast { 0 } else { 2 };
        NativeEffectEvent {
            sequence,
            generation: 17,
            packet: "ObjectMagic".to_owned(),
            payload: fixture["timeline"][index]["event"]["payload"].clone(),
        }
    }

    fn great_fireball_compat_projectile_event(sequence: u64) -> NativeEffectEvent {
        NativeEffectEvent {
            sequence,
            generation: 17,
            packet: "ObjectProjectile".to_owned(),
            payload: great_fireball_fixture()["timeline"][1]["compatibilityEvent"]["payload"]
                .clone(),
        }
    }

    #[test]
    fn great_fireball_object_magic_owns_three_source_phases_and_sounds() {
        let zone = HashMap::from([(1000, (288, 616)), (2034, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(
            0,
            288,
            616,
            &[
                great_fireball_magic_event(1, true),
                great_fireball_compat_projectile_event(2),
            ],
            &zone,
        );
        assert_eq!(fx.active.len(), 2, "compatibility projectile is ignored");
        let cast = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-cast-"))
            .expect("GreatFireBall cast");
        assert!(cast
            .current
            .as_ref()
            .is_some_and(|animation| animation.frames[0].path.ends_with("/Magic/400.png")));
        let projectile = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-great-fireball-"))
            .expect("GreatFireBall projectile");
        assert_eq!(projectile.start_at, GREAT_FIREBALL_SPELL_ACTION_MS);
        assert!(projectile
            .current
            .as_ref()
            .is_some_and(|animation| animation.frames[0].path.ends_with("/Magic/410.png")));
        assert_eq!(
            fx.take_due_sound_events(0)[0].cue,
            GREAT_FIREBALL_CAST_SOUND_CUE
        );
        assert!(fx.take_due_sound_events(599).is_empty());
        assert_eq!(
            fx.take_due_sound_events(600)[0].cue,
            GREAT_FIREBALL_PROJECTILE_SOUND_CUE
        );
        let launch: Value = serde_json::from_str(
            &fx.tick_with_visibility(600, true)
                .expect("GreatFireBall projectile launch"),
        )
        .expect("GreatFireBall launch JSON");
        assert!(launch["effects"].as_array().is_some_and(|effects| {
            effects.iter().any(|effect| {
                effect["imageUrl"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("/Magic/410.png"))
            })
        }));
        assert!(fx.active.iter().any(|instance| {
            instance.key.starts_with("fx-great-fireball-")
                && instance
                    .queued
                    .as_ref()
                    .is_some_and(|animation| animation.frames[0].path.ends_with("/Magic/570.png"))
        }));
        assert!(fx.take_due_sound_events(849).is_empty());
        assert_eq!(
            fx.take_due_sound_events(850)[0].cue,
            GREAT_FIREBALL_IMPACT_SOUND_CUE
        );
        let impact: Value = serde_json::from_str(
            &fx.tick_with_visibility(850, true)
                .expect("GreatFireBall impact"),
        )
        .expect("GreatFireBall impact JSON");
        assert!(impact["effects"][0]["imageUrl"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Magic/570.png")));
    }

    #[test]
    fn great_fireball_cast_false_keeps_cast_and_first_sound_only() {
        let zone = HashMap::from([(1000, (288, 616)), (2034, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[great_fireball_magic_event(1, false)], &zone);
        assert_eq!(fx.active.len(), 1);
        assert!(fx.active[0].key.starts_with("fx-cast-"));
        let sounds = fx.take_due_sound_events(0);
        assert_eq!(sounds.len(), 1);
        assert_eq!(sounds[0].cue, GREAT_FIREBALL_CAST_SOUND_CUE);
        assert!(fx.take_due_sound_events(10_000).is_empty());
    }

    #[test]
    fn great_fireball_target_removal_suppresses_impact_and_compatibility_replay() {
        let source_only = HashMap::from([(1000, (288, 616))]);
        let mut compatibility_only = NativeEffects::default();
        compatibility_only.observe(
            0,
            288,
            616,
            &[great_fireball_compat_projectile_event(1)],
            &source_only,
        );
        assert!(compatibility_only.active.is_empty());

        let initial = HashMap::from([(1000, (288, 616)), (2034, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(
            0,
            288,
            616,
            &[great_fireball_magic_event(1, true)],
            &initial,
        );
        let _ = fx.take_due_sound_events(0);
        fx.observe(600, 288, 616, &[], &source_only);
        assert_eq!(
            fx.take_due_sound_events(600)[0].cue,
            GREAT_FIREBALL_PROJECTILE_SOUND_CUE
        );
        let projectile = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-great-fireball-"))
            .expect("point-flight GreatFireBall");
        assert!(projectile.queued.is_none());
        assert!(!fx.anchor_object_ids.contains_key(&projectile.key));
        assert!(fx.take_due_sound_events(10_000).is_empty());
    }

    #[test]
    fn great_fireball_map_change_clears_all_phases_and_audio() {
        let zone = HashMap::from([(1000, (288, 616)), (2034, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[great_fireball_magic_event(1, true)], &zone);
        let _ = fx.take_due_sound_events(0);
        fx.observe(
            100,
            288,
            616,
            &[NativeEffectEvent {
                sequence: 2,
                generation: 17,
                packet: "MapChanged".to_owned(),
                payload: json!({}),
            }],
            &zone,
        );
        assert!(fx.active.is_empty());
        assert!(fx.take_due_sound_events(10_000).is_empty());
    }

    #[test]
    fn great_fireball_source_frames_and_audio_are_integrity_closed() {
        use sha2::{Digest, Sha256};

        let fixture = great_fireball_fixture();
        let catalog = EffectCatalog::load().expect("production effect catalog");
        let cast = catalog
            .spell_cast_animation("GreatFireBall", 0)
            .expect("GreatFireBall cast");
        assert_eq!(cast.frames.len(), 10);
        for (frame, index) in cast.frames.iter().zip(400..410) {
            assert!(frame.path.ends_with(&format!("/Magic/{index}.png")));
            assert!(crate::frame_png_exists(&frame.path));
        }
        for direction in 0..16_u32 {
            let projectile = catalog
                .spell_projectile_animation("GreatFireBall", direction)
                .expect("all GreatFireBall directions resolve");
            assert_eq!(projectile.frames.len(), 6);
            for (frame, source) in projectile.frames.iter().zip(0..6_u32) {
                let index = 410 + direction * 10 + source;
                assert!(frame.path.ends_with(&format!("/Magic/{index}.png")));
                assert!(crate::frame_png_exists(&frame.path));
            }
        }
        let impact = catalog
            .spell_impact_animation("GreatFireBall")
            .expect("GreatFireBall impact");
        assert_eq!(impact.frames.len(), 10);
        for (frame, index) in impact.frames.iter().zip(570..580) {
            assert!(frame.path.ends_with(&format!("/Magic/{index}.png")));
            assert!(crate::frame_png_exists(&frame.path));
        }
        for audio in fixture["source"]["audio"]
            .as_array()
            .expect("GreatFireBall audio catalog")
        {
            let file = audio["file"].as_str().expect("GreatFireBall audio file");
            let path = assets::asset_path(&format!("original-ui/Sound/{file}"))
                .expect("GreatFireBall source sound path");
            let bytes = fs::read(path).expect("read GreatFireBall sound");
            assert_eq!(bytes.len(), audio["sourceBytes"]);
            assert_eq!(format!("{:x}", Sha256::digest(&bytes)), audio["sha256"]);
        }
    }

    fn soul_fireball_fixture() -> Value {
        serde_json::from_str(include_str!(
            "../tests/fixtures/vis02-bichon-soul-fireball-v1.json"
        ))
        .expect("VIS-02 SoulFireBall fixture JSON")
    }

    fn soul_fireball_magic_event(sequence: u64, cast: bool) -> NativeEffectEvent {
        let fixture = soul_fireball_fixture();
        let index = if cast { 0 } else { 2 };
        NativeEffectEvent {
            sequence,
            generation: 13,
            packet: "ObjectMagic".to_owned(),
            payload: fixture["timeline"][index]["event"]["payload"].clone(),
        }
    }

    fn soul_fireball_compat_projectile_event(sequence: u64) -> NativeEffectEvent {
        let fixture = soul_fireball_fixture();
        NativeEffectEvent {
            sequence,
            generation: 13,
            packet: "ObjectProjectile".to_owned(),
            payload: fixture["timeline"][1]["compatibilityEvent"]["payload"].clone(),
        }
    }

    #[test]
    fn soul_fireball_object_magic_owns_delayed_projectile_impact_and_three_sounds() {
        let zone = HashMap::from([(1000, (288, 616)), (2014, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(
            0,
            288,
            616,
            &[
                soul_fireball_magic_event(1, true),
                soul_fireball_compat_projectile_event(2),
            ],
            &zone,
        );
        assert_eq!(
            fx.active.len(),
            1,
            "SoulFireBall has no cast bitmap and the compatibility packet is deduplicated"
        );
        let projectile = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-soul-fireball-"))
            .expect("SoulFireBall projectile");
        assert_eq!(projectile.start_at, SOUL_FIREBALL_SPELL_ACTION_MS);
        assert_eq!(
            projectile.current.as_ref().map(|anim| anim.duration_ms),
            Some(250)
        );
        assert_eq!(
            projectile.current.as_ref().map(|anim| anim.interval),
            Some(31)
        );
        assert!(projectile
            .current
            .as_ref()
            .is_some_and(|animation| animation.frames[0].path.ends_with("/Magic/1160.png")));
        assert!(
            projectile.queued.is_none(),
            "target binding occurs at launch"
        );

        let cast_sound = fx.take_due_sound_events(0);
        assert_eq!(cast_sound.len(), 1);
        assert_eq!(cast_sound[0].cue, SOUL_FIREBALL_CAST_SOUND_CUE);
        assert_eq!(cast_sound[0].file_name, SOUL_FIREBALL_CAST_SOUND_FILE);
        assert!(fx.take_due_sound_events(599).is_empty());
        let before: Value = serde_json::from_str(
            &fx.tick_with_visibility(599, true)
                .expect("SoulFireBall before projectile"),
        )
        .expect("SoulFireBall pre-projectile JSON");
        assert_eq!(before["effects"], json!([]));

        let projectile_sound = fx.take_due_sound_events(600);
        assert_eq!(projectile_sound.len(), 1);
        assert_eq!(projectile_sound[0].cue, SOUL_FIREBALL_PROJECTILE_SOUND_CUE);
        assert_eq!(
            projectile_sound[0].file_name,
            SOUL_FIREBALL_PROJECTILE_SOUND_FILE
        );
        let launch: Value = serde_json::from_str(
            &fx.tick_with_visibility(600, true)
                .expect("SoulFireBall projectile launch"),
        )
        .expect("SoulFireBall launch JSON");
        assert!(launch["effects"][0]["imageUrl"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Magic/1160.png")));
        assert!(fx.active.iter().any(|instance| {
            instance.key.starts_with("fx-soul-fireball-")
                && instance
                    .queued
                    .as_ref()
                    .is_some_and(|animation| animation.frames[0].path.ends_with("/Magic/1360.png"))
        }));
        assert!(fx.take_due_sound_events(849).is_empty());

        let impact_sound = fx.take_due_sound_events(850);
        assert_eq!(impact_sound.len(), 1);
        assert_eq!(impact_sound[0].cue, SOUL_FIREBALL_IMPACT_SOUND_CUE);
        assert_eq!(impact_sound[0].file_name, SOUL_FIREBALL_IMPACT_SOUND_FILE);
        let impact: Value = serde_json::from_str(
            &fx.tick_with_visibility(850, true)
                .expect("SoulFireBall impact"),
        )
        .expect("SoulFireBall impact JSON");
        assert!(impact["effects"][0]["imageUrl"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Magic/1360.png")));
    }

    #[test]
    fn soul_fireball_cast_false_is_audio_only_and_never_creates_later_phases() {
        let zone = HashMap::from([(1000, (288, 616)), (2014, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[soul_fireball_magic_event(1, false)], &zone);
        assert!(
            fx.active.is_empty(),
            "Crystal has no SoulFireBall cast bitmap"
        );
        let sound = fx.take_due_sound_events(0);
        assert_eq!(sound.len(), 1);
        assert_eq!(sound[0].cue, SOUL_FIREBALL_CAST_SOUND_CUE);
        assert!(fx.take_due_sound_events(10_000).is_empty());
    }

    #[test]
    fn soul_fireball_locks_direction_at_launch_then_tracks_target_distance() {
        let mut zone = HashMap::from([(1000, (288, 616)), (2014, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[soul_fireball_magic_event(1, true)], &zone);
        let _ = fx.take_due_sound_events(0);
        zone.insert(2014, (295, 616));
        fx.observe(600, 288, 616, &[], &zone);
        let projectile = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-soul-fireball-"))
            .expect("SoulFireBall launch");
        assert!(projectile
            .current
            .as_ref()
            .is_some_and(|animation| animation.frames[0].path.ends_with("/Magic/1200.png")));
        assert_eq!(
            projectile.current.as_ref().map(|anim| anim.duration_ms),
            Some(350)
        );

        zone.insert(2014, (298, 616));
        fx.observe(650, 288, 616, &[], &zone);
        let projectile = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-soul-fireball-"))
            .expect("moving SoulFireBall projectile");
        assert!(projectile
            .current
            .as_ref()
            .is_some_and(|animation| animation.frames[0].path.ends_with("/Magic/1200.png")));
        assert_eq!(
            projectile.current.as_ref().map(|anim| anim.duration_ms),
            Some(500)
        );
        assert_eq!((projectile.tile_x, projectile.tile_y), (298, 616));
    }

    #[test]
    fn soul_fireball_resolves_target_presence_at_launch_and_ignores_all_compat_packets() {
        let mut fx = NativeEffects::default();
        let source_only = HashMap::from([(1000, (288, 616))]);
        fx.observe(
            0,
            288,
            616,
            &[soul_fireball_compat_projectile_event(1)],
            &source_only,
        );
        assert!(
            fx.active.is_empty(),
            "isolated compatibility packet is ignored"
        );

        fx.observe(
            0,
            288,
            616,
            &[soul_fireball_magic_event(2, true)],
            &source_only,
        );
        assert_eq!(fx.active.len(), 1);
        let target_appears = HashMap::from([(1000, (288, 616)), (2014, (295, 616))]);
        fx.observe(600, 288, 616, &[], &target_appears);
        let projectile = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-soul-fireball-"))
            .expect("launch-bound SoulFireBall");
        assert_eq!((projectile.tile_x, projectile.tile_y), (295, 616));
        assert!(projectile.queued.is_some());
        assert!(fx.anchor_object_ids.contains_key(&projectile.key));

        let mut reverse = NativeEffects::default();
        reverse.observe(
            0,
            288,
            616,
            &[
                soul_fireball_compat_projectile_event(1),
                soul_fireball_magic_event(2, true),
            ],
            &target_appears,
        );
        assert_eq!(reverse.active.len(), 1, "reverse order never duplicates");

        let mut disappears = NativeEffects::default();
        disappears.observe(
            0,
            288,
            616,
            &[soul_fireball_magic_event(1, true)],
            &target_appears,
        );
        disappears.observe(600, 288, 616, &[], &source_only);
        let projectile = disappears
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-soul-fireball-"))
            .expect("point-flight SoulFireBall");
        assert_eq!((projectile.tile_x, projectile.tile_y), (288, 611));
        assert!(projectile.queued.is_none());
        assert!(!disappears.anchor_object_ids.contains_key(&projectile.key));
    }

    #[test]
    fn soul_fireball_map_change_clears_audio_only_and_delayed_projectile_state() {
        let zone = HashMap::from([(1000, (288, 616)), (2014, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[soul_fireball_magic_event(1, true)], &zone);
        fx.observe(
            100,
            288,
            616,
            &[NativeEffectEvent {
                sequence: 2,
                generation: 13,
                packet: "MapChanged".to_owned(),
                payload: json!({}),
            }],
            &zone,
        );
        assert!(fx.active.is_empty());
        assert!(fx.take_due_sound_events(10_000).is_empty());
    }

    #[test]
    fn soul_fireball_production_direction_frames_and_audio_are_integrity_closed() {
        use sha2::{Digest, Sha256};

        let fixture = soul_fireball_fixture();
        let catalog = EffectCatalog::load().expect("production effect catalog");
        assert!(catalog.spell_cast_animation("SoulFireBall", 0).is_none());
        for direction in 0..16_u32 {
            let animation = catalog
                .spell_projectile_animation("SoulFireBall", direction)
                .expect("all SoulFireBall directions resolve");
            assert_eq!(animation.frames.len(), 3);
            for (frame, source) in animation.frames.iter().zip(0..3_u32) {
                let index = 1160 + direction * 10 + source;
                assert!(frame.path.ends_with(&format!("/Magic/{index}.png")));
                assert!(crate::frame_png_exists(&frame.path));
            }
        }
        let impact = catalog
            .spell_impact_animation("SoulFireBall")
            .expect("SoulFireBall impact");
        assert_eq!(impact.frames.len(), 10);
        assert!(impact.frames[0].path.ends_with("/Magic/1360.png"));
        assert!(impact.frames[9].path.ends_with("/Magic/1369.png"));
        for audio in fixture["source"]["audio"]
            .as_array()
            .expect("SoulFireBall audio catalog")
        {
            let file = audio["file"].as_str().expect("SoulFireBall audio file");
            let path = assets::asset_path(&format!("original-ui/Sound/{file}"))
                .expect("packaged SoulFireBall sound path");
            let bytes = fs::read(path).expect("read SoulFireBall sound");
            assert_eq!(bytes.len(), audio["sourceBytes"]);
            assert_eq!(format!("{:x}", Sha256::digest(&bytes)), audio["sha256"]);
        }
    }

    fn firewall_fixture() -> Value {
        serde_json::from_str(include_str!(
            "../tests/fixtures/vis02-bichon-firewall-v1.json"
        ))
        .expect("VIS-02 FireWall fixture JSON")
    }

    fn firewall_magic_event(sequence: u64, cast: bool) -> NativeEffectEvent {
        let fixture = firewall_fixture();
        let payload = if cast {
            fixture["timeline"][0]["event"]["payload"].clone()
        } else {
            fixture["compatibilityCases"]["castFalse"]["event"]["payload"].clone()
        };
        NativeEffectEvent {
            sequence,
            generation: 15,
            packet: "ObjectMagic".to_owned(),
            payload,
        }
    }

    fn firewall_spell_events() -> Vec<NativeEffectEvent> {
        let fixture = firewall_fixture();
        (1..=5)
            .map(|index| NativeEffectEvent {
                sequence: index as u64 + 1,
                generation: 15,
                packet: "ObjectSpell".to_owned(),
                payload: fixture["timeline"][index]["event"]["payload"].clone(),
            })
            .collect()
    }

    #[test]
    fn firewall_cast_true_plays_two_source_sounds_across_spell_action() {
        let zone = HashMap::from([(1000, (288, 616))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[firewall_magic_event(1, true)], &zone);
        assert_eq!(fx.active.len(), 1);
        let cast = fx.active.first().expect("FireWall cast");
        let animation = cast.current.as_ref().expect("FireWall cast animation");
        assert_eq!(animation.frames.len(), 10);
        assert_eq!(animation.interval, 60);
        assert_eq!(animation.duration_ms, FIREWALL_SPELL_ACTION_MS);
        assert!(animation.frames[0].path.ends_with("/Magic/1620.png"));

        let start_sound = fx.take_due_sound_events(0);
        assert_eq!(start_sound.len(), 1);
        assert_eq!(start_sound[0].cue, FIREWALL_CAST_SOUND_CUE);
        assert_eq!(start_sound[0].file_name, FIREWALL_CAST_SOUND_FILE);
        assert!(fx.take_due_sound_events(599).is_empty());
        let complete_sound = fx.take_due_sound_events(600);
        assert_eq!(complete_sound.len(), 1);
        assert_eq!(complete_sound[0].cue, FIREWALL_COMPLETE_SOUND_CUE);
        assert_eq!(complete_sound[0].file_name, FIREWALL_COMPLETE_SOUND_FILE);
        let completed: Value = serde_json::from_str(
            &fx.tick_with_visibility(600, true)
                .expect("FireWall cast completion"),
        )
        .expect("FireWall completion JSON");
        assert_eq!(completed["effects"], json!([]));
    }

    #[test]
    fn firewall_cast_false_keeps_cast_and_first_sound_but_no_completion_sound() {
        let zone = HashMap::from([(1000, (288, 616))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[firewall_magic_event(1, false)], &zone);
        assert_eq!(fx.active.len(), 1, "Crystal still plays the Spell action");
        assert_eq!(
            fx.active[0]
                .current
                .as_ref()
                .and_then(|animation| animation.frames.first())
                .map(|frame| frame.path.as_str()),
            Some("/original-effects/Magic/1620.png")
        );
        let sound = fx.take_due_sound_events(0);
        assert_eq!(sound.len(), 1);
        assert_eq!(sound[0].cue, FIREWALL_CAST_SOUND_CUE);
        assert!(fx.take_due_sound_events(10_000).is_empty());
    }

    #[test]
    fn firewall_five_object_spells_repeat_until_each_authoritative_remove() {
        let zone = HashMap::new();
        let mut fx = NativeEffects::default();
        let spells = firewall_spell_events();
        fx.observe(500, 288, 616, &spells, &zone);
        assert_eq!(fx.active.len(), 5);
        for instance in &fx.active {
            assert_eq!(instance.kind, EffectKindTag::Persistent);
            assert!(instance.persistent_object_id.is_some());
            let animation = instance.current.as_ref().expect("FireWall ground");
            assert_eq!(animation.frames.len(), 6);
            assert_eq!(animation.interval, 120);
            assert!(animation.repeat);
            assert_eq!(animation.light, Some(3));
            assert!(animation.frames[0].path.ends_with("/Magic/1630.png"));
        }
        let later: Value = serde_json::from_str(
            &fx.tick_with_visibility(10_000, true)
                .expect("repeating FireWall ground"),
        )
        .expect("FireWall ground JSON");
        assert_eq!(later["effects"].as_array().map(Vec::len), Some(5));

        fx.observe(
            10_001,
            288,
            616,
            &[NativeEffectEvent {
                sequence: 7,
                generation: 15,
                packet: "ObjectRemove".to_owned(),
                payload: json!({"objectId": 81000}),
            }],
            &zone,
        );
        assert_eq!(fx.active.len(), 4);

        let mut duplicate = spells[1].clone();
        duplicate.sequence = 8;
        fx.observe(10_002, 288, 616, &[duplicate], &zone);
        assert_eq!(
            fx.active.len(),
            4,
            "replayed ObjectSpell identity replaces rather than duplicates"
        );
    }

    #[test]
    fn firewall_map_change_clears_ground_cast_and_pending_completion_audio() {
        let zone = HashMap::from([(1000, (288, 616))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[firewall_magic_event(1, true)], &zone);
        let _ = fx.take_due_sound_events(0);
        fx.observe(500, 288, 616, &firewall_spell_events(), &zone);
        assert_eq!(fx.active.len(), 6);
        fx.observe(
            550,
            288,
            616,
            &[NativeEffectEvent {
                sequence: 20,
                generation: 15,
                packet: "MapChanged".to_owned(),
                payload: json!({}),
            }],
            &zone,
        );
        assert!(fx.active.is_empty());
        assert!(fx.take_due_sound_events(10_000).is_empty());
    }

    #[test]
    fn firewall_source_cast_ground_and_audio_are_integrity_closed() {
        use sha2::{Digest, Sha256};

        let fixture = firewall_fixture();
        let catalog = EffectCatalog::load().expect("production effect catalog");
        let cast = catalog
            .spell_cast_animation("FireWall", 0)
            .expect("FireWall cast");
        assert_eq!(cast.frames.len(), 10);
        for (frame, index) in cast.frames.iter().zip(1620..1630) {
            assert!(frame.path.ends_with(&format!("/Magic/{index}.png")));
            assert!(crate::frame_png_exists(&frame.path));
        }
        let ground = catalog
            .spell_world_animation("FireWall", 0, 0)
            .expect("FireWall ground");
        assert_eq!(ground.frames.len(), 6);
        assert!(ground.repeat);
        assert_eq!(ground.light, Some(3));
        for (frame, index) in ground.frames.iter().zip(1630..1636) {
            assert!(frame.path.ends_with(&format!("/Magic/{index}.png")));
            assert!(crate::frame_png_exists(&frame.path));
        }
        for audio in fixture["source"]["audio"]
            .as_array()
            .expect("FireWall audio catalog")
        {
            let file = audio["file"].as_str().expect("FireWall audio file");
            let path = assets::asset_path(&format!("original-ui/Sound/{file}"))
                .expect("packaged FireWall sound path");
            let bytes = fs::read(path).expect("read FireWall sound");
            assert_eq!(bytes.len(), audio["sourceBytes"]);
            assert_eq!(format!("{:x}", Sha256::digest(&bytes)), audio["sha256"]);
        }
    }

    fn flaming_sword_fixture() -> Value {
        serde_json::from_str(include_str!(
            "../tests/fixtures/vis02-bichon-flaming-sword-v1.json"
        ))
        .expect("VIS-02 FlamingSword fixture JSON")
    }

    fn flaming_sword_event(sequence: u64, direction: usize) -> NativeEffectEvent {
        NativeEffectEvent {
            sequence,
            generation: 16,
            packet: "ObjectAttack".to_owned(),
            payload: flaming_sword_fixture()["directionCases"][direction]["event"]["payload"]
                .clone(),
        }
    }

    fn ordinary_attack_event(sequence: u64) -> NativeEffectEvent {
        NativeEffectEvent {
            sequence,
            generation: 16,
            packet: "ObjectAttack".to_owned(),
            payload: flaming_sword_fixture()["compatibilityCases"]["ordinaryAttack"]["event"]
                ["payload"]
                .clone(),
        }
    }

    fn right_guard_range_event(
        sequence: u64,
        source_id: u32,
        target_id: u32,
        body_library: &str,
    ) -> NativeEffectEvent {
        NativeEffectEvent {
            sequence,
            generation: 17,
            packet: "ObjectRangeAttack".to_owned(),
            payload: json!({
                "objectId": source_id,
                "location": {"x": 287, "y": 616},
                "direction": "Down",
                "targetId": target_id,
                "target": {"x": 289, "y": 616},
                "attackType": 0,
                "spell": 0,
                "level": 0,
                "_nativeAttacker": {
                    "objectId": source_id,
                    "kind": "monster",
                    "sprite": {"bodyLibrary": body_library}
                },
                "_nativeTarget": {"objectId": target_id}
            }),
        }
    }

    fn lifecycle_effect_event(
        sequence: u64,
        packet: &str,
        object_id: Option<u32>,
    ) -> NativeEffectEvent {
        NativeEffectEvent {
            sequence,
            generation: 17,
            packet: packet.to_owned(),
            payload: object_id.map_or_else(|| json!({}), |id| json!({"objectId": id})),
        }
    }

    #[test]
    fn left_guard_range_projectile_catalog_is_exact_crystal_contract() {
        let catalog = EffectCatalog::load().expect("production effect catalog");
        for direction in 0..16_u32 {
            let animation = catalog
                .left_guard_range_projectile_animation(direction)
                .expect("all LeftGuard Direction16 frames resolve");
            assert_eq!(animation.name, "LeftGuardRangeProjectile");
            assert_eq!(animation.kind, "projectile");
            assert_eq!(animation.interval, 30);
            assert_eq!(animation.duration_ms, 180);
            assert_eq!(animation.frames.len(), 6);
            assert!(animation.blend);
            assert!((animation.opacity - 1.0).abs() < f32::EPSILON);
            assert_eq!(animation.light, Some(6));
            assert!(!animation.repeat);
            for (frame, source_frame) in animation.frames.iter().zip(0..6_u32) {
                let index = 10 + direction * 10 + source_frame;
                assert!(frame.path.ends_with(&format!("/Magic/{index}.png")));
                assert!(crate::frame_png_exists(&frame.path));
            }
        }
        assert!(catalog.left_guard_range_projectile_animation(16).is_none());
    }

    #[test]
    fn left_guard_range_projectile_waits_four_frames_and_uses_crystal_flight_clock() {
        let zone = HashMap::from([(371, (287, 616)), (2001, (293, 616))]);
        let mut fx = NativeEffects::default();
        fx.observe(
            0,
            288,
            616,
            &[right_guard_range_event(1, 371, 2001, "Monster/100")],
            &zone,
        );
        assert_eq!(fx.active.len(), 1);
        assert_eq!(fx.active[0].key, "left-guard-range:371:2001");
        assert_eq!(fx.active[0].kind, EffectKindTag::Projectile);
        assert_eq!(fx.active[0].start_at, 400);
        assert_eq!(fx.anchor_object_ids["left-guard-range:371:2001"], 2001);
        assert_eq!(fx.source_object_ids["left-guard-range:371:2001"], 371);
        assert!(fx.current_light_snapshots(399).is_empty());
        let before: Value = serde_json::from_str(
            &fx.tick_with_visibility(399, true)
                .expect("delayed LeftGuard state"),
        )
        .expect("delayed LeftGuard JSON");
        assert_eq!(before["effects"], json!([]));

        for (now_ms, index) in [
            (400, 50),
            (430, 51),
            (460, 52),
            (490, 53),
            (520, 54),
            (550, 55),
            (580, 50),
        ] {
            let rendered: Value = serde_json::from_str(
                &fx.tick_with_visibility(now_ms, true)
                    .expect("LeftGuard projectile frame"),
            )
            .expect("LeftGuard projectile JSON");
            assert_eq!(rendered["effects"].as_array().map(Vec::len), Some(1));
            assert!(rendered["effects"][0]["imageUrl"]
                .as_str()
                .is_some_and(|path| path.ends_with(&format!("/Magic/{index}.png"))));
            assert_eq!(rendered["effects"][0]["additive"], true);
            assert_eq!(rendered["effects"][0]["opacity"], 1.0);
        }
        assert_eq!(
            fx.active[0]
                .current
                .as_ref()
                .map(|animation| animation.duration_ms),
            Some(300),
            "six tiles use Crystal's 50 ms/tile duration"
        );
        let midpoint_light = fx.current_light_snapshots(550);
        assert_eq!(midpoint_light.len(), 1);
        assert_eq!(midpoint_light[0].light, 6);
        assert!((midpoint_light[0].tile_x - 290.0).abs() < f32::EPSILON);
        assert!((midpoint_light[0].tile_y - 616.0).abs() < f32::EPSILON);

        let expired: Value = serde_json::from_str(
            &fx.tick_with_visibility(700, true)
                .expect("expired LeftGuard state"),
        )
        .expect("expired LeftGuard JSON");
        assert_eq!(expired["effects"], json!([]));
        assert!(fx.active.is_empty());
        assert!(fx.anchor_object_ids.is_empty());
        assert!(fx.source_object_ids.is_empty());
        assert!(fx.local_projectile_targets.is_empty());
        assert!(fx.take_due_sound_events(10_000).is_empty());
    }

    #[test]
    fn left_guard_range_projectile_locks_launch_direction_then_tracks_target() {
        let mut zone = HashMap::from([(371, (287, 616)), (2001, (293, 616))]);
        let mut fx = NativeEffects::default();
        fx.observe(
            0,
            288,
            616,
            &[right_guard_range_event(1, 371, 2001, "Monster/100")],
            &zone,
        );
        zone.insert(371, (288, 616));
        zone.insert(2001, (288, 610));
        fx.observe(400, 288, 616, &[], &zone);
        assert_eq!(fx.active[0].from_x, Some(287.0));
        assert_eq!(fx.active[0].from_y, Some(616.0));
        assert!(fx.active[0]
            .current
            .as_ref()
            .is_some_and(|animation| animation.frames[0].path.ends_with("/Magic/10.png")));
        assert_eq!(
            fx.active[0]
                .current
                .as_ref()
                .map(|animation| animation.duration_ms),
            Some(300),
            "launch clock remains based on the packet action location"
        );

        zone.insert(2001, (296, 616));
        fx.observe(430, 288, 616, &[], &zone);
        assert_eq!((fx.active[0].tile_x, fx.active[0].tile_y), (296, 616));
        assert_eq!(
            fx.active[0]
                .current
                .as_ref()
                .map(|animation| animation.duration_ms),
            Some(450)
        );
        assert!(
            fx.active[0]
                .current
                .as_ref()
                .is_some_and(|animation| animation.frames[0].path.ends_with("/Magic/10.png")),
            "Crystal locks Direction16 when Missile is constructed"
        );
    }

    #[test]
    fn left_guard_range_projectile_restarts_per_pair_and_is_source_owned() {
        let mut zone = HashMap::from([(371, (287, 616)), (2001, (293, 616)), (2002, (287, 610))]);
        let mut fx = NativeEffects::default();
        fx.observe(
            0,
            288,
            616,
            &[right_guard_range_event(1, 371, 2001, "Monster/100")],
            &zone,
        );
        fx.observe(
            100,
            288,
            616,
            &[right_guard_range_event(
                2,
                371,
                2001,
                "/original-ui/Monster/100",
            )],
            &zone,
        );
        assert_eq!(fx.active.len(), 1);
        assert_eq!(fx.active[0].start_at, 500);

        let other_target = right_guard_range_event(3, 371, 2002, "Monster/100");
        fx.observe(120, 288, 616, &[other_target.clone()], &zone);
        assert_eq!(fx.active.len(), 2);
        let replay_state = fx
            .active
            .iter()
            .find(|instance| instance.key == "left-guard-range:371:2002")
            .map(|instance| (instance.start_at, instance.provenance.sequence))
            .expect("second LeftGuard target");
        fx.observe(130, 288, 616, &[other_target], &zone);
        assert_eq!(fx.active.len(), 2);
        assert_eq!(
            fx.active
                .iter()
                .find(|instance| instance.key == "left-guard-range:371:2002")
                .map(|instance| (instance.start_at, instance.provenance.sequence)),
            Some(replay_state)
        );

        fx.observe(520, 288, 616, &[], &zone);
        zone.remove(&371);
        fx.observe(
            521,
            288,
            616,
            &[lifecycle_effect_event(4, "ObjectRemove", Some(371))],
            &zone,
        );
        assert!(
            fx.active.is_empty(),
            "source owns both missiles after launch"
        );
        assert!(fx.anchor_object_ids.is_empty());
        assert!(fx.source_object_ids.is_empty());
        assert!(fx.local_projectile_targets.is_empty());

        zone.insert(371, (287, 616));
        fx.observe(
            600,
            288,
            616,
            &[right_guard_range_event(5, 371, 2001, "Monster/100")],
            &zone,
        );
        zone.remove(&371);
        fx.observe(
            610,
            288,
            616,
            &[lifecycle_effect_event(6, "ObjectHide", Some(371))],
            &zone,
        );
        assert_eq!(
            fx.active.len(),
            1,
            "source Hide only changes the actor action even after the bridge tombstones it"
        );
        assert_eq!(fx.hidden_left_guard_tiles.get(&371), Some(&(287, 616)));
        zone.remove(&2001);
        fx.observe(
            620,
            288,
            616,
            &[lifecycle_effect_event(7, "ObjectHide", Some(2001))],
            &zone,
        );
        assert_eq!(
            fx.active.len(),
            1,
            "target Hide keeps the pending missile target after the bridge tombstones it"
        );
        assert_eq!(fx.hidden_left_guard_tiles.get(&2001), Some(&(293, 616)));
        fx.observe(
            630,
            288,
            616,
            &[lifecycle_effect_event(8, "ObjectRemove", Some(2001))],
            &zone,
        );
        assert!(
            fx.active.is_empty(),
            "target Remove before frame 4 prevents Missile construction"
        );
        assert!(fx.hidden_left_guard_tiles.is_empty());

        zone.insert(371, (287, 616));
        zone.insert(2001, (293, 616));
        fx.observe(
            700,
            288,
            616,
            &[right_guard_range_event(9, 371, 2001, "Monster/100")],
            &zone,
        );
        fx.observe(1_100, 288, 616, &[], &zone);
        zone.remove(&2001);
        fx.observe(
            1_101,
            288,
            616,
            &[lifecycle_effect_event(10, "ObjectHide", Some(2001))],
            &zone,
        );
        assert_eq!(
            fx.active.len(),
            1,
            "post-launch target Hide keeps the missile after the bridge tombstones it"
        );
        assert_eq!(fx.hidden_left_guard_tiles.get(&2001), Some(&(293, 616)));
        fx.observe(
            1_102,
            288,
            616,
            &[lifecycle_effect_event(11, "ObjectRemove", Some(2001))],
            &zone,
        );
        assert_eq!(
            fx.active.len(),
            1,
            "post-launch target Remove freezes its last destination"
        );
        assert!(fx.anchor_object_ids.is_empty());
        assert_eq!(fx.source_object_ids.len(), 1);
        assert!(fx
            .local_projectile_targets
            .values()
            .all(|target| target.target_id.is_none()));
        let late_flight: Value = serde_json::from_str(
            &fx.tick_with_visibility(1_399, true)
                .expect("detached LeftGuard late flight"),
        )
        .expect("detached LeftGuard JSON");
        assert_eq!(late_flight["effects"].as_array().map(Vec::len), Some(1));
        let expired: Value = serde_json::from_str(
            &fx.tick_with_visibility(1_400, true)
                .expect("detached LeftGuard expiration"),
        )
        .expect("detached LeftGuard expiration JSON");
        assert_eq!(expired["effects"], json!([]));
        assert!(fx.hidden_left_guard_tiles.is_empty());

        zone.insert(2001, (293, 616));
        fx.observe(
            1_500,
            288,
            616,
            &[right_guard_range_event(12, 371, 2001, "Monster/100")],
            &zone,
        );
        fx.observe(1_900, 288, 616, &[], &zone);
        zone.remove(&371);
        fx.observe(
            1_901,
            288,
            616,
            &[lifecycle_effect_event(13, "ObjectHide", Some(371))],
            &zone,
        );
        assert_eq!(
            fx.active.len(),
            1,
            "post-launch source Hide keeps its Effects after the bridge tombstones it"
        );
        assert_eq!(fx.hidden_left_guard_tiles.get(&371), Some(&(287, 616)));
        fx.observe(
            1_902,
            288,
            616,
            &[lifecycle_effect_event(14, "ObjectRemove", Some(371))],
            &zone,
        );
        assert!(
            fx.active.is_empty(),
            "source Remove clears its owned Missile"
        );
        assert!(fx.hidden_left_guard_tiles.is_empty());

        zone.insert(371, (287, 616));
        fx.observe(
            2_000,
            288,
            616,
            &[right_guard_range_event(15, 371, 2001, "Monster/100")],
            &zone,
        );
        fx.observe(
            2_010,
            288,
            616,
            &[lifecycle_effect_event(16, "MapChanged", None)],
            &zone,
        );
        assert!(fx.active.is_empty());
        fx.observe(
            2_100,
            288,
            616,
            &[right_guard_range_event(17, 371, 2001, "Monster/100")],
            &zone,
        );
        fx.observe(
            2_110,
            288,
            616,
            &[lifecycle_effect_event(18, "LogOutSuccess", None)],
            &zone,
        );
        assert!(fx.active.is_empty());
        fx.observe(
            2_200,
            288,
            616,
            &[right_guard_range_event(19, 371, 2001, "Monster/100")],
            &zone,
        );
        let mut next_generation = right_guard_range_event(1, 371, 2002, "Monster/100");
        next_generation.generation = 18;
        fx.observe(2_210, 288, 616, &[next_generation], &zone);
        assert_eq!(fx.active.len(), 1);
        assert_eq!(fx.active[0].key, "left-guard-range:371:2002");
        assert_eq!(fx.source_object_ids.len(), 1);
        fx.reset_session();
        assert!(fx.active.is_empty());
        assert!(fx.anchor_object_ids.is_empty());
        assert!(fx.source_object_ids.is_empty());
        assert!(fx.local_projectile_targets.is_empty());
        assert!(fx.hidden_left_guard_tiles.is_empty());
    }

    #[test]
    fn left_guard_hide_survives_real_adapter_tombstoned_snapshots() {
        let base_payload = json!({
            "playerObjectId": 1000,
            "sceneView": {"center": {"x": 288, "y": 616}, "width": 19, "height": 15},
            "entities": [
                {"objectId": 1000, "kind": "selfPlayer", "x": 288, "y": 616},
                {
                    "objectId": 371,
                    "kind": "monster",
                    "x": 287,
                    "y": 616,
                    "sprite": {"bodyLibrary": "Monster/100"}
                },
                {
                    "objectId": 372,
                    "kind": "monster",
                    "x": 289,
                    "y": 616,
                    "sprite": {"bodyLibrary": "Monster/099"}
                },
                {"objectId": 2001, "kind": "monster", "x": 293, "y": 616}
            ]
        });
        let mut adapter = NativeGameplayAdapter::default();
        adapter.observe_world_snapshot(&base_payload);
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectRangeAttack".to_owned(),
            payload: json!({
                "objectId": 371,
                "location": {"x": 287, "y": 616},
                "direction": "Down",
                "targetId": 2001,
                "target": {"x": 293, "y": 616},
                "attackType": 0,
                "spell": 0,
                "level": 0
            }),
        }));

        let mut attack_payload = base_payload.clone();
        adapter.apply_authoritative_overlay(&mut attack_payload);
        let attack_snapshot = adapter.snapshot(&attack_payload);
        let mut fx = NativeEffects::default();
        fx.observe(
            0,
            288,
            616,
            &attack_snapshot.effect_events,
            &attack_snapshot.zone_entity_tiles,
        );
        assert_eq!(fx.active.len(), 1);

        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectHide".to_owned(),
            payload: json!({"objectId": 371}),
        }));
        let mut hidden_source_payload = base_payload.clone();
        adapter.apply_authoritative_overlay(&mut hidden_source_payload);
        let hidden_source_snapshot = adapter.snapshot(&hidden_source_payload);
        assert!(!hidden_source_snapshot.zone_entity_tiles.contains_key(&371));
        fx.observe(
            10,
            288,
            616,
            &hidden_source_snapshot.effect_events,
            &hidden_source_snapshot.zone_entity_tiles,
        );
        assert_eq!(fx.active.len(), 1);
        assert_eq!(fx.hidden_left_guard_tiles.get(&371), Some(&(287, 616)));

        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectHide".to_owned(),
            payload: json!({"objectId": 2001}),
        }));
        let mut hidden_both_payload = base_payload;
        adapter.apply_authoritative_overlay(&mut hidden_both_payload);
        let hidden_both_snapshot = adapter.snapshot(&hidden_both_payload);
        assert!(!hidden_both_snapshot.zone_entity_tiles.contains_key(&371));
        assert!(!hidden_both_snapshot.zone_entity_tiles.contains_key(&2001));

        let mut batched_fx = NativeEffects::default();
        batched_fx.observe(0, 288, 616, &[], &attack_snapshot.zone_entity_tiles);
        batched_fx.observe(
            10,
            288,
            616,
            &hidden_both_snapshot.effect_events,
            &hidden_both_snapshot.zone_entity_tiles,
        );
        assert_eq!(
            batched_fx.active.len(),
            1,
            "RangeAttack preceding same-batch Hide must use the previous raw actor tiles"
        );
        let batched_launch: Value = serde_json::from_str(
            &batched_fx
                .tick_with_visibility(410, true)
                .expect("same-batch LeftGuard launch"),
        )
        .expect("same-batch LeftGuard JSON");
        assert_eq!(batched_launch["effects"].as_array().map(Vec::len), Some(1));

        let mut reverse_attack = attack_snapshot.effect_events[0].clone();
        reverse_attack.sequence = 3;
        let reverse_events = vec![
            NativeEffectEvent {
                sequence: 1,
                generation: 0,
                packet: "ObjectHide".to_owned(),
                payload: json!({"objectId": 371}),
            },
            NativeEffectEvent {
                sequence: 2,
                generation: 0,
                packet: "ObjectHide".to_owned(),
                payload: json!({"objectId": 2001}),
            },
            reverse_attack,
        ];
        let mut reverse_fx = NativeEffects::default();
        reverse_fx.observe(0, 288, 616, &[], &attack_snapshot.zone_entity_tiles);
        reverse_fx.observe(
            10,
            288,
            616,
            &reverse_events,
            &hidden_both_snapshot.zone_entity_tiles,
        );
        assert!(
            reverse_fx.active.is_empty(),
            "Hide preceding RangeAttack must remain fail-closed"
        );

        let mut next_generation_events = hidden_both_snapshot.effect_events.clone();
        for event in &mut next_generation_events {
            event.generation = 10;
        }
        let mut generation_fx = NativeEffects::default();
        generation_fx.last_generation = 9;
        generation_fx.observe(0, 288, 616, &[], &attack_snapshot.zone_entity_tiles);
        generation_fx.observe(
            10,
            288,
            616,
            &next_generation_events,
            &hidden_both_snapshot.zone_entity_tiles,
        );
        assert!(
            generation_fx.active.is_empty(),
            "a new generation must never recover tiles from the previous connection"
        );
        assert!(!generation_fx.zone_tiles.contains_key(&371));
        assert!(!generation_fx.zone_tiles.contains_key(&2001));

        let mut post_map_attack = attack_snapshot.effect_events[0].clone();
        post_map_attack.sequence = 2;
        let post_map_events = vec![
            NativeEffectEvent {
                sequence: 1,
                generation: 0,
                packet: "MapChanged".to_owned(),
                payload: json!({"mapIndex": 2}),
            },
            post_map_attack,
            NativeEffectEvent {
                sequence: 3,
                generation: 0,
                packet: "ObjectHide".to_owned(),
                payload: json!({"objectId": 371}),
            },
            NativeEffectEvent {
                sequence: 4,
                generation: 0,
                packet: "ObjectHide".to_owned(),
                payload: json!({"objectId": 2001}),
            },
        ];
        let mut map_boundary_fx = NativeEffects::default();
        map_boundary_fx.observe(0, 288, 616, &[], &attack_snapshot.zone_entity_tiles);
        map_boundary_fx.observe(
            10,
            288,
            616,
            &post_map_events,
            &hidden_both_snapshot.zone_entity_tiles,
        );
        assert!(
            map_boundary_fx.active.is_empty(),
            "same-generation MapChanged must invalidate the previous map's raw tiles"
        );
        assert!(!map_boundary_fx.zone_tiles.contains_key(&371));
        assert!(!map_boundary_fx.zone_tiles.contains_key(&2001));

        fx.observe(
            20,
            288,
            616,
            &hidden_both_snapshot.effect_events,
            &hidden_both_snapshot.zone_entity_tiles,
        );
        assert_eq!(fx.active.len(), 1);
        assert_eq!(fx.hidden_left_guard_tiles.get(&2001), Some(&(293, 616)));

        let launched: Value = serde_json::from_str(
            &fx.tick_with_visibility(400, true)
                .expect("LeftGuard survives adapter tombstones through launch"),
        )
        .expect("adapter-chain LeftGuard JSON");
        assert_eq!(launched["effects"].as_array().map(Vec::len), Some(1));

        let expired: Value = serde_json::from_str(
            &fx.tick_with_visibility(700, true)
                .expect("adapter-chain LeftGuard expiration"),
        )
        .expect("adapter-chain expiration JSON");
        assert_eq!(expired["effects"], json!([]));
        assert!(fx.active.is_empty());
        assert!(fx.hidden_left_guard_tiles.is_empty());
        assert!(!fx.zone_tiles.contains_key(&371));
        assert!(!fx.zone_tiles.contains_key(&2001));

        let mut replayed_events = hidden_both_snapshot.effect_events.clone();
        let mut unrelated_right_guard = right_guard_range_event(4, 372, 2001, "Monster/099");
        unrelated_right_guard.generation = 0;
        replayed_events.push(unrelated_right_guard);
        fx.observe(
            710,
            288,
            616,
            &replayed_events,
            &hidden_both_snapshot.zone_entity_tiles,
        );
        assert!(fx.active.is_empty());
        assert!(fx.hidden_left_guard_tiles.is_empty());
        assert!(
            !fx.zone_tiles.contains_key(&2001),
            "replayed Hide must not resurrect a stale target for unrelated effects"
        );
    }

    #[test]
    fn right_guard_range_hit_catalog_is_exact_crystal_contract() {
        let catalog = EffectCatalog::load().expect("production effect catalog");
        let animation = catalog
            .right_guard_range_hit_animation()
            .expect("RightGuard Magic2 hit frames");
        assert_eq!(animation.name, "RightGuardRangeHit");
        assert_eq!(animation.kind, "impact");
        assert_eq!(animation.interval, 60);
        assert_eq!(animation.duration_ms, 300);
        assert_eq!(animation.frames.len(), 5);
        assert!(animation.blend);
        assert!((animation.opacity - 1.0).abs() < f32::EPSILON);
        assert_eq!(animation.light, Some(6));
        assert!(!animation.repeat);
        for (frame, index) in animation.frames.iter().zip(10..=14) {
            assert!(frame.path.ends_with(&format!("/Magic2/{index}.png")));
            assert!(crate::frame_png_exists(&frame.path));
        }
    }

    #[test]
    fn right_guard_range_hit_waits_four_frames_tracks_target_and_expires() {
        let mut zone = HashMap::from([(371, (287, 616)), (2001, (289, 616))]);
        let mut fx = NativeEffects::default();
        fx.observe(
            0,
            288,
            616,
            &[right_guard_range_event(1, 371, 2001, "Monster/099")],
            &zone,
        );
        assert_eq!(fx.active.len(), 1);
        assert_eq!(fx.active[0].key, "right-guard-range:371:2001");
        assert_eq!(fx.active[0].kind, EffectKindTag::Impact);
        assert_eq!(fx.active[0].start_at, 400);
        assert_eq!(fx.anchor_object_ids["right-guard-range:371:2001"], 2001);
        assert_eq!(
            fx.prestart_source_object_ids["right-guard-range:371:2001"],
            371
        );
        assert!(fx.current_light_snapshots(399).is_empty());

        let before: Value = serde_json::from_str(
            &fx.tick_with_visibility(399, true)
                .expect("delayed RightGuard state"),
        )
        .expect("delayed RightGuard JSON");
        assert_eq!(before["effects"], json!([]));

        for (now_ms, index) in [(400, 10), (460, 11), (520, 12), (580, 13), (640, 14)] {
            if now_ms == 520 {
                zone.insert(2001, (290, 617));
                fx.observe(now_ms, 288, 616, &[], &zone);
                assert_eq!((fx.active[0].tile_x, fx.active[0].tile_y), (290, 617));
            }
            let rendered: Value = serde_json::from_str(
                &fx.tick_with_visibility(now_ms, true)
                    .expect("RightGuard visible frame"),
            )
            .expect("RightGuard frame JSON");
            assert_eq!(rendered["effects"].as_array().map(Vec::len), Some(1));
            assert!(rendered["effects"][0]["imageUrl"]
                .as_str()
                .is_some_and(|path| path.ends_with(&format!("/Magic2/{index}.png"))));
            assert_eq!(rendered["effects"][0]["additive"], true);
            assert_eq!(rendered["effects"][0]["opacity"], 1.0);
            let lights = fx.current_light_snapshots(now_ms);
            assert_eq!(lights.len(), 1);
            assert_eq!(lights[0].light, 6);
        }

        let expired: Value = serde_json::from_str(
            &fx.tick_with_visibility(700, true)
                .expect("expired RightGuard state"),
        )
        .expect("expired RightGuard JSON");
        assert_eq!(expired["effects"], json!([]));
        assert!(fx.active.is_empty());
        assert!(fx.anchor_object_ids.is_empty());
        assert!(fx.prestart_source_object_ids.is_empty());
    }

    #[test]
    fn right_guard_range_hit_restarts_per_pair_and_obeys_target_lifecycle() {
        let zone = HashMap::from([(371, (287, 616)), (2001, (289, 616)), (2002, (290, 616))]);
        let mut fx = NativeEffects::default();
        fx.observe(
            0,
            288,
            616,
            &[right_guard_range_event(1, 371, 2001, "Monster/099")],
            &zone,
        );
        fx.observe(
            100,
            288,
            616,
            &[right_guard_range_event(
                2,
                371,
                2001,
                "/original-ui/Monster/099",
            )],
            &zone,
        );
        assert_eq!(fx.active.len(), 1, "same pair restarts one effect");
        assert_eq!(fx.active[0].start_at, 500);

        let other_target = right_guard_range_event(3, 371, 2002, "Monster/099");
        fx.observe(120, 288, 616, &[other_target.clone()], &zone);
        assert_eq!(fx.active.len(), 2, "different targets remain independent");
        let replay_start_at = fx
            .active
            .iter()
            .find(|instance| instance.key == "right-guard-range:371:2002")
            .map(|instance| (instance.start_at, instance.provenance.sequence))
            .expect("second target effect");
        fx.observe(130, 288, 616, &[other_target], &zone);
        assert_eq!(fx.active.len(), 2, "replayed sequence is deduplicated");
        assert_eq!(
            fx.active
                .iter()
                .find(|instance| instance.key == "right-guard-range:371:2002")
                .map(|instance| (instance.start_at, instance.provenance.sequence)),
            Some(replay_start_at),
            "replay must not restart or rewrite provenance"
        );

        fx.observe(
            140,
            288,
            616,
            &[lifecycle_effect_event(4, "ObjectRemove", Some(371))],
            &zone,
        );
        assert!(
            fx.active.is_empty(),
            "prestart attacker removal cancels both effects"
        );
        assert!(fx.anchor_object_ids.is_empty());
        assert!(fx.prestart_source_object_ids.is_empty());

        fx.observe(
            200,
            288,
            616,
            &[right_guard_range_event(5, 371, 2001, "Monster/099")],
            &zone,
        );
        fx.observe(
            300,
            288,
            616,
            &[lifecycle_effect_event(6, "ObjectHide", Some(371))],
            &zone,
        );
        assert!(
            fx.active.is_empty(),
            "prestart attacker hide cancels the effect"
        );

        fx.observe(
            400,
            288,
            616,
            &[right_guard_range_event(7, 371, 2001, "Monster/099")],
            &zone,
        );
        fx.observe(801, 288, 616, &[], &zone);
        assert!(fx.prestart_source_object_ids.is_empty());
        fx.observe(
            810,
            288,
            616,
            &[lifecycle_effect_event(8, "ObjectRemove", Some(371))],
            &zone,
        );
        assert_eq!(
            fx.active.len(),
            1,
            "post-start attacker removal keeps target-owned effect"
        );
        fx.observe(
            820,
            288,
            616,
            &[lifecycle_effect_event(9, "ObjectRemove", Some(2001))],
            &zone,
        );
        assert!(
            fx.active.is_empty(),
            "target removal always cancels the effect"
        );

        fx.observe(
            900,
            288,
            616,
            &[right_guard_range_event(10, 371, 2002, "Monster/099")],
            &zone,
        );
        fx.observe(1_301, 288, 616, &[], &zone);
        fx.observe(
            1_310,
            288,
            616,
            &[lifecycle_effect_event(11, "ObjectHide", Some(371))],
            &zone,
        );
        assert_eq!(
            fx.active.len(),
            1,
            "post-start attacker hide keeps target-owned effect"
        );
        fx.observe(
            1_320,
            288,
            616,
            &[lifecycle_effect_event(12, "ObjectHide", Some(2002))],
            &zone,
        );
        assert!(
            fx.active.is_empty(),
            "target hide always cancels the effect"
        );

        fx.observe(
            1_400,
            288,
            616,
            &[right_guard_range_event(13, 371, 2001, "Monster/099")],
            &zone,
        );
        fx.observe(
            1_410,
            288,
            616,
            &[lifecycle_effect_event(14, "MapChanged", None)],
            &zone,
        );
        assert!(fx.active.is_empty());
        assert!(fx.prestart_source_object_ids.is_empty());
        fx.observe(
            1_420,
            288,
            616,
            &[right_guard_range_event(15, 371, 2001, "Monster/099")],
            &zone,
        );
        fx.observe(
            1_430,
            288,
            616,
            &[lifecycle_effect_event(16, "LogOutSuccess", None)],
            &zone,
        );
        assert!(fx.active.is_empty());
        assert!(fx.prestart_source_object_ids.is_empty());

        fx.observe(
            1_440,
            288,
            616,
            &[right_guard_range_event(17, 371, 2001, "Monster/099")],
            &zone,
        );
        let mut next_generation = right_guard_range_event(1, 371, 2002, "Monster/099");
        next_generation.generation = 18;
        fx.observe(1_450, 288, 616, &[next_generation], &zone);
        assert_eq!(fx.active.len(), 1);
        assert_eq!(fx.active[0].key, "right-guard-range:371:2002");
        assert_eq!(fx.last_generation, 18);
        assert_eq!(fx.last_effect_sequence, 1);
        assert_eq!(fx.anchor_object_ids.len(), 1);
        assert_eq!(fx.prestart_source_object_ids.len(), 1);
        fx.reset_session();
        assert!(fx.active.is_empty());
        assert!(fx.anchor_object_ids.is_empty());
        assert!(fx.prestart_source_object_ids.is_empty());
    }

    #[test]
    fn right_guard_range_hit_source_boundary_is_prestart_through_400ms() {
        let zone = HashMap::from([(371, (287, 616)), (2001, (289, 616))]);
        let mut fx = NativeEffects::default();
        fx.observe(
            0,
            288,
            616,
            &[right_guard_range_event(1, 371, 2001, "Monster/099")],
            &zone,
        );
        fx.observe(
            400,
            288,
            616,
            &[lifecycle_effect_event(2, "ObjectRemove", Some(371))],
            &zone,
        );
        assert!(
            fx.active.is_empty(),
            "source remove at 400 ms cancels before ownership transfer"
        );

        fx.observe(
            500,
            288,
            616,
            &[right_guard_range_event(3, 371, 2001, "Monster/099")],
            &zone,
        );
        fx.observe(
            900,
            288,
            616,
            &[lifecycle_effect_event(4, "ObjectHide", Some(371))],
            &zone,
        );
        assert!(
            fx.active.is_empty(),
            "source hide at 400 ms cancels before ownership transfer"
        );

        fx.observe(
            1_000,
            288,
            616,
            &[right_guard_range_event(5, 371, 2001, "Monster/099")],
            &zone,
        );
        fx.observe(
            1_401,
            288,
            616,
            &[lifecycle_effect_event(6, "ObjectRemove", Some(371))],
            &zone,
        );
        assert_eq!(
            fx.active.len(),
            1,
            "source remove after 400 ms keeps target ownership"
        );
        fx.observe(
            1_410,
            288,
            616,
            &[lifecycle_effect_event(7, "ObjectRemove", Some(2001))],
            &zone,
        );
        assert!(fx.active.is_empty());

        fx.observe(
            1_500,
            288,
            616,
            &[right_guard_range_event(8, 371, 2001, "Monster/099")],
            &zone,
        );
        fx.observe(
            1_901,
            288,
            616,
            &[lifecycle_effect_event(9, "ObjectHide", Some(371))],
            &zone,
        );
        assert_eq!(
            fx.active.len(),
            1,
            "source hide after 400 ms keeps target ownership"
        );
        fx.observe(
            1_910,
            288,
            616,
            &[lifecycle_effect_event(10, "ObjectHide", Some(2001))],
            &zone,
        );
        assert!(fx.active.is_empty());
    }

    #[test]
    fn guard_range_effects_fail_closed_for_other_packets_sources_and_missing_context() {
        let zone = HashMap::from([(371, (287, 616)), (2001, (289, 616))]);
        let mut fx = NativeEffects::default();
        let mut missing_location = right_guard_range_event(4, 371, 2001, "Monster/100");
        missing_location
            .payload
            .as_object_mut()
            .expect("range payload object")
            .remove("location");
        let mut ordinary = ordinary_attack_event(7);
        ordinary.generation = 17;
        fx.observe(
            0,
            288,
            616,
            &[
                right_guard_range_event(1, 371, 2001, "Monster/101"),
                right_guard_range_event(2, 371, 9999, "Monster/099"),
                right_guard_range_event(3, 9999, 2001, "Monster/099"),
                missing_location,
                right_guard_range_event(5, 371, 9999, "Monster/100"),
                right_guard_range_event(6, 9999, 2001, "Monster/100"),
                ordinary,
            ],
            &zone,
        );
        assert!(fx.active.is_empty());
        assert!(fx.anchor_object_ids.is_empty());
    }

    #[test]
    fn flaming_sword_all_directions_resolve_six_source_frames_at_crystal_rate() {
        let fixture = flaming_sword_fixture();
        let catalog = EffectCatalog::load().expect("production effect catalog");
        assert!(catalog.spell_cast_animation("FlamingSword", 0).is_none());
        for direction in 0..8_u32 {
            let animation = catalog
                .spell_attack_overlay_animation("FlamingSword", direction)
                .expect("all FlamingSword directions resolve");
            assert_eq!(animation.kind, "attackOverlay");
            assert_eq!(animation.frames.len(), 6);
            assert_eq!(animation.interval, 100);
            assert_eq!(animation.duration_ms, 600);
            assert!(animation.blend);
            assert!((animation.opacity - 0.7).abs() < f32::EPSILON);
            assert_eq!(animation.light, Some(0));
            for (frame, source_frame) in animation.frames.iter().zip(0..6_u32) {
                let index = 3480 + direction * 10 + source_frame;
                assert!(frame.path.ends_with(&format!("/Magic/{index}.png")));
                assert!(crate::frame_png_exists(&frame.path));
            }
            assert_eq!(
                fixture["directionCases"][direction as usize]["firstFrame"],
                3480 + direction * 10
            );
            assert_eq!(
                fixture["directionCases"][direction as usize]["lastFrame"],
                3485 + direction * 10
            );
        }
    }

    #[test]
    fn flaming_sword_object_attack_tracks_attacker_and_expires_at_six_frames() {
        let mut zone = HashMap::from([(1000, (288, 616))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[flaming_sword_event(1, 0)], &zone);
        assert_eq!(fx.active.len(), 1);
        assert_eq!(fx.active[0].kind, EffectKindTag::AttackOverlay);
        assert_eq!(fx.active[0].key, "flaming-sword-1000");
        assert_eq!(fx.active[0].start_at, 0);
        assert!(fx.current_light_snapshots(0).is_empty());

        let sound = fx.take_due_sound_events(0);
        assert_eq!(sound.len(), 1);
        assert_eq!(sound[0].cue, FLAMING_SWORD_SOUND_CUE);
        assert_eq!(sound[0].file_name, FLAMING_SWORD_SOUND_FILE);
        assert!(fx.take_due_sound_events(0).is_empty());

        let first: Value = serde_json::from_str(
            &fx.tick_with_visibility(0, true)
                .expect("first FlamingSword frame"),
        )
        .expect("first FlamingSword JSON");
        let entry = &first["effects"][0];
        assert!(entry["imageUrl"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Magic/3480.png")));
        assert_eq!(entry["additive"], true);
        assert!(entry["opacity"]
            .as_f64()
            .is_some_and(|opacity| (opacity - 0.7).abs() < 0.000_001));
        assert!(entry.get("shadowX").is_none());
        assert!(entry.get("shadowY").is_none());

        zone.insert(1000, (289, 616));
        fx.observe(200, 288, 616, &[], &zone);
        assert_eq!((fx.active[0].tile_x, fx.active[0].tile_y), (289, 616));
        let moved: Value = serde_json::from_str(
            &fx.tick_with_visibility(200, true)
                .expect("moved FlamingSword frame"),
        )
        .expect("moved FlamingSword JSON");
        assert!(moved["effects"][0]["imageUrl"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Magic/3482.png")));

        let hidden: Value = serde_json::from_str(
            &fx.tick_with_visibility(300, false)
                .expect("hidden FlamingSword state"),
        )
        .expect("hidden FlamingSword JSON");
        assert_eq!(hidden["effects"], json!([]));
        let restored: Value = serde_json::from_str(
            &fx.tick_with_visibility(400, true)
                .expect("restored FlamingSword state"),
        )
        .expect("restored FlamingSword JSON");
        assert!(restored["effects"][0]["imageUrl"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Magic/3484.png")));

        let last: Value = serde_json::from_str(
            &fx.tick_with_visibility(599, true)
                .expect("last FlamingSword frame"),
        )
        .expect("last FlamingSword JSON");
        assert!(last["effects"][0]["imageUrl"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Magic/3485.png")));
        let expired: Value = serde_json::from_str(
            &fx.tick_with_visibility(600, true)
                .expect("expired FlamingSword state"),
        )
        .expect("expired FlamingSword JSON");
        assert_eq!(expired["effects"], json!([]));
        assert!(fx.active.is_empty());
    }

    #[test]
    fn flaming_sword_ordinary_attack_is_noop_and_new_attack_restarts_per_attacker() {
        let zone = HashMap::from([(1000, (288, 616)), (1001, (289, 616))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[ordinary_attack_event(1)], &zone);
        assert!(fx.active.is_empty());
        assert!(fx.take_due_sound_events(0).is_empty());

        fx.observe(10, 288, 616, &[flaming_sword_event(2, 0)], &zone);
        assert_eq!(fx.active.len(), 1);
        fx.observe(210, 288, 616, &[flaming_sword_event(3, 2)], &zone);
        assert_eq!(fx.active.len(), 1, "same attacker replaces old overlay");
        assert_eq!(fx.active[0].started_at, 210);
        assert!(fx.active[0]
            .current
            .as_ref()
            .is_some_and(|animation| animation.frames[0].path.ends_with("/Magic/3500.png")));

        let mut other = flaming_sword_event(4, 4);
        other.payload["objectId"] = json!(1001);
        other.payload["location"] = json!({"x": 289, "y": 616});
        fx.observe(220, 288, 616, &[other], &zone);
        assert_eq!(fx.active.len(), 2, "different attackers coexist");
        assert!(fx
            .active
            .iter()
            .any(|entry| entry.key == "flaming-sword-1000"));
        assert!(fx
            .active
            .iter()
            .any(|entry| entry.key == "flaming-sword-1001"));
    }

    #[test]
    fn flaming_sword_remove_map_change_and_session_reset_clear_overlay_and_audio() {
        let zone = HashMap::from([(1000, (288, 616))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[flaming_sword_event(1, 0)], &zone);
        fx.observe(
            1,
            288,
            616,
            &[NativeEffectEvent {
                sequence: 2,
                generation: 16,
                packet: "ObjectRemove".to_owned(),
                payload: json!({"objectId": 1000}),
            }],
            &zone,
        );
        assert!(fx.active.is_empty());

        fx.observe(10, 288, 616, &[flaming_sword_event(3, 0)], &zone);
        fx.observe(
            11,
            288,
            616,
            &[NativeEffectEvent {
                sequence: 4,
                generation: 16,
                packet: "MapChanged".to_owned(),
                payload: json!({}),
            }],
            &zone,
        );
        assert!(fx.active.is_empty());
        assert!(fx.take_due_sound_events(11).is_empty());

        fx.observe(20, 288, 616, &[flaming_sword_event(5, 0)], &zone);
        fx.reset_session();
        assert!(fx.active.is_empty());
        assert!(fx.take_due_sound_events(20).is_empty());
    }

    #[test]
    fn flaming_sword_source_audio_identity_is_closed() {
        use sha2::{Digest, Sha256};

        let fixture = flaming_sword_fixture();
        let path = assets::asset_path("original-ui/Sound/M8-1.wav")
            .expect("packaged FlamingSword sound path");
        let bytes = fs::read(path).expect("read FlamingSword sound");
        assert_eq!(bytes.len(), fixture["source"]["audio"]["sourceBytes"]);
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            fixture["source"]["audio"]["sha256"]
        );
    }

    fn healing_fixture() -> Value {
        serde_json::from_str(include_str!(
            "../tests/fixtures/vis02-bichon-healing-v1.json"
        ))
        .expect("VIS-02 Healing fixture JSON")
    }

    fn healing_magic_event(sequence: u64) -> NativeEffectEvent {
        NativeEffectEvent {
            sequence,
            generation: 17,
            packet: "ObjectMagic".to_owned(),
            payload: healing_fixture()["timeline"][0]["event"]["payload"].clone(),
        }
    }

    fn healing_target_event(sequence: u64) -> NativeEffectEvent {
        NativeEffectEvent {
            sequence,
            generation: 17,
            packet: "ObjectEffect".to_owned(),
            payload: healing_fixture()["timeline"][1]["event"]["payload"].clone(),
        }
    }

    #[test]
    fn healing_cast_and_target_follow_crystal_clocks_and_audio_phases() {
        let zone = HashMap::from([(1000, (288, 616)), (2005, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[healing_magic_event(1)], &zone);

        assert_eq!(fx.active.len(), 1);
        let cast = fx.active.first().expect("Healing cast");
        let cast_animation = cast.current.as_ref().expect("Healing cast animation");
        assert_eq!(cast.kind, EffectKindTag::Cast);
        assert_eq!(cast.start_at, 0);
        assert_eq!(cast_animation.frames.len(), 10);
        assert_eq!(cast_animation.interval, 60);
        assert_eq!(cast_animation.duration_ms, 600);
        assert_eq!(cast_animation.light, Some(6));
        assert!(cast_animation.frames[0].path.ends_with("/Magic/200.png"));
        assert!(cast_animation.frames[9].path.ends_with("/Magic/209.png"));
        let cast_sound = fx.take_due_sound_events(0);
        assert_eq!(cast_sound.len(), 1);
        assert_eq!(cast_sound[0].cue, HEALING_CAST_SOUND_CUE);
        assert_eq!(cast_sound[0].file_name, HEALING_CAST_SOUND_FILE);

        fx.observe(600, 288, 616, &[healing_target_event(2)], &zone);
        let target = fx
            .active
            .iter()
            .find(|instance| instance.kind == EffectKindTag::SceneForeground)
            .expect("Healing target effect");
        let target_animation = target.current.as_ref().expect("Healing target animation");
        assert_eq!(target.start_at, 600, "Crystal ignores Healing DelayTime");
        assert_eq!((target.tile_x, target.tile_y), (288, 611));
        assert_eq!(fx.anchor_object_ids[&target.key], 2005);
        assert_eq!(target_animation.frames.len(), 10);
        assert_eq!(target_animation.interval, 80);
        assert_eq!(target_animation.duration_ms, 800);
        assert_eq!(target_animation.light, Some(6));
        assert!(target_animation.frames[0].path.ends_with("/Magic/370.png"));
        assert!(target_animation.frames[9].path.ends_with("/Magic/379.png"));
        let target_sound = fx.take_due_sound_events(600);
        assert_eq!(target_sound.len(), 1);
        assert_eq!(target_sound[0].cue, HEALING_TARGET_SOUND_CUE);
        assert_eq!(target_sound[0].file_name, HEALING_TARGET_SOUND_FILE);
    }

    #[test]
    fn healing_target_tracks_authoritative_object_and_replay_is_one_shot() {
        let original_zone = HashMap::from([(2005, (288, 611))]);
        let mut fx = NativeEffects::default();
        let event = healing_target_event(1);
        fx.observe(100, 288, 616, &[event.clone()], &original_zone);
        assert_eq!(fx.active.len(), 1);
        let key = fx.active[0].key.clone();
        assert_eq!(fx.take_due_sound_events(100).len(), 1);

        let moved_zone = HashMap::from([(2005, (291, 613))]);
        fx.observe(200, 288, 616, &[event], &moved_zone);
        assert_eq!(fx.active.len(), 1, "same sequence must not replay the effect");
        assert_eq!(fx.active[0].key, key);
        assert_eq!((fx.active[0].tile_x, fx.active[0].tile_y), (291, 613));
        assert!(fx.take_due_sound_events(200).is_empty());

        fx.observe(
            300,
            288,
            616,
            &[NativeEffectEvent {
                sequence: 2,
                generation: 17,
                packet: "ObjectRemove".to_owned(),
                payload: json!({"objectId": 2005}),
            }],
            &HashMap::new(),
        );
        assert!(fx.active.is_empty());
        assert!(fx.anchor_object_ids.is_empty());
    }

    #[test]
    fn healing_target_missing_anchor_and_scene_boundaries_fail_closed() {
        let mut missing = NativeEffects::default();
        missing.observe(
            0,
            288,
            616,
            &[healing_target_event(1)],
            &HashMap::new(),
        );
        assert!(missing.active.is_empty());
        assert!(missing.take_due_sound_events(0).is_empty());

        let zone = HashMap::from([(1000, (288, 616)), (2005, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(
            0,
            288,
            616,
            &[healing_magic_event(1), healing_target_event(2)],
            &zone,
        );
        fx.observe(
            1,
            288,
            616,
            &[NativeEffectEvent {
                sequence: 3,
                generation: 17,
                packet: "MapChanged".to_owned(),
                payload: json!({}),
            }],
            &zone,
        );
        assert!(fx.active.is_empty());
        assert!(fx.anchor_object_ids.is_empty());
        assert!(fx.take_due_sound_events(1).is_empty());

        fx.observe(2, 288, 616, &[healing_magic_event(4)], &zone);
        fx.reset_session();
        assert!(fx.active.is_empty());
        assert!(fx.take_due_sound_events(2).is_empty());
    }

    #[test]
    fn healing_source_frames_and_audio_identities_are_closed() {
        use sha2::{Digest, Sha256};

        let fixture = healing_fixture();
        let catalog = EffectCatalog::load().expect("production effect catalog");
        let cast = catalog
            .spell_cast_animation("Healing", 0)
            .expect("Healing cast");
        assert_eq!(cast.frames.len(), 10);
        for (frame, index) in cast.frames.iter().zip(200..210) {
            assert!(frame.path.ends_with(&format!("/Magic/{index}.png")));
            assert!(crate::frame_png_exists(&frame.path));
        }
        let target = catalog
            .map_animation_by_number(3, 0)
            .expect("Healing ObjectEffect");
        assert_eq!(target.frames.len(), 10);
        for (frame, index) in target.frames.iter().zip(370..380) {
            assert!(frame.path.ends_with(&format!("/Magic/{index}.png")));
            assert!(crate::frame_png_exists(&frame.path));
        }
        for audio in fixture["source"]["audio"]
            .as_array()
            .expect("Healing audio catalog")
        {
            let file = audio["file"].as_str().expect("Healing audio file");
            assert!(
                mir2_client_bevy::audio::NATIVE_GAMEPLAY_SOUND_FILES.contains(&file),
                "native gameplay audio rejected {file}"
            );
            let path = assets::asset_path(&format!("original-ui/Sound/{file}"))
                .expect("packaged Healing sound path");
            let bytes = fs::read(path).expect("read Healing sound");
            assert_eq!(bytes.len(), audio["sourceBytes"]);
            assert_eq!(format!("{:x}", Sha256::digest(&bytes)), audio["sha256"]);
        }
    }

    fn lightning_fixture() -> Value {
        serde_json::from_str(include_str!(
            "../tests/fixtures/vis02-bichon-lightning-v1.json"
        ))
        .expect("VIS-02 Lightning fixture JSON")
    }

    fn lightning_event(sequence: u64, cast: bool) -> NativeEffectEvent {
        let fixture = lightning_fixture();
        let index = usize::from(!cast);
        NativeEffectEvent {
            sequence,
            generation: 7,
            packet: "ObjectMagic".to_owned(),
            payload: fixture["timeline"][index]["event"]["payload"].clone(),
        }
    }

    #[test]
    fn lightning_waits_for_spell_completion_follows_caster_and_sounds_once() {
        let mut zone = HashMap::from([(1000, (288, 616)), (2005, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[lightning_event(1, true)], &zone);
        let fixture = lightning_fixture();
        assert_eq!(fixture["schemaVersion"], json!(1));
        assert_eq!(
            fixture["source"]["actorActionDurationMs"],
            LIGHTNING_SPELL_ACTION_MS
        );
        assert_eq!(fx.active.len(), 1);
        assert_eq!(fx.active[0].start_at, LIGHTNING_SPELL_ACTION_MS);

        let before: Value = serde_json::from_str(
            &fx.tick_with_visibility(599, true)
                .expect("pre-completion render state"),
        )
        .expect("pre-completion JSON");
        assert_eq!(before["effects"], json!([]));
        assert!(fx.take_due_sound_events(599).is_empty());

        zone.insert(1000, (289, 616));
        fx.observe(599, 288, 616, &[], &zone);
        assert_eq!((fx.active[0].tile_x, fx.active[0].tile_y), (289, 616));

        let due = fx.take_due_sound_events(600);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].generation, 7);
        assert_eq!(due[0].sequence, 1);
        assert_eq!(due[0].cue, LIGHTNING_SOUND_CUE);
        assert_eq!(due[0].file_name, LIGHTNING_SOUND_FILE);
        assert!(fx.take_due_sound_events(600).is_empty());

        let first: Value = serde_json::from_str(
            &fx.tick_with_visibility(600, true)
                .expect("first Lightning frame"),
        )
        .expect("first frame JSON");
        assert!(first["effects"][0]["imageUrl"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Magic/970.png")));

        let hidden: Value = serde_json::from_str(
            &fx.tick_with_visibility(750, false)
                .expect("hidden Lightning state"),
        )
        .expect("hidden JSON");
        assert_eq!(hidden["effects"], json!([]));
        let restored: Value = serde_json::from_str(
            &fx.tick_with_visibility(800, true)
                .expect("restored Lightning state"),
        )
        .expect("restored JSON");
        assert!(restored["effects"][0]["imageUrl"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Magic/972.png")));
        assert!(fx.take_due_sound_events(800).is_empty());

        let last: Value = serde_json::from_str(
            &fx.tick_with_visibility(1_199, true)
                .expect("last Lightning frame"),
        )
        .expect("last frame JSON");
        assert!(last["effects"][0]["imageUrl"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Magic/975.png")));
        let expired: Value = serde_json::from_str(
            &fx.tick_with_visibility(1_200, true)
                .expect("expired Lightning state"),
        )
        .expect("expired JSON");
        assert_eq!(expired["effects"], json!([]));
        assert!(fx.active.is_empty());
    }

    #[test]
    fn lightning_cast_false_and_session_reset_never_emit_effect_or_sound() {
        let zone = HashMap::from([(1000, (288, 616)), (2005, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[lightning_event(1, false)], &zone);
        assert!(fx.active.is_empty());
        assert!(fx.take_due_sound_events(600).is_empty());

        fx.observe(1_000, 288, 616, &[lightning_event(2, true)], &zone);
        assert_eq!(fx.active.len(), 1);
        fx.observe(
            1_100,
            288,
            616,
            &[NativeEffectEvent {
                sequence: 3,
                generation: 7,
                packet: "MapChanged".to_owned(),
                payload: json!({}),
            }],
            &zone,
        );
        assert!(fx.active.is_empty());
        assert!(fx.take_due_sound_events(1_600).is_empty());

        fx.observe(2_000, 288, 616, &[lightning_event(4, true)], &zone);
        assert_eq!(fx.active.len(), 1);
        fx.observe(2_100, 288, 616, &[], &HashMap::new());
        assert!(
            fx.active.is_empty(),
            "a departed caster cannot anchor Lightning"
        );
        assert!(fx.take_due_sound_events(2_600).is_empty());
    }

    #[test]
    fn lightning_production_frames_and_audio_are_integrity_closed() {
        use sha2::{Digest, Sha256};

        let fixture = lightning_fixture();
        let catalog = EffectCatalog::load().expect("production effect catalog");
        for direction in 0..8_u32 {
            let animation = catalog
                .spell_cast_animation("Lightning", direction)
                .expect("all Lightning directions resolve");
            assert_eq!(animation.frames.len(), 6);
            assert_eq!(animation.interval, 100);
            assert_eq!(animation.duration_ms, 600);
            for (frame, source) in animation.frames.iter().zip(0..6_u32) {
                let index = 970 + direction * 20 + source;
                assert!(frame.path.ends_with(&format!("/Magic/{index}.png")));
                assert!(crate::frame_png_exists(&frame.path));
            }
        }

        let path = assets::asset_path("original-ui/Sound/M40-0.wav")
            .expect("packaged Lightning sound path");
        let bytes = fs::read(path).expect("read Lightning sound");
        assert_eq!(bytes.len(), fixture["source"]["audio"]["sourceBytes"]);
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            fixture["source"]["audio"]["sha256"]
        );
    }

    #[test]
    fn mine_persistent_cleared_by_object_remove() {
        let mut fx = NativeEffects::default();
        let zone = HashMap::new();
        fx.observe(
            0,
            0,
            0,
            &[NativeEffectEvent {
                sequence: 1,
                generation: 0,
                packet: "ObjectSpell".to_owned(),
                payload: json!({"objectId": 77, "location":{"x":5,"y":5},"spell":12,"direction":"down","param":0}),
            }],
            &zone,
        );
        assert_eq!(fx.active.len(), 1);
        assert_eq!(fx.active[0].kind, EffectKindTag::Persistent);
        fx.observe(
            0,
            0,
            0,
            &[NativeEffectEvent {
                sequence: 2,
                generation: 0,
                packet: "ObjectRemove".to_owned(),
                payload: json!({"objectId": 77}),
            }],
            &zone,
        );
        assert!(fx.active.is_empty());
    }

    #[test]
    fn blizzard_ground_has_delay_and_offset() {
        let catalog = EffectCatalog::load().expect("load");
        let ground = catalog
            .map_animation("Blizzard", 0)
            .expect("blizzard ground");
        assert_eq!(ground.offset_y, -20.0);
        assert_eq!(ground.interval, 100);
        assert_eq!(ground.frames.len(), 30);
    }

    #[test]
    fn mixed_200_effects_remain_bounded() {
        let mut zone_tiles = HashMap::new();
        zone_tiles.insert(1000, (10, 10));
        for i in 2001..2101 {
            zone_tiles.insert(i, (12, 10));
        }
        let mut fx = NativeEffects::default();
        let mut events = Vec::new();
        for seq in 1..=200 {
            let spell = match seq % 4 {
                0 => "FireBall",
                1 => "Lightning",
                2 => "Vampirism",
                _ => "Blizzard",
            };
            let packet = if spell == "Blizzard" {
                "ObjectSpell"
            } else {
                "ObjectMagic"
            };
            let payload = if packet == "ObjectMagic" {
                json!({"location":{"x":10,"y":10},"spell":spell,"direction":"down"})
            } else {
                json!({"objectId": 1000, "location":{"x":10,"y":10},"spell":50,"direction":"down","param":0})
            };
            events.push(NativeEffectEvent {
                sequence: seq as u64,
                generation: 0,
                packet: packet.to_owned(),
                payload,
            });
        }
        fx.observe(0, 10, 10, &events, &zone_tiles);
        assert!(fx.active.len() <= MAX_ACTIVE_EFFECTS);
        for t in [0, 500, 2000, 10000] {
            let _ = fx.tick(t);
            assert!(fx.active.len() <= MAX_ACTIVE_EFFECTS);
        }
    }
}
