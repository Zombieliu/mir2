//! A cheaper observer projection must retain the original Zone and owner route.
use super::*;
use super::super::{RpcZoneOwnerCommandClient, SharedZoneOwnerRpcTransport, ZoneOwnerCommandClient};
use mir2_simulation::WorldSnapshot;

fn public_scene(snapshot: &WorldSnapshot) -> serde_json::Value {
    crate::spectator::sanitize_world(snapshot, usize::MAX).unwrap()
}

#[test]
fn spectator_scene_uses_zone_position_with_stale_personal_mirror() {
    let mut config = GatewayConfig::default();
    config.spawn = Point { x: 288, y: 626 };
    config.map_collision.map_width = 1024;
    config.map_collision.map_height = 1024;
    config.map_collision.play_bounds = mir2_game_data::MapBounds {
        min_x: 270, max_x: 310, min_y: 600, max_y: 650,
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
    let zone_id = ZoneId::new("spectator-scene-authority");
    let resources = factory.resources_for_zone(&zone_id);
    assert!(resources.zone_state.lock().unwrap().zone_manager.install_empty_zone(
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded()),
    ));
    let mut owner = factory.create_runtime(config.clone(), &zone_id);
    start_new_runtime_handle(&mut owner, "scene-owner", "SceneOwner");
    config.spawn = Point { x: 290, y: 626 };
    let mut peer = factory.create_runtime(config, &zone_id);
    start_new_runtime_handle(&mut peer, "scene-peer", "ScenePeer");
    let runtime = owner.as_any_mut().downcast_mut::<SharedInProcessZoneSessionRuntime>().unwrap();
    let private_tick = runtime.inner.world_snapshot().tick;
    let action = runtime.movement_ingress.try_execute(ClientPacket::Walk {
        direction: MirDirection::Down,
    }).unwrap().unwrap();
    assert!(action.packets.iter().any(|packet| matches!(packet,
        ServerPacket::UserLocation { location } if location.position == Point { x: 288, y: 627 }
    )));
    let personal = runtime.inner.world_snapshot();
    let personal_self = personal.entities.iter().find(|e| e.kind == WorldEntityKind::SelfPlayer).unwrap();
    assert_eq!((personal_self.x, personal_self.y), (288, 626));
    let checkpoint = serde_json::to_value(runtime.active_character_checkpoint()).unwrap();
    let full = runtime.world_snapshot();
    let scene = runtime.spectator_world_snapshot();
    assert_eq!(public_scene(&full), public_scene(&scene));
    let scene_self = scene.entities.iter().find(|e| e.kind == WorldEntityKind::SelfPlayer).unwrap();
    assert_eq!((scene_self.x, scene_self.y), (288, 627));
    assert!(!scene.in_safe_zone);
    assert!(scene.entities.iter().any(|e| e.kind == WorldEntityKind::Player));
    assert_eq!(scene.player_hp, full.player_hp);
    assert_eq!(runtime.inner.world_snapshot().tick, private_tick);
    assert_eq!(serde_json::to_value(runtime.active_character_checkpoint()).unwrap(), checkpoint);
}

#[test]
fn spectator_scene_rpc_default_reads_remote_owner_instead_of_local_shadow() {
    let mut remote: ZoneRuntimeHandle = Box::new(InProcessWorldRuntime::new(GatewayConfig::default()));
    start_new_runtime_handle(&mut remote, "remote-scene", "RemoteScene");
    let expected = public_scene(&remote.world_snapshot());
    let hosted = Arc::new(HostedZoneOwnerCommandClient::new(remote));
    let rpc = RpcZoneOwnerCommandClient::new(hosted as SharedZoneOwnerRpcTransport);
    let local: ZoneRuntimeHandle = Box::new(InProcessWorldRuntime::new(GatewayConfig::default()));
    assert_ne!(public_scene(&local.world_snapshot()), expected);
    assert_eq!(public_scene(&rpc.spectator_world_snapshot(&local).unwrap()), expected);
}

#[test]
fn spectator_scene_hosted_handoff_rejects_access_after_runtime_taken() {
    let mut remote: ZoneRuntimeHandle = Box::new(InProcessWorldRuntime::new(GatewayConfig::default()));
    start_new_runtime_handle(&mut remote, "hosted-scene", "HostedScene");
    let expected = public_scene(&remote.world_snapshot());
    let hosted = HostedZoneOwnerCommandClient::new(remote);
    assert_eq!(public_scene(&hosted.spectator_world_snapshot().unwrap()), expected);
    let returned = hosted.take_runtime_for_handoff().unwrap();
    assert_eq!(public_scene(&returned.world_snapshot()), expected);
    assert!(hosted.spectator_world_snapshot().unwrap_err().contains("already handed off"));
}
