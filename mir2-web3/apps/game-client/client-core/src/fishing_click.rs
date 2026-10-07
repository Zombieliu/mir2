//! Shared, renderer-free decision for the existing blocked-walk fishing click.
//!
//! The host owns pointer custody, current map/entity evidence, movement readiness,
//! rod identity, pose provenance and final dispatch. These functions only consume
//! explicit facts; they never send, acknowledge, retry or change game state.

/// Crystal direction order: up, upright, right, downright, down, downleft,
/// left, upleft. Invalid raw direction values are rejected before indexing.
const DIRECTION_DELTAS: [(i32, i32); 8] = [
    (0, -1), (1, -1), (1, 0), (1, 1),
    (0, 1), (-1, 1), (-1, 0), (-1, -1),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FishingWalkCandidate {
    pub direction: u8,
    pub cell: (i32, i32),
}

/// Raw map-cell evidence, bound to the exact three-tile target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FishingWaterCell {
    pub cell: (i32, i32),
    pub light: u8,
}

/// Current host facts. Missing pose/transform facts stay unknown, especially on
/// Web: absence must not become standing=true or a fabricated transform=-1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FishingClickFacts {
    pub origin: (i32, i32),
    pub direction: u8,
    pub requested_walk: bool,
    pub auto_route: bool,
    /// Known blockage of direct, clockwise +1, counter-clockwise -1 walks.
    /// Hosts obtain the matching directions/cells from fishing_walk_candidates.
    pub walk_blocked: [Option<bool>; 3],
    pub rod_present: Option<bool>,
    pub water: Option<FishingWaterCell>,
    /// Preserve the host's actual facing comparison; unknown is not a mismatch.
    pub facing_matches: Option<bool>,
    pub standing: Option<bool>,
    pub fishing: Option<bool>,
    pub transform_type: Option<i16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FishingClickDecision {
    None,
    Turn { direction: u8, delay_ms: u16 },
    Cast,
}

fn checked_target(origin: (i32, i32), direction: u8, distance: i32) -> Option<(i32, i32)> {
    let &(dx, dy) = DIRECTION_DELTAS.get(usize::from(direction))?;
    Some((
        origin.0.checked_add(dx.checked_mul(distance)?)?,
        origin.1.checked_add(dy.checked_mul(distance)?)?,
    ))
}

/// The three one-tile walks tried by Crystal, in their original order. There is
/// no route search or run degradation here: the caller requested a primary Walk.
pub fn fishing_walk_candidates(
    origin: (i32, i32),
    direction: u8,
) -> Option<[FishingWalkCandidate; 3]> {
    DIRECTION_DELTAS.get(usize::from(direction))?;
    let directions = [direction, (direction + 1) % 8, (direction + 7) % 8];
    Some([
        FishingWalkCandidate { direction: directions[0], cell: checked_target(origin, directions[0], 1)? },
        FishingWalkCandidate { direction: directions[1], cell: checked_target(origin, directions[1], 1)? },
        FishingWalkCandidate { direction: directions[2], cell: checked_target(origin, directions[2], 1)? },
    ])
}

/// Checked three-tile offset in the requested direction, including diagonals.
pub fn fishing_water_target(origin: (i32, i32), direction: u8) -> Option<(i32, i32)> {
    checked_target(origin, direction, 3)
}

/// Crystal FishingCell raw light rule. This does not infer water from textures,
/// collision or a decoded attribute whose original light is unknown.
pub const fn fishing_attribute(light: u8) -> Option<u8> {
    if light >= 100 && light <= 119 { Some(light - 100) } else { None }
}

/// Pure decision after normal host walk/turn readiness checks. A blocked Run or
/// auto route is not a fishing request. NewMove itself is not an exclusion.
pub fn decide_fishing_click(
    facts: &FishingClickFacts,
    now_ms: u64,
    last_cast_ms: u64,
) -> FishingClickDecision {
    if !facts.requested_walk || facts.auto_route
        || facts.walk_blocked.iter().any(|blocked| *blocked != Some(true))
        || facts.rod_present != Some(true)
        || fishing_walk_candidates(facts.origin, facts.direction).is_none()
    {
        return FishingClickDecision::None;
    }
    let Some(water) = facts.water else { return FishingClickDecision::None; };
    if fishing_water_target(facts.origin, facts.direction) != Some(water.cell)
        || fishing_attribute(water.light).is_none()
    {
        return FishingClickDecision::None;
    }
    match facts.facing_matches {
        // Native turns before checking standing, current fishing, transform or
        // the cast clock. The host applies the returned delay to movement.
        Some(false) => FishingClickDecision::Turn { direction: facts.direction, delay_ms: 200 },
        Some(true) => {
            if facts.standing != Some(true) || facts.fishing != Some(false)
                || !facts.transform_type.is_some_and(|transform| !(6..=9).contains(&transform))
                || !fishing_cast_ready(now_ms, last_cast_ms)
            {
                FishingClickDecision::None
            } else {
                FishingClickDecision::Cast
            }
        }
        None => FishingClickDecision::None,
    }
}

/// Preserve Native's full-width, saturating one-second last-cast clock.
pub const fn fishing_cast_ready(now_ms: u64, last_cast_ms: u64) -> bool {
    now_ms >= last_cast_ms.saturating_add(1000)
}

/// Claim only an eligible clock. The host must first accept the current Cast
/// decision and finish its own custody checks. No ACK is required for reuse.
pub fn claim_fishing_cast(now_ms: u64, last_cast_ms: &mut u64) -> bool {
    if !fishing_cast_ready(now_ms, *last_cast_ms) { return false; }
    *last_cast_ms = now_ms;
    true
}

/// Only a definitely unsent cast uses this Native-compatible reset. Transport
/// unknown is not definitely unsent and must retain its ordinary clock value.
pub fn release_unsent_fishing_cast(last_cast_ms: &mut u64) {
    *last_cast_ms = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> FishingClickFacts {
        FishingClickFacts {
            origin: (10, 10), direction: 2, requested_walk: true, auto_route: false,
            walk_blocked: [Some(true); 3], rod_present: Some(true),
            water: Some(FishingWaterCell { cell: (13, 10), light: 100 }),
            facing_matches: Some(true), standing: Some(true), fishing: Some(false),
            transform_type: Some(0),
        }
    }

    #[test]
    fn eight_direction_water_targets_use_checked_three_tile_goldens() {
        let targets = [(10, 7), (13, 7), (13, 10), (13, 13), (10, 13), (7, 13), (7, 10), (7, 7)];
        for (direction, expected) in targets.into_iter().enumerate() {
            assert_eq!(fishing_water_target((10, 10), direction as u8), Some(expected));
        }
    }

    #[test]
    fn walk_candidates_preserve_direct_clockwise_counterclockwise_order() {
        assert_eq!(fishing_walk_candidates((10, 10), 2), Some([
            FishingWalkCandidate { direction: 2, cell: (11, 10) },
            FishingWalkCandidate { direction: 3, cell: (11, 11) },
            FishingWalkCandidate { direction: 1, cell: (11, 9) },
        ]));
        assert_eq!(fishing_walk_candidates((10, 10), 0), Some([
            FishingWalkCandidate { direction: 0, cell: (10, 9) },
            FishingWalkCandidate { direction: 1, cell: (11, 9) },
            FishingWalkCandidate { direction: 7, cell: (9, 9) },
        ]));
        assert_eq!(fishing_walk_candidates((10, 10), 7), Some([
            FishingWalkCandidate { direction: 7, cell: (9, 9) },
            FishingWalkCandidate { direction: 0, cell: (10, 9) },
            FishingWalkCandidate { direction: 6, cell: (9, 10) },
        ]));
    }

    #[test]
    fn invalid_direction_and_coordinate_overflow_never_wrap_or_saturate() {
        for direction in [8, 9, u8::MAX] {
            assert_eq!(fishing_water_target((0, 0), direction), None);
            assert_eq!(fishing_walk_candidates((0, 0), direction), None);
            let mut input = facts(); input.direction = direction;
            assert_eq!(decide_fishing_click(&input, 1000, 0), FishingClickDecision::None);
        }
        assert_eq!(fishing_water_target((i32::MAX - 2, 0), 2), None);
        assert_eq!(fishing_water_target((i32::MIN + 2, 0), 6), None);
        assert_eq!(fishing_water_target((0, i32::MIN + 2), 0), None);
        assert_eq!(fishing_water_target((0, i32::MAX - 2), 4), None);
        assert_eq!(fishing_water_target((i32::MAX, 3), 0), Some((i32::MAX, 0)));
        assert_eq!(fishing_walk_candidates((i32::MAX, 3), 0), None, "overflowing alternate cannot prove all walks blocked");
        let mut input = facts(); input.origin = (i32::MAX, 3); input.direction = 0;
        input.water = Some(FishingWaterCell { cell: (i32::MAX, 0), light: 100 });
        assert_eq!(decide_fishing_click(&input, 1000, 0), FishingClickDecision::None);
    }

    #[test]
    fn only_original_raw_light_range_carries_fishing_attributes() {
        assert_eq!(fishing_attribute(100), Some(0));
        assert_eq!(fishing_attribute(110), Some(10));
        assert_eq!(fishing_attribute(119), Some(19));
        for light in [0, 1, 9, 99, 120, u8::MAX] { assert_eq!(fishing_attribute(light), None); }
    }

    #[test]
    fn primary_walk_requires_all_three_known_blocked_candidates_and_no_route() {
        let mut input = facts();
        assert_eq!(decide_fishing_click(&input, 1000, 0), FishingClickDecision::Cast);
        for index in 0..3 {
            for blocked in [Some(false), None] {
                input = facts(); input.walk_blocked[index] = blocked;
                assert_eq!(decide_fishing_click(&input, 1000, 0), FishingClickDecision::None);
            }
        }
        input = facts(); input.requested_walk = false;
        assert_eq!(decide_fishing_click(&input, 1000, 0), FishingClickDecision::None);
        input = facts(); input.auto_route = true;
        assert_eq!(decide_fishing_click(&input, 1000, 0), FishingClickDecision::None);
    }

    #[test]
    fn rod_and_exact_raw_water_are_required_even_before_turn() {
        let mut input = facts(); input.facing_matches = Some(false);
        for rod in [Some(false), None] {
            input.rod_present = rod;
            assert_eq!(decide_fishing_click(&input, 1000, 0), FishingClickDecision::None);
        }
        input = facts(); input.facing_matches = Some(false); input.water = None;
        assert_eq!(decide_fishing_click(&input, 1000, 0), FishingClickDecision::None);
        for water in [FishingWaterCell { cell: (12, 10), light: 100 },
            FishingWaterCell { cell: (13, 10), light: 99 }, FishingWaterCell { cell: (13, 10), light: 120 }] {
            input.water = Some(water);
            assert_eq!(decide_fishing_click(&input, 1000, 0), FishingClickDecision::None);
        }
    }

    #[test]
    fn wrong_facing_turn_precedes_pose_fishing_transform_and_cast_cooldown() {
        let mut input = facts(); input.facing_matches = Some(false);
        input.standing = None; input.fishing = Some(true); input.transform_type = Some(6);
        assert_eq!(decide_fishing_click(&input, 0, u64::MAX),
            FishingClickDecision::Turn { direction: 2, delay_ms: 200 });
        input.transform_type = None;
        assert_eq!(decide_fishing_click(&input, 999, 0),
            FishingClickDecision::Turn { direction: 2, delay_ms: 200 });
        input.facing_matches = None;
        assert_eq!(decide_fishing_click(&input, 1000, 0), FishingClickDecision::None);
    }

    #[test]
    fn matching_facing_requires_known_standing_not_fishing_and_exact_transform() {
        let mut input = facts();
        for standing in [None, Some(false)] {
            input.standing = standing;
            assert_eq!(decide_fishing_click(&input, 1000, 0), FishingClickDecision::None);
        }
        input = facts();
        for fishing in [None, Some(true)] {
            input.fishing = fishing;
            assert_eq!(decide_fishing_click(&input, 1000, 0), FishingClickDecision::None);
        }
        input = facts();
        for transform in [None, Some(6), Some(7), Some(8), Some(9)] {
            input.transform_type = transform;
            assert_eq!(decide_fishing_click(&input, 1000, 0), FishingClickDecision::None);
        }
        for transform in [i16::MIN, -1, 0, 5, 10, i16::MAX] {
            input.transform_type = Some(transform);
            assert_eq!(decide_fishing_click(&input, 1000, 0), FishingClickDecision::Cast);
        }
    }

    #[test]
    fn cast_cooldown_keeps_native_initial_and_exact_one_second_boundaries() {
        let input = facts();
        assert_eq!(decide_fishing_click(&input, 999, 0), FishingClickDecision::None);
        assert_eq!(decide_fishing_click(&input, 1000, 0), FishingClickDecision::Cast);
        assert_eq!(decide_fishing_click(&input, 1999, 1000), FishingClickDecision::None);
        assert_eq!(decide_fishing_click(&input, 2000, 1000), FishingClickDecision::Cast);
        assert!(!fishing_cast_ready(999, 0)); assert!(fishing_cast_ready(1000, 0));
    }

    #[test]
    fn clock_claim_changes_only_accepted_cast_and_rejects_rollback() {
        let mut last = 0;
        assert!(!claim_fishing_cast(999, &mut last)); assert_eq!(last, 0);
        assert!(claim_fishing_cast(1000, &mut last)); assert_eq!(last, 1000);
        assert!(!claim_fishing_cast(500, &mut last)); assert_eq!(last, 1000);
        assert!(!claim_fishing_cast(1999, &mut last)); assert_eq!(last, 1000);
        assert!(claim_fishing_cast(2000, &mut last)); assert_eq!(last, 2000);
        assert!(claim_fishing_cast(3000, &mut last), "ordinary clock needs no ACK barrier");
    }

    #[test]
    fn release_definitely_unsent_resets_zero_instead_of_restoring_prior_cast() {
        let mut last = 1000;
        assert!(claim_fishing_cast(2000, &mut last));
        release_unsent_fishing_cast(&mut last); assert_eq!(last, 0);
        assert!(!claim_fishing_cast(999, &mut last)); assert_eq!(last, 0);
        assert!(claim_fishing_cast(2000, &mut last)); assert_eq!(last, 2000);
    }

    #[test]
    fn full_u64_clock_preserves_saturation_including_native_maximum_boundary() {
        let mut last = u64::MAX - 500;
        assert!(!fishing_cast_ready(u64::MAX - 1, last));
        assert!(!claim_fishing_cast(u64::MAX - 1, &mut last)); assert_eq!(last, u64::MAX - 500);
        assert!(claim_fishing_cast(u64::MAX, &mut last)); assert_eq!(last, u64::MAX);
        assert!(fishing_cast_ready(u64::MAX, last), "preserve Native saturated threshold, including equality");
        assert_eq!(decide_fishing_click(&facts(), u64::MAX, last), FishingClickDecision::Cast);
        assert!(!fishing_cast_ready(u64::MAX - 1, last));
    }
}
