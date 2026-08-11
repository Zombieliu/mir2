//! Shared renderer-neutral entity model and Bevy entity rendering.
//!
//! Mirrors the runtime's placeholder entity rendering (colored sprites keyed by
//! kind) in the shared crate so every host draws entities the same way. Real
//! Crystal sprite atlases are a later asset-pipeline slice; this is the shared
//! fallback surface, ready to swap color fills for atlas textures.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy::sprite::Sprite;
use serde::{Deserialize, Serialize};

use crate::map::TILE_SIZE;

/// Entity kind matching the web client / gateway `WorldEntityKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EntityKind {
    Player,
    SelfPlayer,
    Monster,
    Npc,
}

impl EntityKind {
    pub fn color(self) -> Color {
        match self {
            Self::SelfPlayer => Color::srgb(0.74, 0.58, 0.28),
            Self::Player => Color::srgb(0.60, 0.55, 0.42),
            Self::Monster => Color::srgb(0.55, 0.27, 0.18),
            Self::Npc => Color::srgb(0.48, 0.57, 0.33),
        }
    }

    pub fn size(self) -> Vec2 {
        match self {
            Self::SelfPlayer | Self::Player => Vec2::new(24.0, 32.0),
            Self::Monster => Vec2::new(28.0, 28.0),
            Self::Npc => Vec2::new(20.0, 30.0),
        }
    }

    pub fn facing_color(self) -> Color {
        match self {
            Self::SelfPlayer => Color::srgb(1.0, 0.92, 0.74),
            Self::Player => Color::srgb(0.84, 0.93, 1.0),
            Self::Monster => Color::srgb(1.0, 0.82, 0.74),
            Self::Npc => Color::srgb(0.89, 1.0, 0.87),
        }
    }
}

/// A single entity in renderer-neutral grid coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityModel {
    pub object_id: String,
    pub kind: EntityKind,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub level: Option<u32>,
    pub direction: Option<String>,
}

impl EntityModel {
    /// Body size including the level-based growth bonus.
    pub fn body_size(&self) -> Vec2 {
        let mut size = self.kind.size();
        let level_bonus = self.level.unwrap_or(1).min(30) as f32 * 0.12;
        size.x += level_bonus;
        size.y += level_bonus * 1.8;
        size
    }

    /// Deterministic accent color derived from the entity name.
    pub fn accent_color(&self) -> Color {
        let variant = name_seed(&self.name) % 3;
        match (self.kind, variant) {
            (EntityKind::SelfPlayer, 0) => Color::srgb(1.0, 0.92, 0.74),
            (EntityKind::SelfPlayer, _) => Color::srgb(0.95, 0.83, 0.58),
            (EntityKind::Player, 0) => Color::srgb(0.78, 0.92, 1.0),
            (EntityKind::Player, _) => Color::srgb(0.67, 0.84, 0.98),
            (EntityKind::Monster, 0) => Color::srgb(1.0, 0.70, 0.46),
            (EntityKind::Monster, _) => Color::srgb(0.94, 0.59, 0.34),
            (EntityKind::Npc, 0) => Color::srgb(0.87, 1.0, 0.76),
            (EntityKind::Npc, _) => Color::srgb(0.72, 0.95, 0.66),
        }
    }
}

fn name_seed(name: &str) -> u8 {
    name.bytes().fold(0u8, |acc, byte| acc.wrapping_add(byte))
}

/// The renderer-neutral entity read model consumed by every host.
#[derive(Debug, Clone, Default, Resource, Serialize, Deserialize)]
pub struct EntityModelSet {
    pub entities: Vec<EntityModel>,
}

impl EntityModelSet {
    pub fn index(&self) -> HashMap<String, EntityModel> {
        self.entities
            .iter()
            .cloned()
            .map(|entity| (entity.object_id.clone(), entity))
            .collect()
    }
}

/// Marker on shared entity sprites.
#[derive(Component)]
pub struct MirEntity;

/// Component carrying the entity key for diffing.
#[derive(Component, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityKey {
    pub object_id: String,
}

#[derive(Component, Debug, Clone, PartialEq)]
struct RenderedEntityModel(EntityModel);

/// Build the shared Mir2 entity renderer.
pub struct Mir2EntitiesPlugin;

impl Plugin for Mir2EntitiesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EntityModelSet>().add_systems(
            Update,
            sync_entities.run_if(resource_changed::<EntityModelSet>),
        );
    }
}

fn sync_entities(
    mut commands: Commands,
    model: Res<EntityModelSet>,
    mut existing: Query<
        (Entity, &EntityKey, &mut RenderedEntityModel, &mut Transform),
        With<MirEntity>,
    >,
) {
    let index = model.index();
    let mut live = HashSet::new();

    for (entity, key, mut rendered, mut transform) in &mut existing {
        let Some(next) = index.get(&key.object_id) else {
            commands.entity(entity).despawn();
            continue;
        };

        let visual_changed = rendered.0.kind != next.kind
            || rendered.0.name != next.name
            || rendered.0.level != next.level;
        if visual_changed {
            // Child sprites encode kind, name-derived accent and level-derived
            // size. Rebuild only when one of those visual inputs changes.
            commands.entity(entity).despawn();
            continue;
        }

        transform.translation = entity_position(next);
        rendered.0 = next.clone();
        live.insert(key.object_id.clone());
    }

    for entity in &model.entities {
        if live.contains(&entity.object_id) {
            continue;
        }
        let body_size = entity.body_size();
        let accent = entity.accent_color();

        commands
            .spawn((
                MirEntity,
                EntityKey {
                    object_id: entity.object_id.clone(),
                },
                RenderedEntityModel(entity.clone()),
                Transform::from_translation(entity_position(entity)),
            ))
            .with_children(|parent| {
                parent.spawn((
                    Sprite::from_color(Color::srgba(0.02, 0.02, 0.02, 0.22), Vec2::new(26.0, 10.0)),
                    Transform::from_xyz(0.0, -10.0, 0.5),
                ));
                parent.spawn((
                    Sprite::from_color(entity.kind.color(), body_size),
                    Transform::from_xyz(0.0, 6.0, 1.0),
                ));
                parent.spawn((
                    Sprite::from_color(accent, Vec2::new(body_size.x * 0.5, 6.0)),
                    Transform::from_xyz(0.0, 16.0, 2.0),
                ));
            });
    }
}

fn entity_position(entity: &EntityModel) -> Vec3 {
    Vec3::new(
        entity.x as f32 * TILE_SIZE,
        -(entity.y as f32 * TILE_SIZE) + 16.0,
        1.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(kind: EntityKind) -> EntityModel {
        EntityModel {
            object_id: "1001".to_owned(),
            kind,
            name: "Test".to_owned(),
            x: 9,
            y: 7,
            level: Some(3),
            direction: Some("up".to_owned()),
        }
    }

    #[test]
    fn entity_index_keys_by_object_id() {
        let model = EntityModelSet {
            entities: vec![
                sample(EntityKind::SelfPlayer),
                EntityModel {
                    object_id: "2001".to_owned(),
                    ..sample(EntityKind::Monster)
                },
            ],
        };
        let index = model.index();
        assert!(index.contains_key("1001"));
        assert!(index.contains_key("2001"));
        assert_eq!(index.len(), 2);
    }

    #[test]
    fn body_size_grows_with_level() {
        let low = EntityModel {
            level: Some(1),
            ..sample(EntityKind::Monster)
        };
        let high = EntityModel {
            level: Some(30),
            ..sample(EntityKind::Monster)
        };
        assert!(high.body_size().y > low.body_size().y);
    }

    #[test]
    fn accent_color_is_deterministic_per_name() {
        let a = sample(EntityKind::Player);
        let b = sample(EntityKind::Player);
        assert_eq!(a.accent_color(), b.accent_color());
    }

    #[test]
    fn kinds_have_distinct_palette() {
        assert_ne!(EntityKind::Monster.color(), EntityKind::Npc.color());
        assert_ne!(EntityKind::SelfPlayer.color(), EntityKind::Player.color());
    }

    #[test]
    fn existing_entity_transform_tracks_later_snapshots() {
        let mut app = App::new();
        app.add_plugins(Mir2EntitiesPlugin);
        app.world_mut().resource_mut::<EntityModelSet>().entities =
            vec![sample(EntityKind::SelfPlayer)];
        app.update();

        app.world_mut().resource_mut::<EntityModelSet>().entities[0].x = 12;
        app.world_mut().resource_mut::<EntityModelSet>().entities[0].y = 10;
        app.update();

        let world = app.world_mut();
        let mut query = world.query_filtered::<(&EntityKey, &Transform), With<MirEntity>>();
        let (_, transform) = query
            .iter(world)
            .find(|(key, _)| key.object_id == "1001")
            .expect("updated entity should remain rendered");
        assert_eq!(transform.translation.x, 12.0 * TILE_SIZE);
        assert_eq!(transform.translation.y, -(10.0 * TILE_SIZE) + 16.0);
    }
}
