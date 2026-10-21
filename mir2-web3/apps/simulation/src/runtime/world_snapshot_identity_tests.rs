//! The server supplies map identity from the active World, never a filename
//! heuristic or a client's previous connection/map observation.
use super::*;
use crate::SimulationSession;

fn login(config: SimulationConfig) -> SimulationSession {
    let mut session = SimulationSession::new(config);
    let packets = session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    session
}

fn start(mut session: SimulationSession) -> SimulationSession {
    let packets = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    session
}

#[test]
fn world_snapshot_identity_without_selected_character_never_grants_map() {
    let mut config = SimulationConfig::default();
    config.map.map_index = 987;
    let session = SimulationSession::new(config.clone());
    assert_eq!(session.world_snapshot().map_index, None);
    assert_eq!(
        serde_json::to_value(session.world_snapshot()).unwrap()["mapIndex"],
        serde_json::Value::Null
    );
    let session = login(config);
    assert!(session
        .app
        .world()
        .resource::<SessionResource>()
        .selected_character
        .is_none());
    assert_eq!(session.world_snapshot().map_index, None);
}

#[test]
fn world_snapshot_identity_normal_login_start_preserves_zero_map_index() {
    let mut config = SimulationConfig::default();
    config.map.map_index = 0;
    let mut session = start(login(config));
    let current = session
        .app
        .world()
        .resource::<MapRuntimeResource>()
        .current_map
        .map_index;
    assert_eq!(current, 0);
    assert_eq!(session.world_snapshot().map_index, Some(0));
    assert_eq!(
        serde_json::to_value(session.world_snapshot()).unwrap()["mapIndex"],
        serde_json::json!(0)
    );
    session.handle_packet(ClientPacket::LogOut);
    assert!(session
        .app
        .world()
        .resource::<SessionResource>()
        .selected_character
        .is_none());
    assert_eq!(session.world_snapshot().map_index, None);
}

#[test]
fn world_snapshot_identity_tracks_real_current_map_after_ordinary_entrance_walk() {
    let mut config = SimulationConfig::default();
    config.monster_spawn_source = MonsterSpawnSource::CrystalWorld;
    assert!(crate::config::apply_crystal_map_metadata(&mut config.map));
    let mut session = start(login(config));
    let before = session.world_snapshot().map_index.unwrap();
    // Fixture placement at the real OmaCave entrance. The transfer itself is
    // the ordinary Walk intent and existing imported entrance validation.
    session.force_authoritative_player_transform(Point { x: 146, y: 33 }, MirDirection::Down);
    let packets = session.handle_packet(ClientPacket::Walk {
        direction: MirDirection::Right,
    });
    let info = packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::MapInformation { info } => Some(info),
            _ => None,
        })
        .expect("ordinary entrance Walk must supply destination MapInformation");
    assert_eq!(info.file_name, "D001");
    assert_ne!(info.map_index, before);
    let snapshot = session.world_snapshot();
    assert_eq!(snapshot.map_index, Some(info.map_index));
    assert_eq!(
        snapshot.map_file_name.as_deref(),
        Some(info.file_name.as_str())
    );
    assert_eq!(
        snapshot.map_index,
        Some(
            session
                .app
                .world()
                .resource::<MapRuntimeResource>()
                .current_map
                .map_index
        )
    );
}

#[test]
fn world_snapshot_identity_old_json_without_map_index_defaults_to_none() {
    let session = start(login(SimulationConfig::default()));
    let mut old_json = serde_json::to_value(session.world_snapshot()).unwrap();
    old_json.as_object_mut().unwrap().remove("mapIndex");
    let restored: WorldSnapshot = serde_json::from_value(old_json).unwrap();
    assert_eq!(restored.map_index, None);
    assert!(
        restored.map_file_name.is_some(),
        "a filename cannot invent map identity"
    );
}

#[test]
fn world_snapshot_identity_client_view_preserves_authoritative_map_index() {
    let mut config = SimulationConfig::default();
    config.map.map_index = 0;
    let session = start(login(config));
    let snapshot = session.world_snapshot();
    let view = serde_json::to_value(snapshot.client_view()).unwrap();
    assert_eq!(view["mapIndex"], serde_json::json!(0));
    let decoded: WorldSnapshot = serde_json::from_value(view).unwrap();
    assert_eq!(decoded.map_index, snapshot.map_index);
    assert_eq!(decoded.map_file_name, snapshot.map_file_name);
    assert_eq!(decoded.player_object_id, snapshot.player_object_id);
}
