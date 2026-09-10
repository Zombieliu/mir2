//! Original HumanObject item-set bonuses, shared by player and Hero stat engines.
//! Input tuples are (raw equipment slot, source set ID, source item type).
use std::collections::{BTreeMap, BTreeSet};
struct StatAccumulator<'a>(&'a mut BTreeMap<u8, i32>);
impl StatAccumulator<'_> {
    fn get(&self, stat: u8) -> i32 { self.0.get(&stat).copied().unwrap_or_default() }
    fn add(&mut self, stat: u8, value: i32) { self.0.insert(stat, self.get(stat).saturating_add(value)); }
}
fn needed(set: u8) -> usize {
    match set {
        9..=11 | 21..=25 => 2,
        3..=8 | 13 | 14 | 38 => 3,
        2 => 4,
        1 | 15..=20 | 26..=31 | 39 => 5,
        _ => 0,
    }
}
fn full_set_bonus(set: u8) -> &'static [(u8, i32)] {
    match set {
        9 => &[(12, 50)],
        10 => &[(13, 50)],
        11 => &[(12, 30), (13, 30)],
        3 => &[(10, 2)],
        4 => &[(12, 50), (13, -50)],
        5 => &[(4, 1), (5, 3)],
        6 => &[(6, 1), (7, 2)],
        7 => &[(8, 1), (9, 2)],
        8 => &[(0, 2), (1, 2)],
        1 => &[(4, 2), (5, 5), (14, 2)],
        13 => &[(1, 2), (7, 1), (9, 1)],
        14 => &[(5, 1), (7, 1), (9, 1), (3, 1), (31, 1)],
        15 => &[(5, 2), (1, 2)],
        16 => &[(5, 3), (12, 30), (14, 2)],
        17 => &[(7, 2), (3, 2)],
        18 => &[(7, 2), (13, 40), (11, 2)],
        19 => &[(9, 2), (1, 1), (3, 1)],
        20 => &[(9, 2), (12, 15), (13, 20), (21, 1), (10, 1)],
        21 => &[(5, 1), (16, 25)],
        22 => &[(7, 1), (16, 17)],
        23 => &[(9, 1), (16, 17)],
        24 => &[(5, 1), (16, 20)],
        25 => &[(5, 1), (16, 17)],
        26 => &[(9, 2), (12, 15), (13, 20), (21, 1), (10, 1)],
        27 => &[(30, 1), (31, 1)],
        28 => &[(1, 1), (11, 1)],
        31 => &[(4, 1), (5, 1), (6, 1), (7, 1), (17, 1), (18, 2)],
        39 => &[(4, 1), (5, 2), (7, 2), (10, 1), (12, 50)],
        38 => &[(13, 25), (14, 2)],
        _ => &[],
    }
}
#[derive(Default)]
struct SetGroup {
    set: u8,
    types: BTreeSet<u8>,
}
pub(super) fn apply_sets(values: &mut BTreeMap<u8, i32>, equipment: &[(u8, u8, u8)]) {
    let mut stats = StatAccumulator(values);
    let mut groups: Vec<SetGroup> = Vec::new();
    let mut mir = BTreeSet::new();
    for &(slot, set, kind) in equipment {
        if set == 0 {
            continue;
        }
        if set == 12 {
            mir.insert(slot);
        }
        if let Some(group) = groups
            .iter_mut()
            .find(|g| g.set == set && !g.types.contains(&kind) && g.types.len() < needed(g.set))
        {
            group.types.insert(kind);
        } else {
            groups.push(SetGroup {
                set,
                types: BTreeSet::from([kind]),
            });
        }
    }
    let mut partial = BTreeSet::new();
    for group in groups {
        let set = group.set;
        if group.types.contains(&7) && group.types.contains(&6) && partial.insert(set) {
            match set {
                5 => stats.add(14, 2),
                7 => stats.add(21, 3),
                6 => {
                    stats.add(18, 5);
                    stats.add(16, 20);
                }
                _ => {}
            }
        }
        if set == 38 && group.types.contains(&5) && group.types.contains(&6) {
            stats.add(12, 25);
        }
        if group.types.len() < needed(set) {
            continue;
        }
        if set == 8 {
            stats.add(12, ((f64::from(stats.get(12)) / 100.0) * 30.0) as i32);
        }
        for &(stat, value) in full_set_bonus(set) {
            stats.add(stat, value);
        }
    }
    let mut apply = |condition: bool, values: &[(u8, i32)]| {
        if condition {
            for &(stat, value) in values {
                stats.add(stat, value);
            }
        }
    };
    apply(
        mir.len() == 10,
        &[
            (1, 1),
            (3, 1),
            (16, 70),
            (15, 2),
            (14, 2),
            (12, 70),
            (13, 80),
            (30, 6),
            (31, 6),
        ],
    );
    apply(mir.contains(&7) && mir.contains(&8), &[(3, 1), (1, 1)]);
    apply(mir.contains(&5) && mir.contains(&6), &[(0, 1), (2, 1)]);
    apply(
        (mir.contains(&7) || mir.contains(&8))
            && (mir.contains(&5) || mir.contains(&6))
            && mir.contains(&4),
        &[(3, 1), (1, 1), (16, 30), (18, 17)],
    );
    apply(
        [7, 8, 5, 6, 4].iter().all(|v| mir.contains(v)),
        &[(3, 1), (1, 1), (16, 20), (18, 10)],
    );
    apply(
        [1, 2, 0].iter().all(|v| mir.contains(v)),
        &[(5, 2), (7, 1), (9, 1), (11, 1)],
    );
    apply(
        [1, 11, 10].iter().all(|v| mir.contains(v)),
        &[(5, 1), (7, 1), (9, 1), (17, 17)],
    );
    apply(
        [1, 11, 10, 2, 0].iter().all(|v| mir.contains(v)),
        &[(4, 1), (5, 1), (6, 1), (7, 1), (8, 1), (9, 1), (17, 17)],
    );
}
