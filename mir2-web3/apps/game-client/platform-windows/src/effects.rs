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

/// Crystal GameScene.MapEffect plays SoundList 10091 immediately on Mine.
const MINE_MAP_EFFECT: u64 = 12;
const MINE_HIT_WALL_SOUND_FILE: &str = "91.wav";
const MINE_HIT_WALL_SOUND_CUE: &str = "Mine.hitWall";

/// Crystal completes the six-frame Spell actor action before Lightning begins.
const LIGHTNING_SPELL_ACTION_MS: u64 = 600;
const LIGHTNING_SOUND_FILE: &str = "M40-0.wav";
const LIGHTNING_SOUND_CUE: &str = "Lightning.complete";
const FIREBALL_SPELL_ACTION_MS: u64 = 600;
const FIREBALL_PROJECTILE_STEP_MS: u64 = 30;
const FIREBALL_TILE_TRAVEL_MS: u64 = 50;
const GUARD_RANGE_ACTION_FRAME_DELAY_MS: u64 = 400;
// Crystal Frames.Player: AttackRange1 is eight 100 ms frames. The ordinary
// arrow starts at frame 5; a server-reported Focus proc begins at frame 6.
const ARCHER_NORMAL_ARROW_DELAY_MS: u64 = 500;
const ARCHER_FOCUS_DELAY_MS: u64 = 600;
const ARCHER_NORMAL_ARROW_SOUND_CUE: &str = "Archer.normalArrow";
const ARCHER_NORMAL_ARROW_SOUND_FILE: &str = "M121-0.wav";
const ARCHER_NORMAL_ARROW_IMPACT_SOUND_CUE: &str = "Archer.normalArrow.impact";
const ARCHER_NORMAL_ARROW_IMPACT_SOUND_FILE: &str = "M121-2.wav";
const ARCHER_FOCUS_SOUND_CUE: &str = "Focus.proc";
const ARCHER_FOCUS_SOUND_FILE: &str = "M121-5.wav";
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
const FIRE_BOUNCE_SPELL_ACTION_MS: u64 = 600;
const FIRE_BOUNCE_CAST_SOUND_FILE: &str = "M34-0.wav";
const FIRE_BOUNCE_PROJECTILE_SOUND_FILE: &str = "M34-1.wav";
const FIRE_BOUNCE_IMPACT_SOUND_FILE: &str = "M34-2.wav";
const FIRE_BOUNCE_CAST_SOUND_CUE: &str = "FireBounce.cast";
const FIRE_BOUNCE_PROJECTILE_SOUND_CUE: &str = "FireBounce.projectile";
const FIRE_BOUNCE_IMPACT_SOUND_CUE: &str = "FireBounce.impact";
const FROST_CRUNCH_SPELL_ACTION_MS: u64 = 600;
const FROST_CRUNCH_CAST_SOUND_FILE: &str = "M41-1.wav";
const FROST_CRUNCH_IMPACT_SOUND_FILE: &str = "M41-2.wav";
const FROST_CRUNCH_CAST_SOUND_CUE: &str = "FrostCrunch.cast";
const FROST_CRUNCH_IMPACT_SOUND_CUE: &str = "FrostCrunch.impact";
const HALLUCINATION_SPELL_ACTION_MS: u64 = 600;
const HALLUCINATION_PROJECTILE_STEP_MS: u64 = 48;
const HALLUCINATION_IMPACT_SOUND_FILE: &str = "M76-0.wav";
const HALLUCINATION_IMPACT_SOUND_CUE: &str = "Hallucination.impact";
const FIREWALL_SPELL_ACTION_MS: u64 = 600;
const FIREWALL_CAST_SOUND_FILE: &str = "M39-0.wav";
const FIREWALL_COMPLETE_SOUND_FILE: &str = "M39-1.wav";
const FIREWALL_CAST_SOUND_CUE: &str = "FireWall.cast";
const FIREWALL_COMPLETE_SOUND_CUE: &str = "FireWall.complete";
const HEALING_CAST_SOUND_FILE: &str = "M61-0.wav";
const HEALING_TARGET_SOUND_FILE: &str = "M61-1.wav";
const HEALING_CAST_SOUND_CUE: &str = "Healing.cast";
const HEALING_TARGET_SOUND_CUE: &str = "Healing.target";
const MAGIC_SHIELD_UP_EFFECT: u32 = 6;
const MAGIC_SHIELD_DOWN_EFFECT: u32 = 7;
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
const SCARECROW_ATTACK_SOUND_FILE: &str = "005-1.wav";
const SCARECROW_FLINCH_SOUND_FILE: &str = "005-2.wav";
const SCARECROW_DIE_SOUND_FILE: &str = "005-3.wav";
const MONSTER_STRUCK_SHORT_FILE: &str = "60.wav";
const MONSTER_STRUCK_WOODEN_FILE: &str = "61.wav";
const MONSTER_STRUCK_SWORD_FILE: &str = "62.wav";
const MONSTER_STRUCK_SWORD_2_FILE: &str = "63.wav";
const MONSTER_STRUCK_AXE_FILE: &str = "64.wav";
const MONSTER_STRUCK_CLUB_FILE: &str = "65.wav";
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

fn crystal_projectile_clock(distance: u64, process_interval_ms: u64) -> (u64, u64) {
    let duration_ms = distance.saturating_mul(FIREBALL_TILE_TRAVEL_MS).max(1);
    let process_frame_count = (duration_ms / process_interval_ms.max(1)).max(1);
    let frame_interval_ms = (duration_ms / process_frame_count).max(1);
    (duration_ms, frame_interval_ms)
}

fn projectile_process_interval_ms(spell: &str) -> u64 {
    match spell {
        "Hallucination" => HALLUCINATION_PROJECTILE_STEP_MS,
        _ => FIREBALL_PROJECTILE_STEP_MS,
    }
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

fn actor_is_archer(actor: &Value) -> bool {
    actor_is_player(actor)
        && actor
            .get("classKey")
            .or_else(|| actor.get("class"))
            .and_then(Value::as_str)
            .is_some_and(|class| class.eq_ignore_ascii_case("archer"))
}

fn effect_object_id(payload: &Value) -> Option<u32> {
    payload
        .get("objectId")
        .and_then(Value::as_u64)
        .and_then(|id| u32::try_from(id).ok())
        .filter(|id| *id != 0)
}

fn packet_direction(payload: &Value) -> Option<u32> {
    payload.get("direction").and_then(|direction| {
        direction
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value < 8)
            .or_else(|| direction.as_str().map(direction_index))
    })
}

fn five_class_ranged_spell(spell: &str) -> bool {
    matches!(
        spell,
        "StraightShot"
            | "DoubleShot"
            | "ElementalShot"
            | "Stonetrap"
            | "BindingShot"
            | "SummonVampire"
            | "SummonSnakes"
            | "VampireShot"
            | "PoisonShot"
            | "CrippleShot"
            | "DelayedExplosion"
            | "CatTongue"
    )
}

fn actor_has_source_buff(actor: &Value, name: &str, source_id: u64) -> Option<bool> {
    // ObjectPlayer carries Crystal BuffType bytes (VampireShot=16,
    // PoisonShot=17). The native owner snapshot carries typed buff names.
    let buffs = actor.get("buffs")?.as_array()?;
    Some(buffs.iter().any(|buff| {
        buff.as_u64() == Some(source_id)
            || buff.as_str() == Some(name)
            || buff.get("type").and_then(Value::as_str) == Some(name)
    }))
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

fn monster_struck_file_for_weapon(weapon: i32) -> Option<&'static str> {
    match weapon {
        0 | 23 | 28 | 40 => Some(MONSTER_STRUCK_WOODEN_FILE),
        1 | 6 | 10 | 12 | 17 | 22 | 27 | 30 | 36 | 39 => Some(MONSTER_STRUCK_SHORT_FILE),
        2 | 8 | 11 | 15 | 18 | 20 | 25 | 31 | 33 | 34 | 37 | 41 => Some(MONSTER_STRUCK_SWORD_FILE),
        3 | 5 | 7 | 9 | 13 | 19 | 24 | 26 | 29 | 32 | 35 => Some(MONSTER_STRUCK_SWORD_2_FILE),
        4 | 14 | 16 | 38 => Some(MONSTER_STRUCK_AXE_FILE),
        21 => Some(MONSTER_STRUCK_CLUB_FILE),
        _ => None,
    }
}

fn monster_struck_weapon_file(attacker: Option<&Value>) -> Option<&'static str> {
    monster_struck_file_for_weapon(player_weapon_index(attacker))
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
    #[cfg(test)]
    frame_asset_root: std::path::PathBuf,
}

fn library_dir(library: &str) -> String {
    library.replace(":", "_")
}

#[cfg(test)]
fn exported_effect_test_root() -> std::path::PathBuf {
    // Contract tests consume the actual Source exports in this checkout. A
    // focused effects fixture need not include every terrain pack required to
    // start the game; it must never silently read an installed/R2 catalog.
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/public")
}

#[cfg(test)]
fn exported_effect_test_path(root: &std::path::Path, web_path: &str) -> Option<std::path::PathBuf> {
    let relative = web_path.trim_start_matches('/');
    if relative.is_empty() || relative.contains(['\\', ':']) {
        return None;
    }
    let relative = std::path::Path::new(relative);
    if !relative
        .components()
        .all(|component| matches!(component, std::path::Component::Normal(_)))
    {
        return None;
    }
    let path = root.join(relative);
    path.is_file().then_some(path)
}

impl EffectCatalog {
    /// Load the exported effect manifest + per-library frame metadata. Returns
    /// None (non-throwing) when the manifest/libraries are unavailable, so the
    /// native client never panics on a missing asset bundle.
    pub(crate) fn load() -> Option<EffectCatalog> {
        #[cfg(not(test))]
        let asset_path = assets::asset_path;
        #[cfg(test)]
        let frame_asset_root = exported_effect_test_root();
        #[cfg(test)]
        let asset_path = |web_path: &str| exported_effect_test_path(&frame_asset_root, web_path);
        let manifest_path = asset_path("original-effects/effects.generated.json")?;
        let bytes = fs::read(&manifest_path).ok()?;
        let manifest: EffectsManifest = serde_json::from_slice(&bytes).ok()?;

        let mut libraries = HashMap::new();
        for library in &manifest.available {
            let dir = library_dir(library);
            let Some(meta_path) = asset_path(&format!("original-effects/{dir}/meta.json")) else {
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
            #[cfg(test)]
            frame_asset_root,
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
            #[cfg(not(test))]
            let frame_exists = crate::frame_png_exists(&frame.path);
            #[cfg(test)]
            let frame_exists =
                exported_effect_test_path(&self.frame_asset_root, &frame.path).is_some();
            if !frame_exists {
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

    fn warrior_attack_animation(
        &self,
        spell: &str,
        direction: u32,
        level: u32,
    ) -> Option<Animation> {
        if spell == "FlamingSword" {
            return self.spell_attack_overlay_animation(spell, direction);
        }
        let entry = self.map_by_name.get(&format!("Warrior{spell}Attack"))?;
        self.resolve_animation(entry, direction, level)
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
        if let Some(sub) = entry.projectile.as_ref() {
            return self.resolve_sub(sub, spell, "projectile", direction);
        }

        // The current generated manifest predates `groundTalisman` exporting
        // its nested launch phase. Crystal's six point-ground talismans still
        // have their exact launch data in the legacy top-level projectile
        // entry: Magic 1160, three frames, 30 ms, Direction16 stride 10.
        // Keep this compatibility read tightly closed to that proven source
        // shape; ordinary projectiles remain nested-phase only.
        if ground_talisman(spell).is_none()
            || entry.kind.as_deref() != Some("projectile")
            || entry.library != "Magic"
            || entry.base != 1160
            || entry.count != 3
            || entry.interval != 30
        {
            return None;
        }
        let mut legacy_projectile = entry.clone();
        legacy_projectile.direction_count = Some(16);
        legacy_projectile.direction_stride = Some(10);
        self.resolve_animation(&legacy_projectile, direction, 0)
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

    fn client_direction_animation(&self, name: &str, direction: u32) -> Option<Animation> {
        self.resolve_animation(self.map_by_name.get(name)?, direction, 0)
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

    fn archer_normal_arrow_animation(&self, direction: u32) -> Option<Animation> {
        // PlayerObject.CreateProjectile(1030, Magic3, true, 5, 30, 5).
        // These are source frame indices, not a replacement image fallback.
        let entry = self.map_by_name.get("ArcherNormalArrow")?;
        if entry.library != "Magic3"
            || entry.base != 1030
            || entry.count != 5
            || entry.interval != 30
            || entry.direction_count != Some(16)
            || entry.direction_stride != Some(10)
        {
            return None;
        }
        let animation = self.resolve_animation(entry, direction, 0)?;
        (animation.kind == "projectile"
            && animation.blend
            && (animation.opacity - 1.0).abs() < f32::EPSILON
            && animation.light == Some(6)
            && !animation.repeat)
            .then_some(animation)
    }

    fn archer_focus_proc_animation(&self) -> Option<Animation> {
        // PlayerObject adds ten Magic3 frames for Frame.Count * FrameInterval:
        // eight actor frames * 100 ms, hence an 80 ms effect-frame interval.
        let entry = self.map_by_name.get("ArcherFocusProc")?;
        if entry.library != "Magic3"
            || entry.base != 2730
            || entry.count != 10
            || entry.interval != 80
        {
            return None;
        }
        let animation = self.resolve_animation(entry, 0, 0)?;
        (animation.kind == "attackOverlay"
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
    /// Crystal's target-free normal arrow keeps extending its destination
    /// until it is outside Globals.DataRange. A vanished bound target instead
    /// retains its last known location through Missile's strong reference.
    unbound_archer_arrow: bool,
    unbound_archer_arrow_process_at: Option<u64>,
}

/// Separate from the classic spell callbacks: these arrows are launched at
/// actual Archer body-frame boundaries, and a Vampire return is a stationary
/// caster effect after target completion, not a missile travelling backwards.
#[derive(Debug, Clone)]
struct FiveClassProjectile {
    spell: String,
    source_id: u32,
    target_id: Option<u32>,
    fallback: (i32, i32),
    launched: bool,
    target_bound_at_launch: bool,
    process_interval_ms: u64,
    last_process_at: Option<u64>,
    explode_at_ground: bool,
    arrow_index: usize,
}

#[derive(Debug, Clone, Copy)]
struct NativeElementState {
    enabled: bool,
    casted: bool,
    owner: bool,
}

#[derive(Debug, Clone)]
struct NativeEffectLoopControl {
    generation: u64,
    sequence: u64,
    file_name: Option<&'static str>,
}

#[derive(Debug, Clone)]
struct DoubleSlashBodyEffect {
    object_id: u32,
    source_sequence: u64,
    keys: [String; 2],
    started_at: Option<u64>,
}

/// Crystal PlayerObject Spell-completion talismans. Unlike targeted bolts,
/// their completion effect is anchored to the packet TargetPoint, even when
/// the tracked object has moved or died. SoundList.lst only registers 20710
/// from these calls, mapping it to M69-0. Unregistered source IDs stay silent.
fn ground_talisman(spell: &str) -> Option<(&'static str, Option<(&'static str, &'static str)>)> {
    Some(match spell {
        "MassHiding" => ("MassHiding", None),
        "SoulShield" => ("SoulShield", None),
        "BlessedArmour" => (
            "BlessedArmour",
            Some(("BlessedArmour.projectile", "M69-0.wav")),
        ),
        "Curse" => ("Curse", None),
        "Plague" => ("Plague", None),
        "PoisonCloud" => ("PoisonCloud", None),
        _ => return None,
    })
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
    /// SoulFireBall, and the first FireBounce missile locally after the Spell
    /// action completes, so consume the adjacent compatibility packet instead
    /// of drawing twice. Later FireBounce hops have different sources and remain
    /// authoritative ObjectProjectile events.
    local_projectile_dedupe: HashMap<(String, u32, u32), u64>,
    /// Target presence is resolved at Crystal's Spell-completion boundary,
    /// not when ObjectMagic first arrives. This preserves point-flight when a
    /// target disappears during wind-up and binds a target that appears before
    /// launch.
    local_projectile_targets: HashMap<String, LocalProjectileTarget>,
    five_class_projectiles: HashMap<String, FiveClassProjectile>,
    five_class_actor_contexts: HashMap<u32, Value>,
    elemental_state: HashMap<u32, NativeElementState>,
    elemental_orb_thresholds: HashMap<(u32, u32), u32>,
    elemental_loop_owner: Option<(u32, u32)>,
    concentration_paused_at: HashMap<u32, u64>,
    hidden_five_class_effect_keys: HashSet<String>,
    delayed_explosion_stages: HashMap<u32, u32>,
    elemental_barrier_objects: HashSet<u32>,
    elemental_barrier_down_objects: HashSet<u32>,
    ready_element_loop_controls: Vec<NativeEffectLoopControl>,
    double_slash_body_effects: Vec<DoubleSlashBodyEffect>,
    /// Targets whose shared presentation clock is currently in Crystal's
    /// terminal `Dead` action. A plain `dead` packet flag is insufficient:
    /// FireBall-family impacts remain visible throughout the `Die` action.
    dead_action_object_ids: HashSet<u32>,
    /// Audio-only cast branches (SoulFireBall has no cast bitmap) still need a
    /// one-shot queue that does not depend on an active render instance.
    ready_sounds: Vec<mir2_client_bevy::audio::NativeGameplaySoundEvent>,
    pending_sounds: Vec<PendingEffectSound>,
    dead_player_sound_keys: HashSet<String>,
    dead_scarecrow_sound_keys: HashSet<String>,
    /// ObjectRemove/ObjectHide terminates the current actor incarnation. Keep
    /// that audio gate across native ingest snapshots until a render payload
    /// proves the object id disappeared and then authoritatively reappeared.
    terminated_scarecrow_sound_keys: HashSet<String>,
    render_actor_keys: HashSet<String>,
    /// `ObjectPlayer.effect` is the only authoritative recovery signal when an
    /// actor enters AOI with an already-running MagicShield. A received Down
    /// suppresses a stale render snapshot until the actor leaves or a new Up
    /// proves that the shield was applied again.
    render_actor_object_ids: HashSet<u32>,
    render_magic_shield_object_ids: HashSet<u32>,
    magic_shield_down_object_ids: HashSet<u32>,
    /// Map/logout boundaries remain terminal across observe calls. A changed
    /// authoritative render-scene identity reopens the next map only after the
    /// boundary's own event batch has been consumed.
    scene_audio_terminated: bool,
    render_scene_identity: Option<String>,
    render_scene_changed: bool,
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
            five_class_projectiles: HashMap::new(),
            five_class_actor_contexts: HashMap::new(),
            elemental_state: HashMap::new(),
            elemental_orb_thresholds: HashMap::new(),
            elemental_loop_owner: None,
            concentration_paused_at: HashMap::new(),
            hidden_five_class_effect_keys: HashSet::new(),
            delayed_explosion_stages: HashMap::new(),
            elemental_barrier_objects: HashSet::new(),
            elemental_barrier_down_objects: HashSet::new(),
            ready_element_loop_controls: Vec::new(),
            double_slash_body_effects: Vec::new(),
            dead_action_object_ids: HashSet::new(),
            ready_sounds: Vec::new(),
            pending_sounds: Vec::new(),
            dead_player_sound_keys: HashSet::new(),
            dead_scarecrow_sound_keys: HashSet::new(),
            terminated_scarecrow_sound_keys: HashSet::new(),
            render_actor_keys: HashSet::new(),
            render_actor_object_ids: HashSet::new(),
            render_magic_shield_object_ids: HashSet::new(),
            magic_shield_down_object_ids: HashSet::new(),
            scene_audio_terminated: false,
            render_scene_identity: None,
            render_scene_changed: false,
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
    fn clear_effect_key(&mut self, key: &str) {
        self.active.retain(|instance| instance.key != key);
        self.anchor_object_ids.remove(key);
        self.source_object_ids.remove(key);
        self.prestart_source_object_ids.remove(key);
        self.anchor_player_keys.remove(key);
        self.local_projectile_targets.remove(key);
        self.five_class_projectiles.remove(key);
        self.pending_sounds.retain(|pending| pending.key != key);
        self.hidden_five_class_effect_keys.remove(key);
    }

    fn push_actor_animation(
        &mut self,
        key: String,
        object_id: u32,
        animation: Animation,
        start_at: u64,
        provenance: &EffectProvenance,
    ) -> bool {
        let Some((tile_x, tile_y)) = self.zone_tiles.get(&object_id).copied() else {
            return false;
        };
        self.anchor_object_ids.insert(key.clone(), object_id);
        self.active.push(EffectInstance {
            key,
            kind: EffectKindTag::SceneForeground,
            tile_x,
            tile_y,
            from_x: None,
            from_y: None,
            current: Some(animation),
            queued: None,
            return_queued: None,
            started_at: self.now_ms,
            start_at,
            persistent_object_id: None,
            provenance: provenance.clone(),
        });
        true
    }

    fn queue_five_class_sound(
        &mut self,
        key: &str,
        due_at_ms: u64,
        cue: &str,
        file: &str,
        provenance: &EffectProvenance,
    ) {
        self.pending_sounds.push(PendingEffectSound {
            key: key.to_owned(),
            due_at_ms,
            requires_active_effect: true,
            event: mir2_client_bevy::audio::NativeGameplaySoundEvent {
                generation: provenance.generation,
                sequence: provenance.sequence,
                cue: cue.to_owned(),
                file_name: file.to_owned(),
            },
        });
    }

    fn apply_double_slash_overlay(
        &mut self,
        payload: &Value,
        provenance: &EffectProvenance,
    ) -> bool {
        let double_slash = payload.get("spell").is_some_and(|spell| {
            spell.as_str() == Some("DoubleSlash") || spell.as_u64() == Some(92)
        });
        if !double_slash {
            return false;
        }
        let Some(id) = effect_object_id(payload) else {
            return true;
        };
        let Some(actor) = self.five_class_actor_contexts.get(&id) else {
            return true;
        };
        if !actor_is_player(actor)
            || !actor_is_assassin(actor)
            || actor.get("ridingMount").and_then(Value::as_bool) == Some(true)
        {
            return true;
        }
        let (Some(direction), Some(catalog)) = (packet_direction(payload), effect_catalog()) else {
            return true;
        };
        let (Some(first), Some(second)) = (
            catalog.client_direction_animation("AssassinDoubleSlashAttack1", direction),
            catalog.client_direction_animation("AssassinDoubleSlashAttack4", direction),
        ) else {
            return true;
        };
        if [first.clone(), second.clone()].iter().any(|animation| {
            animation.frames.len() != 6
                || animation.interval != 46
                || animation.kind != "attackOverlay"
                || !animation.blend
                || (animation.opacity - 0.7).abs() > f32::EPSILON
        }) {
            return true;
        }
        let body_sequence = actor
            .get("_nativeAnimationSequence")
            .and_then(Value::as_u64);
        let first_key = self.next_key("double-slash-first");
        let second_key = self.next_key("double-slash-second");
        // A real body hint owns the action start. A queued attack must not
        // flash its overlay or play its second strike during locomotion.
        let first_at = if body_sequence.is_some() {
            u64::MAX
        } else {
            self.now_ms
        };
        let second_at = first_at.saturating_add(first.duration_ms);
        if self.push_actor_animation(first_key.clone(), id, first, first_at, provenance) {
            self.queue_five_class_sound(
                &first_key,
                first_at,
                "DoubleSlash.attack1",
                "M92-0.wav",
                provenance,
            );
        }
        if self.push_actor_animation(second_key.clone(), id, second, second_at, provenance) {
            self.queue_five_class_sound(
                &second_key,
                second_at,
                "DoubleSlash.attack4",
                "M92-1.wav",
                provenance,
            );
        }
        if let Some(source_sequence) = body_sequence {
            self.double_slash_body_effects.push(DoubleSlashBodyEffect {
                object_id: id,
                source_sequence,
                keys: [first_key, second_key],
                started_at: None,
            });
            while self.double_slash_body_effects.len() > MAX_ACTIVE_EFFECTS / 2 {
                let removed = self.double_slash_body_effects.remove(0);
                for key in removed.keys {
                    self.clear_effect_key(&key);
                }
            }
        }
        true
    }

    fn reconcile_double_slash_body_clocks(
        &mut self,
        presentation: &crate::entity_presentation::NativeEntityPresentation,
    ) {
        use crate::entity_presentation::NativeDoubleSlashBodyClock;
        let mut retired = Vec::new();
        for effect in &mut self.double_slash_body_effects {
            match presentation.double_slash_body_clock(effect.object_id, effect.source_sequence) {
                NativeDoubleSlashBodyClock::Pending => {}
                NativeDoubleSlashBodyClock::Retired => retired.extend(effect.keys.clone()),
                NativeDoubleSlashBodyClock::Started(started_at) => {
                    if effect.started_at == Some(started_at) {
                        continue;
                    }
                    effect.started_at = Some(started_at);
                    for (index, key) in effect.keys.iter().enumerate() {
                        let start_at = started_at.saturating_add(index as u64 * 276);
                        if let Some(instance) =
                            self.active.iter_mut().find(|instance| &instance.key == key)
                        {
                            instance.start_at = start_at;
                        }
                        for sound in self
                            .pending_sounds
                            .iter_mut()
                            .filter(|sound| &sound.key == key)
                        {
                            sound.due_at_ms = start_at;
                        }
                    }
                }
            }
        }
        for key in retired {
            self.clear_effect_key(&key);
        }
        let active = &self.active;
        self.double_slash_body_effects.retain(|effect| {
            effect
                .keys
                .iter()
                .any(|key| active.iter().any(|instance| &instance.key == key))
        });
    }

    fn five_class_projectile_animation(
        catalog: &EffectCatalog,
        spell: &str,
        direction: u32,
    ) -> Option<Animation> {
        match spell {
            "DelayedExplosion" => {
                catalog.client_direction_animation("ArcherDelayedExplosionArrow", direction)
            }
            "ElementalShot" => {
                catalog.client_direction_animation("ArcherElementalShotArrow", direction)
            }
            _ => catalog.spell_projectile_animation(spell, direction),
        }
    }

    fn apply_five_class_ranged_magic(
        &mut self,
        payload: &Value,
        catalog: &EffectCatalog,
        provenance: &EffectProvenance,
        spell: &str,
    ) -> bool {
        if !five_class_ranged_spell(spell) {
            return false;
        }
        let Some(source_id) = effect_object_id(payload) else {
            return true;
        };
        let Some(actor) = self.five_class_actor_contexts.get(&source_id) else {
            return true;
        };
        if if spell == "CatTongue" {
            !actor_is_player(actor) || !actor_is_assassin(actor)
        } else {
            !actor_is_archer(actor)
        } {
            return true;
        }
        if matches!(spell, "StraightShot" | "CatTongue")
            && payload.get("cast").and_then(Value::as_bool) != Some(true)
        {
            return true;
        }
        if spell == "ElementalShot" {
            if !self
                .elemental_state
                .get(&source_id)
                .is_some_and(|state| state.enabled && (!state.casted || state.owner))
            {
                return true;
            }
            if let Some(animation) = catalog.map_animation("ArcherElementalShotCast", 0) {
                let key = self.next_key("elemental-shot-cast");
                let start_at = self.now_ms.saturating_add(100);
                if self.push_actor_animation(
                    key.clone(),
                    source_id,
                    animation,
                    start_at,
                    provenance,
                ) {
                    self.queue_five_class_sound(
                        &key,
                        start_at,
                        "ElementalShot.cast",
                        "M128-0.wav",
                        provenance,
                    );
                }
            }
        }
        let (Some(from_x), Some(from_y), Some(to_x), Some(to_y)) = (
            value_f32(payload, "location", "x").map(|x| x as i32),
            value_f32(payload, "location", "y").map(|y| y as i32),
            value_f32(payload, "target", "x").map(|x| x as i32),
            value_f32(payload, "target", "y").map(|y| y as i32),
        ) else {
            return true;
        };
        let target_id = payload
            .get("targetId")
            .and_then(Value::as_u64)
            .and_then(|id| u32::try_from(id).ok())
            .filter(|id| *id != 0);
        let launch_offsets: &[u64] = match spell {
            "StraightShot" => &[800],
            "CatTongue" => &[600],
            "DoubleShot" => &[500, 700],
            "DelayedExplosion" => &[500],
            _ => &[700],
        };
        for (index, delay) in launch_offsets.iter().enumerate() {
            let Some(mut animation) = Self::five_class_projectile_animation(
                catalog,
                spell,
                projectile_direction16((from_x, from_y), (to_x, to_y)),
            ) else {
                continue;
            };
            let process_interval_ms = match spell {
                "Stonetrap" => 20,
                "VampireShot" | "PoisonShot" | "CrippleShot" | "BindingShot" | "SummonVampire"
                | "SummonSnakes" => 10,
                _ => 30,
            };
            let (duration, interval) = crystal_projectile_clock(
                max_tile_distance((from_x, from_y), (to_x, to_y)),
                process_interval_ms,
            );
            animation.duration_ms = duration;
            animation.interval = interval;
            let key = self.next_key("five-class-arrow");
            let start_at = self.now_ms.saturating_add(*delay);
            self.source_object_ids.insert(key.clone(), source_id);
            self.five_class_projectiles.insert(
                key.clone(),
                FiveClassProjectile {
                    spell: spell.to_owned(),
                    source_id,
                    target_id,
                    fallback: (to_x, to_y),
                    launched: false,
                    target_bound_at_launch: false,
                    process_interval_ms,
                    last_process_at: None,
                    explode_at_ground: spell == "Stonetrap",
                    arrow_index: index,
                },
            );
            // Each DoubleShot arrow owns a distinct cue under the same packet
            // sequence, otherwise the bounded audio queue drops the second.
            let (cue, file) = match spell {
                "StraightShot" => ("StraightShot.launch".to_owned(), "M122-0.wav"),
                "CrippleShot" => ("CrippleShot.launch".to_owned(), "M136-0.wav"),
                "CatTongue" => ("CatTongue.launch".to_owned(), "M107-0.wav"),
                "ElementalShot" => (String::new(), ""),
                _ => (format!("{spell}.launch{index}"), "M121-0.wav"),
            };
            // Source M107 WAVs are absent. Never substitute another skill.
            if !file.is_empty() && !file.starts_with("M107-") {
                self.queue_five_class_sound(&key, start_at, &cue, file, provenance);
            }
            let mut arrow_provenance = provenance.clone();
            arrow_provenance.spell = spell.to_owned();
            self.active.push(EffectInstance {
                key,
                kind: EffectKindTag::Projectile,
                tile_x: to_x,
                tile_y: to_y,
                from_x: Some(from_x as f32),
                from_y: Some(from_y as f32),
                current: Some(animation),
                queued: None,
                return_queued: None,
                started_at: self.now_ms,
                start_at,
                persistent_object_id: None,
                provenance: arrow_provenance,
            });
        }
        true
    }

    fn refresh_five_class_projectiles(&mut self, now_ms: u64) {
        let Some(catalog) = effect_catalog() else {
            return;
        };
        let mut completed = Vec::new();
        let mut canceled = Vec::new();
        for instance in &mut self.active {
            let Some(state) = self.five_class_projectiles.get_mut(&instance.key) else {
                continue;
            };
            if now_ms < instance.start_at || state.last_process_at == Some(now_ms) {
                continue;
            }
            state.last_process_at = Some(now_ms);
            let Some((source_x, source_y)) = self.zone_tiles.get(&state.source_id).copied() else {
                canceled.push(instance.key.clone());
                continue;
            };
            if !state.launched {
                let target_tile = state
                    .target_id
                    .and_then(|id| self.zone_tiles.get(&id).copied());
                state.target_bound_at_launch = target_tile.is_some();
                if let Some(target) = target_tile {
                    state.fallback = target;
                } else {
                    state.target_id = None;
                }
                let Some(animation) = Self::five_class_projectile_animation(
                    catalog,
                    &state.spell,
                    projectile_direction16((source_x, source_y), state.fallback),
                ) else {
                    canceled.push(instance.key.clone());
                    continue;
                };
                instance.current = Some(animation);
                instance.from_x = Some(source_x as f32);
                instance.from_y = Some(source_y as f32);
                state.launched = true;
            }
            if let Some(target_tile) = state
                .target_id
                .and_then(|id| self.zone_tiles.get(&id).copied())
            {
                state.fallback = target_tile;
            } else if !state.target_bound_at_launch && !state.explode_at_ground {
                let source = (
                    instance.from_x.unwrap_or(source_x as f32) as i32,
                    instance.from_y.unwrap_or(source_y as f32) as i32,
                );
                if max_tile_distance((source_x, source_y), state.fallback) < 16 {
                    state.fallback = (
                        state
                            .fallback
                            .0
                            .saturating_add(state.fallback.0.saturating_sub(source.0)),
                        state
                            .fallback
                            .1
                            .saturating_add(state.fallback.1.saturating_sub(source.1)),
                    );
                }
            }
            instance.tile_x = state.fallback.0;
            instance.tile_y = state.fallback.1;
            let Some(animation) = instance.current.as_mut() else {
                continue;
            };
            let from = (
                instance.from_x.unwrap_or(source_x as f32) as i32,
                instance.from_y.unwrap_or(source_y as f32) as i32,
            );
            let (duration, interval) = crystal_projectile_clock(
                max_tile_distance(from, state.fallback),
                state.process_interval_ms,
            );
            animation.duration_ms = duration;
            animation.interval = interval;
            if now_ms >= instance.start_at.saturating_add(duration) {
                completed.push((
                    instance.key.clone(),
                    state.clone(),
                    instance.provenance.clone(),
                ));
            }
        }
        for key in canceled {
            self.clear_effect_key(&key);
        }
        for (key, state, provenance) in completed {
            // Deliver pending launch first when a sparse render tick crosses
            // both boundaries. Clearing the arrow must not discard that cue.
            let mut pending_due = Vec::new();
            self.pending_sounds.retain(|sound| {
                if sound.key == key && now_ms >= sound.due_at_ms {
                    pending_due.push(sound.event.clone());
                    false
                } else {
                    true
                }
            });
            self.ready_sounds.extend(pending_due);
            let impact_at =
                self.active
                    .iter()
                    .find(|instance| instance.key == key)
                    .and_then(|instance| {
                        instance.current.as_ref().map(|animation| {
                            instance.start_at.saturating_add(animation.duration_ms)
                        })
                    })
                    .unwrap_or(now_ms);
            self.clear_effect_key(&key);
            self.complete_five_class_projectile(&state, impact_at, &provenance, catalog);
        }
    }

    fn complete_five_class_projectile(
        &mut self,
        state: &FiveClassProjectile,
        impact_at: u64,
        provenance: &EffectProvenance,
        catalog: &EffectCatalog,
    ) {
        let spell = state.spell.as_str();
        let target = state
            .target_id
            .filter(|id| self.zone_tiles.contains_key(id));
        let bound = state.target_bound_at_launch;
        let dead = target.is_some_and(|id| self.dead_action_object_ids.contains(&id));
        let (impact, sound, returns) = match spell {
            "StraightShot" if !dead => (
                catalog.spell_impact_animation(spell),
                Some("M122-2.wav"),
                false,
            ),
            "CatTongue" if !dead => (catalog.spell_impact_animation(spell), None, false),
            "VampireShot" => (
                catalog.spell_impact_animation(spell),
                Some("M133-2.wav"),
                true,
            ),
            "PoisonShot" => (
                catalog.spell_impact_animation(spell),
                Some("M135-2.wav"),
                false,
            ),
            "CrippleShot" => {
                let Some(actor) = self.five_class_actor_contexts.get(&state.source_id) else {
                    return;
                };
                let (Some(poison), Some(vampire)) = (
                    actor_has_source_buff(actor, "PoisonShot", 17),
                    actor_has_source_buff(actor, "VampireShot", 16),
                ) else {
                    return;
                };
                if poison {
                    (
                        catalog.map_animation("ArcherCripplePoisonImpact", 0),
                        Some("M136-6.wav"),
                        false,
                    )
                } else if vampire {
                    (
                        catalog.map_animation("ArcherCrippleVampireImpact", 0),
                        Some("M136-7.wav"),
                        true,
                    )
                } else {
                    (
                        catalog.spell_impact_animation(spell),
                        Some("M136-5.wav"),
                        false,
                    )
                }
            }
            "Stonetrap" => (None, Some("M130-7.wav"), false),
            "ElementalShot" => (None, Some("M128-2.wav"), false),
            "SummonVampire" => (None, Some("M132-7.wav"), false),
            "DoubleShot" | "DelayedExplosion" => (None, Some("M121-2.wav"), false),
            // M137-7 and M140-7 are absent in the actual source Sound tree.
            _ => (None, None, false),
        };
        if !bound && !state.explode_at_ground {
            return;
        }
        if let Some(file) = sound {
            self.queue_immediate_sound(
                provenance,
                &format!("{spell}.impact{}", state.arrow_index),
                file,
            );
        }
        let (Some(target), Some(impact)) = (target, impact) else {
            return;
        };
        let return_at = impact_at.saturating_add(impact.duration_ms);
        let key = self.next_key("five-class-impact");
        if !self.push_actor_animation(key, target, impact, impact_at, provenance) {
            return;
        }
        if returns {
            if let Some(mut animation) = catalog.map_animation("ArcherVampireReturn", 0) {
                // PlayerObject adds this Effect to the caster; it does not
                // return by traversing a reverse projectile path.
                animation.kind = "target".to_owned();
                let key = self.next_key("archer-vampire-return");
                if self.push_actor_animation(
                    key.clone(),
                    state.source_id,
                    animation,
                    return_at,
                    provenance,
                ) {
                    self.queue_five_class_sound(
                        &key,
                        return_at,
                        "VampireShot.return",
                        "M45-2.wav",
                        provenance,
                    );
                }
            }
        }
    }

    fn queue_element_loop(
        &mut self,
        file_name: Option<&'static str>,
        provenance: &EffectProvenance,
    ) {
        if file_name.is_none() {
            self.elemental_loop_owner = None;
        }
        // One source audio channel replaces the previous orb sound. The
        // protocol sequence is reused because this is a control for that
        // same authoritative SetElemental event, not a synthetic timer event.
        self.ready_element_loop_controls
            .push(NativeEffectLoopControl {
                generation: provenance.generation,
                sequence: provenance.sequence,
                file_name,
            });
        if self.ready_element_loop_controls.len() > 32 {
            self.ready_element_loop_controls.remove(0);
        }
    }

    fn apply_concentration(&mut self, payload: &Value, provenance: &EffectProvenance) {
        let Some(id) = effect_object_id(payload) else {
            return;
        };
        if !self
            .five_class_actor_contexts
            .get(&id)
            .is_some_and(actor_is_player)
        {
            return;
        }
        let (Some(enabled), Some(interrupted)) = (
            payload.get("enabled").and_then(Value::as_bool),
            payload.get("interrupted").and_then(Value::as_bool),
        ) else {
            return;
        };
        let key = format!("archer-concentration-{id}");
        if !enabled {
            self.clear_effect_key(&key);
            self.concentration_paused_at.remove(&id);
            return;
        }
        if interrupted {
            if self.active.iter().any(|instance| instance.key == key) {
                self.concentration_paused_at
                    .entry(id)
                    .or_insert(self.now_ms);
                self.hidden_five_class_effect_keys.insert(key);
            }
            return;
        }
        if let Some(paused_at) = self.concentration_paused_at.remove(&id) {
            if let Some(instance) = self.active.iter_mut().find(|instance| instance.key == key) {
                instance.start_at = instance
                    .start_at
                    .saturating_add(self.now_ms.saturating_sub(paused_at));
            }
            self.hidden_five_class_effect_keys.remove(&key);
        }
        if self.active.iter().any(|instance| instance.key == key) {
            return;
        }
        if let Some(animation) =
            effect_catalog().as_ref().and_then(|catalog| catalog.map_animation("ArcherConcentration", 0))
        {
            if self.push_actor_animation(key.clone(), id, animation, self.now_ms, provenance) {
                self.queue_five_class_sound(
                    &key,
                    self.now_ms,
                    "Concentration.start",
                    "M129-0.wav",
                    provenance,
                );
            }
        }
    }

    fn apply_elemental(&mut self, payload: &Value, provenance: &EffectProvenance) {
        let Some(id) = effect_object_id(payload) else {
            return;
        };
        let Some(actor) = self.five_class_actor_contexts.get(&id) else {
            return;
        };
        if !actor_is_player(actor) {
            return;
        }
        let (Some(enabled), Some(casted), Some(level), Some(element_type), Some(exp_last)) = (
            payload.get("enabled").and_then(Value::as_bool),
            payload.get("casted").and_then(Value::as_bool),
            payload
                .get("value")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok()),
            payload
                .get("elementType")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok()),
            payload
                .get("expLast")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok()),
        ) else {
            return;
        };
        let owner = actor.get("kind").and_then(Value::as_str) == Some("selfPlayer")
            || actor.get("_nativeSelf").and_then(Value::as_bool) == Some(true);
        self.elemental_state.insert(
            id,
            NativeElementState {
                enabled,
                casted,
                owner,
            },
        );
        let remove = self
            .elemental_orb_thresholds
            .iter()
            .filter_map(|(&(owner_id, ty), &threshold)| {
                (owner_id == id && (!enabled || (ty < 4 && level >= threshold)))
                    .then_some((owner_id, ty))
            })
            .collect::<Vec<_>>();
        for (owner_id, ty) in remove {
            self.clear_effect_key(&format!("archer-element-{owner_id}-{ty}"));
            self.elemental_orb_thresholds.remove(&(owner_id, ty));
            if self.elemental_loop_owner == Some((owner_id, ty)) {
                self.queue_element_loop(None, provenance);
            }
        }
        if !enabled {
            if self
                .elemental_loop_owner
                .is_some_and(|(owner_id, _)| owner_id == id)
            {
                self.queue_element_loop(None, provenance);
            }
            return;
        }
        let (name, file) = match element_type {
            1 => ("ArcherElementGreen", "M126-5.wav"),
            2 => ("ArcherElementBlue", "M126-6.wav"),
            3 => ("ArcherElementRed", "M126-7.wav"),
            4 => ("ArcherElementMixed", "M126-8.wav"),
            _ => return,
        };
        let key = format!("archer-element-{id}-{element_type}");
        self.clear_effect_key(&key);
        let Some(animation) = effect_catalog().as_ref().and_then(|catalog| catalog.map_animation(name, 0))
        else {
            return;
        };
        if self.push_actor_animation(key.clone(), id, animation, self.now_ms, provenance) {
            self.elemental_orb_thresholds
                .insert((id, element_type), exp_last);
            if owner && matches!(element_type, 3 | 4) {
                self.elemental_loop_owner = Some((id, element_type));
                self.queue_element_loop(Some(file), provenance);
            } else {
                self.queue_element_loop(None, provenance);
                self.queue_five_class_sound(
                    &key,
                    self.now_ms,
                    &format!("Meditation.element{element_type}"),
                    file,
                    provenance,
                );
            }
        }
    }

    fn apply_binding_shot(&mut self, payload: &Value, provenance: &EffectProvenance) {
        let Some(id) = effect_object_id(payload) else {
            return;
        };
        let key = format!("archer-binding-{id}");
        if payload.get("enabled").and_then(Value::as_bool) == Some(false) {
            self.clear_effect_key(&key);
            return;
        }
        if !self
            .five_class_actor_contexts
            .get(&id)
            .is_some_and(|actor| actor.get("kind").and_then(Value::as_str) == Some("monster"))
        {
            return;
        }
        let Some(value) = payload
            .get("value")
            .and_then(Value::as_i64)
            .and_then(|value| u64::try_from(value).ok())
        else {
            return;
        };
        let (Some(catalog), Some((tile_x, tile_y))) =
            (effect_catalog(), self.zone_tiles.get(&id).copied())
        else {
            return;
        };
        let (Some(drop), Some(mut down), Some(fall)) = (
            catalog.map_animation("ArcherBindingDrop", 0),
            catalog.map_animation("ArcherBindingDown", 0),
            catalog.map_animation("ArcherBindingFall", 0),
        ) else {
            return;
        };
        // Source RepeatUntil is relative to receipt, not to the end of Drop.
        let down_until = self.now_ms.saturating_add(value.saturating_sub(1_500));
        let down_start = self.now_ms.saturating_add(drop.duration_ms);
        down.duration_ms = down_until.saturating_sub(down_start).max(1);
        down.interval = down.duration_ms;
        down.repeat = false;
        self.clear_effect_key(&key);
        self.anchor_object_ids.insert(key.clone(), id);
        self.active.push(EffectInstance {
            key: key.clone(),
            kind: EffectKindTag::SceneForeground,
            tile_x,
            tile_y,
            from_x: None,
            from_y: None,
            current: Some(drop),
            queued: Some(down),
            return_queued: Some(fall),
            started_at: self.now_ms,
            start_at: self.now_ms,
            persistent_object_id: None,
            provenance: provenance.clone(),
        });
        self.queue_five_class_sound(
            &key,
            down_start,
            "BindingShot.drop",
            "M130-5.wav",
            provenance,
        );
        self.queue_five_class_sound(
            &key,
            down_until.max(down_start.saturating_add(1)),
            "BindingShot.fall",
            "M130-6.wav",
            provenance,
        );
    }

    fn apply_elemental_barrier_struck(&mut self, payload: &Value, provenance: &EffectProvenance) {
        let Some(id) = effect_object_id(payload)
            .or_else(|| payload.get("_nativeTarget").and_then(effect_object_id))
        else {
            return;
        };
        if !self.elemental_barrier_objects.contains(&id) {
            return;
        }
        let (Some(catalog), Some((tile_x, tile_y))) =
            (effect_catalog(), self.zone_tiles.get(&id).copied())
        else {
            return;
        };
        let (Some(struck), Some(loop_animation)) = (
            catalog.map_animation("ArcherBarrierStruck", 0),
            catalog.map_animation("ArcherBarrierPersistent16", 0),
        ) else {
            return;
        };
        let key = format!("archer-barrier-{id}");
        self.clear_effect_key(&key);
        self.anchor_object_ids.insert(key.clone(), id);
        self.active.push(EffectInstance {
            key,
            kind: EffectKindTag::SceneForeground,
            tile_x,
            tile_y,
            from_x: None,
            from_y: None,
            current: Some(struck),
            queued: Some(loop_animation),
            return_queued: None,
            started_at: self.now_ms,
            start_at: self.now_ms,
            persistent_object_id: None,
            provenance: provenance.clone(),
        });
    }

    fn apply_five_class_object_effect(
        &mut self,
        effect: u32,
        object_id: u32,
        payload: &Value,
        catalog: &EffectCatalog,
        provenance: &EffectProvenance,
    ) -> bool {
        if !matches!(effect, 1 | 13 | 14 | 15 | 16 | 17 | 18) {
            return false;
        }
        if matches!(effect, 13 | 14) {
            if !self
                .five_class_actor_contexts
                .get(&object_id)
                .is_some_and(actor_is_player)
            {
                return true;
            }
            let key = format!("archer-barrier-{object_id}");
            self.clear_effect_key(&key);
            if effect == 14 {
                self.elemental_barrier_objects.remove(&object_id);
                self.elemental_barrier_down_objects.insert(object_id);
                if let Some(animation) = catalog.map_animation("ElementalBarrierDown", 0) {
                    let key = self.next_key("archer-barrier-down");
                    if self.push_actor_animation(
                        key.clone(),
                        object_id,
                        animation,
                        self.now_ms,
                        provenance,
                    ) {
                        self.queue_five_class_sound(
                            &key,
                            self.now_ms,
                            "ElementalBarrier.down",
                            "M131-5.wav",
                            provenance,
                        );
                    }
                }
            } else {
                self.elemental_barrier_objects.insert(object_id);
                self.elemental_barrier_down_objects.remove(&object_id);
                if let Some(animation) = catalog.map_animation("ElementalBarrierUp", 0) {
                    self.push_actor_animation(key, object_id, animation, self.now_ms, provenance);
                }
            }
            return true;
        }
        if effect == 15 {
            let Some(stage @ 0..=2) = payload
                .get("effectType")
                .and_then(Value::as_u64)
                .and_then(|stage| u32::try_from(stage).ok())
            else {
                return true;
            };
            let key = format!("delayed-explosion-{object_id}");
            let current = self
                .delayed_explosion_stages
                .get(&object_id)
                .copied()
                .filter(|_| self.active.iter().any(|instance| instance.key == key));
            if current.is_some_and(|current| stage <= current) {
                return true;
            }
            // Crystal's first packet creates stage zero even when EffectType
            // arrives above zero; only an existing effect advances its stage.
            let stage = current.map_or(0, |_| stage);
            let Some(animation) =
                catalog.map_animation(&format!("ArcherDelayedExplosionStage{stage}"), 0)
            else {
                return true;
            };
            self.clear_effect_key(&key);
            if self.push_actor_animation(key.clone(), object_id, animation, self.now_ms, provenance)
            {
                self.delayed_explosion_stages.insert(object_id, stage);
                let file = match stage {
                    1 => Some("M125-0.wav"),
                    2 => Some("M125-5.wav"),
                    _ => None,
                };
                if let Some(file) = file {
                    self.queue_five_class_sound(
                        &key,
                        self.now_ms,
                        &format!("DelayedExplosion.stage{stage}"),
                        file,
                        provenance,
                    );
                }
            }
            return true;
        }
        let name = match effect {
            1 => "FatalSword",
            16 => "MPEater",
            17 => "Hemorrhage",
            _ => "Bleeding",
        };
        if let Some(animation) = catalog.map_animation(name, 0) {
            let key = self.next_key("five-class-object");
            if self.push_actor_animation(key.clone(), object_id, animation, self.now_ms, provenance)
            {
                let file = match effect {
                    1 | 16 => Some("M91-0.wav"),
                    17 => Some("M104-0.wav"),
                    _ => None,
                };
                if let Some(file) = file {
                    self.queue_five_class_sound(
                        &key,
                        self.now_ms,
                        &format!("{name}.target"),
                        file,
                        provenance,
                    );
                }
            }
        }
        if effect == 16 {
            if let Some(owner_id) = payload
                .get("effectType")
                .and_then(Value::as_u64)
                .and_then(|id| u32::try_from(id).ok())
            {
                if let Some(animation) = catalog.map_animation("MPEaterSecondary", 0) {
                    let key = self.next_key("mp-eater-owner");
                    self.push_actor_animation(key, owner_id, animation, self.now_ms, provenance);
                }
            }
        }
        if effect == 17 {
            for name in ["HemorrhageLayer2", "HemorrhageLayer3"] {
                if let Some(animation) = catalog.map_animation(name, 0) {
                    let key = self.next_key("hemorrhage-layer");
                    self.push_actor_animation(key, object_id, animation, self.now_ms, provenance);
                }
            }
        }
        true
    }

    fn apply_dark_body_disappear(&mut self, payload: &Value, provenance: &EffectProvenance) {
        let (Some(x), Some(y)) = (
            value_f32(payload, "location", "x"),
            value_f32(payload, "location", "y"),
        ) else {
            return;
        };
        self.queue_immediate_sound(provenance, "DarkBody.disappear", "M103-1.wav");
        let Some(animation) = effect_catalog().as_ref()
            .and_then(|catalog| catalog.map_animation("AssassinDarkBodyDisappear", 0))
        else {
            return;
        };
        let key = self.next_key("dark-body-disappear");
        // Source adds the smoke to MapControl before removing the actor. It
        // remains at that tile and must not be deleted by the actor tombstone.
        self.active.push(EffectInstance {
            key,
            kind: EffectKindTag::SceneForeground,
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

    fn apply_back_step_sound(&mut self, payload: &Value, provenance: &EffectProvenance) {
        let Some(actor) = payload
            .get("_nativeAttacker")
            .filter(|actor| actor_is_archer(actor))
        else {
            return;
        };
        let Some(id) = effect_object_id(actor) else {
            return;
        };
        let key = format!("back-step-sound-{id}");
        self.pending_sounds.retain(|sound| sound.key != key);
        for (delay, cue, file) in [
            (
                100,
                "BackStep.jump",
                if actor_is_female(actor) {
                    "M127-6.wav"
                } else {
                    "M127-5.wav"
                },
            ),
            (700, "BackStep.land", "M127-7.wav"),
        ] {
            self.pending_sounds.push(PendingEffectSound {
                key: key.clone(),
                due_at_ms: self.now_ms.saturating_add(delay),
                requires_active_effect: false,
                event: mir2_client_bevy::audio::NativeGameplaySoundEvent {
                    generation: provenance.generation,
                    sequence: provenance.sequence,
                    cue: cue.to_owned(),
                    file_name: file.to_owned(),
                },
            });
        }
    }

    fn apply_five_class_monster_death(&mut self, payload: &Value, provenance: &EffectProvenance) {
        let Some(actor) = payload.get("_nativeTarget") else {
            return;
        };
        if actor.get("kind").and_then(Value::as_str) != Some("monster") {
            return;
        }
        let (Some(id), Some(image)) = (
            effect_object_id(actor),
            actor.get("image").and_then(Value::as_u64),
        ) else {
            return;
        };
        self.clear_effect_key(&format!("snake-totem-aura-{id}"));
        let name = match image {
            359 => "ArcherVampireSpiderDeath",
            362 => "ArcherCharmedSnakeDeath",
            _ => return,
        };
        if let Some(animation) = effect_catalog().as_ref().and_then(|catalog| catalog.map_animation(name, 0))
        {
            let key = self.next_key("archer-summon-death");
            self.push_actor_animation(key, id, animation, self.now_ms, provenance);
        }
    }

    fn apply_trap_rock_range(&mut self, payload: &Value, provenance: &EffectProvenance) -> bool {
        let Some(actor) = payload.get("_nativeAttacker") else {
            return false;
        };
        if actor.get("kind").and_then(Value::as_str) != Some("monster")
            || actor.get("image").and_then(Value::as_u64) != Some(130)
        {
            return false;
        }
        let Some(target_id) = payload
            .get("targetId")
            .and_then(Value::as_u64)
            .and_then(|id| u32::try_from(id).ok())
        else {
            return true;
        };
        if let Some(animation) =
            effect_catalog().as_ref().and_then(|catalog| catalog.map_animation("ArcherTrapRockImpact", 0))
        {
            let key = self.next_key("trap-rock-impact");
            let start_at = self.now_ms.saturating_add(400);
            if self.push_actor_animation(key.clone(), target_id, animation, start_at, provenance) {
                if let Some(source_id) = effect_object_id(actor) {
                    self.prestart_source_object_ids
                        .insert(key.clone(), source_id);
                }
                self.queue_five_class_sound(
                    &key,
                    start_at,
                    "Stonetrap.rockImpact",
                    "130-6.wav",
                    provenance,
                );
            }
        }
        true
    }

    pub(crate) fn observe_render_payload(&mut self, payload: &Value) {
        self.five_class_actor_contexts = payload
            .get("entities")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|actor| Some((effect_object_id(actor)?, actor.clone())))
            .collect();
        self.post_world_depth_bounds = Some(crate::atlas::post_world_depth_bounds(payload));
        let map_file_name = payload
            .get("mapFileName")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let map_index = payload.get("mapIndex").and_then(Value::as_i64);
        let scene_identity = match (map_index, map_file_name) {
            (Some(index), Some(file_name)) => Some(format!("index:{index}|file:{file_name}")),
            (Some(index), None) => Some(format!("index:{index}")),
            (None, Some(file_name)) => Some(format!("file:{file_name}")),
            (None, None) => None,
        };
        if scene_identity != self.render_scene_identity {
            self.render_scene_identity = scene_identity;
            self.render_scene_changed = true;
        }

        let current_actor_keys = payload
            .get("entities")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|actor| actor_sound_key(actor))
            .collect::<HashSet<_>>();
        let current_actor_object_ids = payload
            .get("entities")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|actor| actor.get("objectId").and_then(Value::as_u64))
            .filter_map(|object_id| u32::try_from(object_id).ok())
            .collect::<HashSet<_>>();
        self.magic_shield_down_object_ids
            .retain(|object_id| current_actor_object_ids.contains(object_id));
        self.elemental_barrier_down_objects
            .retain(|object_id| current_actor_object_ids.contains(object_id));
        self.render_magic_shield_object_ids = payload
            .get("entities")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|actor| !actor.get("dead").and_then(Value::as_bool).unwrap_or(false))
            .filter(|actor| {
                actor.get("effect").and_then(Value::as_u64)
                    == Some(u64::from(MAGIC_SHIELD_UP_EFFECT))
            })
            .filter_map(|actor| actor.get("objectId").and_then(Value::as_u64))
            .filter_map(|object_id| u32::try_from(object_id).ok())
            .collect();
        self.render_actor_object_ids = current_actor_object_ids;
        for reappeared in current_actor_keys.difference(&self.render_actor_keys) {
            self.terminated_scarecrow_sound_keys.remove(reappeared);
        }
        self.render_actor_keys = current_actor_keys;
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
                if self.hidden_five_class_effect_keys.contains(&instance.key) {
                    return None;
                }
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
        // Crystal rejects a second ObjectStruck while the first Struck is
        // still the tail of that actor's ActionFeed. A native ingest batch is
        // the interval before the renderer consumes those queued actions, so
        // retain one tail marker per Scarecrow only for this observe call.
        // The next call starts a fresh action-consumption boundary and may
        // legitimately enqueue another hit reaction.
        let mut scarecrow_struck_tails = HashSet::new();
        let mut terminated_scarecrow_keys = self.terminated_scarecrow_sound_keys.clone();
        let mut scene_audio_terminated = self.scene_audio_terminated;
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
                scarecrow_struck_tails.clear();
                terminated_scarecrow_keys.clear();
                scene_audio_terminated = false;
                self.terminated_scarecrow_sound_keys.clear();
                self.scene_audio_terminated = false;
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
            let earlier_scene_boundary = events.iter().any(|candidate| {
                candidate.generation == event.generation
                    && candidate.sequence > previous_effect_sequence
                    && candidate.sequence < event.sequence
                    && matches!(candidate.packet.as_str(), "MapChanged" | "LogOutSuccess")
            });
            let suppress_scarecrow_struck =
                if matches!(event.packet.as_str(), "Struck" | "ObjectStruck") {
                    event
                        .payload
                        .get("_nativeTarget")
                        .filter(|actor| actor_is_scarecrow(actor))
                        .and_then(actor_sound_key)
                        .is_some_and(|actor_key| {
                            let actor_is_in_changed_scene = self.render_scene_changed
                                && self.render_actor_keys.contains(&actor_key)
                                && !earlier_scene_boundary;
                            (scene_audio_terminated && !actor_is_in_changed_scene)
                                || terminated_scarecrow_keys.contains(&actor_key)
                                || !scarecrow_struck_tails.insert(actor_key)
                        })
                } else {
                    false
                };
            match event.packet.as_str() {
                "MapChanged" | "LogOutSuccess" => {
                    scene_audio_terminated = true;
                    scarecrow_struck_tails.clear();
                    terminated_scarecrow_keys.clear();
                }
                "ObjectRemove" | "ObjectHide" => {
                    if let Some(actor_key) = event
                        .payload
                        .get("objectId")
                        .and_then(Value::as_u64)
                        .map(|object_id| object_id.to_string())
                    {
                        scarecrow_struck_tails.remove(&actor_key);
                        terminated_scarecrow_keys.insert(actor_key);
                    }
                }
                "ObjectAttack" | "ObjectRangeAttack" | "ObjectMagic" | "ObjectSpell"
                | "ObjectDied" | "ObjectRevived" => {
                    if let Some(actor_key) = event
                        .payload
                        .get("objectId")
                        .and_then(Value::as_u64)
                        .map(|object_id| object_id.to_string())
                    {
                        scarecrow_struck_tails.remove(&actor_key);
                    }
                }
                _ => {}
            }
            let is_left_guard_range = event.packet == "ObjectRangeAttack"
                && event
                    .payload
                    .get("_nativeAttacker")
                    .and_then(|attacker| actor_sprite_library(attacker, "bodyLibrary"))
                    .is_some_and(|library| {
                        matches!(library, "Monster/100" | "/original-ui/Monster/100")
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
                    suppress_scarecrow_struck,
                );
            } else {
                self.apply_event(
                    &event.packet,
                    &event.payload,
                    zone_tiles,
                    &provenance,
                    suppress_scarecrow_struck,
                );
            }
        }
        let latest_sequence = self.last_effect_sequence;
        self.local_projectile_dedupe
            .retain(|_, sequence| latest_sequence.saturating_sub(*sequence) <= 2);
        self.terminated_scarecrow_sound_keys = terminated_scarecrow_keys;
        self.scene_audio_terminated = scene_audio_terminated && !self.render_scene_changed;
        self.render_scene_changed = false;
        self.reconcile_magic_shield_render_state(zone_tiles);
        self.reconcile_elemental_barrier_render_state(zone_tiles);
        self.reconcile_elemental_orbs_render_state();
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
        self.terminated_scarecrow_sound_keys.clear();
        self.render_actor_keys.clear();
        self.render_actor_object_ids.clear();
        self.render_magic_shield_object_ids.clear();
        self.magic_shield_down_object_ids.clear();
        self.scene_audio_terminated = false;
        self.render_scene_identity = None;
        self.render_scene_changed = false;
        self.zone_tiles.clear();
        self.dead_action_object_ids.clear();
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

    /// Start Crystal's client-owned `Settings.NewMove` destination marker.
    /// This is presentation-only: the Zone still validates the movement intent
    /// independently and no local world position is fabricated.
    pub(crate) fn start_new_move_destination(
        &mut self,
        now_ms: u64,
        tile: (i32, i32),
        draw_offset: (f32, f32),
    ) -> bool {
        let Some(mut animation) = effect_catalog()
            .as_ref()
            .and_then(|catalog| catalog.map_animation("NewMoveDestination", 0))
        else {
            return false;
        };
        animation.offset_x += draw_offset.0;
        animation.offset_y += draw_offset.1;
        let key = self.next_key("new-move-destination");
        self.now_ms = now_ms;
        self.active.push(EffectInstance {
            key,
            kind: EffectKindTag::Ground,
            tile_x: tile.0,
            tile_y: tile.1,
            from_x: None,
            from_y: None,
            current: Some(animation),
            queued: None,
            return_queued: None,
            started_at: now_ms,
            start_at: now_ms,
            persistent_object_id: None,
            provenance: EffectProvenance {
                generation: self.last_generation,
                sequence: 0,
                packet: "LocalPointer".to_owned(),
                spell: "NewMoveDestination".to_owned(),
            },
        });
        while self.active.len() > MAX_ACTIVE_EFFECTS {
            self.active.remove(0);
        }
        true
    }

    fn next_key(&mut self, tag: &str) -> String {
        self.instance_seq = self.instance_seq.saturating_add(1);
        format!("fx-{tag}-{}", self.instance_seq)
    }

    fn clear_active_effects(&mut self) {
        self.queue_element_loop(
            None,
            &EffectProvenance {
                generation: self.last_generation,
                sequence: self.last_effect_sequence,
                packet: "boundary".to_owned(),
                spell: String::new(),
            },
        );
        self.active.clear();
        self.anchor_object_ids.clear();
        self.source_object_ids.clear();
        self.hidden_left_guard_tiles.clear();
        self.prestart_source_object_ids.clear();
        self.anchor_player_keys.clear();
        self.local_projectile_dedupe.clear();
        self.local_projectile_targets.clear();
        self.five_class_projectiles.clear();
        self.elemental_state.clear();
        self.elemental_orb_thresholds.clear();
        self.elemental_loop_owner = None;
        self.concentration_paused_at.clear();
        self.hidden_five_class_effect_keys.clear();
        self.delayed_explosion_stages.clear();
        self.double_slash_body_effects.clear();
        self.elemental_barrier_objects.clear();
        self.elemental_barrier_down_objects.clear();
        self.ready_sounds.clear();
        self.pending_sounds.clear();
        self.dead_player_sound_keys.clear();
        self.dead_scarecrow_sound_keys.clear();
        self.revived_player_effect_keys.clear();
        self.magic_shield_down_object_ids.clear();
    }

    fn replace_dead_action_object_ids(&mut self, object_ids: HashSet<u32>) {
        self.dead_action_object_ids = object_ids;
    }

    fn reconcile_snake_totem_auras(&mut self, ready_object_ids: &HashSet<u32>) {
        let ready = ready_object_ids
            .iter()
            .copied()
            .filter(|id| {
                !self.scene_audio_terminated
                    && self.zone_tiles.contains_key(id)
                    && self.five_class_actor_contexts.get(id).is_some_and(|actor| {
                        actor.get("kind").and_then(Value::as_str) == Some("monster")
                            && actor.get("image").and_then(Value::as_u64) == Some(361)
                            && actor.get("dead").and_then(Value::as_bool) != Some(true)
                    })
            })
            .collect::<HashSet<_>>();
        let remove = self
            .active
            .iter()
            .filter_map(|instance| {
                let id = instance
                    .key
                    .strip_prefix("snake-totem-aura-")?
                    .parse::<u32>()
                    .ok()?;
                (!ready.contains(&id)).then_some(instance.key.clone())
            })
            .collect::<Vec<_>>();
        for key in remove {
            self.clear_effect_key(&key);
        }
        for id in ready {
            let key = format!("snake-totem-aura-{id}");
            if self.active.iter().any(|instance| instance.key == key) {
                continue;
            }
            if let Some(animation) = effect_catalog().as_ref()
                .and_then(|catalog| catalog.map_animation("ArcherSnakeTotemAura", 0))
            {
                self.push_actor_animation(
                    key,
                    id,
                    animation,
                    self.now_ms,
                    &EffectProvenance {
                        generation: self.last_generation,
                        sequence: 0,
                        packet: "StandingCycle".to_owned(),
                        spell: "SummonSnakes".to_owned(),
                    },
                );
            }
        }
    }

    /// Crystal checks the target's current action in the missile completion
    /// callback. Keep the projectile and its tracking intact during flight so
    /// a target revived before arrival can still receive the visual impact;
    /// suppress only at the exact completion boundary, and cancel its paired
    /// impact sound in the same operation.
    fn suppress_dead_target_impacts_at_completion(&mut self, now_ms: u64) {
        let mut suppressed = HashSet::new();
        for instance in &mut self.active {
            if !matches!(
                instance.provenance.spell.as_str(),
                "FireBall"
                    | "GreatFireBall"
                    | "FireBounce"
                    | "SoulFireBall"
                    | "FrostCrunch"
                    | "Hallucination"
            ) {
                continue;
            }
            let Some(target) = self.local_projectile_targets.get(&instance.key) else {
                continue;
            };
            let Some(target_id) = target.target_id else {
                continue;
            };
            let Some(projectile) = instance.current.as_ref() else {
                continue;
            };
            if projectile.kind != "projectile"
                || now_ms < instance.start_at.saturating_add(projectile.duration_ms)
                || !self.dead_action_object_ids.contains(&target_id)
            {
                continue;
            }
            if instance.queued.take().is_some() {
                suppressed.insert(instance.key.clone());
            }
        }
        if !suppressed.is_empty() {
            self.pending_sounds
                .retain(|pending| !suppressed.contains(&pending.key));
        }
    }

    fn refresh_anchor_tiles(&mut self) {
        let now_ms = self.now_ms;
        self.refresh_five_class_projectiles(now_ms);
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
                "FireBounce" => (
                    "FireBounce",
                    Some((FIRE_BOUNCE_IMPACT_SOUND_CUE, FIRE_BOUNCE_IMPACT_SOUND_FILE)),
                ),
                "FrostCrunch" => (
                    "FrostCrunch",
                    Some((
                        FROST_CRUNCH_IMPACT_SOUND_CUE,
                        FROST_CRUNCH_IMPACT_SOUND_FILE,
                    )),
                ),
                "SoulFireBall" => (
                    "SoulFireBall",
                    Some((
                        SOUL_FIREBALL_IMPACT_SOUND_CUE,
                        SOUL_FIREBALL_IMPACT_SOUND_FILE,
                    )),
                ),
                "Hallucination" => (
                    "Hallucination",
                    Some((
                        HALLUCINATION_IMPACT_SOUND_CUE,
                        HALLUCINATION_IMPACT_SOUND_FILE,
                    )),
                ),
                "LeftGuardRangeProjectile" => ("LeftGuardRangeProjectile", None),
                "ArcherNormalArrow" => (
                    "ArcherNormalArrow",
                    Some((
                        ARCHER_NORMAL_ARROW_IMPACT_SOUND_CUE,
                        ARCHER_NORMAL_ARROW_IMPACT_SOUND_FILE,
                    )),
                ),
                spell if ground_talisman(spell).is_some() => {
                    let (spell, _) = ground_talisman(spell).expect("checked talisman");
                    (spell, None)
                }
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
            let source = if ground_talisman(spell).is_some() || spell == "ArcherNormalArrow" {
                self.source_object_ids
                    .get(&instance.key)
                    .and_then(|id| resolve_tile(&instance.key, *id))
                    .unwrap_or(source)
            } else {
                source
            };
            if ground_talisman(spell).is_some() || spell == "ArcherNormalArrow" {
                instance.from_x = Some(source.0 as f32);
                instance.from_y = Some(source.1 as f32);
            }
            let direction = projectile_direction16(source, destination);
            let launch_animation = effect_catalog().as_ref().and_then(|catalog| {
                if spell == "LeftGuardRangeProjectile" {
                    catalog.left_guard_range_projectile_animation(direction)
                } else if spell == "ArcherNormalArrow" {
                    catalog.archer_normal_arrow_animation(direction)
                } else {
                    catalog.spell_projectile_animation(spell, direction)
                }
            });
            let Some(mut launch_animation) = launch_animation else {
                target_state.resolved_at_launch = true;
                failed_launches.push(instance.key.clone());
                continue;
            };
            let (duration_ms, frame_interval_ms) = crystal_projectile_clock(
                max_tile_distance(source, destination),
                launch_animation.interval,
            );
            launch_animation.duration_ms = duration_ms;
            launch_animation.interval = frame_interval_ms;
            *projectile = launch_animation;
            instance.tile_x = destination.0;
            instance.tile_y = destination.1;
            instance.queued = if matches!(spell, "LeftGuardRangeProjectile" | "ArcherNormalArrow") {
                None
            } else {
                bound_target
                    .or_else(|| ground_talisman(spell).map(|_| destination))
                    .and_then(|_| {
                        effect_catalog()
                            .as_ref()
                            .and_then(|catalog| catalog.spell_impact_animation(spell))
                    })
            };
            target_state.resolved_at_launch = true;
            target_state.unbound_archer_arrow =
                spell == "ArcherNormalArrow" && bound_target.is_none();
            if let Some(target_id) = target_state.target_id.filter(|_| bound_target.is_some()) {
                anchors_to_insert.push((instance.key.clone(), target_id));
            }
            if let Some((impact_sound_cue, impact_sound_file)) = impact_sound.filter(|_| {
                instance.queued.is_some()
                    || ground_talisman(spell).is_some()
                    || (spell == "ArcherNormalArrow" && bound_target.is_some())
            }) {
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

        for instance in &mut self.active {
            let Some(target) = self.local_projectile_targets.get_mut(&instance.key) else {
                continue;
            };
            if !target.resolved_at_launch
                || !target.unbound_archer_arrow
                || target.unbound_archer_arrow_process_at == Some(now_ms)
            {
                continue;
            }
            target.unbound_archer_arrow_process_at = Some(now_ms);
            let (Some(from_x), Some(from_y), Some(projectile)) =
                (instance.from_x, instance.from_y, instance.current.as_mut())
            else {
                continue;
            };
            let source = (from_x as i32, from_y as i32);
            let owner_tile = self
                .source_object_ids
                .get(&instance.key)
                .and_then(|id| resolve_tile(&instance.key, *id))
                .unwrap_or(source);
            // Missile.Process extends a target-free arrow once per processing
            // pass while its destination is inside Globals.DataRange (16).
            if max_tile_distance(owner_tile, target.fallback) < 16 {
                target.fallback = (
                    target
                        .fallback
                        .0
                        .saturating_add(target.fallback.0.saturating_sub(source.0)),
                    target
                        .fallback
                        .1
                        .saturating_add(target.fallback.1.saturating_sub(source.1)),
                );
            }
            instance.tile_x = target.fallback.0;
            instance.tile_y = target.fallback.1;
            let (duration, interval) = crystal_projectile_clock(
                max_tile_distance(source, target.fallback),
                FIREBALL_PROJECTILE_STEP_MS,
            );
            projectile.duration_ms = duration;
            projectile.interval = interval;
        }

        let anchors = &self.anchor_object_ids;
        let mut missing = missing_prestart_sources;
        missing.extend(missing_owned_sources);
        missing.extend(failed_launches);
        let mut detached_left_guard_targets = Vec::new();
        let mut local_projectile_impact_due = Vec::new();
        for instance in &mut self.active {
            if ground_talisman(&instance.provenance.spell).is_some()
                && instance.current.as_ref().is_some_and(|animation| {
                    now_ms >= instance.start_at.saturating_add(animation.duration_ms)
                })
            {
                if let Some(target) = self.local_projectile_targets.get(&instance.key) {
                    instance.tile_x = target.fallback.0;
                    instance.tile_y = target.fallback.1;
                }
                continue;
            }
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
                        "FireBounce" => (true, Some(FIRE_BOUNCE_IMPACT_SOUND_CUE)),
                        "FrostCrunch" => (true, Some(FROST_CRUNCH_IMPACT_SOUND_CUE)),
                        "SoulFireBall" => (true, Some(SOUL_FIREBALL_IMPACT_SOUND_CUE)),
                        "Hallucination" => (true, Some(HALLUCINATION_IMPACT_SOUND_CUE)),
                        "LeftGuardRangeProjectile" => (true, None),
                        "ArcherNormalArrow" => (true, Some(ARCHER_NORMAL_ARROW_IMPACT_SOUND_CUE)),
                        spell if ground_talisman(spell).is_some() => (true, None),
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
                            let (duration_ms, frame_interval_ms) = crystal_projectile_clock(
                                max_tile_distance(source, destination),
                                projectile_process_interval_ms(&instance.provenance.spell),
                            );
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
            } else if (instance.provenance.spell == "LeftGuardRangeProjectile"
                || instance.provenance.spell == "ArcherNormalArrow"
                || ground_talisman(&instance.provenance.spell).is_some())
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
        // Talismans complete into independent point effects. Normal arrows
        // have no completion bitmap. Deliver their due arrival sounds before
        // the renderer prunes those completed source-owned instances.
        let completed_projectiles = self
            .active
            .iter()
            .filter(|instance| {
                !missing.contains(&instance.key)
                    && (ground_talisman(&instance.provenance.spell).is_some()
                        || instance.provenance.spell == "ArcherNormalArrow")
                    && instance.current.as_ref().is_some_and(|animation| {
                        now_ms >= instance.start_at.saturating_add(animation.duration_ms)
                    })
            })
            .map(|instance| instance.key.clone())
            .collect::<HashSet<_>>();
        for key in &completed_projectiles {
            self.anchor_object_ids.remove(key);
            self.source_object_ids.remove(key);
        }
        let mut completed_sounds = Vec::new();
        self.pending_sounds.retain(|sound| {
            if completed_projectiles.contains(&sound.key) && now_ms >= sound.due_at_ms {
                completed_sounds.push(sound.event.clone());
                false
            } else {
                true
            }
        });
        self.ready_sounds.extend(completed_sounds);
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
        self.refresh_five_class_projectiles(now_ms);
        self.suppress_dead_target_impacts_at_completion(now_ms);
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
        suppress_duplicate_scarecrow_struck: bool,
    ) {
        for key in ["_nativeAttacker", "_nativeTarget"] {
            if let Some(actor) = payload.get(key).filter(|actor| {
                actor_is_player(actor)
                    || actor.get("kind").and_then(Value::as_str) == Some("monster")
            }) {
                if let Some(id) = effect_object_id(actor) {
                    self.five_class_actor_contexts.insert(id, actor.clone());
                }
            }
        }
        self.cancel_pet_prestart(packet, payload);
        match packet {
            "MapChanged" | "LogOutSuccess" => self.clear_active_effects(),
            "Struck" | "ObjectStruck" => {
                if !suppress_duplicate_scarecrow_struck {
                    self.apply_scarecrow_struck_sound(payload, provenance);
                }
                self.apply_player_struck_sound(payload, provenance);
                self.apply_elemental_barrier_struck(payload, provenance);
            }
            "Death" | "ObjectDied" => {
                if packet == "ObjectDied" && payload.get("kind").and_then(Value::as_u64) == Some(2)
                {
                    self.apply_dark_body_disappear(payload, provenance);
                    self.apply_object_remove(payload, false);
                    return;
                }
                self.apply_player_death_sound(payload, provenance);
                self.apply_scarecrow_death_sound(payload, provenance);
                self.apply_five_class_monster_death(payload, provenance);
                if let Some(id) = effect_object_id(payload)
                    .or_else(|| payload.get("_nativeTarget").and_then(effect_object_id))
                {
                    self.clear_effect_key(&format!("delayed-explosion-{id}"));
                    self.delayed_explosion_stages.remove(&id);
                }
            }
            "IntelligentCreaturePickup" => self.apply_pet_pickup_sound(payload, provenance),
            "ObjectAttack" => self.apply_object_attack(payload, provenance),
            "ObjectRangeAttack" => self.apply_object_range_attack(payload, zone_tiles, provenance),
            "ObjectMagic" => self.apply_object_magic(payload, provenance),
            "ObjectProjectile" => self.apply_object_projectile(payload, zone_tiles, provenance),
            "ObjectEffect" => self.apply_object_effect(payload, zone_tiles, provenance),
            "MapEffect" => self.apply_map_effect(payload, provenance),
            "ObjectSpell" => self.apply_object_spell(payload, zone_tiles, provenance),
            "SetConcentration" => self.apply_concentration(payload, provenance),
            "SetElemental" => self.apply_elemental(payload, provenance),
            "SetBindingShot" => self.apply_binding_shot(payload, provenance),
            "RemoveDelayedExplosion" => {
                if let Some(id) = effect_object_id(payload) {
                    self.clear_effect_key(&format!("delayed-explosion-{id}"));
                    self.delayed_explosion_stages.remove(&id);
                }
            }
            "UserBackStep" | "ObjectBackStep" => self.apply_back_step_sound(payload, provenance),
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
        self.apply_pet_attack_effects(payload, provenance);
        self.apply_scarecrow_attack_sound(payload, provenance);
        if self.apply_double_slash_overlay(payload, provenance) {
            return;
        }
        let spell = payload.get("spell").and_then(|value| {
            value.as_str().or_else(|| {
                value.as_u64().and_then(|id| match id {
                    2 => Some("Slaying"),
                    3 => Some("Thrusting"),
                    4 => Some("HalfMoon"),
                    6 => Some("TwinDrakeBlade"),
                    8 => Some("FlamingSword"),
                    10 => Some("CrossHalfMoon"),
                    _ => None,
                })
            })
        });
        let Some(spell) = spell.filter(|spell| {
            matches!(
                *spell,
                "Slaying"
                    | "Thrusting"
                    | "HalfMoon"
                    | "TwinDrakeBlade"
                    | "FlamingSword"
                    | "CrossHalfMoon"
            )
        }) else {
            return;
        };
        // Only the authoritative attack packet chooses the overlay/level. A
        // learned/toggled skill or a monster attack Type cannot invent one.
        let level = match spell {
            "Slaying" | "Thrusting" | "HalfMoon" => {
                match payload.get("level").and_then(Value::as_u64) {
                    Some(level @ 0..=3) => level as u32,
                    _ => return,
                }
            }
            _ => 0,
        };
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
        let Some(animation) = catalog.warrior_attack_animation(spell, direction, level) else {
            return;
        };

        // One overlay per attacker: a newer authoritative attack restarts its
        // six-frame clock while distinct attackers remain independent.
        let key = if spell == "FlamingSword" {
            format!("flaming-sword-{object_id}")
        } else {
            format!("warrior-attack-{object_id}")
        };
        let previous_keys = [
            format!("flaming-sword-{object_id}"),
            format!("warrior-attack-{object_id}"),
        ];
        self.active
            .retain(|instance| !previous_keys.contains(&instance.key));
        for previous in &previous_keys {
            self.anchor_object_ids.remove(previous);
        }
        self.pending_sounds.retain(|pending| pending.key != key);
        self.anchor_object_ids.insert(key.clone(), object_id);
        if spell == "FlamingSword" {
            self.queue_immediate_sound(
                provenance,
                FLAMING_SWORD_SOUND_CUE,
                FLAMING_SWORD_SOUND_FILE,
            );
        }
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

    fn apply_scarecrow_attack_sound(&mut self, payload: &Value, provenance: &EffectProvenance) {
        let Some(attacker) = payload
            .get("_nativeAttacker")
            .filter(|actor| actor_is_scarecrow(actor))
        else {
            return;
        };
        let Some(actor_key) = actor_sound_key(attacker) else {
            return;
        };
        if self.dead_scarecrow_sound_keys.contains(&actor_key) {
            return;
        }
        self.queue_immediate_sound(
            provenance,
            &format!("Scarecrow.{actor_key}.Attack"),
            SCARECROW_ATTACK_SOUND_FILE,
        );
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

    fn apply_scarecrow_struck_sound(&mut self, payload: &Value, provenance: &EffectProvenance) {
        let Some(target) = payload
            .get("_nativeTarget")
            .filter(|actor| actor_is_scarecrow(actor))
        else {
            return;
        };
        let Some(actor_key) = actor_sound_key(target) else {
            return;
        };
        if self.dead_scarecrow_sound_keys.contains(&actor_key) {
            return;
        }

        // MonsterObject enters Struck by calling PlayFlinchSound first, then
        // PlayStruckSound. Keep that order even when the attacker is unknown:
        // BaseSound + 2 remains valid, while an unknown StruckWeapon is silent.
        self.queue_immediate_sound(
            provenance,
            &format!("Scarecrow.{actor_key}.Flinch"),
            SCARECROW_FLINCH_SOUND_FILE,
        );
        if let Some(struck_file) = monster_struck_weapon_file(payload.get("_nativeAttacker")) {
            self.queue_immediate_sound(
                provenance,
                &format!("Scarecrow.{actor_key}.StruckWeapon"),
                struck_file,
            );
        }
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
        self.cancel_scarecrow_attack_sound(&actor_key);
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

    fn cancel_scarecrow_attack_sound(&mut self, actor_key: &str) {
        let attack_cue = format!("Scarecrow.{actor_key}.Attack");
        self.ready_sounds.retain(|event| event.cue != attack_cue);
    }

    fn schedule_local_projectile_from_object_magic(
        &mut self,
        payload: &Value,
        catalog: &EffectCatalog,
        provenance: &EffectProvenance,
        spell: &'static str,
        tag: &'static str,
        spell_action_ms: u64,
        projectile_sound: Option<(&'static str, &'static str)>,
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
            crystal_projectile_clock(max_tile_distance(source, destination), projectile.interval);
        projectile.duration_ms = duration_ms;
        projectile.interval = frame_interval_ms;

        let now = self.now_ms;
        let start_at = now.saturating_add(spell_action_ms);
        let key = self.next_key(tag);
        if ground_talisman(spell).is_some() {
            if let Some(source_id) = source_id {
                self.source_object_ids.insert(key.clone(), source_id);
            }
        }
        self.local_projectile_targets.insert(
            key.clone(),
            LocalProjectileTarget {
                target_id,
                fallback: destination,
                resolved_at_launch: false,
                unbound_archer_arrow: false,
                unbound_archer_arrow_process_at: None,
            },
        );
        if let Some((projectile_sound_cue, projectile_sound_file)) = projectile_sound {
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
        }
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
        if self.apply_five_class_ranged_magic(payload, catalog, provenance, spell) {
            return;
        }
        if spell == "ElementalBarrier" {
            let Some(id) = effect_object_id(payload) else {
                return;
            };
            if !self
                .elemental_state
                .get(&id)
                .is_some_and(|state| state.enabled)
                || self.elemental_barrier_objects.contains(&id)
            {
                return;
            }
        }
        if spell == "ExplosiveTrap" {
            self.queue_immediate_sound(provenance, "ExplosiveTrap.lay", "M124-0.wav");
            return;
        }
        if let Some((spell, launch_sound)) = ground_talisman(spell) {
            self.schedule_local_projectile_from_object_magic(
                payload,
                catalog,
                provenance,
                spell,
                "ground-talisman",
                600,
                launch_sound,
            );
            return;
        }
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
                Some((
                    SOUL_FIREBALL_PROJECTILE_SOUND_CUE,
                    SOUL_FIREBALL_PROJECTILE_SOUND_FILE,
                )),
            );
            return;
        }
        if spell == "Hallucination" {
            // Crystal has no Hallucination cast bitmap or cast sound. Cast=true
            // creates the source-owned missile only after the six-frame Spell
            // actor action completes; its sole sound belongs to target impact.
            self.schedule_local_projectile_from_object_magic(
                payload,
                catalog,
                provenance,
                "Hallucination",
                "hallucination",
                HALLUCINATION_SPELL_ACTION_MS,
                None,
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
        if let Some(id) = effect_object_id(payload) {
            let five_class_cast = match spell {
                "Haste" => Some("M93-0.wav"),
                "LightBody" => Some("M95-0.wav"),
                "HeavenlySword" => Some("M96-0.wav"),
                "FireBurst" => Some("M97-0.wav"),
                "Trap" => Some("M98-0.wav"),
                "PoisonSword" => Some("M99-0.wav"),
                "MoonLight" => Some("M100-0.wav"),
                "SwiftFeet" => Some("M102-0.wav"),
                "DarkBody" => Some("M103-0.wav"),
                "ElementalBarrier" => Some("M131-0.wav"),
                "CrescentSlash" => self.five_class_actor_contexts.get(&id).map(|actor| {
                    if actor_is_female(actor) {
                        "M105-1.wav"
                    } else {
                        "M105-0.wav"
                    }
                }),
                _ => None,
            };
            if let Some(file) = five_class_cast {
                self.anchor_object_ids.insert(key.clone(), id);
                self.queue_five_class_sound(&key, now, &format!("{spell}.cast"), file, provenance);
                if payload.get("cast").and_then(Value::as_bool) == Some(true) {
                    let completion_file = match spell {
                        "CrescentSlash" => Some("M105-2.wav"),
                        "Trap"
                            if payload
                                .get("targetId")
                                .and_then(Value::as_u64)
                                .and_then(|id| u32::try_from(id).ok())
                                .is_some_and(|id| self.zone_tiles.contains_key(&id)) =>
                        {
                            Some("M98-1.wav")
                        }
                        _ => None,
                    };
                    if let Some(file) = completion_file {
                        self.queue_five_class_sound(
                            &key,
                            now.saturating_add(600),
                            &format!("{spell}.complete"),
                            file,
                            provenance,
                        );
                    }
                }
            }
        }
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
        if spell == "FireBounce" {
            self.pending_sounds.push(PendingEffectSound {
                key: key.clone(),
                due_at_ms: now,
                requires_active_effect: true,
                event: mir2_client_bevy::audio::NativeGameplaySoundEvent {
                    generation: provenance.generation,
                    sequence: provenance.sequence,
                    cue: FIRE_BOUNCE_CAST_SOUND_CUE.to_owned(),
                    file_name: FIRE_BOUNCE_CAST_SOUND_FILE.to_owned(),
                },
            });
        }
        if spell == "FrostCrunch" {
            self.pending_sounds.push(PendingEffectSound {
                key: key.clone(),
                due_at_ms: now,
                requires_active_effect: true,
                event: mir2_client_bevy::audio::NativeGameplaySoundEvent {
                    generation: provenance.generation,
                    sequence: provenance.sequence,
                    cue: FROST_CRUNCH_CAST_SOUND_CUE.to_owned(),
                    file_name: FROST_CRUNCH_CAST_SOUND_FILE.to_owned(),
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
        if spell == "ImmortalSkin" {
            // Crystal creates these two attached layers concurrently. The
            // secondary's 600 ms lifetime is not an impact after the 2397 ms
            // primary (17 * floor(2400 / 17)) finishes.
            if let Some(object_id) = payload
                .get("objectId")
                .and_then(Value::as_u64)
                .and_then(|id| u32::try_from(id).ok())
            {
                self.anchor_object_ids.insert(key.clone(), object_id);
                if let Some(secondary) = catalog.map_animation("ImmortalSkinSecondary", 0) {
                    let secondary_key = self.next_key("immortal-skin-secondary");
                    self.anchor_object_ids
                        .insert(secondary_key.clone(), object_id);
                    self.active.push(EffectInstance {
                        key: secondary_key,
                        kind: EffectKindTag::Cast,
                        tile_x: x as i32,
                        tile_y: y as i32,
                        from_x: None,
                        from_y: None,
                        current: Some(secondary),
                        queued: None,
                        return_queued: None,
                        started_at: now,
                        start_at: now,
                        persistent_object_id: None,
                        provenance: provenance.clone(),
                    });
                }
            }
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
        if spell == "FireBang" && payload.get("cast").and_then(Value::as_bool) == Some(true) {
            // Crystal PlayerObject: after the six-frame Spell action, create
            // Magic 1660..1669 at TargetPoint even when TargetID is zero.
            // This independent ground explosion has no projectile/actor anchor.
            if let (Some(target_x), Some(target_y), Some(source_id), Some(impact)) = (
                value_f32(payload, "target", "x"),
                value_f32(payload, "target", "y"),
                payload
                    .get("objectId")
                    .and_then(Value::as_u64)
                    .and_then(|id| u32::try_from(id).ok()),
                catalog.spell_impact_animation("FireBang"),
            ) {
                let impact_key = self.next_key("firebang-impact");
                self.prestart_source_object_ids
                    .insert(impact_key.clone(), source_id);
                self.active.push(EffectInstance {
                    key: impact_key,
                    kind: EffectKindTag::Impact,
                    tile_x: target_x as i32,
                    tile_y: target_y as i32,
                    from_x: None,
                    from_y: None,
                    current: Some(impact),
                    queued: None,
                    return_queued: None,
                    started_at: now,
                    start_at: now.saturating_add(600),
                    persistent_object_id: None,
                    provenance: provenance.clone(),
                });
            }
        }
        if spell == "FireBall" {
            self.schedule_local_projectile_from_object_magic(
                payload,
                catalog,
                provenance,
                "FireBall",
                "fireball",
                FIREBALL_SPELL_ACTION_MS,
                Some((
                    FIREBALL_PROJECTILE_SOUND_CUE,
                    FIREBALL_PROJECTILE_SOUND_FILE,
                )),
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
                Some((
                    GREAT_FIREBALL_PROJECTILE_SOUND_CUE,
                    GREAT_FIREBALL_PROJECTILE_SOUND_FILE,
                )),
            );
        }
        if spell == "FireBounce" {
            self.schedule_local_projectile_from_object_magic(
                payload,
                catalog,
                provenance,
                "FireBounce",
                "fire-bounce",
                FIRE_BOUNCE_SPELL_ACTION_MS,
                Some((
                    FIRE_BOUNCE_PROJECTILE_SOUND_CUE,
                    FIRE_BOUNCE_PROJECTILE_SOUND_FILE,
                )),
            );
        }
        if spell == "FrostCrunch" {
            self.schedule_local_projectile_from_object_magic(
                payload,
                catalog,
                provenance,
                "FrostCrunch",
                "frost-crunch",
                FROST_CRUNCH_SPELL_ACTION_MS,
                None,
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
        // These three Crystal player paths never consume ObjectProjectile;
        // current Rust servers emit it only as a compatibility supplement.
        // Ignore those packets regardless of order/replay and let ObjectMagic
        // own their delayed local missile. FireBounce differs: dedupe only its
        // legacy initial player-to-target supplement below, then consume later
        // monster-to-monster packets as authoritative bounce hops.
        if matches!(spell, "SoulFireBall" | "GreatFireBall" | "Hallucination")
            || five_class_ranged_spell(spell)
            || ground_talisman(spell).is_some()
        {
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
        if matches!(
            spell,
            "FireBall" | "SoulFireBall" | "FrostCrunch" | "FireBounce"
        ) && self
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
        if spell == "FireBounce" {
            self.apply_fire_bounce_projectile(
                catalog,
                source_id,
                destination_id,
                (from_x, from_y),
                (to_x, to_y),
                provenance,
            );
            return;
        }
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

    fn apply_fire_bounce_projectile(
        &mut self,
        catalog: &EffectCatalog,
        source_id: u32,
        destination_id: u32,
        source: (i32, i32),
        destination: (i32, i32),
        provenance: &EffectProvenance,
    ) {
        let direction = projectile_direction16(source, destination);
        let Some(mut projectile) = catalog.spell_projectile_animation("FireBounce", direction)
        else {
            return;
        };
        let Some(impact) = catalog.spell_impact_animation("FireBounce") else {
            return;
        };
        let (duration_ms, frame_interval_ms) =
            crystal_projectile_clock(max_tile_distance(source, destination), projectile.interval);
        projectile.duration_ms = duration_ms;
        projectile.interval = frame_interval_ms;

        let now = self.now_ms;
        let key = self.next_key("fire-bounce-hop");
        self.pending_sounds.push(PendingEffectSound {
            key: key.clone(),
            due_at_ms: now,
            requires_active_effect: true,
            event: mir2_client_bevy::audio::NativeGameplaySoundEvent {
                generation: provenance.generation,
                sequence: provenance.sequence,
                cue: FIRE_BOUNCE_PROJECTILE_SOUND_CUE.to_owned(),
                file_name: FIRE_BOUNCE_PROJECTILE_SOUND_FILE.to_owned(),
            },
        });
        self.pending_sounds.push(PendingEffectSound {
            key: key.clone(),
            due_at_ms: now.saturating_add(duration_ms),
            requires_active_effect: true,
            event: mir2_client_bevy::audio::NativeGameplaySoundEvent {
                generation: provenance.generation,
                sequence: provenance.sequence,
                cue: FIRE_BOUNCE_IMPACT_SOUND_CUE.to_owned(),
                file_name: FIRE_BOUNCE_IMPACT_SOUND_FILE.to_owned(),
            },
        });
        self.active.push(EffectInstance {
            key: key.clone(),
            kind: EffectKindTag::Projectile,
            tile_x: destination.0,
            tile_y: destination.1,
            from_x: Some(source.0 as f32),
            from_y: Some(source.1 as f32),
            current: Some(projectile),
            queued: Some(impact),
            return_queued: None,
            started_at: now,
            start_at: now,
            persistent_object_id: None,
            provenance: provenance.clone(),
        });
        self.anchor_object_ids.insert(key.clone(), destination_id);
        self.source_object_ids.insert(key.clone(), source_id);
        self.local_projectile_targets.insert(
            key,
            LocalProjectileTarget {
                target_id: Some(destination_id),
                fallback: destination,
                resolved_at_launch: true,
                unbound_archer_arrow: false,
                unbound_archer_arrow_process_at: None,
            },
        );
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
        if effect == MAGIC_SHIELD_DOWN_EFFECT {
            self.magic_shield_down_object_ids.insert(object_id);
            self.clear_magic_shield_effect(object_id);
            return;
        }
        if self.apply_five_class_object_effect(effect, object_id, payload, catalog, provenance) {
            return;
        }
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
        let key = if effect == MAGIC_SHIELD_UP_EFFECT {
            self.magic_shield_down_object_ids.remove(&object_id);
            self.clear_magic_shield_effect(object_id);
            format!("magic-shield-{object_id}")
        } else {
            self.next_key("obj")
        };
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

    fn clear_magic_shield_effect(&mut self, object_id: u32) {
        let key = format!("magic-shield-{object_id}");
        self.active.retain(|instance| instance.key != key);
        self.anchor_object_ids.remove(&key);
        self.source_object_ids.remove(&key);
        self.prestart_source_object_ids.remove(&key);
        self.pending_sounds.retain(|pending| pending.key != key);
        self.local_projectile_targets.remove(&key);
    }

    fn reconcile_magic_shield_render_state(&mut self, zone_tiles: &HashMap<u32, (i32, i32)>) {
        let missing = self
            .render_magic_shield_object_ids
            .iter()
            .filter(|object_id| !self.magic_shield_down_object_ids.contains(object_id))
            .filter(|object_id| {
                let key = format!("magic-shield-{object_id}");
                !self.active.iter().any(|instance| instance.key == key)
            })
            .copied()
            .collect::<Vec<_>>();
        for object_id in missing {
            self.apply_object_effect(
                &json!({
                    "objectId": object_id,
                    "effect": MAGIC_SHIELD_UP_EFFECT,
                    "delayTime": 0
                }),
                zone_tiles,
                &EffectProvenance {
                    generation: self.last_generation,
                    sequence: 0,
                    packet: "ObjectPlayer".to_owned(),
                    spell: "effect".to_owned(),
                },
            );
        }
    }

    fn reconcile_elemental_barrier_render_state(&mut self, zone_tiles: &HashMap<u32, (i32, i32)>) {
        if self.scene_audio_terminated {
            return;
        }
        let missing = self
            .five_class_actor_contexts
            .iter()
            .filter(|(id, actor)| {
                actor_is_player(actor)
                    && actor.get("effect").and_then(Value::as_u64) == Some(13)
                    && actor.get("dead").and_then(Value::as_bool) != Some(true)
                    && !self.elemental_barrier_down_objects.contains(id)
                    && !self
                        .active
                        .iter()
                        .any(|instance| instance.key == format!("archer-barrier-{id}"))
            })
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        for object_id in missing {
            if !zone_tiles.contains_key(&object_id) {
                continue;
            }
            let Some(animation) = effect_catalog().as_ref()
                .and_then(|catalog| catalog.map_animation("ArcherBarrierPersistent16", 0))
            else {
                continue;
            };
            self.elemental_barrier_objects.insert(object_id);
            self.push_actor_animation(
                format!("archer-barrier-{object_id}"),
                object_id,
                animation,
                self.now_ms,
                &EffectProvenance {
                    generation: self.last_generation,
                    sequence: self.last_effect_sequence,
                    packet: "ObjectPlayer".to_owned(),
                    spell: "effect".to_owned(),
                },
            );
        }
    }

    fn reconcile_elemental_orbs_render_state(&mut self) {
        if self.scene_audio_terminated {
            return;
        }
        let loaded = self
            .five_class_actor_contexts
            .iter()
            .filter_map(|(&id, actor)| {
                if !actor_is_player(actor)
                    || self.elemental_state.contains_key(&id)
                    || !self.zone_tiles.contains_key(&id)
                    || actor.get("dead").and_then(Value::as_bool) == Some(true)
                {
                    return None;
                }
                let effect = actor.get("elementOrbEffect")?.as_u64()?;
                let level = u32::try_from(actor.get("elementOrbLevel")?.as_u64()?).ok()?;
                let max = u32::try_from(actor.get("elementOrbMax")?.as_u64()?).ok()?;
                (effect > 0 && effect <= 4).then_some((
                    id,
                    effect as u32,
                    level,
                    max,
                    actor.get("kind").and_then(Value::as_str) == Some("selfPlayer")
                        || actor.get("_nativeSelf").and_then(Value::as_bool) == Some(true),
                ))
            })
            .collect::<Vec<_>>();
        for (id, effect, level, max, owner) in loaded {
            self.elemental_state.insert(
                id,
                NativeElementState {
                    enabled: true,
                    casted: false,
                    owner,
                },
            );
            let types = if effect == 4 {
                vec![4]
            } else {
                (1..=effect).collect()
            };
            let mut last_sound = None;
            let provenance = EffectProvenance {
                generation: self.last_generation,
                sequence: self.last_effect_sequence,
                packet: "ObjectPlayer".to_owned(),
                spell: "Meditation".to_owned(),
            };
            for ty in types {
                // Source first-load orbs do not loop their sound. Each new
                // color stops the preceding channel, leaving the last color.
                if ty < 4 && level >= max {
                    continue;
                }
                let (name, file) = match ty {
                    1 => ("ArcherElementGreen", "M126-5.wav"),
                    2 => ("ArcherElementBlue", "M126-6.wav"),
                    3 => ("ArcherElementRed", "M126-7.wav"),
                    _ => ("ArcherElementMixed", "M126-8.wav"),
                };
                if let Some(animation) =
                    effect_catalog().as_ref().and_then(|catalog| catalog.map_animation(name, 0))
                {
                    let key = format!("archer-element-{id}-{ty}");
                    if self.push_actor_animation(
                        key.clone(),
                        id,
                        animation,
                        self.now_ms,
                        &provenance,
                    ) {
                        self.elemental_orb_thresholds.insert((id, ty), max);
                        last_sound = Some((key, ty, file));
                    }
                }
            }
            if let Some((key, ty, file)) = last_sound {
                self.queue_element_loop(None, &provenance);
                self.queue_five_class_sound(
                    &key,
                    self.now_ms,
                    &format!("Meditation.element{ty}"),
                    file,
                    &provenance,
                );
            }
        }
    }

    fn apply_object_range_attack(
        &mut self,
        payload: &Value,
        zone_tiles: &HashMap<u32, (i32, i32)>,
        provenance: &EffectProvenance,
    ) {
        if self.apply_trap_rock_range(payload, provenance) {
            return;
        }
        let Some(attacker) = payload.get("_nativeAttacker") else {
            return;
        };
        if actor_is_archer(attacker) {
            self.apply_archer_normal_range_attack(payload, zone_tiles, provenance);
            return;
        }
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

    fn apply_archer_normal_range_attack(
        &mut self,
        payload: &Value,
        zone_tiles: &HashMap<u32, (i32, i32)>,
        provenance: &EffectProvenance,
    ) {
        let Some(source_id) = payload
            .get("objectId")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .filter(|id| zone_tiles.contains_key(id))
        else {
            return;
        };
        if payload
            .get("_nativeAttacker")
            .and_then(|actor| actor.get("objectId"))
            .and_then(Value::as_u64)
            != Some(u64::from(source_id))
        {
            return;
        }
        let point = |field: &str| -> Option<(i32, i32)> {
            Some((
                i32::try_from(payload.get(field)?.get("x")?.as_i64()?).ok()?,
                i32::try_from(payload.get(field)?.get("y")?.as_i64()?).ok()?,
            ))
        };
        let (Some(source), Some(fallback)) = (point("location"), point("target")) else {
            return;
        };
        let target_id = payload
            .get("targetId")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .filter(|id| *id != 0);
        let destination = target_id
            .and_then(|id| zone_tiles.get(&id).copied())
            .unwrap_or(fallback);
        let direction = projectile_direction16(source, destination);
        let Some(mut arrow) = effect_catalog()
            .as_ref()
            .and_then(|catalog| catalog.archer_normal_arrow_animation(direction))
        else {
            return;
        };
        let (duration, interval) = crystal_projectile_clock(
            max_tile_distance(source, destination),
            FIREBALL_PROJECTILE_STEP_MS,
        );
        arrow.duration_ms = duration;
        arrow.interval = interval;
        let key = self.next_key("archer-normal-arrow");
        let start_at = self.now_ms.saturating_add(ARCHER_NORMAL_ARROW_DELAY_MS);
        let mut arrow_provenance = provenance.clone();
        arrow_provenance.spell = "ArcherNormalArrow".to_owned();
        self.active.push(EffectInstance {
            key: key.clone(),
            kind: EffectKindTag::Projectile,
            tile_x: destination.0,
            tile_y: destination.1,
            from_x: Some(source.0 as f32),
            from_y: Some(source.1 as f32),
            current: Some(arrow),
            queued: None,
            return_queued: None,
            started_at: self.now_ms,
            start_at,
            persistent_object_id: None,
            provenance: arrow_provenance,
        });
        self.source_object_ids.insert(key.clone(), source_id);
        self.local_projectile_targets.insert(
            key.clone(),
            LocalProjectileTarget {
                target_id,
                fallback,
                resolved_at_launch: false,
                unbound_archer_arrow: false,
                unbound_archer_arrow_process_at: None,
            },
        );
        self.pending_sounds.push(PendingEffectSound {
            key,
            due_at_ms: start_at,
            requires_active_effect: true,
            event: mir2_client_bevy::audio::NativeGameplaySoundEvent {
                generation: provenance.generation,
                sequence: provenance.sequence,
                cue: ARCHER_NORMAL_ARROW_SOUND_CUE.to_owned(),
                file_name: ARCHER_NORMAL_ARROW_SOUND_FILE.to_owned(),
            },
        });
        if payload.get("spell").map(spell_number_u32) == Some(121) {
            self.apply_archer_focus_proc(source_id, source, provenance);
        }
    }

    fn apply_archer_focus_proc(
        &mut self,
        source_id: u32,
        source: (i32, i32),
        provenance: &EffectProvenance,
    ) {
        let Some(animation) = effect_catalog()
            .as_ref()
            .and_then(EffectCatalog::archer_focus_proc_animation)
        else {
            return;
        };
        let key = self.next_key("archer-focus-proc");
        let start_at = self.now_ms.saturating_add(ARCHER_FOCUS_DELAY_MS);
        let mut focus_provenance = provenance.clone();
        focus_provenance.spell = "ArcherFocusProc".to_owned();
        self.active.push(EffectInstance {
            key: key.clone(),
            kind: EffectKindTag::AttackOverlay,
            tile_x: source.0,
            tile_y: source.1,
            from_x: None,
            from_y: None,
            current: Some(animation),
            queued: None,
            return_queued: None,
            started_at: self.now_ms,
            start_at,
            persistent_object_id: None,
            provenance: focus_provenance,
        });
        self.source_object_ids.insert(key.clone(), source_id);
        self.anchor_object_ids.insert(key.clone(), source_id);
        self.pending_sounds.push(PendingEffectSound {
            key,
            due_at_ms: start_at,
            requires_active_effect: true,
            event: mir2_client_bevy::audio::NativeGameplaySoundEvent {
                generation: provenance.generation,
                sequence: provenance.sequence,
                cue: ARCHER_FOCUS_SOUND_CUE.to_owned(),
                file_name: ARCHER_FOCUS_SOUND_FILE.to_owned(),
            },
        });
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
            crystal_projectile_clock(max_tile_distance(source, destination), animation.interval);
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
                unbound_archer_arrow: false,
                unbound_archer_arrow_process_at: None,
            },
        );
    }

    fn apply_map_effect(&mut self, payload: &Value, provenance: &EffectProvenance) {
        // Audio is owned by the accepted packet, not by successful PNG/catalog
        // loading. Crystal plays 10091 before creating the wall-hit effect.
        self.apply_mine_wall_sound(payload, provenance);
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

    fn apply_mine_wall_sound(&mut self, payload: &Value, provenance: &EffectProvenance) {
        if provenance.packet != "MapEffect"
            || provenance.sequence == 0
            || payload.get("effect").and_then(Value::as_u64) != Some(MINE_MAP_EFFECT)
        {
            return;
        }
        // The Crystal wire location is a Point of integer i32 map cells.
        // Reject incomplete, fractional and overflowing positions; observers
        // of another nearby miner use the same source-owned sound.
        let location = payload.get("location");
        if ["x", "y"].iter().any(|axis| {
            location
                .and_then(|location| location.get(*axis))
                .and_then(Value::as_i64)
                .and_then(|coordinate| i32::try_from(coordinate).ok())
                .is_none()
        }) {
            return;
        }
        self.queue_immediate_sound(
            provenance,
            MINE_HIT_WALL_SOUND_CUE,
            MINE_HIT_WALL_SOUND_FILE,
        );
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
        let detonating = spell_name == "ExplosiveTrap"
            && (payload.get("param").and_then(Value::as_bool) == Some(true) || param != 0);
        let animation = if detonating {
            catalog.map_animation("ArcherExplosiveTrapDetonation", 0)
        } else {
            catalog.spell_world_animation(&spell_name, direction, param)
        };
        let Some(mut anim) = animation else {
            return;
        };
        // Persistent world spell loops until it is removed by ObjectRemove.
        anim.repeat = !detonating;
        let now = self.now_ms;
        let key = format!("spell-{object_id}");
        self.clear_effect_key(&key);
        if detonating {
            self.queue_immediate_sound(provenance, "ExplosiveTrap.detonate", "M124-5.wav");
        }
        self.active.push(EffectInstance {
            key,
            kind: if detonating {
                EffectKindTag::Ground
            } else {
                EffectKindTag::Persistent
            },
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
        self.five_class_actor_contexts.remove(&object_id);
        self.elemental_state.remove(&object_id);
        if self
            .elemental_loop_owner
            .is_some_and(|(id, _)| id == object_id)
        {
            self.queue_element_loop(
                None,
                &EffectProvenance {
                    generation: self.last_generation,
                    sequence: self.last_effect_sequence,
                    packet: "ObjectRemove".to_owned(),
                    spell: "Meditation".to_owned(),
                },
            );
        }
        self.elemental_orb_thresholds
            .retain(|(id, _), _| *id != object_id);
        self.concentration_paused_at.remove(&object_id);
        self.delayed_explosion_stages.remove(&object_id);
        self.elemental_barrier_objects.remove(&object_id);
        self.elemental_barrier_down_objects.remove(&object_id);
        for projectile in self.five_class_projectiles.values_mut() {
            if projectile.target_id == Some(object_id) {
                projectile.target_id = None;
            }
        }
        self.clear_magic_shield_effect(object_id);
        self.magic_shield_down_object_ids.remove(&object_id);
        self.render_actor_object_ids.remove(&object_id);
        self.render_magic_shield_object_ids.remove(&object_id);
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
                        && (left_guard_start_times
                            .get(key)
                            .is_some_and(|start_at| self.now_ms > *start_at)
                            || self.active.iter().any(|instance| {
                                instance.key == *key
                                    && (ground_talisman(&instance.provenance.spell).is_some()
                                        || instance.provenance.spell == "ArcherNormalArrow")
                                    && self.now_ms >= instance.start_at
                            })))
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
        self.five_class_projectiles
            .retain(|key, _| !removed_keys.contains(key));
        self.hidden_five_class_effect_keys
            .retain(|key| !removed_keys.contains(key));
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
        self.suppress_dead_target_impacts_at_completion(now_ms);
        let hidden_five_class_effect_keys = &self.hidden_five_class_effect_keys;

        let rendered = self
            .active
            .iter_mut()
            .filter_map(|instance| {
                if hidden_five_class_effect_keys.contains(&instance.key) { return None; }
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
        self.five_class_projectiles
            .retain(|key, _| live_keys.contains(&key.as_str()));
        self.hidden_five_class_effect_keys
            .retain(|key| live_keys.contains(&key.as_str()));
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
            let progress = if ground_talisman(&instance.provenance.spell).is_some() {
                // Missile.Draw uses the integer processing frame, rather than
                // interpolating continuously between Crystal's 30 ms steps.
                let count = (animation.duration_ms / 30).max(1);
                let interval = (animation.duration_ms / count).max(1);
                (now_ms.saturating_sub(started_at) / interval).min(count) as f32 / count as f32
            } else {
                projectile_progress(animation.duration_ms, now_ms.saturating_sub(started_at))
            };
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
    presentation: bevy::prelude::Res<crate::entity_presentation::NativeEntityPresentation>,
    mut gameplay_audio: Option<
        bevy::prelude::ResMut<mir2_client_bevy::audio::NativeGameplayAudioQueue>,
    >,
    player_ui: Option<
        bevy::prelude::Res<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>,
    >,
) {
    let now_ms = u64::try_from(time.elapsed().as_millis()).unwrap_or(u64::MAX);
    effects.replace_dead_action_object_ids(presentation.dead_action_object_ids());
    effects.reconcile_double_slash_body_clocks(&presentation);
    effects.reconcile_snake_totem_auras(&presentation.snake_totem_aura_ready_object_ids());
    if let Some(queue) = gameplay_audio.as_deref_mut() {
        for event in effects.take_due_sound_events(now_ms) {
            queue.push(event);
        }
        for control in std::mem::take(&mut effects.ready_element_loop_controls) {
            queue.set_loop(
                control.generation,
                control.sequence,
                "Elemental.orbs",
                control.file_name,
            );
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

    fn five_class_test_actor(class: &str) -> Value {
        json!({"objectId": 1, "kind": "selfPlayer", "classKey": class,
            "genderKey": "male", "x": 10, "y": 10, "direction": "right",
            "dead": false, "buffs": [], "ridingMount": false,
            "sprite": {"bodyLibrary": if class == "Archer" { "ARArmour/00" } else { "AArmour/00" }}})
    }

    fn five_class_test_event(sequence: u64, packet: &str, payload: Value) -> NativeEffectEvent {
        NativeEffectEvent {
            generation: 77,
            sequence,
            packet: packet.to_owned(),
            payload,
        }
    }

    fn five_class_test_magic(sequence: u64, spell: &str, class: &str) -> NativeEffectEvent {
        five_class_test_event(
            sequence,
            "ObjectMagic",
            json!({
                "objectId": 1, "spell": spell, "cast": true, "direction": "right",
                "location": {"x": 10, "y": 10}, "target": {"x": 16, "y": 10}, "targetId": 2,
                "_nativeAttacker": five_class_test_actor(class),
                "_nativeTarget": {"objectId": 2, "kind": "monster", "image": 5,
                    "x": 16, "y": 10, "dead": false, "sprite": {"bodyLibrary": "Monster/005"}}
            }),
        )
    }

    fn five_class_test_fx(class: &str) -> NativeEffects {
        effect_catalog()
            .as_ref()
            .expect("five-class tests require the real exported effects manifest and metadata");
        let mut fx = NativeEffects::default();
        fx.observe_render_payload(&json!({"mapIndex": 1, "entities": [
            five_class_test_actor(class), {"objectId": 2, "kind": "monster", "image": 5,
                "x": 16, "y": 10, "dead": false, "sprite": {"bodyLibrary": "Monster/005"}}
        ]}));
        fx
    }

    #[test]
    fn five_class_effect_catalog_requires_the_complete_real_source_frame_union() {
        let catalog = EffectCatalog::load().expect("real worktree effect exports must be present");
        assert_eq!(catalog.frame_asset_root, exported_effect_test_root());
        let requirements_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../web/scripts/asset-pipeline/five-class-skill-requirements.json");
        let requirements: Value = serde_json::from_slice(
            &fs::read(requirements_path).expect("source-bound five-class skill requirements"),
        )
        .expect("valid source-bound skill requirements JSON");
        assert_eq!(requirements["profileId"], "platinum_fiveclass");
        assert_eq!(requirements["skills"].as_array().unwrap().len(), 34);
        for library in requirements["effects"]
            .as_array()
            .expect("effect frame union")
        {
            let name = library["library"].as_str().expect("source library name");
            for required in library["frames"].as_array().expect("required source frames") {
                let index = required["index"].as_i64().expect("source frame index");
                let frames = catalog.resolve_frames(name, index, 1);
                assert_eq!(frames.len(), 1, "missing real {name} frame {index}");
                let frame = &frames[0];
                let png_path = exported_effect_test_path(&catalog.frame_asset_root, &frame.path)
                    .expect("source frame PNG must exist in this checkout");
                let png = fs::read(png_path).expect("read actual source PNG");
                assert!(png.len() >= 24, "truncated source PNG {name}/{index}");
                assert_eq!(
                    &png[..8],
                    b"\x89PNG\r\n\x1a\n",
                    "{name}/{index} PNG signature"
                );
                assert_eq!(&png[12..16], b"IHDR", "{name}/{index} PNG dimensions");
                let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
                let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
                assert!(width > 0 && height > 0, "empty source PNG {name}/{index}");
                assert_eq!(frame.width, width as f32, "{name}/{index} metadata width");
                assert_eq!(frame.height, height as f32, "{name}/{index} metadata height");
            }
        }
    }

    #[test]
    fn five_class_effect_fixture_rejects_missing_png_and_path_escape_without_fallback() {
        let root = exported_effect_test_root();
        for invalid in [
            "",
            "../effects.generated.json",
            "/original-effects/../Magic2/1980.png",
            "C:/original-effects/Magic2/1980.png",
            "original-effects\\Magic2\\1980.png",
        ] {
            assert!(
                exported_effect_test_path(&root, invalid).is_none(),
                "{invalid}"
            );
        }
        let missing = "/original-effects/Magic2/__missing_five_class_source_fixture__.png";
        assert!(exported_effect_test_path(&root, missing).is_none());
        let mut catalog = EffectCatalog::load().expect("real source fixture");
        assert_eq!(catalog.resolve_frames("Magic2", 1980, 1).len(), 1);
        catalog
            .libraries
            .get_mut("Magic2")
            .unwrap()
            .frames
            .get_mut("1980")
            .unwrap()
            .path = missing.to_owned();
        assert!(catalog.resolve_frames("Magic2", 1980, 1).is_empty());
    }

    #[test]
    fn five_class_double_slash_uses_two_source_overlays_and_cancels_removed_actor() {
        let zone = HashMap::from([(1, (10, 10)), (2, (16, 10))]);
        let event = five_class_test_event(
            1,
            "ObjectAttack",
            json!({
                "objectId": 1, "spell": "DoubleSlash", "direction": "right",
                "_nativeAttacker": five_class_test_actor("Assassin")
            }),
        );
        let mut fx = five_class_test_fx("Assassin");
        fx.observe(0, 10, 10, &[event.clone()], &zone);
        assert_eq!(fx.active.len(), 2);
        assert_eq!((fx.active[0].start_at, fx.active[1].start_at), (0, 276));
        let first = fx.active[0].current.as_ref().unwrap();
        let second = fx.active[1].current.as_ref().unwrap();
        assert_eq!(
            (first.frames.len(), first.interval, first.duration_ms),
            (6, 46, 276)
        );
        assert!(first.frames[0].path.ends_with("/Magic2/1980.png"));
        assert!(second.frames[0].path.ends_with("/Magic2/2070.png"));
        assert_eq!(fx.take_due_sound_events(0)[0].file_name, "M92-0.wav");
        fx.observe(1, 10, 10, &[event], &zone);
        assert!(fx.take_due_sound_events(275).is_empty());
        assert_eq!(fx.take_due_sound_events(276)[0].file_name, "M92-1.wav");
        assert!(fx.take_due_sound_events(277).is_empty());
        let mut rejected = five_class_test_fx("Warrior");
        rejected.observe(
            0,
            10,
            10,
            &[five_class_test_event(
                1,
                "ObjectAttack",
                json!({
                    "objectId": 1, "spell": 92, "direction": "right",
                    "_nativeAttacker": five_class_test_actor("Warrior")
                }),
            )],
            &zone,
        );
        assert!(rejected.active.is_empty());
        assert!(rejected.take_due_sound_events(1_000).is_empty());
        fx.observe(
            278,
            10,
            10,
            &[five_class_test_event(
                2,
                "ObjectRemove",
                json!({"objectId": 1}),
            )],
            &HashMap::from([(2, (16, 10))]),
        );
        assert!(fx.active.is_empty());
    }

    #[test]
    fn five_class_double_slash_overlay_waits_for_matching_body_fifo_and_revival_cancels() {
        use crate::entity_presentation::NativeEntityPresentation;
        let zone = HashMap::from([(1, (10, 10))]);
        let mut actor = five_class_test_actor("Assassin");
        actor["_nativeAnimationAction"] = json!("walking");
        actor["_nativeAnimationSequence"] = json!(1);
        let mut presentation = NativeEntityPresentation::default();
        presentation.observe_packet_payload(json!({"entities": [actor.clone()]}), 0);
        actor["_nativeAnimationAction"] = json!("attack1");
        actor["_nativeAnimationSequence"] = json!(2);
        actor["_nativeAnimationSpell"] = json!("DoubleSlash");
        let snapshot = json!({"entities": [actor.clone()]});
        presentation.observe_packet_payload(snapshot.clone(), 10);
        let mut fx = five_class_test_fx("Assassin");
        fx.observe(10, 10, 10, &[five_class_test_event(1, "ObjectAttack", json!({
            "objectId": 1, "spell": "DoubleSlash", "direction": "right", "_nativeAttacker": actor
        }))], &zone);
        fx.reconcile_double_slash_body_clocks(&presentation);
        assert!(fx
            .active
            .iter()
            .all(|instance| instance.start_at == u64::MAX));
        assert!(fx.take_due_sound_events(599).is_empty());
        presentation.observe_packet_payload(snapshot.clone(), 600);
        fx.reconcile_double_slash_body_clocks(&presentation);
        assert_eq!((fx.active[0].start_at, fx.active[1].start_at), (600, 876));
        assert_eq!(fx.take_due_sound_events(600)[0].file_name, "M92-0.wav");
        presentation.observe_packet_payload(snapshot, 875);
        fx.reconcile_double_slash_body_clocks(&presentation);
        assert!(fx.take_due_sound_events(875).is_empty());
        // An actual revival is a life barrier, not the queued second slash.
        let mut revived = five_class_test_actor("Assassin");
        revived["_nativeAnimationAction"] = json!("revive");
        revived["_nativeAnimationSequence"] = json!(3);
        presentation.observe_packet_payload(json!({"entities": [revived]}), 875);
        fx.reconcile_double_slash_body_clocks(&presentation);
        assert!(fx.active.is_empty());
        assert!(fx.take_due_sound_events(876).is_empty());
    }

    #[test]
    fn five_class_straight_and_double_shots_use_source_launch_clocks_and_no_compat_duplicate() {
        let zone = HashMap::from([(1, (10, 10)), (2, (16, 10))]);
        let mut straight = five_class_test_fx("Archer");
        straight.observe(
            0,
            10,
            10,
            &[five_class_test_magic(1, "StraightShot", "Archer")],
            &zone,
        );
        assert_eq!(straight.active.len(), 1);
        assert_eq!(straight.active[0].start_at, 800);
        assert!(straight.take_due_sound_events(799).is_empty());
        assert_eq!(
            straight.take_due_sound_events(800)[0].file_name,
            "M122-0.wav"
        );
        assert!(straight.take_due_sound_events(1_099).is_empty());
        assert_eq!(
            straight.take_due_sound_events(1_100)[0].file_name,
            "M122-2.wav"
        );
        assert_eq!(straight.active.len(), 1);
        assert!(straight.active[0].current.as_ref().unwrap().frames[0]
            .path
            .ends_with("/Magic3/1370.png"));
        let mut double = five_class_test_fx("Archer");
        double.observe(
            0,
            10,
            10,
            &[
                five_class_test_magic(1, "DoubleShot", "Archer"),
                five_class_test_event(
                    2,
                    "ObjectProjectile",
                    json!({"spell": "DoubleShot",
                "sourceId": 1, "destinationId": 2, "source": {"x": 10, "y": 10},
                "destination": {"x": 16, "y": 10}}),
                ),
            ],
            &zone,
        );
        assert_eq!(double.active.len(), 2);
        assert_eq!(
            (double.active[0].start_at, double.active[1].start_at),
            (500, 700)
        );
        assert!(double.take_due_sound_events(499).is_empty());
        let first = double.take_due_sound_events(500);
        let second = double.take_due_sound_events(700);
        assert_eq!(
            (first[0].file_name.as_str(), second[0].file_name.as_str()),
            ("M121-0.wav", "M121-0.wav")
        );
        assert_ne!(first[0].cue, second[0].cue);
        assert_eq!(double.take_due_sound_events(800)[0].file_name, "M121-2.wav");
        assert_eq!(
            double.take_due_sound_events(1_000)[0].file_name,
            "M121-2.wav"
        );
        assert!(double.take_due_sound_events(1_001).is_empty());
    }

    #[test]
    fn five_class_failed_or_wrong_class_shots_and_dead_straight_impact_fail_closed() {
        let zone = HashMap::from([(1, (10, 10)), (2, (16, 10))]);
        for (spell, class, cast) in [
            ("StraightShot", "Archer", false),
            ("StraightShot", "Warrior", true),
            ("CatTongue", "Archer", true),
        ] {
            let mut fx = five_class_test_fx(class);
            let mut event = five_class_test_magic(1, spell, class);
            event.payload["cast"] = json!(cast);
            fx.observe(0, 10, 10, &[event], &zone);
            assert!(fx.active.is_empty(), "{spell}/{class}/{cast}");
            assert!(fx.take_due_sound_events(10_000).is_empty());
        }
        let mut fx = five_class_test_fx("Archer");
        fx.observe(
            0,
            10,
            10,
            &[five_class_test_magic(1, "StraightShot", "Archer")],
            &zone,
        );
        let _ = fx.take_due_sound_events(800);
        fx.replace_dead_action_object_ids(HashSet::from([2]));
        assert!(fx.take_due_sound_events(1_100).is_empty());
        assert!(fx.active.is_empty());
    }

    #[test]
    fn five_class_vampire_return_is_caster_attached_after_target_impact() {
        let zone = HashMap::from([(1, (10, 10)), (2, (16, 10))]);
        let mut fx = five_class_test_fx("Archer");
        fx.observe(
            0,
            10,
            10,
            &[five_class_test_magic(1, "VampireShot", "Archer")],
            &zone,
        );
        assert_eq!(fx.take_due_sound_events(700)[0].file_name, "M121-0.wav");
        assert_eq!(fx.take_due_sound_events(1_000)[0].file_name, "M133-2.wav");
        let impact = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-five-class-impact-"))
            .unwrap();
        let return_at = 1_000 + impact.current.as_ref().unwrap().duration_ms;
        let returning = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-archer-vampire-return-"))
            .unwrap();
        assert_eq!(returning.start_at, return_at);
        assert_eq!(fx.anchor_object_ids[&returning.key], 1);
        assert!(returning.from_x.is_none() && returning.from_y.is_none());
        assert_eq!(returning.current.as_ref().unwrap().kind, "target");
        assert!(fx.take_due_sound_events(return_at - 1).is_empty());
        assert_eq!(
            fx.take_due_sound_events(return_at)[0].file_name,
            "M45-2.wav"
        );
        let moved_zone = HashMap::from([(1, (11, 11)), (2, (16, 10))]);
        fx.observe(return_at, 11, 11, &[], &moved_zone);
        let returning = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-archer-vampire-return-"))
            .unwrap();
        assert_eq!((returning.tile_x, returning.tile_y), (11, 11));
    }

    #[test]
    fn five_class_cripple_uses_current_real_poison_buff_before_vampire() {
        let zone = HashMap::from([(1, (10, 10)), (2, (16, 10))]);
        for (buffs, expected_frame, expected_sound, returns) in [
            (Some(json!([])), Some(2490), Some("M136-5.wav"), false),
            (Some(json!([16])), Some(2510), Some("M136-7.wav"), true),
            (Some(json!([16, 17])), Some(2500), Some("M136-6.wav"), false),
            (None, None, None, false),
        ] {
            let mut fx = five_class_test_fx("Archer");
            fx.observe(
                0,
                10,
                10,
                &[five_class_test_magic(1, "CrippleShot", "Archer")],
                &zone,
            );
            let _ = fx.take_due_sound_events(700);
            let actor = fx
                .five_class_actor_contexts
                .get_mut(&1)
                .unwrap()
                .as_object_mut()
                .unwrap();
            if let Some(buffs) = buffs {
                actor.insert("buffs".to_owned(), buffs);
            } else {
                actor.remove("buffs");
            }
            let sounds = fx.take_due_sound_events(1_000);
            if let Some(frame) = expected_frame {
                let impact = fx
                    .active
                    .iter()
                    .find(|instance| instance.key.starts_with("fx-five-class-impact-"))
                    .unwrap();
                assert!(impact.current.as_ref().unwrap().frames[0]
                    .path
                    .ends_with(&format!("/Magic3/{frame}.png")));
                assert_eq!(sounds[0].file_name, expected_sound.unwrap());
            } else {
                assert!(fx.active.is_empty() && sounds.is_empty());
            }
            assert_eq!(
                fx.active
                    .iter()
                    .any(|instance| instance.key.starts_with("fx-archer-vampire-return-")),
                returns
            );
        }
    }

    #[test]
    fn five_class_target_effects_preserve_three_hemorrhage_layers_and_mp_owner() {
        let zone = HashMap::from([(1, (10, 10)), (2, (16, 10))]);
        let mut fx = five_class_test_fx("Assassin");
        fx.observe(
            0,
            10,
            10,
            &[
                five_class_test_event(
                    1,
                    "ObjectEffect",
                    json!({"objectId": 2, "effect": 17, "effectType": 0, "delayTime": 500}),
                ),
                five_class_test_event(
                    2,
                    "ObjectEffect",
                    json!({"objectId": 2, "effect": 16, "effectType": 1, "delayTime": 500}),
                ),
            ],
            &zone,
        );
        assert_eq!(fx.active.len(), 5);
        let mut starts = fx
            .active
            .iter()
            .flat_map(|instance| instance.current.as_ref())
            .map(|animation| (animation.frames[0].path.clone(), animation.duration_ms))
            .collect::<Vec<_>>();
        starts.sort();
        for (library, frame, duration) in [
            ("Magic3", 0, 400),
            ("Magic3", 28, 600),
            ("Magic3", 46, 800),
            ("Magic2", 2400, 900),
            ("Magic2", 2411, 1_900),
        ] {
            assert!(starts.iter().any(|(path, time)| path
                .ends_with(&format!("/{library}/{frame}.png"))
                && *time == duration));
        }
        let owner = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-mp-eater-owner-"))
            .unwrap();
        assert_eq!(fx.anchor_object_ids[&owner.key], 1);
        assert!(fx.active.iter().all(|instance| instance.start_at == 0));
        let sounds = fx.take_due_sound_events(0);
        assert_eq!(
            sounds
                .iter()
                .map(|sound| sound.file_name.as_str())
                .collect::<HashSet<_>>(),
            HashSet::from(["M104-0.wav", "M91-0.wav"])
        );
    }

    #[test]
    fn five_class_concentration_pauses_hides_and_resumes_without_restarting_sound() {
        let zone = HashMap::from([(1, (10, 10)), (2, (16, 10))]);
        let mut fx = five_class_test_fx("Archer");
        fx.observe(
            0,
            10,
            10,
            &[five_class_test_event(
                1,
                "SetConcentration",
                json!({
            "objectId": 1, "enabled": true, "interrupted": false}),
            )],
            &zone,
        );
        assert_eq!(fx.take_due_sound_events(0)[0].file_name, "M129-0.wav");
        fx.observe(
            350,
            10,
            10,
            &[five_class_test_event(
                2,
                "SetConcentration",
                json!({
            "objectId": 1, "enabled": true, "interrupted": true}),
            )],
            &zone,
        );
        assert!(fx
            .hidden_five_class_effect_keys
            .contains("archer-concentration-1"));
        assert!(fx.current_light_snapshots(700).is_empty());
        fx.observe(
            800,
            10,
            10,
            &[five_class_test_event(
                3,
                "SetConcentration",
                json!({
            "objectId": 1, "enabled": true, "interrupted": false}),
            )],
            &zone,
        );
        assert!(fx.hidden_five_class_effect_keys.is_empty());
        assert_eq!(fx.active[0].start_at, 450);
        assert!(fx.take_due_sound_events(800).is_empty());
        fx.observe(
            900,
            10,
            10,
            &[five_class_test_event(
                4,
                "SetConcentration",
                json!({
            "objectId": 1, "enabled": false, "interrupted": false}),
            )],
            &zone,
        );
        assert!(fx.active.is_empty());
    }

    #[test]
    fn five_class_elemental_arrow_requires_typed_state_and_owner_loop_stops_at_boundary() {
        let zone = HashMap::from([(1, (10, 10)), (2, (16, 10))]);
        let mut absent = five_class_test_fx("Archer");
        absent.observe(
            0,
            10,
            10,
            &[five_class_test_magic(1, "ElementalShot", "Archer")],
            &zone,
        );
        assert!(absent.active.is_empty());
        let mut fx = five_class_test_fx("Archer");
        fx.observe(
            0,
            10,
            10,
            &[five_class_test_event(
                1,
                "SetElemental",
                json!({
                    "objectId": 1, "enabled": true, "casted": true, "value": 1,
                    "elementType": 3, "expLast": 3
                }),
            )],
            &zone,
        );
        assert_eq!(
            fx.ready_element_loop_controls.last().unwrap().file_name,
            Some("M126-7.wav")
        );
        // Crystal ignores Casted only for the actual local owner.
        fx.observe(
            0,
            10,
            10,
            &[five_class_test_magic(2, "ElementalShot", "Archer")],
            &zone,
        );
        assert_eq!(fx.active.len(), 3);
        assert!(fx.take_due_sound_events(99).is_empty());
        assert_eq!(fx.take_due_sound_events(100)[0].file_name, "M128-0.wav");
        fx.observe(
            200,
            10,
            10,
            &[five_class_test_event(3, "MapChanged", json!({}))],
            &HashMap::new(),
        );
        assert!(fx.active.is_empty());
        assert_eq!(
            fx.ready_element_loop_controls.last().unwrap().file_name,
            None
        );
        assert!(fx.take_due_sound_events(1_000).is_empty());
        assert!(fx.elemental_state.is_empty());
    }

    #[test]
    fn five_class_binding_uses_receipt_deadline_and_three_source_phases() {
        let zone = HashMap::from([(1, (10, 10)), (2, (16, 10))]);
        let mut fx = five_class_test_fx("Archer");
        fx.observe(
            0,
            10,
            10,
            &[five_class_test_event(
                1,
                "SetBindingShot",
                json!({
            "objectId": 2, "enabled": true, "value": 6_000}),
            )],
            &zone,
        );
        assert_eq!(fx.active.len(), 1);
        let instance = &fx.active[0];
        assert_eq!(instance.current.as_ref().unwrap().duration_ms, 696);
        assert!(advance_instance(instance, 695).unwrap().0.frames[0]
            .path
            .ends_with("/MagicC/0.png"));
        assert!(advance_instance(instance, 696).unwrap().0.frames[0]
            .path
            .ends_with("/MagicC/7.png"));
        assert!(advance_instance(instance, 4_500).unwrap().0.frames[0]
            .path
            .ends_with("/MagicC/8.png"));
        assert!(advance_instance(instance, 5_196).is_none());
        assert!(fx.take_due_sound_events(695).is_empty());
        assert_eq!(fx.take_due_sound_events(696)[0].file_name, "M130-5.wav");
        assert!(fx.take_due_sound_events(4_499).is_empty());
        assert_eq!(fx.take_due_sound_events(4_500)[0].file_name, "M130-6.wav");
        fx.observe(
            4_501,
            10,
            10,
            &[five_class_test_event(
                2,
                "ObjectRemove",
                json!({"objectId": 2}),
            )],
            &HashMap::from([(1, (10, 10))]),
        );
        assert!(fx.active.is_empty());
        assert!(fx.take_due_sound_events(6_000).is_empty());
    }

    #[test]
    fn five_class_delayed_explosion_stages_are_monotonic_and_removed_explicitly() {
        let zone = HashMap::from([(1, (10, 10)), (2, (16, 10))]);
        let mut fx = five_class_test_fx("Archer");
        for (sequence, at, requested, expected) in
            [(1, 0, 2, 0), (2, 100, 1, 1), (3, 200, 0, 1), (4, 300, 2, 2)]
        {
            fx.observe(
                at,
                10,
                10,
                &[five_class_test_event(
                    sequence,
                    "ObjectEffect",
                    json!({"objectId": 2, "effect": 15, "effectType": requested}),
                )],
                &zone,
            );
            assert_eq!(fx.delayed_explosion_stages[&2], expected);
            let animation = fx
                .active
                .iter()
                .find(|instance| instance.key == "delayed-explosion-2")
                .unwrap()
                .current
                .as_ref()
                .unwrap();
            assert_eq!(animation.repeat, expected != 2);
            assert!(animation.frames[0]
                .path
                .ends_with(&format!("/Magic3/{}.png", 1590 + expected * 10)));
        }
        fx.observe(
            400,
            10,
            10,
            &[five_class_test_event(
                5,
                "RemoveDelayedExplosion",
                json!({"objectId": 2}),
            )],
            &zone,
        );
        assert!(fx.active.is_empty() && fx.delayed_explosion_stages.is_empty());
        assert!(fx.take_due_sound_events(5_000).is_empty());
    }

    #[test]
    fn five_class_explosive_trap_boolean_detonation_is_ground_nonrepeat_and_finite() {
        let mut fx = NativeEffects::default();
        let armed = five_class_test_event(
            1,
            "ObjectSpell",
            json!({"objectId": 99, "spell": 124,
            "location": {"x": 15, "y": 10}, "direction": "right", "param": false}),
        );
        fx.observe(0, 10, 10, &[armed], &HashMap::new());
        assert_eq!(fx.active.len(), 1);
        assert!(fx.active[0].current.as_ref().unwrap().repeat);
        fx.observe(
            10,
            10,
            10,
            &[five_class_test_event(
                2,
                "ObjectSpell",
                json!({
            "objectId": 99, "spell": 124, "location": {"x": 15, "y": 10},
            "direction": "right", "param": true}),
            )],
            &HashMap::new(),
        );
        assert_eq!(fx.active.len(), 1);
        assert_eq!(fx.active[0].kind, EffectKindTag::Ground);
        let animation = fx.active[0].current.as_ref().unwrap();
        assert_eq!(
            (
                animation.frames.len(),
                animation.duration_ms,
                animation.repeat
            ),
            (9, 900, false)
        );
        assert!(animation.frames[0].path.ends_with("/Magic3/1570.png"));
        assert_eq!(fx.take_due_sound_events(10)[0].file_name, "M124-5.wav");
        assert!(instance_still_active(&fx.active[0], 909));
        assert!(!instance_still_active(&fx.active[0], 910));
    }

    #[test]
    fn five_class_dark_body_smoke_survives_actor_remove_and_back_step_gender_clocks() {
        let zone = HashMap::from([(1, (10, 10))]);
        let mut smoke = five_class_test_fx("Assassin");
        smoke.observe(
            0,
            10,
            10,
            &[five_class_test_event(
                1,
                "ObjectDied",
                json!({
            "objectId": 1, "kind": 2, "location": {"x": 10, "y": 10},
            "_nativeTarget": five_class_test_actor("Assassin")}),
            )],
            &zone,
        );
        assert_eq!(smoke.active.len(), 1);
        assert!(smoke.anchor_object_ids.is_empty() && smoke.source_object_ids.is_empty());
        let animation = smoke.active[0].current.as_ref().unwrap();
        assert_eq!((animation.frames.len(), animation.duration_ms), (10, 1_200));
        assert!(animation.frames[0].path.ends_with("/Magic2/2600.png"));
        assert_eq!(smoke.take_due_sound_events(0)[0].file_name, "M103-1.wav");
        assert!(smoke.five_class_actor_contexts.get(&1).is_none());
        for (gender, expected) in [("male", "M127-5.wav"), ("female", "M127-6.wav")] {
            let mut fx = five_class_test_fx("Archer");
            let mut actor = five_class_test_actor("Archer");
            actor["genderKey"] = json!(gender);
            fx.observe(
                0,
                10,
                10,
                &[five_class_test_event(
                    1,
                    "ObjectBackStep",
                    json!({"objectId": 1, "_nativeAttacker": actor}),
                )],
                &zone,
            );
            assert!(fx.active.is_empty());
            assert!(fx.take_due_sound_events(99).is_empty());
            assert_eq!(fx.take_due_sound_events(100)[0].file_name, expected);
            assert!(fx.take_due_sound_events(699).is_empty());
            assert_eq!(fx.take_due_sound_events(700)[0].file_name, "M127-7.wav");
            assert!(fx.take_due_sound_events(800).is_empty());
        }
    }

    #[test]
    fn five_class_barrier_aoi_recovery_struck_and_down_do_not_replay_stale_up() {
        let zone = HashMap::from([(1, (10, 10))]);
        let mut fx = NativeEffects::default();
        let mut actor = five_class_test_actor("Archer");
        actor["effect"] = json!(13);
        let snapshot = json!({"mapIndex": 1, "entities": [actor]});
        fx.observe_render_payload(&snapshot);
        fx.observe(
            0,
            10,
            10,
            &[five_class_test_event(
                1,
                "ObjectPlayer",
                json!({"objectId": 1}),
            )],
            &zone,
        );
        assert_eq!(fx.active[0].current.as_ref().unwrap().frames.len(), 16);
        assert!(fx.elemental_barrier_objects.contains(&1));
        fx.observe(
            1,
            10,
            10,
            &[five_class_test_event(
                2,
                "ObjectStruck",
                json!({"objectId": 1}),
            )],
            &zone,
        );
        let barrier = fx
            .active
            .iter()
            .find(|instance| instance.key == "archer-barrier-1")
            .unwrap();
        assert_eq!(
            (
                barrier.current.as_ref().unwrap().frames.len(),
                barrier.current.as_ref().unwrap().duration_ms
            ),
            (5, 600)
        );
        assert_eq!(
            (
                barrier.queued.as_ref().unwrap().frames.len(),
                barrier.queued.as_ref().unwrap().duration_ms
            ),
            (16, 3_200)
        );
        fx.observe(
            2,
            10,
            10,
            &[five_class_test_event(
                3,
                "ObjectEffect",
                json!({"objectId": 1, "effect": 14}),
            )],
            &zone,
        );
        assert!(!fx.elemental_barrier_objects.contains(&1));
        fx.observe_render_payload(&snapshot);
        fx.observe(3, 10, 10, &[], &zone);
        assert!(!fx
            .active
            .iter()
            .any(|instance| instance.key == "archer-barrier-1"));
        assert!(fx.elemental_barrier_down_objects.contains(&1));
        fx.observe(
            4,
            10,
            10,
            &[five_class_test_event(
                4,
                "ObjectEffect",
                json!({"objectId": 1, "effect": 13}),
            )],
            &zone,
        );
        assert!(fx.elemental_barrier_objects.contains(&1));
        assert!(!fx.elemental_barrier_down_objects.contains(&1));
    }

    #[test]
    fn five_class_summon_effects_require_real_actor_and_actual_standing_cycle() {
        let zone = HashMap::from([(1, (10, 10)), (2, (16, 10)), (3, (11, 11))]);
        let mut fx = five_class_test_fx("Archer");
        let actor = json!({"objectId": 3, "kind": "monster", "image": 361, "dead": false,
            "x": 11, "y": 11, "sprite": {"bodyLibrary": "Monster/361"}});
        fx.observe_render_payload(
            &json!({"mapIndex": 1, "entities": [five_class_test_actor("Archer"), actor]}),
        );
        fx.observe(0, 10, 10, &[], &zone);
        fx.reconcile_snake_totem_auras(&HashSet::new());
        assert!(fx.active.is_empty());
        fx.reconcile_snake_totem_auras(&HashSet::from([3]));
        assert_eq!(fx.active.len(), 1);
        assert_eq!(fx.active[0].key, "snake-totem-aura-3");
        assert!(fx.active[0].current.as_ref().unwrap().repeat);
        assert!(fx.active[0].current.as_ref().unwrap().frames[0]
            .path
            .ends_with("/Monster/361/2.png"));
        fx.reconcile_snake_totem_auras(&HashSet::from([3]));
        assert_eq!(fx.active.len(), 1);
        fx.reconcile_snake_totem_auras(&HashSet::new());
        assert!(fx.active.is_empty());
        for (image, library, frame, duration) in [
            (359, "Monster/359", 296, 1_000),
            (362, "Monster/362", 40, 800),
        ] {
            fx.observe(
                0,
                10,
                10,
                &[five_class_test_event(
                    image,
                    "ObjectDied",
                    json!({
                "objectId": 3, "kind": 0, "_nativeTarget": {"objectId": 3, "kind": "monster",
                    "image": image, "dead": true, "sprite": {"bodyLibrary": library}}}),
                )],
                &zone,
            );
            let animation = fx.active.last().unwrap().current.as_ref().unwrap();
            assert_eq!(animation.duration_ms, duration);
            assert!(animation.frames[0]
                .path
                .ends_with(&format!("/{library}/{frame}.png")));
        }
        let mut rock = five_class_test_fx("Archer");
        rock.observe(
            0,
            10,
            10,
            &[five_class_test_event(
                1,
                "ObjectRangeAttack",
                json!({
            "objectId": 3, "targetId": 1, "_nativeAttacker": {"objectId": 3,
                "kind": "monster", "image": 130, "sprite": {"bodyLibrary": "Monster/130"}}}),
            )],
            &zone,
        );
        assert_eq!(rock.active.len(), 1);
        assert_eq!(rock.active[0].start_at, 400);
        assert!(rock.active[0].current.as_ref().unwrap().frames[0]
            .path
            .ends_with("/Monster/130/26.png"));
        assert!(rock.take_due_sound_events(399).is_empty());
        assert_eq!(rock.take_due_sound_events(400)[0].file_name, "130-6.wav");
    }

    #[test]
    fn five_class_aoi_orbs_restore_true_colors_and_disabled_state_beats_stale_snapshot() {
        let zone = HashMap::from([(1, (10, 10))]);
        let mut actor = five_class_test_actor("Archer");
        actor["elementOrbEffect"] = json!(3);
        actor["elementOrbLevel"] = json!(1);
        actor["elementOrbMax"] = json!(3);
        let snapshot = json!({"mapIndex": 1, "entities": [actor.clone()]});
        let mut fx = NativeEffects::default();
        fx.observe_render_payload(&snapshot);
        fx.observe(
            0,
            10,
            10,
            &[five_class_test_event(1, "ObjectPlayer", actor)],
            &zone,
        );
        assert_eq!(fx.active.len(), 3);
        assert!(fx.elemental_state[&1].enabled);
        assert!(fx
            .active
            .iter()
            .all(|instance| instance.current.as_ref().unwrap().repeat));
        assert!(!fx
            .ready_element_loop_controls
            .iter()
            .any(|control| control.file_name.is_some()));
        assert_eq!(fx.take_due_sound_events(0)[0].file_name, "M126-7.wav");
        fx.observe(
            100,
            10,
            10,
            &[five_class_test_event(
                2,
                "SetElemental",
                json!({
            "objectId": 1, "enabled": false, "casted": false, "value": 3,
            "elementType": 0, "expLast": 3}),
            )],
            &zone,
        );
        fx.observe_render_payload(&snapshot);
        fx.observe(101, 10, 10, &[], &zone);
        assert!(!fx.elemental_state[&1].enabled);
        assert!(fx.active.is_empty());
        assert!(fx.take_due_sound_events(101).is_empty());
    }

    #[test]
    fn five_class_source_orb_threshold_stops_only_its_current_loop() {
        let zone = HashMap::from([(1, (10, 10))]);
        let mut fx = five_class_test_fx("Archer");
        fx.observe(
            0,
            10,
            10,
            &[five_class_test_event(
                1,
                "SetElemental",
                json!({
            "objectId": 1, "enabled": true, "casted": false, "value": 1,
            "elementType": 3, "expLast": 3}),
            )],
            &zone,
        );
        assert_eq!(fx.elemental_loop_owner, Some((1, 3)));
        fx.observe(
            100,
            10,
            10,
            &[five_class_test_event(
                2,
                "SetElemental",
                json!({
            "objectId": 1, "enabled": true, "casted": false, "value": 3,
            "elementType": 0, "expLast": 3}),
            )],
            &zone,
        );
        assert!(fx.active.is_empty());
        assert_eq!(fx.elemental_loop_owner, None);
        assert_eq!(
            fx.ready_element_loop_controls.last().unwrap().file_name,
            None
        );
        assert!(fx.elemental_state[&1].enabled);
    }

    fn mining_map_event(generation: u64, sequence: u64) -> NativeEffectEvent {
        NativeEffectEvent {
            generation,
            sequence,
            packet: "MapEffect".to_owned(),
            payload: json!({"effect": 12, "value": 0, "location": {"x": 83, "y": 8}}),
        }
    }

    #[test]
    fn mining_wall_sound_does_not_depend_on_effect_images() {
        let mut fx = NativeEffects::default();
        let provenance = EffectProvenance {
            generation: 91,
            sequence: 40,
            packet: "MapEffect".to_owned(),
            spell: "effect".to_owned(),
        };
        // This enters the production map-effect handler. An unresolvable
        // wall-hit sprite must not suppress the original immediate sound.
        fx.apply_map_effect(
            &json!({"effect": 12, "value": u32::MAX, "location": {"x": 83, "y": 8}}),
            &provenance,
        );
        assert!(fx.active.is_empty());
        assert_eq!(
            fx.take_due_sound_events(0),
            vec![mir2_client_bevy::audio::NativeGameplaySoundEvent {
                generation: 91,
                sequence: 40,
                cue: MINE_HIT_WALL_SOUND_CUE.to_owned(),
                file_name: MINE_HIT_WALL_SOUND_FILE.to_owned(),
            }]
        );
        assert!(fx.take_due_sound_events(0).is_empty());
    }

    #[test]
    fn mining_map_effect_sound_dedupes_and_clears_at_scene_boundaries() {
        let mut fx = NativeEffects::default();
        let zone = HashMap::new();
        let first = mining_map_event(91, 1);
        fx.observe(100, 82, 8, &[first.clone()], &zone);
        let due = fx.take_due_sound_events(100);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].cue, MINE_HIT_WALL_SOUND_CUE);
        assert_eq!(due[0].file_name, MINE_HIT_WALL_SOUND_FILE);
        fx.observe(101, 82, 8, &[first], &zone);
        assert!(fx.take_due_sound_events(101).is_empty());
        fx.observe(200, 82, 8, &[mining_map_event(92, 1)], &zone);
        assert_eq!(fx.take_due_sound_events(200).len(), 1);
        for (sequence, boundary) in [(2, "MapChanged"), (4, "LogOutSuccess")] {
            fx.observe(
                300,
                82,
                8,
                &[
                    mining_map_event(92, sequence),
                    NativeEffectEvent {
                        generation: 92,
                        sequence: sequence + 1,
                        packet: boundary.to_owned(),
                        payload: json!({}),
                    },
                ],
                &zone,
            );
            assert!(
                fx.take_due_sound_events(300).is_empty(),
                "{boundary} must discard the pending old-scene sound"
            );
        }
    }

    #[test]
    fn mining_wall_sound_rejects_non_mine_or_malformed_packet_context() {
        let provenance = EffectProvenance {
            generation: 91,
            sequence: 40,
            packet: "MapEffect".to_owned(),
            spell: "effect".to_owned(),
        };
        for payload in [
            json!({"effect": 13, "location": {"x": 83, "y": 8}}),
            json!({"effect": "12", "location": {"x": 83, "y": 8}}),
            json!({"effect": 4_294_967_308_u64, "location": {"x": 83, "y": 8}}),
            json!({"effect": 12}),
            json!({"effect": 12, "location": {"x": 83}}),
            json!({"effect": 12, "location": {"x": 83, "y": "8"}}),
            json!({"effect": 12, "location": {"x": 83.5, "y": 8}}),
            json!({"effect": 12, "location": {"x": 2_147_483_648_i64, "y": 8}}),
        ] {
            let mut fx = NativeEffects::default();
            fx.apply_mine_wall_sound(&payload, &provenance);
            assert!(fx.take_due_sound_events(0).is_empty(), "{payload}");
        }
        for (packet, sequence) in [("ObjectAttack", 40), ("MapEffect", 0)] {
            let mut fx = NativeEffects::default();
            fx.apply_mine_wall_sound(
                &mining_map_event(91, 1).payload,
                &EffectProvenance {
                    packet: packet.to_owned(),
                    sequence,
                    ..provenance.clone()
                },
            );
            assert!(fx.take_due_sound_events(0).is_empty());
        }
    }

    #[test]
    fn mining_wall_clip_matches_original_source_sha256() {
        use sha2::{Digest, Sha256};
        let path = mir2_client_bevy::audio::discover_audio_file(MINE_HIT_WALL_SOUND_FILE)
            .expect("actual packaged/source SoundList 10091 clip");
        let bytes = fs::read(path).unwrap();
        assert_eq!(bytes.len(), 164_642);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(
            format!("{:X}", Sha256::digest(&bytes)),
            "EBCB5E32521CDF7256C334BE1951064EDD21E080264823B327BC6803ACDD38F1"
        );
    }

    fn magic_shield_effect_event(sequence: u64, effect: u32) -> NativeEffectEvent {
        NativeEffectEvent {
            sequence,
            generation: 18,
            packet: "ObjectEffect".to_owned(),
            payload: json!({
                "objectId": 1000,
                "effect": effect,
                "effectType": 0,
                "delayTime": 0,
                "time": 0
            }),
        }
    }

    #[test]
    fn magic_shield_up_is_idempotent_down_clears_and_recast_restores() {
        let zone = HashMap::from([(1000, (288, 616))]);
        let mut fx = NativeEffects::default();

        fx.observe(
            0,
            288,
            616,
            &[magic_shield_effect_event(1, MAGIC_SHIELD_UP_EFFECT)],
            &zone,
        );
        assert_eq!(fx.active.len(), 1);
        assert_eq!(fx.active[0].key, "magic-shield-1000");
        assert!(fx.active[0]
            .current
            .as_ref()
            .is_some_and(|animation| animation.repeat));

        fx.observe(
            1_100,
            288,
            616,
            &[magic_shield_effect_event(2, MAGIC_SHIELD_UP_EFFECT)],
            &zone,
        );
        assert_eq!(fx.active.len(), 1, "repeated Up must replace the loop");
        assert_eq!(fx.active[0].started_at, 1_100);

        fx.observe(
            1_200,
            288,
            616,
            &[magic_shield_effect_event(3, MAGIC_SHIELD_DOWN_EFFECT)],
            &HashMap::new(),
        );
        assert!(fx.active.is_empty(), "Down must not require a visible tile");
        assert!(!fx.anchor_object_ids.contains_key("magic-shield-1000"));

        fx.observe(
            1_300,
            288,
            616,
            &[magic_shield_effect_event(4, MAGIC_SHIELD_UP_EFFECT)],
            &zone,
        );
        assert_eq!(fx.active.len(), 1, "a later cast must restore the loop");
    }

    #[test]
    fn magic_shield_late_join_restores_without_stale_snapshot_overriding_down() {
        let zone = HashMap::from([(1000, (288, 616))]);
        let unshielded_actor = json!({
            "mapIndex": 1,
            "entities": [{
                "objectId": 1000,
                "kind": "player",
                "location": {"x": 288, "y": 616},
                "effect": 0,
                "dead": false
            }]
        });
        let shielded_actor = json!({
            "mapIndex": 1,
            "entities": [{
                "objectId": 1000,
                "kind": "player",
                "location": {"x": 288, "y": 616},
                "effect": MAGIC_SHIELD_UP_EFFECT,
                "dead": false
            }]
        });
        let mut fx = NativeEffects::default();

        fx.observe_render_payload(&unshielded_actor);
        fx.observe(
            1_000,
            288,
            616,
            &[magic_shield_effect_event(1, MAGIC_SHIELD_UP_EFFECT)],
            &zone,
        );
        assert_eq!(
            fx.active.len(),
            1,
            "a stale unshielded snapshot must not erase a newer Up"
        );

        fx.observe_render_payload(&shielded_actor);
        fx.observe(
            1_100,
            288,
            616,
            &[magic_shield_effect_event(2, MAGIC_SHIELD_DOWN_EFFECT)],
            &zone,
        );
        assert!(fx.active.is_empty());

        fx.observe_render_payload(&shielded_actor);
        fx.observe(1_200, 288, 616, &[], &zone);
        assert!(
            fx.active.is_empty(),
            "a stale ObjectPlayer snapshot must not override a newer Down"
        );

        fx.observe_render_payload(&shielded_actor);
        fx.observe(
            1_250,
            288,
            616,
            &[magic_shield_effect_event(3, MAGIC_SHIELD_UP_EFFECT)],
            &zone,
        );
        assert_eq!(fx.active.len(), 1, "a newer Up must clear Down suppression");
        fx.observe(
            1_275,
            288,
            616,
            &[magic_shield_effect_event(4, MAGIC_SHIELD_DOWN_EFFECT)],
            &zone,
        );
        assert!(fx.active.is_empty());

        fx.observe_render_payload(&json!({"mapIndex": 1, "entities": []}));
        fx.observe(1_300, 288, 616, &[], &HashMap::new());
        fx.observe_render_payload(&shielded_actor);
        fx.observe(1_400, 288, 616, &[], &zone);
        assert_eq!(
            fx.active.len(),
            1,
            "a later AOI incarnation must trust its ObjectPlayer state"
        );
        assert_eq!(fx.active[0].key, "magic-shield-1000");
        assert_eq!(fx.active[0].provenance.packet, "ObjectPlayer");
    }

    fn firebang_event(sequence: u64, cast: bool, target_id: u32) -> NativeEffectEvent {
        let mut event = talisman_event("FireBang", sequence, cast, target_id);
        event.payload["target"] = json!({"x":287,"y":622});
        event
    }

    #[test]
    fn firebang_empty_ground_completion_has_exact_frames_and_fixed_target() {
        let mut fx = NativeEffects::default();
        let zone = HashMap::from([(1000, (288, 616)), (2000, (287, 620))]);
        let event = firebang_event(1, true, 0);
        fx.observe(0, 288, 616, &[event.clone(), event], &zone);
        assert_eq!(
            fx.active.len(),
            2,
            "duplicate must not schedule two explosions"
        );
        let before: Value =
            serde_json::from_str(&fx.tick_with_visibility(599, true).unwrap()).unwrap();
        assert!(before["effects"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| !v["imageUrl"].as_str().unwrap().ends_with("/1660.png")));
        for (now, frame) in [(600, 1660), (1500, 1669)] {
            let state: Value =
                serde_json::from_str(&fx.tick_with_visibility(now, true).unwrap()).unwrap();
            assert_eq!(state["effects"].as_array().unwrap().len(), 1);
            assert!(state["effects"][0]["imageUrl"]
                .as_str()
                .unwrap()
                .ends_with(&format!("/{frame}.png")));
            assert_eq!((fx.active[0].tile_x, fx.active[0].tile_y), (287, 622));
        }
        let end: Value =
            serde_json::from_str(&fx.tick_with_visibility(1600, true).unwrap()).unwrap();
        assert_eq!(end["effects"], json!([]));
        assert!(fx.take_due_sound_events(1600).is_empty());

        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[firebang_event(1, true, 2000)], &zone);
        fx.observe(
            601,
            288,
            616,
            &[],
            &HashMap::from([(1000, (288, 616)), (2000, (280, 610))]),
        );
        fx.tick_with_visibility(601, true);
        assert_eq!((fx.active[0].tile_x, fx.active[0].tile_y), (287, 622));
        assert!(fx.anchor_object_ids.is_empty());
    }

    #[test]
    fn firebang_failed_cast_and_source_lifecycle_do_not_leak_completion() {
        let zone = HashMap::from([(1000, (288, 616))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[firebang_event(1, false, 0)], &zone);
        assert_eq!(fx.active.len(), 1);
        assert!(fx.prestart_source_object_ids.is_empty());
        let state: Value =
            serde_json::from_str(&fx.tick_with_visibility(600, true).unwrap()).unwrap();
        assert_eq!(state["effects"], json!([]));
        for removed_at in [100, 601] {
            let mut fx = NativeEffects::default();
            fx.observe(0, 288, 616, &[firebang_event(1, true, 0)], &zone);
            if removed_at > 600 {
                fx.tick_with_visibility(601, true);
            }
            fx.observe(
                removed_at,
                288,
                616,
                &[NativeEffectEvent {
                    generation: 42,
                    sequence: 2,
                    packet: "ObjectRemove".into(),
                    payload: json!({"objectId":1000}),
                }],
                &HashMap::new(),
            );
            fx.tick_with_visibility(700, true);
            assert_eq!(fx.active.len(), usize::from(removed_at > 600));
        }
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[firebang_event(1, true, 0)], &zone);
        let mut next_generation = firebang_event(1, false, 0);
        next_generation.generation = 43;
        fx.observe(100, 288, 616, &[next_generation], &zone);
        assert!(fx.prestart_source_object_ids.is_empty());
        let state: Value =
            serde_json::from_str(&fx.tick_with_visibility(700, true).unwrap()).unwrap();
        assert_eq!(
            state["effects"],
            json!([]),
            "new generation must retire old pending explosion"
        );
        for reset_connection in [false, true] {
            let mut fx = NativeEffects::default();
            fx.observe(0, 288, 616, &[firebang_event(1, true, 0)], &zone);
            if reset_connection {
                fx.reset_for_new_connection();
            } else {
                fx.clear_active_effects();
            }
            assert!(fx.active.is_empty());
            assert!(fx.prestart_source_object_ids.is_empty());
        }
    }

    fn talisman_event(spell: &str, sequence: u64, cast: bool, target_id: u32) -> NativeEffectEvent {
        NativeEffectEvent {
            sequence,
            generation: 42,
            packet: "ObjectMagic".into(),
            payload: json!({
                "objectId":1000,"spell":spell,"direction":"Up","location":{"x":288,"y":616},
                "targetId":target_id,"target":{"x":288,"y":611},"cast":cast,"level":3
            }),
        }
    }

    #[test]
    fn ground_talismans_launch_after_spell_and_complete_at_fixed_point_with_source_sounds() {
        let zone = HashMap::from([(1000, (288, 616))]);
        for (spell, impact_path, duration) in [
            ("MassHiding", Some("/Magic/1540.png"), 800),
            ("SoulShield", Some("/Magic/1320.png"), 1200),
            ("BlessedArmour", Some("/Magic/1340.png"), 1200),
            ("Curse", Some("/Magic2/950.png"), 1992),
            ("Plague", Some("/Magic3/110.png"), 1200),
            ("PoisonCloud", None, 0),
        ] {
            let mut fx = NativeEffects::default();
            fx.observe(0, 288, 616, &[talisman_event(spell, 1, true, 0)], &zone);
            assert_eq!(fx.active.len(), 1, "{spell}");
            assert!(fx.take_due_sound_events(0).is_empty());
            let before: Value =
                serde_json::from_str(&fx.tick_with_visibility(599, true).unwrap()).unwrap();
            assert_eq!(before["effects"], json!([]));
            let launch: Value =
                serde_json::from_str(&fx.tick_with_visibility(600, true).unwrap()).unwrap();
            assert!(launch["effects"][0]["imageUrl"]
                .as_str()
                .unwrap()
                .ends_with("/Magic/1160.png"));
            let (_, launch_sound) = ground_talisman(spell).unwrap();
            let sounds = fx.take_due_sound_events(600);
            assert_eq!(sounds.len(), usize::from(spell == "BlessedArmour"));
            if spell == "BlessedArmour" {
                assert_eq!(sounds[0].file_name, "M69-0.wav");
            }
            if let Some((cue, file)) = launch_sound {
                assert_eq!((&*sounds[0].cue, &*sounds[0].file_name), (cue, file));
            }
            assert!(fx.take_due_sound_events(849).is_empty());
            let impact: Value =
                serde_json::from_str(&fx.tick_with_visibility(850, true).unwrap()).unwrap();
            let sounds = fx.take_due_sound_events(850);
            assert!(
                sounds.is_empty(),
                "{spell} unregistered source arrival sound remains silent"
            );
            if let Some(path) = impact_path {
                assert!(impact["effects"][0]["imageUrl"]
                    .as_str()
                    .unwrap()
                    .ends_with(path));
                assert_eq!(fx.active[0].queued.as_ref().unwrap().duration_ms, duration);
                assert_eq!((fx.active[0].tile_x, fx.active[0].tile_y), (288, 611));
            } else {
                assert_eq!(impact["effects"], json!([]));
            }
            assert!(fx.take_due_sound_events(850).is_empty());
        }
    }

    #[test]
    fn ground_talismans_reject_failed_cast_and_compatibility_packets_and_reset_cleanly() {
        let zone = HashMap::from([(1000, (288, 616))]);
        for spell in [
            "MassHiding",
            "SoulShield",
            "BlessedArmour",
            "Curse",
            "Plague",
            "PoisonCloud",
        ] {
            let mut fx = NativeEffects::default();
            fx.observe(0, 288, 616, &[talisman_event(spell, 1, false, 0)], &zone);
            assert!(fx.active.is_empty());
            let compat = NativeEffectEvent {
                sequence: 2,
                generation: 42,
                packet: "ObjectProjectile".into(),
                payload: json!({"spell":spell,"sourceId":1000,"destinationId":0}),
            };
            fx.observe(1, 288, 616, &[compat], &zone);
            assert!(fx.active.is_empty());
            let event = talisman_event(spell, 3, true, 0);
            fx.observe(2, 288, 616, &[event.clone(), event], &zone);
            assert_eq!(fx.active.len(), 1);
            fx.reset_for_new_connection();
            assert!(fx.active.is_empty());
            assert!(fx.take_due_sound_events(2000).is_empty());
        }
    }

    #[test]
    fn ground_talismans_track_flight_but_impact_packet_point_even_when_target_dies() {
        let zone = HashMap::from([(1000, (288, 616)), (2041, (288, 610))]);
        let mut fx = NativeEffects::default();
        fx.observe(
            0,
            288,
            616,
            &[talisman_event("Curse", 1, true, 2041)],
            &zone,
        );
        fx.tick_with_visibility(600, true);
        assert_eq!(fx.active[0].current.as_ref().unwrap().duration_ms, 300);
        fx.replace_dead_action_object_ids(HashSet::from([2041]));
        fx.tick_with_visibility(900, true);
        assert_eq!((fx.active[0].tile_x, fx.active[0].tile_y), (288, 611));
        assert!(fx.active[0].queued.is_some());
        assert!(fx.anchor_object_ids.is_empty());
        assert!(fx.take_due_sound_events(900).is_empty());
    }

    #[test]
    fn ground_talismans_resolve_all_sixteen_directions() {
        let catalog = effect_catalog().as_ref().unwrap();
        for spell in [
            "MassHiding",
            "SoulShield",
            "BlessedArmour",
            "Curse",
            "Plague",
            "PoisonCloud",
        ] {
            assert!(catalog.spell_cast_animation(spell, 0).is_none());
            for direction in 0..16 {
                let anim = catalog
                    .spell_projectile_animation(spell, direction)
                    .unwrap();
                assert_eq!(anim.frames.len(), 3);
                assert!(anim.frames[0]
                    .path
                    .ends_with(&format!("/Magic/{}.png", 1160 + direction * 10)));
            }
        }
    }

    #[test]
    fn ground_talismans_keep_launched_target_reference_but_cancel_removed_caster() {
        let zone = HashMap::from([(1000, (288, 616)), (2041, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(
            0,
            288,
            616,
            &[talisman_event("Plague", 1, true, 2041)],
            &zone,
        );
        fx.tick_with_visibility(600, true);
        let removal = NativeEffectEvent {
            generation: 42,
            sequence: 2,
            packet: "ObjectRemove".into(),
            payload: json!({"objectId":2041}),
        };
        fx.observe(
            700,
            288,
            616,
            &[removal],
            &HashMap::from([(1000, (288, 616))]),
        );
        assert_eq!(fx.active.len(), 1);
        fx.tick_with_visibility(850, true);
        assert!(fx.active[0].queued.is_some());
        assert!(fx.take_due_sound_events(850).is_empty());

        let mut removed = NativeEffects::default();
        removed.observe(0, 288, 616, &[talisman_event("Plague", 1, true, 0)], &zone);
        let removal = NativeEffectEvent {
            generation: 42,
            sequence: 2,
            packet: "ObjectRemove".into(),
            payload: json!({"objectId":1000}),
        };
        removed.observe(100, 288, 616, &[removal], &HashMap::new());
        assert!(removed.active.is_empty());
        assert!(removed.take_due_sound_events(2000).is_empty());
    }

    #[test]
    fn new_move_destination_uses_crystal_magic3_frames_and_expires_at_600ms() {
        let mut effects = NativeEffects::default();
        let base = effect_catalog()
            .as_ref()
            .and_then(|catalog| catalog.map_animation("NewMoveDestination", 0))
            .expect("Crystal new-move effect");
        assert_eq!(base.frames.len(), 10);
        assert_eq!(base.interval, 60);
        assert!(base.frames[0].path.ends_with("/Magic3/500.png"));
        assert!(base.frames[9].path.ends_with("/Magic3/509.png"));

        assert!(effects.start_new_move_destination(1_000, (11, 10), (3.0, -2.0)));
        let instance = effects.active.last().expect("destination marker");
        assert_eq!(instance.kind, EffectKindTag::Ground);
        let animation = instance.current.as_ref().expect("marker animation");
        assert_eq!(animation.offset_x, base.offset_x + 3.0);
        assert_eq!(animation.offset_y, base.offset_y - 2.0);

        let first = effects.tick(1_000).expect("first marker frame");
        assert!(first.contains("/original-effects/Magic3/500.png"));
        let last = effects.tick(1_599).expect("last marker frame");
        assert!(last.contains("/original-effects/Magic3/509.png"));
        let expired = effects.tick(1_600).expect("expired marker state");
        assert!(expired.contains("\"effects\":[]"));
        assert!(effects.active.is_empty());
    }

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
            SCARECROW_ATTACK_SOUND_FILE,
            SCARECROW_FLINCH_SOUND_FILE,
            SCARECROW_DIE_SOUND_FILE,
            MONSTER_STRUCK_SHORT_FILE,
            MONSTER_STRUCK_WOODEN_FILE,
            MONSTER_STRUCK_SWORD_FILE,
            MONSTER_STRUCK_SWORD_2_FILE,
            MONSTER_STRUCK_AXE_FILE,
            MONSTER_STRUCK_CLUB_FILE,
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
            &[player_sound_event(1, "ObjectDied", scarecrow.clone(), None)],
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
    fn scarecrow_attack_audio_uses_exact_actor_context_and_repeats_per_action() {
        let event = |sequence| NativeEffectEvent {
            sequence,
            generation: 0,
            packet: "ObjectAttack".to_owned(),
            payload: json!({
                "objectId": 5030,
                "_nativeAttacker": monster_actor(5_030, "Monster/005"),
            }),
        };
        let mut fx = NativeEffects::default();
        fx.observe(1_000, 288, 616, &[event(1)], &HashMap::new());
        let first = fx.take_due_sound_events(1_000);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].file_name, SCARECROW_ATTACK_SOUND_FILE);
        assert_eq!(first[0].cue, "Scarecrow.5030.Attack");

        fx.observe(1_100, 288, 616, &[event(2)], &HashMap::new());
        let second = fx.take_due_sound_events(1_100);
        assert_eq!(
            second.len(),
            1,
            "a distinct Attack1 action must sound again"
        );
        assert_eq!(second[0].file_name, SCARECROW_ATTACK_SOUND_FILE);
    }

    #[test]
    fn scarecrow_attack_audio_is_exact_and_same_batch_lifecycle_scoped() {
        for attacker in [
            monster_actor(5_040, "Monster/004"),
            json!({
                "objectId": 5041,
                "kind": "player",
                "sprite": { "bodyLibrary": "Monster/005" },
            }),
            json!({
                "objectId": 5042,
                "kind": "monster",
                "name": "Scarecrow",
                "sprite": { "bodyLibrary": "Monster/006" },
            }),
        ] {
            let mut fx = NativeEffects::default();
            fx.observe(
                1_000,
                288,
                616,
                &[NativeEffectEvent {
                    sequence: 1,
                    generation: 0,
                    packet: "ObjectAttack".to_owned(),
                    payload: json!({"objectId": attacker["objectId"], "_nativeAttacker": attacker}),
                }],
                &HashMap::new(),
            );
            assert!(fx.take_due_sound_events(1_000).is_empty());
        }

        for lifecycle_packet in ["ObjectRemove", "ObjectHide", "MapChanged", "LogOutSuccess"] {
            let mut fx = NativeEffects::default();
            let events = [
                NativeEffectEvent {
                    sequence: 1,
                    generation: 0,
                    packet: "ObjectAttack".to_owned(),
                    payload: json!({
                        "objectId": 5050,
                        "_nativeAttacker": monster_actor(5_050, "Monster/005"),
                    }),
                },
                NativeEffectEvent {
                    sequence: 2,
                    generation: 0,
                    packet: lifecycle_packet.to_owned(),
                    payload: if matches!(lifecycle_packet, "ObjectRemove" | "ObjectHide") {
                        json!({"objectId": 5050})
                    } else {
                        json!({})
                    },
                },
            ];
            fx.observe(1_000, 288, 616, &events, &HashMap::new());
            assert!(
                fx.take_due_sound_events(1_000).is_empty(),
                "{lifecycle_packet} must cancel the same-batch Scarecrow attack cue"
            );
        }
    }

    #[test]
    fn scarecrow_struck_audio_preserves_flinch_then_crystal_weapon_family_order() {
        for (weapons, expected_file) in [
            (&[0, 23, 28, 40][..], MONSTER_STRUCK_WOODEN_FILE),
            (
                &[1, 6, 10, 12, 17, 22, 27, 30, 36, 39][..],
                MONSTER_STRUCK_SHORT_FILE,
            ),
            (
                &[2, 8, 11, 15, 18, 20, 25, 31, 33, 34, 37, 41][..],
                MONSTER_STRUCK_SWORD_FILE,
            ),
            (
                &[3, 5, 7, 9, 13, 19, 24, 26, 29, 32, 35][..],
                MONSTER_STRUCK_SWORD_2_FILE,
            ),
            (&[4, 14, 16, 38][..], MONSTER_STRUCK_AXE_FILE),
            (&[21][..], MONSTER_STRUCK_CLUB_FILE),
        ] {
            for weapon in weapons {
                assert_eq!(monster_struck_file_for_weapon(*weapon), Some(expected_file));
            }
        }
        for unknown in [-2, -1, 42, i32::MAX] {
            assert_eq!(monster_struck_file_for_weapon(unknown), None);
        }

        let cases = [
            ("warrior", "CWeapon/00", MONSTER_STRUCK_WOODEN_FILE),
            ("warrior", "CWeapon/01", MONSTER_STRUCK_SHORT_FILE),
            ("warrior", "CWeapon/02", MONSTER_STRUCK_SWORD_FILE),
            ("warrior", "CWeapon/03", MONSTER_STRUCK_SWORD_2_FILE),
            ("warrior", "CWeapon/04", MONSTER_STRUCK_AXE_FILE),
            ("warrior", "CWeapon/06", MONSTER_STRUCK_SHORT_FILE),
            ("warrior", "CWeapon/21", MONSTER_STRUCK_CLUB_FILE),
            ("assassin", "CWeapon/04", MONSTER_STRUCK_SHORT_FILE),
        ];

        for (index, (class, weapon, expected_struck)) in cases.into_iter().enumerate() {
            let target_id = 5_100 + index as u32;
            let target = monster_actor(target_id, "Monster/005");
            let attacker = player_actor(
                6_100 + index as u32,
                "male",
                class,
                "CArmour/00",
                Some(weapon),
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
                    target,
                    Some(attacker),
                )],
                &HashMap::new(),
            );
            let sounds = fx.take_due_sound_events(1_000);
            assert_eq!(sounds.len(), 2, "weapon case {weapon}");
            assert_eq!(sounds[0].file_name, SCARECROW_FLINCH_SOUND_FILE);
            assert_eq!(sounds[1].file_name, expected_struck);
        }

        let target = monster_actor(5_199, "Monster/005");
        let mut numeric_override = player_actor(
            6_199,
            "male",
            "warrior",
            "CArmour/00",
            Some("CWeapon/02"),
            None,
        );
        numeric_override["weapon"] = Value::from(4);
        let mut fx = NativeEffects::default();
        fx.observe(
            1_000,
            288,
            616,
            &[player_sound_event(
                1,
                "ObjectStruck",
                target,
                Some(numeric_override),
            )],
            &HashMap::new(),
        );
        let sounds = fx.take_due_sound_events(1_000);
        assert_eq!(sounds[0].file_name, SCARECROW_FLINCH_SOUND_FILE);
        assert_eq!(sounds[1].file_name, MONSTER_STRUCK_AXE_FILE);
    }

    #[test]
    fn scarecrow_struck_audio_dedupes_one_action_feed_tail_but_replays_after_consume() {
        let target = monster_actor(5_205, "Monster/005");
        let attacker = player_actor(
            6_205,
            "male",
            "warrior",
            "CArmour/00",
            Some("CWeapon/02"),
            None,
        );
        let first = player_sound_event(1, "ObjectStruck", target.clone(), Some(attacker.clone()));
        let duplicate_tail =
            player_sound_event(2, "ObjectStruck", target.clone(), Some(attacker.clone()));
        let mut fx = NativeEffects::default();
        fx.observe(
            1_000,
            288,
            616,
            &[first.clone(), duplicate_tail.clone()],
            &HashMap::new(),
        );
        assert_eq!(
            fx.take_due_sound_events(1_000)
                .into_iter()
                .map(|event| event.file_name)
                .collect::<Vec<_>>(),
            vec![
                SCARECROW_FLINCH_SOUND_FILE.to_owned(),
                MONSTER_STRUCK_SWORD_FILE.to_owned(),
            ]
        );

        // A later renderer observation represents Crystal consuming the
        // ActionFeed head. The next authoritative hit is a new reaction and
        // must not be suppressed by the previous batch's tail marker.
        let replay = player_sound_event(3, "ObjectStruck", target, Some(attacker));
        fx.observe(
            1_001,
            288,
            616,
            &[first, duplicate_tail, replay],
            &HashMap::new(),
        );
        assert_eq!(
            fx.take_due_sound_events(1_001)
                .into_iter()
                .map(|event| event.file_name)
                .collect::<Vec<_>>(),
            vec![
                SCARECROW_FLINCH_SOUND_FILE.to_owned(),
                MONSTER_STRUCK_SWORD_FILE.to_owned(),
            ]
        );

        for boundary in ["ObjectRemove", "ObjectHide", "MapChanged", "LogOutSuccess"] {
            let target = monster_actor(5_205, "Monster/005");
            let attacker = player_actor(
                6_205,
                "male",
                "warrior",
                "CArmour/00",
                Some("CWeapon/02"),
                None,
            );
            let events = [
                player_sound_event(1, "ObjectStruck", target.clone(), Some(attacker.clone())),
                NativeEffectEvent {
                    sequence: 2,
                    generation: 0,
                    packet: boundary.to_owned(),
                    payload: if matches!(boundary, "ObjectRemove" | "ObjectHide") {
                        json!({"objectId": 5205})
                    } else {
                        json!({})
                    },
                },
                player_sound_event(3, "ObjectStruck", target, Some(attacker)),
            ];
            let mut terminated = NativeEffects::default();
            terminated.observe(1_000, 288, 616, &events, &HashMap::new());
            assert!(
                terminated.take_due_sound_events(1_000).is_empty(),
                "{boundary} must cancel earlier cues and suppress stale later Struck"
            );
        }

        for boundary in ["MapChanged", "LogOutSuccess"] {
            let target = monster_actor(5_205, "Monster/005");
            let attacker = player_actor(
                6_205,
                "male",
                "warrior",
                "CArmour/00",
                Some("CWeapon/02"),
                None,
            );
            let events = [
                NativeEffectEvent {
                    sequence: 1,
                    generation: 0,
                    packet: boundary.to_owned(),
                    payload: json!({}),
                },
                player_sound_event(2, "ObjectStruck", target, Some(attacker)),
            ];
            let mut terminated = NativeEffects::default();
            terminated.observe(1_000, 288, 616, &events, &HashMap::new());
            assert!(
                terminated.take_due_sound_events(1_000).is_empty(),
                "{boundary} must suppress a stale later Struck"
            );
        }

        let target = monster_actor(5_205, "Monster/005");
        let attacker = player_actor(
            6_205,
            "male",
            "warrior",
            "CArmour/00",
            Some("CWeapon/02"),
            None,
        );
        let present = json!({"mapIndex": 1, "entities": [target.clone()]});
        let absent = json!({"mapIndex": 1, "entities": []});
        let mut removed = NativeEffects::default();
        removed.observe_render_payload(&present);
        removed.observe(900, 288, 616, &[], &HashMap::new());
        removed.observe_render_payload(&absent);
        let removal_events = [
            player_sound_event(1, "ObjectStruck", target.clone(), Some(attacker.clone())),
            NativeEffectEvent {
                sequence: 2,
                generation: 0,
                packet: "ObjectRemove".to_owned(),
                payload: json!({"objectId": 5205}),
            },
        ];
        removed.observe(1_000, 288, 616, &removal_events, &HashMap::new());
        assert!(removed.take_due_sound_events(1_000).is_empty());
        let stale_after_remove =
            player_sound_event(3, "ObjectStruck", target.clone(), Some(attacker.clone()));
        removed.observe(
            1_001,
            288,
            616,
            &[
                removal_events[0].clone(),
                removal_events[1].clone(),
                stale_after_remove.clone(),
            ],
            &HashMap::new(),
        );
        assert!(
            removed.take_due_sound_events(1_001).is_empty(),
            "an actor removal gate must survive across observe calls"
        );
        removed.observe_render_payload(&present);
        removed.observe(
            1_002,
            288,
            616,
            &[
                removal_events[0].clone(),
                removal_events[1].clone(),
                stale_after_remove,
                player_sound_event(4, "ObjectStruck", target.clone(), Some(attacker.clone())),
            ],
            &HashMap::new(),
        );
        assert_eq!(
            removed
                .take_due_sound_events(1_002)
                .into_iter()
                .map(|event| event.file_name)
                .collect::<Vec<_>>(),
            vec![
                SCARECROW_FLINCH_SOUND_FILE.to_owned(),
                MONSTER_STRUCK_SWORD_FILE.to_owned(),
            ],
            "a render-proven actor reappearance starts a fresh incarnation"
        );

        for boundary in ["MapChanged", "LogOutSuccess"] {
            let mut terminated = NativeEffects::default();
            terminated.observe_render_payload(&present);
            terminated.observe(900, 288, 616, &[], &HashMap::new());
            let boundary_event = NativeEffectEvent {
                sequence: 1,
                generation: 0,
                packet: boundary.to_owned(),
                payload: if boundary == "MapChanged" {
                    json!({"mapIndex": 2})
                } else {
                    json!({})
                },
            };
            terminated.observe(1_000, 288, 616, &[boundary_event.clone()], &HashMap::new());
            let stale =
                player_sound_event(2, "ObjectStruck", target.clone(), Some(attacker.clone()));
            terminated.observe(
                1_001,
                288,
                616,
                &[boundary_event.clone(), stale.clone()],
                &HashMap::new(),
            );
            assert!(
                terminated.take_due_sound_events(1_001).is_empty(),
                "{boundary} must remain terminal across observe calls"
            );

            if boundary == "MapChanged" {
                terminated
                    .observe_render_payload(&json!({"mapIndex": 2, "entities": [target.clone()]}));
                terminated.observe(
                    1_002,
                    288,
                    616,
                    &[
                        boundary_event,
                        stale,
                        player_sound_event(
                            3,
                            "ObjectStruck",
                            target.clone(),
                            Some(attacker.clone()),
                        ),
                    ],
                    &HashMap::new(),
                );
                assert_eq!(
                    terminated
                        .take_due_sound_events(1_002)
                        .into_iter()
                        .map(|event| event.file_name)
                        .collect::<Vec<_>>(),
                    vec![
                        SCARECROW_FLINCH_SOUND_FILE.to_owned(),
                        MONSTER_STRUCK_SWORD_FILE.to_owned(),
                    ],
                    "a changed render-scene identity reopens the new map"
                );
            }
        }

        let mut boundary_batch = NativeEffects::default();
        boundary_batch.observe_render_payload(&present);
        boundary_batch.observe(900, 288, 616, &[], &HashMap::new());
        boundary_batch
            .observe_render_payload(&json!({"mapIndex": 2, "entities": [target.clone()]}));
        let map_changed = NativeEffectEvent {
            sequence: 1,
            generation: 0,
            packet: "MapChanged".to_owned(),
            payload: json!({"mapIndex": 2}),
        };
        let stale_in_boundary_batch =
            player_sound_event(2, "ObjectStruck", target.clone(), Some(attacker.clone()));
        boundary_batch.observe(
            1_000,
            288,
            616,
            &[map_changed.clone(), stale_in_boundary_batch.clone()],
            &HashMap::new(),
        );
        assert!(
            boundary_batch.take_due_sound_events(1_000).is_empty(),
            "a changed render payload cannot reopen audio inside its MapChanged batch"
        );
        boundary_batch.observe(
            1_001,
            288,
            616,
            &[
                map_changed,
                stale_in_boundary_batch,
                player_sound_event(3, "ObjectStruck", target.clone(), Some(attacker.clone())),
            ],
            &HashMap::new(),
        );
        assert_eq!(
            boundary_batch
                .take_due_sound_events(1_001)
                .into_iter()
                .map(|event| event.file_name)
                .collect::<Vec<_>>(),
            vec![
                SCARECROW_FLINCH_SOUND_FILE.to_owned(),
                MONSTER_STRUCK_SWORD_FILE.to_owned(),
            ],
            "the consumed boundary reopens audio only for the next batch"
        );

        let target = monster_actor(5_205, "Monster/005");
        let attacker = player_actor(
            6_205,
            "male",
            "warrior",
            "CArmour/00",
            Some("CWeapon/02"),
            None,
        );
        let events = [
            NativeEffectEvent {
                sequence: 1,
                generation: 0,
                packet: "LogOutSuccess".to_owned(),
                payload: json!({}),
            },
            NativeEffectEvent {
                generation: 1,
                ..player_sound_event(1, "ObjectStruck", target, Some(attacker))
            },
        ];
        let mut next_generation = NativeEffects::default();
        next_generation.observe(1_000, 288, 616, &events, &HashMap::new());
        assert_eq!(
            next_generation
                .take_due_sound_events(1_000)
                .into_iter()
                .map(|event| event.file_name)
                .collect::<Vec<_>>(),
            vec![
                SCARECROW_FLINCH_SOUND_FILE.to_owned(),
                MONSTER_STRUCK_SWORD_FILE.to_owned(),
            ],
            "a new connection generation starts a fresh scene boundary"
        );
    }

    #[test]
    fn scarecrow_struck_unknown_attacker_lifecycle_and_lethal_order_are_fail_closed() {
        let target = monster_actor(5_200, "/original-ui/Monster/005");
        for attacker in [
            None,
            Some(monster_actor(6_200, "Monster/004")),
            Some(player_actor(
                6_201,
                "male",
                "warrior",
                "CArmour/00",
                None,
                None,
            )),
        ] {
            let mut unknown = NativeEffects::default();
            unknown.observe(
                1_000,
                288,
                616,
                &[player_sound_event(
                    1,
                    "ObjectStruck",
                    target.clone(),
                    attacker,
                )],
                &HashMap::new(),
            );
            let sounds = unknown.take_due_sound_events(1_000);
            assert_eq!(sounds.len(), 1);
            assert_eq!(sounds[0].file_name, SCARECROW_FLINCH_SOUND_FILE);
        }

        for lifecycle_packet in ["ObjectRemove", "ObjectHide", "MapChanged", "LogOutSuccess"] {
            let attacker = player_actor(
                6_200,
                "male",
                "warrior",
                "CArmour/00",
                Some("CWeapon/02"),
                None,
            );
            let events = [
                player_sound_event(1, "ObjectStruck", target.clone(), Some(attacker)),
                NativeEffectEvent {
                    sequence: 2,
                    generation: 0,
                    packet: lifecycle_packet.to_owned(),
                    payload: if matches!(lifecycle_packet, "ObjectRemove" | "ObjectHide") {
                        json!({"objectId": 5200})
                    } else {
                        json!({})
                    },
                },
            ];
            let mut fx = NativeEffects::default();
            fx.observe(1_000, 288, 616, &events, &HashMap::new());
            assert!(
                fx.take_due_sound_events(1_000).is_empty(),
                "{lifecycle_packet} must cancel both Scarecrow Struck cues"
            );
        }

        let attacker = player_actor(
            6_201,
            "male",
            "warrior",
            "CArmour/00",
            Some("CWeapon/02"),
            None,
        );
        let lethal = [
            player_sound_event(1, "ObjectStruck", target.clone(), Some(attacker)),
            player_sound_event(2, "ObjectDied", target, None),
        ];
        let mut fx = NativeEffects::default();
        fx.observe(1_000, 288, 616, &lethal, &HashMap::new());
        let sounds = fx.take_due_sound_events(1_000);
        assert_eq!(sounds.len(), 3);
        assert_eq!(sounds[0].file_name, SCARECROW_FLINCH_SOUND_FILE);
        assert_eq!(sounds[1].file_name, MONSTER_STRUCK_SWORD_FILE);
        assert_eq!(sounds[2].file_name, SCARECROW_DIE_SOUND_FILE);
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

        for lifecycle_packet in ["ObjectRemove", "ObjectHide", "MapChanged", "LogOutSuccess"] {
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

        for phase in 0..10_u64 {
            let rendered = fx
                .tick(phase * 100)
                .expect("TrapHexagon should render every Crystal phase");
            let rendered: Value = serde_json::from_str(&rendered).expect("effect render JSON");
            let effect = &rendered["effects"][0];
            assert!(effect["imageUrl"]
                .as_str()
                .is_some_and(|path| path.ends_with(&format!("/Magic/{}.png", 1390 + phase))));
            assert_eq!(effect["additive"], true);
            assert!((effect["opacity"].as_f64().unwrap_or_default() - 0.8).abs() < 1e-6);
        }
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
            frame_asset_root: exported_effect_test_root(),
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

    fn fire_bounce_fixture() -> Value {
        serde_json::from_str(include_str!(
            "../tests/fixtures/vis02-bichon-fire-bounce-v1.json"
        ))
        .expect("VIS-02 FireBounce fixture JSON")
    }

    fn fire_bounce_magic_event(sequence: u64, cast: bool) -> NativeEffectEvent {
        let fixture = fire_bounce_fixture();
        let index = if cast { 0 } else { 2 };
        NativeEffectEvent {
            sequence,
            generation: 29,
            packet: "ObjectMagic".to_owned(),
            payload: fixture["timeline"][index]["event"]["payload"].clone(),
        }
    }

    fn fire_bounce_legacy_compat_projectile_event(sequence: u64) -> NativeEffectEvent {
        NativeEffectEvent {
            sequence,
            generation: 29,
            packet: "ObjectProjectile".to_owned(),
            payload: fire_bounce_fixture()["timeline"][0]["legacyCompatibilityEvent"]["payload"]
                .clone(),
        }
    }

    fn fire_bounce_hop_event(sequence: u64) -> NativeEffectEvent {
        NativeEffectEvent {
            sequence,
            generation: 29,
            packet: "ObjectProjectile".to_owned(),
            payload: fire_bounce_fixture()["timeline"][1]["event"]["payload"].clone(),
        }
    }

    #[test]
    fn fire_bounce_object_magic_owns_delayed_first_projectile_and_legacy_dedupe() {
        let zone = HashMap::from([(1000, (288, 616)), (2154, (288, 611)), (2155, (290, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(
            0,
            288,
            616,
            &[
                fire_bounce_magic_event(1, true),
                fire_bounce_legacy_compat_projectile_event(2),
            ],
            &zone,
        );

        assert_eq!(
            fx.active.len(),
            2,
            "legacy player-to-primary ObjectProjectile must not duplicate the local first leg"
        );
        let cast = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-cast-"))
            .expect("FireBounce cast");
        assert!(cast
            .current
            .as_ref()
            .is_some_and(|animation| animation.frames[0].path.ends_with("/Magic/400.png")));
        let projectile = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-fire-bounce-"))
            .expect("FireBounce first projectile");
        assert_eq!(projectile.start_at, FIRE_BOUNCE_SPELL_ACTION_MS);
        assert_eq!(
            projectile
                .current
                .as_ref()
                .map(|animation| animation.duration_ms),
            Some(250)
        );
        assert!(projectile
            .current
            .as_ref()
            .is_some_and(|animation| animation.frames[0].path.ends_with("/Magic/410.png")));

        let cast_sound = fx.take_due_sound_events(0);
        assert_eq!(cast_sound.len(), 1);
        assert_eq!(cast_sound[0].cue, FIRE_BOUNCE_CAST_SOUND_CUE);
        assert!(fx.take_due_sound_events(599).is_empty());
        let projectile_sound = fx.take_due_sound_events(600);
        assert_eq!(projectile_sound.len(), 1);
        assert_eq!(projectile_sound[0].cue, FIRE_BOUNCE_PROJECTILE_SOUND_CUE);

        let launch: Value = serde_json::from_str(
            &fx.tick_with_visibility(600, true)
                .expect("FireBounce first projectile launch"),
        )
        .expect("FireBounce first launch JSON");
        assert!(launch["effects"].as_array().is_some_and(|effects| {
            effects.iter().any(|effect| {
                effect["imageUrl"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("/Magic/410.png"))
            })
        }));
        assert!(fx.active.iter().any(|instance| {
            instance.key.starts_with("fx-fire-bounce-")
                && instance
                    .queued
                    .as_ref()
                    .is_some_and(|animation| animation.frames[0].path.ends_with("/Magic/570.png"))
        }));
        assert!(fx.take_due_sound_events(849).is_empty());
        let impact_sound = fx.take_due_sound_events(850);
        assert_eq!(impact_sound.len(), 1);
        assert_eq!(impact_sound[0].cue, FIRE_BOUNCE_IMPACT_SOUND_CUE);
        let impact: Value = serde_json::from_str(
            &fx.tick_with_visibility(850, true)
                .expect("FireBounce first impact"),
        )
        .expect("FireBounce first impact JSON");
        assert!(impact["effects"].as_array().is_some_and(|effects| {
            effects.iter().any(|effect| {
                effect["imageUrl"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("/Magic/570.png"))
            })
        }));
    }

    #[test]
    fn fire_bounce_authoritative_hop_uses_source_direction_and_crystal_clock() {
        let zone = HashMap::from([(2154, (288, 611)), (2155, (290, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[fire_bounce_hop_event(1)], &zone);

        assert_eq!(fx.active.len(), 1);
        let hop = &fx.active[0];
        assert!(hop.key.starts_with("fx-fire-bounce-hop-"));
        let projectile = hop.current.as_ref().expect("FireBounce hop projectile");
        assert_eq!(projectile.duration_ms, 100);
        assert_eq!(projectile.interval, 33);
        assert!(projectile.frames[0].path.ends_with("/Magic/450.png"));
        assert!(hop
            .queued
            .as_ref()
            .is_some_and(|animation| animation.frames[0].path.ends_with("/Magic/570.png")));

        let launch_sound = fx.take_due_sound_events(0);
        assert_eq!(launch_sound.len(), 1);
        assert_eq!(launch_sound[0].cue, FIRE_BOUNCE_PROJECTILE_SOUND_CUE);
        assert!(fx.take_due_sound_events(99).is_empty());
        let impact_sound = fx.take_due_sound_events(100);
        assert_eq!(impact_sound.len(), 1);
        assert_eq!(impact_sound[0].cue, FIRE_BOUNCE_IMPACT_SOUND_CUE);
        let impact: Value = serde_json::from_str(
            &fx.tick_with_visibility(100, true)
                .expect("FireBounce hop impact"),
        )
        .expect("FireBounce hop impact JSON");
        assert!(impact["effects"][0]["imageUrl"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Magic/570.png")));
    }

    #[test]
    fn fire_bounce_cast_false_keeps_cast_only_and_boundaries_clear_chain() {
        let zone = HashMap::from([(1000, (288, 616)), (2154, (288, 611)), (2155, (290, 611))]);
        let mut cast_false = NativeEffects::default();
        cast_false.observe(0, 288, 616, &[fire_bounce_magic_event(1, false)], &zone);
        assert_eq!(cast_false.active.len(), 1);
        assert!(cast_false.active[0].key.starts_with("fx-cast-"));
        let sounds = cast_false.take_due_sound_events(0);
        assert_eq!(sounds.len(), 1);
        assert_eq!(sounds[0].cue, FIRE_BOUNCE_CAST_SOUND_CUE);
        assert!(cast_false.take_due_sound_events(10_000).is_empty());

        for boundary in ["MapChanged", "LogOutSuccess"] {
            let mut fx = NativeEffects::default();
            fx.observe(
                0,
                288,
                616,
                &[fire_bounce_magic_event(1, true), fire_bounce_hop_event(2)],
                &zone,
            );
            fx.observe(
                10,
                288,
                616,
                &[NativeEffectEvent {
                    sequence: 3,
                    generation: 29,
                    packet: boundary.to_owned(),
                    payload: json!({}),
                }],
                &zone,
            );
            assert!(fx.active.is_empty(), "{boundary} clears FireBounce");
            assert!(fx.local_projectile_targets.is_empty());
            assert!(fx.local_projectile_dedupe.is_empty());
            assert!(fx.take_due_sound_events(10_000).is_empty());
        }

        let mut reset = NativeEffects::default();
        reset.observe(0, 288, 616, &[fire_bounce_magic_event(1, true)], &zone);
        reset.reset_session();
        assert!(reset.active.is_empty());
        assert!(reset.local_projectile_targets.is_empty());
        assert!(reset.local_projectile_dedupe.is_empty());
        assert!(reset.pending_sounds.is_empty());
    }

    #[test]
    fn fire_bounce_completion_suppresses_only_terminal_dead_target() {
        let zone = HashMap::from([(2154, (288, 611)), (2155, (290, 611))]);

        let mut dying = NativeEffects::default();
        dying.observe(0, 288, 616, &[fire_bounce_hop_event(1)], &zone);
        let _ = dying.take_due_sound_events(0);
        dying.replace_dead_action_object_ids(HashSet::new());
        assert_eq!(
            dying.take_due_sound_events(100)[0].cue,
            FIRE_BOUNCE_IMPACT_SOUND_CUE,
            "Crystal's Die phase retains the FireBounce impact"
        );
        let dying_impact: Value = serde_json::from_str(
            &dying
                .tick_with_visibility(100, true)
                .expect("FireBounce Die-phase impact"),
        )
        .expect("FireBounce Die-phase impact JSON");
        assert!(dying_impact["effects"][0]["imageUrl"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Magic/570.png")));

        let mut dead = NativeEffects::default();
        dead.observe(0, 288, 616, &[fire_bounce_hop_event(1)], &zone);
        let _ = dead.take_due_sound_events(0);
        dead.replace_dead_action_object_ids(HashSet::from([2155]));
        assert!(dead.take_due_sound_events(100).is_empty());
        let suppressed: Value = serde_json::from_str(
            &dead
                .tick_with_visibility(100, true)
                .expect("FireBounce Dead-phase suppression"),
        )
        .expect("FireBounce Dead-phase JSON");
        assert_eq!(suppressed["effects"], json!([]));

        let mut revived = NativeEffects::default();
        revived.observe(0, 288, 616, &[fire_bounce_hop_event(1)], &zone);
        let _ = revived.take_due_sound_events(0);
        revived.replace_dead_action_object_ids(HashSet::from([2155]));
        let _ = revived.tick_with_visibility(50, true);
        revived.replace_dead_action_object_ids(HashSet::new());
        assert_eq!(
            revived.take_due_sound_events(100)[0].cue,
            FIRE_BOUNCE_IMPACT_SOUND_CUE
        );
        let restored: Value = serde_json::from_str(
            &revived
                .tick_with_visibility(100, true)
                .expect("FireBounce revived-target impact"),
        )
        .expect("FireBounce revived-target JSON");
        assert!(restored["effects"][0]["imageUrl"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Magic/570.png")));
    }

    #[test]
    fn fire_bounce_production_frames_and_audio_are_integrity_closed() {
        use sha2::{Digest, Sha256};

        let fixture = fire_bounce_fixture();
        assert_eq!(fixture["source"]["actorActionDurationMs"], json!(600));
        assert_eq!(
            fixture["source"]["authoritativeFirstHitBaseDelayMs"],
            json!(500)
        );
        let catalog = EffectCatalog::load().expect("production effect catalog");
        let cast = catalog
            .spell_cast_animation("FireBounce", 0)
            .expect("FireBounce cast");
        assert_eq!(cast.frames.len(), 10);
        for (frame, index) in cast.frames.iter().zip(400..410) {
            assert!(frame.path.ends_with(&format!("/Magic/{index}.png")));
            assert!(crate::frame_png_exists(&frame.path));
        }
        for direction in 0..16_u32 {
            let projectile = catalog
                .spell_projectile_animation("FireBounce", direction)
                .expect("all FireBounce directions resolve");
            assert_eq!(projectile.frames.len(), 6);
            for (frame, source) in projectile.frames.iter().zip(0..6_u32) {
                let index = 410 + direction * 10 + source;
                assert!(frame.path.ends_with(&format!("/Magic/{index}.png")));
                assert!(crate::frame_png_exists(&frame.path));
            }
        }
        let impact = catalog
            .spell_impact_animation("FireBounce")
            .expect("FireBounce impact");
        assert_eq!(impact.frames.len(), 10);
        for (frame, index) in impact.frames.iter().zip(570..580) {
            assert!(frame.path.ends_with(&format!("/Magic/{index}.png")));
            assert!(crate::frame_png_exists(&frame.path));
        }
        for audio in fixture["source"]["audio"]
            .as_array()
            .expect("FireBounce audio catalog")
        {
            let file = audio["file"].as_str().expect("FireBounce audio file");
            let path = assets::asset_path(&format!("original-ui/Sound/{file}"))
                .expect("packaged FireBounce sound path");
            let bytes = fs::read(path).expect("read FireBounce sound");
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

    fn frost_crunch_magic_event(sequence: u64, cast: bool) -> NativeEffectEvent {
        NativeEffectEvent {
            sequence,
            generation: 19,
            packet: "ObjectMagic".to_owned(),
            payload: json!({
                "objectId": 1000,
                "spell": "FrostCrunch",
                "cast": cast,
                "direction": "Up",
                "location": {"x": 288, "y": 616},
                "targetId": 2041,
                "target": {"x": 288, "y": 611}
            }),
        }
    }

    fn frost_crunch_compat_projectile_event(sequence: u64) -> NativeEffectEvent {
        NativeEffectEvent {
            sequence,
            generation: 19,
            packet: "ObjectProjectile".to_owned(),
            payload: json!({
                "spell": "FrostCrunch",
                "sourceId": 1000,
                "destinationId": 2041
            }),
        }
    }

    fn hallucination_fixture() -> Value {
        serde_json::from_str(include_str!(
            "../tests/fixtures/vis02-bichon-hallucination-v1.json"
        ))
        .expect("VIS-02 Hallucination fixture JSON")
    }

    fn hallucination_magic_event(sequence: u64, cast: bool) -> NativeEffectEvent {
        let fixture = hallucination_fixture();
        let payload = if cast {
            fixture["timeline"][0]["event"]["payload"].clone()
        } else {
            fixture["timeline"][2]["event"]["payload"].clone()
        };
        NativeEffectEvent {
            sequence,
            generation: 23,
            packet: "ObjectMagic".to_owned(),
            payload,
        }
    }

    fn hallucination_compat_projectile_event(sequence: u64) -> NativeEffectEvent {
        let fixture = hallucination_fixture();
        NativeEffectEvent {
            sequence,
            generation: 23,
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
    fn fireball_family_suppresses_impact_and_sound_only_for_dead_action_at_completion() {
        for (event, target_id, impact_cue, impact_frame) in [
            (
                fireball_magic_event(1, true),
                2005,
                FIREBALL_IMPACT_SOUND_CUE,
                "/Magic/170.png",
            ),
            (
                great_fireball_magic_event(1, true),
                2034,
                GREAT_FIREBALL_IMPACT_SOUND_CUE,
                "/Magic/570.png",
            ),
            (
                soul_fireball_magic_event(1, true),
                2014,
                SOUL_FIREBALL_IMPACT_SOUND_CUE,
                "/Magic/1360.png",
            ),
        ] {
            let zone = HashMap::from([(1000, (288, 616)), (target_id, (288, 611))]);

            let mut dying = NativeEffects::default();
            dying.observe(0, 288, 616, &[event.clone()], &zone);
            let _ = dying.take_due_sound_events(0);
            dying.observe(600, 288, 616, &[], &zone);
            let _ = dying.take_due_sound_events(600);
            dying.replace_dead_action_object_ids(HashSet::new());
            let impact_sound = dying.take_due_sound_events(850);
            assert_eq!(
                impact_sound
                    .iter()
                    .map(|sound| sound.cue.as_str())
                    .collect::<Vec<_>>(),
                vec![impact_cue],
                "Die must retain the {impact_cue} completion sound"
            );
            let impact: Value = serde_json::from_str(
                &dying
                    .tick_with_visibility(850, true)
                    .expect("Die-phase impact render state"),
            )
            .expect("Die-phase impact JSON");
            assert!(impact["effects"].as_array().is_some_and(|effects| {
                effects.iter().any(|effect| {
                    effect["imageUrl"]
                        .as_str()
                        .is_some_and(|path| path.ends_with(impact_frame))
                })
            }));

            let mut dead = NativeEffects::default();
            dead.observe(0, 288, 616, &[event], &zone);
            let _ = dead.take_due_sound_events(0);
            dead.observe(600, 288, 616, &[], &zone);
            let _ = dead.take_due_sound_events(600);
            dead.replace_dead_action_object_ids(HashSet::from([target_id]));
            assert!(
                dead.take_due_sound_events(850).is_empty(),
                "Dead must suppress the {impact_cue} completion sound"
            );
            let suppressed: Value = serde_json::from_str(
                &dead
                    .tick_with_visibility(850, true)
                    .expect("Dead-phase suppression render state"),
            )
            .expect("Dead-phase suppression JSON");
            assert_eq!(suppressed["effects"], json!([]));
        }
    }

    #[test]
    fn fireball_target_revived_before_completion_restores_impact() {
        let zone = HashMap::from([(1000, (288, 616)), (2005, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[fireball_magic_event(1, true)], &zone);
        let _ = fx.take_due_sound_events(0);
        fx.observe(600, 288, 616, &[], &zone);
        let _ = fx.take_due_sound_events(600);

        fx.replace_dead_action_object_ids(HashSet::from([2005]));
        let _ = fx.tick_with_visibility(700, true);
        assert!(fx.active.iter().any(|instance| {
            instance.key.starts_with("fx-fireball-") && instance.queued.is_some()
        }));

        fx.replace_dead_action_object_ids(HashSet::new());
        assert_eq!(
            fx.take_due_sound_events(850)[0].cue,
            FIREBALL_IMPACT_SOUND_CUE
        );
        let impact: Value = serde_json::from_str(
            &fx.tick_with_visibility(850, true)
                .expect("revived target impact"),
        )
        .expect("revived target impact JSON");
        assert!(impact["effects"][0]["imageUrl"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Magic/170.png")));
    }

    #[test]
    fn frost_crunch_object_magic_owns_cast_projectile_impact_and_two_sounds() {
        let zone = HashMap::from([(1000, (288, 616)), (2041, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(
            0,
            288,
            616,
            &[
                frost_crunch_magic_event(1, true),
                frost_crunch_compat_projectile_event(2),
            ],
            &zone,
        );
        assert_eq!(
            fx.active.len(),
            2,
            "compatibility packet must not duplicate the local FrostCrunch missile"
        );
        let cast = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-cast-"))
            .expect("FrostCrunch cast");
        assert_eq!(cast.start_at, 0);
        assert!(cast
            .current
            .as_ref()
            .is_some_and(|animation| animation.frames[0].path.ends_with("/Magic2/400.png")));
        let projectile = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-frost-crunch-"))
            .expect("FrostCrunch projectile");
        assert_eq!(projectile.start_at, FROST_CRUNCH_SPELL_ACTION_MS);
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
            .is_some_and(|animation| animation.frames[0].path.ends_with("/Magic2/410.png")));
        assert!(
            projectile.queued.is_none(),
            "target binding occurs at launch"
        );

        let cast_sound = fx.take_due_sound_events(0);
        assert_eq!(cast_sound.len(), 1);
        assert_eq!(cast_sound[0].cue, FROST_CRUNCH_CAST_SOUND_CUE);
        assert_eq!(cast_sound[0].file_name, FROST_CRUNCH_CAST_SOUND_FILE);
        assert!(fx.take_due_sound_events(599).is_empty());

        let before: Value = serde_json::from_str(
            &fx.tick_with_visibility(599, true)
                .expect("FrostCrunch before projectile"),
        )
        .expect("FrostCrunch pre-projectile JSON");
        assert_eq!(before["effects"].as_array().map(Vec::len), Some(1));

        let launch: Value = serde_json::from_str(
            &fx.tick_with_visibility(600, true)
                .expect("FrostCrunch projectile launch"),
        )
        .expect("FrostCrunch launch JSON");
        assert!(launch["effects"][0]["imageUrl"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Magic2/410.png")));
        assert!(fx.take_due_sound_events(600).is_empty());
        assert!(fx.active.iter().any(|instance| {
            instance.key.starts_with("fx-frost-crunch-")
                && instance
                    .queued
                    .as_ref()
                    .is_some_and(|animation| animation.frames[0].path.ends_with("/Magic2/570.png"))
        }));
        assert!(fx.take_due_sound_events(849).is_empty());

        let impact_sound = fx.take_due_sound_events(850);
        assert_eq!(impact_sound.len(), 1);
        assert_eq!(impact_sound[0].cue, FROST_CRUNCH_IMPACT_SOUND_CUE);
        assert_eq!(impact_sound[0].file_name, FROST_CRUNCH_IMPACT_SOUND_FILE);
        let impact: Value = serde_json::from_str(
            &fx.tick_with_visibility(850, true)
                .expect("FrostCrunch impact"),
        )
        .expect("FrostCrunch impact JSON");
        assert!(impact["effects"][0]["imageUrl"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Magic2/570.png")));
        assert!(fx.take_due_sound_events(850).is_empty());
    }

    #[test]
    fn frost_crunch_suppresses_impact_and_sound_only_for_dead_action_at_completion() {
        let zone = HashMap::from([(1000, (288, 616)), (2041, (288, 611))]);

        let mut dying = NativeEffects::default();
        dying.observe(0, 288, 616, &[frost_crunch_magic_event(1, true)], &zone);
        let _ = dying.take_due_sound_events(0);
        dying.observe(600, 288, 616, &[], &zone);
        assert!(dying.take_due_sound_events(600).is_empty());
        dying.replace_dead_action_object_ids(HashSet::new());
        let impact_sound = dying.take_due_sound_events(850);
        assert_eq!(
            impact_sound
                .iter()
                .map(|sound| sound.cue.as_str())
                .collect::<Vec<_>>(),
            vec![FROST_CRUNCH_IMPACT_SOUND_CUE],
            "Die must retain the FrostCrunch completion sound"
        );
        let impact: Value = serde_json::from_str(
            &dying
                .tick_with_visibility(850, true)
                .expect("FrostCrunch Die-phase impact"),
        )
        .expect("FrostCrunch Die-phase JSON");
        assert!(impact["effects"].as_array().is_some_and(|effects| {
            effects.iter().any(|effect| {
                effect["imageUrl"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("/Magic2/570.png"))
            })
        }));

        let mut dead = NativeEffects::default();
        dead.observe(0, 288, 616, &[frost_crunch_magic_event(1, true)], &zone);
        let _ = dead.take_due_sound_events(0);
        dead.observe(600, 288, 616, &[], &zone);
        assert!(dead.take_due_sound_events(600).is_empty());
        dead.replace_dead_action_object_ids(HashSet::from([2041]));
        assert!(
            dead.take_due_sound_events(850).is_empty(),
            "Dead must suppress the FrostCrunch completion sound"
        );
        let suppressed: Value = serde_json::from_str(
            &dead
                .tick_with_visibility(850, true)
                .expect("FrostCrunch Dead-phase suppression"),
        )
        .expect("FrostCrunch Dead-phase JSON");
        assert_eq!(suppressed["effects"], json!([]));
    }

    #[test]
    fn frost_crunch_target_revived_before_completion_restores_impact() {
        let zone = HashMap::from([(1000, (288, 616)), (2041, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[frost_crunch_magic_event(1, true)], &zone);
        let _ = fx.take_due_sound_events(0);
        fx.observe(600, 288, 616, &[], &zone);
        assert!(fx.take_due_sound_events(600).is_empty());

        fx.replace_dead_action_object_ids(HashSet::from([2041]));
        let _ = fx.tick_with_visibility(700, true);
        assert!(fx.active.iter().any(|instance| {
            instance.key.starts_with("fx-frost-crunch-") && instance.queued.is_some()
        }));

        fx.replace_dead_action_object_ids(HashSet::new());
        assert_eq!(
            fx.take_due_sound_events(850)[0].cue,
            FROST_CRUNCH_IMPACT_SOUND_CUE
        );
        let impact: Value = serde_json::from_str(
            &fx.tick_with_visibility(850, true)
                .expect("FrostCrunch revived target impact"),
        )
        .expect("FrostCrunch revived target JSON");
        assert!(impact["effects"][0]["imageUrl"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Magic2/570.png")));
    }

    #[test]
    fn frost_crunch_target_removal_map_logout_and_reset_clear_delayed_impact() {
        let initial = HashMap::from([(1000, (288, 616)), (2041, (288, 611))]);

        let mut removal = NativeEffects::default();
        removal.observe(0, 288, 616, &[frost_crunch_magic_event(1, true)], &initial);
        let _ = removal.take_due_sound_events(0);
        removal.observe(600, 288, 616, &[], &HashMap::from([(1000, (288, 616))]));
        assert!(removal.take_due_sound_events(600).is_empty());
        let projectile = removal
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-frost-crunch-"))
            .expect("point-flight FrostCrunch");
        assert!(projectile.queued.is_none());
        assert!(!removal.anchor_object_ids.contains_key(&projectile.key));
        assert!(removal.take_due_sound_events(10_000).is_empty());

        for boundary in ["MapChanged", "LogOutSuccess"] {
            let mut fx = NativeEffects::default();
            fx.observe(0, 288, 616, &[frost_crunch_magic_event(1, true)], &initial);
            let _ = fx.take_due_sound_events(0);
            fx.observe(
                100,
                288,
                616,
                &[NativeEffectEvent {
                    sequence: 2,
                    generation: 19,
                    packet: boundary.to_owned(),
                    payload: json!({}),
                }],
                &initial,
            );
            assert!(
                fx.active.is_empty(),
                "{boundary} clears active FrostCrunch state"
            );
            assert!(
                fx.take_due_sound_events(10_000).is_empty(),
                "{boundary} clears delayed sounds"
            );
        }

        let mut reset = NativeEffects::default();
        reset.observe(0, 288, 616, &[frost_crunch_magic_event(1, true)], &initial);
        let _ = reset.take_due_sound_events(0);
        reset.reset_session();
        assert!(reset.active.is_empty());
        assert!(reset.local_projectile_targets.is_empty());
        assert!(reset.pending_sounds.is_empty());
    }

    #[test]
    fn hallucination_object_magic_owns_delayed_directional_projectile_and_impact_sound() {
        let zone = HashMap::from([(1000, (288, 616)), (2076, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(
            0,
            288,
            616,
            &[
                hallucination_magic_event(1, true),
                hallucination_compat_projectile_event(2),
            ],
            &zone,
        );
        assert_eq!(
            fx.active.len(),
            1,
            "Hallucination has no cast bitmap and ignores the compatibility projectile"
        );
        let projectile = fx
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-hallucination-"))
            .expect("Hallucination projectile");
        assert_eq!(projectile.start_at, HALLUCINATION_SPELL_ACTION_MS);
        assert_eq!(
            projectile
                .current
                .as_ref()
                .map(|animation| animation.duration_ms),
            Some(250)
        );
        assert_eq!(
            projectile
                .current
                .as_ref()
                .map(|animation| animation.interval),
            Some(50),
            "Crystal duration/count clock turns the 48 ms process interval into five 50 ms frames"
        );
        assert!(projectile
            .current
            .as_ref()
            .is_some_and(|animation| { animation.frames[0].path.ends_with("/Magic/1160.png") }));
        assert!(
            projectile.queued.is_none(),
            "target binding occurs at launch"
        );
        assert!(fx.take_due_sound_events(0).is_empty());

        let before: Value = serde_json::from_str(
            &fx.tick_with_visibility(599, true)
                .expect("Hallucination before projectile"),
        )
        .expect("Hallucination pre-projectile JSON");
        assert_eq!(before["effects"], json!([]));
        assert!(fx.take_due_sound_events(599).is_empty());

        let launch: Value = serde_json::from_str(
            &fx.tick_with_visibility(600, true)
                .expect("Hallucination projectile launch"),
        )
        .expect("Hallucination launch JSON");
        assert!(launch["effects"][0]["imageUrl"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Magic/1160.png")));
        assert!(fx.take_due_sound_events(600).is_empty());
        assert!(fx.active.iter().any(|instance| {
            instance.key.starts_with("fx-hallucination-")
                && instance
                    .queued
                    .as_ref()
                    .is_some_and(|animation| animation.frames[0].path.ends_with("/Magic2/1110.png"))
        }));
        assert!(fx.take_due_sound_events(849).is_empty());

        let impact_sound = fx.take_due_sound_events(850);
        assert_eq!(impact_sound.len(), 1);
        assert_eq!(impact_sound[0].cue, HALLUCINATION_IMPACT_SOUND_CUE);
        assert_eq!(impact_sound[0].file_name, HALLUCINATION_IMPACT_SOUND_FILE);
        let impact: Value = serde_json::from_str(
            &fx.tick_with_visibility(850, true)
                .expect("Hallucination impact"),
        )
        .expect("Hallucination impact JSON");
        assert!(impact["effects"][0]["imageUrl"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Magic2/1110.png")));
        assert!(fx.take_due_sound_events(850).is_empty());
    }

    #[test]
    fn hallucination_cast_false_has_no_visual_or_audio_phases() {
        let zone = HashMap::from([(1000, (288, 616)), (2076, (288, 611))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 288, 616, &[hallucination_magic_event(1, false)], &zone);
        assert!(fx.active.is_empty());
        assert!(fx.take_due_sound_events(0).is_empty());
        assert!(fx.take_due_sound_events(10_000).is_empty());
    }

    #[test]
    fn hallucination_completion_uses_current_terminal_dead_state() {
        let zone = HashMap::from([(1000, (288, 616)), (2076, (288, 611))]);

        let mut dying = NativeEffects::default();
        dying.observe(0, 288, 616, &[hallucination_magic_event(1, true)], &zone);
        dying.observe(600, 288, 616, &[], &zone);
        dying.replace_dead_action_object_ids(HashSet::new());
        assert_eq!(
            dying.take_due_sound_events(850)[0].cue,
            HALLUCINATION_IMPACT_SOUND_CUE,
            "Die must retain Hallucination impact"
        );
        let dying_impact: Value = serde_json::from_str(
            &dying
                .tick_with_visibility(850, true)
                .expect("Hallucination Die-phase impact"),
        )
        .expect("Hallucination Die-phase JSON");
        assert!(dying_impact["effects"].as_array().is_some_and(|effects| {
            effects.iter().any(|effect| {
                effect["imageUrl"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("/Magic2/1110.png"))
            })
        }));

        let mut dead = NativeEffects::default();
        dead.observe(0, 288, 616, &[hallucination_magic_event(1, true)], &zone);
        dead.observe(600, 288, 616, &[], &zone);
        dead.replace_dead_action_object_ids(HashSet::from([2076]));
        assert!(dead.take_due_sound_events(850).is_empty());
        let suppressed: Value = serde_json::from_str(
            &dead
                .tick_with_visibility(850, true)
                .expect("Hallucination Dead-phase suppression"),
        )
        .expect("Hallucination Dead-phase JSON");
        assert_eq!(suppressed["effects"], json!([]));

        let mut revived = NativeEffects::default();
        revived.observe(0, 288, 616, &[hallucination_magic_event(1, true)], &zone);
        revived.observe(600, 288, 616, &[], &zone);
        revived.replace_dead_action_object_ids(HashSet::from([2076]));
        let _ = revived.tick_with_visibility(700, true);
        assert!(revived.active.iter().any(|instance| {
            instance.key.starts_with("fx-hallucination-") && instance.queued.is_some()
        }));
        revived.replace_dead_action_object_ids(HashSet::new());
        assert_eq!(
            revived.take_due_sound_events(850)[0].cue,
            HALLUCINATION_IMPACT_SOUND_CUE
        );
        let restored: Value = serde_json::from_str(
            &revived
                .tick_with_visibility(850, true)
                .expect("Hallucination revived-target impact"),
        )
        .expect("Hallucination revived-target JSON");
        assert!(restored["effects"][0]["imageUrl"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Magic2/1110.png")));
    }

    #[test]
    fn hallucination_target_absence_compatibility_and_session_boundaries_fail_closed() {
        let initial = HashMap::from([(1000, (288, 616)), (2076, (288, 611))]);
        let source_only = HashMap::from([(1000, (288, 616))]);

        let mut compatibility_only = NativeEffects::default();
        compatibility_only.observe(
            0,
            288,
            616,
            &[hallucination_compat_projectile_event(1)],
            &initial,
        );
        assert!(compatibility_only.active.is_empty());

        let mut absent = NativeEffects::default();
        absent.observe(
            0,
            288,
            616,
            &[hallucination_magic_event(1, true)],
            &source_only,
        );
        absent.observe(600, 288, 616, &[], &source_only);
        let point_flight = absent
            .active
            .iter()
            .find(|instance| instance.key.starts_with("fx-hallucination-"))
            .expect("point-flight Hallucination");
        assert!(point_flight.queued.is_none());
        assert!(!absent.anchor_object_ids.contains_key(&point_flight.key));
        assert!(absent.take_due_sound_events(10_000).is_empty());

        for boundary in ["MapChanged", "LogOutSuccess"] {
            let mut fx = NativeEffects::default();
            fx.observe(0, 288, 616, &[hallucination_magic_event(1, true)], &initial);
            fx.observe(
                100,
                288,
                616,
                &[NativeEffectEvent {
                    sequence: 2,
                    generation: 23,
                    packet: boundary.to_owned(),
                    payload: json!({}),
                }],
                &initial,
            );
            assert!(fx.active.is_empty(), "{boundary} clears Hallucination");
            assert!(
                fx.take_due_sound_events(10_000).is_empty(),
                "{boundary} clears delayed Hallucination audio"
            );
        }

        let mut reset = NativeEffects::default();
        reset.observe(0, 288, 616, &[hallucination_magic_event(1, true)], &initial);
        reset.reset_session();
        assert!(reset.active.is_empty());
        assert!(reset.local_projectile_targets.is_empty());
        assert!(reset.pending_sounds.is_empty());
    }

    #[test]
    fn hallucination_production_frames_and_audio_are_integrity_closed() {
        use sha2::{Digest, Sha256};

        let fixture = hallucination_fixture();
        let catalog = EffectCatalog::load().expect("production effect catalog");
        assert!(catalog.spell_cast_animation("Hallucination", 0).is_none());
        for direction in 0..16_u32 {
            let animation = catalog
                .spell_projectile_animation("Hallucination", direction)
                .expect("all Hallucination directions resolve");
            assert_eq!(animation.interval, HALLUCINATION_PROJECTILE_STEP_MS);
            assert_eq!(animation.frames.len(), 3);
            for (frame, source) in animation.frames.iter().zip(0..3_u32) {
                let index = 1160 + direction * 10 + source;
                assert!(frame.path.ends_with(&format!("/Magic/{index}.png")));
                assert!(crate::frame_png_exists(&frame.path));
            }
        }
        let impact = catalog
            .spell_impact_animation("Hallucination")
            .expect("Hallucination impact");
        assert_eq!(impact.frames.len(), 10);
        assert!(impact.frames[0].path.ends_with("/Magic2/1110.png"));
        assert!(impact.frames[9].path.ends_with("/Magic2/1119.png"));
        for frame in &impact.frames {
            assert!(crate::frame_png_exists(&frame.path));
        }
        let audio = &fixture["source"]["audio"][0];
        let file = audio["file"].as_str().expect("Hallucination audio file");
        let path = assets::asset_path(&format!("original-ui/Sound/{file}"))
            .expect("packaged Hallucination sound path");
        let bytes = fs::read(path).expect("read Hallucination sound");
        assert_eq!(bytes.len(), audio["sourceBytes"]);
        assert_eq!(format!("{:x}", Sha256::digest(&bytes)), audio["sha256"]);
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

    fn archer_normal_range_event(sequence: u64, spell: u32) -> NativeEffectEvent {
        let mut event = right_guard_range_event(sequence, 371, 2001, "ARArmour/00");
        event.payload["_nativeAttacker"] = player_actor(
            371,
            "male",
            "archer",
            "ARArmour/00",
            Some("ARWeapon/00"),
            None,
        );
        event.payload["target"] = json!({"x": 293, "y": 616});
        event.payload["spell"] = json!(spell);
        event
    }

    #[test]
    fn archer_normal_arrow_and_focus_catalog_follow_exact_source_frames() {
        let catalog = EffectCatalog::load().expect("production effect catalog");
        for direction in 0..16 {
            let arrow = catalog
                .archer_normal_arrow_animation(direction)
                .expect("all eighty source normal-arrow frames are required");
            assert_eq!(arrow.kind, "projectile");
            assert_eq!(arrow.frames.len(), 5);
            assert_eq!(arrow.interval, 30);
            assert_eq!(arrow.duration_ms, 150);
            assert_eq!(arrow.light, Some(6));
            assert!(arrow.blend && !arrow.repeat);
            for (frame, offset) in arrow.frames.iter().zip(0..5) {
                assert!(frame
                    .path
                    .ends_with(&format!("/Magic3/{}.png", 1030 + direction * 10 + offset)));
                assert!(crate::frame_png_exists(&frame.path));
            }
        }
        assert!(catalog.archer_normal_arrow_animation(16).is_none());
        let focus = catalog
            .archer_focus_proc_animation()
            .expect("ten source Focus frames");
        assert_eq!(focus.frames.len(), 10);
        assert_eq!(focus.interval, 80);
        assert_eq!(focus.duration_ms, 800);
        assert_eq!(focus.light, Some(6));
        assert!(focus.blend && !focus.repeat);
        for (frame, index) in focus.frames.iter().zip(2730..2740) {
            assert!(frame.path.ends_with(&format!("/Magic3/{index}.png")));
            assert!(crate::frame_png_exists(&frame.path));
        }
    }

    #[test]
    fn archer_catalog_fails_closed_on_missing_frames_or_changed_direction_contract() {
        let mut catalog = EffectCatalog::load().expect("production effect catalog");
        let original = catalog.map_by_name["ArcherNormalArrow"].clone();
        catalog
            .map_by_name
            .get_mut("ArcherNormalArrow")
            .unwrap()
            .direction_stride = Some(12);
        assert!(catalog.archer_normal_arrow_animation(4).is_none());
        catalog
            .map_by_name
            .insert("ArcherNormalArrow".to_owned(), original);
        catalog
            .libraries
            .get_mut("Magic3")
            .unwrap()
            .frames
            .remove("1072");
        assert!(catalog.archer_normal_arrow_animation(4).is_none());
        assert!(catalog.archer_normal_arrow_animation(5).is_some());
        catalog
            .libraries
            .get_mut("Magic3")
            .unwrap()
            .frames
            .remove("2732");
        assert!(catalog.archer_focus_proc_animation().is_none());
    }

    #[test]
    fn archer_normal_arrow_waits_frame_five_and_delivers_each_sound_once() {
        let zone = HashMap::from([(371, (287, 616)), (2001, (293, 616))]);
        let event = archer_normal_range_event(1, 0);
        let mut fx = NativeEffects::default();
        fx.observe(0, 287, 616, &[event.clone()], &zone);
        assert_eq!(fx.active.len(), 1);
        assert_eq!(fx.active[0].start_at, 500);
        let before: Value = serde_json::from_str(&fx.tick(499).unwrap()).unwrap();
        assert_eq!(before["effects"], json!([]));
        assert!(fx.take_due_sound_events(499).is_empty());
        let launch: Value = serde_json::from_str(&fx.tick(500).unwrap()).unwrap();
        assert_eq!(launch["effects"].as_array().map(Vec::len), Some(1));
        assert!(launch["effects"][0]["imageUrl"]
            .as_str()
            .unwrap()
            .ends_with("/Magic3/1070.png"));
        let sounds = fx.take_due_sound_events(500);
        assert_eq!(sounds.len(), 1);
        assert_eq!(sounds[0].file_name, "M121-0.wav");
        assert_eq!(sounds[0].generation, 17);
        assert_eq!(sounds[0].sequence, 1);
        fx.observe(501, 287, 616, &[event], &zone);
        assert_eq!(
            fx.active.len(),
            1,
            "a duplicate packet cannot mint another arrow"
        );
        assert!(fx.take_due_sound_events(501).is_empty());
        fx.tick(650);
        let light = fx.current_light_snapshots(650);
        assert_eq!(light.len(), 1);
        assert_eq!(light[0].light, 6);
        assert!((light[0].tile_x - 290.0).abs() < f32::EPSILON);
        let arrival: Value = serde_json::from_str(&fx.tick(800).unwrap()).unwrap();
        assert_eq!(arrival["effects"], json!([]));
        let sounds = fx.take_due_sound_events(800);
        assert_eq!(sounds.len(), 1);
        assert_eq!(sounds[0].file_name, "M121-2.wav");
        assert!(fx.take_due_sound_events(10_000).is_empty());
        assert!(fx.active.is_empty() && fx.local_projectile_targets.is_empty());
    }

    #[test]
    fn archer_normal_arrow_binds_at_launch_then_tracks_without_rotating() {
        let mut zone = HashMap::from([(371, (287, 616)), (2001, (293, 616))]);
        let mut fx = NativeEffects::default();
        fx.observe(0, 287, 616, &[archer_normal_range_event(1, 0)], &zone);
        zone.insert(371, (288, 616));
        zone.insert(2001, (288, 622));
        fx.observe(499, 288, 616, &[], &zone);
        let launch: Value = serde_json::from_str(&fx.tick(500).unwrap()).unwrap();
        assert!(launch["effects"][0]["imageUrl"]
            .as_str()
            .unwrap()
            .ends_with("/Magic3/1110.png"));
        assert_eq!(fx.active[0].from_x, Some(288.0));
        assert_eq!(fx.take_due_sound_events(500)[0].file_name, "M121-0.wav");
        zone.insert(2001, (288, 625));
        fx.observe(600, 288, 616, &[], &zone);
        let flight: Value = serde_json::from_str(&fx.tick(600).unwrap()).unwrap();
        assert!(flight["effects"][0]["imageUrl"]
            .as_str()
            .unwrap()
            .ends_with("/Magic3/1113.png"));
        assert_eq!(fx.active[0].current.as_ref().unwrap().duration_ms, 450);
        zone.remove(&2001);
        fx.observe(
            601,
            288,
            616,
            &[lifecycle_effect_event(2, "ObjectRemove", Some(2001))],
            &zone,
        );
        assert_eq!(
            fx.active.len(),
            1,
            "a launched arrow retains its last bound target point"
        );
        assert_eq!((fx.active[0].tile_x, fx.active[0].tile_y), (288, 625));
        assert!(!fx.local_projectile_targets[&fx.active[0].key].unbound_archer_arrow);
        fx.tick(950);
        assert_eq!(fx.take_due_sound_events(950)[0].file_name, "M121-2.wav");
        assert!(fx.active.is_empty());
    }

    #[test]
    fn archer_focus_proc_requires_the_authoritative_focus_spell_byte() {
        let zone = HashMap::from([(371, (287, 616)), (2001, (293, 616))]);
        let mut normal = NativeEffects::default();
        normal.observe(0, 287, 616, &[archer_normal_range_event(1, 0)], &zone);
        assert_eq!(normal.active.len(), 1);
        let mut focus = NativeEffects::default();
        focus.observe(0, 287, 616, &[archer_normal_range_event(1, 121)], &zone);
        assert_eq!(focus.active.len(), 2);
        assert_eq!(focus.active[1].start_at, 600);
        focus.tick(500);
        assert_eq!(focus.take_due_sound_events(500)[0].file_name, "M121-0.wav");
        let proc: Value = serde_json::from_str(&focus.tick(600).unwrap()).unwrap();
        assert!(proc["effects"].as_array().unwrap().iter().any(|effect| {
            effect["imageUrl"]
                .as_str()
                .unwrap()
                .ends_with("/Magic3/2730.png")
        }));
        let sounds = focus.take_due_sound_events(600);
        assert_eq!(sounds.len(), 1);
        assert_eq!(sounds[0].file_name, "M121-5.wav");
        focus.tick(800);
        assert_eq!(focus.take_due_sound_events(800)[0].file_name, "M121-2.wav");
        let remaining: Value = serde_json::from_str(&focus.tick(1320).unwrap()).unwrap();
        assert!(remaining["effects"][0]["imageUrl"]
            .as_str()
            .unwrap()
            .ends_with("/Magic3/2739.png"));
        focus.tick(1400);
        assert!(focus.active.is_empty());
        assert!(focus.take_due_sound_events(10_000).is_empty());
    }

    #[test]
    fn archer_ground_arrow_extends_outside_source_data_range_without_target_sound() {
        let zone = HashMap::from([(371, (287, 616))]);
        let mut event = archer_normal_range_event(1, 0);
        event.payload["targetId"] = json!(0);
        event.payload["target"] = json!({"x": 289, "y": 616});
        let mut fx = NativeEffects::default();
        fx.observe(0, 287, 616, &[event], &zone);
        fx.tick(500);
        assert_eq!(fx.active[0].tile_x, 291);
        assert_eq!(fx.take_due_sound_events(500)[0].file_name, "M121-0.wav");
        fx.tick(500);
        assert_eq!(
            fx.active[0].tile_x, 291,
            "one clock instant processes the arrow once"
        );
        fx.tick(501);
        assert_eq!(fx.active[0].tile_x, 295);
        fx.tick(502);
        assert_eq!(fx.active[0].tile_x, 303);
        fx.tick(503);
        assert_eq!(fx.active[0].tile_x, 303);
        fx.tick(1300);
        assert!(fx.active.is_empty());
        assert!(fx.take_due_sound_events(10_000).is_empty());
    }

    #[test]
    fn archer_normal_arrow_rejects_unknown_actor_and_clears_at_lifecycle_boundaries() {
        let zone = HashMap::from([(371, (287, 616)), (2001, (293, 616))]);
        for mutation in [
            "missingActor",
            "otherClass",
            "monster",
            "wrongOwner",
            "fractionalPoint",
        ] {
            let mut event = archer_normal_range_event(1, 121);
            match mutation {
                "missingActor" => {
                    event
                        .payload
                        .as_object_mut()
                        .unwrap()
                        .remove("_nativeAttacker");
                }
                "otherClass" => event.payload["_nativeAttacker"]["classKey"] = json!("warrior"),
                "monster" => event.payload["_nativeAttacker"]["kind"] = json!("monster"),
                "wrongOwner" => event.payload["_nativeAttacker"]["objectId"] = json!(372),
                "fractionalPoint" => event.payload["location"]["x"] = json!(287.5),
                _ => unreachable!(),
            }
            let mut fx = NativeEffects::default();
            fx.observe(0, 287, 616, &[event], &zone);
            assert!(fx.active.is_empty(), "{mutation}");
            assert!(fx.take_due_sound_events(10_000).is_empty(), "{mutation}");
        }
        for (packet, object_id) in [
            ("MapChanged", None),
            ("LogOutSuccess", None),
            ("ObjectRemove", Some(371)),
        ] {
            let mut fx = NativeEffects::default();
            fx.observe(0, 287, 616, &[archer_normal_range_event(1, 121)], &zone);
            fx.observe(
                499,
                287,
                616,
                &[lifecycle_effect_event(2, packet, object_id)],
                &zone,
            );
            assert!(fx.active.is_empty(), "{packet}");
            assert!(fx.take_due_sound_events(10_000).is_empty(), "{packet}");
            assert!(fx.local_projectile_targets.is_empty(), "{packet}");
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
    fn warrior_attack_source_frames_cover_all_directions_and_packet_levels() {
        let catalog = EffectCatalog::load().expect("exported source assets");
        for (spell, library, base, stride, levelled) in [
            ("Slaying", "Magic", 1820, 10, true),
            ("Thrusting", "Magic", 2190, 10, true),
            ("HalfMoon", "Magic", 2560, 10, true),
            ("TwinDrakeBlade", "Magic2", 220, 20, false),
            ("CrossHalfMoon", "Magic2", 40, 10, false),
        ] {
            for level in 0..if levelled { 4 } else { 1 } {
                for direction in 0..8 {
                    let anim = catalog
                        .warrior_attack_animation(spell, direction, level)
                        .unwrap();
                    assert_eq!(
                        (anim.frames.len(), anim.interval, anim.duration_ms),
                        (6, 100, 600)
                    );
                    assert_eq!(anim.light, Some(0));
                    assert!((anim.opacity - 0.7).abs() < f32::EPSILON);
                    for (index, frame) in anim.frames.iter().enumerate() {
                        let source = base
                            + direction * stride
                            + if levelled { level * 90 } else { 0 }
                            + index as u32;
                        assert!(
                            frame.path.ends_with(&format!("/{library}/{source}.png")),
                            "{spell}/{direction}/{level}: {}",
                            frame.path
                        );
                    }
                    assert!(anim.frame_at(599).is_some());
                    assert!(anim.frame_at(600).is_none());
                }
            }
            assert!(catalog.warrior_attack_animation(spell, 8, 0).is_none());
        }
        assert!(catalog.warrior_attack_animation("HalfMoon", 0, 4).is_none());
    }

    #[test]
    fn warrior_attack_uses_packet_spell_and_level_and_follows_actor() {
        let mut zone = HashMap::from([(1000, (288, 616))]);
        let mut fx = NativeEffects::default();
        let event = NativeEffectEvent {
            sequence: 1,
            generation: 16,
            packet: "ObjectAttack".into(),
            payload: json!({"objectId":1000,"location":{"x":288,"y":616},"direction":"Right","spell":"HalfMoon","level":3}),
        };
        fx.observe(0, 288, 616, &[event.clone()], &zone);
        assert_eq!(fx.active.len(), 1);
        assert!(fx.active[0].current.as_ref().unwrap().frames[0]
            .path
            .ends_with("/Magic/2850.png"));
        fx.observe(50, 288, 616, &[event], &zone);
        assert_eq!(fx.active.len(), 1);
        assert_eq!(fx.active[0].started_at, 0);
        zone.insert(1000, (289, 617));
        fx.observe(200, 288, 616, &[], &zone);
        assert_eq!((fx.active[0].tile_x, fx.active[0].tile_y), (289, 617));
        for (sequence, spell, level) in [
            (2, "Thrusting", json!(null)),
            (3, "Slaying", json!(4)),
            (4, "Fencing", json!(3)),
        ] {
            let mut rejected = NativeEffects::default();
            rejected.observe(0,288,616,&[NativeEffectEvent {sequence,generation:16,packet:"ObjectAttack".into(),
                payload:json!({"objectId":1000,"location":{"x":288,"y":616},"direction":"Right","spell":spell,"level":level})}],&zone);
            assert!(rejected.active.is_empty());
        }
        fx.tick_with_visibility(600, true);
        assert!(fx.active.is_empty());
    }

    #[test]
    fn immortal_skin_layers_start_together_follow_actor_and_expire_independently() {
        let mut zone = HashMap::from([(1000, (288, 616))]);
        let mut fx = NativeEffects::default();
        let event = NativeEffectEvent {
            sequence: 1,
            generation: 16,
            packet: "ObjectMagic".into(),
            payload: json!({"objectId":1000,"location":{"x":288,"y":616},"direction":"Down","spell":"ImmortalSkin","cast":true}),
        };
        fx.observe(0, 288, 616, &[event.clone()], &zone);
        assert_eq!(fx.active.len(), 2);
        let mut durations = fx
            .active
            .iter()
            .map(|e| e.current.as_ref().unwrap().duration_ms)
            .collect::<Vec<_>>();
        durations.sort();
        assert_eq!(durations, vec![600, 2397]);
        let paths = fx
            .active
            .iter()
            .map(|e| e.current.as_ref().unwrap().frames[0].path.as_str())
            .collect::<Vec<_>>();
        assert!(paths.iter().any(|p| p.ends_with("/Magic3/550.png")));
        assert!(paths.iter().any(|p| p.ends_with("/Magic3/570.png")));
        assert!(fx.active.iter().all(|e| e.start_at == 0));
        fx.observe(50, 288, 616, &[event], &zone);
        assert_eq!(fx.active.len(), 2);
        zone.insert(1000, (289, 617));
        fx.observe(200, 288, 616, &[], &zone);
        assert!(fx.active.iter().all(|e| (e.tile_x, e.tile_y) == (289, 617)));
        fx.tick_with_visibility(600, true);
        assert_eq!(fx.active.len(), 1);
        assert!(fx.active[0].current.as_ref().unwrap().frames[0]
            .path
            .ends_with("/Magic3/550.png"));
        fx.tick_with_visibility(2397, true);
        assert!(fx.active.is_empty());
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
        assert_eq!(
            fx.active.len(),
            1,
            "same sequence must not replay the effect"
        );
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
        missing.observe(0, 288, 616, &[healing_target_event(1)], &HashMap::new());
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

include!("pet_effects.rs");
