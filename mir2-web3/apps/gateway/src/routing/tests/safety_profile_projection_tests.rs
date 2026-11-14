//! Actual shared movement and metadata publication, with an explicit open
//! collision fixture so this regression tests safety authority rather than terrain.
use super::*;

#[test]
fn gateway_fast_walk_keeps_manifest_safety_after_stale_profile_projection() {
    let mut config = GatewayConfig::default();
    config.spawn = Point { x: 288, y: 626 };
    config.map_collision.map_width = 1024;
    config.map_collision.map_height = 1024;
    config.map_collision.play_bounds = mir2_game_data::MapBounds {
        min_x: 270,
        max_x: 310,
        min_y: 600,
        max_y: 650,
    };
    config.map_collision.region_bounds = config.map_collision.play_bounds.clone();
    config.map_collision.blocked_cells.clear();
    config.map_collision.doors.clear();
    config.visible_monsters.clear();
    config.visible_players.clear();
    config.visible_npcs.clear();
    config.map_transfers.clear();
    let mut factory = SharedInProcessZoneRuntimeFactory::new();
    factory.autonomous_ticks_by_default = false;
    let zone_id = ZoneId::new("manifest-safety-regression");
    let resources = factory.resources_for_zone(&zone_id);
    assert!(resources
        .zone_state
        .lock()
        .unwrap()
        .zone_manager
        .install_empty_zone(ZoneRuntime::new_with_collision(
            ZoneKey::for_map("0"),
            ZoneCollision::unbounded()
        ),));
    let mut handle = factory.create_runtime(config, &zone_id);
    start_new_runtime_handle(&mut handle, "safety-projection", "SafetyOwner");
    let runtime = handle
        .as_any_mut()
        .downcast_mut::<SharedInProcessZoneSessionRuntime>()
        .unwrap();
    let before = runtime.world_snapshot();
    let identity = runtime.inner.active_identity().unwrap();
    let initial_safety = runtime.movement_ingress.player_safety(&identity).unwrap().unwrap();
    assert!(initial_safety.in_safe_zone);
    let self_entity = before
        .entities
        .iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .unwrap();
    assert_eq!((self_entity.x, self_entity.y), (288, 626));
    assert!(before.in_safe_zone);
    let private_tick = runtime.inner.world_snapshot().tick;
    let action = runtime
        .movement_ingress
        .try_execute(ClientPacket::Walk {
            direction: MirDirection::Down,
        })
        .unwrap()
        .unwrap();
    assert!(action.packets.iter().any(|packet| matches!(packet, ServerPacket::UserLocation { location } if location.position == Point { x: 288, y: 627 })));
    assert_eq!(
        runtime.inner.world_snapshot().tick,
        private_tick,
        "fast movement cannot tick the personal world"
    );
    assert!(
        runtime.inner.world_snapshot().in_safe_zone,
        "the real personal mirror still retains its earlier safe position"
    );
    assert!(!runtime.world_snapshot().in_safe_zone);
    assert!(!runtime.movement_ingress.player_safety(&identity).unwrap().unwrap().in_safe_zone);
    let mut wrong_identity = identity.clone();
    wrong_identity.character_index += 1;
    assert!(runtime.movement_ingress.player_safety(&wrong_identity).unwrap().is_none());
    // This is the Gateway's ordinary metadata tail, intentionally before a private
    // movement drain. It must not write its stale personal safe flag back into Zone.
    runtime.sync_zone_snapshot_preserving_owner_vitals();
    assert!(!runtime.world_snapshot().in_safe_zone);
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::SwitchGroup {
            allow_group: true,
        }))
        .unwrap();
    let after = runtime.world_snapshot();
    assert!(!after.in_safe_zone);
    let self_entity = after
        .entities
        .iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .unwrap();
    assert_eq!((self_entity.x, self_entity.y), (288, 627));
}

fn safety_gateway_fixture(enrolled: bool) -> (GatewaySession, Arc<Mutex<SharedInProcessZoneState>>) {
    use crate::companion_relay::{CharacterBinding, CompanionPolicy, Enrollment};
    let mut config = GatewayConfig::default();
    config.spawn = Point { x: 288, y: 626 };
    config.visible_monsters.clear();
    config.visible_players.clear();
    config.visible_npcs.clear();
    config.map_transfers.clear();
    config.map_hazards.clear();
    config.save_recovery_dir = None;
    {
        let mut accounts = config.account_store.lock().unwrap();
        let save = accounts.accounts.get_mut("demo").unwrap().saves.get_mut(&0).unwrap();
        save.position = config.spawn.clone();
        save.map_file_name = "0".into();
    }
    let zone_id = ZoneId::new(if enrolled { "safety-enrolled" } else { "safety-ordinary" });
    let mut factory = SharedInProcessZoneRuntimeFactory::new();
    factory.autonomous_ticks_by_default = false;
    let factory = Arc::new(factory);
    let state = factory.resources_for_zone(&zone_id).zone_state;
    assert!(state.lock().unwrap().zone_manager.install_empty_zone(
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded())
    ));
    let registry = ZoneRegistry::new(zone_id, factory as SharedZoneRuntimeFactory);
    registry.configure_companion_owner_fences(&CompanionPolicy {
        schema_version: 1,
        realm_id: "safety-trial".into(), world_epoch: "safety-world".into(),
        enrollments: vec![Enrollment {
            resident_id: "safety-alo".into(), archive_id: "safety-archive".into(), display_name: "阿洛".into(),
            owner: CharacterBinding { account_id: "safety-owner".into(), character_index: 0 },
            actor: CharacterBinding { account_id: if enrolled { "demo" } else { "another-actor" }.into(), character_index: 0 },
        }],
    }).unwrap();
    let mut session = GatewaySession::try_new_with_zone_registry(config, &registry).unwrap();
    session.try_handle_packet(ClientPacket::Login { account_id: "demo".into(), password: "demo".into() }).unwrap();
    session.try_handle_packet(ClientPacket::StartGame { character_index: 0 }).unwrap();
    (session, state)
}

fn note_actual_snapshot(session: &GatewaySession) {
    let snapshot = session.world_snapshot();
    let stamp = session.companion_safety_snapshot_stamp(Some(&snapshot)).unwrap();
    session.note_companion_safety_snapshot_sent(stamp).unwrap();
}

#[test]
fn companion_safety_changed_only_after_crossing_and_returning_to_safe_zone() {
    let (session, state) = safety_gateway_fixture(true);
    assert!(session.world_snapshot().in_safe_zone);
    assert!(session.companion_safety_snapshot_pending().unwrap(), "no successful baseline yet");
    note_actual_snapshot(&session);
    assert!(!session.companion_safety_snapshot_pending().unwrap());
    let before = session.world_snapshot();
    // This is the actual fast movement ingress: the personal mirror never
    // executes a movement or a Tick just to detect a boundary crossing.
    let result = session.zone_movement_ingress().unwrap().try_execute(ClientPacket::Walk { direction: MirDirection::Down }).unwrap().unwrap();
    assert!(result.packets.iter().any(|packet| matches!(packet, ServerPacket::UserLocation { location } if location.position == Point { x: 288, y: 627 })));
    assert!(!session.world_snapshot().in_safe_zone);
    assert!(session.companion_safety_snapshot_pending().unwrap());
    // A snapshot captured before the committed step may complete sending
    // afterward. Recording that actual old flag must not eat the new change.
    let old_stamp = session.companion_safety_snapshot_stamp(Some(&before)).unwrap();
    session.note_companion_safety_snapshot_sent(old_stamp).unwrap();
    assert!(session.companion_safety_snapshot_pending().unwrap());
    note_actual_snapshot(&session);
    assert!(!session.companion_safety_snapshot_pending().unwrap(), "unchanged idle ticks stay packet-only");
    let identity = session.active_identity().unwrap();
    let key = ZonePresenceKey::from_identity(&identity);
    let mut state = state.lock().unwrap();
    let id = state.zone_sessions[&key].clone();
    let now = SharedInProcessZoneSessionRuntime::zone_now_ms() + 5_000;
    let mut out = state.zone_manager.handle(ZoneCommand::Walk { session_id: id.clone(), direction: MirDirection::Up, seq: 2, now_ms: now });
    out.extend(state.zone_manager.handle(ZoneCommand::TickPlayerMovement { session_id: id, now_ms: now }));
    state.dispatch_zone_outbounds(out, Some(&key));
    drop(state);
    let returned = session.world_snapshot();
    assert!(returned.in_safe_zone);
    assert!(returned.entities.iter().any(|entity| entity.kind == WorldEntityKind::SelfPlayer && entity.x == 288 && entity.y == 626));
    assert!(session.companion_safety_snapshot_pending().unwrap());
    note_actual_snapshot(&session);
    assert!(!session.companion_safety_snapshot_pending().unwrap());
}

#[test]
fn companion_safety_unsent_snapshot_never_consumes_a_pending_boundary_change() {
    let (session, _) = safety_gateway_fixture(true);
    let snapshot = session.world_snapshot();
    let _unsent = session.companion_safety_snapshot_stamp(Some(&snapshot)).unwrap();
    assert!(session.companion_safety_snapshot_pending().unwrap());
    assert!(session.companion_safety_snapshot_pending().unwrap(), "building or a failed send is not a delivered baseline");
    note_actual_snapshot(&session);
    assert!(!session.companion_safety_snapshot_pending().unwrap());
}

#[test]
fn companion_safety_ordinary_fast_path_does_not_even_read_zone_state() {
    let (session, state) = safety_gateway_fixture(false);
    let before = session.world_snapshot();
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _locked = state.lock().unwrap();
        panic!("controlled Zone poison proves the ordinary path never queries it");
    }));
    assert!(state.is_poisoned());
    assert!(!session.companion_safety_snapshot_pending().unwrap());
    assert!(session.companion_safety_snapshot_stamp(Some(&before)).unwrap().is_none());
    state.clear_poison();
}

#[test]
fn companion_safety_logout_and_new_admission_cannot_reuse_sent_baseline() {
    let (mut session, _) = safety_gateway_fixture(true);
    note_actual_snapshot(&session);
    assert!(!session.companion_safety_snapshot_pending().unwrap());
    session.try_handle_packet(ClientPacket::LogOut).unwrap();
    assert!(!session.companion_safety_snapshot_pending().unwrap());
    session.try_handle_packet(ClientPacket::StartGame { character_index: 0 }).unwrap();
    assert!(session.companion_safety_snapshot_pending().unwrap());
    note_actual_snapshot(&session);
    assert!(!session.companion_safety_snapshot_pending().unwrap());
}
