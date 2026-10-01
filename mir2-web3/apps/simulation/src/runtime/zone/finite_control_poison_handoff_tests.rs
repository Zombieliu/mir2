//! Counted leases must remain outside the finite online-handoff contract.
use super::*;
use mir2_protocol::MirGender;

fn joined(map: &str) -> (ZoneRuntime, SessionId) {
    let session = SessionId::new("finite-versus-counted-poison");
    let mut zone = ZoneRuntime::new_with_collision(ZoneKey::for_map(map), ZoneCollision::unbounded());
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: session.clone(), account_id: "finite-versus-counted-account".into(),
        character_index: 0, object_id: 101, name: "PoisonScope".into(),
        class: MirClass::Wizard, gender: MirGender::Male, level: 26,
        hp: 1_000, max_hp: 1_000, mp: 100, map_file_name: map.into(),
        position: Point { x: 328, y: 264 }, direction: MirDirection::Right,
        chat_profile: Default::default(), combat_stats: Default::default(),
    }));
    (zone, session)
}

#[test]
fn finite_handoff_does_not_turn_a_counted_paralysis_lease_into_an_immortal_mask() {
    let (mut source, session) = joined("source");
    // Real server application creates the counted AI poison lease with its own
    // tick clock. It shares a control bit but is deliberately not transferable.
    source.apply_native_player_periodic_poison(0, &session, 101, 32, 0, 5, 1_000, 100);
    source.apply_native_player_status_poison(&session, CRYSTAL_POISON_SLOW, 3_000, 100);
    let clock = source.player_finite_control_poison_clock(&session).unwrap();
    let (mut destination, same_session) = joined("destination");
    destination.restore_player_finite_control_poison_clock(&same_session, clock);
    assert_eq!(destination.players[&session].poison, CRYSTAL_POISON_SLOW);
    assert!(!destination.players[&session].native_status_poison_deadlines.contains_key(&32));
    assert!(destination.native_periodic_player_poisons.is_empty());
    destination.tick(3_100);
    assert_eq!(destination.players[&session].poison, 0,
        "the finite slow clock expires and no discarded counted bit can survive");
}
