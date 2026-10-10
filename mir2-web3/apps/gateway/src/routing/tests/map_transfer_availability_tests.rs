//! Normal authenticated adapter paths on a server-configured missing source.
//! Trusted setup is distinct from successful Source Map.Load/population.
use super::*;
use mir2_game_data::MapBounds;
use mir2_simulation::{MapTransferRecord, ZoneCollision, ZoneRuntime};

const MISSING: &str = "missing-portal-source-20261010";
const SOURCE: Point = Point { x: 10, y: 10 };

fn source_runtime(explicit_arena: bool, account: &str) -> SharedInProcessZoneSessionRuntime {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut config = GatewayConfig::default();
    config.map_transfers.push(MapTransferRecord {
        key: "server-configured-terrain-exit".into(),
        from_map_file_name: MISSING.into(),
        from_bounds: MapBounds { min_x: 10, max_x: 10, min_y: 10, max_y: 10 },
        to_map_file_name: "0".into(),
        to_map_title: "Bichon Province".into(),
        to_position: Point { x: 330, y: 270 },
        to_direction: MirDirection::Down,
        conquest_index: 0,
    });
    {
        let mut state = shared.lock().unwrap();
        state.experience_authority = Some(config);
        if explicit_arena {
            assert!(state.zone_manager.install_empty_zone(ZoneRuntime::new_with_collision(
                ZoneKey::for_map(MISSING), ZoneCollision::unbounded(),
            )));
        }
    }
    let mut runtime = shared_session_runtime(shared);
    start_new_runtime(&mut runtime, account, "TerrainSource");
    // Only the trusted fixture places the character; the action under test is
    // the ordinary Walk/Run/Turn adapter, never a client debug teleport packet.
    runtime.execute(WorldCommand::TransferMap {
        key: format!("crystal:{MISSING}:10:10"),
    }).unwrap();
    assert_eq!(runtime.world_snapshot().map_file_name.as_deref(), Some(MISSING));
    assert!(runtime.world_snapshot().map_transfers.iter()
        .any(|transfer| transfer.key == "server-configured-terrain-exit"),
        "fixture must expose its configured entrance independently of transfer admission");
    runtime
}

fn stays_on_missing_source(packet: ClientPacket, account: &str) {
    let mut runtime = source_runtime(false, account);
    let packets = runtime.execute(WorldCommand::ClientPacket(packet)).unwrap();
    assert!(!packets.iter().any(|packet| matches!(packet, ServerPacket::MapInformation { .. })),
        "missing source terrain must not trigger a configured transfer: {packets:?}");
    assert_eq!(runtime.world_snapshot().map_file_name.as_deref(), Some(MISSING));
    let session_id = runtime.current_zone_session_id().unwrap();
    let presence_key = runtime.current_presence_key().unwrap();
    let identity = runtime.inner.active_identity().unwrap();
    {
        let state = runtime.zone_state.lock().unwrap();
        assert_eq!(state.zone_manager.player_transform(&session_id).unwrap().0, SOURCE);
        assert_eq!(state.zone_manager.zone_key_for_session(&session_id).unwrap().map_file_name, MISSING);
    }
    let later = runtime.execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive { time: 100 })).unwrap();
    assert!(!later.iter().any(|packet| matches!(packet, ServerPacket::MapInformation { .. })));
    let logout = runtime.execute(WorldCommand::ClientPacket(ClientPacket::LogOut)).unwrap();
    assert!(logout.iter().any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
    {
        let state = runtime.zone_state.lock().unwrap();
        assert!(state.zone_manager.zone_key_for_session(&session_id).is_none());
        assert!(!state.zone_sessions.contains_key(&presence_key));
        assert!(!state.zone_session_keys.contains_key(&session_id));
    }
    let login = runtime.execute(WorldCommand::ClientPacket(ClientPacket::Login {
        account_id: account.into(), password: account.into(),
    })).unwrap();
    assert!(login.iter().any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let start = runtime.execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
        character_index: identity.character_index,
    })).unwrap();
    assert!(start.iter().any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    let rejoined = runtime.current_zone_session_id().unwrap();
    {
        let state = runtime.zone_state.lock().unwrap();
        assert_eq!(state.zone_sessions.get(&presence_key), Some(&rejoined));
        assert_eq!(state.zone_session_keys.get(&rejoined), Some(&presence_key));
        assert_eq!(state.zone_manager.zone_key_for_session(&rejoined).unwrap().map_file_name, MISSING);
    }
    assert_eq!(runtime.world_snapshot().map_file_name.as_deref(), Some(MISSING),
        "normal save/relogin must retain the rejected transfer's original map");
    assert_eq!(runtime.inner.local_player_position(), Some(SOURCE));
    runtime.execute(WorldCommand::ClientPacket(ClientPacket::LogOut)).unwrap();
}

#[test]
fn transfer_observation_rejects_missing_reverse_session_index() {
    let runtime = source_runtime(true, "terrain-index");
    let session_id = runtime.current_zone_session_id().unwrap();
    runtime.zone_state.lock().unwrap().zone_session_keys.remove(&session_id);
    assert!(runtime.current_zone_transfer_key().is_none());
}

#[test]
fn transfer_observation_rejects_teardown_fence() {
    let runtime = source_runtime(true, "terrain-fence");
    let key = runtime.current_presence_key().unwrap();
    runtime.zone_state.lock().unwrap().teardown_fences.insert(key);
    assert!(runtime.current_zone_transfer_key().is_none());
}

#[test]
fn transfer_observation_rejects_presence_and_cached_map_mismatch() {
    let runtime = source_runtime(true, "terrain-map");
    let key = runtime.current_presence_key().unwrap();
    runtime.zone_state.lock().unwrap().players.get_mut(&key).unwrap().map_file_name = "0".into();
    assert!(runtime.current_zone_transfer_key().is_none());
}

#[test]
fn missing_source_walk_does_not_transfer_or_save_another_map() {
    stays_on_missing_source(ClientPacket::Walk { direction: MirDirection::Right }, "terrain-walk");
}

#[test]
fn missing_source_run_does_not_transfer_or_save_another_map() {
    stays_on_missing_source(ClientPacket::Run { direction: MirDirection::Right }, "terrain-run");
}

#[test]
fn missing_source_turn_does_not_transfer_or_save_another_map() {
    stays_on_missing_source(ClientPacket::Turn { direction: MirDirection::Right }, "terrain-turn");
}

#[test]
fn trusted_same_name_open_arena_keeps_configured_transfer() {
    let mut runtime = source_runtime(true, "terrain-arena");
    let packets = runtime.execute(WorldCommand::ClientPacket(ClientPacket::Turn {
        direction: MirDirection::Right,
    })).unwrap();
    assert!(packets.iter().any(|packet| matches!(packet,
        ServerPacket::MapInformation { info } if info.file_name == "0")));
    assert_eq!(runtime.world_snapshot().map_file_name.as_deref(), Some("0"));
    assert_eq!(runtime.inner.local_player_position(), Some(Point { x: 330, y: 270 }));
    runtime.execute(WorldCommand::ClientPacket(ClientPacket::LogOut)).unwrap();
}
