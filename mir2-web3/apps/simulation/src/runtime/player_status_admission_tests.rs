//! Personal status admission must agree with shared-zone escape rules.
use super::*;

fn world_with_status(key: &str, expires_at_tick: u64) -> World {
    let mut world = World::new();
    world.insert_resource(RuntimeClockResource { tick: 10 });
    world.insert_resource(BuffResource {
        buffs: vec![BuffState {
            key: key.into(),
            name: key.into(),
            description: "status admission fixture".into(),
            expires_at_tick,
            real_time_duration: None,
            attack_bonus: 0,
            defence_bonus: 0,
            stats: Vec::new(),
        }],
    });
    world
}

#[test]
fn crowded_personal_stun_permits_escape_and_melee_but_blocks_cast() {
    let world = world_with_status(MAN_TREE_STUN_BUFF_KEY, 20);
    assert!(!crystal_player_movement_blocked_by_status(&world));
    assert!(!crystal_player_attack_blocked_by_status(&world));
    assert!(crystal_player_magic_blocked_by_status(&world));
}

#[test]
fn crowded_personal_dazed_permits_escape_but_blocks_attack_and_cast() {
    let world = world_with_status(HELL_KEEPER_DAZED_BUFF_KEY, 20);
    assert!(!crystal_player_movement_blocked_by_status(&world));
    assert!(crystal_player_attack_blocked_by_status(&world));
    assert!(crystal_player_magic_blocked_by_status(&world));
}

#[test]
fn crowded_personal_real_control_is_finite_and_still_blocks_escape() {
    for key in [CAVE_MAGGOT_PARALYSIS_BUFF_KEY, ICE_GUARD_FROZEN_BUFF_KEY] {
        let mut world = world_with_status(key, 20);
        assert!(crystal_player_movement_blocked_by_status(&world));
        world.resource_mut::<RuntimeClockResource>().tick = 20;
        assert!(!crystal_player_movement_blocked_by_status(&world));
        assert!(!crystal_player_attack_blocked_by_status(&world));
        assert!(!crystal_player_magic_blocked_by_status(&world));
    }
}
