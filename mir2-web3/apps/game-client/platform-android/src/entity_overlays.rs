//! Android-only Crystal actor labels and authoritative health feedback.
//!
//! The shared renderer owns actor sprites. This adapter only projects fields
//! already present in the validated renderer-neutral model. It never invents
//! actors, exact HP, targeting state, collision, or combat outcomes.

use bevy::{prelude::*, text::LineBreak, ui::FocusPolicy};
use mir2_client_bevy::{
    crystal_ui::{
        overlays::NativePlayerUiState,
        typography::{crystal_text_font, CRYSTAL_DEFAULT_FONT_SIZE_PX},
    },
    native_shell::{NativeShellModel, NativeShellScreen},
    read_model::UiReadModel,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};

const STAGE_WIDTH: f32 = 1024.0;
const STAGE_HEIGHT: f32 = 768.0;
const CELL_WIDTH: f32 = 48.0;
const CELL_HEIGHT: f32 = 32.0;
const ENTITY_LEFT_ORIGIN: f32 = 480.0;
const ENTITY_TOP_ORIGIN: f32 = 352.0;
const PLAYER_NAME_TOP: f32 = -17.0;
const PLAYER_GUILD_TOP: f32 = -5.0;
const NPC_MONSTER_NAME_TOP: f32 = -18.0;
const SPLIT_NAME_STEP: f32 = 12.0;
const CORPSE_NAME_SHIFT: f32 = 27.0;
const HEALTH_LEFT: f32 = 8.0;
const HEALTH_TOP: f32 = -64.0;
const HEALTH_WIDTH: f32 = 32.0;
const HEALTH_HEIGHT: f32 = 4.0;
const OVERLAY_Z_INDEX: i32 = 850;
const MAX_MODEL_BYTES: usize = 2 * 1024 * 1024;
const MAX_WORLD_OBJECTS: usize = 8192;
const MAX_VISIBLE_ACTORS: usize = 256;
const MAX_DAMAGE_FLOATERS: usize = 48;
const MAX_DAMAGE_FLOATERS_PER_ACTOR: usize = 10;

#[derive(Component)]
pub(crate) struct ActorOverlayRoot;

#[derive(Component)]
pub(crate) struct DamageOverlayRoot;

#[derive(Component)]
struct ActorNameLine;

#[derive(Component)]
struct ActorHealthBar;

#[derive(Component)]
struct DamageFloaterNode(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActorKind {
    SelfPlayer,
    Player,
    Monster,
    Npc,
}

#[derive(Clone, Debug, PartialEq)]
struct ActorOverlay {
    object_id: u32,
    kind: ActorKind,
    name: String,
    guild_name: Option<String>,
    color: [u8; 4],
    x: i32,
    y: i32,
    image: Option<u16>,
    dead: bool,
    health: Option<HealthPacket>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HealthPacket {
    percent: u8,
    expire_seconds: u8,
    generation: u64,
    revision: u64,
    hit_revision: Option<u64>,
}

#[derive(Resource, Debug, Default)]
pub(crate) struct ActorOverlayModel {
    center: Option<(i32, i32)>,
    actors: Vec<ActorOverlay>,
    revision: u64,
    active_floaters: VecDeque<ActiveDamageFloater>,
    last_damage_sequence: u64,
}

impl ActorOverlayModel {
    pub(crate) fn center(&self) -> Option<(i32, i32)> {
        self.center
    }

    pub(crate) fn actor_positions(&self) -> HashMap<u32, (i32, i32)> {
        self.actors
            .iter()
            .map(|actor| (actor.object_id, (actor.x, actor.y)))
            .collect()
    }

    pub(crate) fn replace(&mut self, projected: ProjectedActorOverlays) {
        if self.center == Some(projected.center) && self.actors == projected.actors {
            return;
        }
        self.center = Some(projected.center);
        self.actors = projected.actors;
        self.revision = self.revision.wrapping_add(1);
    }

    pub(crate) fn reset(&mut self) {
        if self.center.is_none()
            && self.actors.is_empty()
            && self.active_floaters.is_empty()
            && self.last_damage_sequence == 0
        {
            return;
        }
        self.center = None;
        self.actors.clear();
        self.active_floaters.clear();
        self.last_damage_sequence = 0;
        self.revision = self.revision.wrapping_add(1);
    }

    pub(crate) fn observe_damage_events(
        &mut self,
        events: impl IntoIterator<Item = crate::live_entity::LiveDamageEvent>,
        now_ms: u64,
    ) {
        self.active_floaters
            .retain(|floater| floater.expires_at_ms > now_ms);
        for event in events {
            if event.sequence <= self.last_damage_sequence {
                continue;
            }
            self.last_damage_sequence = event.sequence;
            let variant = if event.damage_type == 1 {
                DamageVariant::Miss
            } else if event.damage_type == 2 {
                DamageVariant::Critical
            } else if event.damage_type != 0 && event.damage > 0 {
                DamageVariant::Heal
            } else {
                DamageVariant::Hit
            };
            let text = match variant {
                DamageVariant::Miss => "Miss".to_owned(),
                DamageVariant::Critical if event.damage == 0 => "Crit".to_owned(),
                DamageVariant::Heal => format!("+{}", event.damage),
                DamageVariant::Hit | DamageVariant::Critical => {
                    event.damage.unsigned_abs().to_string()
                }
            };
            while self
                .active_floaters
                .iter()
                .filter(|floater| floater.object_id == event.object_id)
                .count()
                >= MAX_DAMAGE_FLOATERS_PER_ACTOR
            {
                if let Some(index) = self
                    .active_floaters
                    .iter()
                    .position(|floater| floater.object_id == event.object_id)
                {
                    self.active_floaters.remove(index);
                }
            }
            while self.active_floaters.len() >= MAX_DAMAGE_FLOATERS {
                self.active_floaters.pop_front();
            }
            let duration_ms = if variant == DamageVariant::Miss {
                1_600
            } else {
                1_800
            };
            self.active_floaters.push_back(ActiveDamageFloater {
                sequence: event.sequence,
                object_id: event.object_id,
                text,
                variant,
                started_at_ms: now_ms,
                expires_at_ms: now_ms.saturating_add(duration_ms),
            });
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DamageVariant {
    Hit,
    Miss,
    Critical,
    Heal,
}

#[derive(Clone, Debug)]
struct ActiveDamageFloater {
    sequence: u64,
    object_id: u32,
    text: String,
    variant: DamageVariant,
    started_at_ms: u64,
    expires_at_ms: u64,
}

#[derive(Clone, Debug)]
struct DamageFloaterEntry {
    sequence: u64,
    text: String,
    color: Color,
    left: f32,
    top: f32,
    font_size: f32,
}

#[derive(Debug)]
pub(crate) struct ProjectedActorOverlays {
    center: (i32, i32),
    actors: Vec<ActorOverlay>,
}

pub(crate) fn project(
    models_json: &str,
    center_x: i32,
    center_y: i32,
) -> Option<ProjectedActorOverlays> {
    if models_json.len() > MAX_MODEL_BYTES {
        return None;
    }
    let models: Value = serde_json::from_str(models_json).ok()?;
    let entities = models.get("entities")?.as_array()?;
    let drops = match models.get("groundDrops") {
        None => &[][..],
        Some(value) => value.as_array()?.as_slice(),
    };
    if entities
        .len()
        .checked_add(drops.len())
        .is_none_or(|count| count > MAX_WORLD_OBJECTS)
    {
        return None;
    }

    let mut ids = HashSet::with_capacity(entities.len().saturating_add(drops.len()));
    let mut actors = Vec::new();
    for entity in entities {
        let object_id = object_id(entity.get("objectId")?)?;
        let x = coordinate(entity.get("x")?)?;
        let y = coordinate(entity.get("y")?)?;
        let kind = match entity.get("kind")?.as_str()? {
            "selfPlayer" => ActorKind::SelfPlayer,
            "player" => ActorKind::Player,
            "monster" => ActorKind::Monster,
            "npc" => ActorKind::Npc,
            _ => return None,
        };
        let name = bounded_text(entity.get("name")?)?;
        let guild_name = match entity.get("guildName") {
            None | Some(Value::Null) => None,
            Some(value) => {
                let value = value.as_str()?.trim();
                if value.chars().count() > 128 {
                    return None;
                }
                (!value.is_empty()).then(|| value.to_owned())
            }
        };
        let name_color = match entity.get("nameColourArgb") {
            None | Some(Value::Null) => None,
            Some(value) => Some(i32::try_from(value.as_i64()?).ok()?),
        };
        let image = match entity.get("image") {
            None | Some(Value::Null) => None,
            Some(value) => Some(u16::try_from(value.as_u64()?).ok()?),
        };
        let dead = match entity.get("dead") {
            None | Some(Value::Null) => false,
            Some(value) => value.as_bool()?,
        };
        let health = health_packet(entity)?;
        if !ids.insert(object_id) {
            return None;
        }
        let dx = x.saturating_sub(center_x);
        let dy = y.saturating_sub(center_y);
        if dx.unsigned_abs() > 24 || dy.unsigned_abs() > 32 {
            continue;
        }
        let fallback = if kind == ActorKind::Npc {
            [0x00, 0xff, 0x00, 0xff]
        } else {
            [0xff, 0xff, 0xff, 0xff]
        };
        actors.push(ActorOverlay {
            object_id,
            kind,
            name,
            guild_name,
            color: name_color.and_then(argb).unwrap_or(fallback),
            x,
            y,
            image,
            dead,
            health,
        });
    }
    for drop in drops {
        let object_id = object_id(drop.get("objectId")?)?;
        if !ids.insert(object_id)
            || coordinate(drop.get("x")?).is_none()
            || coordinate(drop.get("y")?).is_none()
        {
            return None;
        }
    }
    actors.sort_by_key(|actor| (actor.kind != ActorKind::SelfPlayer, actor.object_id));
    actors.truncate(MAX_VISIBLE_ACTORS);
    Some(ProjectedActorOverlays {
        center: (center_x, center_y),
        actors,
    })
}

fn object_id(value: &Value) -> Option<u32> {
    let value = value
        .as_str()
        .and_then(|value| value.parse().ok())
        .or_else(|| value.as_u64().and_then(|value| u32::try_from(value).ok()))?;
    (value != 0).then_some(value)
}

fn coordinate(value: &Value) -> Option<i32> {
    i32::try_from(value.as_i64()?).ok()
}

fn bounded_text(value: &Value) -> Option<String> {
    let text = value.as_str()?.trim();
    (!text.is_empty() && text.chars().count() <= 128).then(|| text.to_owned())
}

fn argb(value: i32) -> Option<[u8; 4]> {
    if value == -1 {
        return None;
    }
    let bits = value as u32;
    let alpha = ((bits >> 24) & 0xff) as u8;
    (alpha != 0).then_some([
        ((bits >> 16) & 0xff) as u8,
        ((bits >> 8) & 0xff) as u8,
        (bits & 0xff) as u8,
        alpha,
    ])
}

fn health_packet(entity: &Value) -> Option<Option<HealthPacket>> {
    let values = [
        entity.get("_healthPercent"),
        entity.get("_healthExpireSeconds"),
        entity.get("_healthGeneration"),
        entity.get("_healthRevision"),
    ];
    if values.iter().all(|value| value.is_none()) {
        return Some(None);
    }
    Some(Some(HealthPacket {
        percent: u8::try_from(values[0]?.as_u64()?).ok()?.min(100),
        expire_seconds: u8::try_from(values[1]?.as_u64()?).ok()?,
        generation: values[2]?.as_u64()?,
        revision: values[3]?.as_u64()?.max(1),
        hit_revision: match entity.get("_healthHitRevision") {
            None | Some(Value::Null) => None,
            Some(value) => Some(value.as_u64()?.max(1)),
        },
    }))
}

#[derive(Default)]
struct HealthWindow {
    generation: u64,
    revision: u64,
    expires_at_ms: u64,
    hit_revision: u64,
    hit_expires_at_ms: u64,
}

#[derive(Default)]
struct HealthWindows(HashMap<u32, HealthWindow>);

#[derive(Debug, PartialEq, Eq)]
struct RenderKey {
    revision: u64,
    names_visible: bool,
    exact_self_hp: Option<(i32, i32)>,
    visible_health: Vec<(u32, u8)>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<ActorOverlayModel>()
        .add_systems(Update, (sync, sync_damage).chain());
}

#[allow(clippy::too_many_arguments)]
fn sync(
    mut commands: Commands,
    shell: Res<NativeShellModel>,
    model: Res<ActorOverlayModel>,
    player: Option<Res<NativePlayerUiState>>,
    read_model: Option<Res<UiReadModel>>,
    time: Res<Time>,
    roots: Query<Entity, With<ActorOverlayRoot>>,
    mut health_windows: Local<HealthWindows>,
    mut rendered: Local<Option<RenderKey>>,
) {
    let names_visible = player
        .as_deref()
        .is_none_or(|state| state.core.options.name_view);
    let exact_self_hp = read_model.as_deref().and_then(|model| {
        (model.player.max_hp > 0).then_some((model.player.hp, model.player.max_hp))
    });
    let now_ms = u64::try_from(time.elapsed().as_millis()).unwrap_or(u64::MAX);
    let mut present = HashSet::new();
    let mut visible_health = Vec::new();
    for actor in &model.actors {
        present.insert(actor.object_id);
        let Some(packet) = actor.health.filter(|_| actor.kind == ActorKind::Monster) else {
            continue;
        };
        let window = health_windows.0.entry(actor.object_id).or_default();
        if window.generation != packet.generation {
            *window = HealthWindow::default();
        }
        if window.generation != packet.generation || window.revision != packet.revision {
            window.generation = packet.generation;
            window.revision = packet.revision;
            window.expires_at_ms =
                now_ms.saturating_add(u64::from(packet.expire_seconds).saturating_mul(1000));
        }
        if let Some(hit_revision) = packet.hit_revision {
            if window.hit_revision != hit_revision {
                window.hit_revision = hit_revision;
                window.hit_expires_at_ms = now_ms.saturating_add(5_000);
            }
        }
        if !actor.dead
            && packet.percent > 0
            && now_ms < window.expires_at_ms.max(window.hit_expires_at_ms)
        {
            visible_health.push((actor.object_id, packet.percent));
        }
    }
    health_windows.0.retain(|id, _| present.contains(id));
    visible_health.sort_unstable();
    let key = RenderKey {
        revision: model.revision,
        names_visible,
        exact_self_hp,
        visible_health: visible_health.clone(),
    };
    let in_game = shell.screen == NativeShellScreen::InGame && model.center.is_some();
    if in_game && rendered.as_ref() == Some(&key) {
        return;
    }
    *rendered = in_game.then_some(key);
    for root in &roots {
        commands.entity(root).despawn();
    }
    if !in_game || (model.actors.is_empty() && visible_health.is_empty()) {
        return;
    }
    let center = model.center.expect("checked above");
    let visible_health = visible_health.into_iter().collect::<HashMap<_, _>>();
    commands
        .spawn((
            ActorOverlayRoot,
            FocusPolicy::Pass,
            Node {
                position_type: PositionType::Absolute,
                width: px(STAGE_WIDTH),
                height: px(STAGE_HEIGHT),
                ..default()
            },
            GlobalZIndex(OVERLAY_Z_INDEX),
        ))
        .with_children(|root| {
            for actor in &model.actors {
                let left =
                    ENTITY_LEFT_ORIGIN + actor.x.saturating_sub(center.0) as f32 * CELL_WIDTH;
                let top = ENTITY_TOP_ORIGIN + actor.y.saturating_sub(center.1) as f32 * CELL_HEIGHT;
                if actor.kind == ActorKind::SelfPlayer && !actor.dead {
                    if let Some((hp, max_hp)) = exact_self_hp {
                        spawn_health_bar(root, left, top, hp as f32 / max_hp as f32);
                    }
                } else if let Some(percent) = visible_health.get(&actor.object_id) {
                    spawn_health_bar(root, left, top, f32::from(*percent) / 100.0);
                }
                if !names_visible {
                    continue;
                }
                let corpse_shift = if actor.dead { CORPSE_NAME_SHIFT } else { 0.0 };
                if matches!(actor.kind, ActorKind::SelfPlayer | ActorKind::Player) {
                    if let Some(guild) = actor.guild_name.as_deref() {
                        spawn_name_line(
                            root,
                            guild,
                            actor.color,
                            left,
                            top + PLAYER_GUILD_TOP + corpse_shift,
                            50.0,
                        );
                    }
                    spawn_name_line(
                        root,
                        &actor.name,
                        actor.color,
                        left,
                        top + PLAYER_NAME_TOP + corpse_shift,
                        50.0,
                    );
                    continue;
                }
                let lines = actor
                    .name
                    .split('_')
                    .filter(|line| !line.is_empty())
                    .collect::<Vec<_>>();
                let adjustment = -((lines.len().saturating_sub(1) as f32) * 10.0) / 2.0;
                let pet_offset = if actor.kind == ActorKind::Monster && actor.name.contains('_') {
                    match actor.image {
                        Some(10001) => -10.0,
                        Some(10000..=10014) => -20.0,
                        _ => 0.0,
                    }
                } else {
                    0.0
                };
                for (index, line) in lines.into_iter().enumerate() {
                    let color = if actor.kind == ActorKind::Npc && index > 0 {
                        [0xff, 0xff, 0xff, 0xff]
                    } else {
                        actor.color
                    };
                    spawn_name_line(
                        root,
                        line,
                        color,
                        left,
                        top + NPC_MONSTER_NAME_TOP
                            + adjustment
                            + pet_offset
                            + index as f32 * SPLIT_NAME_STEP
                            + corpse_shift,
                        48.0,
                    );
                }
            }
        });
}

fn sync_damage(
    mut commands: Commands,
    shell: Res<NativeShellModel>,
    time: Res<Time>,
    mut model: ResMut<ActorOverlayModel>,
    roots: Query<Entity, With<DamageOverlayRoot>>,
    mut nodes: Query<(Entity, &DamageFloaterNode, &mut Node, &mut TextColor)>,
) {
    let now_ms = u64::try_from(time.elapsed().as_millis()).unwrap_or(u64::MAX);
    let in_game = shell.screen == NativeShellScreen::InGame && model.center.is_some();
    if !in_game {
        model.active_floaters.clear();
        for root in &roots {
            commands.entity(root).despawn();
        }
        return;
    }
    model
        .active_floaters
        .retain(|floater| floater.expires_at_ms > now_ms);
    let mut desired = damage_floater_entries(&model, now_ms)
        .into_iter()
        .map(|entry| (entry.sequence, entry))
        .collect::<HashMap<_, _>>();
    if desired.is_empty() {
        for root in &roots {
            commands.entity(root).despawn();
        }
        return;
    }

    let mut roots = roots.iter();
    let root = roots.next().unwrap_or_else(|| {
        commands
            .spawn((
                DamageOverlayRoot,
                FocusPolicy::Pass,
                Node {
                    position_type: PositionType::Absolute,
                    width: px(STAGE_WIDTH),
                    height: px(STAGE_HEIGHT),
                    ..default()
                },
                GlobalZIndex(OVERLAY_Z_INDEX + 1),
            ))
            .id()
    });
    for duplicate in roots {
        commands.entity(duplicate).despawn();
    }
    for (entity, marker, mut node, mut color) in &mut nodes {
        let Some(entry) = desired.remove(&marker.0) else {
            commands.entity(entity).despawn();
            continue;
        };
        node.left = px(entry.left);
        node.top = px(entry.top);
        color.0 = entry.color;
    }
    for entry in desired.into_values() {
        commands.entity(root).with_children(|root| {
            root.spawn((
                Name::new(format!("AndroidDamageFloater:{}", entry.sequence)),
                DamageFloaterNode(entry.sequence),
                FocusPolicy::Pass,
                Node {
                    position_type: PositionType::Absolute,
                    left: px(entry.left),
                    top: px(entry.top),
                    width: px(80.0),
                    min_width: px(80.0),
                    ..default()
                },
                Text::new(entry.text),
                crystal_text_font(entry.font_size),
                TextColor(entry.color),
                TextLayout::justify(Justify::Center),
                TextShadow {
                    offset: Vec2::splat(1.0),
                    color: Color::BLACK,
                },
            ));
        });
    }
}

fn damage_floater_entries(model: &ActorOverlayModel, now_ms: u64) -> Vec<DamageFloaterEntry> {
    let Some((center_x, center_y)) = model.center else {
        return Vec::new();
    };
    let mut entries = model
        .active_floaters
        .iter()
        .filter_map(|floater| {
            let actor = model
                .actors
                .iter()
                .find(|actor| actor.object_id == floater.object_id)?;
            let life_ms = floater
                .expires_at_ms
                .saturating_sub(floater.started_at_ms)
                .max(1);
            let progress =
                now_ms.saturating_sub(floater.started_at_ms).min(life_ms) as f32 / life_ms as f32;
            let eased = 1.0 - (1.0 - progress) * (1.0 - progress);
            let rise = 6.0 - 36.0 * eased;
            let opacity = if progress < 0.16 {
                progress / 0.16
            } else if progress < 0.30 {
                1.0
            } else {
                ((1.0 - progress) / 0.70).clamp(0.0, 1.0)
            };
            let is_player = matches!(actor.kind, ActorKind::SelfPlayer | ActorKind::Player);
            let (red, green, blue, font_size) = match (floater.variant, is_player) {
                (DamageVariant::Miss, true) => (0xff, 0x9d, 0x92, 13.0),
                (DamageVariant::Miss, false) => (0xcf, 0xcf, 0xcf, 13.0),
                (DamageVariant::Critical, _) => (0xff, 0x3b, 0x2f, 18.0),
                (DamageVariant::Heal, _) => (0x6b, 0xff, 0x7a, 15.0),
                (DamageVariant::Hit, true) => (0xff, 0x5a, 0x4d, 15.0),
                (DamageVariant::Hit, false) => (0xf4, 0xf4, 0xf4, 15.0),
            };
            Some(DamageFloaterEntry {
                sequence: floater.sequence,
                text: floater.text.clone(),
                color: Color::srgba_u8(red, green, blue, (opacity * 255.0).round() as u8),
                left: ENTITY_LEFT_ORIGIN + actor.x.saturating_sub(center_x) as f32 * CELL_WIDTH
                    - 16.0,
                top: ENTITY_TOP_ORIGIN + actor.y.saturating_sub(center_y) as f32 * CELL_HEIGHT
                    - 65.0
                    + rise,
                font_size,
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.sequence);
    entries
}

fn spawn_health_bar(root: &mut ChildSpawnerCommands, left: f32, top: f32, ratio: f32) {
    root.spawn((
        ActorHealthBar,
        FocusPolicy::Pass,
        Node {
            position_type: PositionType::Absolute,
            left: px(left + HEALTH_LEFT),
            top: px(top + HEALTH_TOP),
            width: px(HEALTH_WIDTH),
            height: px(HEALTH_HEIGHT),
            border: UiRect::all(px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb_u8(0x27, 0x00, 0x00)),
        BorderColor::all(Color::srgb_u8(0x10, 0x10, 0x10)),
    ))
    .with_children(|bar| {
        bar.spawn((
            FocusPolicy::Pass,
            Node {
                width: percent((ratio.clamp(0.0, 1.0) * 100.0).floor()),
                height: percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgb_u8(0x00, 0xc0, 0x00)),
        ));
    });
}

fn spawn_name_line(
    root: &mut ChildSpawnerCommands,
    text: &str,
    color: [u8; 4],
    left: f32,
    top: f32,
    width: f32,
) {
    for (offset, color) in crystal_outline_offsets()
        .into_iter()
        .map(|offset| (offset, Color::BLACK))
        .chain(std::iter::once((
            Vec2::ONE,
            Color::srgba_u8(color[0], color[1], color[2], color[3]),
        )))
    {
        root.spawn((
            ActorNameLine,
            FocusPolicy::Pass,
            Node {
                position_type: PositionType::Absolute,
                left: px(left + offset.x),
                top: px(top + offset.y),
                width: px(width),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|line| {
            line.spawn((
                FocusPolicy::Pass,
                Node {
                    flex_shrink: 0.0,
                    ..default()
                },
                Text::new(text.to_owned()),
                crystal_text_font(CRYSTAL_DEFAULT_FONT_SIZE_PX),
                TextColor(color),
                TextLayout::new(Justify::Center, LineBreak::NoWrap),
            ));
        });
    }
}

fn crystal_outline_offsets() -> [Vec2; 4] {
    [
        Vec2::new(1.0, 0.0),
        Vec2::new(0.0, 1.0),
        Vec2::new(2.0, 1.0),
        Vec2::new(1.0, 2.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState;

    fn fixture() -> String {
        serde_json::json!({
            "entities": [
                {"objectId":"7","kind":"selfPlayer","name":"Hero","guildName":"Codex","nameColourArgb":-256,"x":10,"y":20,"dead":false},
                {"objectId":"8","kind":"npc","name":"Weapon_Smith","guildName":"","x":11,"y":20},
                {"objectId":"9","kind":"monster","name":"Scarecrow","nameColourArgb":-65536,"x":12,"y":20,"image":3,"_healthPercent":73,"_healthExpireSeconds":3,"_healthGeneration":1,"_healthRevision":2}
            ],
            "groundDrops": []
        }).to_string()
    }

    #[test]
    fn projection_keeps_crystal_actor_fields_and_server_health_percent() {
        let projection = project(&fixture(), 10, 20).unwrap();
        assert_eq!(projection.center, (10, 20));
        assert_eq!(projection.actors.len(), 3);
        assert_eq!(projection.actors[0].kind, ActorKind::SelfPlayer);
        assert_eq!(projection.actors[0].guild_name.as_deref(), Some("Codex"));
        assert_eq!(projection.actors[0].color, [255, 255, 0, 255]);
        assert_eq!(projection.actors[1].color, [0, 255, 0, 255]);
        assert_eq!(projection.actors[2].health.unwrap().percent, 73);
    }

    #[test]
    fn malformed_or_duplicate_actor_data_fails_closed() {
        assert!(project(r#"{"entities":[],"groundDrops":[]}"#, 0, 0).is_some());
        assert!(project(
            r#"{"entities":[{"objectId":"1","kind":"monster","name":"A","x":0,"y":0},{"objectId":"1","kind":"npc","name":"B","x":1,"y":0}],"groundDrops":[]}"#,
            0,
            0
        )
        .is_none());
        assert!(project(
            r#"{"entities":[{"objectId":"1","kind":"monster","name":"A","x":0,"y":0,"_healthPercent":50}],"groundDrops":[]}"#,
            0,
            0
        )
        .is_none());
    }

    #[test]
    fn authoritative_damage_floaters_are_deduplicated_capped_and_animated() {
        let mut model = ActorOverlayModel::default();
        model.replace(project(&fixture(), 10, 20).unwrap());
        let critical = crate::live_entity::LiveDamageEvent {
            sequence: 1,
            object_id: 9,
            damage: 12,
            damage_type: 2,
        };
        model.observe_damage_events([critical], 1_000);
        model.observe_damage_events([critical], 1_100);
        assert_eq!(model.active_floaters.len(), 1);
        assert_eq!(model.active_floaters[0].text, "12");
        assert_eq!(model.active_floaters[0].variant, DamageVariant::Critical);
        let entries = damage_floater_entries(&model, 1_900);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].left, 560.0);
        assert!(entries[0].top < 293.0);
        assert_eq!(entries[0].font_size, 18.0);

        for sequence in 2..=60 {
            model.observe_damage_events(
                [crate::live_entity::LiveDamageEvent {
                    sequence,
                    object_id: 9,
                    damage: sequence as i32,
                    damage_type: 0,
                }],
                1_100,
            );
        }
        assert_eq!(model.active_floaters.len(), MAX_DAMAGE_FLOATERS_PER_ACTOR);
        assert_eq!(model.active_floaters.front().unwrap().sequence, 51);
        model.reset();
        assert!(model.active_floaters.is_empty());
        assert_eq!(model.last_damage_sequence, 0);
    }

    #[test]
    fn confirmed_hit_reveals_zero_expiry_health_without_inventing_exact_hp() {
        let snapshot = serde_json::json!({
            "entities": [{
                "objectId":"9", "kind":"monster", "name":"Deer", "x":10, "y":20,
                "_healthPercent":73, "_healthExpireSeconds":0,
                "_healthGeneration":1, "_healthRevision":1, "_healthHitRevision":1
            }],
            "groundDrops": []
        })
        .to_string();
        let mut app = App::new();
        app.init_resource::<Time>();
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        let mut model = ActorOverlayModel::default();
        model.replace(project(&snapshot, 10, 20).unwrap());
        app.insert_resource(model);
        app.add_systems(Update, sync);
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<ActorHealthBar>>()
                .iter(app.world())
                .count(),
            1
        );
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(5));
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<ActorHealthBar>>()
                .iter(app.world())
                .count(),
            0
        );
    }

    #[test]
    fn names_follow_name_view_but_exact_self_and_timed_monster_bars_do_not() {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        let mut player = NativePlayerUiState::default();
        player.core.options.name_view = false;
        app.insert_resource(player);
        let mut read = UiReadModel::default();
        read.player.hp = 40;
        read.player.max_hp = 80;
        app.insert_resource(read);
        let mut model = ActorOverlayModel::default();
        model.replace(project(&fixture(), 10, 20).unwrap());
        model.observe_damage_events(
            [crate::live_entity::LiveDamageEvent {
                sequence: 1,
                object_id: 9,
                damage: 12,
                damage_type: 2,
            }],
            0,
        );
        app.insert_resource(model);
        app.add_systems(Update, (sync, sync_damage).chain());
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<ActorNameLine>>()
                .iter(app.world())
                .count(),
            0
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<ActorHealthBar>>()
                .iter(app.world())
                .count(),
            2
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<DamageFloaterNode>>()
                .iter(app.world())
                .count(),
            1
        );
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(3));
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<ActorHealthBar>>()
                .iter(app.world())
                .count(),
            1
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<DamageFloaterNode>>()
                .iter(app.world())
                .count(),
            0
        );
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .options
            .name_view = true;
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<ActorNameLine>>()
                .iter(app.world())
                .count(),
            25
        );
        for policy in app.world_mut().query::<&FocusPolicy>().iter(app.world()) {
            assert_eq!(*policy, FocusPolicy::Pass);
        }
    }
}
