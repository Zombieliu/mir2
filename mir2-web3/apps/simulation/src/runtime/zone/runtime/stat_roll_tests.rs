use super::zone_roll_stat_range;
use std::collections::BTreeSet;

#[test]
fn stat_roll_does_not_alias_real_combat_cadences() {
    let epoch = 1_791_265_024_000u64;
    for cadence in [300, 600, 2500] {
        for (lo, hi) in [(4, 6), (2, 6)] {
            let rolls: BTreeSet<_> = (0..128).map(|step|
                zone_roll_stat_range(lo, hi, epoch + step * cadence, 1000, 17)
            ).collect();
            assert_eq!(rolls.len(), (hi - lo + 1) as usize,
                "combat cadence {cadence} aliases inclusive stat range {lo}..={hi}");
        }
    }
}

#[test]
fn stat_roll_replay_and_actor_channels_are_independent() {
    let tick = 1_791_265_024_000;
    let a: Vec<_> = (0..128).map(|step|
        zone_roll_stat_range(0, 65535, tick + step * 600, 1000, 17)
    ).collect();
    let replay: Vec<_> = (0..128).map(|step|
        zone_roll_stat_range(0, 65535, tick + step * 600, 1000, 17)
    ).collect();
    let actor: Vec<_> = (0..128).map(|step|
        zone_roll_stat_range(0, 65535, tick + step * 600, 1001, 17)
    ).collect();
    let channel: Vec<_> = (0..128).map(|step|
        zone_roll_stat_range(0, 65535, tick + step * 600, 1000, 18)
    ).collect();
    assert_eq!(a, replay);
    assert_ne!(a, actor);
    assert_ne!(a, channel);
}

#[test]
fn stat_roll_handles_constant_reversed_negative_and_wide_ranges() {
    for tick in [0, 1, u64::MAX, 1_791_265_024_000] {
        assert_eq!(zone_roll_stat_range(7, 7, tick, 1, 9), 7);
        assert_eq!(zone_roll_stat_range(-7, -2, tick, 1, 9), 0);
        assert!((2..=6).contains(&zone_roll_stat_range(6, 2, tick, 1, 9)));
        assert!((0..=i32::MAX).contains(&zone_roll_stat_range(0, i32::MAX, tick, 1, 9)));
    }
}
