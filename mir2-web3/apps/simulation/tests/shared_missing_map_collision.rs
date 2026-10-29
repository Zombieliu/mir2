//! Missing ordinary terrain must not silently become an explicitly open arena.
//! These are normal Join/Walk/Run/tick paths, not a successful Map.Load proof.
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    SessionId, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey, ZoneOutbound, ZoneRuntime,
};

const MAP: &str = "missing-collision-regression-20261010";

fn session(name: &str) -> SessionId {
    SessionId::new(name)
}

fn join(name: &str, id: u32, class: MirClass, gender: MirGender, x: i32) -> ZoneJoin {
    ZoneJoin {
        session_id: session(name),
        account_id: format!("{name}-account"),
        character_index: 0,
        object_id: id,
        name: name.into(),
        class,
        gender,
        level: 20,
        hp: 100,
        max_hp: 100,
        mp: 100,
        map_file_name: MAP.into(),
        position: Point { x, y: 10 },
        direction: MirDirection::Down,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }
}

fn packets_for<'a>(out: &'a [ZoneOutbound], name: &str) -> Vec<&'a ServerPacket> {
    let id = session(name);
    out.iter()
        .flat_map(|outbound| match outbound {
            ZoneOutbound::ToSession {
                session_id,
                packets,
            } if *session_id == id => packets.as_slice(),
            ZoneOutbound::ToMany {
                session_ids,
                packets,
            } if session_ids.contains(&id) => packets.as_slice(),
            ZoneOutbound::ToAll { packets } => packets.as_slice(),
            _ => &[],
        })
        .collect()
}

fn rejects_missing_terrain(run: bool) {
    for class in [MirClass::Warrior, MirClass::Wizard, MirClass::Taoist] {
        for gender in [MirGender::Male, MirGender::Female] {
            let mut zone = ZoneRuntime::new(ZoneKey::for_map(MAP));
            zone.handle(ZoneCommand::Join(join("owner", 101, class, gender, 10)));
            zone.handle(ZoneCommand::Join(join("observer", 102, class, gender, 15)));
            let original = zone.player_position(&session("owner")).unwrap();
            let command = if run {
                ZoneCommand::Run {
                    session_id: session("owner"),
                    direction: MirDirection::Right,
                    seq: 1,
                    now_ms: 0,
                }
            } else {
                ZoneCommand::Walk {
                    session_id: session("owner"),
                    direction: MirDirection::Right,
                    seq: 1,
                    now_ms: 0,
                }
            };
            let mut out = zone.handle(command);
            out.extend(zone.tick_pending_movement(0));
            assert_eq!(
                zone.player_position(&session("owner")),
                Some(original.clone()),
                "{class:?}/{gender:?}"
            );
            assert!(packets_for(&out, "owner").iter().any(|packet| matches!(
                packet, ServerPacket::UserLocation { location } if location.position == original
            )), "Missing terrain must correct the owner rather than acknowledge successful movement: {out:?}");
            assert!(
                !packets_for(&out, "observer").iter().any(|packet| matches!(
                    packet,
                    ServerPacket::ObjectWalk { .. } | ServerPacket::ObjectRun { .. }
                )),
                "Rejected movement must not broadcast a fictional walk/run: {out:?}"
            );
            for now_ms in [600, 1_200, 10_000] {
                let later = zone.tick_pending_movement(now_ms);
                assert_eq!(
                    zone.player_position(&session("owner")),
                    Some(original.clone())
                );
                assert!(
                    !packets_for(&later, "owner")
                        .iter()
                        .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })),
                    "The rejected intent must be consumed, not replay later: {later:?}"
                );
            }
        }
    }
}

#[test]
fn normal_missing_map_walk_corrects_owner_without_broadcast_or_later_drift() {
    rejects_missing_terrain(false);
}

#[test]
fn normal_missing_map_run_corrects_owner_without_broadcast_or_later_drift() {
    rejects_missing_terrain(true);
}

#[test]
fn trusted_explicit_unbounded_arena_retains_normal_walk_and_run() {
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map(MAP), ZoneCollision::unbounded());
    zone.handle(ZoneCommand::Join(join(
        "owner",
        101,
        MirClass::Warrior,
        MirGender::Male,
        10,
    )));
    zone.handle(ZoneCommand::Walk {
        session_id: session("owner"),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    zone.tick_pending_movement(0);
    assert_eq!(
        zone.player_position(&session("owner")),
        Some(Point { x: 11, y: 10 })
    );
    zone.handle(ZoneCommand::Run {
        session_id: session("owner"),
        direction: MirDirection::Right,
        seq: 2,
        now_ms: 600,
    });
    zone.tick_pending_movement(600);
    assert_eq!(
        zone.player_position(&session("owner")),
        Some(Point { x: 13, y: 10 })
    );
}

#[test]
fn missing_map_stays_closed_after_ordinary_checkpoint_restore() {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map(MAP));
    zone.handle(ZoneCommand::Join(join(
        "owner",
        101,
        MirClass::Taoist,
        MirGender::Female,
        10,
    )));
    let bytes = zone.checkpoint_bytes().unwrap();
    let mut restored = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
    let original = restored.player_position(&session("owner")).unwrap();
    restored.handle(ZoneCommand::Walk {
        session_id: session("owner"),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    restored.tick_pending_movement(0);
    assert_eq!(restored.player_position(&session("owner")), Some(original));
}

#[test]
fn strict_restore_does_not_turn_missing_terrain_into_an_explicit_open_arena() {
    let zone = ZoneRuntime::new_with_collision(ZoneKey::for_map(MAP), ZoneCollision::unbounded());
    let bytes = zone.checkpoint_bytes().unwrap();
    assert!(ZoneRuntime::restore_checkpoint(&bytes)
        .err()
        .expect("Strict map recovery must reject an open-arena collision identity")
        .contains("state root mismatch"));
}
