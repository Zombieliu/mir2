use mir2_protocol::{
    ClientPacket, MirClass, MirDirection, MirGender, MirGridType, Point, ServerPacket,
};
use mir2_simulation::{
    reset_account_password_after_recovery, validate_commercial_identity_credentials,
    validate_production_player_command, InProcessWorldRuntime, SessionId, SimulationConfig,
    SimulationSession, WorldCommand, WorldRuntime, ZoneCommand, ZoneJoin, ZoneKey, ZoneRuntime,
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
fn town_revive_from_field_changes_to_the_configured_bind_map() {
    let config = SimulationConfig::default().with_crystal_world_runtime();
    let bind_map_file_name = config.map.file_name.clone();
    let bind_position = config.spawn.clone();
    let mut runtime = InProcessWorldRuntime::new(config);
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: 0,
        }))
        .expect("fixture character should start");
    runtime
        .execute(WorldCommand::TransferMap {
            key: "crystal:2:406:453".to_string(),
        })
        .expect("test fixture should enter a non-bind map");
    assert_eq!(runtime.world_snapshot().map_file_name.as_deref(), Some("2"));

    runtime
        .execute(WorldCommand::ApplyHandoffTransform {
            position: Point { x: 406, y: 453 },
            direction: MirDirection::DownLeft,
            hp: Some(0),
            mp: None,
        })
        .expect("test fixture should mark the player dead");
    let packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::TownRevive))
        .expect("TownRevive should execute");

    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::MapInformation { info } if info.file_name == bind_map_file_name
    )));
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::Revived)));
    let snapshot = runtime.world_snapshot();
    let player = snapshot
        .player_object_id
        .and_then(|object_id| {
            snapshot
                .entities
                .iter()
                .find(|entity| entity.object_id == object_id)
        })
        .expect("revived player should remain present");
    assert_eq!(
        snapshot.map_file_name.as_deref(),
        Some(bind_map_file_name.as_str())
    );
    assert_eq!((player.x, player.y), (bind_position.x, bind_position.y));
    assert!(snapshot.player_hp.is_some_and(|hp| hp > 0));
}

#[test]
fn queued_potion_cannot_revive_a_dead_player_before_town_revive() {
    let config = SimulationConfig::default()
        .with_crystal_world_runtime()
        .with_platinum_176_profile();
    let bind_position = config.spawn.clone();
    let mut runtime = InProcessWorldRuntime::new(config);
    runtime
        .execute(WorldCommand::PasskeyLogin {
            account_id: "queued-potion-death-lifecycle".to_string(),
        })
        .expect("fixture account should authenticate");
    let created = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::NewCharacter {
            name: "PotionGuard".to_string(),
            gender: MirGender::Male,
            class: MirClass::Warrior,
        }))
        .expect("fixture character should be created");
    let character_index = created
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .expect("new character response should include its index");
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index,
        }))
        .expect("fixture character should start");
    let potion_unique_id = runtime
        .world_snapshot()
        .inventory_items
        .iter()
        .find(|item| item.name == "(HP)DrugSmall")
        .map(|item| item.unique_id)
        .expect("Platinum starter inventory should contain one HP drug");
    runtime
        .execute(WorldCommand::ApplyHandoffTransform {
            position: bind_position.clone(),
            direction: MirDirection::Down,
            hp: Some(10),
            mp: None,
        })
        .expect("fixture should lower player HP");
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::UseItem {
            unique_id: potion_unique_id,
            grid: MirGridType::Inventory,
        }))
        .expect("normal HP drug should queue its timed recovery");
    runtime
        .execute(WorldCommand::ApplyHandoffTransform {
            position: bind_position,
            direction: MirDirection::Down,
            hp: Some(0),
            mp: None,
        })
        .expect("fixture should mark the player dead after the potion was queued");

    let tick_packets = runtime
        .execute(WorldCommand::Tick)
        .expect("dead-player tick should execute");

    assert_eq!(runtime.world_snapshot().player_hp, Some(0));
    assert!(!tick_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::Revived | ServerPacket::ObjectRevived { .. }
    )));
    let revive_packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::TownRevive))
        .expect("explicit TownRevive should still revive the player");
    assert!(revive_packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::Revived)));
    assert!(runtime.world_snapshot().player_hp.is_some_and(|hp| hp > 0));
}

#[test]
fn start_game_recovers_an_out_of_bounds_legacy_revive_transform() {
    let config = SimulationConfig::default().with_crystal_world_runtime();
    let bind_map_file_name = config.map.file_name.clone();
    let bind_position = config.spawn.clone();
    {
        let mut store = config
            .account_store
            .lock()
            .expect("test account store should lock");
        let save = store
            .accounts
            .get_mut("demo")
            .and_then(|account| account.saves.get_mut(&0))
            .expect("default character save should exist");
        save.map_file_name = "2".to_string();
        save.map_title = "SerpentValley".to_string();
        save.position = Point { x: 288, y: 616 };
    }

    let mut runtime = InProcessWorldRuntime::new(config);
    let packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: 0,
        }))
        .expect("legacy save should still start");

    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::MapInformation { info } if info.file_name == bind_map_file_name
    )));
    let snapshot = runtime.world_snapshot();
    let player = snapshot
        .player_object_id
        .and_then(|object_id| {
            snapshot
                .entities
                .iter()
                .find(|entity| entity.object_id == object_id)
        })
        .expect("recovered player should be present");
    assert_eq!(
        snapshot.map_file_name.as_deref(),
        Some(bind_map_file_name.as_str())
    );
    assert_eq!((player.x, player.y), (bind_position.x, bind_position.y));
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

#[test]
fn password_accounts_are_written_as_argon2id_and_can_login() {
    let config = SimulationConfig::default();
    let mut session = SimulationSession::new(config.clone());

    let created = session.handle_packet(ClientPacket::NewAccount {
        account_id: "argon-account".to_string(),
        password: "CommercialPassword42!".to_string(),
        birth_date_binary: 0,
        user_name: String::new(),
        secret_question: String::new(),
        secret_answer: String::new(),
        email_address: String::new(),
    });
    assert!(matches!(
        created.as_slice(),
        [ServerPacket::NewAccount { result: 8 }]
    ));

    let stored = config
        .account_store
        .lock()
        .expect("account store mutex should not be poisoned")
        .accounts
        .get("argon-account")
        .expect("created account should exist")
        .password
        .clone();
    assert!(stored.starts_with("$argon2id$"));
    assert!(!stored.contains("CommercialPassword42!"));

    let logged_in = session.handle_packet(ClientPacket::Login {
        account_id: "argon-account".to_string(),
        password: "CommercialPassword42!".to_string(),
    });
    assert!(matches!(
        logged_in.as_slice(),
        [ServerPacket::LoginSuccess { .. }]
    ));
}

#[test]
fn successful_legacy_plaintext_login_is_migrated_to_argon2id() {
    let config = SimulationConfig::default();
    {
        let mut store = config
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned");
        let account = store
            .accounts
            .get_mut("demo")
            .expect("default demo account should exist");
        account.password = "legacy-password".to_string();
    }

    let mut session = SimulationSession::new(config.clone());
    let logged_in = session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "legacy-password".to_string(),
    });
    assert!(matches!(
        logged_in.as_slice(),
        [ServerPacket::LoginSuccess { .. }]
    ));

    let migrated = config
        .account_store
        .lock()
        .expect("account store mutex should not be poisoned")
        .accounts
        .get("demo")
        .expect("default demo account should exist")
        .password
        .clone();
    assert!(migrated.starts_with("$argon2id$"));
    assert_ne!(migrated, "legacy-password");
}

#[test]
fn commercial_identity_policy_rejects_weak_or_ambiguous_credentials() {
    assert!(validate_commercial_identity_credentials("valid.account", "LongEnough42!").is_ok());
    assert!(validate_commercial_identity_credentials("x", "LongEnough42!").is_err());
    assert!(validate_commercial_identity_credentials("bad account", "LongEnough42!").is_err());
    assert!(validate_commercial_identity_credentials("valid.account", "password").is_err());
    assert!(validate_commercial_identity_credentials("valid.account", "valid.account").is_err());
}

#[test]
fn recovery_password_reset_rehashes_and_invalidates_the_previous_password() {
    let config = SimulationConfig::default();
    reset_account_password_after_recovery(&config, "demo", "RecoveredPassword42!")
        .expect("recovery reset should persist a commercial password");

    let stored = config
        .account_store
        .lock()
        .expect("account store mutex should not be poisoned")
        .accounts
        .get("demo")
        .expect("default demo account should exist")
        .password
        .clone();
    assert!(stored.starts_with("$argon2id$"));

    let mut old_session = SimulationSession::new(config.clone());
    let old_login = old_session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    assert!(!matches!(
        old_login.as_slice(),
        [ServerPacket::LoginSuccess { .. }]
    ));

    let mut new_session = SimulationSession::new(config);
    let new_login = new_session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "RecoveredPassword42!".to_string(),
    });
    assert!(matches!(
        new_login.as_slice(),
        [ServerPacket::LoginSuccess { .. }]
    ));
}
