use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    InProcessWorldRuntime, SessionId, SimulationConfig, WorldCommand, WorldRuntime, ZoneCommand,
    ZoneJoin, ZoneKey, ZoneRuntime, validate_production_player_command,
};

fn assert_rejected(command: WorldCommand) {
    assert!(
        validate_production_player_command(true, &command).is_err(),
        "command should be rejected on production player path: {command:?}"
    );
}

#[test]
fn unauthenticated_start_game_rejected() {
    let command = WorldCommand::ClientPacket(ClientPacket::StartGame { character_index: 0 });

    assert!(validate_production_player_command(false, &command).is_err());
}

#[test]
fn unauthenticated_new_character_rejected() {
    let command = WorldCommand::ClientPacket(ClientPacket::NewCharacter {
        name: "Blade".to_string(),
        gender: MirGender::Female,
        class: MirClass::Wizard,
    });

    assert!(validate_production_player_command(false, &command).is_err());
}

#[test]
fn unauthenticated_delete_character_rejected() {
    let command = WorldCommand::ClientPacket(ClientPacket::DeleteCharacter { character_index: 0 });

    assert!(validate_production_player_command(false, &command).is_err());
}

#[test]
fn stage5_command_rejected_for_player_path() {
    assert_rejected(WorldCommand::Stage5Command {
        action: "qa.giveItem".to_string(),
        args: vec!["Gold".to_string()],
    });
}

#[test]
fn move_to_rejected_for_player_path() {
    assert_rejected(WorldCommand::MoveTo {
        position: Point { x: 330, y: 270 },
        running: false,
    });
}

#[test]
fn debug_crystal_teleport_rejected_in_prod_path() {
    assert_rejected(WorldCommand::TransferMap {
        key: "crystal:0:330:270".to_string(),
    });
}

#[test]
fn raw_passkey_login_rejected_for_player_path() {
    assert_rejected(WorldCommand::PasskeyLogin {
        account_id: "demo".to_string(),
    });
}

#[test]
fn production_player_wrapper_rejects_before_runtime_execution() {
    let mut runtime = InProcessWorldRuntime::new(SimulationConfig::default());

    let error = runtime
        .execute_production_player_command(
            false,
            WorldCommand::ClientPacket(ClientPacket::StartGame { character_index: 0 }),
        )
        .expect_err("unauthenticated StartGame should not reach runtime");

    assert!(error.contains("authenticated account"));
    assert!(runtime.active_identity().is_none());
}

#[test]
fn passkey_account_can_create_character_and_start_game() {
    let account_id = "sui:0xpasskey-lifecycle";
    let mut runtime = InProcessWorldRuntime::new(SimulationConfig::default());

    let login = runtime
        .execute(WorldCommand::PasskeyLogin {
            account_id: account_id.to_string(),
        })
        .expect("passkey login should succeed");
    assert!(matches!(
        login.as_slice(),
        [ServerPacket::LoginSuccess { characters }] if characters.is_empty()
    ));

    let created = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::NewCharacter {
            name: "PassBlade".to_string(),
            gender: MirGender::Male,
            class: MirClass::Warrior,
        }))
        .expect("authenticated passkey account should create a character");
    let character_index = created
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .expect("new character response should contain its server index");

    let started = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index,
        }))
        .expect("passkey character should start the game");
    assert!(started.iter().any(|packet| matches!(
        packet,
        ServerPacket::StartGame {
            result: 4,
            resolution
        } if *resolution > 0
    )));
    let identity = runtime
        .active_identity()
        .expect("started passkey character should become active");
    assert_eq!(identity.account_id, account_id);
    assert_eq!(identity.character_index, character_index);
}

#[test]
fn each_joined_player_has_unique_object_id() {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map("0"));
    let first = SessionId::new("first");
    let second = SessionId::new("second");

    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: first.clone(),
        account_id: "first-account".to_string(),
        character_index: 0,
        object_id: 7,
        name: "Scout".to_string(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 7,
        hp: 60,
        max_hp: 60,
        mp: 100,
        map_file_name: "0".to_string(),
        position: Point { x: 330, y: 270 },
        direction: MirDirection::Down,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: second.clone(),
        account_id: "second-account".to_string(),
        character_index: 0,
        object_id: 7,
        name: "Blade".to_string(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 7,
        hp: 60,
        max_hp: 60,
        mp: 100,
        map_file_name: "0".to_string(),
        position: Point { x: 332, y: 270 },
        direction: MirDirection::Down,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));

    assert_ne!(
        zone.player_object_id(&first),
        zone.player_object_id(&second)
    );
}
