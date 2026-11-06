use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_protocol::{ClientPacket, MirClass, MirGender, ServerPacket};

use super::{
    change_account_password, character_save_for_start, commit_active_character_save_transaction,
    create_account_with_password, delete_character_from_account, persist_active_character_save,
    recovery_login_preflight_after_source_refresh, reset_account_password_after_recovery,
    snapshot_active_character_save, RecoveryCredential, RecoveryLoginPreflight,
};
use crate::config::{
    AccountSourceRefreshOutcome, AccountStoreDatabaseMode, AccountStoreTransactionFault,
    Stage5MailMessage, Stage5SystemsState,
};
use crate::runtime::resources::{
    GmRuntimeResource, InventoryResource, PlayerPermissionResource, PlayerRuntimeResource,
    SessionResource,
};
use crate::runtime::session::SimulationSession;
use crate::{AccountStore, SimulationConfig};

fn snapshot_account_store(config: &SimulationConfig) -> serde_json::Value {
    let store = config.account_store.lock().unwrap();
    serde_json::to_value(&*store)
        .expect("the serializable account store should produce a stable test snapshot")
}

fn snapshot_serialized_account_store(config: &SimulationConfig) -> Vec<u8> {
    let store = config.account_store.lock().unwrap();
    serde_json::to_vec(&*store)
        .expect("the account store should produce deterministic serialized test bytes")
}

fn snapshot_account(config: &SimulationConfig, account_id: &str) -> serde_json::Value {
    let store = config.account_store.lock().unwrap();
    serde_json::to_value(
        store
            .accounts
            .get(account_id)
            .expect("fixture account should exist"),
    )
    .expect("the serializable account should produce a stable test snapshot")
}

fn snapshot_serialized_account(config: &SimulationConfig, account_id: &str) -> Vec<u8> {
    let store = config.account_store.lock().unwrap();
    serde_json::to_vec(
        store
            .accounts
            .get(account_id)
            .expect("fixture account should exist"),
    )
    .expect("the fixture account should serialize deterministically")
}

fn snapshot_serialized_disk_account(path: &std::path::Path, account_id: &str) -> Vec<u8> {
    let file = fs::read(path).expect("account-store file should remain readable");
    let store: AccountStore =
        serde_json::from_slice(&file).expect("account-store file should decode");
    serde_json::to_vec(
        store
            .accounts
            .get(account_id)
            .expect("fixture disk account should exist"),
    )
    .expect("the fixture disk account should serialize deterministically")
}

fn account_identity_fingerprint(
    config: &SimulationConfig,
    account_id: &str,
) -> (Vec<(i32, String)>, Vec<(i32, i32, String)>) {
    let store = config.account_store.lock().unwrap();
    let account = store
        .accounts
        .get(account_id)
        .expect("fixture account should exist");
    let roster = account
        .characters
        .iter()
        .map(|character| (character.index, character.name.clone()))
        .collect();
    let durable = account
        .saves
        .iter()
        .map(|(key, save)| (*key, save.character.index, save.character.name.clone()))
        .collect();
    (roster, durable)
}

fn durable_revision_and_gold(
    config: &SimulationConfig,
    account_id: &str,
    character_index: i32,
) -> (u64, u32) {
    let store = config.account_store.lock().unwrap();
    let save = store
        .accounts
        .get(account_id)
        .and_then(|account| account.saves.get(&character_index))
        .expect("fixture durable save should exist");
    (save.revision, save.gold)
}

fn unique_account_id(prefix: &str) -> String {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let account_id = format!("{prefix}{suffix:x}");
    assert!(account_id.len() <= 32);
    account_id
}

fn seed_accounts_from_demo(config: &SimulationConfig, accounts: &[(&str, &str, u8)]) {
    let mut store = config.account_store.lock().unwrap();
    let seed = store
        .accounts
        .remove("demo")
        .expect("default account should seed the account fixtures");
    for (account_id, character_name, gm_level) in accounts {
        let mut account = seed.clone();
        account.gm_level = *gm_level;
        account.characters[0].name = (*character_name).to_string();
        account
            .saves
            .get_mut(&0)
            .expect("seeded account save should exist")
            .character
            .name = (*character_name).to_string();
        store.accounts.insert((*account_id).to_string(), account);
    }
}

fn send_chat(session: &mut SimulationSession, message: &str) -> Vec<ServerPacket> {
    session.handle_packet(ClientPacket::Chat {
        message: message.to_string(),
        linked_items: Vec::new(),
    })
}

fn durable_mailbox(
    config: &SimulationConfig,
    account_id: &str,
    character_index: i32,
) -> Vec<Stage5MailMessage> {
    let store = config.account_store.lock().unwrap();
    let encoded = store
        .accounts
        .get(account_id)
        .and_then(|account| account.saves.get(&character_index))
        .and_then(|save| save.stage5_systems_json.as_deref())
        .expect("fixture character should have durable stage5 state");
    serde_json::from_str::<Stage5SystemsState>(encoded)
        .expect("fixture stage5 state should decode")
        .mail
}

fn assert_external_mail_refresh_is_observationally_rejected(
    session: &mut SimulationSession,
    config: &SimulationConfig,
) {
    let before_store = snapshot_account_store(config);
    let before_world = session.world_snapshot();
    let (before_account_id, before_selected) = {
        let state = session.app.world().resource::<SessionResource>();
        (state.account_id.clone(), state.selected_character.clone())
    };

    assert!(!session.refresh_active_external_mail());

    assert_eq!(snapshot_account_store(config), before_store);
    assert_eq!(session.world_snapshot(), before_world);
    let state = session.app.world().resource::<SessionResource>();
    assert_eq!(state.account_id, before_account_id);
    assert_eq!(state.selected_character, before_selected);
}

fn fixture_mail(
    id: u32,
    delivery_nonce: &str,
    to: &str,
    subject: &str,
    body: &str,
    gold: u32,
) -> Stage5MailMessage {
    Stage5MailMessage {
        id,
        delivery_nonce: delivery_nonce.to_string(),
        from: "Postmaster".to_string(),
        to: to.to_string(),
        subject: subject.to_string(),
        body: body.to_string(),
        gold,
        items: Vec::new(),
        item_states_json: Vec::new(),
        opened: false,
        locked: false,
        claimed: false,
        deleted: false,
    }
}

const COLLISION_ACCOUNT_A: &str = "durable_collision_a";
const COLLISION_ACCOUNT_B: &str = "durable_collision_b";
const COLLISION_CHARACTER_A: &str = "DurableAlpha";
const COLLISION_CHARACTER_B: &str = "DurableBeta";
const COLLISION_PRIVATE_MAIL_ID: u32 = 52;
const SEND_MAIL_SENDER_ACCOUNT: &str = "mail_identity_sender";
const SEND_MAIL_TARGET_ACCOUNT: &str = "mail_identity_target";
const SEND_MAIL_SENDER_CHARACTER: &str = "MailSender";
const SEND_MAIL_TARGET_CHARACTER: &str = "MailTarget";
const SEND_MAIL_OTHER_CHARACTER: &str = "OtherTarget";

fn send_mail_target_identity_fixture(
    label: &str,
    mismatch_target_save: bool,
) -> (
    SimulationConfig,
    SimulationSession,
    std::path::PathBuf,
    std::path::PathBuf,
    u64,
) {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "mir2-send-mail-target-identity-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("send-mail fixture directory should be created");
    let path = root.join("accounts.json");
    let config = SimulationConfig::default().with_account_store_path(&path);

    {
        let mut store = config.account_store.lock().unwrap();
        let seed = store
            .accounts
            .remove("demo")
            .expect("default account should seed the send-mail fixture");

        let mut sender = seed.clone();
        sender.characters[0].name = SEND_MAIL_SENDER_CHARACTER.to_string();
        sender
            .saves
            .get_mut(&0)
            .expect("sender save should exist")
            .character
            .name = SEND_MAIL_SENDER_CHARACTER.to_string();

        let mut target = seed;
        let mut target_character = target.characters[0].clone();
        target_character.name = SEND_MAIL_TARGET_CHARACTER.to_string();
        let mut other_character = target_character.clone();
        other_character.index = 1;
        other_character.name = SEND_MAIL_OTHER_CHARACTER.to_string();
        target.characters = vec![target_character.clone(), other_character.clone()];

        let mut target_save = target
            .saves
            .remove(&0)
            .expect("target seed save should exist");
        target_save.character = target_character;
        target_save.stage5_systems_json = Some(
            serde_json::to_string(&Stage5SystemsState::default())
                .expect("target mailbox should encode"),
        );
        let mut other_save = target_save.clone();
        other_save.character = other_character;
        target.saves.clear();
        target.saves.insert(
            0,
            if mismatch_target_save {
                other_save.clone()
            } else {
                target_save
            },
        );
        target.saves.insert(1, other_save);

        store
            .accounts
            .insert(SEND_MAIL_SENDER_ACCOUNT.to_string(), sender);
        store
            .accounts
            .insert(SEND_MAIL_TARGET_ACCOUNT.to_string(), target);
    }
    config
        .save_account_store()
        .expect("send-mail fixture should have a durable baseline");

    let mut sender = SimulationSession::new(config.clone());
    assert!(sender
        .handle_packet(ClientPacket::Login {
            account_id: SEND_MAIL_SENDER_ACCOUNT.to_string(),
            password: "demo".to_string(),
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert!(sender
        .handle_packet(ClientPacket::StartGame { character_index: 0 })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    sender
        .app
        .world_mut()
        .resource_mut::<PlayerRuntimeResource>()
        .gold = 3_000;
    let attachment_id = sender
        .app
        .world()
        .resource::<InventoryResource>()
        .inventory_items
        .iter()
        .find(|item| item.key == "dagger")
        .expect("seed dagger should provide a deterministic mail attachment")
        .unique_id;
    assert_ne!(attachment_id, 0);
    persist_active_character_save(sender.app.world())
        .expect("sender gold and attachment should have a durable baseline");

    (config, sender, root, path, attachment_id)
}

fn durable_identity_collision_fixture(
    label: &str,
    stale_durable_revision: bool,
) -> (
    SimulationConfig,
    SimulationSession,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "mir2-durable-identity-collision-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("collision fixture directory should be created");
    let path = root.join("accounts.json");
    let config = SimulationConfig::default().with_account_store_path(&path);

    let mail_a = fixture_mail(
        51,
        "durable-collision-alpha",
        COLLISION_CHARACTER_A,
        "Alpha private mail",
        "This belongs only to account A.",
        51,
    );
    let mut mail_b = fixture_mail(
        COLLISION_PRIVATE_MAIL_ID,
        "durable-collision-beta",
        COLLISION_CHARACTER_B,
        "Beta private mail",
        "This must never be read, claimed, or merged by account A.",
        52_000,
    );
    mail_b.items = vec!["wooden-sword".to_string()];

    {
        let mut store = config.account_store.lock().unwrap();
        let seed = store
            .accounts
            .remove("demo")
            .expect("default account should seed the durable collision fixture");

        let mut account_a = seed.clone();
        account_a.characters[0].name = COLLISION_CHARACTER_A.to_string();
        let save_a = account_a
            .saves
            .get_mut(&0)
            .expect("account A save should exist");
        save_a.character.name = COLLISION_CHARACTER_A.to_string();
        let mut systems_a = Stage5SystemsState::default();
        systems_a.mail = vec![mail_a];
        save_a.stage5_systems_json =
            Some(serde_json::to_string(&systems_a).expect("account A mailbox should encode"));

        let mut account_b = seed;
        // The roster is deliberately forged to match A's active index/name.
        account_b.characters[0].name = COLLISION_CHARACTER_A.to_string();
        let save_b = account_b
            .saves
            .get_mut(&0)
            .expect("account B save should exist");
        // The durable save retains B's real embedded identity. This independent
        // identity boundary must reject the transaction before revision/mail use.
        save_b.character.name = COLLISION_CHARACTER_B.to_string();
        let mut systems_b = Stage5SystemsState::default();
        systems_b.mail = vec![mail_b];
        save_b.stage5_systems_json =
            Some(serde_json::to_string(&systems_b).expect("account B mailbox should encode"));

        store
            .accounts
            .insert(COLLISION_ACCOUNT_A.to_string(), account_a);
        store
            .accounts
            .insert(COLLISION_ACCOUNT_B.to_string(), account_b);
    }
    config
        .save_account_store()
        .expect("collision fixture should have a durable baseline");

    let mut session = SimulationSession::new(config.clone());
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: COLLISION_ACCOUNT_A.to_string(),
            password: "demo".to_string(),
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert!(session
        .handle_packet(ClientPacket::StartGame { character_index: 0 })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));

    let active_revision = snapshot_active_character_save(session.app.world())
        .expect("account A should have an active save")
        .revision;
    {
        let mut store = config.account_store.lock().unwrap();
        let durable_save = store
            .accounts
            .get_mut(COLLISION_ACCOUNT_B)
            .and_then(|account| account.saves.get_mut(&0))
            .expect("account B durable save should exist");
        durable_save.revision = if stale_durable_revision {
            active_revision
                .checked_add(7)
                .expect("test revision should not overflow")
        } else {
            active_revision
        };
    }
    config
        .save_account_store()
        .expect("collision revision fixture should be durable");

    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .account_id = Some(COLLISION_ACCOUNT_B.to_string());

    (config, session, root, path)
}

fn assert_collision_state_unchanged(
    session: &SimulationSession,
    config: &SimulationConfig,
    path: &std::path::Path,
    before_world: &crate::WorldSnapshot,
    before_store: &[u8],
    before_file: &[u8],
    before_session: &(Option<String>, Option<crate::CharacterRecord>, Option<u64>),
) {
    let after_world = session.world_snapshot();
    assert_eq!(&after_world, before_world);
    let after_store = snapshot_serialized_account_store(config);
    assert_eq!(after_store.as_slice(), before_store);
    let after_file = fs::read(path).expect("account-store file should remain readable");
    assert_eq!(after_file.as_slice(), before_file);
    let state = session.app.world().resource::<SessionResource>();
    assert_eq!(
        &(
            state.account_id.clone(),
            state.selected_character.clone(),
            state.active_save_revision(),
        ),
        before_session
    );
}

fn assert_rejected_bound_identity_isolated(
    session: &mut SimulationSession,
    config: &SimulationConfig,
    bound_identity: &str,
) {
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .account_id = Some(bound_identity.to_string());
    let before = snapshot_account_store(config);

    let transaction_error = commit_active_character_save_transaction(session.app.world(), |save| {
        save.gold = 88_888;
        Ok(())
    })
    .unwrap_err();
    assert!(transaction_error.contains("requires an account identity"));
    assert_eq!(snapshot_account_store(config), before);

    let persist_error = persist_active_character_save(session.app.world()).unwrap_err();
    assert!(persist_error.contains("without an authenticated account identity"));
    assert_eq!(snapshot_account_store(config), before);

    let new_character = session.handle_packet(ClientPacket::NewCharacter {
        name: "MustNotExist".to_string(),
        gender: MirGender::Female,
        class: MirClass::Wizard,
    });
    assert!(new_character.is_empty());
    assert_eq!(snapshot_account_store(config), before);

    let delete_character =
        session.handle_packet(ClientPacket::DeleteCharacter { character_index: 0 });
    assert!(delete_character.is_empty());
    assert_eq!(snapshot_account_store(config), before);

    let start_game = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(start_game.is_empty());
    assert_eq!(snapshot_account_store(config), before);
    assert_eq!(
        session
            .app
            .world()
            .resource::<SessionResource>()
            .account_id
            .as_deref(),
        Some(bound_identity)
    );
}

#[test]
fn active_snapshot_without_account_identity_is_rejected() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    let login = session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let start = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(start
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    assert!(snapshot_active_character_save(session.app.world()).is_some());

    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .account_id = None;

    let error = persist_active_character_save(session.app.world()).unwrap_err();
    assert!(error.contains("without an authenticated account identity"));
}

#[test]
fn new_character_without_account_identity_is_rejected_and_does_not_mutate_account_store() {
    // Crystal silently ignores NewCharacter outside Select stage:
    // ../Crystal/Server/MirNetwork/MirConnection.cs:1031-1035.
    // The production Rust boundary rejects unauthenticated lifecycle commands before dispatch:
    // apps/simulation/src/world_runtime.rs:148-154. Direct SimulationSession callers therefore
    // retain Crystal's packet-visible silence while still failing closed before any mutation.
    let config = SimulationConfig::default();
    let before = snapshot_account_store(&config);
    let mut session = SimulationSession::new(config.clone());

    let packets = session.handle_packet(ClientPacket::NewCharacter {
        name: "NoAuthCharacter".to_string(),
        gender: MirGender::Male,
        class: MirClass::Warrior,
    });

    assert!(packets.is_empty());
    assert_eq!(snapshot_account_store(&config), before);
}

#[test]
fn delete_character_without_account_identity_is_rejected_and_does_not_mutate_account_store() {
    // Crystal silently ignores DeleteCharacter outside Select stage:
    // ../Crystal/Server/MirNetwork/MirConnection.cs:1037-1040.
    let config = SimulationConfig::default();
    let before = snapshot_account_store(&config);
    let mut session = SimulationSession::new(config.clone());

    let packets = session.handle_packet(ClientPacket::DeleteCharacter { character_index: 0 });

    assert!(packets.is_empty());
    assert_eq!(snapshot_account_store(&config), before);
}

#[test]
fn start_game_without_account_identity_is_rejected_and_does_not_mutate_account_store() {
    // Crystal first ignores StartGame outside Select
    // (../Crystal/Server/MirNetwork/MirConnection.cs:1069-1072), then uses Result=1 for the
    // Select-stage missing-account case
    // (../Crystal/Server/MirNetwork/MirConnection.cs:1079-1082). Result=2 is reserved for a
    // missing character on a bound account
    // (../Crystal/Server/MirNetwork/MirConnection.cs:1095-1098).
    let config = SimulationConfig::default();
    let before = snapshot_account_store(&config);
    let mut session = SimulationSession::new(config.clone());

    let packets = session.handle_packet(ClientPacket::StartGame { character_index: 0 });

    assert!(matches!(
        packets.as_slice(),
        [ServerPacket::StartGame {
            result: 1,
            resolution: 0
        }]
    ));
    assert_eq!(snapshot_account_store(&config), before);
}

#[test]
fn in_game_character_lifecycle_requests_are_silent_and_leave_world_and_store_unchanged() {
    // Crystal checks Stage == Select before all three handlers and silently returns otherwise:
    // ../Crystal/Server/MirNetwork/MirConnection.cs:1031-1035 (NewCharacter),
    // ../Crystal/Server/MirNetwork/MirConnection.cs:1037-1040 (DeleteCharacter), and
    // ../Crystal/Server/MirNetwork/MirConnection.cs:1069-1072 (StartGame).
    let config = SimulationConfig::default();
    let mut session = SimulationSession::new(config.clone());
    let login = session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let start = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(start
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));

    let before_store = snapshot_account_store(&config);
    let before_world = session.world_snapshot();
    let before_selected = session
        .app
        .world()
        .resource::<SessionResource>()
        .selected_character
        .clone();

    let new_character = session.handle_packet(ClientPacket::NewCharacter {
        name: "MustNotBeCreatedInGame".to_string(),
        gender: MirGender::Female,
        class: MirClass::Wizard,
    });
    assert!(new_character.is_empty());
    assert_eq!(snapshot_account_store(&config), before_store);
    assert_eq!(session.world_snapshot(), before_world);
    assert_eq!(
        session
            .app
            .world()
            .resource::<SessionResource>()
            .selected_character,
        before_selected
    );

    let delete_character =
        session.handle_packet(ClientPacket::DeleteCharacter { character_index: 0 });
    assert!(delete_character.is_empty());
    assert_eq!(snapshot_account_store(&config), before_store);
    assert_eq!(session.world_snapshot(), before_world);
    assert_eq!(
        session
            .app
            .world()
            .resource::<SessionResource>()
            .selected_character,
        before_selected
    );

    let reenter = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(reenter.is_empty());
    assert_eq!(snapshot_account_store(&config), before_store);
    assert_eq!(session.world_snapshot(), before_world);
    assert_eq!(
        session
            .app
            .world()
            .resource::<SessionResource>()
            .selected_character,
        before_selected
    );
}

#[test]
fn canonical_select_missing_character_returns_crystal_result_two_without_mutation() {
    // Crystal returns Result=2 when the bound account has no matching character:
    // ../Crystal/Server/MirNetwork/MirConnection.cs:1095-1098.
    let config = SimulationConfig::default();
    let mut session = SimulationSession::new(config.clone());
    let login = session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let before = snapshot_account_store(&config);

    let packets = session.handle_packet(ClientPacket::StartGame {
        character_index: i32::MAX,
    });

    assert!(matches!(
        packets.as_slice(),
        [ServerPacket::StartGame {
            result: 2,
            resolution: 0
        }]
    ));
    assert_eq!(snapshot_account_store(&config), before);
    assert!(session
        .app
        .world()
        .resource::<SessionResource>()
        .selected_character
        .is_none());
}

#[test]
fn noncanonical_bound_identity_cannot_alias_or_mutate_either_account() {
    const CANONICAL_ACCOUNT: &str = "victim";
    const WHITESPACE_ACCOUNT: &str = " victim ";

    let config = SimulationConfig::default();
    {
        let mut store = config.account_store.lock().unwrap();
        let seed = store
            .accounts
            .remove("demo")
            .expect("default account should seed the isolation fixture");
        store
            .accounts
            .insert(CANONICAL_ACCOUNT.to_string(), seed.clone());
        store.accounts.insert(WHITESPACE_ACCOUNT.to_string(), seed);
        assert_eq!(store.accounts.len(), 2);
    }

    let mut session = SimulationSession::new(config.clone());
    let login = session.handle_packet(ClientPacket::Login {
        account_id: CANONICAL_ACCOUNT.to_string(),
        password: "demo".to_string(),
    });
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let start = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(start
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));

    session
        .app
        .world_mut()
        .resource_mut::<PlayerRuntimeResource>()
        .gold = 77_777;
    assert_rejected_bound_identity_isolated(&mut session, &config, WHITESPACE_ACCOUNT);
    assert_rejected_bound_identity_isolated(&mut session, &config, "");

    let store = config.account_store.lock().unwrap();
    assert_eq!(store.accounts.len(), 2);
    assert!(store.accounts.contains_key(CANONICAL_ACCOUNT));
    assert!(store.accounts.contains_key(WHITESPACE_ACCOUNT));
}

#[test]
fn noncanonical_live_identity_cannot_read_merge_or_mutate_either_mailbox() {
    const CANONICAL_ACCOUNT: &str = "victim";
    const WHITESPACE_ACCOUNT: &str = " victim ";
    const CANONICAL_CHARACTER: &str = "VictimHero";
    const WHITESPACE_CHARACTER: &str = "WhitespaceHero";

    let mut canonical_mail = fixture_mail(
        7,
        "canonical-delivery",
        CANONICAL_CHARACTER,
        "Canonical",
        "Only the canonical account may load this.",
        70,
    );
    canonical_mail.items = vec!["dagger".to_string()];
    let mut whitespace_mail = fixture_mail(
        7,
        "whitespace-delivery",
        WHITESPACE_CHARACTER,
        "Whitespace",
        "This must never merge into the canonical live world.",
        700,
    );
    whitespace_mail.items = vec!["wooden-sword".to_string()];
    let config = SimulationConfig::default();
    {
        let mut store = config.account_store.lock().unwrap();
        let seed = store
            .accounts
            .remove("demo")
            .expect("default account should seed the mailbox isolation fixture");
        let mut canonical = seed.clone();
        canonical.characters[0].name = CANONICAL_CHARACTER.to_string();
        let canonical_save = canonical
            .saves
            .get_mut(&0)
            .expect("canonical fixture save should exist");
        canonical_save.character.name = CANONICAL_CHARACTER.to_string();
        let mut canonical_systems = Stage5SystemsState::default();
        canonical_systems.mail = vec![canonical_mail.clone()];
        canonical_save.stage5_systems_json = Some(
            serde_json::to_string(&canonical_systems)
                .expect("canonical fixture mailbox should encode"),
        );

        let mut whitespace = seed;
        whitespace.characters[0].name = WHITESPACE_CHARACTER.to_string();
        let whitespace_save = whitespace
            .saves
            .get_mut(&0)
            .expect("whitespace fixture save should exist");
        whitespace_save.character.name = WHITESPACE_CHARACTER.to_string();
        let mut whitespace_systems = Stage5SystemsState::default();
        whitespace_systems.mail = vec![whitespace_mail.clone()];
        whitespace_save.stage5_systems_json = Some(
            serde_json::to_string(&whitespace_systems)
                .expect("whitespace fixture mailbox should encode"),
        );

        store
            .accounts
            .insert(CANONICAL_ACCOUNT.to_string(), canonical);
        store
            .accounts
            .insert(WHITESPACE_ACCOUNT.to_string(), whitespace);
    }

    let mut session = SimulationSession::new(config.clone());
    let login = session.handle_packet(ClientPacket::Login {
        account_id: CANONICAL_ACCOUNT.to_string(),
        password: "demo".to_string(),
    });
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let start = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(start
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    assert_eq!(
        session.world_snapshot().stage5_systems.mail,
        vec![canonical_mail.clone()]
    );

    let before_store = snapshot_account_store(&config);
    let before_world = session.world_snapshot();
    let before_live_mail = before_world.stage5_systems.mail.clone();
    let before_selected = session
        .app
        .world()
        .resource::<SessionResource>()
        .selected_character
        .clone();

    // Crystal silently returns for out-of-Game mail requests
    // (../Crystal/Server/MirNetwork/MirConnection.cs:2017-2042) and for an
    // unknown mail identity (../Crystal/Server/MirObjects/PlayerObject.cs:11881-11957).
    // An identity/persistence rejection must therefore emit no ReceiveMail
    // payload at all: even a redacted list would be a newly invented contract.
    for bound_identity in [WHITESPACE_ACCOUNT, ""] {
        session
            .app
            .world_mut()
            .resource_mut::<SessionResource>()
            .account_id = Some(bound_identity.to_string());
        assert!(!session.refresh_active_external_mail());

        for command in [
            ClientPacket::ReadMail { mail_id: 7 },
            ClientPacket::LockMail {
                mail_id: 7,
                lock: true,
            },
            ClientPacket::DeleteMail { mail_id: 7 },
        ] {
            let packets = session.handle_packet(command);
            assert!(
                packets.is_empty(),
                "rejected mail status must not expose any cached payload: {packets:?}"
            );
        }

        assert_eq!(
            session.world_snapshot().stage5_systems.mail,
            before_live_mail
        );
        assert_eq!(session.world_snapshot(), before_world);
        assert_eq!(
            session
                .app
                .world()
                .resource::<SessionResource>()
                .selected_character,
            before_selected
        );
        assert_eq!(snapshot_account_store(&config), before_store);
    }

    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .account_id = Some(WHITESPACE_ACCOUNT.to_string());
    let collect = session.handle_packet(ClientPacket::CollectParcel { mail_id: 7 });
    assert!(collect.is_empty());
    assert_eq!(
        session.world_snapshot().stage5_systems.mail,
        before_live_mail
    );
    assert_eq!(snapshot_account_store(&config), before_store);

    let send = session.handle_packet(ClientPacket::SendMail {
        name: CANONICAL_CHARACTER.to_string(),
        message: "Must not send from a malformed identity.".to_string(),
        gold: 0,
        items_idx: [0; 5],
        stamped: false,
    });
    assert!(matches!(
        send.as_slice(),
        [ServerPacket::MailSent { result: -1 }]
    ));
    assert_eq!(
        session.world_snapshot().stage5_systems.mail,
        before_live_mail
    );
    assert_eq!(snapshot_account_store(&config), before_store);
    assert_eq!(session.world_snapshot(), before_world);
    assert_eq!(
        session
            .app
            .world()
            .resource::<SessionResource>()
            .selected_character,
        before_selected
    );

    let whitespace_before_canonical_save = snapshot_account(&config, WHITESPACE_ACCOUNT);
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .account_id = Some(CANONICAL_ACCOUNT.to_string());
    persist_active_character_save(session.app.world())
        .expect("restored canonical identity should save its own live world");

    assert_eq!(
        durable_mailbox(&config, CANONICAL_ACCOUNT, 0),
        vec![canonical_mail]
    );
    assert_eq!(
        durable_mailbox(&config, WHITESPACE_ACCOUNT, 0),
        vec![whitespace_mail]
    );
    assert_eq!(
        snapshot_account(&config, WHITESPACE_ACCOUNT),
        whitespace_before_canonical_save
    );
}

#[test]
fn canonical_account_switch_with_colliding_index_cannot_refresh_foreign_mail() {
    const ACCOUNT_A: &str = "mail_collision_a";
    const ACCOUNT_B: &str = "mail_collision_b";
    const CHARACTER_A: &str = "CollisionAlpha";
    const CHARACTER_B: &str = "CollisionBeta";

    let mail_a = fixture_mail(
        21,
        "collision-alpha-delivery",
        CHARACTER_A,
        "Alpha private mail",
        "This belongs only to account A.",
        21,
    );
    let mut mail_b = fixture_mail(
        22,
        "collision-beta-delivery",
        CHARACTER_B,
        "Beta private mail",
        "This must never merge into account A's live World.",
        22_000,
    );
    mail_b.items = vec!["wooden-sword".to_string()];

    let config = SimulationConfig::default();
    {
        let mut store = config.account_store.lock().unwrap();
        let seed = store
            .accounts
            .remove("demo")
            .expect("default account should seed the collision fixture");

        let mut account_a = seed.clone();
        account_a.characters[0].name = CHARACTER_A.to_string();
        let save_a = account_a
            .saves
            .get_mut(&0)
            .expect("account A save should exist");
        save_a.character.name = CHARACTER_A.to_string();
        let mut systems_a = Stage5SystemsState::default();
        systems_a.mail = vec![mail_a.clone()];
        save_a.stage5_systems_json = Some(serde_json::to_string(&systems_a).unwrap());

        let mut account_b = seed;
        account_b.characters[0].name = CHARACTER_B.to_string();
        let save_b = account_b
            .saves
            .get_mut(&0)
            .expect("account B save should exist");
        save_b.character.name = CHARACTER_B.to_string();
        let mut systems_b = Stage5SystemsState::default();
        systems_b.mail = vec![mail_b.clone()];
        save_b.stage5_systems_json = Some(serde_json::to_string(&systems_b).unwrap());

        store.accounts.insert(ACCOUNT_A.to_string(), account_a);
        store.accounts.insert(ACCOUNT_B.to_string(), account_b);
    }

    let mut session = SimulationSession::new(config.clone());
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: ACCOUNT_A.to_string(),
            password: "demo".to_string(),
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert!(session
        .handle_packet(ClientPacket::StartGame { character_index: 0 })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    assert_eq!(session.world_snapshot().stage5_systems.mail, vec![mail_a]);

    // Both account IDs are canonical and both rosters use index 0. Merely
    // rebinding A's live session to B must not make B's differently named
    // character or private mailbox authoritative for A's selected character.
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .account_id = Some(ACCOUNT_B.to_string());
    assert_external_mail_refresh_is_observationally_rejected(&mut session, &config);

    // Even if B's roster is corrupted to claim A's exact index+name, the
    // durable save identity is an independent mandatory check.
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut(ACCOUNT_B)
        .unwrap()
        .characters[0]
        .name = CHARACTER_A.to_string();
    assert_external_mail_refresh_is_observationally_rejected(&mut session, &config);
    assert_eq!(durable_mailbox(&config, ACCOUNT_B, 0), vec![mail_b]);
}

#[test]
fn forged_roster_durable_identity_collision_rejects_all_private_mail_transactions() {
    let (config, mut session, root, path) =
        durable_identity_collision_fixture("mail-transactions", false);

    for (operation, command) in [
        (
            "ReadMail",
            ClientPacket::ReadMail {
                mail_id: u64::from(COLLISION_PRIVATE_MAIL_ID),
            },
        ),
        (
            "LockMail",
            ClientPacket::LockMail {
                mail_id: u64::from(COLLISION_PRIVATE_MAIL_ID),
                lock: true,
            },
        ),
        (
            "DeleteMail",
            ClientPacket::DeleteMail {
                mail_id: u64::from(COLLISION_PRIVATE_MAIL_ID),
            },
        ),
        (
            "CollectParcel",
            ClientPacket::CollectParcel {
                mail_id: u64::from(COLLISION_PRIVATE_MAIL_ID),
            },
        ),
    ] {
        let before_world = session.world_snapshot();
        let before_store = snapshot_serialized_account_store(&config);
        let before_file = fs::read(&path).expect("account-store file should exist");
        let before_session = {
            let state = session.app.world().resource::<SessionResource>();
            (
                state.account_id.clone(),
                state.selected_character.clone(),
                state.active_save_revision(),
            )
        };

        let packets = session.handle_packet(command);

        assert!(
            packets.is_empty(),
            "{operation} durable identity rejection must be silent: {packets:?}"
        );
        assert!(
            !packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::ReceiveMail { .. })),
            "{operation} must not expose any mailbox payload: {packets:?}"
        );
        let packet_debug = format!("{packets:?}");
        for private_value in [
            "Beta private mail",
            "This must never be read, claimed, or merged by account A.",
            "wooden-sword",
            "52000",
        ] {
            assert!(
                !packet_debug.contains(private_value),
                "{operation} leaked private account B data: {packet_debug}"
            );
        }
        assert_collision_state_unchanged(
            &session,
            &config,
            &path,
            &before_world,
            &before_store,
            &before_file,
            &before_session,
        );
    }

    fs::remove_dir_all(root).expect("collision fixture directory should be removed");
}

fn assert_forged_roster_full_save_is_rejected(label: &str, stale_durable_revision: bool) {
    let (config, session, root, path) =
        durable_identity_collision_fixture(label, stale_durable_revision);
    let before_world = session.world_snapshot();
    let before_store = snapshot_serialized_account_store(&config);
    let before_file = fs::read(&path).expect("account-store file should exist");
    let before_session = {
        let state = session.app.world().resource::<SessionResource>();
        (
            state.account_id.clone(),
            state.selected_character.clone(),
            state.active_save_revision(),
        )
    };

    let error = persist_active_character_save(session.app.world())
        .expect_err("forged roster must not authorize a full save into account B");

    assert!(
        error.contains("full character durable save identity mismatch"),
        "durable identity must fail before revision/CAS/mail processing: {error}"
    );
    assert_collision_state_unchanged(
        &session,
        &config,
        &path,
        &before_world,
        &before_store,
        &before_file,
        &before_session,
    );
    fs::remove_dir_all(root).expect("collision fixture directory should be removed");
}

#[test]
fn forged_roster_durable_identity_collision_rejects_normal_revision_full_save() {
    assert_forged_roster_full_save_is_rejected("normal-full-save", false);
}

#[test]
fn forged_roster_durable_identity_collision_rejects_stale_revision_full_save() {
    assert_forged_roster_full_save_is_rejected("stale-full-save", true);
}

#[test]
fn canonical_identity_still_commits_mail_transaction_and_full_save() {
    let (config, mut session, root, path) =
        durable_identity_collision_fixture("canonical-positive", false);
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .account_id = Some(COLLISION_ACCOUNT_A.to_string());
    let before_account_b = snapshot_serialized_account(&config, COLLISION_ACCOUNT_B);
    let before_disk_account_b = snapshot_serialized_disk_account(&path, COLLISION_ACCOUNT_B);
    let before_identity_b = account_identity_fingerprint(&config, COLLISION_ACCOUNT_B);
    let (before_mail_revision, _) = durable_revision_and_gold(&config, COLLISION_ACCOUNT_A, 0);
    let before_mail_file = fs::read(&path).expect("canonical fixture file should exist");

    let packets = session.handle_packet(ClientPacket::ReadMail { mail_id: 51 });

    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ReceiveMail { .. })),
        "canonical ReadMail should retain its existing success response: {packets:?}"
    );
    assert!(
        durable_mailbox(&config, COLLISION_ACCOUNT_A, 0)[0].opened,
        "canonical private transaction should persist the read status"
    );
    let (before_full_save_revision, before_full_save_gold) =
        durable_revision_and_gold(&config, COLLISION_ACCOUNT_A, 0);
    assert_eq!(
        before_full_save_revision,
        before_mail_revision
            .checked_add(1)
            .expect("mail transaction revision should not overflow"),
        "canonical mail transaction must advance the durable revision exactly once"
    );
    let before_full_save_file =
        fs::read(&path).expect("canonical mail transaction file should exist");
    assert_ne!(
        before_full_save_file, before_mail_file,
        "canonical mail transaction must publish changed file bytes"
    );

    let persisted_gold = before_full_save_gold
        .checked_add(1_337)
        .expect("canonical test gold should not overflow");
    session
        .app
        .world_mut()
        .resource_mut::<PlayerRuntimeResource>()
        .gold = persisted_gold;
    persist_active_character_save(session.app.world())
        .expect("canonical full save should remain valid after the private transaction");

    let (after_full_save_revision, after_full_save_gold) =
        durable_revision_and_gold(&config, COLLISION_ACCOUNT_A, 0);
    assert_eq!(
        after_full_save_gold, persisted_gold,
        "canonical full save must write the changed World gold field"
    );
    assert_eq!(
        after_full_save_revision,
        before_full_save_revision
            .checked_add(1)
            .expect("full-save revision should not overflow"),
        "canonical full save must advance revision exactly once"
    );
    assert_eq!(
        session
            .app
            .world()
            .resource::<SessionResource>()
            .active_save_revision(),
        Some(after_full_save_revision),
        "the live CAS revision must track the committed full save"
    );
    let after_full_save_file =
        fs::read(&path).expect("canonical full-save file should remain readable");
    assert_ne!(
        after_full_save_file, before_full_save_file,
        "canonical full save must publish changed file bytes"
    );
    assert_eq!(
        snapshot_serialized_account(&config, COLLISION_ACCOUNT_B),
        before_account_b,
        "legal account A transactions must not change serialized account B"
    );
    assert_eq!(
        snapshot_serialized_disk_account(&path, COLLISION_ACCOUNT_B),
        before_disk_account_b,
        "legal account A transactions must not change account B in the durable file"
    );
    assert_eq!(
        account_identity_fingerprint(&config, COLLISION_ACCOUNT_B),
        before_identity_b,
        "legal account A transactions must not change account B roster/save identity"
    );

    fs::remove_dir_all(root).expect("collision fixture directory should be removed");
}

#[test]
fn send_mail_target_durable_identity_mismatch_is_silent_and_atomic() {
    let (config, mut sender, root, path, attachment_id) =
        send_mail_target_identity_fixture("mismatch", true);
    let before_world = sender.world_snapshot();
    let before_store = snapshot_serialized_account_store(&config);
    let before_file = fs::read(&path).expect("send-mail fixture file should exist");
    let before_sender = snapshot_serialized_account(&config, SEND_MAIL_SENDER_ACCOUNT);
    let before_target = snapshot_serialized_account(&config, SEND_MAIL_TARGET_ACCOUNT);
    let before_target_identity = account_identity_fingerprint(&config, SEND_MAIL_TARGET_ACCOUNT);
    let before_target_mailbox = durable_mailbox(&config, SEND_MAIL_TARGET_ACCOUNT, 0);
    let before_sender_revision = durable_revision_and_gold(&config, SEND_MAIL_SENDER_ACCOUNT, 0);
    let before_target_revision = durable_revision_and_gold(&config, SEND_MAIL_TARGET_ACCOUNT, 0);
    let before_other_revision = durable_revision_and_gold(&config, SEND_MAIL_TARGET_ACCOUNT, 1);
    let before_session = {
        let state = sender.app.world().resource::<SessionResource>();
        (
            state.account_id.clone(),
            state.selected_character.clone(),
            state.active_save_revision(),
        )
    };
    assert!(before_world
        .inventory_items
        .iter()
        .any(|item| item.unique_id == attachment_id));

    let packets = sender.handle_packet(ClientPacket::SendMail {
        name: SEND_MAIL_TARGET_CHARACTER.to_string(),
        message: "must not cross the durable identity boundary".to_string(),
        gold: 1_000,
        items_idx: [attachment_id, 0, 0, 0, 0],
        stamped: false,
    });

    assert!(
        packets.is_empty(),
        "target durable identity mismatch must be protocol-silent: {packets:?}"
    );
    assert_collision_state_unchanged(
        &sender,
        &config,
        &path,
        &before_world,
        &before_store,
        &before_file,
        &before_session,
    );
    assert_eq!(
        snapshot_serialized_account(&config, SEND_MAIL_SENDER_ACCOUNT),
        before_sender,
        "rejected mail must not debit sender gold or attachments"
    );
    assert_eq!(
        snapshot_serialized_account(&config, SEND_MAIL_TARGET_ACCOUNT),
        before_target,
        "rejected mail must not alter either target save"
    );
    assert_eq!(
        account_identity_fingerprint(&config, SEND_MAIL_TARGET_ACCOUNT),
        before_target_identity
    );
    assert_eq!(
        durable_mailbox(&config, SEND_MAIL_TARGET_ACCOUNT, 0),
        before_target_mailbox,
        "rejected mail must not write an external mailbox"
    );
    assert_eq!(
        durable_revision_and_gold(&config, SEND_MAIL_SENDER_ACCOUNT, 0),
        before_sender_revision
    );
    assert_eq!(
        durable_revision_and_gold(&config, SEND_MAIL_TARGET_ACCOUNT, 0),
        before_target_revision
    );
    assert_eq!(
        durable_revision_and_gold(&config, SEND_MAIL_TARGET_ACCOUNT, 1),
        before_other_revision
    );

    fs::remove_dir_all(root).expect("send-mail fixture directory should be removed");
}

#[test]
fn canonical_cross_account_send_mail_commits_sender_and_exact_target() {
    let (config, mut sender, root, path, attachment_id) =
        send_mail_target_identity_fixture("canonical", false);
    let before_world = sender.world_snapshot();
    let before_file = fs::read(&path).expect("send-mail fixture file should exist");
    let before_target_identity = account_identity_fingerprint(&config, SEND_MAIL_TARGET_ACCOUNT);
    let (before_sender_revision, before_sender_gold) =
        durable_revision_and_gold(&config, SEND_MAIL_SENDER_ACCOUNT, 0);
    let (before_target_revision, _) =
        durable_revision_and_gold(&config, SEND_MAIL_TARGET_ACCOUNT, 0);
    let before_other_revision = durable_revision_and_gold(&config, SEND_MAIL_TARGET_ACCOUNT, 1);
    assert!(durable_mailbox(&config, SEND_MAIL_TARGET_ACCOUNT, 0).is_empty());
    assert!(durable_mailbox(&config, SEND_MAIL_TARGET_ACCOUNT, 1).is_empty());

    let packets = sender.handle_packet(ClientPacket::SendMail {
        name: SEND_MAIL_TARGET_CHARACTER.to_string(),
        message: "canonical cross-account delivery".to_string(),
        gold: 1_000,
        items_idx: [attachment_id, 0, 0, 0, 0],
        stamped: false,
    });

    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::MailSent { result: 1 })));
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 1_100 })));
    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::DeleteItem {
            unique_id,
            count: 1
        } if *unique_id == attachment_id
    )));
    let after_world = sender.world_snapshot();
    assert_eq!(after_world.gold, before_world.gold - 1_100);
    assert!(!after_world
        .inventory_items
        .iter()
        .any(|item| item.unique_id == attachment_id));

    let (after_sender_revision, after_sender_gold) =
        durable_revision_and_gold(&config, SEND_MAIL_SENDER_ACCOUNT, 0);
    let (after_target_revision, _) =
        durable_revision_and_gold(&config, SEND_MAIL_TARGET_ACCOUNT, 0);
    assert_eq!(after_sender_gold, before_sender_gold - 1_100);
    assert_eq!(
        after_sender_revision,
        before_sender_revision
            .checked_add(1)
            .expect("sender revision should not overflow")
    );
    assert_eq!(
        after_target_revision,
        before_target_revision
            .checked_add(1)
            .expect("target revision should not overflow")
    );
    assert_eq!(
        sender
            .app
            .world()
            .resource::<SessionResource>()
            .active_save_revision(),
        Some(after_sender_revision)
    );
    assert_eq!(
        durable_revision_and_gold(&config, SEND_MAIL_TARGET_ACCOUNT, 1),
        before_other_revision,
        "the other target character save must remain unchanged"
    );
    assert_eq!(
        account_identity_fingerprint(&config, SEND_MAIL_TARGET_ACCOUNT),
        before_target_identity,
        "a canonical delivery must preserve target roster/save identities"
    );

    let target_mailbox = durable_mailbox(&config, SEND_MAIL_TARGET_ACCOUNT, 0);
    assert_eq!(target_mailbox.len(), 1);
    let delivered = &target_mailbox[0];
    assert_eq!(delivered.from, SEND_MAIL_SENDER_CHARACTER);
    assert_eq!(delivered.to, SEND_MAIL_TARGET_CHARACTER);
    assert_eq!(delivered.body, "canonical cross-account delivery");
    assert_eq!(delivered.gold, 1_000);
    assert_eq!(delivered.items, vec!["dagger".to_string()]);
    assert_eq!(delivered.item_states_json.len(), 1);
    let delivered_item: serde_json::Value = serde_json::from_str(&delivered.item_states_json[0])
        .expect("delivered attachment state should decode");
    assert_eq!(
        delivered_item
            .get("unique_id")
            .and_then(serde_json::Value::as_u64),
        Some(attachment_id)
    );
    assert!(durable_mailbox(&config, SEND_MAIL_TARGET_ACCOUNT, 1).is_empty());
    assert_ne!(
        fs::read(&path).expect("canonical send-mail file should remain readable"),
        before_file,
        "canonical cross-account SendMail must publish durable file bytes"
    );

    fs::remove_dir_all(root).expect("send-mail fixture directory should be removed");
}

#[test]
fn malformed_empty_and_switched_identities_cannot_reuse_gm_or_leak_private_packets() {
    const GM_ACCOUNT: &str = "victim";
    const OTHER_ACCOUNT: &str = "other";
    const GM_CHARACTER: &str = "VictimHero";
    const OTHER_CHARACTER: &str = "OtherHero";

    let config = SimulationConfig::default();
    {
        let mut store = config.account_store.lock().unwrap();
        let seed = store
            .accounts
            .remove("demo")
            .expect("default account should seed the GM isolation fixture");
        let mut gm_account = seed.clone();
        gm_account.gm_level = 2;
        gm_account.characters[0].name = GM_CHARACTER.to_string();
        gm_account
            .saves
            .get_mut(&0)
            .expect("GM fixture save should exist")
            .character
            .name = GM_CHARACTER.to_string();

        let mut other_account = seed;
        other_account.gm_level = 0;
        other_account.characters[0].name = OTHER_CHARACTER.to_string();
        other_account
            .saves
            .get_mut(&0)
            .expect("other fixture save should exist")
            .character
            .name = OTHER_CHARACTER.to_string();

        store.accounts.insert(GM_ACCOUNT.to_string(), gm_account);
        store
            .accounts
            .insert(OTHER_ACCOUNT.to_string(), other_account);
    }

    let mut session = SimulationSession::new(config.clone());
    let login = session.handle_packet(ClientPacket::Login {
        account_id: GM_ACCOUNT.to_string(),
        password: "demo".to_string(),
    });
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let start = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(start
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    assert_eq!(
        session
            .app
            .world()
            .resource::<PlayerPermissionResource>()
            .gm_level,
        2
    );

    let before_store = snapshot_account_store(&config);
    let before_world = session.world_snapshot();
    let before_selected = session
        .app
        .world()
        .resource::<SessionResource>()
        .selected_character
        .clone();

    // Crystal consumes unauthorized @ commands silently
    // (../Crystal/Server/MirObjects/PlayerObject.cs:2152-2184). A malformed
    // identity must additionally invalidate the cached account-derived rank.
    for bound_identity in [" victim ", ""] {
        session
            .app
            .world_mut()
            .resource_mut::<SessionResource>()
            .account_id = Some(bound_identity.to_string());
        session
            .app
            .world_mut()
            .resource_mut::<PlayerPermissionResource>()
            .gm_level = 2;

        let gm_packets = session.handle_packet(ClientPacket::Chat {
            message: "@LEVEL 42".to_string(),
            linked_items: Vec::new(),
        });
        assert!(
            gm_packets.is_empty(),
            "invalid identity GM command must be completely silent: {gm_packets:?}"
        );
        assert_eq!(
            session
                .app
                .world()
                .resource::<PlayerPermissionResource>()
                .gm_level,
            0
        );

        // Crystal rejects ranking requests outside Game/Observer without a
        // response (../Crystal/Server/MirNetwork/MirConnection.cs:2182-2185).
        let ranking_packets = session.handle_packet(ClientPacket::GetRanking {
            rank_type: 0,
            rank_index: 0,
            online_only: false,
        });
        assert!(
            ranking_packets.is_empty(),
            "invalid identity ranking must not expose listing payload: {ranking_packets:?}"
        );
        assert_eq!(snapshot_account_store(&config), before_store);
        assert_eq!(session.world_snapshot(), before_world);
        assert_eq!(
            session
                .app
                .world()
                .resource::<SessionResource>()
                .selected_character,
            before_selected
        );
    }

    // Switching the bound key to another real, non-GM account must not carry
    // the old account's cached GM level into that account's session identity.
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .account_id = Some(OTHER_ACCOUNT.to_string());
    session
        .app
        .world_mut()
        .resource_mut::<PlayerPermissionResource>()
        .gm_level = 2;
    let switched_packets = session.handle_packet(ClientPacket::Chat {
        message: "@LEVEL 42".to_string(),
        linked_items: Vec::new(),
    });
    assert!(switched_packets.is_empty());
    let switched_ranking_packets = session.handle_packet(ClientPacket::GetRanking {
        rank_type: 0,
        rank_index: 0,
        online_only: false,
    });
    assert!(
        switched_ranking_packets.is_empty(),
        "a switched account that does not own the selected character must not receive rankings: {switched_ranking_packets:?}"
    );
    assert_eq!(
        session
            .app
            .world()
            .resource::<PlayerPermissionResource>()
            .gm_level,
        0
    );
    assert_eq!(snapshot_account_store(&config), before_store);
    assert_eq!(session.world_snapshot(), before_world);
    assert_eq!(
        session
            .app
            .world()
            .resource::<SessionResource>()
            .selected_character,
        before_selected
    );

    // Revoke the authoritative account rank while its old value is cached.
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .account_id = Some(GM_ACCOUNT.to_string());
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut(GM_ACCOUNT)
        .unwrap()
        .gm_level = 0;
    session
        .app
        .world_mut()
        .resource_mut::<PlayerPermissionResource>()
        .gm_level = 2;
    let revoked_store = snapshot_account_store(&config);
    let revoked_packets = session.handle_packet(ClientPacket::Chat {
        message: "@LEVEL 42".to_string(),
        linked_items: Vec::new(),
    });
    assert!(revoked_packets.is_empty());
    assert_eq!(
        session
            .app
            .world()
            .resource::<PlayerPermissionResource>()
            .gm_level,
        0
    );
    assert_eq!(snapshot_account_store(&config), revoked_store);
    assert_eq!(session.world_snapshot(), before_world);
    assert_eq!(
        session
            .app
            .world()
            .resource::<SessionResource>()
            .selected_character,
        before_selected
    );

    // Crystal LogOut is silent outside Game/Observer
    // (../Crystal/Server/MirNetwork/MirConnection.cs:1126-1152). A malformed
    // Select session must not return its previously cached CharacterSelectInfo.
    for bound_identity in [" victim ", ""] {
        let mut select_session = SimulationSession::new(config.clone());
        let login = select_session.handle_packet(ClientPacket::Login {
            account_id: GM_ACCOUNT.to_string(),
            password: "demo".to_string(),
        });
        assert!(login
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        assert!(select_session
            .app
            .world()
            .resource::<SessionResource>()
            .characters
            .iter()
            .any(|character| character.name == GM_CHARACTER));
        let select_store = snapshot_account_store(&config);
        let select_world = select_session.world_snapshot();
        let select_selected = select_session
            .app
            .world()
            .resource::<SessionResource>()
            .selected_character
            .clone();
        select_session
            .app
            .world_mut()
            .resource_mut::<SessionResource>()
            .account_id = Some(bound_identity.to_string());
        select_session
            .app
            .world_mut()
            .resource_mut::<PlayerPermissionResource>()
            .gm_level = 2;

        let logout_packets = select_session.handle_packet(ClientPacket::LogOut);
        assert!(
            logout_packets.is_empty(),
            "Select-stage LogOut must not expose an old roster: {logout_packets:?}"
        );
        assert!(select_session
            .app
            .world()
            .resource::<SessionResource>()
            .characters
            .is_empty());
        assert_eq!(
            select_session
                .app
                .world()
                .resource::<PlayerPermissionResource>()
                .gm_level,
            0
        );
        assert_eq!(snapshot_account_store(&config), select_store);
        assert_eq!(select_session.world_snapshot(), select_world);
        assert_eq!(
            select_session
                .app
                .world()
                .resource::<SessionResource>()
                .selected_character,
            select_selected
        );
    }
}

#[test]
fn legal_gm_and_ranking_paths_sync_exact_authorized_levels() {
    const CHARACTER: &str = "AuthorizedMage";
    let account_id = unique_account_id("gmsync");
    let config = SimulationConfig::default();
    seed_accounts_from_demo(&config, &[(account_id.as_str(), CHARACTER, 2)]);

    let mut session = SimulationSession::new(config.clone());
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: account_id.clone(),
            password: "demo".to_string(),
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert!(session
        .handle_packet(ClientPacket::StartGame { character_index: 0 })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));

    // A stale-low cache is raised to the exact durable rank before dispatch,
    // and the authorized GM command must still execute.
    session
        .app
        .world_mut()
        .resource_mut::<PlayerPermissionResource>()
        .gm_level = 0;
    let level_eight = send_chat(&mut session, "@LEVEL 8");
    assert!(level_eight
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LevelChanged { level: 8, .. })));
    assert_eq!(
        session
            .app
            .world()
            .resource::<PlayerPermissionResource>()
            .gm_level,
        2
    );

    // Downgrading the authoritative account rank synchronizes 2 -> 1 without
    // incorrectly denying a command that remains GM-authorized.
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut(&account_id)
        .unwrap()
        .gm_level = 1;
    let level_nine = send_chat(&mut session, "@LEVEL 9");
    assert!(level_nine
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LevelChanged { level: 9, .. })));
    assert_eq!(
        session
            .app
            .world()
            .resource::<PlayerPermissionResource>()
            .gm_level,
        1
    );

    let rankings = session.handle_packet(ClientPacket::GetRanking {
        rank_type: 0,
        rank_index: 0,
        online_only: false,
    });
    assert!(rankings.iter().any(|packet| matches!(
        packet,
        ServerPacket::Rankings { listing_details, .. }
            if listing_details.iter().any(|entry| entry.name == CHARACTER)
    )));

    // Revocation synchronizes 1 -> 0 and the gated command is consumed without
    // mutating the selected character.
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut(&account_id)
        .unwrap()
        .gm_level = 0;
    let denied = send_chat(&mut session, "@LEVEL 10");
    assert!(!denied
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LevelChanged { .. })));
    assert_eq!(
        session
            .app
            .world()
            .resource::<PlayerPermissionResource>()
            .gm_level,
        0
    );
    assert_eq!(
        session
            .app
            .world()
            .resource::<SessionResource>()
            .selected_character
            .as_ref()
            .map(|character| character.level),
        Some(9)
    );

    // A valid password grant contributes level 1, but the durable level 2 wins
    // the max(store/env, password) merge after the Crystal resolver sets 1.
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut(&account_id)
        .unwrap()
        .gm_level = 2;
    session
        .app
        .world_mut()
        .resource_mut::<GmRuntimeResource>()
        .password = Some("exact-gm-password".to_string());
    let prompt = send_chat(&mut session, "@LOGIN");
    assert!(prompt.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectChat { text, .. } if text.contains("GM Password")
    )));
    let granted = send_chat(&mut session, "exact-gm-password");
    assert!(granted.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectChat { text, .. } if text.contains("made a GM")
    )));
    assert_eq!(
        session
            .app
            .world()
            .resource::<PlayerPermissionResource>()
            .gm_level,
        2
    );
}

#[test]
fn gm_password_binding_login_switch_and_successful_disconnect_clear_permissions() {
    const SHARED_CHARACTER: &str = "PasswordTwin";
    let account_a = unique_account_id("gmba");
    let account_b = unique_account_id("gmbb");
    let config = SimulationConfig::default();
    seed_accounts_from_demo(
        &config,
        &[
            (account_a.as_str(), SHARED_CHARACTER, 0),
            (account_b.as_str(), SHARED_CHARACTER, 0),
        ],
    );

    let mut session = SimulationSession::new(config.clone());
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: account_a.clone(),
            password: "demo".to_string(),
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert!(session
        .handle_packet(ClientPacket::StartGame { character_index: 0 })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    session
        .app
        .world_mut()
        .resource_mut::<GmRuntimeResource>()
        .password = Some("identity-bound-password".to_string());

    let first_prompt = send_chat(&mut session, "@LOGIN");
    assert!(!first_prompt.is_empty());
    assert!(
        session
            .app
            .world()
            .resource::<GmRuntimeResource>()
            .login_pending
    );

    // A and B deliberately own the same index+name. The next line still cannot
    // consume A's prompt after only the exact account identity changes to B.
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .account_id = Some(account_b.clone());
    let rejected_password = send_chat(&mut session, "identity-bound-password");
    assert!(rejected_password.is_empty());
    assert_eq!(
        session
            .app
            .world()
            .resource::<PlayerPermissionResource>()
            .gm_level,
        0
    );
    assert!(
        !session
            .app
            .world()
            .resource::<GmRuntimeResource>()
            .login_pending
    );

    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .account_id = Some(account_a.clone());
    assert!(!send_chat(&mut session, "@LOGIN").is_empty());
    let granted = send_chat(&mut session, "identity-bound-password");
    assert!(!granted.is_empty());
    assert_eq!(
        session
            .app
            .world()
            .resource::<PlayerPermissionResource>()
            .gm_level,
        1
    );
    assert!(!send_chat(&mut session, "@SUPERMAN").is_empty());
    assert!(
        session
            .app
            .world()
            .resource::<GmRuntimeResource>()
            .gm_never_die
    );

    // A password-derived authorization is bound just as strictly as the
    // prompt. Switching only the account ID to B must clear both rank and GM
    // runtime flags even though B owns the same character index+name.
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .account_id = Some(account_b.clone());
    let mismatched_authorization = send_chat(&mut session, "@LEVEL 30");
    assert!(!mismatched_authorization
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LevelChanged { .. })));
    assert_eq!(
        session
            .app
            .world()
            .resource::<PlayerPermissionResource>()
            .gm_level,
        0
    );
    assert!(
        !session
            .app
            .world()
            .resource::<GmRuntimeResource>()
            .gm_never_die
    );

    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .account_id = Some(account_a.clone());
    assert!(!send_chat(&mut session, "@LOGIN").is_empty());
    assert!(!send_chat(&mut session, "identity-bound-password").is_empty());
    assert!(!send_chat(&mut session, "@SUPERMAN").is_empty());
    assert!(
        session
            .app
            .world()
            .resource::<GmRuntimeResource>()
            .gm_never_die
    );

    // A successful account Login is a new identity boundary and must clear the
    // password grant and every mutable GM runtime flag before binding B.
    let switched_login = session.handle_packet(ClientPacket::Login {
        account_id: account_b.clone(),
        password: "demo".to_string(),
    });
    assert!(switched_login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert_eq!(
        session
            .app
            .world()
            .resource::<PlayerPermissionResource>()
            .gm_level,
        0
    );
    assert!(
        !session
            .app
            .world()
            .resource::<GmRuntimeResource>()
            .gm_never_die
    );
    assert!(session
        .app
        .world()
        .resource::<SessionResource>()
        .selected_character
        .is_none());

    assert!(session
        .handle_packet(ClientPacket::StartGame { character_index: 0 })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    assert!(!send_chat(&mut session, "@LOGIN").is_empty());
    assert!(!send_chat(&mut session, "identity-bound-password").is_empty());
    assert_eq!(
        session
            .app
            .world()
            .resource::<PlayerPermissionResource>()
            .gm_level,
        1
    );
    assert!(!send_chat(&mut session, "@SUPERMAN").is_empty());

    let disconnected = session.handle_packet(ClientPacket::Disconnect);
    assert!(matches!(
        disconnected.as_slice(),
        [ServerPacket::Disconnect { reason: 0 }]
    ));
    assert_eq!(
        session
            .app
            .world()
            .resource::<PlayerPermissionResource>()
            .gm_level,
        0
    );
    let gm = session.app.world().resource::<GmRuntimeResource>();
    assert!(!gm.login_pending);
    assert!(!gm.gm_never_die);
}

#[test]
fn disconnect_persistence_failure_still_clears_gm_permissions() {
    const CHARACTER: &str = "DisconnectGm";
    let account_id = unique_account_id("gmdisc");
    let config = SimulationConfig::default();
    seed_accounts_from_demo(&config, &[(account_id.as_str(), CHARACTER, 2)]);
    let mut session = SimulationSession::new(config.clone());
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id,
            password: "demo".to_string(),
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert!(session
        .handle_packet(ClientPacket::StartGame { character_index: 0 })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    assert!(!send_chat(&mut session, "@SUPERMAN").is_empty());
    assert!(
        session
            .app
            .world()
            .resource::<GmRuntimeResource>()
            .gm_never_die
    );

    let before_store = snapshot_account_store(&config);
    let before_world = session.world_snapshot();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::Persist);
    let disconnected = session.handle_packet(ClientPacket::Disconnect);

    assert!(disconnected.is_empty());
    assert_eq!(snapshot_account_store(&config), before_store);
    assert_eq!(session.world_snapshot(), before_world);
    assert_eq!(
        session
            .app
            .world()
            .resource::<PlayerPermissionResource>()
            .gm_level,
        0
    );
    let gm = session.app.world().resource::<GmRuntimeResource>();
    assert!(!gm.login_pending);
    assert!(!gm.gm_never_die);
}

#[test]
fn trusted_select_account_for_recovery_still_starts_game() {
    let config = SimulationConfig::default();
    let mut session = SimulationSession::new(config.clone());

    session
        .select_account_for_recovery("demo")
        .expect("trusted recovery selection should set account");
    assert!(session
        .app
        .world()
        .resource::<SessionResource>()
        .selected_character
        .is_none());

    let packets = session.handle_packet(ClientPacket::StartGame { character_index: 0 });

    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
}

#[test]
fn standard_and_passkey_recovery_preflight_fail_closed_for_source_states() {
    let config = SimulationConfig::default();
    assert_eq!(
        recovery_login_preflight_after_source_refresh(
            &config,
            "demo",
            RecoveryCredential::Standard("demo"),
            Ok(AccountSourceRefreshOutcome::Missing),
        )
        .unwrap(),
        RecoveryLoginPreflight::Missing
    );
    assert_eq!(
        recovery_login_preflight_after_source_refresh(
            &config,
            "demo",
            RecoveryCredential::Passkey,
            Ok(AccountSourceRefreshOutcome::Missing),
        )
        .unwrap(),
        RecoveryLoginPreflight::Missing
    );
    assert!(recovery_login_preflight_after_source_refresh(
        &config,
        "demo",
        RecoveryCredential::Standard("demo"),
        Err("injected source unavailable".to_string()),
    )
    .is_err());
    assert!(recovery_login_preflight_after_source_refresh(
        &config,
        "demo",
        RecoveryCredential::Passkey,
        Err("injected source unavailable".to_string()),
    )
    .is_err());

    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .is_banned = true;
    assert!(matches!(
        recovery_login_preflight_after_source_refresh(
            &config,
            "demo",
            RecoveryCredential::Standard("demo"),
            Ok(AccountSourceRefreshOutcome::Refreshed),
        )
        .unwrap(),
        RecoveryLoginPreflight::Banned(_)
    ));
    assert!(matches!(
        recovery_login_preflight_after_source_refresh(
            &config,
            "demo",
            RecoveryCredential::Passkey,
            Ok(AccountSourceRefreshOutcome::Refreshed),
        )
        .unwrap(),
        RecoveryLoginPreflight::Banned(_)
    ));
}

#[test]
fn first_time_passkey_provision_is_durable_and_locked() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "mir2-passkey-provision-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("accounts.json");
    let config = SimulationConfig::default().with_account_store_path(&path);
    let default_character = config.default_character.clone();
    let account_id = format!("wallet:first-{unique}");
    SimulationSession::provision_passkey_account(&config, &account_id)
        .expect("trusted setup should provision the passkey account durably");
    let mut session = SimulationSession::new(config.clone());

    let packets = session.passkey_login(&account_id);

    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let persisted = AccountStore::load_or_new(&path, default_character);
    let account = persisted.accounts.get(&account_id).unwrap();
    assert_ne!(account.password, "demo");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn passkey_provision_persistence_failure_rolls_back_live_account() {
    let config = SimulationConfig::default();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::Persist);
    let account_id = "wallet:must-not-survive-failed-provision";

    assert!(SimulationSession::provision_passkey_account(&config, account_id).is_err());
    assert!(!config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .contains_key(account_id));
}

#[test]
fn existing_only_passkey_login_does_not_provision_missing_account() {
    let config = SimulationConfig::default();
    let account_id = "wallet:existing-only-missing";
    let mut session = SimulationSession::new(config.clone());

    let packets = session.passkey_login(account_id);

    assert!(packets
        .iter()
        .all(|packet| !matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert!(!config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .contains_key(account_id));
}

#[test]
fn account_creation_persistence_failure_returns_crystal_failure_and_rolls_back_live_store() {
    let config = SimulationConfig::default();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::Persist);
    let account_id = "durable_user";

    assert_eq!(
        create_account_with_password(&config, account_id, "DurablePassword!234"),
        0
    );
    assert!(!config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .contains_key(account_id));
}

#[test]
fn password_change_persistence_failure_returns_crystal_failure_and_keeps_old_hash() {
    let config = SimulationConfig::default();
    let before = config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get("demo")
        .unwrap()
        .password
        .clone();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::Persist);

    assert_eq!(
        change_account_password(config.clone(), "demo", "demo", "ChangedPassword!234",),
        0
    );
    assert_eq!(
        config
            .account_store
            .lock()
            .unwrap()
            .accounts
            .get("demo")
            .unwrap()
            .password,
        before
    );
}

#[test]
fn recovery_password_reset_persistence_failure_keeps_old_hash() {
    let config = SimulationConfig::default();
    let before = config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get("demo")
        .unwrap()
        .password
        .clone();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::Persist);

    assert!(
        reset_account_password_after_recovery(&config, "demo", "RecoveredPassword!234",).is_err()
    );
    assert_eq!(
        config
            .account_store
            .lock()
            .unwrap()
            .accounts
            .get("demo")
            .unwrap()
            .password,
        before
    );
}

#[test]
fn new_character_persistence_failure_returns_crystal_failure_and_keeps_roster() {
    let config = SimulationConfig::default();
    let template = config.default_character.clone();
    let mut session = SimulationSession::new(config.clone());
    let login = session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let before = config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get("demo")
        .unwrap()
        .characters
        .clone();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::Persist);

    let packets = session.handle_packet(ClientPacket::NewCharacter {
        name: "MustNotPersist".to_string(),
        gender: template.gender,
        class: template.class,
    });

    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::NewCharacter { result: 1 })));
    assert_eq!(
        config
            .account_store
            .lock()
            .unwrap()
            .accounts
            .get("demo")
            .unwrap()
            .characters,
        before
    );
}

#[test]
fn delete_character_persistence_failure_keeps_character() {
    let config = SimulationConfig::default();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::Persist);

    assert!(delete_character_from_account(&config, "demo", 0).is_err());
    assert!(config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get("demo")
        .unwrap()
        .characters
        .iter()
        .any(|character| character.index == 0));
}

#[test]
fn local_missing_account_start_returns_none_without_recreating_account() {
    let config = SimulationConfig::default();
    config.account_store.lock().unwrap().accounts.remove("demo");

    assert!(character_save_for_start(&config, "demo", 0)
        .unwrap()
        .is_none());
    assert!(!config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .contains_key("demo"));
}

#[test]
fn source_of_truth_start_requires_authoritative_refresh_and_does_not_recreate_account() {
    let mut config = SimulationConfig::default();
    config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
    config.account_store.lock().unwrap().accounts.remove("demo");

    assert!(character_save_for_start(&config, "demo", 0).is_err());
    assert!(!config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .contains_key("demo"));
}

#[test]
fn start_normalization_persistence_failure_returns_crystal_failure_and_keeps_live_save() {
    let config = SimulationConfig::default();
    {
        let mut store = config.account_store.lock().unwrap();
        let save = store
            .accounts
            .get_mut("demo")
            .unwrap()
            .saves
            .get_mut(&0)
            .unwrap();
        save.hp = 120;
        save.max_hp = 120;
        save.mp = 45;
    }
    let mut session = SimulationSession::new(config.clone());
    let login = session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::Persist);

    let packets = session.handle_packet(ClientPacket::StartGame { character_index: 0 });

    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::StartGame {
            result: 2,
            resolution: 0
        }
    )));
    let store = config.account_store.lock().unwrap();
    let save = store.accounts.get("demo").unwrap().saves.get(&0).unwrap();
    assert_eq!((save.hp, save.max_hp, save.mp), (120, 120, 45));
}

#[test]
fn legacy_password_migration_persistence_failure_rejects_login_and_keeps_live_password() {
    let config = SimulationConfig::default();
    assert_eq!(
        config
            .account_store
            .lock()
            .unwrap()
            .accounts
            .get("demo")
            .unwrap()
            .password,
        "demo"
    );
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::Persist);
    let mut session = SimulationSession::new(config.clone());

    let packets = session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });

    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::Login { result: 4 })));
    assert_eq!(
        config
            .account_store
            .lock()
            .unwrap()
            .accounts
            .get("demo")
            .unwrap()
            .password,
        "demo"
    );
}
