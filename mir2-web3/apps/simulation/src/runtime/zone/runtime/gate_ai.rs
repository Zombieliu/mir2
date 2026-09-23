//! Crystal AI81 peaceful castle gate geometry. Conquest ownership/war commands
//! must supply trusted shared authority before enabling gate damage or opening.
use super::*;

fn gate_offsets(effect: u8) -> &'static [(i32, i32)] {
    match effect {
        1 => &[
            (0, -1),
            (0, -2),
            (1, -1),
            (1, -2),
            (-1, 0),
            (-2, 0),
            (-1, -1),
            (-1, 1),
        ],
        2 => &[
            (0, -1),
            (-1, -1),
            (-1, 0),
            (0, 1),
            (1, 1),
            (1, 0),
            (1, -1),
            (0, -2),
        ],
        3 => &[
            (1, -1),
            (0, -1),
            (0, -2),
            (-1, -1),
            (-2, -1),
            (-1, 0),
            (-1, 1),
            (-2, 0),
        ],
        4 => &[(-2, 0), (-1, 1), (0, 2), (0, 1), (-1, 0), (-1, -1), (1, 1)],
        _ => &[],
    }
}

/// Integer equivalent of .NET Math.Round (ties to even), without float drift.
fn closed_facing(hp: i32, max_hp: i32) -> MirDirection {
    let denominator = i64::from(max_hp.max(1));
    let numerator = 3 * i64::from(hp.max(0));
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    let rounded = quotient
        + i64::from(
            remainder * 2 > denominator || (remainder * 2 == denominator && quotient % 2 != 0),
        );
    match 3 - rounded.clamp(1, 3) {
        0 => MirDirection::Up,
        1 => MirDirection::UpRight,
        _ => MirDirection::Right,
    }
}

pub(in crate::runtime::zone) fn initialize_gate(m: &mut ZoneNativeMonster) {
    if m.ai == 81 {
        m.direction = closed_facing(m.hp, m.max_hp);
    }
}

impl ZoneRuntime {
    /// Derived dynamic occupancy never mutates static map walls. Removing a
    /// gate therefore cannot accidentally remove an overlapping real wall.
    pub(super) fn gate_blocks_tile(&self, point: &Point) -> bool {
        self.native_monsters.values().any(|m| {
            if m.ai != 81 || m.dead || m.hp <= 0 {
                return false;
            }
            let effect = crystal_monster_by_name(&m.name).map_or(0, |t| t.effect);
            gate_offsets(effect).iter().any(|(dx, dy)| {
                i64::from(m.position.x) + i64::from(*dx) == i64::from(point.x)
                    && i64::from(m.position.y) + i64::from(*dy) == i64::from(point.y)
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gate_rounding_matches_dotnet_half_even() {
        assert_eq!(closed_facing(100, 100), MirDirection::Up);
        assert_eq!(closed_facing(50, 60), MirDirection::UpRight); // 2.5 -> 2
        assert_eq!(closed_facing(30, 60), MirDirection::UpRight); // 1.5 -> 2
        assert_eq!(closed_facing(10, 60), MirDirection::Right); // .5 -> 0, min 1
    }
    #[test]
    fn all_four_source_gate_footprints_are_distinct_and_unique() {
        for effect in 1..=4 {
            let offsets = gate_offsets(effect);
            let unique: std::collections::BTreeSet<_> = offsets.iter().copied().collect();
            assert_eq!(unique.len(), if effect == 4 { 7 } else { 8 });
            assert!(!unique.contains(&(0, 0)));
        }
        assert!(gate_offsets(0).is_empty());
        assert_ne!(gate_offsets(1), gate_offsets(2));
        assert_ne!(gate_offsets(2), gate_offsets(3));
    }
}
