use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    SimulationConfig, SimulationSession, VisiblePlayerRecord, WorldEntityDisposition,
    WorldEntityKind, WorldEntitySnapshot,
};

const FOX_START: u32 = 990_100;
const FOX_COUNT: u32 = 16;

fn start_session(config: SimulationConfig) -> SimulationSession {
    let mut session = SimulationSession::new(config);
    let login = session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    assert!(
        login
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })),
        "demo login should succeed: {login:?}"
    );
    let start = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(
        start.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::StartGame {
                    result: 4,
                    resolution
                } if *resolution > 0
            )
        }),
        "StartGame should enter the world: {start:?}"
    );
    session
}

fn self_player(session: &SimulationSession) -> WorldEntitySnapshot {
    session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .expect("world snapshot should contain SelfPlayer")
}

fn monster_snapshot(
    object_id: u32,
    name: &str,
    ai: u8,
    position: Point,
    disposition: WorldEntityDisposition,
    dead: bool,
) -> WorldEntitySnapshot {
    WorldEntitySnapshot {
        object_id,
        kind: WorldEntityKind::Monster,
        name: name.to_string(),
        owner_name: None,
        ai: Some(ai),
        x: position.x,
        y: position.y,
        direction: MirDirection::Left,
        class: None,
        gender: None,
        level: Some(1),
        hp: Some(if dead { 0 } else { 100 }),
        max_hp: Some(100),
        light: 0,
        wing_effect: None,
        name_colour_argb: -1,
        dead,
        riding_mount: None,
        can_mount_attack: None,
        has_class_weapon: None,
        dazed: None,
        fishing: None,
        disposition,
        sprite: None,
        quest_ids: Vec::new(),
        quest_icon: None,
    }
}

fn add_shared_monster(
    session: &mut SimulationSession,
    object_id: u32,
    name: &str,
    ai: u8,
    position: Point,
    disposition: WorldEntityDisposition,
    dead: bool,
) {
    assert!(
        session.apply_shared_entity_snapshot(&monster_snapshot(
            object_id,
            name,
            ai,
            position,
            disposition,
            dead,
        )),
        "shared monster should materialize: object_id={object_id}, name={name}"
    );
}

fn entity_position(session: &SimulationSession, object_id: u32) -> Point {
    let entity = session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.object_id == object_id)
        .unwrap_or_else(|| panic!("object {object_id} should be visible"));
    Point {
        x: entity.x,
        y: entity.y,
    }
}

fn add_fox_ring(session: &mut SimulationSession, position: Point) {
    for offset in 0..FOX_COUNT {
        add_shared_monster(
            session,
            FOX_START + offset,
            "GreatFoxSpirit",
            50,
            position.clone(),
            WorldEntityDisposition::Hostile,
            false,
        );
    }
}

#[test]
fn recall_uses_first_qualified_opposing_monster_before_player() {
    let mut session = start_session(SimulationConfig::default());
    let player = self_player(&session);
    let fox_position = Point {
        x: player.x + 10,
        y: player.y,
    };
    let opposing_monster_position = Point {
        x: fox_position.x + 4,
        y: fox_position.y,
    };

    add_fox_ring(&mut session, fox_position.clone());
    add_shared_monster(
        &mut session,
        991_001,
        "Deer",
        1,
        opposing_monster_position.clone(),
        WorldEntityDisposition::Neutral,
        false,
    );
    // A same-side monster is in range but is not an IsAttackTarget candidate.
    add_shared_monster(
        &mut session,
        991_002,
        "Deer",
        0,
        Point {
            x: fox_position.x + 3,
            y: fox_position.y + 1,
        },
        WorldEntityDisposition::Hostile,
        false,
    );
    // A dead opposing monster is in range but must be skipped.
    add_shared_monster(
        &mut session,
        991_003,
        "Deer",
        1,
        Point {
            x: fox_position.x + 4,
            y: fox_position.y + 1,
        },
        WorldEntityDisposition::Neutral,
        true,
    );

    let mut recalled = false;
    for _ in 0..2_000 {
        let packets = session.tick();
        let after = entity_position(&session, 991_001);
        if after != opposing_monster_position {
            assert!(
                (after.x - fox_position.x).abs().max((after.y - fox_position.y).abs()) <= 1,
                "qualified opposing monster should land near the Fox: after={after:?}, fox={fox_position:?}"
            );
            assert!(
                packets.iter().any(|packet| {
                    matches!(
                        packet,
                        ServerPacket::ObjectTeleportOut { object_id, .. }
                            | ServerPacket::ObjectTeleportIn { object_id, .. }
                            if *object_id == 991_001
                    )
                }),
                "successful Monster recall must advertise the actual target object"
            );
            recalled = true;
            break;
        }
    }
    assert!(
        recalled,
        "bounded deterministic Fox ring should recall the opposing Monster"
    );
}
#[test]
fn recall_never_targets_remote_player_mirror() {
    let mut config = SimulationConfig::default();
    let remote_position = Point {
        x: config.spawn.x + 14,
        y: config.spawn.y,
    };
    config.visible_players.push(VisiblePlayerRecord {
        object_id: 992_001,
        name: "RemotePlayer".to_string(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 1,
        armour_shape: None,
        weapon_shape: None,
        position: remote_position.clone(),
        direction: MirDirection::Left,
    });

    let mut session = start_session(config);
    let player = self_player(&session);
    let fox_position = Point {
        x: player.x + 10,
        y: player.y,
    };
    add_fox_ring(&mut session, fox_position);

    let before_player = Point {
        x: player.x,
        y: player.y,
    };
    let mut player_recalled = false;
    for _ in 0..2_000 {
        let packets = session.tick();
        assert_eq!(
            entity_position(&session, 992_001),
            remote_position,
            "RemotePlayer is only a mirror and must never be mutated by local recall"
        );
        assert!(!packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectTeleportOut { object_id, .. }
                    | ServerPacket::ObjectTeleportIn { object_id, .. }
                    if *object_id == 992_001
            )
        }));

        if entity_position(&session, player.object_id) != before_player {
            player_recalled = true;
            break;
        }
    }
    assert!(
        player_recalled,
        "the local SelfPlayer should remain the only supported player recall target"
    );
}
