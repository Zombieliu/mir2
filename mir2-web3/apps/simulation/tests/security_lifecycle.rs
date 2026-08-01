use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, Point, ServerPacket};
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
