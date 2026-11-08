use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{
    validate_production_player_command, InProcessWorldRuntime, SimulationConfig, WorldCommand,
    WorldRuntime,
};

fn active_runtime() -> InProcessWorldRuntime {
    let mut runtime = InProcessWorldRuntime::new(SimulationConfig::default());
    let packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        }))
        .unwrap();
    let index = packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::LoginSuccess { characters } => {
                characters.first().map(|entry| entry.index)
            }
            _ => None,
        })
        .unwrap();
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: index,
        }))
        .unwrap();
    runtime
}

fn projection(runtime: &InProcessWorldRuntime, members: Vec<String>) -> WorldCommand {
    let identity = runtime.active_identity().unwrap();
    WorldCommand::SyncPartyView {
        account_id: identity.account_id,
        character_index: identity.character_index,
        allow_group: true,
        members,
    }
}

#[test]
fn shared_party_projection_is_trusted_identity_bound_and_does_not_advance_game_time() {
    let mut runtime = active_runtime();
    let name = runtime.active_identity().unwrap().character_name;
    let members = vec![name, "Companion".to_string()];
    let before_tick = runtime.world_snapshot().tick;
    assert!(
        validate_production_player_command(true, &projection(&runtime, members.clone())).is_err()
    );
    runtime
        .execute(projection(&runtime, members.clone()))
        .unwrap();
    let after = runtime.world_snapshot();
    assert_eq!(after.stage5_systems.group.members, members);
    assert!(after.stage5_systems.group.allow_group);
    assert_eq!(after.tick, before_tick);
    let wrong = WorldCommand::SyncPartyView {
        account_id: "another-account".to_string(),
        character_index: 0,
        allow_group: false,
        members: Vec::new(),
    };
    assert!(runtime.execute(wrong).is_err());
    assert_eq!(
        runtime.world_snapshot().stage5_systems.group.members,
        members
    );
}

#[test]
fn malformed_rosters_and_stale_character_updates_fail_without_mutation() {
    let mut runtime = active_runtime();
    let name = runtime.active_identity().unwrap().character_name;
    for members in [
        vec!["Stranger".to_string()],
        vec![name.clone(), name.to_uppercase()],
        vec![" ".to_string()],
    ] {
        assert!(runtime.execute(projection(&runtime, members)).is_err());
    }
    let stale = projection(&runtime, vec![name]);
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap();
    assert!(runtime.execute(stale).is_err());
}
