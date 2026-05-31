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

/// Find an 8-direction path from `start` to `goal`.
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
    if start == goal {
        return Some(Vec::new());
    }
    if tile_distance(start, goal) > PATHFIND_MAX_RANGE {
        return None;
    }
    if !can_occupy(world, goal.clone(), ignore_entity) {
        return None;
    }

    let start_key = (start.x, start.y);
    let goal_key = (goal.x, goal.y);

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
        if current.key == goal_key {
            return Some(reconstruct_path(&came_from, start_key, goal_key));
        }
        // Skip stale heap entries left over from a cheaper path being found.
        if current.g > *g_score.get(&current.key).unwrap_or(&i32::MAX) {
            continue;
        }

        expansions += 1;
        if expansions > PATHFIND_MAX_EXPANSIONS {
            return None;
        }

        let current_point = Point {
            x: current.key.0,
            y: current.key.1,
        };
        for direction in DIRECTIONS {
            let next = offset_point(&current_point, direction, 1);
            let next_key = (next.x, next.y);
            if next_key != goal_key && !can_occupy(world, next.clone(), ignore_entity) {
                continue;
            }
            // The goal tile is validated above, but intermediate tiles must be
            // occupiable. (The `next_key != goal_key` guard above lets us reuse
            // the already-confirmed goal without a redundant check.)
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

/// Compute the next tile to step onto when routing from `from` toward `to`.
///
/// `step` is the number of tiles to advance (1 for walk, 2 for run). The returned
/// point is the tile the mover should occupy after this step, following the A*
/// route. Returns `None` when no route exists.
pub(super) fn next_step_toward(
    world: &World,
    from: &Point,
    to: &Point,
    step: i32,
    ignore_entity: Option<Entity>,
) -> Option<Point> {
    let path = find_path(world, from, to, ignore_entity)?;
    if path.is_empty() {
        return None;
    }
    let index = (step.max(1) as usize).min(path.len()) - 1;
    Some(path[index].clone())
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
