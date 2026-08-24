use mir2_protocol::{
    ChatType, ClientPacket, MirClass, MirDirection, MirGender, Point, ServerPacket,
};
use mir2_simulation::{
    AccountRecord, CharacterRecord, CharacterSaveRecord, SessionId, SimulationConfig,
    SimulationSession, WorldEntityKind, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey,
    ZoneOutbound, ZoneRuntime,
};

const PENAL_HINT: &str = "A Mysterious evil force seems to be pushing you back.";
const DOGYO_HINT: &str = "Doors are locked until you reach level 50!";

#[derive(Clone)]
struct GateCase {
    map: &'static str,
    source: Point,
    denied_level: u16,
    allowed_level: u16,
    denied_pk_points: i32,
    allowed_pk_points: i32,
    hint: &'static str,
    destination_map: &'static str,
    destination: Point,
}

const GATE_CASES: [GateCase; 6] = [
    GateCase {
        map: "3",
        source: Point { x: 861, y: 686 },
        denied_level: 1,
        allowed_level: 1,
        denied_pk_points: 199,
        allowed_pk_points: 200,
        hint: PENAL_HINT,
        destination_map: "D1801",
        destination: Point { x: 128, y: 171 },
    },
    GateCase {
        map: "3",
        source: Point { x: 862, y: 687 },
        denied_level: 1,
        allowed_level: 1,
        denied_pk_points: 199,
        allowed_pk_points: 200,
        hint: PENAL_HINT,
        destination_map: "D1801",
        destination: Point { x: 128, y: 171 },
    },
    GateCase {
        map: "DogYoArena2",
        source: Point { x: 117, y: 26 },
        denied_level: 49,
        allowed_level: 50,
        denied_pk_points: 0,
        allowed_pk_points: 0,
        hint: DOGYO_HINT,
        destination_map: "DogYoHyun",
        destination: Point { x: 21, y: 765 },
    },
    GateCase {
        map: "DogYoArena2",
        source: Point { x: 118, y: 27 },
        denied_level: 49,
        allowed_level: 50,
        denied_pk_points: 0,
        allowed_pk_points: 0,
        hint: DOGYO_HINT,
        destination_map: "DogYoHyun",
        destination: Point { x: 21, y: 765 },
    },
    GateCase {
        map: "DogYoArena2",
        source: Point { x: 119, y: 28 },
        denied_level: 49,
        allowed_level: 50,
        denied_pk_points: 0,
        allowed_pk_points: 0,
        hint: DOGYO_HINT,
        destination_map: "DogYoHyun",
        destination: Point { x: 21, y: 765 },
    },
    GateCase {
        map: "DogYoArena2",
        source: Point { x: 119, y: 29 },
        denied_level: 49,
        allowed_level: 50,
        denied_pk_points: 0,
        allowed_pk_points: 0,
        hint: DOGYO_HINT,
        destination_map: "DogYoHyun",
        destination: Point { x: 21, y: 765 },
    },
];

fn start_session(case: &GateCase, suffix: &str, level: u16, pk_points: i32) -> SimulationSession {
    let account_id = format!("map-coordinate-{suffix}");
    let character = CharacterRecord {
        index: 0,
        name: format!("Gate{suffix}"),
        level,
        class: MirClass::Warrior,
        gender: MirGender::Male,
    };
    let mut save = CharacterSaveRecord::new(character.clone());
    save.map_file_name = case.map.to_string();
    save.map_title = case.map.to_string();
    save.position = Point {
        x: case.source.x - 1,
        y: case.source.y - 1,
    };
    save.direction = MirDirection::DownRight;
    save.pk_points = pk_points;

    let config = SimulationConfig::default().with_crystal_world_runtime();
    let mut account = AccountRecord::empty();
    account.characters.push(character);
    account.saves.insert(0, save);
    config
        .account_store
        .lock()
        .expect("account store mutex")
        .accounts
        .insert(account_id.clone(), account);

    let mut session = SimulationSession::new(config);
    let login = session.handle_packet(ClientPacket::Login {
        account_id,
        password: "demo".to_string(),
    });
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let start = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(start
        .iter()
        .any(|packet| matches!(packet, ServerPacket::MapInformation { info } if info.file_name == case.map)));
    session
}

fn self_position(session: &SimulationSession) -> Point {
    let snapshot = session.world_snapshot();
    let player = snapshot
        .entities
        .iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .expect("self player");
    Point {
        x: player.x,
        y: player.y,
    }
}

fn has_hint(packets: &[ServerPacket], expected: &str) -> bool {
    packets.iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::Chat { message, chat_type: ChatType::Hint }
                if message == expected
        )
    })
}

#[test]
fn personal_session_enforces_all_six_authoritative_map_coordinate_gates() {
    for (index, case) in GATE_CASES.iter().enumerate() {
        let mut denied = start_session(
            case,
            &format!("{index}-denied"),
            case.denied_level,
            case.denied_pk_points,
        );
        let denied_packets = denied.handle_packet(ClientPacket::Walk {
            direction: MirDirection::DownRight,
        });
        assert_eq!(
            denied.world_snapshot().map_file_name.as_deref(),
            Some(case.map)
        );
        assert_eq!(self_position(&denied), case.source.clone());
        assert!(has_hint(&denied_packets, case.hint), "{denied_packets:?}");
        assert!(!denied_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::MapInformation { info } if info.file_name == case.destination_map
        )));

        let mut allowed = start_session(
            case,
            &format!("{index}-allowed"),
            case.allowed_level,
            case.allowed_pk_points,
        );
        let allowed_packets = allowed.handle_packet(ClientPacket::Walk {
            direction: MirDirection::DownRight,
        });
        assert!(
            !has_hint(&allowed_packets, case.hint),
            "{allowed_packets:?}"
        );
        assert!(
            allowed_packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::MapInformation { info } if info.file_name == case.destination_map
            )),
            "{allowed_packets:?}"
        );
        assert_eq!(
            allowed.world_snapshot().map_file_name.as_deref(),
            Some(case.destination_map)
        );
        assert_eq!(self_position(&allowed), case.destination.clone());
    }
}

fn zone_join(case: &GateCase, suffix: &str, level: u16, pk_points: i32) -> ZoneJoin {
    let mut chat_profile = mir2_simulation::ZoneChatProfile::default();
    chat_profile.pk_points = pk_points;
    ZoneJoin {
        session_id: SessionId::new(format!("zone-{suffix}")),
        account_id: format!("zone-{suffix}-account"),
        character_index: 0,
        object_id: 10_000,
        name: format!("ZoneGate{suffix}"),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level,
        hp: 100,
        max_hp: 100,
        mp: 100,
        map_file_name: case.map.to_string(),
        position: Point {
            x: case.source.x - 1,
            y: case.source.y - 1,
        },
        direction: MirDirection::DownRight,
        chat_profile,
        combat_stats: Default::default(),
    }
}

fn zone_walk(zone: &mut ZoneRuntime, session_id: SessionId) -> Vec<ZoneOutbound> {
    let mut outbounds = zone.handle(ZoneCommand::Walk {
        session_id: session_id.clone(),
        direction: MirDirection::DownRight,
        seq: 1,
        now_ms: 1,
    });
    outbounds.extend(zone.handle(ZoneCommand::TickPlayerMovement {
        session_id,
        now_ms: 1,
    }));
    outbounds
}

fn zone_has_hint(outbounds: &[ZoneOutbound], session_id: &SessionId, expected: &str) -> bool {
    outbounds.iter().any(|outbound| {
        matches!(
            outbound,
            ZoneOutbound::ToSession { session_id: owner, packets }
                if owner == session_id && has_hint(packets, expected)
        )
    })
}

#[test]
fn shared_zone_uses_join_level_and_pk_points_for_all_six_gates() {
    for (index, case) in GATE_CASES.iter().enumerate() {
        let denied_join = zone_join(
            case,
            &format!("{index}-denied"),
            case.denied_level,
            case.denied_pk_points,
        );
        let denied_id = denied_join.session_id.clone();
        let mut denied_zone =
            ZoneRuntime::new_with_collision(ZoneKey::for_map(case.map), ZoneCollision::unbounded());
        denied_zone.handle(ZoneCommand::Join(denied_join));
        let denied_outbounds = zone_walk(&mut denied_zone, denied_id.clone());
        assert_eq!(
            denied_zone.player_position(&denied_id),
            Some(case.source.clone())
        );
        assert!(zone_has_hint(&denied_outbounds, &denied_id, case.hint));

        let allowed_join = zone_join(
            case,
            &format!("{index}-allowed"),
            case.allowed_level,
            case.allowed_pk_points,
        );
        let allowed_id = allowed_join.session_id.clone();
        let mut allowed_zone =
            ZoneRuntime::new_with_collision(ZoneKey::for_map(case.map), ZoneCollision::unbounded());
        allowed_zone.handle(ZoneCommand::Join(allowed_join));
        let allowed_outbounds = zone_walk(&mut allowed_zone, allowed_id.clone());
        assert_eq!(
            allowed_zone.player_position(&allowed_id),
            Some(case.source.clone())
        );
        assert!(!zone_has_hint(&allowed_outbounds, &allowed_id, case.hint));
    }
}
