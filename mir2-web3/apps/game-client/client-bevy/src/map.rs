//! Shared renderer-neutral map model and Bevy terrain rendering.
//!
//! The Web runtime and every native host consume the same map read model so
//! terrain tiles render identically everywhere. Real Crystal map atlases are a
//! later asset-pipeline slice; this module renders deterministic terrain-color
//! tiles (matching the runtime's Crystal-stepped palette) as the shared
//! fallback, ready to be swapped for atlas textures.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy::sprite::Sprite;
use serde::{Deserialize, Serialize};

/// Tile size in world units (matches the web client's 32px grid).
pub const TILE_SIZE: f32 = 32.0;

/// Terrain kinds mirroring the runtime `TerrainKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TerrainKind {
    Grass,
    Dirt,
    Road,
    Water,
    Stone,
}

impl TerrainKind {
    /// Crystal-stepped base tile color with per-cell variation.
    pub fn base_color(self, variation: u32) -> Color {
        let palette = match self {
            Self::Grass => [
                [0.16, 0.24, 0.12, 0.28],
                [0.18, 0.27, 0.14, 0.28],
                [0.14, 0.22, 0.11, 0.28],
            ],
            Self::Dirt => [
                [0.34, 0.22, 0.13, 0.30],
                [0.30, 0.20, 0.11, 0.30],
                [0.36, 0.24, 0.15, 0.30],
            ],
            Self::Road => [
                [0.45, 0.35, 0.23, 0.30],
                [0.40, 0.31, 0.20, 0.30],
                [0.48, 0.38, 0.26, 0.30],
            ],
            Self::Water => [
                [0.09, 0.25, 0.31, 0.26],
                [0.11, 0.30, 0.37, 0.26],
                [0.08, 0.22, 0.28, 0.26],
            ],
            Self::Stone => [
                [0.42, 0.39, 0.34, 0.28],
                [0.38, 0.35, 0.30, 0.28],
                [0.46, 0.43, 0.37, 0.28],
            ],
        };
        let [r, g, b, a] = palette[variation as usize % palette.len()];
        Color::srgba(r, g, b, a)
    }

    /// Lighter accent color drawn as a thin inner tile for visual depth.
    pub fn accent_color(self, variation: u32) -> Color {
        let palette = match self {
            Self::Grass => [
                [0.26, 0.40, 0.20, 0.18],
                [0.22, 0.34, 0.18, 0.18],
                [0.29, 0.43, 0.24, 0.18],
            ],
            Self::Dirt => [
                [0.46, 0.30, 0.18, 0.20],
                [0.40, 0.27, 0.15, 0.20],
                [0.52, 0.34, 0.20, 0.20],
            ],
            Self::Road => [
                [0.56, 0.46, 0.31, 0.22],
                [0.50, 0.40, 0.28, 0.22],
                [0.60, 0.50, 0.34, 0.22],
            ],
            Self::Water => [
                [0.13, 0.32, 0.40, 0.20],
                [0.15, 0.36, 0.44, 0.20],
                [0.11, 0.29, 0.36, 0.20],
            ],
            Self::Stone => [
                [0.50, 0.47, 0.41, 0.22],
                [0.46, 0.43, 0.37, 0.22],
                [0.54, 0.51, 0.44, 0.22],
            ],
        };
        let [r, g, b, a] = palette[variation as usize % palette.len()];
        Color::srgba(r, g, b, a)
    }
}

/// A single terrain patch in renderer-neutral grid coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerrainPatch {
    pub x: i32,
    pub y: i32,
    pub width: u16,
    pub height: u16,
    pub kind: TerrainKind,
}

/// The renderer-neutral map read model consumed by every host.
#[derive(Debug, Clone, Default, Resource, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapModel {
    pub patches: Vec<TerrainPatch>,
    pub center_x: i32,
    pub center_y: i32,
    /// Crystal's global `TimeOfDay` light setting (0..=4). The minimap uses
    /// this value to select its day/dawn/evening/night indicator.
    #[serde(default)]
    pub time_of_day_light_setting: Option<u8>,
}

impl MapModel {
    /// Terrain kind at a grid cell, or `None` outside every patch.
    pub fn terrain_at(&self, x: i32, y: i32) -> Option<TerrainKind> {
        self.patches
            .iter()
            .rev()
            .find(|patch| {
                x >= patch.x
                    && x < patch.x + i32::from(patch.width)
                    && y >= patch.y
                    && y < patch.y + i32::from(patch.height)
            })
            .map(|patch| patch.kind)
    }

    /// Deterministic per-cell variation so tiles look textured without assets.
    pub fn variation(x: i32, y: i32) -> u32 {
        let ux = u32::from_ne_bytes(x.to_ne_bytes());
        let uy = u32::from_ne_bytes(y.to_ne_bytes());
        (ux.wrapping_mul(31)).wrapping_add(uy.wrapping_mul(17))
    }
}

/// Marker on the shared map root node.
#[derive(Component)]
pub struct MirMapRoot;

/// Build the shared Mir2 map terrain renderer.
pub struct Mir2MapPlugin;

impl Plugin for Mir2MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapModel>().add_systems(
            Update,
            sync_map_terrain.run_if(resource_changed::<MapModel>),
        );
    }
}

/// Spawn/despawn terrain tiles to match the current [`MapModel`].
fn sync_map_terrain(
    mut commands: Commands,
    model: Res<MapModel>,
    mut existing: Query<(Entity, &MapTileKey, &mut Sprite), With<MirMapRoot>>,
) {
    // Later patches are authoritative when patches overlap, matching
    // `terrain_at`. Build one desired entry per cell before diffing ECS state.
    let mut desired = HashMap::new();
    for patch in &model.patches {
        for dx in 0..patch.width {
            for dy in 0..patch.height {
                desired.insert(
                    MapTileKey {
                        x: patch.x + i32::from(dx),
                        y: patch.y + i32::from(dy),
                    },
                    patch.kind,
                );
            }
        }
    }

    let mut live = HashSet::new();
    for (entity, key, mut sprite) in &mut existing {
        let Some(kind) = desired.get(key) else {
            commands.entity(entity).despawn();
            continue;
        };
        if !live.insert(key.clone()) {
            // Heal duplicate entities produced by an earlier overlapping model.
            commands.entity(entity).despawn();
            continue;
        }
        sprite.color = kind.base_color(MapModel::variation(key.x, key.y));
    }

    for (key, kind) in desired {
        if live.contains(&key) {
            continue;
        }
        commands.spawn((
            MirMapRoot,
            key.clone(),
            Sprite {
                color: kind.base_color(MapModel::variation(key.x, key.y)),
                custom_size: Some(Vec2::splat(TILE_SIZE)),
                ..default()
            },
            Transform::from_translation(tile_to_world(key.x, key.y)),
        ));
    }
}

/// Grid-key component for diffing terrain tiles.
#[derive(Component, Debug, Clone, PartialEq, Eq, Hash)]
pub struct MapTileKey {
    pub x: i32,
    pub y: i32,
}

/// Convert grid coordinates to world translation (y-flipped like Mir maps).
pub fn tile_to_world(x: i32, y: i32) -> Vec3 {
    Vec3::new(x as f32 * TILE_SIZE, -(y as f32 * TILE_SIZE), 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_lookup_returns_kind_inside_patch() {
        let model = MapModel {
            patches: vec![TerrainPatch {
                x: 0,
                y: 0,
                width: 5,
                height: 5,
                kind: TerrainKind::Grass,
            }],
            ..default()
        };
        assert_eq!(model.terrain_at(2, 3), Some(TerrainKind::Grass));
        assert_eq!(model.terrain_at(4, 4), Some(TerrainKind::Grass));
        assert_eq!(model.terrain_at(5, 0), None);
        assert_eq!(model.terrain_at(0, 5), None);
    }

    #[test]
    fn later_patch_wins_over_later_when_overlapping() {
        let model = MapModel {
            patches: vec![
                TerrainPatch {
                    x: 0,
                    y: 0,
                    width: 10,
                    height: 10,
                    kind: TerrainKind::Grass,
                },
                TerrainPatch {
                    x: 4,
                    y: 4,
                    width: 4,
                    height: 4,
                    kind: TerrainKind::Water,
                },
            ],
            ..default()
        };
        assert_eq!(model.terrain_at(2, 2), Some(TerrainKind::Grass));
        assert_eq!(model.terrain_at(5, 5), Some(TerrainKind::Water));
    }

    #[test]
    fn colors_are_deterministic_and_bounded() {
        for kind in [
            TerrainKind::Grass,
            TerrainKind::Dirt,
            TerrainKind::Road,
            TerrainKind::Water,
            TerrainKind::Stone,
        ] {
            for variation in 0..8 {
                let base = kind.base_color(variation).to_srgba();
                let accent = kind.accent_color(variation).to_srgba();
                assert!(base.red >= 0.0 && base.red <= 1.0);
                assert!(accent.red >= 0.0 && accent.red <= 1.0);
            }
        }
    }

    #[test]
    fn tile_world_conversion_flips_y() {
        let t = tile_to_world(3, 4);
        assert!((t.x - 96.0).abs() < 1e-5);
        assert!((t.y + 128.0).abs() < 1e-5);
    }

    #[test]
    fn overlapping_patches_render_one_tile_and_later_patch_wins() {
        let mut app = App::new();
        app.add_plugins(Mir2MapPlugin);
        *app.world_mut().resource_mut::<MapModel>() = MapModel {
            patches: vec![
                TerrainPatch {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                    kind: TerrainKind::Grass,
                },
                TerrainPatch {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                    kind: TerrainKind::Water,
                },
            ],
            ..default()
        };
        app.update();

        let world = app.world_mut();
        let mut query = world.query_filtered::<(&MapTileKey, &Sprite), With<MirMapRoot>>();
        let tiles = query.iter(world).collect::<Vec<_>>();
        assert_eq!(tiles.len(), 1);
        assert_eq!(
            tiles[0].1.color,
            TerrainKind::Water.base_color(MapModel::variation(0, 0))
        );
    }
}
