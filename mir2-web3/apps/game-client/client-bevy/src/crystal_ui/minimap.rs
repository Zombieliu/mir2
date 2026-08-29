//! Crystal large-minimap presentation for the Windows-native client.
//!
//! The first visual Candidate intentionally supports the required Bichon
//! vertical slice. The source minimap index is server data in Crystal; the
//! current renderer-neutral model does not expose it yet, so the profile is
//! selected from the authoritative map title without changing shared schema.

use bevy::prelude::*;
use bevy::ui::{widget::NodeImageMode, Display, Node, PositionType, Val};

use crate::crystal_ui::overlays::{NativePlayerUiSet, NativePlayerUiState};
use crate::entities::{EntityKind, EntityModel, EntityModelSet};
use crate::map::MapModel;
use crate::native_shell::{NativeShellModel, NativeShellScreen};
use crate::read_model::UiReadModel;

const VIEW_LEFT: f32 = 901.0;
const VIEW_TOP: f32 = 22.0;
const VIEW_WIDTH: f32 = 120.0;
const VIEW_HEIGHT: f32 = 108.0;

#[derive(Component)]
pub struct CrystalMiniMapRoot;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MiniMapProfile {
    pub image_index: u16,
    pub image_width: f32,
    pub image_height: f32,
    pub map_width: f32,
    pub map_height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MiniMapCrop {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

pub struct Mir2CrystalMiniMapPlugin;

impl Plugin for Mir2CrystalMiniMapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_crystal_minimap).add_systems(
            Update,
            render_crystal_minimap.after(NativePlayerUiSet::Mutate),
        );
    }
}

pub fn mini_map_profile(map_name: Option<&str>) -> Option<MiniMapProfile> {
    let map_name = map_name?.trim();
    if map_name.eq_ignore_ascii_case("BichonProvince")
        || map_name.eq_ignore_ascii_case("Bichon Province")
    {
        return Some(MiniMapProfile {
            image_index: 101,
            image_width: 1050.0,
            image_height: 700.0,
            map_width: 700.0,
            map_height: 700.0,
        });
    }
    None
}

pub fn source_crop(profile: MiniMapProfile, center_x: i32, center_y: i32) -> MiniMapCrop {
    let scale_x = profile.image_width / profile.map_width;
    let scale_y = profile.image_height / profile.map_height;
    let max_left = (profile.image_width - VIEW_WIDTH).max(0.0);
    let max_top = (profile.image_height - VIEW_HEIGHT).max(0.0);
    MiniMapCrop {
        left: (center_x as f32 * scale_x - VIEW_WIDTH * 0.5).clamp(0.0, max_left),
        top: (center_y as f32 * scale_y - VIEW_HEIGHT * 0.5).clamp(0.0, max_top),
        width: VIEW_WIDTH.min(profile.image_width),
        height: VIEW_HEIGHT.min(profile.image_height),
    }
}

pub fn marker_position(
    profile: MiniMapProfile,
    crop: MiniMapCrop,
    entity_x: i32,
    entity_y: i32,
) -> Option<Vec2> {
    let source_x = entity_x as f32 * profile.image_width / profile.map_width;
    let source_y = entity_y as f32 * profile.image_height / profile.map_height;
    let x = source_x - crop.left;
    let y = source_y - crop.top;
    (x >= 0.0 && x < crop.width && y >= 0.0 && y < crop.height).then_some(Vec2::new(x, y))
}

pub fn marker_color(kind: EntityKind) -> Color {
    match kind {
        EntityKind::SelfPlayer => Color::WHITE,
        EntityKind::Npc => Color::srgb(0.0, 1.0, 0.0),
        EntityKind::Monster => Color::srgb(1.0, 0.0, 0.0),
        EntityKind::Player => Color::srgb(0.2, 0.45, 1.0),
    }
}

fn spawn_crystal_minimap(mut commands: Commands) {
    commands.spawn((
        CrystalMiniMapRoot,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Px(1024.0),
            height: Val::Px(768.0),
            display: Display::None,
            ..default()
        },
        GlobalZIndex(905),
    ));
}

fn render_crystal_minimap(
    shell: Option<Res<NativeShellModel>>,
    ui_model: Res<UiReadModel>,
    map_model: Res<MapModel>,
    entities: Res<EntityModelSet>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut roots: Query<(Entity, &mut Node), With<CrystalMiniMapRoot>>,
    minimap_state: Option<Res<NativePlayerUiState>>,
) {
    let Ok((root_entity, mut root_node)) = roots.single_mut() else {
        return;
    };
    let in_game = shell
        .as_deref()
        .is_some_and(|shell| shell.screen == NativeShellScreen::InGame);
    let minimap_visible = minimap_state
        .as_deref()
        .map(|s| s.minimap_visible())
        .unwrap_or(true);
    let profile = mini_map_profile(ui_model.player.map_name.as_deref());
    let visible = in_game && minimap_visible && profile.is_some();
    root_node.display = if visible {
        Display::Flex
    } else {
        Display::None
    };
    if !visible {
        commands.entity(root_entity).despawn_children();
        return;
    }

    let shell_changed = shell.as_ref().is_some_and(|shell| shell.is_changed());
    let minimap_changed = minimap_state.as_ref().is_some_and(|s| s.is_changed());
    if !shell_changed
        && !ui_model.is_changed()
        && !map_model.is_changed()
        && !entities.is_changed()
        && !minimap_changed
    {
        return;
    }

    commands.entity(root_entity).despawn_children();
    let Some(profile) = profile else {
        return;
    };
    let crop = source_crop(profile, map_model.center_x, map_model.center_y);

    commands.entity(root_entity).with_children(|root| {
        root.spawn((
            absolute_node(VIEW_LEFT, VIEW_TOP, VIEW_WIDTH, VIEW_HEIGHT),
            ImageNode {
                image: asset_server.load(format!("original-ui/MMap/{}.png", profile.image_index)),
                rect: Some(Rect::new(
                    crop.left,
                    crop.top,
                    crop.left + crop.width,
                    crop.top + crop.height,
                )),
                image_mode: NodeImageMode::Stretch,
                ..default()
            },
        ));

        for entity in &entities.entities {
            spawn_marker(root, profile, crop, entity);
        }
    });
}

fn spawn_marker(
    parent: &mut ChildSpawnerCommands,
    profile: MiniMapProfile,
    crop: MiniMapCrop,
    entity: &EntityModel,
) {
    let Some(position) = marker_position(profile, crop, entity.x, entity.y) else {
        return;
    };
    parent.spawn((
        absolute_node(
            VIEW_LEFT + position.x - 1.0,
            VIEW_TOP + position.y - 1.0,
            2.0,
            2.0,
        ),
        BackgroundColor(marker_color(entity.kind)),
    ));
}

fn absolute_node(left: f32, top: f32, width: f32, height: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(left),
        top: Val::Px(top),
        width: Val::Px(width),
        height: Val::Px(height),
        ..default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bichon_profile_matches_source_map_and_exported_minimap() {
        assert_eq!(
            mini_map_profile(Some("BichonProvince")),
            Some(MiniMapProfile {
                image_index: 101,
                image_width: 1050.0,
                image_height: 700.0,
                map_width: 700.0,
                map_height: 700.0,
            })
        );
        assert_eq!(mini_map_profile(Some("Unknown")), None);
    }

    #[test]
    fn source_crop_centres_the_bichon_player_and_clamps_edges() {
        let profile = mini_map_profile(Some("BichonProvince")).unwrap();
        assert_eq!(
            source_crop(profile, 335, 262),
            MiniMapCrop {
                left: 442.5,
                top: 208.0,
                width: 120.0,
                height: 108.0,
            }
        );
        assert_eq!(source_crop(profile, 0, 0).left, 0.0);
        assert_eq!(source_crop(profile, 0, 0).top, 0.0);
    }

    #[test]
    fn marker_positions_share_the_same_source_crop_transform() {
        let profile = mini_map_profile(Some("BichonProvince")).unwrap();
        let crop = source_crop(profile, 335, 262);
        assert_eq!(
            marker_position(profile, crop, 335, 262),
            Some(Vec2::new(60.0, 54.0))
        );
        assert_eq!(marker_position(profile, crop, 0, 0), None);
        assert_eq!(marker_color(EntityKind::SelfPlayer), Color::WHITE);
    }
}
