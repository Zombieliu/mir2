//! Shared Bevy character panel.
//!
//! Renders the player's stats (name / level / HP / MP / gold / map) from the
//! shared [`UiReadModel`] — the same read model the HUD uses. Purely
//! presentational; character progression stays server-authoritative.

use bevy::prelude::*;
use bevy::ui::{
    AlignItems, BackgroundColor, Display, FlexDirection, Node, PositionType, UiRect, Val,
};

use crate::read_model::UiReadModel;

/// Marker on the character panel root.
#[derive(Component)]
pub struct CharacterPanelRoot;

/// Marker on the character stats text.
#[derive(Component)]
pub struct CharacterText;

/// Build the shared Mir2 character panel (initially hidden, toggled by hosts).
pub struct Mir2CharacterPlugin;

impl Plugin for Mir2CharacterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_character_panel).add_systems(
            Update,
            update_character_panel.run_if(resource_changed::<UiReadModel>),
        );
    }
}

fn spawn_character_panel(mut commands: Commands) {
    commands
        .spawn((
            CharacterPanelRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(12.0),
                top: Val::Px(150.0),
                width: Val::Px(200.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexStart,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.72)),
        ))
        .with_children(|parent| {
            parent.spawn((
                CharacterText,
                Text::new(""),
                TextFont {
                    font_size: bevy::prelude::FontSize::Px(13.0),
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.92, 0.86)),
            ));
        });
}

fn update_character_panel(model: Res<UiReadModel>, texts: Query<&mut Text, With<CharacterText>>) {
    let label = character_panel_label(&model);
    for mut text in texts {
        text.0 = label.clone();
    }
}

/// Build the character panel text (renderer-neutral, testable).
pub fn character_panel_label(model: &UiReadModel) -> String {
    let name = model.player.name.as_deref().unwrap_or("?");
    let map = model.player.map_name.as_deref().unwrap_or("");
    format!(
        "{name}\nLv. {}\n\nHP   {}\nMP   {}\nGold {}\nMap  {map}",
        model.player.level,
        model.player.hp_label(),
        model.player.mp_label(),
        model.player.gold_label(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read_model::PlayerStats;

    #[test]
    fn character_panel_shows_stats() {
        let model = UiReadModel {
            player: PlayerStats {
                hp: 80,
                max_hp: 100,
                mp: 20,
                max_mp: 50,
                gold: 999,
                level: 7,
                name: Some("Hero".to_owned()),
                map_name: Some("BichonProvince".to_owned()),
            },
        };
        let label = character_panel_label(&model);
        assert!(label.contains("Hero"));
        assert!(label.contains("Lv. 7"));
        assert!(label.contains("80 / 100"));
        assert!(label.contains("20 / 50"));
        assert!(label.contains("999"));
        assert!(label.contains("BichonProvince"));
    }
}
