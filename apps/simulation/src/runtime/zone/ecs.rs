//! ECS scaffolding for the shared zone (L2 step 1).
//!
//! This is the *additive* first slice of the L2 migration described in
//! `docs/L2-ECS-ZONE-DESIGN.md`: it stands up a `bevy_ecs::World` inside the
//! zone and mirrors player entities into it at the same join/leave/move sites
//! the `players` map and the AOI grid already use. The `players` `BTreeMap`
//! remains the source of truth; this mirror is read-only bookkeeping that
//! proves the entity/index plumbing under the existing 141-test oracle before
//! any system is ported to read from it.
//!
//! Net behavior change: none. Nothing in the tick reads from this World yet.

use std::collections::BTreeMap;

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mir2_protocol::Point;

use super::types::SessionId;

/// Marker + identity for a player entity mirrored into the zone ECS world.
///
/// `session_id`/`object_id` are read by tests today and will be read by
/// gameplay in L2 step 2 (when visibility/combat systems query this World
/// instead of the `players` map). Held intentionally; see
/// docs/L2-ECS-ZONE-DESIGN.md.
#[derive(Component, Debug, Clone)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct ZonePlayerEntity {
    pub session_id: SessionId,
    pub object_id: u32,
}

/// Tile position mirrored from the authoritative `ZonePlayer`.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ZonePosition {
    pub x: i32,
    pub y: i32,
}

impl From<&Point> for ZonePosition {
    fn from(p: &Point) -> Self {
        Self { x: p.x, y: p.y }
    }
}

/// The zone's ECS world plus the id→entity indexes the current map-based code
/// relies on. Holds the `World` that future L2 slices will make authoritative.
#[derive(Debug)]
pub(super) struct ZoneEcs {
    world: World,
    player_entities: BTreeMap<SessionId, Entity>,
}

impl Default for ZoneEcs {
    fn default() -> Self {
        Self {
            world: World::new(),
            player_entities: BTreeMap::new(),
        }
    }
}

impl ZoneEcs {
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// Mirror a player on join. Idempotent: re-inserting an existing session
    /// updates its entity in place rather than leaking a second entity.
    pub(super) fn upsert_player(
        &mut self,
        session_id: &SessionId,
        object_id: u32,
        position: &Point,
    ) {
        let pos = ZonePosition::from(position);
        if let Some(entity) = self.player_entities.get(session_id).copied() {
            if let Ok(mut entity_mut) = self.world.get_entity_mut(entity) {
                entity_mut.insert((
                    ZonePlayerEntity {
                        session_id: session_id.clone(),
                        object_id,
                    },
                    pos,
                ));
                return;
            }
            // Stale index entry (entity despawned out from under us): fall through
            // and respawn a fresh one.
        }
        let entity = self
            .world
            .spawn((
                ZonePlayerEntity {
                    session_id: session_id.clone(),
                    object_id,
                },
                pos,
            ))
            .id();
        self.player_entities.insert(session_id.clone(), entity);
    }

    /// Update a mirrored player's position on move. No-op if not yet mirrored.
    pub(super) fn move_player(&mut self, session_id: &SessionId, position: &Point) {
        if let Some(entity) = self.player_entities.get(session_id).copied() {
            if let Ok(mut entity_mut) = self.world.get_entity_mut(entity) {
                entity_mut.insert(ZonePosition::from(position));
            }
        }
    }

    /// Remove a mirrored player on leave. No-op if absent.
    pub(super) fn remove_player(&mut self, session_id: &SessionId) {
        if let Some(entity) = self.player_entities.remove(session_id) {
            let _ = self.world.despawn(entity);
        }
    }

    #[cfg(test)]
    pub(super) fn player_count(&self) -> usize {
        self.player_entities.len()
    }

    /// Snapshot the mirror as `(session_id, object_id, x, y)` tuples, sorted by
    /// session id. Used to assert the ECS mirror matches the authoritative
    /// `players` map (the L2 step-1 invariant). Reads `ZonePlayerEntity` so the
    /// identity fields are genuinely exercised, not dormant.
    #[cfg(test)]
    pub(super) fn mirror_snapshot(&mut self) -> Vec<(SessionId, u32, i32, i32)> {
        let mut out: Vec<(SessionId, u32, i32, i32)> = self
            .world
            .query::<(&ZonePlayerEntity, &ZonePosition)>()
            .iter(&self.world)
            .map(|(player, pos)| (player.session_id.clone(), player.object_id, pos.x, pos.y))
            .collect();
        out.sort();
        out
    }

    /// Read back a mirrored player's position (test/diagnostic equivalence check).
    #[cfg(test)]
    pub(super) fn player_position(&self, session_id: &SessionId) -> Option<ZonePosition> {
        let entity = *self.player_entities.get(session_id)?;
        self.world.get::<ZonePosition>(entity).copied()
    }

    /// Number of live `ZonePlayerEntity` entities actually in the world — used
    /// by tests to assert the index and the world agree (no leaks/orphans).
    #[cfg(test)]
    pub(super) fn world_player_entity_count(&mut self) -> usize {
        self.world
            .query::<&ZonePlayerEntity>()
            .iter(&self.world)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sid(s: &str) -> SessionId {
        SessionId::new(s)
    }
    fn p(x: i32, y: i32) -> Point {
        Point { x, y }
    }

    #[test]
    fn upsert_move_remove_keeps_index_and_world_in_sync() {
        let mut ecs = ZoneEcs::new();
        ecs.upsert_player(&sid("a"), 101, &p(10, 10));
        ecs.upsert_player(&sid("b"), 102, &p(20, 20));
        assert_eq!(ecs.player_count(), 2);
        assert_eq!(ecs.world_player_entity_count(), 2);
        assert_eq!(
            ecs.player_position(&sid("a")),
            Some(ZonePosition { x: 10, y: 10 })
        );

        ecs.move_player(&sid("a"), &p(11, 12));
        assert_eq!(
            ecs.player_position(&sid("a")),
            Some(ZonePosition { x: 11, y: 12 })
        );
        // Index and world still agree after a move.
        assert_eq!(ecs.player_count(), 2);
        assert_eq!(ecs.world_player_entity_count(), 2);

        ecs.remove_player(&sid("a"));
        assert_eq!(ecs.player_count(), 1);
        assert_eq!(ecs.world_player_entity_count(), 1);
        assert_eq!(ecs.player_position(&sid("a")), None);
    }

    #[test]
    fn upsert_existing_session_does_not_leak_entities() {
        let mut ecs = ZoneEcs::new();
        ecs.upsert_player(&sid("a"), 101, &p(1, 1));
        ecs.upsert_player(&sid("a"), 101, &p(2, 2)); // same session re-join
        assert_eq!(ecs.player_count(), 1);
        assert_eq!(ecs.world_player_entity_count(), 1);
        assert_eq!(
            ecs.player_position(&sid("a")),
            Some(ZonePosition { x: 2, y: 2 })
        );
    }

    #[test]
    fn move_or_remove_unknown_session_is_noop() {
        let mut ecs = ZoneEcs::new();
        ecs.move_player(&sid("ghost"), &p(5, 5)); // no panic
        ecs.remove_player(&sid("ghost")); // no panic
        assert_eq!(ecs.player_count(), 0);
        assert_eq!(ecs.world_player_entity_count(), 0);
    }
}
