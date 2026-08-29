//! Native Crystal entity nameplates and the self health bar.
//!
//! Web keeps these as DOM overlays. The Windows host owns an equivalent Bevy
//! UI layer so native entity sprites do not depend on a browser surface.

use std::collections::HashMap;

use bevy::prelude::*;
use mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState;
use mir2_client_bevy::crystal_ui::typography::{crystal_text_font, CRYSTAL_DEFAULT_FONT_SIZE_PX};
use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};
use mir2_client_bevy::quest_model::{QuestStatus, QuestTracker};
use serde_json::Value;

use crate::entity_presentation::NativeEntityPresentation;
use crate::gameplay_bridge::NativeDamageEvent;

const STAGE_WIDTH: f32 = 1024.0;
const STAGE_HEIGHT: f32 = 768.0;
const CELL_WIDTH: f32 = 48.0;
const CELL_HEIGHT: f32 = 32.0;
const OVERLAY_Z_INDEX: i32 = 850;
// Crystal MapObject.DrawName and PlayerObject.DrawName use
// `Dead ? 35 : 8`, so a corpse keeps its label and moves it down by 27px.
const CRYSTAL_CORPSE_NAME_SHIFT_Y_PX: f32 = 27.0;
const CRYSTAL_PLAYER_NAME_TOP_OFFSET_PX: f32 = -17.0;
// PlayerObject.DrawName places the guild line 12 px below the player name:
// `-(19 - label_height / 2) + 8` versus `-(31 - label_height / 2) + 8`.
const CRYSTAL_PLAYER_GUILD_TOP_OFFSET_PX: f32 = -5.0;
const CRYSTAL_NPC_MONSTER_NAME_TOP_OFFSET_PX: f32 = -18.0;
const CRYSTAL_QUEST_MARKER_WIDTH_PX: f32 = 28.0;
const CRYSTAL_QUEST_MARKER_HEIGHT_PX: f32 = 29.0;
const CRYSTAL_QUEST_MARKER_FRAME_INTERVAL_MS: u64 = 500;
const CRYSTAL_QUEST_MARKER_FALLBACK_LEFT_PX: f32 = 12.0;
const CRYSTAL_QUEST_MARKER_FALLBACK_TOP_PX: f32 = -58.0;

#[derive(Component)]
pub(crate) struct NativeEntityOverlayRoot;

#[derive(Resource, Debug, Default)]
pub struct NativeEntityOverlays {
    latest_payload: Option<Value>,
    active_floaters: Vec<ActiveDamageFloater>,
    last_damage_sequence: u64,
    dirty: bool,
    last_in_game: bool,
    last_visibility: Option<OverlayVisibility>,
    last_hovered_object_id: Option<String>,
    last_self_hovered: bool,
    last_quest_marker_phase: u8,
}

/// Local Crystal name/drop presentation flags. They only select which labels
/// this host draws from its latest authoritative scene payload; neither flag
/// changes packet ingestion, entity state, pickup eligibility, or the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OverlayVisibility {
    name_view: bool,
    drop_view: bool,
}

impl OverlayVisibility {
    fn from_player_ui(state: Option<&NativePlayerUiState>) -> Self {
        let options = state.map(|state| &state.core.options);
        Self {
            name_view: options.map(|options| options.name_view).unwrap_or(true),
            drop_view: options.map(|options| options.drop_view).unwrap_or(true),
        }
    }
}

impl NativeEntityOverlays {
    pub fn reset_session(&mut self) {
        self.latest_payload = None;
        self.active_floaters.clear();
        self.last_damage_sequence = 0;
        self.dirty = true;
        self.last_in_game = false;
        self.last_hovered_object_id = None;
        self.last_self_hovered = false;
    }

    pub fn replace_payload(&mut self, payload: Value) {
        self.latest_payload = Some(payload);
        self.dirty = true;
    }

    pub fn observe_damage_events(&mut self, events: &[NativeDamageEvent], now_ms: u64) {
        self.active_floaters
            .retain(|floater| floater.expires_at_ms > now_ms);
        for event in events {
            if event.sequence <= self.last_damage_sequence {
                continue;
            }
            self.last_damage_sequence = self.last_damage_sequence.max(event.sequence);
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
                >= 10
            {
                if let Some(index) = self
                    .active_floaters
                    .iter()
                    .position(|floater| floater.object_id == event.object_id)
                {
                    self.active_floaters.remove(index);
                }
            }
            while self.active_floaters.len() >= 48 {
                self.active_floaters.remove(0);
            }
            let duration_ms = if variant == DamageVariant::Miss {
                1_600
            } else {
                1_800
            };
            self.active_floaters.push(ActiveDamageFloater {
                sequence: event.sequence,
                object_id: event.object_id,
                text,
                variant,
                started_at_ms: now_ms,
                expires_at_ms: now_ms.saturating_add(duration_ms),
            });
            self.dirty = true;
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

#[derive(Debug)]
struct DamageFloaterEntry {
    key: u64,
    text: String,
    color: Color,
    left: f32,
    top: f32,
    width: f32,
    font_size: f32,
}

#[derive(Debug)]
struct OverlayEntry {
    name: Option<String>,
    quest_marker: Option<QuestMarkerKind>,
    color: Color,
    left: f32,
    top: f32,
    width: f32,
    font_size: f32,
    self_health_ratio: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum QuestMarkerKind {
    QuestionWhite = 1,
    ExclamationYellow = 2,
    QuestionYellow = 3,
    ExclamationBlue = 5,
    QuestionBlue = 6,
    ExclamationGreen = 52,
    QuestionGreen = 53,
}

impl QuestMarkerKind {
    fn from_crystal_discriminant(value: i64) -> Option<Self> {
        match value {
            1 => Some(Self::QuestionWhite),
            2 => Some(Self::ExclamationYellow),
            3 => Some(Self::QuestionYellow),
            5 => Some(Self::ExclamationBlue),
            6 => Some(Self::QuestionBlue),
            52 => Some(Self::ExclamationGreen),
            53 => Some(Self::QuestionGreen),
            _ => None,
        }
    }

    fn first_frame_index(self) -> u16 {
        981 + u16::from(self as u8) * 2
    }

    fn asset_path(self, phase: u8) -> String {
        format!(
            "original-ui/Prguse/{}.png",
            self.first_frame_index() + u16::from(phase % 2)
        )
    }
}

fn quest_marker_animation_phase(now_ms: u64) -> u8 {
    ((now_ms / CRYSTAL_QUEST_MARKER_FRAME_INTERVAL_MS) % 2) as u8
}

pub fn sync_native_entity_overlays(
    mut commands: Commands,
    shell: Res<NativeShellModel>,
    mut overlays: ResMut<NativeEntityOverlays>,
    roots: Query<Entity, With<NativeEntityOverlayRoot>>,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    player_ui: Option<Res<NativePlayerUiState>>,
    presentation: Res<NativeEntityPresentation>,
    quest_tracker: Option<Res<QuestTracker>>,
) {
    let now_ms = u64::try_from(time.elapsed().as_millis()).unwrap_or(u64::MAX);
    let motion_now_ms = crate::entity_presentation::native_motion_clock_ms();
    let previous_floater_count = overlays.active_floaters.len();
    overlays
        .active_floaters
        .retain(|floater| floater.expires_at_ms > now_ms);
    if overlays.active_floaters.len() != previous_floater_count
        || !overlays.active_floaters.is_empty()
        || presentation.has_active_motion(motion_now_ms)
    {
        overlays.dirty = true;
    }
    let in_game = shell.screen == NativeShellScreen::InGame;
    let visibility = OverlayVisibility::from_player_ui(player_ui.as_deref());
    let hovered_object_id = presentation.hovered_object_id();
    let self_hovered = presentation.self_hovered();
    let quest_marker_phase = quest_marker_animation_phase(now_ms);
    if overlays.last_quest_marker_phase != quest_marker_phase
        || quest_tracker
            .as_ref()
            .is_some_and(|tracker| tracker.is_changed())
    {
        overlays.dirty = true;
        overlays.last_quest_marker_phase = quest_marker_phase;
    }
    if !overlays.dirty
        && overlays.last_in_game == in_game
        && overlays.last_visibility == Some(visibility)
        && overlays.last_hovered_object_id.as_deref() == hovered_object_id
        && overlays.last_self_hovered == self_hovered
    {
        return;
    }
    overlays.last_in_game = in_game;
    overlays.last_visibility = Some(visibility);
    overlays.last_hovered_object_id = hovered_object_id.map(str::to_owned);
    overlays.last_self_hovered = self_hovered;
    overlays.dirty = false;
    for root in &roots {
        commands.entity(root).despawn();
    }
    if !in_game {
        return;
    }
    let Some(payload) = overlays.latest_payload.as_ref() else {
        return;
    };
    let motion_offsets = payload
        .get("entities")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entity| {
            let object_id = entity.get("objectId").and_then(normalized_object_id)?;
            Some((
                object_id.clone(),
                presentation.entity_screen_offset(&object_id, motion_now_ms),
            ))
        })
        .collect::<HashMap<_, _>>();
    let camera_offset = presentation.camera_screen_offset(motion_now_ms);
    let entries = overlay_entries_with_motion(
        payload,
        visibility,
        hovered_object_id,
        self_hovered,
        quest_tracker.as_deref(),
        &motion_offsets,
        camera_offset,
    );
    let floaters = damage_floater_entries_with_motion(
        payload,
        &overlays.active_floaters,
        now_ms,
        &motion_offsets,
    );
    if entries.is_empty() && floaters.is_empty() {
        return;
    }

    commands
        .spawn((
            NativeEntityOverlayRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(STAGE_WIDTH),
                height: Val::Px(STAGE_HEIGHT),
                ..default()
            },
            GlobalZIndex(OVERLAY_Z_INDEX),
        ))
        .with_children(|root| {
            for entry in entries {
                if let Some(ratio) = entry.self_health_ratio {
                    root.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(entry.left + 8.0),
                            top: Val::Px(entry.top - 47.0),
                            width: Val::Px(32.0),
                            height: Val::Px(4.0),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb_u8(0x27, 0x00, 0x00)),
                        BorderColor::all(Color::srgb_u8(0x10, 0x10, 0x10)),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            Node {
                                width: Val::Percent((ratio * 100.0).clamp(0.0, 100.0)),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb_u8(0x00, 0xc0, 0x00)),
                        ));
                    });
                }

                if let Some(marker) = entry.quest_marker {
                    root.spawn((
                        Name::new(format!("NativeQuestMarker:{marker:?}")),
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(entry.left),
                            top: Val::Px(entry.top),
                            width: Val::Px(CRYSTAL_QUEST_MARKER_WIDTH_PX),
                            height: Val::Px(CRYSTAL_QUEST_MARKER_HEIGHT_PX),
                            ..default()
                        },
                        ImageNode {
                            image: asset_server.load(marker.asset_path(quest_marker_phase)),
                            ..default()
                        },
                    ));
                }

                let Some(name) = entry.name else {
                    continue;
                };
                for offset in crystal_outline_offsets() {
                    root.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(entry.left + offset.x),
                            top: Val::Px(entry.top + offset.y),
                            width: Val::Px(entry.width),
                            min_width: Val::Px(entry.width),
                            ..default()
                        },
                        Text::new(name.clone()),
                        crystal_text_font(entry.font_size),
                        TextColor(Color::BLACK),
                        TextLayout::justify(Justify::Center),
                    ));
                }
                root.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(entry.left + 1.0),
                        top: Val::Px(entry.top + 1.0),
                        width: Val::Px(entry.width),
                        min_width: Val::Px(entry.width),
                        ..default()
                    },
                    Text::new(name),
                    crystal_text_font(entry.font_size),
                    TextColor(entry.color),
                    TextLayout::justify(Justify::Center),
                ));
            }
            for floater in floaters {
                root.spawn((
                    Name::new(format!("NativeDamageFloater:{}", floater.key)),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(floater.left),
                        top: Val::Px(floater.top),
                        width: Val::Px(floater.width),
                        min_width: Val::Px(floater.width),
                        ..default()
                    },
                    Text::new(floater.text),
                    crystal_text_font(floater.font_size),
                    TextColor(floater.color),
                    TextLayout::justify(Justify::Center),
                    TextShadow {
                        offset: Vec2::splat(1.0),
                        color: Color::BLACK,
                    },
                ));
            }
        });
}

/// `MirLabel.DrawControl` renders the black outline in this exact order before
/// drawing the foreground at `(1, 1)`. Keep this as geometry rather than a
/// renderer shadow so all four source pixels remain independently auditable.
fn crystal_outline_offsets() -> [Vec2; 4] {
    [
        Vec2::new(1.0, 0.0),
        Vec2::new(0.0, 1.0),
        Vec2::new(2.0, 1.0),
        Vec2::new(1.0, 2.0),
    ]
}

fn damage_floater_entries(
    payload: &Value,
    floaters: &[ActiveDamageFloater],
    now_ms: u64,
) -> Vec<DamageFloaterEntry> {
    damage_floater_entries_with_motion(payload, floaters, now_ms, &HashMap::new())
}

fn damage_floater_entries_with_motion(
    payload: &Value,
    floaters: &[ActiveDamageFloater],
    now_ms: u64,
    motion_offsets: &HashMap<String, (f32, f32)>,
) -> Vec<DamageFloaterEntry> {
    let center = payload.get("sceneView").and_then(|view| view.get("center"));
    let center_x = center
        .and_then(|center| center.get("x"))
        .and_then(value_i64)
        .unwrap_or(0);
    let center_y = center
        .and_then(|center| center.get("y"))
        .and_then(value_i64)
        .unwrap_or(0);
    let origin_x = (STAGE_WIDTH / 2.0 / CELL_WIDTH).floor() * CELL_WIDTH;
    let origin_y = ((STAGE_HEIGHT / 2.0 / CELL_HEIGHT).floor() - 1.0) * CELL_HEIGHT;
    let player_object_id = payload
        .get("playerObjectId")
        .and_then(value_i64)
        .and_then(|value| u32::try_from(value).ok());
    let entities = payload
        .get("entities")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();

    floaters
        .iter()
        .filter_map(|floater| {
            let target = entities
                .iter()
                .find(|entity| {
                    entity
                        .get("objectId")
                        .and_then(value_i64)
                        .and_then(|value| u32::try_from(value).ok())
                        == Some(floater.object_id)
                })
                .or_else(|| {
                    player_object_id.and_then(|player_object_id| {
                        entities.iter().find(|entity| {
                            entity
                                .get("objectId")
                                .and_then(value_i64)
                                .and_then(|value| u32::try_from(value).ok())
                                == Some(player_object_id)
                        })
                    })
                })?;
            let x = target.get("x").and_then(value_i64)?;
            let y = target.get("y").and_then(value_i64)?;
            let object_id = target
                .get("objectId")
                .and_then(normalized_object_id)
                .unwrap_or_default();
            let (motion_x, motion_y) = motion_offsets
                .get(&object_id)
                .copied()
                .unwrap_or((0.0, 0.0));
            let kind = target
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("monster");
            let is_player = matches!(kind, "selfPlayer" | "player" | "hero")
                || player_object_id == Some(floater.object_id);
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
            let (red, green, blue, font_size) = match (floater.variant, is_player) {
                (DamageVariant::Miss, true) => (0xff, 0x9d, 0x92, 13.0),
                (DamageVariant::Miss, false) => (0xcf, 0xcf, 0xcf, 13.0),
                (DamageVariant::Critical, _) => (0xff, 0x3b, 0x2f, 18.0),
                (DamageVariant::Heal, _) => (0x6b, 0xff, 0x7a, 15.0),
                (DamageVariant::Hit, true) => (0xff, 0x5a, 0x4d, 15.0),
                (DamageVariant::Hit, false) => (0xf4, 0xf4, 0xf4, 15.0),
            };
            Some(DamageFloaterEntry {
                key: floater.sequence,
                text: floater.text.clone(),
                color: Color::srgba_u8(red, green, blue, (opacity * 255.0).round() as u8),
                left: origin_x + (x - center_x) as f32 * CELL_WIDTH - 16.0 + motion_x,
                top: origin_y + (y - center_y) as f32 * CELL_HEIGHT - 65.0 + rise + motion_y,
                width: 80.0,
                font_size,
            })
        })
        .collect()
}

fn overlay_entries(
    payload: &Value,
    visibility: OverlayVisibility,
    hovered_object_id: Option<&str>,
    self_hovered: bool,
    quest_tracker: Option<&QuestTracker>,
) -> Vec<OverlayEntry> {
    overlay_entries_with_motion(
        payload,
        visibility,
        hovered_object_id,
        self_hovered,
        quest_tracker,
        &HashMap::new(),
        (0.0, 0.0),
    )
}

fn overlay_entries_with_motion(
    payload: &Value,
    visibility: OverlayVisibility,
    hovered_object_id: Option<&str>,
    self_hovered: bool,
    quest_tracker: Option<&QuestTracker>,
    motion_offsets: &HashMap<String, (f32, f32)>,
    camera_offset: (f32, f32),
) -> Vec<OverlayEntry> {
    let center = payload.get("sceneView").and_then(|view| view.get("center"));
    let center_x = center
        .and_then(|center| center.get("x"))
        .and_then(value_i64)
        .unwrap_or(0);
    let center_y = center
        .and_then(|center| center.get("y"))
        .and_then(value_i64)
        .unwrap_or(0);
    let origin_x = (STAGE_WIDTH / 2.0 / CELL_WIDTH).floor() * CELL_WIDTH;
    let origin_y = ((STAGE_HEIGHT / 2.0 / CELL_HEIGHT).floor() - 1.0) * CELL_HEIGHT;
    let player_hp = payload.get("playerHp").and_then(value_i64);
    let player_max_hp = payload.get("playerMaxHp").and_then(value_i64);

    let mut entries = Vec::new();
    if visibility.name_view || hovered_object_id.is_some() || self_hovered {
        entries.extend(
            payload
                .get("entities")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .flat_map(|entity| {
                    let Some(name) = entity.get("name").and_then(Value::as_str).map(str::trim)
                    else {
                        return Vec::new();
                    };
                    if name.is_empty() {
                        return Vec::new();
                    }
                    let kind = entity
                        .get("kind")
                        .and_then(Value::as_str)
                        .unwrap_or("monster");
                    let x = entity.get("x").and_then(value_i64).unwrap_or(0);
                    let y = entity.get("y").and_then(value_i64).unwrap_or(0);
                    let dead = entity.get("dead").and_then(Value::as_bool) == Some(true);
                    let is_self = kind == "selfPlayer";
                    let object_id = entity.get("objectId").and_then(|value| match value {
                        Value::Number(number) => Some(number.to_string()),
                        Value::String(value) if !value.is_empty() => Some(value.clone()),
                        _ => None,
                    });
                    let (motion_x, motion_y) = object_id
                        .as_deref()
                        .and_then(|object_id| motion_offsets.get(object_id))
                        .copied()
                        .unwrap_or((0.0, 0.0));
                    let hovered = if is_self {
                        self_hovered
                    } else {
                        object_id
                            .as_deref()
                            .is_some_and(|object_id| hovered_object_id == Some(object_id))
                    };
                    if !visibility.name_view && !hovered {
                        return Vec::new();
                    }
                    let guild_name = matches!(kind, "selfPlayer" | "player")
                        .then(|| entity.get("guildName").and_then(Value::as_str))
                        .flatten()
                        .map(str::trim)
                        .filter(|guild_name| !guild_name.is_empty());
                    let lines = if matches!(kind, "npc" | "monster") {
                        name.split('_')
                            .filter(|part| !part.is_empty())
                            .collect::<Vec<_>>()
                    } else {
                        vec![name]
                    };
                    let display_name = lines.join("\n");
                    let line_adjustment = if matches!(kind, "npc" | "monster") {
                        -((lines.len().saturating_sub(1) as f32) * 10.0) / 2.0
                    } else {
                        0.0
                    };
                    let corpse_shift = if dead {
                        CRYSTAL_CORPSE_NAME_SHIFT_Y_PX
                    } else {
                        0.0
                    };
                    let color = entity
                        .get("nameColourArgb")
                        .and_then(value_i64)
                        .and_then(argb_color)
                        .unwrap_or_else(|| {
                            if kind == "npc" {
                                Color::srgb_u8(0x00, 0xff, 0x00)
                            } else {
                                Color::WHITE
                            }
                        });
                    let left = origin_x + (x - center_x) as f32 * CELL_WIDTH + motion_x;
                    let top = origin_y + (y - center_y) as f32 * CELL_HEIGHT + motion_y;
                    let width = if matches!(kind, "npc" | "monster") {
                        48.0
                    } else {
                        50.0
                    };
                    let mut entity_entries = Vec::with_capacity(2);
                    if kind == "npc" {
                        if let Some(marker) = quest_marker_for_entity(entity, quest_tracker) {
                            entity_entries.push(OverlayEntry {
                                name: None,
                                quest_marker: Some(marker),
                                color: Color::WHITE,
                                left: left + CRYSTAL_QUEST_MARKER_FALLBACK_LEFT_PX,
                                top: top + CRYSTAL_QUEST_MARKER_FALLBACK_TOP_PX,
                                width: CRYSTAL_QUEST_MARKER_WIDTH_PX,
                                font_size: CRYSTAL_DEFAULT_FONT_SIZE_PX,
                                self_health_ratio: None,
                            });
                        }
                    }
                    if let Some(guild_name) = guild_name {
                        entity_entries.push(OverlayEntry {
                            name: Some(guild_name.to_owned()),
                            quest_marker: None,
                            color,
                            left,
                            top: top + CRYSTAL_PLAYER_GUILD_TOP_OFFSET_PX + corpse_shift,
                            width,
                            font_size: CRYSTAL_DEFAULT_FONT_SIZE_PX,
                            self_health_ratio: None,
                        });
                    }
                    entity_entries.push(OverlayEntry {
                        name: Some(display_name),
                        quest_marker: None,
                        color,
                        left,
                        top: top
                            + if matches!(kind, "npc" | "monster") {
                                CRYSTAL_NPC_MONSTER_NAME_TOP_OFFSET_PX + line_adjustment
                            } else {
                                CRYSTAL_PLAYER_NAME_TOP_OFFSET_PX
                            }
                            + corpse_shift,
                        width,
                        font_size: CRYSTAL_DEFAULT_FONT_SIZE_PX,
                        self_health_ratio: None,
                    });
                    entity_entries
                }),
        );
    }

    // Crystal's User.DrawHealth path is independent of NameView and
    // SelfPlayer.MouseOver. Keep one health-only entry so moving the cursor
    // cannot make the self bar appear or disappear with the nameplate.
    if let Some(entity) = payload
        .get("entities")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|entity| {
            entity.get("kind").and_then(Value::as_str) == Some("selfPlayer")
                && entity.get("dead").and_then(Value::as_bool) != Some(true)
        })
    {
        let hp = entity.get("hp").and_then(value_i64).or(player_hp);
        let max_hp = entity.get("maxHp").and_then(value_i64).or(player_max_hp);
        if let (Some(hp), Some(max_hp)) = (hp, max_hp) {
            if max_hp > 0 {
                let x = entity.get("x").and_then(value_i64).unwrap_or(0);
                let y = entity.get("y").and_then(value_i64).unwrap_or(0);
                let object_id = entity
                    .get("objectId")
                    .and_then(normalized_object_id)
                    .unwrap_or_default();
                let (motion_x, motion_y) = motion_offsets
                    .get(&object_id)
                    .copied()
                    .unwrap_or((0.0, 0.0));
                entries.push(OverlayEntry {
                    name: None,
                    quest_marker: None,
                    color: Color::WHITE,
                    left: origin_x + (x - center_x) as f32 * CELL_WIDTH + motion_x,
                    top: origin_y + (y - center_y) as f32 * CELL_HEIGHT - 17.0 + motion_y,
                    width: 50.0,
                    font_size: CRYSTAL_DEFAULT_FONT_SIZE_PX,
                    self_health_ratio: Some((hp as f32 / max_hp as f32).clamp(0.0, 1.0)),
                });
            }
        }
    }

    // Crystal's DropView draws item names after map/world rendering. The
    // native snapshot already carries groundDrops, so this is a presentation
    // gate only; drops remain authoritative and pick-up logic stays live.
    if visibility.drop_view {
        entries.extend(
            payload
                .get("groundDrops")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .take(256)
                .filter_map(|drop| {
                    let name = drop.get("name").and_then(Value::as_str)?.trim();
                    if name.is_empty() {
                        return None;
                    }
                    let x = drop.get("x").and_then(value_i64)?;
                    let y = drop.get("y").and_then(value_i64)?;
                    Some(OverlayEntry {
                        name: Some(name.to_owned()),
                        quest_marker: None,
                        color: Color::srgb_u8(0xff, 0xe6, 0x58),
                        left: origin_x + (x - center_x) as f32 * CELL_WIDTH - 16.0
                            + camera_offset.0,
                        top: origin_y + (y - center_y) as f32 * CELL_HEIGHT - 18.0
                            + camera_offset.1,
                        width: 80.0,
                        font_size: CRYSTAL_DEFAULT_FONT_SIZE_PX,
                        self_health_ratio: None,
                    })
                }),
        );
    }
    entries
}

fn quest_marker_for_entity(
    entity: &Value,
    quest_tracker: Option<&QuestTracker>,
) -> Option<QuestMarkerKind> {
    if let Some(marker) = entity
        .get("questIcon")
        .and_then(value_i64)
        .and_then(QuestMarkerKind::from_crystal_discriminant)
    {
        return Some(marker);
    }

    let tracker = quest_tracker?;
    let object_id = entity.get("objectId").and_then(value_i64)?;
    let object_id = u32::try_from(object_id).ok()?;
    let quest_indexes = entity.get("questIds").and_then(Value::as_array).map(|ids| {
        ids.iter()
            .filter_map(value_i64)
            .filter_map(|value| i32::try_from(value).ok())
            .collect::<Vec<_>>()
    });
    let listed_for_npc = |quest_index: i32| {
        quest_indexes
            .as_ref()
            .is_none_or(|indexes| indexes.contains(&quest_index))
    };

    // Crystal first walks CurrentQuests in insertion order and chooses the
    // first quest whose finish NPC is this object. Any current quest therefore
    // wins over an available quest without inventing a status priority.
    for quest in &tracker.active_quests {
        if !matches!(
            quest.status,
            QuestStatus::InProgress | QuestStatus::ReadyToTurnIn
        ) {
            continue;
        }
        let finish_npc = quest
            .finish_npc_index
            .filter(|finish| *finish != 0)
            .or(quest.accept_npc_index);
        if finish_npc != Some(object_id) {
            continue;
        }
        return Some(match quest.status {
            QuestStatus::InProgress => QuestMarkerKind::QuestionWhite,
            QuestStatus::ReadyToTurnIn => QuestMarkerKind::QuestionYellow,
            _ => unreachable!("current quest status was filtered above"),
        });
    }

    for quest in &tracker.active_quests {
        if listed_for_npc(quest.quest_index)
            && quest.status == QuestStatus::NotStarted
            && quest.accept_npc_index == Some(object_id)
        {
            // The legacy tracker model does not carry QuestType. This path is
            // only a compatibility fallback for snapshots predating
            // authoritative questIcon, so use Crystal's general/repeatable
            // presentation instead of guessing blue/green.
            return Some(QuestMarkerKind::ExclamationYellow);
        }
    }

    None
}

fn value_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| value.as_str()?.parse().ok())
}

fn normalized_object_id(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => Some(number.to_string()),
        Value::String(value) if !value.is_empty() => Some(value.clone()),
        _ => None,
    }
}

fn argb_color(value: i64) -> Option<Color> {
    if value == -1 {
        return None;
    }
    let argb = value as u32;
    let alpha = ((argb >> 24) & 0xff) as u8;
    if alpha == 0 {
        return None;
    }
    let red = ((argb >> 16) & 0xff) as u8;
    let green = ((argb >> 8) & 0xff) as u8;
    let blue = (argb & 0xff) as u8;
    Some(Color::srgba_u8(red, green, blue, alpha))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn overlays_match_crystal_cell_offsets_names_and_self_health() {
        let entries = overlay_entries(
            &json!({
                "sceneView": {"center": {"x": 10, "y": 20}},
                "playerHp": 9,
                "playerMaxHp": 18,
                "entities": [
                    {"objectId": 1, "kind": "selfPlayer", "name": "Hero", "guildName": "Guard", "x": 10, "y": 20},
                    {"objectId": 2, "kind": "npc", "name": "Weapon_Smith", "x": 11, "y": 19}
                ]
            }),
            OverlayVisibility {
                name_view: true,
                drop_view: true,
            },
            None,
            false,
            None,
        );
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].left, 480.0);
        assert_eq!(entries[0].top, 347.0);
        assert_eq!(entries[0].self_health_ratio, None);
        assert_eq!(entries[0].name.as_deref(), Some("Guard"));
        assert_eq!(entries[1].left, 480.0);
        assert_eq!(entries[1].top, 335.0);
        assert_eq!(entries[1].name.as_deref(), Some("Hero"));
        assert_eq!(entries[2].name.as_deref(), Some("Weapon\nSmith"));
        assert_eq!(entries[2].left, 528.0);
        assert_eq!(entries[2].top, 297.0);
        assert_eq!(entries[2].width, 48.0);
        assert_eq!(entries[3].name, None);
        assert_eq!(entries[3].self_health_ratio, Some(0.5));
    }

    #[test]
    fn player_guild_and_name_follow_crystal_two_line_offsets_and_color() {
        let entries = overlay_entries(
            &json!({
                "sceneView": {"center": {"x": 10, "y": 20}},
                "entities": [
                    {
                        "objectId": 1,
                        "kind": "player",
                        "name": "Scout",
                        "guildName": "BichonGuard",
                        "nameColourArgb": 0xff12_3456u32,
                        "x": 10,
                        "y": 20
                    },
                    {
                        "objectId": 2,
                        "kind": "player",
                        "name": "Fallen",
                        "guildName": "BichonGuard",
                        "nameColourArgb": 0xff12_3456u32,
                        "x": 11,
                        "y": 20,
                        "dead": true
                    }
                ]
            }),
            OverlayVisibility {
                name_view: true,
                drop_view: false,
            },
            None,
            false,
            None,
        );
        assert_eq!(
            entry_names(&entries),
            ["BichonGuard", "Scout", "BichonGuard", "Fallen"]
        );
        assert_eq!(entries[0].top - entries[1].top, 12.0);
        assert_eq!(entries[2].top - entries[0].top, 27.0);
        assert_eq!(entries[3].top - entries[1].top, 27.0);
        for entry in &entries {
            assert_eq!(entry.color, Color::srgba_u8(0x12, 0x34, 0x56, 0xff));
            assert_eq!(entry.width, 50.0);
        }
    }

    #[test]
    fn player_guild_lines_follow_name_view_and_hover_without_duplicates() {
        let payload = json!({
            "sceneView": {"center": {"x": 10, "y": 20}},
            "playerHp": 9,
            "playerMaxHp": 18,
            "entities": [
                {"objectId": 1, "kind": "selfPlayer", "name": "Self", "guildName": "Codex", "x": 10, "y": 20},
                {"objectId": "2", "kind": "player", "name": "Remote", "guildName": "BichonGuard", "x": 11, "y": 20},
                {"objectId": 3, "kind": "player", "name": "Solo", "x": 12, "y": 20}
            ]
        });
        let off = OverlayVisibility {
            name_view: false,
            drop_view: false,
        };
        assert!(entry_names(&overlay_entries(&payload, off, None, false, None)).is_empty());
        assert_eq!(
            entry_names(&overlay_entries(&payload, off, Some("2"), false, None)),
            ["BichonGuard", "Remote"]
        );
        assert_eq!(
            entry_names(&overlay_entries(&payload, off, None, true, None)),
            ["Codex", "Self"]
        );
        assert_eq!(
            entry_names(&overlay_entries(&payload, off, Some("2"), true, None)),
            ["Codex", "Self", "BichonGuard", "Remote"]
        );
        assert_eq!(
            entry_names(&overlay_entries(
                &payload,
                OverlayVisibility {
                    name_view: true,
                    drop_view: false,
                },
                Some("2"),
                true,
                None,
            )),
            ["Codex", "Self", "BichonGuard", "Remote", "Solo"]
        );
    }

    #[test]
    fn name_and_drop_view_are_independent_presentation_gates() {
        let payload = json!({
            "sceneView": {"center": {"x": 10, "y": 20}},
            "entities": [{"objectId": 1, "kind": "selfPlayer", "name": "Hero", "x": 10, "y": 20}],
            "groundDrops": [{"objectId": 9, "name": "Potion", "x": 11, "y": 20}]
        });
        let names_only = overlay_entries(
            &payload,
            OverlayVisibility {
                name_view: true,
                drop_view: false,
            },
            None,
            false,
            None,
        );
        assert_eq!(
            names_only
                .iter()
                .filter_map(|entry| entry.name.as_deref())
                .collect::<Vec<_>>(),
            ["Hero"]
        );

        let drops_only = overlay_entries(
            &payload,
            OverlayVisibility {
                name_view: false,
                drop_view: true,
            },
            None,
            false,
            None,
        );
        assert_eq!(
            drops_only
                .iter()
                .filter_map(|entry| entry.name.as_deref())
                .collect::<Vec<_>>(),
            ["Potion"]
        );

        assert!(overlay_entries(
            &payload,
            OverlayVisibility {
                name_view: false,
                drop_view: false
            },
            None,
            false,
            None,
        )
        .is_empty());
    }

    #[test]
    fn motion_offsets_keep_self_locked_and_move_remote_drop_and_damage_labels() {
        let payload = json!({
            "sceneView": {"center": {"x": 10, "y": 20}},
            "playerHp": 9,
            "playerMaxHp": 18,
            "entities": [
                {"objectId": 1, "kind": "selfPlayer", "name": "Self", "x": 10, "y": 20},
                {"objectId": 2, "kind": "monster", "name": "Deer", "x": 11, "y": 20}
            ],
            "groundDrops": [{"objectId": 9, "name": "Potion", "x": 11, "y": 20}]
        });
        let offsets = HashMap::from([("1".to_owned(), (0.0, 0.0)), ("2".to_owned(), (-16.0, 8.0))]);
        let entries = overlay_entries_with_motion(
            &payload,
            OverlayVisibility {
                name_view: true,
                drop_view: true,
            },
            None,
            false,
            None,
            &offsets,
            (40.0, 0.0),
        );
        let named = |name: &str| {
            entries
                .iter()
                .find(|entry| entry.name.as_deref() == Some(name))
                .expect("named overlay")
        };
        assert_eq!((named("Self").left, named("Self").top), (480.0, 335.0));
        assert_eq!((named("Deer").left, named("Deer").top), (512.0, 342.0));
        assert_eq!((named("Potion").left, named("Potion").top), (552.0, 334.0));
        let health = entries
            .iter()
            .find(|entry| entry.self_health_ratio.is_some())
            .expect("self health overlay");
        assert_eq!((health.left, health.top), (480.0, 335.0));

        let floater = ActiveDamageFloater {
            sequence: 1,
            object_id: 2,
            text: "5".to_owned(),
            variant: DamageVariant::Hit,
            started_at_ms: 1_000,
            expires_at_ms: 2_800,
        };
        let static_entry = damage_floater_entries(&payload, std::slice::from_ref(&floater), 1_100)
            .pop()
            .expect("static damage floater");
        let moved_entry = damage_floater_entries_with_motion(
            &payload,
            std::slice::from_ref(&floater),
            1_100,
            &offsets,
        )
        .pop()
        .expect("moving damage floater");
        assert_eq!(moved_entry.left - static_entry.left, -16.0);
        assert_eq!(moved_entry.top - static_entry.top, 8.0);
    }

    #[test]
    fn living_names_follow_name_view_and_hover_identity_without_duplicates() {
        let payload = json!({
            "sceneView": {"center": {"x": 10, "y": 20}},
            "playerHp": 9,
            "playerMaxHp": 18,
            "selectedObjectId": 4,
            "entities": [
                {"objectId": 1, "kind": "selfPlayer", "name": "Self", "x": 10, "y": 20},
                {"objectId": "2", "kind": "player", "name": "Remote", "x": 11, "y": 20},
                {"objectId": 3, "kind": "npc", "name": "Town_Guard", "x": 12, "y": 20},
                {"objectId": 4, "kind": "monster", "name": "Deer", "x": 13, "y": 20},
                {"objectId": 5, "kind": "monster", "name": "", "x": 14, "y": 20},
                {"objectId": 6, "kind": "monster", "name": "Corpse", "x": 15, "y": 20, "dead": true}
            ]
        });
        let off = OverlayVisibility {
            name_view: false,
            drop_view: false,
        };
        let hidden_names = overlay_entries(&payload, off, None, false, None);
        assert!(entry_names(&hidden_names).is_empty());
        assert_eq!(health_ratios(&hidden_names), [0.5]);

        let self_only = overlay_entries(&payload, off, None, true, None);
        assert_eq!(entry_names(&self_only), ["Self"]);
        assert_eq!(health_ratios(&self_only), [0.5]);
        for (object_id, expected) in [("2", "Remote"), ("3", "Town\nGuard"), ("4", "Deer")] {
            let hovered = overlay_entries(&payload, off, Some(object_id), false, None);
            assert_eq!(entry_names(&hovered), [expected]);
            assert_eq!(health_ratios(&hovered), [0.5]);
        }

        let overlapping = overlay_entries(&payload, off, Some("2"), true, None);
        assert_eq!(entry_names(&overlapping), ["Self", "Remote"]);

        let on = overlay_entries(
            &payload,
            OverlayVisibility {
                name_view: true,
                drop_view: false,
            },
            Some("4"),
            true,
            None,
        );
        assert_eq!(
            entry_names(&on),
            ["Self", "Remote", "Town\nGuard", "Deer", "Corpse"]
        );
    }

    #[test]
    fn dead_entities_keep_crystal_nameplates_shifted_down_without_self_health() {
        let entries = overlay_entries(
            &json!({
                "sceneView": {"center": {"x": 10, "y": 20}},
                "playerHp": 0,
                "playerMaxHp": 18,
                "entities": [
                    {"objectId": 1, "kind": "player", "name": "Living", "x": 10, "y": 20, "dead": false},
                    {"objectId": 2, "kind": "player", "name": "Fallen", "x": 10, "y": 20, "dead": true},
                    {"objectId": 3, "kind": "monster", "name": "Living_Deer", "x": 11, "y": 20, "dead": false},
                    {"objectId": 4, "kind": "monster", "name": "Fallen_Deer", "x": 11, "y": 20, "dead": true},
                    {"objectId": 5, "kind": "selfPlayer", "name": "SelfCorpse", "x": 12, "y": 20, "dead": true}
                ]
            }),
            OverlayVisibility {
                name_view: true,
                drop_view: false,
            },
            None,
            false,
            None,
        );
        assert_eq!(
            entry_names(&entries),
            [
                "Living",
                "Fallen",
                "Living\nDeer",
                "Fallen\nDeer",
                "SelfCorpse"
            ]
        );
        assert_eq!(entries[1].top - entries[0].top, 27.0);
        assert_eq!(entries[3].top - entries[2].top, 27.0);
        assert!(health_ratios(&entries).is_empty());
    }

    #[test]
    fn npc_quest_markers_follow_authoritative_quest_status() {
        let payload = json!({
            "sceneView": {"center": {"x": 10, "y": 20}},
            "entities": [
                {"objectId": 3, "kind": "npc", "name": "Assistant_Jane", "x": 12, "y": 20, "questIds": [1], "questIcon": 2},
                {"objectId": 4, "kind": "npc", "name": "CraftsLady_Jude", "x": 13, "y": 20, "questIds": [2], "questIcon": 1},
                {"objectId": 5, "kind": "npc", "name": "Merchant_Ruben", "x": 14, "y": 20, "questIds": [3], "questIcon": 3}
            ]
        });
        let tracker = QuestTracker {
            active_quests: vec![
                mir2_client_bevy::quest_model::Quest {
                    quest_index: 1,
                    accept_npc_index: Some(3),
                    finish_npc_index: Some(3),
                    title: "Available".to_owned(),
                    npc_name: Some("Assistant Jane".to_owned()),
                    status: QuestStatus::NotStarted,
                    objectives: vec![],
                    rewards: vec![],
                    unknown_text: None,
                },
                mir2_client_bevy::quest_model::Quest {
                    quest_index: 2,
                    accept_npc_index: Some(4),
                    finish_npc_index: Some(4),
                    title: "Progress".to_owned(),
                    npc_name: Some("CraftsLady Jude".to_owned()),
                    status: QuestStatus::InProgress,
                    objectives: vec![],
                    rewards: vec![],
                    unknown_text: None,
                },
                mir2_client_bevy::quest_model::Quest {
                    quest_index: 3,
                    accept_npc_index: Some(5),
                    finish_npc_index: Some(5),
                    title: "Turn In".to_owned(),
                    npc_name: Some("Merchant Ruben".to_owned()),
                    status: QuestStatus::ReadyToTurnIn,
                    objectives: vec![],
                    rewards: vec![],
                    unknown_text: None,
                },
            ],
        };
        let entries = overlay_entries(
            &payload,
            OverlayVisibility {
                name_view: true,
                drop_view: false,
            },
            None,
            false,
            Some(&tracker),
        );
        assert_eq!(
            entry_names(&entries),
            ["Assistant\nJane", "CraftsLady\nJude", "Merchant\nRuben"]
        );
        assert_eq!(
            quest_markers(&entries),
            [
                QuestMarkerKind::ExclamationYellow,
                QuestMarkerKind::QuestionWhite,
                QuestMarkerKind::QuestionYellow
            ]
        );
        assert_eq!(entries[0].left, 588.0);
        assert_eq!(entries[0].top, 294.0);
        assert_eq!(
            QuestMarkerKind::QuestionWhite.asset_path(0),
            "original-ui/Prguse/983.png"
        );
        assert_eq!(
            QuestMarkerKind::QuestionWhite.asset_path(1),
            "original-ui/Prguse/984.png"
        );
        assert_eq!(
            QuestMarkerKind::ExclamationYellow.asset_path(0),
            "original-ui/Prguse/985.png"
        );
        assert_eq!(
            QuestMarkerKind::ExclamationYellow.asset_path(1),
            "original-ui/Prguse/986.png"
        );
        assert_eq!(
            QuestMarkerKind::QuestionYellow.asset_path(0),
            "original-ui/Prguse/987.png"
        );
        assert_eq!(
            QuestMarkerKind::QuestionYellow.asset_path(1),
            "original-ui/Prguse/988.png"
        );
        assert_eq!(
            QuestMarkerKind::ExclamationBlue.asset_path(0),
            "original-ui/Prguse/991.png"
        );
        assert_eq!(
            QuestMarkerKind::QuestionBlue.asset_path(1),
            "original-ui/Prguse/994.png"
        );
        assert_eq!(
            QuestMarkerKind::ExclamationGreen.asset_path(0),
            "original-ui/Prguse/1085.png"
        );
        assert_eq!(
            QuestMarkerKind::QuestionGreen.asset_path(1),
            "original-ui/Prguse/1088.png"
        );
        assert_eq!(quest_marker_animation_phase(0), 0);
        assert_eq!(quest_marker_animation_phase(499), 0);
        assert_eq!(quest_marker_animation_phase(500), 1);
        assert_eq!(quest_marker_animation_phase(1_000), 0);

        let mixed = json!({"objectId": 4, "questIds": [1, 2, 3]});
        assert_eq!(
            quest_marker_for_entity(&mixed, Some(&tracker)),
            Some(QuestMarkerKind::QuestionWhite),
            "the current quest targeting this NPC must win over unrelated statuses"
        );
        assert_eq!(
            quest_marker_for_entity(&json!({"objectId": 4}), Some(&tracker)),
            Some(QuestMarkerKind::QuestionWhite),
            "legacy snapshots missing questIds must recover from tracker NPC indexes"
        );
        assert_eq!(
            quest_marker_for_entity(&json!({"objectId": 4, "questIds": []}), Some(&tracker)),
            Some(QuestMarkerKind::QuestionWhite),
            "Crystal current-quest matching must not depend on the NPC available list"
        );
        assert_eq!(
            quest_marker_for_entity(&json!({"objectId": 99}), Some(&tracker)),
            None,
            "a quest marker must not appear on an NPC with the wrong role"
        );
        assert_eq!(
            quest_marker_for_entity(&json!({"questIcon": 53}), None),
            Some(QuestMarkerKind::QuestionGreen),
            "authoritative questIcon must not depend on a client tracker"
        );
    }

    #[test]
    fn argb_requires_alpha_and_preserves_channels() {
        assert!(argb_color(-1).is_none());
        assert!(argb_color(0x00ff_0000).is_none());
        assert_eq!(
            argb_color(0xff12_3456),
            Some(Color::srgba_u8(0x12, 0x34, 0x56, 0xff))
        );
    }

    #[test]
    fn nameplate_outline_matches_mir_label_four_pass_geometry() {
        assert_eq!(
            crystal_outline_offsets(),
            [
                Vec2::new(1.0, 0.0),
                Vec2::new(0.0, 1.0),
                Vec2::new(2.0, 1.0),
                Vec2::new(1.0, 2.0),
            ]
        );
    }

    fn entry_names(entries: &[OverlayEntry]) -> Vec<&str> {
        entries
            .iter()
            .filter_map(|entry| entry.name.as_deref())
            .collect()
    }

    fn health_ratios(entries: &[OverlayEntry]) -> Vec<f32> {
        entries
            .iter()
            .filter_map(|entry| entry.self_health_ratio)
            .collect()
    }

    fn quest_markers(entries: &[OverlayEntry]) -> Vec<QuestMarkerKind> {
        entries
            .iter()
            .filter_map(|entry| entry.quest_marker)
            .collect()
    }

    #[test]
    fn packet_damage_events_are_deduplicated_capped_and_animated_over_target() {
        let mut overlays = NativeEntityOverlays::default();
        let event = NativeDamageEvent {
            sequence: 1,
            object_id: 2,
            damage: 12,
            damage_type: 2,
        };
        overlays.observe_damage_events(std::slice::from_ref(&event), 1_000);
        overlays.observe_damage_events(std::slice::from_ref(&event), 1_100);
        assert_eq!(overlays.active_floaters.len(), 1);
        assert_eq!(overlays.active_floaters[0].text, "12");
        assert_eq!(overlays.active_floaters[0].variant, DamageVariant::Critical);

        let entries = damage_floater_entries(
            &json!({
                "sceneView": {"center": {"x": 10, "y": 20}},
                "playerObjectId": 1,
                "entities": [
                    {"objectId": 1, "kind": "selfPlayer", "x": 10, "y": 20},
                    {"objectId": 2, "kind": "monster", "x": 11, "y": 19}
                ]
            }),
            &overlays.active_floaters,
            1_900,
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].text, "12");
        assert_eq!(entries[0].left, 512.0);
        assert!(entries[0].top < 255.0);
        assert_eq!(entries[0].font_size, 18.0);
        overlays.reset_session();
        assert!(overlays.active_floaters.is_empty());
        assert_eq!(overlays.last_damage_sequence, 0);
    }
}
