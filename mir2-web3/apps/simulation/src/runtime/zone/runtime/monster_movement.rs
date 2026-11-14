use super::*;
use std::collections::VecDeque;

const DETOUR_RADIUS: i32 = 8;
const DETOUR_DIAMETER: usize = DETOUR_RADIUS as usize * 2 + 1;
const DETOUR_MAX_VISITED: usize = 256;
const DIRECT_LOOKAHEAD_STEPS: usize = 8;

impl ZoneRuntime {
    /// Keep an unobstructed Crystal MoveTo route cheap. On obstruction, search a
    /// bounded neighborhood before its seven-direction fallback: rotation alone
    /// can alternate forever between two cells at concave walls. Direct-route
    /// lookahead also prevents undoing a necessary retreat on the following tick.
    /// The search has no retained state and returns only one legal shared step.
    pub(super) fn native_monster_chase_step(
        &self,
        object_id: u32,
        position: &Point,
        target: &Point,
        preferred_direction: MirDirection,
    ) -> Option<(MirDirection, Point)> {
        let direct = offset_point(position, preferred_direction, 1);
        // DragonStatue retains its fixed direction; do not add turning/movement
        // alternatives to this special species through the generic chase path.
        if self
            .native_monsters
            .get(&object_id)
            .is_some_and(|monster| monster.ai == 54)
        {
            return self
                .can_native_monster_occupy(object_id, &direct)
                .then_some((preferred_direction, direct));
        }
        if self.native_monster_direct_chase_path_clear(object_id, &direct, target) {
            return Some((preferred_direction, direct));
        }
        let directions = [
            MirDirection::Up,
            MirDirection::UpRight,
            MirDirection::Right,
            MirDirection::DownRight,
            MirDirection::Down,
            MirDirection::DownLeft,
            MirDirection::Left,
            MirDirection::UpLeft,
        ];
        let first = directions
            .iter()
            .position(|direction| *direction == preferred_direction)?;
        let clockwise = object_id & 1 == 0;
        let ordered: [MirDirection; 8] = std::array::from_fn(|offset| {
            let index = if clockwise {
                (first + offset) % directions.len()
            } else {
                (first + directions.len() - offset) % directions.len()
            };
            directions[index]
        });

        // A full attack ring (for example eight monsters already surrounding a
        // player) cannot accept another pursuer. Avoid scanning the neighborhood
        // each tick when no attack tile is currently available.
        let attack_tile_available = ordered.iter().any(|direction| {
            self.can_native_monster_occupy(object_id, &offset_point(target, *direction, 1))
        });
        if attack_tile_available {
            if let Some(step) =
                self.native_monster_detour_step(object_id, position, target, &ordered)
            {
                return Some(step);
            }
        }
        // Preserve Crystal's deterministic side choice when a path cannot be
        // established within this local budget, rather than freezing the actor.
        for direction in ordered.into_iter().skip(1) {
            let destination = offset_point(position, direction, 1);
            if self.can_native_monster_occupy(object_id, &destination) {
                return Some((direction, destination));
            }
        }
        None
    }

    fn native_monster_direct_chase_path_clear(
        &self,
        object_id: u32,
        first_step: &Point,
        target: &Point,
    ) -> bool {
        let mut current = first_step.clone();
        for _ in 0..DIRECT_LOOKAHEAD_STEPS {
            if !self.can_native_monster_occupy(object_id, &current) {
                return false;
            }
            if current == *target || native_monster_adjacent_to(&current, target) {
                return true;
            }
            let Some(direction) = zone_direction_toward(&current, target) else {
                return false;
            };
            current = offset_point(&current, direction, 1);
        }
        true
    }

    fn native_monster_detour_step(
        &self,
        object_id: u32,
        position: &Point,
        target: &Point,
        directions: &[MirDirection; 8],
    ) -> Option<(MirDirection, Point)> {
        let mut visited = [false; DETOUR_DIAMETER * DETOUR_DIAMETER];
        visited[DETOUR_DIAMETER * DETOUR_DIAMETER / 2] = true;
        let mut visited_count = 1;
        let mut queue = VecDeque::from([(position.clone(), None::<MirDirection>)]);
        let mut best_distance = zone_tile_distance(position, target);
        let mut progress_direction = None;

        'search: while let Some((current, first_direction)) = queue.pop_front() {
            for direction in directions {
                let next = offset_point(&current, *direction, 1);
                let dx = i64::from(next.x) - i64::from(position.x);
                let dy = i64::from(next.y) - i64::from(position.y);
                if dx.abs() > i64::from(DETOUR_RADIUS) || dy.abs() > i64::from(DETOUR_RADIUS) {
                    continue;
                }
                let index = (dy + i64::from(DETOUR_RADIUS)) as usize * DETOUR_DIAMETER
                    + (dx + i64::from(DETOUR_RADIUS)) as usize;
                if visited[index] {
                    continue;
                }
                if visited_count >= DETOUR_MAX_VISITED {
                    break 'search;
                }
                visited[index] = true;
                visited_count += 1;
                if !self.can_native_monster_occupy(object_id, &next) {
                    continue;
                }
                let first_direction = first_direction.unwrap_or(*direction);
                let distance = zone_tile_distance(&next, target);
                if distance == 1 {
                    return Some((first_direction, offset_point(position, first_direction, 1)));
                }
                if distance < best_distance {
                    best_distance = distance;
                    progress_direction = Some(first_direction);
                }
                queue.push_back((next, Some(first_direction)));
            }
        }
        progress_direction.map(|direction| (direction, offset_point(position, direction, 1)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chase_can_enter_an_empty_follow_cell_for_monster_master_pets() {
        let zone = ZoneRuntime::new_with_collision(
            ZoneKey::for_map("empty-pet-follow"),
            ZoneCollision::unbounded(),
        );
        let source = Point { x: 10, y: 10 };
        let target = Point { x: 11, y: 10 };
        assert_eq!(
            zone.native_monster_chase_step(101, &source, &target, MirDirection::Right),
            Some((MirDirection::Right, target))
        );
    }
}
