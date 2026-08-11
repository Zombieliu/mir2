//! Shared in-game HUD.
//!
//! Renders HP / MP bars, gold, level and player name from the shared
//! [`UiReadModel`]. The same read model the Web React surface consumes, so
//! native and Web HUD values never drift. Purely presentational: it never
//! mutates gameplay state or grants anything.

use bevy::prelude::*;
use bevy::ui::{
    AlignItems, BackgroundColor, Display, FlexDirection, JustifyContent, Node, PositionType,
    UiRect, Val,
};

use crate::read_model::UiReadModel;

/// Color palette matching the Crystal HUD (hp red, mp blue, gold amber).
pub const HP_BAR_COLOR: Color = Color::srgb(0.85, 0.15, 0.15);
pub const MP_BAR_COLOR: Color = Color::srgb(0.15, 0.45, 0.90);
pub const BAR_BACKGROUND_COLOR: Color = Color::srgba(0.0, 0.0, 0.0, 0.55);
pub const HUD_TEXT_COLOR: Color = Color::srgb(0.95, 0.95, 0.90);
pub const GOLD_COLOR: Color = Color::srgb(0.95, 0.72, 0.22);

/// Width of the HP/MP bar in CSS px.
const BAR_WIDTH: f32 = 220.0;
/// Height of one bar row.
const BAR_HEIGHT: f32 = 14.0;

/// The root UI node the HUD is spawned under.
#[derive(Component)]
pub struct HudRoot;

/// Marker for the HP fill bar (resized each frame to the HP fraction).
#[derive(Component)]
pub struct HpFill;

/// Marker for the MP fill bar.
#[derive(Component)]
pub struct MpFill;

/// Marker for the HUD text node holding gold / level / name.
#[derive(Component)]
pub struct HudText;

/// Build the shared Mir2 in-game HUD.
pub struct Mir2HudPlugin;

impl Plugin for Mir2HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiReadModel>()
            .add_systems(Startup, spawn_hud_root)
            .add_systems(Update, (update_hud_bars, update_hud_text).chain());
    }
}

fn spawn_hud_root(mut commands: Commands) {
    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(12.0),
                width: Val::Px(BAR_WIDTH),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::FlexStart,
                row_gap: Val::Px(4.0),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                HpFill,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(BAR_HEIGHT),
                    ..default()
                },
                BackgroundColor(HP_BAR_COLOR),
            ));
            parent.spawn((
                MpFill,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(BAR_HEIGHT),
                    ..default()
                },
                BackgroundColor(MP_BAR_COLOR),
            ));
            parent.spawn((
                HudText,
                Node {
                    width: Val::Px(BAR_WIDTH),
                    padding: UiRect::top(Val::Px(4.0)),
                    ..default()
                },
                Text::new(""),
                TextFont {
                    font_size: bevy::prelude::FontSize::Px(14.0),
                    ..default()
                },
                TextColor(HUD_TEXT_COLOR),
            ));
        });
}

fn update_hud_bars(
    model: Res<UiReadModel>,
    mut bar_nodes: ParamSet<(
        Query<&mut Node, With<HpFill>>,
        Query<&mut Node, With<MpFill>>,
    )>,
) {
    let hp = model.player.normalized_hp();
    let mp = model.player.normalized_mp();

    for mut node in bar_nodes.p0() {
        node.width = Val::Percent(hp * 100.0);
    }
    for mut node in bar_nodes.p1() {
        node.width = Val::Percent(mp * 100.0);
    }
}

fn update_hud_text(model: Res<UiReadModel>, texts: Query<&mut Text, With<HudText>>) {
    let label = hud_text_label(&model);
    for mut text in texts {
        text.0 = label.clone();
    }
}
/// Build the multi-line HUD text (name / level / gold / hp-mp) without Bevy
/// dependencies so it is unit-testable.
pub fn hud_text_label(model: &UiReadModel) -> String {
    let name = model.player.name.as_deref().unwrap_or("?");
    let map = model.player.map_name.as_deref().unwrap_or("");
    format!(
        "{name}  Lv.{}\n{}\n{}  {}  {}",
        model.player.level,
        map,
        model.player.gold_label(),
        model.player.hp_label(),
        model.player.mp_label(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read_model::PlayerStats;

    #[test]
    fn hud_label_includes_name_level_gold_and_bars() {
        let model = UiReadModel {
            player: PlayerStats {
                hp: 50,
                max_hp: 100,
                mp: 25,
                max_mp: 50,
                gold: 1234,
                level: 3,
                name: Some("Demo".to_owned()),
                map_name: Some("BichonProvince".to_owned()),
            },
        };
        let label = hud_text_label(&model);
        assert!(label.contains("Demo"));
        assert!(label.contains("Lv.3"));
        assert!(label.contains("BichonProvince"));
        assert!(label.contains("1234"));
        assert!(label.contains("50 / 100"));
        assert!(label.contains("25 / 50"));
    }

    #[test]
    fn hud_label_falls_back_when_name_missing() {
        let model = UiReadModel {
            player: PlayerStats::default(),
        };
        let label = hud_text_label(&model);
        assert!(label.starts_with("?  Lv.0"));
    }

    #[test]
    fn hp_and_mp_fills_use_crystal_bar_colors() {
        let mut app = App::new();
        app.add_plugins(Mir2HudPlugin);
        app.update();

        let world = app.world_mut();
        let hp = world
            .query_filtered::<&BackgroundColor, With<HpFill>>()
            .iter(world)
            .next()
            .expect("hp fill");
        assert_eq!(hp.0, HP_BAR_COLOR);
        let mp = world
            .query_filtered::<&BackgroundColor, With<MpFill>>()
            .iter(world)
            .next()
            .expect("mp fill");
        assert_eq!(mp.0, MP_BAR_COLOR);
    }
}
