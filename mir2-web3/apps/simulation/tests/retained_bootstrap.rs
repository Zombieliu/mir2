use mir2_protocol::{ClientPacket, MirDirection, Point, ServerPacket};
use mir2_simulation::{
    validate_production_player_command, InProcessWorldRuntime, SimulationConfig, WorldCommand,
    WorldRuntime,
};

fn active_runtime() -> InProcessWorldRuntime {
    let mut runtime = InProcessWorldRuntime::new(SimulationConfig::default());
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        }))
        .unwrap();
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: 0,
        }))
        .unwrap();
    runtime
}

#[test]
fn retained_bootstrap_replays_catalog_without_reloading_or_mutating_character() {
    let mut runtime = active_runtime();
    runtime
        .execute(WorldCommand::ApplyHandoffTransform {
            position: Point { x: 3, y: 4 },
            direction: MirDirection::Left,
            hp: Some(1),
            mp: Some(1),
        })
        .unwrap();
    let before = serde_json::to_value(runtime.world_snapshot()).unwrap();
    let checkpoint = serde_json::to_value(runtime.active_character_checkpoint()).unwrap();
    let identity = runtime.active_identity();
    let packets = runtime
        .execute(WorldCommand::ReplayRetainedStartGameBootstrap { character_index: 0 })
        .unwrap();
    assert!(matches!(
        packets.first(),
        Some(ServerPacket::StartGame { result: 4, .. })
    ));
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::MapInformation { .. })));
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::UserInformation { .. })));
    assert_eq!(
        packets
            .iter()
            .filter(|p| matches!(p, ServerPacket::GameShopInfo { .. }))
            .count(),
        105
    );
    assert_eq!(
        serde_json::to_value(runtime.world_snapshot()).unwrap(),
        before
    );
    assert_eq!(
        serde_json::to_value(runtime.active_character_checkpoint()).unwrap(),
        checkpoint
    );
    assert_eq!(runtime.active_identity(), identity);
    assert!(
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index: 0
            }))
            .unwrap()
            .is_empty(),
        "public in-game StartGame retains Crystal stage rejection"
    );
}

#[test]
fn retained_bootstrap_wrong_character_does_not_replay_or_change_identity() {
    let mut runtime = active_runtime();
    let before = serde_json::to_value(runtime.world_snapshot()).unwrap();
    let identity = runtime.active_identity();
    let packets = runtime
        .execute(WorldCommand::ReplayRetainedStartGameBootstrap { character_index: 1 })
        .unwrap();
    assert!(matches!(
        packets.as_slice(),
        [ServerPacket::StartGame { result: 2, .. }]
    ));
    assert_eq!(
        serde_json::to_value(runtime.world_snapshot()).unwrap(),
        before
    );
    assert_eq!(runtime.active_identity(), identity);
}

#[test]
fn retained_bootstrap_requires_account_and_is_never_a_production_player_command() {
    let command = WorldCommand::ReplayRetainedStartGameBootstrap { character_index: 0 };
    for authenticated in [false, true] {
        assert!(validate_production_player_command(authenticated, &command).is_err());
    }
    let mut runtime = InProcessWorldRuntime::new(SimulationConfig::default());
    assert!(matches!(
        runtime.execute(command).unwrap().as_slice(),
        [ServerPacket::StartGame { result: 1, .. }]
    ));
}
