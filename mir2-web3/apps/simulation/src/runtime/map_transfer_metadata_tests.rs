use super::*;
use crate::{SimulationConfig, SimulationSession};
use mir2_protocol::ClientPacket;

fn session_at_entrance(position: Point) -> SimulationSession {
    let mut config = SimulationConfig::default();
    config.monster_spawn_source = crate::config::MonsterSpawnSource::CrystalWorld;
    assert!(crate::config::apply_crystal_map_metadata(&mut config.map));
    let mut session = SimulationSession::new(config);
    let login = session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    assert!(login
        .iter()
        .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    // Only fixture placement: the transfer below is an ordinary Walk through
    // an actual imported entrance, never a debug or explicit transfer command.
    session.force_authoritative_player_transform(position, MirDirection::Down);
    session
}

fn assert_destination(
    session: &SimulationSession,
    packets: &[ServerPacket],
    file: &str,
    index: i32,
    image: u16,
) {
    let info = packets
        .iter()
        .find_map(|p| match p {
            ServerPacket::MapInformation { info } => Some(info),
            _ => None,
        })
        .expect("ordinary entrance Walk must emit MapInformation");
    assert_eq!(info.file_name, file);
    assert_eq!(info.map_index, index);
    assert_eq!(info.mini_map, image);
    assert_eq!(info.big_map, image);
    let mut expected = info.clone();
    assert!(crate::config::apply_crystal_map_metadata(&mut expected));
    assert_eq!(
        serde_json::to_value(info).unwrap(),
        serde_json::to_value(&expected).unwrap(),
        "all destination metadata must match the manifest, including light/music/flags"
    );
    let current = &session
        .app
        .world()
        .resource::<MapRuntimeResource>()
        .current_map;
    assert_eq!(
        serde_json::to_value(current).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
    let snapshot = session.world_snapshot();
    assert_eq!(snapshot.map_file_name.as_deref(), Some(file));
    assert_eq!(snapshot.map_title.as_deref(), Some(expected.title.as_str()));
}

#[test]
fn map_transfer_metadata_actual_dead_mine_entrance_walk_roundtrip() {
    let mut session = session_at_entrance(Point { x: 663, y: 214 });
    let enter = session.handle_packet(ClientPacket::Walk {
        direction: MirDirection::Down,
    });
    assert_destination(&session, &enter, "D401", 47, 8);
    // This entrance lands at24,181, immediately north of the real exit24,182.
    // Let ordinary movement cooldown advance before the return Walk.
    for _ in 0..3 {
        session.tick();
    }
    let leave = session.handle_packet(ClientPacket::Walk {
        direction: MirDirection::Down,
    });
    assert_destination(&session, &leave, "0", 1, 101);
}

#[test]
fn map_transfer_metadata_actual_oma_cave_entrance_walk_roundtrip() {
    let mut session = session_at_entrance(Point { x: 146, y: 33 });
    let enter = session.handle_packet(ClientPacket::Walk {
        direction: MirDirection::Right,
    });
    assert_destination(&session, &enter, "D001", 39, 1);
    session.force_authoritative_player_transform(Point { x: 148, y: 364 }, MirDirection::Down);
    let leave = session.handle_packet(ClientPacket::Walk {
        direction: MirDirection::Down,
    });
    assert_destination(&session, &leave, "0", 1, 101);
}
