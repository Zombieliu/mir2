//! Native Crystal entity nameplates and the self health bar.
//!
//! Web keeps these as DOM overlays. The Windows host owns an equivalent Bevy
//! UI layer so native entity sprites do not depend on a browser surface.

use bevy::prelude::*;
use mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState;
use mir2_client_bevy::crystal_ui::typography::{
    crystal_text_font, CRYSTAL_DEFAULT_FONT_SIZE_PX,
};
use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};
use serde_json::Value;

use crate::gameplay_bridge::NativeDamageEvent;

const STAGE_WIDTH: f32 = 1024.0;
const STAGE_HEIGHT: f32 = 768.0;
const CELL_WIDTH: f32 = 48.0;
const CELL_HEIGHT: f32 = 32.0;
const OVERLAY_Z_INDEX: i32 = 850;

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
    name: String,
    color: Color,
    left: f32,
    top: f32,
    width: f32,
    self_health_ratio: Option<f32>,
}

pub fn sync_native_entity_overlays(
    mut commands: Commands,
    shell: Res<NativeShellModel>,
    mut overlays: ResMut<NativeEntityOverlays>,
    roots: Query<Entity, With<NativeEntityOverlayRoot>>,
    time: Res<Time>,
    player_ui: Option<Res<NativePlayerUiState>>,
) {
    let now_ms = u64::try_from(time.elapsed().as_millis()).unwrap_or(u64::MAX);
    let previous_floater_count = overlays.active_floaters.len();
    overlays
        .active_floaters
        .retain(|floater| floater.expires_at_ms > now_ms);
    if overlays.active_floaters.len() != previous_floater_count
        || !overlays.active_floaters.is_empty()
    {
        overlays.dirty = true;
    }
    let in_game = shell.screen == NativeShellScreen::InGame;
    let visibility = OverlayVisibility::from_player_ui(player_ui.as_deref());
    if !overlays.dirty
        && overlays.last_in_game == in_game
        && overlays.last_visibility == Some(visibility)
    {
        return;
    }
    overlays.last_in_game = in_game;
    overlays.last_visibility = Some(visibility);
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
    let entries = overlay_entries(payload, visibility);
    let floaters = damage_floater_entries(payload, &overlays.active_floaters, now_ms);
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
                        Text::new(entry.name.clone()),
                        crystal_text_font(CRYSTAL_DEFAULT_FONT_SIZE_PX),
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
                    Text::new(entry.name),
                    crystal_text_font(CRYSTAL_DEFAULT_FONT_SIZE_PX),
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
                left: origin_x + (x - center_x) as f32 * CELL_WIDTH - 16.0,
                top: origin_y + (y - center_y) as f32 * CELL_HEIGHT - 65.0 + rise,
                width: 80.0,
                font_size,
            })
        })
        .collect()
}

fn overlay_entries(payload: &Value, visibility: OverlayVisibility) -> Vec<OverlayEntry> {
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
    if visibility.name_view {
        entries.extend(
            payload
                .get("entities")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|entity| {
                    let name = entity.get("name")?.as_str()?.trim();
                    if name.is_empty() {
                        return None;
                    }
                    let kind = entity
                        .get("kind")
                        .and_then(Value::as_str)
                        .unwrap_or("monster");
                    let x = entity.get("x").and_then(value_i64).unwrap_or(0);
                    let y = entity.get("y").and_then(value_i64).unwrap_or(0);
                    let dead = entity.get("dead").and_then(Value::as_bool) == Some(true);
                    if dead {
                        return None;
                    }
                    let is_self = kind == "selfPlayer";
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
                    let top_offset = if matches!(kind, "npc" | "monster") {
                        -18.0
                    } else {
                        -17.0
                    } + line_adjustment;
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
                    let self_health_ratio = is_self
                        .then(|| {
                            let hp = entity.get("hp").and_then(value_i64).or(player_hp)?;
                            let max_hp =
                                entity.get("maxHp").and_then(value_i64).or(player_max_hp)?;
                            (max_hp > 0).then_some((hp as f32 / max_hp as f32).clamp(0.0, 1.0))
                        })
                        .flatten();

                    Some(OverlayEntry {
                        name: display_name,
                        color,
                        left: origin_x + (x - center_x) as f32 * CELL_WIDTH,
                        top: origin_y + (y - center_y) as f32 * CELL_HEIGHT + top_offset,
                        width: if matches!(kind, "npc" | "monster") {
                            48.0
                        } else {
                            50.0
                        },
                        self_health_ratio,
                    })
                }),
        );
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
                        name: name.to_owned(),
                        color: Color::srgb_u8(0xff, 0xe6, 0x58),
                        left: origin_x + (x - center_x) as f32 * CELL_WIDTH - 16.0,
                        top: origin_y + (y - center_y) as f32 * CELL_HEIGHT - 18.0,
                        width: 80.0,
                        self_health_ratio: None,
                    })
                }),
        );
    }
    entries
}

fn value_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| value.as_str()?.parse().ok())
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
                    {"objectId": 1, "kind": "selfPlayer", "name": "Hero", "x": 10, "y": 20},
                    {"objectId": 2, "kind": "npc", "name": "Weapon_Smith", "x": 11, "y": 19}
                ]
            }),
            OverlayVisibility {
                name_view: true,
                drop_view: true,
            },
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].left, 480.0);
        assert_eq!(entries[0].top, 335.0);
        assert_eq!(entries[0].self_health_ratio, Some(0.5));
        assert_eq!(entries[1].name, "Weapon\nSmith");
        assert_eq!(entries[1].left, 528.0);
        assert_eq!(entries[1].top, 297.0);
        assert_eq!(entries[1].width, 48.0);
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
        );
        assert_eq!(
            names_only
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            ["Hero"]
        );

        let drops_only = overlay_entries(
            &payload,
            OverlayVisibility {
                name_view: false,
                drop_view: true,
            },
        );
        assert_eq!(
            drops_only
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            ["Potion"]
        );

        assert!(overlay_entries(
            &payload,
            OverlayVisibility {
                name_view: false,
                drop_view: false
            },
        )
        .is_empty());
    }

    #[test]
    fn dead_entities_do_not_emit_crystal_nameplates() {
        let entries = overlay_entries(
            &json!({
                "sceneView": {"center": {"x": 10, "y": 20}},
                "entities": [
                    {"objectId": 1, "kind": "selfPlayer", "name": "Fallen", "x": 10, "y": 20, "dead": true},
                    {"objectId": 2, "kind": "monster", "name": "Living_Deer", "x": 11, "y": 20, "dead": false}
                ]
            }),
            OverlayVisibility {
                name_view: true,
                drop_view: false,
            },
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Living\nDeer");
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
