use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{InProcessWorldRuntime, SimulationConfig, WorldCommand, WorldRuntime};

fn enter_demo(runtime: &mut InProcessWorldRuntime) {
    let login = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        }))
        .expect("demo login should execute");
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: 0,
        }))
        .expect("demo character should enter the world");
}

#[test]
fn stale_save_reaches_world_runtime_and_blocks_leave_success() {
    let config = SimulationConfig::default();
    let mut current = InProcessWorldRuntime::new(config.clone());
    let mut stale = InProcessWorldRuntime::new(config);
    enter_demo(&mut current);
    enter_demo(&mut stale);

    current
        .save_active_character()
        .expect("current session should advance the durable revision");

    let stale_error = stale
        .save_active_character()
        .expect_err("stale save must reach the caller");
    assert!(stale_error.contains("stale full character save rejected"));

    let before = stale.world_snapshot();
    for packet in [ClientPacket::LogOut, ClientPacket::Disconnect] {
        let error = stale
            .execute(WorldCommand::ClientPacket(packet))
            .expect_err("leave must fail before emitting a success packet");
        assert!(error.contains("stale full character save rejected"));
        assert!(
            stale.active_identity().is_some(),
            "failed leave must retain the active character"
        );
        let after = stale.world_snapshot();
        assert_eq!(after.map_file_name, before.map_file_name);
        assert_eq!(after.player_object_id, before.player_object_id);
        assert_eq!(
            after
                .entities
                .iter()
                .find(|entity| entity.kind == mir2_simulation::WorldEntityKind::SelfPlayer)
                .map(|entity| (entity.x, entity.y)),
            before
                .entities
                .iter()
                .find(|entity| entity.kind == mir2_simulation::WorldEntityKind::SelfPlayer)
                .map(|entity| (entity.x, entity.y))
        );
    }
}

#[test]
fn successful_save_still_allows_logout_success() {
    let mut runtime = InProcessWorldRuntime::new(SimulationConfig::default());
    enter_demo(&mut runtime);

    let packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .expect("successful durable save should allow logout");

    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
    assert!(runtime.active_identity().is_none());
}
