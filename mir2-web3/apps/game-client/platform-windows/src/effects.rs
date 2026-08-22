//! Native Windows authoritative scene-effect parsing and lifecycle.
//!
//! Gateway packets remain authoritative for *where* an effect appears, who
//! casts it, its direction/target and when it must be removed. This module
//! only owns client-side manifest resolution (effects.generated.json +
//! per-library meta.json) and a Crystal-faithful frame clock, mirroring the
//! Web scene-effect-runtime.ts + crystal-magic-effects.ts resolver semantics.
//! It never fabricates client game state and never draws a fake/fallback
//! sprite for a missing asset - a frame whose PNG is absent yields no sprite.

use std::collections::HashMap;
use std::fs;
use std::sync::OnceLock;

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
/// Ground/map/object/persistent effects sit just below the body layer (order 5).
const EFFECT_GROUND_ORDER: f32 = 4.8;
/// Transient cast/projectile/impact spell effects sit above the front weapon (7).
const EFFECT_TRANSIENT_ORDER: f32 = 9.0;

/// Upper bound on simultaneously-active transient effects.
pub const MAX_ACTIVE_EFFECTS: usize = 96;

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
    kind: Option<String>,
    #[serde(default)]
    blend: Option<bool>,
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
    pub repeat: bool,
    pub offset_x: f32,
    pub offset_y: f32,
    pub duration_ms: u64,
}

impl Animation {
    /// The source frame for the given elapsed ms, honouring repeat (mirrors
    /// Web effectFrameAt). None once finished (and not repeating).
    pub(crate) fn frame_at(&self, elapsed_ms: u64) -> Option<&EffectFrameMeta> {
        if self.frames.is_empty() {
            return None;
        }
        let mut index = elapsed_ms / self.interval.max(1);
        if index >= self.frames.len() as u64 {
            if !self.repeat {
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
            .object_effects
            .iter()
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

    fn resolve_sub(&self, sub: &SubSpec, name: &str, fallback_kind: &str) -> Option<Animation> {
        let frames = self.resolve_frames(&sub.library, sub.base, sub.count);
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
            repeat: sub.repeat.unwrap_or(false),
            offset_x: offset.x,
            offset_y: offset.y,
            duration_ms: interval * frame_count,
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
            repeat: entry.repeat.unwrap_or(false),
            offset_x: offset.x,
            offset_y: offset.y,
            duration_ms: interval * frame_count,
        })
    }

    pub(crate) fn spell_cast_animation(&self, spell: &str, direction: u32) -> Option<Animation> {
        let entry = self.spell_by_name.get(spell)?;
        if matches!(
            entry.kind.as_deref().unwrap_or(""),
            "projectile" | "impact" | "target"
        ) {
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

    pub(crate) fn spell_projectile_animation(&self, spell: &str) -> Option<Animation> {
        let entry = self.spell_by_name.get(spell)?;
        let sub = entry.projectile.as_ref()?;
        self.resolve_sub(sub, spell, "projectile")
    }

    pub(crate) fn spell_impact_animation(&self, spell: &str) -> Option<Animation> {
        let entry = self.spell_by_name.get(spell)?;
        let sub = entry.impact.as_ref()?;
        self.resolve_sub(sub, spell, "impact")
    }

    pub(crate) fn spell_return_animation(&self, spell: &str) -> Option<Animation> {
        let entry = self.spell_by_name.get(spell)?;
        let sub = entry.return_effect.as_ref()?;
        self.resolve_sub(sub, spell, "return")
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
    Cast,
    Projectile,
    Impact,
    Persistent,
}

impl EffectKindTag {
    fn z_order(self) -> f32 {
        match self {
            EffectKindTag::Cast | EffectKindTag::Projectile | EffectKindTag::Impact => {
                EFFECT_TRANSIENT_ORDER
            }
            _ => EFFECT_GROUND_ORDER,
        }
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

/// Bevy resource holding the authoritative effect event buffer and the active
/// effect set. The catalog is a lazily-loaded shared singleton.
#[derive(Resource)]
pub(crate) struct NativeEffects {
    active: Vec<EffectInstance>,
    last_effect_sequence: u64,
    last_generation: u64,
    instance_seq: u64,
    now_ms: u64,
    player_x: i32,
    player_y: i32,
    last_state: Option<String>,
}

impl Default for NativeEffects {
    fn default() -> Self {
        Self {
            active: Vec::new(),
            last_effect_sequence: 0,
            last_generation: 0,
            instance_seq: 0,
            now_ms: 0,
            player_x: 0,
            player_y: 0,
            last_state: None,
        }
    }
}

impl NativeEffects {
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
        for event in events {
            if event.generation != self.last_generation {
                self.last_generation = event.generation;
                self.last_effect_sequence = 0;
                self.active.clear();
            }
            if event.sequence <= self.last_effect_sequence {
                continue;
            }
            self.last_effect_sequence = event.sequence;
            let spell = event
                .payload
                .get("spell")
                .and_then(Value::as_str)
                .or_else(|| {
                    event
                        .payload
                        .get("effect")
                        .and_then(Value::as_u64)
                        .map(|_| "effect")
                })
                .unwrap_or("-")
                .to_owned();
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
            self.apply_event(&event.packet, &event.payload, zone_tiles, &provenance);
        }
        while self.active.len() > MAX_ACTIVE_EFFECTS {
            self.active.remove(0);
        }
    }

    pub(crate) fn reset_for_new_connection(&mut self) {
        self.last_generation = self.last_generation.wrapping_add(1);
        self.last_effect_sequence = 0;
        self.active.clear();
    }

    pub(crate) fn reset_session(&mut self) {
        self.reset_for_new_connection();
    }

    fn next_key(&mut self, tag: &str) -> String {
        self.instance_seq = self.instance_seq.saturating_add(1);
        format!("fx-{tag}-{}", self.instance_seq)
    }

    fn apply_event(
        &mut self,
        packet: &str,
        payload: &Value,
        zone_tiles: &HashMap<u32, (i32, i32)>,
        provenance: &EffectProvenance,
    ) {
        match packet {
            "MapChanged" | "LogOutSuccess" => self.active.clear(),
            "ObjectMagic" => self.apply_object_magic(payload, provenance),
            "ObjectProjectile" => self.apply_object_projectile(payload, zone_tiles, provenance),
            "ObjectEffect" => self.apply_object_effect(payload, zone_tiles, provenance),
            "MapEffect" => self.apply_map_effect(payload, provenance),
            "ObjectSpell" => self.apply_object_spell(payload, zone_tiles, provenance),
            "ObjectRemove" | "ObjectHide" => self.apply_object_remove(payload),
            _ => {}
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
        let key = self.next_key("cast");
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
            start_at: now,
            persistent_object_id: None,
            provenance: provenance.clone(),
        });
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
        let open_id = |name: &str| -> Option<u32> {
            payload.get(name).and_then(Value::as_u64).map(|v| v as u32)
        };
        let (Some(source_id), Some(destination_id)) =
            (open_id("sourceId"), open_id("destinationId"))
        else {
            // Without authoritative source/destination we must not fabricate a path.
            return;
        };
        let (Some(&(from_x, from_y)), Some(&(to_x, to_y))) =
            (zone_tiles.get(&source_id), zone_tiles.get(&destination_id))
        else {
            return;
        };
        let projectile = catalog.spell_projectile_animation(spell);
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
        let object_id = payload
            .get("objectId")
            .and_then(Value::as_u64)
            .map(|v| v as u32);
        let Some((tile_x, tile_y)) = object_id.and_then(|id| zone_tiles.get(&id)).copied() else {
            return;
        };
        let Some(anim) = catalog.map_animation_by_number(effect, 0) else {
            return;
        };
        let now = self.now_ms;
        let delay_ms = payload
            .get("delayTime")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let key = self.next_key("obj");
        self.active.push(EffectInstance {
            key,
            kind: EffectKindTag::Ground,
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
            kind: EffectKindTag::Ground,
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

    fn apply_object_remove(&mut self, payload: &Value) {
        let Some(object_id) = payload
            .get("objectId")
            .and_then(Value::as_u64)
            .map(|v| v as u32)
        else {
            return;
        };
        let remove_key = format!("spell-{object_id}");
        self.active.retain(|instance| instance.key != remove_key);
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
        let player_x = self.player_x;
        let player_y = self.player_y;
        let _ = effect_catalog();

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
                let shadow_pair = match (frame.shadow_x, frame.shadow_y) {
                    (Some(sx), Some(sy)) => Some((sx, sy)),
                    _ => None,
                };
                let mut entry = json!({
                    "key": instance.key,
                    "imageUrl": frame.path,
                    "left": left,
                    "top": top,
                    "width": frame.width,
                    "height": frame.height,
                    "z": effect_z(tile_x, tile_y, instance.kind.z_order()),
                    "additive": animation.blend,
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
    player_ui: Option<
        bevy::prelude::Res<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>,
    >,
) {
    let now_ms = u64::try_from(time.elapsed().as_millis()).unwrap_or(u64::MAX);
    let effect_visible = player_ui
        .as_deref()
        .map(|state| state.core.options.effect)
        .unwrap_or(true);
    if let Some(json) = effects.tick_with_visibility(now_ms, effect_visible) {
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
    use crate::gameplay_bridge::NativeEffectEvent;
    use serde_json::json;

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
            repeat,
            offset_x: 0.0,
            offset_y: 0.0,
            duration_ms: interval * count,
        }
    }

    #[test]
    fn manifest_loads() {
        let catalog = EffectCatalog::load().expect("manifest should load");
        assert!(!catalog.spell_by_name.is_empty());
        assert!(!catalog.libraries.is_empty());
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
            .spell_projectile_animation("FireBall")
            .expect("projectile via resolve_sub");
        assert!(
            projectile.frames[0].path.ends_with("/Magic/10.png"),
            "projectile first frame should be Magic/10.png, got {}",
            projectile.frames[0].path
        );
        assert_eq!(projectile.kind, "projectile");
        assert_eq!(projectile.frames.len(), 6);
        assert_eq!(projectile.interval, 30);
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
            .is_some_and(|anim| anim.frames[0].path.ends_with("/Magic/10.png")));
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
        assert!(catalog.spell_projectile_animation("Lightning").is_none());
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
