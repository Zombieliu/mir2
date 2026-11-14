use super::*;
use crate::runtime::drops::{CRYSTAL_DROP_RANGE, CRYSTAL_DROP_STACK_SIZE};

impl ZoneRuntime {
    /// Crystal ItemObject.Drop: prefer an empty cell in expanding square rings,
    /// then the least populated valid cell, with at most five items per cell.
    pub(super) fn native_ground_drop_position(&self, origin: &Point) -> Option<Point> {
        let mut best: Option<(Point, usize)> = None;
        for radius in 0..=CRYSTAL_DROP_RANGE {
            for y in (origin.y - radius)..=(origin.y + radius) {
                let mut x = origin.x - radius;
                let step = if (y - origin.y).abs() == radius {
                    1
                } else {
                    (radius * 2).max(1)
                };
                while x <= origin.x + radius {
                    let point = Point { x, y };
                    x += step;
                    if self.collision.is_ground_drop_blocked(&point)
                        || !self.can_occupy(&point, None)
                    {
                        continue;
                    }
                    let count = self
                        .ground_drops
                        .values()
                        .filter(|stored| stored.drop.x == point.x && stored.drop.y == point.y)
                        .count();
                    let count = count + self.player_drop_reserved_cell_count(&point);
                    if count >= CRYSTAL_DROP_STACK_SIZE {
                        continue;
                    }
                    if count == 0 {
                        return Some(point);
                    }
                    if best
                        .as_ref()
                        .is_none_or(|(_, best_count)| count < *best_count)
                    {
                        best = Some((point, count));
                    }
                }
            }
        }
        best.map(|(point, _)| point)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::zone::collision::ZoneBounds;

    fn drop(object_id: u32) -> GroundDropSnapshot {
        GroundDropSnapshot {
            object_id,
            name: "Gold".to_string(),
            name_colour_argb: -1,
            icon: 0,
            x: 0,
            y: 0,
            quantity: 1,
            source_monster: String::new(),
            owner_object_id: None,
            ownership_remaining_ticks: None,
            loot: GroundDropLootSnapshot::Gold { amount: 25 },
        }
    }

    fn spawn(zone: &mut ZoneRuntime, count: u32) -> Vec<GroundDropSnapshot> {
        zone.spawn_native_monster_drops(
            "Field Wasp",
            &Point { x: 10, y: 10 },
            101,
            (0..count).map(|id| drop(9200 + id)).collect(),
            1000,
        )
    }

    #[test]
    fn native_ground_drops_spread_before_stacking_and_keep_ownership_payload() {
        let mut zone = ZoneRuntime::new_with_collision(
            ZoneKey::for_map("drop-test"),
            ZoneCollision::unbounded(),
        );
        let drops = spawn(&mut zone, 9);
        assert_eq!(drops.len(), 9);
        let cells: BTreeSet<_> = drops.iter().map(|drop| (drop.x, drop.y)).collect();
        assert_eq!(cells.len(), 9);
        assert_eq!((drops[0].x, drops[0].y), (10, 10));
        assert!(drops.iter().all(|drop| {
            drop.owner_object_id == Some(101)
                && drop.ownership_remaining_ticks == Some(200)
                && drop.source_monster == "Field Wasp"
                && matches!(drop.loot, GroundDropLootSnapshot::Gold { amount: 25 })
                && (drop.x - 10).abs() <= 1
                && (drop.y - 10).abs() <= 1
        }));
        let more = spawn(&mut zone, 2);
        assert!(more.iter().all(|drop| !cells.contains(&(drop.x, drop.y))));
        assert_ne!(more[0].object_id, drops[0].object_id);
    }

    #[test]
    fn native_ground_drops_skip_static_collision_and_blocking_objects() {
        let mut zone = ZoneRuntime::new_with_collision(
            ZoneKey::for_map("drop-test"),
            ZoneCollision::unbounded()
                .with_blocked_cells([Point { x: 10, y: 10 }, Point { x: 9, y: 9 }]),
        );
        let npc = mir2_protocol::NpcInfo {
            object_id: 100,
            name: "Blocker".to_string(),
            name_colour_argb: -1,
            image: 0,
            colour_argb: -1,
            location: Point { x: 10, y: 9 },
            direction: MirDirection::Down,
            quest_ids: Vec::new(),
        };
        zone.apply_zone_object_packets(&[ServerPacket::ObjectNpc { info: npc }], 0);
        let drops = spawn(&mut zone, 3);
        assert_eq!(drops.len(), 3);
        assert!(drops
            .iter()
            .all(|drop| ![(10, 10), (9, 9), (10, 9)].contains(&(drop.x, drop.y))));
    }

    #[test]
    fn native_ground_drops_use_least_full_cells_and_enforce_stack_limit() {
        let mut zone = ZoneRuntime::new_with_collision(
            ZoneKey::for_map("drop-test"),
            ZoneCollision::unbounded().with_bounds(ZoneBounds::new(10, 11, 10, 10)),
        );
        let drops = spawn(&mut zone, 12);
        assert_eq!(drops.len(), 10);
        for x in 10..=11 {
            assert_eq!(
                drops
                    .iter()
                    .filter(|drop| drop.x == x && drop.y == 10)
                    .count(),
                5
            );
        }
        assert_eq!(zone.ground_drop_count(), 10);
    }

    #[test]
    fn native_ground_drops_do_not_spawn_when_every_cell_is_blocked() {
        let mut zone = ZoneRuntime::new_with_collision(
            ZoneKey::for_map("drop-test"),
            ZoneCollision::unbounded()
                .with_bounds(ZoneBounds::new(10, 10, 10, 10))
                .with_blocked_cells([Point { x: 10, y: 10 }]),
        );
        assert!(spawn(&mut zone, 2).is_empty());
        assert_eq!(zone.ground_drop_count(), 0);
    }
}
