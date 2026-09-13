//! Android packet-to-effect projection for the shared Bevy renderer.
//!
//! The producer consumes only authoritative Gateway packets and resolves every
//! visible frame through the checked-in Crystal effect manifest. Unknown or
//! incomplete catalog entries stay invisible; there is no synthetic fallback.

use bevy::prelude::*;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

const MAX_PACKET_BYTES: usize = 16 * 1024;
const MAX_ACTIVE_EFFECTS: usize = 96;
const MAX_EFFECT_FRAMES: usize = 256;
const MAX_PRELOAD_IMAGES: usize = 512;
const STAGE_WIDTH: f32 = 1024.0;
const STAGE_HEIGHT: f32 = 768.0;
const CELL_WIDTH: f32 = 48.0;
const CELL_HEIGHT: f32 = 32.0;
const ENTITY_ORIGIN_X: f32 = 480.0;
const ENTITY_ORIGIN_Y: f32 = 352.0;
const EFFECT_DEPTH_GAIN: f32 = 10.0;
const EFFECT_GROUND_ORDER: f32 = 4.8;
const EFFECT_TRANSIENT_ORDER: f32 = 9.0;
const PROJECTILE_TILE_MS: u64 = 50;

const MANIFEST_JSON: &str =
    include_str!("../../../web/public/original-effects/effects.generated.json");
const EFFECT_META_JSON: &str =
    include_str!("../../../web/public/original-effects/Effect/meta.json");
const MAGIC_META_JSON: &str = include_str!("../../../web/public/original-effects/Magic/meta.json");
const MAGIC2_META_JSON: &str =
    include_str!("../../../web/public/original-effects/Magic2/meta.json");
const MAGIC3_META_JSON: &str =
    include_str!("../../../web/public/original-effects/Magic3/meta.json");

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EffectFrame {
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

#[derive(Debug, Deserialize)]
struct LibraryMeta {
    frames: HashMap<String, EffectFrame>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
struct EffectOffset {
    #[serde(default)]
    x: f32,
    #[serde(default)]
    y: f32,
}

#[derive(Clone, Debug, Deserialize)]
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
    repeat: Option<bool>,
    #[serde(default)]
    offset: Option<EffectOffset>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EffectSpec {
    #[serde(default)]
    spell: Option<String>,
    #[serde(default)]
    spell_id: Option<u32>,
    #[serde(default)]
    effect: Option<String>,
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
    value_count: Option<i64>,
    #[serde(default)]
    value_stride: Option<i64>,
    #[serde(default)]
    blend: Option<bool>,
    #[serde(default)]
    rate: Option<f32>,
    #[serde(default)]
    repeat: Option<bool>,
    #[serde(default)]
    offset: Option<EffectOffset>,
    #[serde(default)]
    projectile: Option<SubSpec>,
    #[serde(default)]
    impact: Option<SubSpec>,
    #[serde(default, rename = "returnEffect")]
    return_effect: Option<SubSpec>,
}

#[derive(Debug, Deserialize)]
struct EffectName {
    id: u32,
    name: String,
}

#[derive(Debug, Deserialize)]
struct EffectsManifest {
    #[serde(default)]
    spell_effect_map: Vec<EffectName>,
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

#[derive(Clone, Debug)]
struct Animation {
    kind: String,
    frames: Vec<EffectFrame>,
    interval_ms: u64,
    duration_ms: u64,
    additive: bool,
    opacity: f32,
    repeat: bool,
    offset: EffectOffset,
}

impl Animation {
    fn frame_at(&self, elapsed_ms: u64, persistent: bool) -> Option<&EffectFrame> {
        if self.frames.is_empty() {
            return None;
        }
        let mut frame = elapsed_ms / self.interval_ms.max(1);
        if frame >= self.frames.len() as u64 {
            if !(self.repeat || persistent || self.kind == "projectile") {
                return None;
            }
            frame %= self.frames.len() as u64;
        }
        self.frames.get(frame as usize)
    }
}

#[derive(Debug)]
struct EffectCatalog {
    libraries: HashMap<String, LibraryMeta>,
    spells: HashMap<String, EffectSpec>,
    ground: HashMap<String, EffectSpec>,
    mapped: HashMap<String, EffectSpec>,
    effect_names: HashMap<u32, String>,
    spell_names: HashMap<u32, String>,
}

impl EffectCatalog {
    fn load() -> Option<Self> {
        let manifest: EffectsManifest = serde_json::from_str(MANIFEST_JSON).ok()?;
        let libraries = [
            ("Effect", EFFECT_META_JSON),
            ("Magic", MAGIC_META_JSON),
            ("Magic2", MAGIC2_META_JSON),
            ("Magic3", MAGIC3_META_JSON),
        ]
        .into_iter()
        .map(|(name, raw)| Some((name.to_owned(), serde_json::from_str(raw).ok()?)))
        .collect::<Option<HashMap<_, _>>>()?;
        let EffectsManifest {
            spell_effect_map,
            spell_effects,
            ground_effects,
            client_effects,
            object_effects,
            map_effects,
        } = manifest;
        let mut spells = HashMap::new();
        let mut ground = HashMap::new();
        let mut mapped = HashMap::new();
        let mut spell_names = HashMap::new();
        for spec in spell_effects {
            if let Some(name) = spec.spell.clone() {
                if let Some(id) = spec.spell_id {
                    spell_names.insert(id, name.clone());
                }
                spells.insert(name, spec);
            }
        }
        for spec in ground_effects {
            if let Some(name) = spec.spell.clone() {
                if let Some(id) = spec.spell_id {
                    spell_names.insert(id, name.clone());
                }
                ground.insert(name, spec);
            }
        }
        for spec in client_effects
            .into_iter()
            .chain(object_effects)
            .chain(map_effects)
        {
            if let Some(name) = spec.effect.clone() {
                mapped.insert(name, spec);
            }
        }
        let effect_names = spell_effect_map
            .into_iter()
            .map(|entry| (entry.id, entry.name))
            .collect();
        Some(Self {
            libraries,
            spells,
            ground,
            mapped,
            effect_names,
            spell_names,
        })
    }

    fn resolve_frames(&self, library: &str, base: i64, count: i64) -> Option<Vec<EffectFrame>> {
        if !(1..=MAX_EFFECT_FRAMES as i64).contains(&count) {
            return None;
        }
        let library_meta = self.libraries.get(library)?;
        let prefix = format!("/original-effects/{library}/");
        let frames = (0..count)
            .map(|offset| {
                library_meta
                    .frames
                    .get(&(base + offset).to_string())
                    .cloned()
            })
            .collect::<Option<Vec<_>>>()?;
        frames
            .iter()
            .all(|frame| {
                frame.path.starts_with(&prefix)
                    && frame.path.ends_with(".png")
                    && frame.width.is_finite()
                    && frame.height.is_finite()
                    && (1.0..=2048.0).contains(&frame.width)
                    && (1.0..=2048.0).contains(&frame.height)
                    && frame.x.is_finite()
                    && frame.y.is_finite()
            })
            .then_some(frames)
    }

    fn animation(&self, spec: &EffectSpec, direction: u32, value: u32) -> Option<Animation> {
        if spec
            .direction_count
            .is_some_and(|count| count <= 0 || direction >= count as u32)
            || spec
                .value_count
                .is_some_and(|count| count <= 0 || value >= count as u32)
        {
            return None;
        }
        let base = spec.base
            + i64::from(direction) * spec.direction_stride.unwrap_or(0)
            + i64::from(value) * spec.value_stride.unwrap_or(0);
        self.build_animation(
            &spec.library,
            base,
            spec.count,
            spec.interval,
            spec.kind.as_deref().unwrap_or("impact"),
            spec.blend,
            spec.rate,
            spec.repeat,
            spec.offset,
        )
    }

    fn sub_animation(
        &self,
        spec: &SubSpec,
        direction: u32,
        fallback_kind: &str,
    ) -> Option<Animation> {
        if spec
            .direction_count
            .is_some_and(|count| count <= 0 || direction >= count as u32)
        {
            return None;
        }
        self.build_animation(
            &spec.library,
            spec.base + i64::from(direction) * spec.direction_stride.unwrap_or(0),
            spec.count,
            spec.interval.unwrap_or(100),
            spec.kind.as_deref().unwrap_or(fallback_kind),
            spec.blend,
            spec.rate,
            spec.repeat,
            spec.offset,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build_animation(
        &self,
        library: &str,
        base: i64,
        count: i64,
        interval: i64,
        kind: &str,
        blend: Option<bool>,
        rate: Option<f32>,
        repeat: Option<bool>,
        offset: Option<EffectOffset>,
    ) -> Option<Animation> {
        let interval_ms = u64::try_from(interval).ok()?.max(1);
        let frames = self.resolve_frames(library, base, count)?;
        let duration_ms = interval_ms.checked_mul(frames.len() as u64)?;
        Some(Animation {
            kind: kind.to_owned(),
            frames,
            interval_ms,
            duration_ms,
            additive: blend.unwrap_or(true),
            opacity: rate.unwrap_or(1.0).clamp(0.0, 1.0),
            repeat: repeat.unwrap_or(false),
            offset: offset.unwrap_or_default(),
        })
    }

    fn mapped_animation(&self, effect: u32, value: u32) -> Option<Animation> {
        let name = self.effect_names.get(&effect)?;
        self.mapped
            .get(name)
            .or_else(|| self.ground.get(name))
            .and_then(|spec| self.animation(spec, 0, value))
    }

    fn cast_animation(&self, spell: &str, direction: u32) -> Option<Animation> {
        let spec = self.spells.get(spell)?;
        (!matches!(
            spec.kind.as_deref(),
            Some("projectile" | "impact" | "target" | "attackOverlay")
        ))
        .then(|| self.animation(spec, direction, 0))
        .flatten()
    }

    fn world_animation(&self, spell: u32, direction: u32, value: u32) -> Option<Animation> {
        let name = self.spell_names.get(&spell)?;
        self.mapped
            .get(name)
            .or_else(|| self.ground.get(name))
            .or_else(|| self.spells.get(name))
            .and_then(|spec| self.animation(spec, direction, value))
    }

    fn projectile_phases(&self, spell: &str, direction: u32) -> Option<Vec<Animation>> {
        let spec = self.spells.get(spell)?;
        let mut phases = Vec::new();
        if let Some(animation) = spec
            .projectile
            .as_ref()
            .and_then(|sub| self.sub_animation(sub, direction, "projectile"))
        {
            phases.push(animation);
        }
        if let Some(animation) = spec
            .impact
            .as_ref()
            .and_then(|sub| self.sub_animation(sub, 0, "impact"))
        {
            phases.push(animation);
        }
        if let Some(animation) = spec
            .return_effect
            .as_ref()
            .and_then(|sub| self.sub_animation(sub, 0, "return"))
        {
            phases.push(animation);
        }
        (!phases.is_empty()).then_some(phases)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EffectPacketOutcome {
    Applied,
    Ignored,
    Rejected,
}

#[derive(Debug)]
struct ActiveEffect {
    key: String,
    from: Option<(f32, f32)>,
    to: (f32, f32),
    start_at_ms: u64,
    phases: Vec<Animation>,
    persistent: bool,
    anchor_object_id: Option<u32>,
    source_object_id: Option<u32>,
    destination_object_id: Option<u32>,
}

#[derive(Resource)]
pub(crate) struct SceneEffects {
    catalog: Option<EffectCatalog>,
    active: Vec<ActiveEffect>,
    sequence: u64,
    last_state: Option<String>,
}

impl Default for SceneEffects {
    fn default() -> Self {
        Self {
            catalog: EffectCatalog::load(),
            active: Vec::new(),
            sequence: 0,
            last_state: None,
        }
    }
}

impl SceneEffects {
    pub(crate) fn clear(&mut self) {
        self.active.clear();
        self.last_state = None;
    }

    pub(crate) fn observe_packet(
        &mut self,
        raw: &str,
        now_ms: u64,
        actor_positions: &HashMap<u32, (i32, i32)>,
    ) -> EffectPacketOutcome {
        if raw.is_empty() || raw.len() > MAX_PACKET_BYTES {
            return EffectPacketOutcome::Rejected;
        }
        let Ok(envelope) = serde_json::from_str::<Value>(raw) else {
            return EffectPacketOutcome::Rejected;
        };
        if envelope.get("type").and_then(Value::as_str) != Some("packet") {
            return EffectPacketOutcome::Rejected;
        }
        let Some(packet) = envelope.get("packet").and_then(Value::as_str) else {
            return EffectPacketOutcome::Rejected;
        };
        let Some(payload) = envelope.get("payload").and_then(Value::as_object) else {
            return EffectPacketOutcome::Rejected;
        };
        let body = payload
            .get("info")
            .and_then(Value::as_object)
            .unwrap_or(payload);
        match packet {
            "ObjectMagic" => self.object_magic(body, now_ms),
            "ObjectProjectile" => self.object_projectile(body, now_ms, actor_positions),
            "ObjectEffect" => self.object_effect(body, now_ms, actor_positions),
            "MapEffect" => self.map_effect(body, now_ms),
            "ObjectSpell" => self.object_spell(body, now_ms),
            "ObjectRemove" | "ObjectHide" => {
                let Some(object_id) = unsigned_u32(body, "objectId").filter(|id| *id != 0) else {
                    return EffectPacketOutcome::Ignored;
                };
                let before = self.active.len();
                self.active.retain(|effect| {
                    effect.anchor_object_id != Some(object_id)
                        && effect.source_object_id != Some(object_id)
                        && effect.destination_object_id != Some(object_id)
                });
                if self.active.len() != before {
                    EffectPacketOutcome::Applied
                } else {
                    EffectPacketOutcome::Ignored
                }
            }
            _ => EffectPacketOutcome::Ignored,
        }
    }

    fn object_magic(&mut self, body: &Map<String, Value>, now_ms: u64) -> EffectPacketOutcome {
        let (
            Some(object_id),
            Some(location),
            Some(direction),
            Some(spell),
            Some(_target_id),
            Some(_target),
            Some(_cast),
            Some(_level),
            Some(_self_broadcast),
        ) = (
            unsigned_u32(body, "objectId").filter(|id| *id != 0),
            point(body, "location"),
            string(body, "direction", 16).and_then(direction_index),
            string(body, "spell", 64),
            unsigned_u32(body, "targetId"),
            point(body, "target"),
            body.get("cast").and_then(Value::as_bool),
            unsigned_u32(body, "level"),
            body.get("selfBroadcast").and_then(Value::as_bool),
        )
        else {
            return EffectPacketOutcome::Rejected;
        };
        if body
            .get("secondaryTargetIds")
            .and_then(Value::as_array)
            .is_none_or(|ids| {
                ids.len() > 128
                    || ids
                        .iter()
                        .any(|id| id.as_u64().and_then(|id| u32::try_from(id).ok()).is_none())
            })
        {
            return EffectPacketOutcome::Rejected;
        }
        let Some(animation) = self
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.cast_animation(spell, direction))
        else {
            return EffectPacketOutcome::Ignored;
        };
        let key = self.next_key("cast");
        self.push(ActiveEffect {
            key,
            from: None,
            to: (location.0 as f32, location.1 as f32),
            start_at_ms: now_ms,
            phases: vec![animation],
            persistent: false,
            anchor_object_id: Some(object_id),
            source_object_id: Some(object_id),
            destination_object_id: None,
        });
        EffectPacketOutcome::Applied
    }

    fn object_projectile(
        &mut self,
        body: &Map<String, Value>,
        now_ms: u64,
        actor_positions: &HashMap<u32, (i32, i32)>,
    ) -> EffectPacketOutcome {
        let (Some(spell), Some(source_id), Some(destination_id)) = (
            string(body, "spell", 64),
            unsigned_u32(body, "sourceId").filter(|id| *id != 0),
            unsigned_u32(body, "destinationId").filter(|id| *id != 0),
        ) else {
            return EffectPacketOutcome::Rejected;
        };
        let (Some(source), Some(destination)) = (
            actor_positions.get(&source_id).copied(),
            actor_positions.get(&destination_id).copied(),
        ) else {
            return EffectPacketOutcome::Ignored;
        };
        let direction = projectile_direction16(source, destination);
        let Some(mut phases) = self
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.projectile_phases(spell, direction))
        else {
            return EffectPacketOutcome::Ignored;
        };
        if let Some(projectile) = phases.iter_mut().find(|phase| phase.kind == "projectile") {
            let distance = u64::from(
                source
                    .0
                    .abs_diff(destination.0)
                    .max(source.1.abs_diff(destination.1)),
            );
            projectile.duration_ms = distance.saturating_mul(PROJECTILE_TILE_MS).max(1);
            let ticks = (projectile.duration_ms / projectile.interval_ms.max(1)).max(1);
            projectile.interval_ms = (projectile.duration_ms / ticks).max(1);
        }
        let key = self.next_key("projectile");
        self.push(ActiveEffect {
            key,
            from: Some((source.0 as f32, source.1 as f32)),
            to: (destination.0 as f32, destination.1 as f32),
            start_at_ms: now_ms,
            phases,
            persistent: false,
            anchor_object_id: None,
            source_object_id: Some(source_id),
            destination_object_id: Some(destination_id),
        });
        EffectPacketOutcome::Applied
    }

    fn object_effect(
        &mut self,
        body: &Map<String, Value>,
        now_ms: u64,
        actor_positions: &HashMap<u32, (i32, i32)>,
    ) -> EffectPacketOutcome {
        let (Some(object_id), Some(effect), Some(_effect_type), Some(delay_ms), Some(_time)) = (
            unsigned_u32(body, "objectId").filter(|id| *id != 0),
            unsigned_u32(body, "effect"),
            unsigned_u32(body, "effectType"),
            unsigned_u32(body, "delayTime"),
            unsigned_u32(body, "time"),
        ) else {
            return EffectPacketOutcome::Rejected;
        };
        let Some(position) = actor_positions.get(&object_id).copied() else {
            return EffectPacketOutcome::Ignored;
        };
        let Some(animation) = self
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.mapped_animation(effect, 0))
        else {
            return EffectPacketOutcome::Ignored;
        };
        let key = self.next_key("object");
        self.push(ActiveEffect {
            key,
            from: None,
            to: (position.0 as f32, position.1 as f32),
            // Crystal's Healing branch attaches immediately; generic object
            // effects honor the authoritative delayTime value.
            start_at_ms: now_ms.saturating_add(u64::from(if effect == 3 {
                0
            } else {
                delay_ms.min(60_000)
            })),
            phases: vec![animation],
            persistent: false,
            anchor_object_id: Some(object_id),
            source_object_id: None,
            destination_object_id: None,
        });
        EffectPacketOutcome::Applied
    }

    fn map_effect(&mut self, body: &Map<String, Value>, now_ms: u64) -> EffectPacketOutcome {
        let (Some(location), Some(effect), Some(value)) = (
            point(body, "location"),
            unsigned_u32(body, "effect"),
            unsigned_u32(body, "value"),
        ) else {
            return EffectPacketOutcome::Rejected;
        };
        let Some(animation) = self
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.mapped_animation(effect, value))
        else {
            return EffectPacketOutcome::Ignored;
        };
        let key = self.next_key("map");
        self.push(ActiveEffect {
            key,
            from: None,
            to: (location.0 as f32, location.1 as f32),
            start_at_ms: now_ms,
            phases: vec![animation],
            persistent: false,
            anchor_object_id: None,
            source_object_id: None,
            destination_object_id: None,
        });
        EffectPacketOutcome::Applied
    }

    fn object_spell(&mut self, body: &Map<String, Value>, now_ms: u64) -> EffectPacketOutcome {
        let (Some(object_id), Some(location), Some(spell), Some(direction), Some(param)) = (
            unsigned_u32(body, "objectId").filter(|id| *id != 0),
            point(body, "location"),
            unsigned_u32(body, "spell"),
            string(body, "direction", 16).and_then(direction_index),
            unsigned_u32(body, "param"),
        ) else {
            return EffectPacketOutcome::Rejected;
        };
        let Some(mut animation) = self
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.world_animation(spell, direction, param))
        else {
            return EffectPacketOutcome::Ignored;
        };
        animation.repeat = true;
        self.active
            .retain(|effect| !(effect.persistent && effect.anchor_object_id == Some(object_id)));
        self.push(ActiveEffect {
            key: format!("spell-{object_id}"),
            from: None,
            to: (location.0 as f32, location.1 as f32),
            start_at_ms: now_ms,
            phases: vec![animation],
            persistent: true,
            anchor_object_id: Some(object_id),
            source_object_id: None,
            destination_object_id: None,
        });
        EffectPacketOutcome::Applied
    }

    fn next_key(&mut self, prefix: &str) -> String {
        self.sequence = self.sequence.wrapping_add(1).max(1);
        format!("android-{prefix}-{}", self.sequence)
    }

    fn push(&mut self, effect: ActiveEffect) {
        while self.active.len() >= MAX_ACTIVE_EFFECTS {
            let index = self
                .active
                .iter()
                .position(|active| !active.persistent)
                .unwrap_or(0);
            self.active.remove(index);
        }
        self.active.push(effect);
    }

    pub(crate) fn tick(
        &mut self,
        now_ms: u64,
        center: Option<(i32, i32)>,
        actor_positions: &HashMap<u32, (i32, i32)>,
        visible: bool,
    ) -> Option<String> {
        for effect in &mut self.active {
            if let Some(position) = effect
                .anchor_object_id
                .and_then(|id| actor_positions.get(&id))
            {
                effect.to = (position.0 as f32, position.1 as f32);
            }
            if let Some(position) = effect
                .source_object_id
                .and_then(|id| actor_positions.get(&id))
            {
                if effect.from.is_some() {
                    effect.from = Some((position.0 as f32, position.1 as f32));
                }
            }
            if let Some(position) = effect
                .destination_object_id
                .and_then(|id| actor_positions.get(&id))
            {
                effect.to = (position.0 as f32, position.1 as f32);
            }
        }
        let rendered = center
            .filter(|_| visible)
            .map(|center| {
                self.active
                    .iter()
                    .filter_map(|effect| render_entry(effect, now_ms, center))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.active.retain(|effect| {
            effect.persistent
                || effect.phases.iter().any(|phase| phase.repeat)
                || effect_end_ms(effect) > now_ms
        });
        let preload_image_urls = preload_image_urls(&self.active);
        let state = json!({
            "enabled": visible && center.is_some(),
            "stageWidth": STAGE_WIDTH,
            "stageHeight": STAGE_HEIGHT,
            "preloadImageUrls": preload_image_urls,
            "effects": rendered,
        })
        .to_string();
        if self.last_state.as_deref() == Some(&state) {
            return None;
        }
        self.last_state = Some(state.clone());
        Some(state)
    }
}

fn preload_image_urls(effects: &[ActiveEffect]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut urls = Vec::new();
    for frame in effects
        .iter()
        .flat_map(|effect| &effect.phases)
        .flat_map(|phase| &phase.frames)
    {
        for url in std::iter::once(&frame.path).chain(frame.mask_path.iter()) {
            if seen.insert(url.as_str()) {
                urls.push(url.clone());
                if urls.len() == MAX_PRELOAD_IMAGES {
                    return urls;
                }
            }
        }
    }
    urls
}

fn effect_end_ms(effect: &ActiveEffect) -> u64 {
    effect.phases.iter().fold(effect.start_at_ms, |end, phase| {
        end.saturating_add(phase.duration_ms)
    })
}

fn active_phase(effect: &ActiveEffect, now_ms: u64) -> Option<(&Animation, u64)> {
    if now_ms < effect.start_at_ms {
        return None;
    }
    let mut start = effect.start_at_ms;
    for phase in &effect.phases {
        let end = start.saturating_add(phase.duration_ms);
        if effect.persistent || phase.repeat || now_ms < end {
            return Some((phase, start));
        }
        start = end;
    }
    None
}

fn render_entry(effect: &ActiveEffect, now_ms: u64, center: (i32, i32)) -> Option<Value> {
    let (animation, phase_start) = active_phase(effect, now_ms)?;
    let elapsed = now_ms.saturating_sub(phase_start);
    let frame = animation.frame_at(elapsed, effect.persistent)?;
    let (tile_x, tile_y) = if animation.kind == "projectile" {
        let from = effect.from?;
        let progress = (elapsed as f32 / animation.duration_ms.max(1) as f32).clamp(0.0, 1.0);
        (
            from.0 + (effect.to.0 - from.0) * progress,
            from.1 + (effect.to.1 - from.1) * progress,
        )
    } else if animation.kind == "return" {
        let from = effect.from?;
        let progress = (elapsed as f32 / animation.duration_ms.max(1) as f32).clamp(0.0, 1.0);
        (
            effect.to.0 + (from.0 - effect.to.0) * progress,
            effect.to.1 + (from.1 - effect.to.1) * progress,
        )
    } else {
        effect.to
    };
    let left =
        ENTITY_ORIGIN_X + (tile_x - center.0 as f32) * CELL_WIDTH + frame.x + animation.offset.x;
    let top =
        ENTITY_ORIGIN_Y + (tile_y - center.1 as f32) * CELL_HEIGHT + frame.y + animation.offset.y;
    let transient =
        !matches!(animation.kind.as_str(), "ground" | "persistent") && !effect.persistent;
    let z = (tile_y * 1000.0 + tile_x * 10.0) * EFFECT_DEPTH_GAIN
        + if transient {
            EFFECT_TRANSIENT_ORDER
        } else {
            EFFECT_GROUND_ORDER
        };
    let mut entry = Map::from_iter([
        ("key".into(), json!(effect.key)),
        ("imageUrl".into(), json!(frame.path)),
        ("left".into(), json!(left)),
        ("top".into(), json!(top)),
        ("width".into(), json!(frame.width)),
        ("height".into(), json!(frame.height)),
        ("z".into(), json!(z)),
        ("additive".into(), json!(animation.additive)),
        ("opacity".into(), json!(animation.opacity)),
    ]);
    if let Some(mask) = frame.mask_path.as_ref() {
        entry.insert("maskImageUrl".into(), json!(mask));
        entry.insert("frameX".into(), json!(frame.x));
        entry.insert("frameY".into(), json!(frame.y));
        for (name, value) in [
            ("maskWidth", frame.mask_width),
            ("maskHeight", frame.mask_height),
            ("maskX", frame.mask_x),
            ("maskY", frame.mask_y),
        ] {
            if let Some(value) = value {
                entry.insert(name.into(), json!(value));
            }
        }
    }
    if let (Some(x), Some(y)) = (frame.shadow_x, frame.shadow_y) {
        entry.insert("shadowX".into(), json!(x));
        entry.insert("shadowY".into(), json!(y));
    }
    Some(Value::Object(entry))
}

fn unsigned_u32(body: &Map<String, Value>, key: &str) -> Option<u32> {
    u32::try_from(body.get(key)?.as_u64()?).ok()
}

fn string<'a>(body: &'a Map<String, Value>, key: &str, max: usize) -> Option<&'a str> {
    body.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= max)
}

fn point(body: &Map<String, Value>, key: &str) -> Option<(i32, i32)> {
    let point = body.get(key)?.as_object()?;
    let x = i32::try_from(point.get("x")?.as_i64()?).ok()?;
    let y = i32::try_from(point.get("y")?.as_i64()?).ok()?;
    (x >= 0 && y >= 0).then_some((x, y))
}

fn direction_index(direction: &str) -> Option<u32> {
    Some(match direction.to_ascii_lowercase().as_str() {
        "up" => 0,
        "upright" => 1,
        "right" => 2,
        "downright" => 3,
        "down" => 4,
        "downleft" => 5,
        "left" => 6,
        "upleft" => 7,
        _ => return None,
    })
}

fn projectile_direction16(source: (i32, i32), destination: (i32, i32)) -> u32 {
    let dx = (destination.0 - source.0) as f32;
    let dy = (destination.1 - source.1) as f32;
    if dx == 0.0 && dy == 0.0 {
        return 0;
    }
    let degrees = dx.atan2(-dy).to_degrees().rem_euclid(360.0);
    (((degrees + 11.25).rem_euclid(360.0) / 22.5).floor() as u32) % 16
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<SceneEffects>();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn positions() -> HashMap<u32, (i32, i32)> {
        HashMap::from([(42, (302, 634)), (77, (299, 629))])
    }

    #[test]
    fn catalog_resolves_exact_crystal_frames() {
        let catalog = EffectCatalog::load().expect("tracked effect catalog");
        let healing = catalog.mapped_animation(3, 0).expect("Healing");
        assert_eq!(healing.frames.len(), 10);
        assert_eq!(healing.frames[0].path, "/original-effects/Magic/370.png");
        assert!(healing.additive);
        let fire_wall = catalog.world_animation(39, 4, 0).expect("FireWall");
        assert_eq!(fire_wall.frames[0].path, "/original-effects/Magic/1630.png");
        assert!(fire_wall.repeat);
    }

    #[test]
    fn authoritative_effect_packets_render_without_fallbacks() {
        let mut effects = SceneEffects::default();
        assert_eq!(
            effects.observe_packet(
                r#"{"type":"packet","packet":"ObjectMagic","payload":{"objectId":42,"location":{"x":302,"y":634},"direction":"Down","spell":"FireBall","targetId":77,"target":{"x":299,"y":629},"cast":true,"level":1,"selfBroadcast":false,"secondaryTargetIds":[]}}"#,
                0,
                &positions(),
            ),
            EffectPacketOutcome::Applied
        );
        let cast: Value = serde_json::from_str(
            &effects
                .tick(0, Some((302, 634)), &positions(), true)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            cast["effects"][0]["imageUrl"],
            "/original-effects/Magic/0.png"
        );
        assert!(cast["preloadImageUrls"]
            .as_array()
            .is_some_and(|urls| urls.len() > 1));
        assert_eq!(
            effects.observe_packet(
                r#"{"type":"packet","packet":"ObjectEffect","payload":{"objectId":77,"effect":3,"effectType":0,"delayTime":900,"time":0}}"#,
                100,
                &positions(),
            ),
            EffectPacketOutcome::Applied
        );
        let state: Value = serde_json::from_str(
            &effects
                .tick(100, Some((302, 634)), &positions(), true)
                .unwrap(),
        )
        .unwrap();
        let healing = state["effects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["imageUrl"] == "/original-effects/Magic/370.png")
            .expect("Healing frame");
        assert_eq!(healing["left"], 323.0);
        assert_eq!(healing["top"], 107.0);
        assert_eq!(healing["additive"], true);

        assert_eq!(
            effects.observe_packet(
                r#"{"type":"packet","packet":"MapEffect","payload":{"location":{"x":302,"y":634},"effect":12,"value":0}}"#,
                200,
                &positions(),
            ),
            EffectPacketOutcome::Applied
        );
        let state: Value = serde_json::from_str(
            &effects
                .tick(200, Some((302, 634)), &positions(), true)
                .unwrap(),
        )
        .unwrap();
        assert!(state["effects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| { entry["imageUrl"] == "/original-effects/Effect/0.png" }));
    }

    #[test]
    fn projectile_interpolates_and_object_spell_is_removed_by_lifecycle() {
        let mut effects = SceneEffects::default();
        assert_eq!(
            effects.observe_packet(
                r#"{"type":"packet","packet":"ObjectProjectile","payload":{"spell":"FireBall","sourceId":42,"destinationId":77}}"#,
                0,
                &positions(),
            ),
            EffectPacketOutcome::Applied
        );
        let launch: Value = serde_json::from_str(
            &effects
                .tick(0, Some((302, 634)), &positions(), true)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            launch["effects"][0]["imageUrl"],
            "/original-effects/Magic/160.png"
        );
        let later: Value = serde_json::from_str(
            &effects
                .tick(125, Some((302, 634)), &positions(), true)
                .unwrap(),
        )
        .unwrap();
        assert_ne!(launch["effects"][0]["left"], later["effects"][0]["left"]);

        assert_eq!(
            effects.observe_packet(
                r#"{"type":"packet","packet":"ObjectSpell","payload":{"objectId":501,"location":{"x":302,"y":634},"spell":39,"direction":"Down","param":0}}"#,
                200,
                &positions(),
            ),
            EffectPacketOutcome::Applied
        );
        let persistent: Value = serde_json::from_str(
            &effects
                .tick(2_000, Some((302, 634)), &positions(), true)
                .unwrap(),
        )
        .unwrap();
        assert!(persistent["effects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| { entry["key"] == "spell-501" }));
        effects.observe_packet(
            r#"{"type":"packet","packet":"ObjectRemove","payload":{"objectId":501}}"#,
            2_001,
            &positions(),
        );
        let removed: Value = serde_json::from_str(
            &effects
                .tick(2_001, Some((302, 634)), &positions(), true)
                .unwrap(),
        )
        .unwrap();
        assert!(!removed["effects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| { entry["key"] == "spell-501" }));
    }

    #[test]
    fn malformed_or_unknown_effects_never_draw_placeholders() {
        let mut effects = SceneEffects::default();
        assert_eq!(
            effects.observe_packet(
                r#"{"type":"packet","packet":"MapEffect","payload":{"location":{"x":1,"y":2},"effect":999,"value":0}}"#,
                0,
                &HashMap::new(),
            ),
            EffectPacketOutcome::Ignored
        );
        assert_eq!(
            effects.observe_packet(
                r#"{"type":"packet","packet":"MapEffect","payload":{"effect":12,"value":0}}"#,
                0,
                &HashMap::new(),
            ),
            EffectPacketOutcome::Rejected
        );
        let state: Value = serde_json::from_str(
            &effects
                .tick(0, Some((1, 2)), &HashMap::new(), true)
                .unwrap(),
        )
        .unwrap();
        assert!(state["effects"].as_array().unwrap().is_empty());
    }
}
