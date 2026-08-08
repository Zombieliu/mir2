//! Server-side A* pathfinding over the Mir2 tile grid.
//!
//! Crystal lets the *client* compute click-to-move routes and stream per-step
//! Walk/Run intents that the server validates. The Rust simulation keeps that
//! per-step validation path intact (see [`super::movement::move_player_by_direction`]),
//! but the authoritative click-to-move helper ([`super::movement`]'s `move_to`)
//! historically walked a straight line toward the destination and simply stopped
//! when it hit an obstacle. That made `MoveTo` (used by QA/auto-walk and hero
//! following) get stuck on walls, trees and other blockers.
//!
//! This module adds a bounded 8-direction A* search so click-to-move and hero
//! following can route *around* static blockers and other occupants, matching the
//! routing behaviour players expect from the original client.
//!
//! Movement uses uniform step cost (a diagonal step costs the same as an
//! orthogonal one, exactly like Mir2), so the Chebyshev tile distance is both an
//! admissible and consistent heuristic and A* expands very few nodes in the
//! common case.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::World;
use mir2_protocol::{MirDirection, Point};

use super::movement::{can_occupy, offset_point, tile_distance};

/// Upper bound on nodes expanded before giving up. Keeps a pathological request
/// (e.g. a destination walled off entirely) from scanning the whole map.
const PATHFIND_MAX_EXPANSIONS: usize = 6_000;

/// Maximum Chebyshev distance we will even attempt to route. Click-to-move only
/// ever targets a tile inside the player's own screen, so anything beyond this is
/// almost certainly a stale/garbage request.
const PATHFIND_MAX_RANGE: i32 = 96;

const DIRECTIONS: [MirDirection; 8] = [
    MirDirection::Up,
    MirDirection::UpRight,
    MirDirection::Right,
    MirDirection::DownRight,
    MirDirection::Down,
    MirDirection::DownLeft,
    MirDirection::Left,
    MirDirection::UpLeft,
];

#[derive(Clone, Copy, Eq, PartialEq)]
struct Frontier {
    f: i32,
    g: i32,
    key: (i32, i32),
}

impl Ord for Frontier {
    fn cmp(&self, other: &Self) -> Ordering {
        // `BinaryHeap` is a max-heap, so invert to pop the lowest `f` first.
        // Break ties toward the larger `g` (closer to the goal) for straighter
        // looking paths.
        other
            .f
            .cmp(&self.f)
            .then_with(|| other.g.cmp(&self.g))
            .then_with(|| self.key.cmp(&other.key))
    }
}

impl PartialOrd for Frontier {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Find an 8-direction path from `start` to `goal` over the live ECS world.
///
/// Returns the ordered list of tiles to step onto, **excluding** `start` and
/// **including** `goal`. Returns `Some(vec![])` when already standing on `goal`,
/// and `None` when the goal is unreachable (blocked, out of range, or search
/// budget exhausted).
///
/// Walkability uses the same [`can_occupy`] check the step validator uses, so a
/// path produced here is steppable tile-by-tile (subject to dynamic occupancy
/// changing between ticks, which the caller re-validates anyway).
pub(super) fn find_path(
    world: &World,
    start: &Point,
    goal: &Point,
    ignore_entity: Option<Entity>,
) -> Option<Vec<Point>> {
    find_path_with(start, goal, |point| {
        can_occupy(world, point.clone(), ignore_entity)
    })
}

/// Like [`find_path`] but routes to any walkable tile **adjacent** to `goal`
/// rather than onto `goal` itself. This is what a monster/hero chasing a player
/// needs: the target tile is occupied by the player, so we path to a tile next
/// to them (from which they can attack). The returned path excludes `start` and
/// ends on the adjacent tile. Returns `Some(vec![])` when already adjacent.
pub(super) fn find_path_adjacent(
    world: &World,
    start: &Point,
    goal: &Point,
    ignore_entity: Option<Entity>,
) -> Option<Vec<Point>> {
    find_path_adjacent_with(start, goal, |point| {
        can_occupy(world, point.clone(), ignore_entity)
    })
}

/// Pure 8-direction A* core, parameterised over a walkability predicate so it
/// can be unit-tested without constructing a Bevy `World`. `is_walkable` is
/// called for the goal tile and every intermediate tile (never for `start`).
pub(super) fn find_path_with<F>(
    start: &Point,
    goal: &Point,
    mut is_walkable: F,
) -> Option<Vec<Point>>
where
    F: FnMut(&Point) -> bool,
{
    if start == goal {
        return Some(Vec::new());
    }
    if !is_walkable(goal) {
        return None;
    }
    astar(start, goal, |point| point == goal, &mut is_walkable)
}

/// Pure adjacency variant: succeeds on reaching any walkable tile within one
/// Chebyshev tile of `goal`. `goal` itself does not need to be walkable.
pub(super) fn find_path_adjacent_with<F>(
    start: &Point,
    goal: &Point,
    mut is_walkable: F,
) -> Option<Vec<Point>>
where
    F: FnMut(&Point) -> bool,
{
    if tile_distance(start, goal) <= 1 {
        return Some(Vec::new());
    }
    astar(
        start,
        goal,
        |point| tile_distance(point, goal) <= 1,
        &mut is_walkable,
    )
}

/// Shared bounded A* over the 8-connected tile grid. Expands from `start`
/// toward `goal` (used only by the heuristic) until `is_goal` accepts a tile.
/// `start` is never passed to `is_walkable`; every other expanded tile is.
fn astar<G, F>(
    start: &Point,
    goal: &Point,
    mut is_goal: G,
    is_walkable: &mut F,
) -> Option<Vec<Point>>
where
    G: FnMut(&Point) -> bool,
    F: FnMut(&Point) -> bool,
{
    if tile_distance(start, goal) > PATHFIND_MAX_RANGE {
        return None;
    }

    let start_key = (start.x, start.y);

    let mut open: BinaryHeap<Frontier> = BinaryHeap::new();
    let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();
    let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();

    g_score.insert(start_key, 0);
    open.push(Frontier {
        f: tile_distance(start, goal),
        g: 0,
        key: start_key,
    });

    let mut expansions = 0usize;
    while let Some(current) = open.pop() {
        let current_point = Point {
            x: current.key.0,
            y: current.key.1,
        };
        if current.key != start_key && is_goal(&current_point) {
            return Some(reconstruct_path(&came_from, start_key, current.key));
        }
        // Skip stale heap entries left over from a cheaper path being found.
        if current.g > *g_score.get(&current.key).unwrap_or(&i32::MAX) {
            continue;
        }

        expansions += 1;
        if expansions > PATHFIND_MAX_EXPANSIONS {
            return None;
        }

        for direction in DIRECTIONS {
            let next = offset_point(&current_point, direction, 1);
            let next_key = (next.x, next.y);
            if !is_walkable(&next) {
                continue;
            }
            let tentative_g = current.g + 1;
            if tentative_g < *g_score.get(&next_key).unwrap_or(&i32::MAX) {
                came_from.insert(next_key, current.key);
                g_score.insert(next_key, tentative_g);
                open.push(Frontier {
                    f: tentative_g + tile_distance(&next, goal),
                    g: tentative_g,
                    key: next_key,
                });
            }
        }
    }

    None
}

fn reconstruct_path(
    came_from: &HashMap<(i32, i32), (i32, i32)>,
    start_key: (i32, i32),
    goal_key: (i32, i32),
) -> Vec<Point> {
    let mut path = Vec::new();
    let mut key = goal_key;
    while key != start_key {
        path.push(Point { x: key.0, y: key.1 });
        match came_from.get(&key) {
            Some(prev) => key = *prev,
            None => break,
        }
    }
    path.reverse();
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn point(x: i32, y: i32) -> Point {
        Point { x, y }
    }

    /// Every returned step is exactly one Chebyshev tile from the previous one,
    /// the path ends on the goal and never crosses a blocked tile.
    fn assert_valid_path(
        start: &Point,
        path: &[Point],
        goal: &Point,
        blocked: &HashSet<(i32, i32)>,
    ) {
        assert_eq!(path.last(), Some(goal), "path must end on the goal");
        let mut prev = start.clone();
        for step in path {
            let dx = (step.x - prev.x).abs();
            let dy = (step.y - prev.y).abs();
            assert!(
                dx <= 1 && dy <= 1 && (dx + dy) > 0,
                "step {step:?} is not adjacent to {prev:?}"
            );
            assert!(
                !blocked.contains(&(step.x, step.y)),
                "path walks through blocked tile {step:?}"
            );
            prev = step.clone();
        }
    }

    #[test]
    fn empty_path_when_already_on_goal() {
        let here = point(5, 5);
        let path = find_path_with(&here, &here, |_| true).expect("same-tile path exists");
        assert!(path.is_empty());
    }

    #[test]
    fn straight_diagonal_on_open_ground() {
        let start = point(0, 0);
        let goal = point(4, 4);
        let path = find_path_with(&start, &goal, |_| true).expect("open-ground path exists");
        // Uniform-cost diagonal: 4 single diagonal steps.
        assert_eq!(path.len(), 4);
        assert_valid_path(&start, &path, &goal, &HashSet::new());
    }

    #[test]
    fn routes_around_a_wall() {
        // Vertical wall at x == 2 for y in 0..=4, with a gap at y == 5 so the
        // only way across is to go around the bottom. A straight line from
        // (0,2) to (4,2) would slam into the wall at x==2.
        let mut blocked: HashSet<(i32, i32)> = HashSet::new();
        for y in 0..=4 {
            blocked.insert((2, y));
        }
        let start = point(0, 2);
        let goal = point(4, 2);
        let path = find_path_with(&start, &goal, |p| !blocked.contains(&(p.x, p.y)))
            .expect("a detour around the wall exists");
        assert_valid_path(&start, &path, &goal, &blocked);
        // The detour is necessarily longer than the blocked straight line.
        assert!(
            path.len() > 4,
            "expected a detour, got {} steps",
            path.len()
        );
    }

    #[test]
    fn none_when_goal_is_walled_off() {
        // Fully enclose the goal tile.
        let goal = point(3, 3);
        let mut blocked: HashSet<(i32, i32)> = HashSet::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx != 0 || dy != 0 {
                    blocked.insert((goal.x + dx, goal.y + dy));
                }
            }
        }
        let start = point(0, 0);
        let path = find_path_with(&start, &goal, |p| !blocked.contains(&(p.x, p.y)));
        assert!(path.is_none(), "walled-off goal must be unreachable");
    }

    #[test]
    fn none_when_goal_tile_itself_blocked() {
        let start = point(0, 0);
        let goal = point(2, 2);
        let path = find_path_with(&start, &goal, |p| (p.x, p.y) != (goal.x, goal.y));
        assert!(path.is_none(), "blocked goal tile is unreachable");
    }

    #[test]
    fn none_when_beyond_max_range() {
        let start = point(0, 0);
        let goal = point(PATHFIND_MAX_RANGE + 1, 0);
        let path = find_path_with(&start, &goal, |_| true);
        assert!(path.is_none(), "targets beyond max range are rejected");
    }

    #[test]
    fn adjacent_path_reaches_a_tile_next_to_an_occupied_goal() {
        // Goal tile is occupied (e.g. the player); a chaser must stop next to it.
        let start = point(0, 0);
        let goal = point(5, 0);
        let path = find_path_adjacent_with(&start, &goal, |p| (p.x, p.y) != (goal.x, goal.y))
            .expect("a tile adjacent to the goal is reachable");
        let last = path.last().expect("non-empty path");
        assert!(
            (last.x - goal.x).abs() <= 1 && (last.y - goal.y).abs() <= 1,
            "path must end adjacent to the goal, ended at {last:?}"
        );
        assert_ne!(
            (last.x, last.y),
            (goal.x, goal.y),
            "must not enter the goal"
        );
        // Steps are single-tile and never the occupied goal tile.
        let mut prev = start.clone();
        for step in &path {
            assert!((step.x - prev.x).abs() <= 1 && (step.y - prev.y).abs() <= 1);
            assert_ne!((step.x, step.y), (goal.x, goal.y));
            prev = step.clone();
        }
    }

    #[test]
    fn adjacent_path_is_empty_when_already_adjacent() {
        let start = point(4, 0);
        let goal = point(5, 0);
        let path = find_path_adjacent_with(&start, &goal, |_| true)
            .expect("already adjacent should succeed");
        assert!(path.is_empty(), "no step needed when already adjacent");
    }

    #[test]
    fn adjacent_path_routes_around_a_wall_to_reach_the_target() {
        // Wall at x==2 (y in 0..=4) with the chaser left of it and the target
        // tile (occupied) on the right; the only approach is around the bottom.
        let mut blocked: HashSet<(i32, i32)> = HashSet::new();
        for y in 0..=4 {
            blocked.insert((2, y));
        }
        let start = point(0, 2);
        let goal = point(4, 2);
        let path = find_path_adjacent_with(&start, &goal, |p| {
            (p.x, p.y) != (goal.x, goal.y) && !blocked.contains(&(p.x, p.y))
        })
        .expect("a detour to a tile adjacent to the target exists");
        let last = path.last().expect("non-empty path");
        assert!((last.x - goal.x).abs() <= 1 && (last.y - goal.y).abs() <= 1);
        for step in &path {
            assert!(
                !blocked.contains(&(step.x, step.y)),
                "stepped into wall {step:?}"
            );
        }
    }
}
