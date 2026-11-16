//! Process-local trusted-host authorization. Never encoded in a command or
//! checkpoint; cold hosts must rebind their own policy before admitting users.
use mir2_protocol::Point;
use std::fmt::Debug;
use std::sync::Arc;

/// Read-only access to the very same retained occupancy predicate used by the
/// final movement validator. No Zone/ledger lock or separately loaded map.
#[derive(Clone, Copy)]
pub struct ZoneMovementOccupancy<'a> {
    source: MovementOccupancySource<'a>,
}
#[derive(Clone, Copy)]
enum MovementOccupancySource<'a> {
    Zone { zone: &'a super::ZoneRuntime, ignore_session: &'a super::SessionId },
    #[cfg(any(test, feature = "test-support"))]
    Fixture(&'a [Point]),
}
impl Debug for ZoneMovementOccupancy<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ZoneMovementOccupancy").finish_non_exhaustive()
    }
}
impl<'a> ZoneMovementOccupancy<'a> {
    fn for_zone(zone: &'a super::ZoneRuntime, ignore_session: &'a super::SessionId) -> Self {
        Self { source: MovementOccupancySource::Zone { zone, ignore_session } }
    }
    pub fn can_occupy(&self, point: &Point) -> bool {
        match self.source {
            MovementOccupancySource::Zone { zone, ignore_session } => zone.can_player_movement_occupy(point, Some(ignore_session)),
            #[cfg(any(test, feature = "test-support"))]
            MovementOccupancySource::Fixture(blocked) => !blocked.contains(point),
        }
    }

    /// Candidate coordinates are gathered once, then checked by the exact
    /// movement predicate. Static terrain is supplied separately by collision.
    /// This avoids scanning every live monster once per grid cell.
    pub fn dynamic_blocking_cells(&self, bounds: crate::runtime::zone::ZoneBounds) -> Vec<Point> {
        let inside = |point: &Point| point.x >= bounds.min_x && point.x <= bounds.max_x
            && point.y >= bounds.min_y && point.y <= bounds.max_y;
        let mut candidates = std::collections::BTreeSet::new();
        match self.source {
            MovementOccupancySource::Zone { zone, .. } => {
                for (x, y) in zone.occupancy.keys() {
                    let point = Point { x: *x, y: *y };
                    if inside(&point) { candidates.insert((point.x, point.y)); }
                }
                for object in zone.objects.values() {
                    if inside(&object.position) { candidates.insert((object.position.x, object.position.y)); }
                }
                for monster in zone.native_monsters.values() {
                    if inside(&monster.position) { candidates.insert((monster.position.x, monster.position.y)); }
                    if monster.ai == 81 {
                        let effect = mir2_game_data::crystal_monster_by_name(&monster.name).map_or(0, |template| template.effect);
                        for (dx, dy) in super::gate_offsets(effect) {
                            let point = Point { x: monster.position.x.saturating_add(*dx), y: monster.position.y.saturating_add(*dy) };
                            if inside(&point) { candidates.insert((point.x, point.y)); }
                        }
                    }
                }
            }
            #[cfg(any(test, feature = "test-support"))]
            MovementOccupancySource::Fixture(blocked) => {
                candidates.extend(blocked.iter().filter(|point| inside(point)).map(|point| (point.x, point.y)));
            }
        }
        candidates.into_iter().map(|(x, y)| Point { x, y }).filter(|point| !self.can_occupy(point)).collect()
    }
    /// Isolated host fixture only; never available to normal clients or a bin
    /// built without test-support. Production always borrows the retained Zone.
    #[cfg(any(test, feature = "test-support"))]
    pub fn for_test(blocked: &'a [Point]) -> Self {
        Self { source: MovementOccupancySource::Fixture(blocked) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoneActionKind {
    Move,
    Attack,
    Pickup,
    MapTransfer,
}

#[derive(Debug)]
pub struct ZoneActionBoundaryRequest<'a> {
    pub account_id: &'a str,
    pub character_index: i32,
    pub map_file_name: &'a str,
    pub origin: &'a Point,
    /// Actual authoritative tiles, including every intermediate run tile.
    pub path: &'a [Point],
    pub now_ms: u64,
    pub kind: ZoneActionKind,
    pub crosses_map_exit: bool,
    /// The exact retained Zone terrain. Host callbacks must not re-enter Zone
    /// or consult a separately loaded/client-supplied navigation map.
    pub collision: &'a super::ZoneCollision,
    pub occupancy: ZoneMovementOccupancy<'a>,
}
pub trait ZoneActionBoundary: Send + Sync + Debug {
    fn allows(&self, request: &ZoneActionBoundaryRequest<'_>) -> bool;
}

impl super::ZoneRuntime {
    pub fn set_action_boundary(&mut self, boundary: Arc<dyn ZoneActionBoundary>) {
        self.action_boundary = Some(boundary);
    }
    pub(super) fn boundary_allows(
        &self,
        session: &super::SessionId,
        kind: ZoneActionKind,
        path: &[Point],
        now: u64,
        crosses_map_exit: bool,
    ) -> bool {
        let Some(boundary) = &self.action_boundary else {
            return true;
        };
        let Some(player) = self.players.get(session) else {
            return false;
        };
        boundary.allows(&ZoneActionBoundaryRequest {
            account_id: &player.account_id,
            character_index: player.character_index,
            map_file_name: &self.key.map_file_name,
            origin: &player.position,
            path,
            now_ms: now,
            kind,
            crosses_map_exit,
            collision: &self.collision,
            occupancy: ZoneMovementOccupancy::for_zone(self, session),
        })
    }
    pub(super) fn boundary_move_allows(
        &self,
        session: &super::SessionId,
        path: &[Point],
        direction: mir2_protocol::MirDirection,
        now: u64,
    ) -> bool {
        if self.action_boundary.is_none() {
            return true;
        }
        let Some(player) = self.players.get(session) else {
            return false;
        };
        let exit = path.iter().any(|point| {
            // Direct Crystal movement links are steppable transfer exceptions
            // even when the ordinary terrain marks them outside/blocked.
            (self.collision.is_ground_drop_blocked(point)
                && !self.collision.is_player_movement_blocked(point))
                || super::crystal_map_coordinate_decision(
                    &self.key.map_file_name,
                    point,
                    player.level,
                    player.chat_profile.pk_points,
                    direction,
                )
                .is_some()
        });
        self.boundary_allows(session, ZoneActionKind::Move, path, now, exit)
    }
    pub(super) fn boundary_attack_allows(
        &self,
        session: &super::SessionId,
        object_id: u32,
        extra: Option<&Point>,
        now: u64,
    ) -> bool {
        if self.action_boundary.is_none() {
            return true;
        }
        let target = self
            .players
            .values()
            .find(|p| p.object_id == object_id)
            .map(|p| p.position.clone())
            .or_else(|| {
                self.native_monsters
                    .get(&object_id)
                    .map(|p| p.position.clone())
            })
            .or_else(|| self.objects.get(&object_id).map(|p| p.position.clone()));
        let mut path = target.into_iter().collect::<Vec<_>>();
        if let Some(extra) = extra {
            path.push(extra.clone());
        }
        self.boundary_allows(session, ZoneActionKind::Attack, &path, now, false)
    }
}

#[cfg(test)]
#[path = "action_boundary_tests.rs"]
mod tests;
