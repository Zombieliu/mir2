//! Crystal large-minimap presentation for the Windows-native client.
//!
//! The source minimap index and map dimensions are authoritative map data.
//! The renderer combines them with the loaded MMap image dimensions, so it
//! never guesses geometry from a localized map title.

use bevy::prelude::*;
use bevy::ui::{widget::NodeImageMode, Display, Node, PositionType, Val};

use crate::big_map::BigMapModel;
use crate::crystal_ui::overlays::{NativePlayerUiSet, NativePlayerUiState, OVERLAY_MINIMAP_Z};
use crate::crystal_ui::quest_targets::tracker_targets_monster;
use crate::entities::{EntityKind, EntityModel, EntityModelSet};
use crate::map::MapModel;
use crate::native_shell::{NativeShellModel, NativeShellScreen};
use crate::quest_model::QuestTracker;

const VIEW_LEFT: f32 = 901.0;
const VIEW_TOP: f32 = 22.0;
const VIEW_WIDTH: f32 = 120.0;
const VIEW_HEIGHT: f32 = 108.0;

#[derive(Component)]
pub struct CrystalMiniMapRoot;

/// Keeps the current image handle alive while the asynchronous asset load is
/// pending, and replaces it only when the authoritative MMap index changes.
#[derive(Resource, Default)]
struct MiniMapAssetState {
    image_index: Option<u16>,
    image: Option<Handle<Image>>,
}

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

/// Geometry of the image actually displayed during the preceding frame.
/// Input must use this crop, not a newly predicted player/camera centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MiniMapView {
    pub map_index: i32,
    pub map_epoch: u64,
    pub profile: MiniMapProfile,
    pub crop: MiniMapCrop,
}

#[derive(Debug, Default, Resource)]
pub struct MiniMapViewState {
    pub displayed: Option<MiniMapView>,
}

pub fn mini_map_contains(point: Vec2) -> bool {
    (VIEW_LEFT..VIEW_LEFT + VIEW_WIDTH).contains(&point.x)
        && (VIEW_TOP..VIEW_TOP + VIEW_HEIGHT).contains(&point.y)
}

impl MiniMapView {
    pub fn matches_map(self, map: &MapModel, identity: &BigMapModel) -> bool {
        identity.current_map_index == Some(self.map_index)
            && identity.reset_epoch == self.map_epoch
            && map.mini_map_index == Some(self.profile.image_index)
            && map.map_width.map(f32::from) == Some(self.profile.map_width)
            && map.map_height.map(f32::from) == Some(self.profile.map_height)
    }

    /// Inverse of the ImageNode crop/stretch and marker projection above.
    pub fn tile_at(self, point: Vec2) -> Option<(i32, i32)> {
        if !mini_map_contains(point) {
            return None;
        }
        let x = (self.crop.left + (point.x - VIEW_LEFT) * self.crop.width / VIEW_WIDTH)
            * self.profile.map_width
            / self.profile.image_width;
        let y = (self.crop.top + (point.y - VIEW_TOP) * self.crop.height / VIEW_HEIGHT)
            * self.profile.map_height
            / self.profile.image_height;
        ((0.0..self.profile.map_width).contains(&x) && (0.0..self.profile.map_height).contains(&y))
            .then_some((x.floor() as i32, y.floor() as i32))
    }
}

pub struct Mir2CrystalMiniMapPlugin;

impl Plugin for Mir2CrystalMiniMapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_crystal_minimap)
            .init_resource::<MiniMapAssetState>()
            .init_resource::<MiniMapViewState>()
            .add_systems(
                Update,
                render_crystal_minimap.after(NativePlayerUiSet::Mutate),
            );
    }
}

pub fn mini_map_profile(
    mini_map_index: Option<u16>,
    map_width: Option<u16>,
    map_height: Option<u16>,
    image_width: u32,
    image_height: u32,
) -> Option<MiniMapProfile> {
    let image_index = mini_map_index.filter(|index| *index > 0)?;
    let map_width = map_width.filter(|value| *value > 0)?;
    let map_height = map_height.filter(|value| *value > 0)?;
    (image_width > 0 && image_height > 0).then_some(MiniMapProfile {
        image_index,
        image_width: image_width as f32,
        image_height: image_height as f32,
        map_width: map_width as f32,
        map_height: map_height as f32,
    })
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

fn marker_display_position(crop: MiniMapCrop, source_position: Vec2) -> Vec2 {
    Vec2::new(
        source_position.x * VIEW_WIDTH / crop.width,
        source_position.y * VIEW_HEIGHT / crop.height,
    )
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
        GlobalZIndex(OVERLAY_MINIMAP_Z),
    ));
}

fn render_crystal_minimap(
    shell: Option<Res<NativeShellModel>>,
    map_model: Res<MapModel>,
    entities: Res<EntityModelSet>,
    quest_tracker: Option<Res<QuestTracker>>,
    asset_server: Res<AssetServer>,
    images: Res<Assets<Image>>,
    mut asset_state: ResMut<MiniMapAssetState>,
    mut commands: Commands,
    mut roots: Query<(Entity, &mut Node), With<CrystalMiniMapRoot>>,
    minimap_state: Option<Res<NativePlayerUiState>>,
    map_identity: Option<Res<BigMapModel>>,
    mut view_state: ResMut<MiniMapViewState>,
) {
    let Ok((root_entity, mut root_node)) = roots.single_mut() else {
        view_state.displayed = None;
        return;
    };
    let in_game = shell
        .as_deref()
        .is_some_and(|shell| shell.screen == NativeShellScreen::InGame);
    let minimap_visible = minimap_state
        .as_deref()
        .map(|s| s.minimap_visible() && !s.local_keys.camera_hidden)
        .unwrap_or(true);
    let requested_image_index = map_model.mini_map_index;
    if asset_state.image_index != requested_image_index {
        asset_state.image_index = requested_image_index;
        asset_state.image = requested_image_index
            .map(|image_index| asset_server.load(format!("original-ui/MMap/{image_index}.png")));
    }
    let profile = asset_state
        .image
        .as_ref()
        .and_then(|handle| images.get(handle))
        .and_then(|image| {
            mini_map_profile(
                requested_image_index,
                map_model.map_width,
                map_model.map_height,
                image.width(),
                image.height(),
            )
        });
    let visible = in_game && minimap_visible && profile.is_some();
    root_node.display = if visible {
        Display::Flex
    } else {
        Display::None
    };
    if !visible {
        view_state.displayed = None;
        commands.entity(root_entity).despawn_children();
        return;
    }

    let shell_changed = shell.as_ref().is_some_and(|shell| shell.is_changed());
    let minimap_changed = minimap_state.as_ref().is_some_and(|s| s.is_changed());
    if !shell_changed
        && !map_model.is_changed()
        && !images.is_changed()
        && !entities.is_changed()
        && !quest_tracker
            .as_ref()
            .is_some_and(|tracker| tracker.is_changed())
        && !minimap_changed
        && !map_identity
            .as_ref()
            .is_some_and(|identity| identity.is_changed())
    {
        return;
    }

    commands.entity(root_entity).despawn_children();
    let Some(profile) = profile else {
        return;
    };
    let crop = source_crop(profile, map_model.center_x, map_model.center_y);
    view_state.displayed = map_identity.as_deref().and_then(|identity| {
        Some(MiniMapView {
            map_index: identity.current_map_index?,
            map_epoch: identity.reset_epoch,
            profile,
            crop,
        })
    });
    let image = asset_state
        .image
        .clone()
        .expect("loaded minimap profile retains its image handle");

    commands.entity(root_entity).with_children(|root| {
        root.spawn((
            absolute_node(VIEW_LEFT, VIEW_TOP, VIEW_WIDTH, VIEW_HEIGHT),
            ImageNode {
                image,
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
            let quest_target = entity.kind == EntityKind::Monster
                && quest_tracker
                    .as_deref()
                    .is_some_and(|tracker| tracker_targets_monster(tracker, &entity.name));
            spawn_marker(root, profile, crop, entity, quest_target);
        }
    });
}

fn spawn_marker(
    parent: &mut ChildSpawnerCommands,
    profile: MiniMapProfile,
    crop: MiniMapCrop,
    entity: &EntityModel,
    quest_target: bool,
) {
    let Some(source_position) = marker_position(profile, crop, entity.x, entity.y) else {
        return;
    };
    // ImageNode stretches a source crop smaller than the HUD viewport. Apply
    // the same display scale to markers so they remain on their map pixels.
    let position = marker_display_position(crop, source_position);
    let (size, color) = marker_style(entity.kind, quest_target);
    parent.spawn((
        absolute_node(
            VIEW_LEFT + position.x - size * 0.5,
            VIEW_TOP + position.y - size * 0.5,
            size,
            size,
        ),
        BackgroundColor(color),
    ));
}

fn marker_style(kind: EntityKind, quest_target: bool) -> (f32, Color) {
    if quest_target {
        (5.0, Color::srgb_u8(0xff, 0xe6, 0x58))
    } else {
        (2.0, marker_color(kind))
    }
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
    fn d401_profile_uses_authoritative_index_image_and_map_dimensions() {
        assert_eq!(
            mini_map_profile(Some(8), Some(200), Some(200), 300, 199),
            Some(MiniMapProfile {
                image_index: 8,
                image_width: 300.0,
                image_height: 199.0,
                map_width: 200.0,
                map_height: 200.0,
            })
        );
        assert_eq!(mini_map_profile(None, Some(200), Some(200), 300, 199), None);
        assert_eq!(mini_map_profile(Some(8), None, Some(200), 300, 199), None);
    }

    #[test]
    fn map_change_uses_the_new_authoritative_image_geometry() {
        let d401 = mini_map_profile(Some(8), Some(200), Some(200), 300, 199).unwrap();
        let next_map = mini_map_profile(Some(9), Some(400), Some(300), 600, 399).unwrap();
        assert_eq!(d401.image_index, 8);
        assert_eq!(next_map.image_index, 9);
        assert_ne!(source_crop(d401, 100, 100), source_crop(next_map, 100, 100));
    }

    #[test]
    fn source_crop_centres_the_d401_player_and_clamps_edges() {
        let profile = mini_map_profile(Some(8), Some(200), Some(200), 300, 199).unwrap();
        assert_eq!(
            source_crop(profile, 100, 100),
            MiniMapCrop {
                left: 90.0,
                top: 45.5,
                width: 120.0,
                height: 108.0,
            }
        );
        assert_eq!(source_crop(profile, 0, 0).left, 0.0);
        assert_eq!(source_crop(profile, 0, 0).top, 0.0);
    }

    #[test]
    fn marker_positions_share_the_same_source_crop_transform() {
        let profile = mini_map_profile(Some(8), Some(200), Some(200), 300, 199).unwrap();
        let crop = source_crop(profile, 100, 100);
        assert_eq!(
            marker_position(profile, crop, 100, 100),
            Some(Vec2::new(60.0, 54.0))
        );
        assert_eq!(marker_position(profile, crop, 0, 0), None);
        assert_eq!(marker_color(EntityKind::SelfPlayer), Color::WHITE);
    }

    #[test]
    fn small_source_crop_scales_markers_with_the_stretched_image() {
        let profile = mini_map_profile(Some(8), Some(100), Some(80), 100, 80).unwrap();
        let crop = source_crop(profile, 50, 40);
        assert_eq!(
            crop,
            MiniMapCrop {
                left: 0.0,
                top: 0.0,
                width: 100.0,
                height: 80.0
            }
        );
        let source = marker_position(profile, crop, 50, 40).expect("centre marker");
        assert_eq!(marker_display_position(crop, source), Vec2::new(60.0, 54.0));
    }

    #[test]
    fn quest_target_marker_is_larger_and_gold() {
        assert_eq!(
            marker_style(EntityKind::Monster, true),
            (5.0, Color::srgb_u8(0xff, 0xe6, 0x58))
        );
        assert_eq!(
            marker_style(EntityKind::Monster, false),
            (2.0, marker_color(EntityKind::Monster))
        );
    }

    #[test]
    fn click_inverse_uses_clamped_crop_and_small_image_stretch() {
        for profile in [
            mini_map_profile(Some(8), Some(200), Some(200), 300, 199).unwrap(),
            mini_map_profile(Some(14), Some(100), Some(80), 70, 50).unwrap(),
        ] {
            for centre in [(0, 0), (50, 40), (199, 199)] {
                let crop = source_crop(profile, centre.0, centre.1);
                let view = MiniMapView {
                    map_index: 1,
                    map_epoch: 3,
                    profile,
                    crop,
                };
                for (dx, dy) in [(0.0, 0.0), (60.0, 54.0), (119.9, 107.9)] {
                    let tile = view
                        .tile_at(Vec2::new(VIEW_LEFT + dx, VIEW_TOP + dy))
                        .unwrap();
                    let expected = (
                        ((crop.left + dx * crop.width / VIEW_WIDTH) / profile.image_width
                            * profile.map_width)
                            .floor() as i32,
                        ((crop.top + dy * crop.height / VIEW_HEIGHT) / profile.image_height
                            * profile.map_height)
                            .floor() as i32,
                    );
                    assert_eq!(tile, expected);
                    assert!(tile.0 >= 0 && (tile.0 as f32) < profile.map_width);
                    assert!(tile.1 >= 0 && (tile.1 as f32) < profile.map_height);
                }
                for point in [
                    Vec2::new(900.9, 40.0),
                    Vec2::new(1021.0, 40.0),
                    Vec2::new(920.0, 21.9),
                    Vec2::new(920.0, 130.0),
                    Vec2::splat(f32::NAN),
                ] {
                    assert_eq!(view.tile_at(point), None);
                }
            }
        }
    }

    #[test]
    fn click_snapshot_rejects_map_epoch_image_and_dimensions_but_retains_displayed_crop() {
        let profile = mini_map_profile(Some(8), Some(200), Some(200), 300, 199).unwrap();
        let view = MiniMapView {
            map_index: 4,
            map_epoch: 2,
            profile,
            crop: source_crop(profile, 100, 100),
        };
        let mut identity = BigMapModel::default();
        identity.current_map_index = Some(4);
        identity.reset_epoch = 2;
        let mut map = MapModel {
            mini_map_index: Some(8),
            map_width: Some(200),
            map_height: Some(200),
            ..default()
        };
        assert!(view.matches_map(&map, &identity));
        map.center_x = 105;
        assert!(view.matches_map(&map, &identity));
        assert_eq!(view.tile_at(Vec2::new(961.0, 76.0)), Some((100, 100)));
        identity.reset_epoch += 1;
        assert!(!view.matches_map(&map, &identity));
        identity.reset_epoch -= 1;
        identity.current_map_index = Some(5);
        assert!(!view.matches_map(&map, &identity));
        identity.current_map_index = Some(4);
        map.mini_map_index = Some(9);
        assert!(!view.matches_map(&map, &identity));
        map.mini_map_index = Some(8);
        map.map_width = Some(201);
        assert!(!view.matches_map(&map, &identity));
    }

    #[test]
    fn minimap_content_stays_above_the_hud_skin_during_night_lighting() {
        use crate::crystal_ui::overlays::{OVERLAY_CHAT_Z, OVERLAY_HUD_Z};

        assert_eq!(OVERLAY_MINIMAP_Z, OVERLAY_HUD_Z + 1);
        assert!(OVERLAY_MINIMAP_Z < OVERLAY_CHAT_Z);
    }
}
