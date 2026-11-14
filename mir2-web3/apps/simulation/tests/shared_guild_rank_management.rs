//! Direct durable authority tests on isolated prepared stores.
//! Ordinary typed packet RED/GREEN is separate in the Gateway test target.
use mir2_protocol::{MirClass, MirGender};
use mir2_simulation::{
    AccountRecord, CharacterRecord, CharacterSaveRecord, SharedGuildMember, SharedGuildRank,
    SharedGuildRecord, SimulationConfig, Stage5FriendIdentity,
};
use std::collections::BTreeMap;

const GUILD_ID: &str = "1123456789abcdef0123456789abcdef";
const OTHER_GUILD_ID: &str = "2123456789abcdef0123456789abcdef";

fn identity(account_id: &str, character_index: i32) -> Stage5FriendIdentity {
    Stage5FriendIdentity {
        account_id: account_id.into(),
        character_index,
    }
}

fn fixture() -> SimulationConfig {
    let config = SimulationConfig::default();
    let mut store = config.account_store.lock().unwrap();
    store.accounts.clear();
    for (index, account, name) in [
        (0, "leader", "Leader"),
        (1, "officer", "Officer"),
        (2, "member", "Member"),
        (3, "foreign", "Foreign"),
    ] {
        let character = CharacterRecord {
            index,
            name: name.into(),
            level: 22,
            class: MirClass::Warrior,
            gender: MirGender::Male,
        };
        let mut record = AccountRecord::empty();
        record.characters.push(character.clone());
        record
            .saves
            .insert(index, CharacterSaveRecord::new(character));
        store.accounts.insert(account.into(), record);
    }
    let guild = SharedGuildRecord {
        id: GUILD_ID.into(),
        active_wars: Default::default(),
        name: "Rank Guild".into(),
        revision: 7,
        level: 0,
        experience: 41,
        spare_points: 2,
        gold: 99_123,
        ranks: vec![
            SharedGuildRank {
                index: 0,
                name: "Leader".into(),
                options: 255,
            },
            SharedGuildRank {
                index: 1,
                name: "Officer".into(),
                options: 1,
            },
            SharedGuildRank {
                index: 2,
                name: "Members".into(),
                options: 0,
            },
        ],
        members: vec![
            SharedGuildMember {
                membership_epoch: 4,
                identity: identity("leader", 0),
                name: "Leader".into(),
                rank_index: 0,
            },
            SharedGuildMember {
                membership_epoch: 5,
                identity: identity("officer", 1),
                name: "Officer".into(),
                rank_index: 1,
            },
            SharedGuildMember {
                membership_epoch: 6,
                identity: identity("member", 2),
                name: "Member".into(),
                rank_index: 2,
            },
        ],
        notice: vec!["unchanged notice".into()],
        storage: BTreeMap::new(),
        buffs: BTreeMap::new(),
        last_buff_tick_ms: 0,
        experience_receipts: Default::default(),
        experience_receipt_payloads: Default::default(),
    };
    let mut foreign = guild.clone();
    foreign.id = OTHER_GUILD_ID.into();
    foreign.name = "Foreign Guild".into();
    foreign.members = vec![SharedGuildMember {
        membership_epoch: 2,
        identity: identity("foreign", 3),
        name: "Foreign".into(),
        rank_index: 0,
    }];
    store.shared_guilds.insert(GUILD_ID.into(), guild);
    store.shared_guilds.insert(OTHER_GUILD_ID.into(), foreign);
    drop(store);
    config
}

fn stored(config: &SimulationConfig) -> SharedGuildRecord {
    config.account_store.lock().unwrap().shared_guilds[GUILD_ID].clone()
}

#[test]
fn committed_rank_name_changes_only_selected_name_and_one_revision_in_complete_store() {
    let config = fixture();
    let mut expected = config.account_store.lock().unwrap().clone();
    let guild = expected.shared_guilds.get_mut(GUILD_ID).unwrap();
    guild.ranks[2].name = "Veterans".into();
    guild.revision += 1;
    let committed = config
        .commit_shared_guild_rank_name(&identity("leader", 0), 2, "Veterans")
        .unwrap();
    assert_eq!(committed, expected.shared_guilds[GUILD_ID]);
    assert_eq!(
        serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}

#[test]
fn direct_authority_rejects_wrong_character_missing_character_rank_permission_and_target() {
    let config = fixture();
    let before = stored(&config);
    for actor in [
        identity("leader", 2),
        identity("missing", 0),
        identity("member", 2),
    ] {
        assert!(config
            .commit_shared_guild_rank_name(&actor, 2, "Forbidden")
            .is_err());
        assert_eq!(stored(&config), before);
    }
    assert!(config
        .commit_shared_guild_rank_name(&identity("leader", 0), 255, "Unknown")
        .is_err());
    assert!(config
        .commit_shared_guild_rank_name(&identity("officer", 1), 0, "Superior")
        .is_err());
    assert_eq!(stored(&config), before);
    // Fixture-only orphan tests the character check without introducing a production deletion API.
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("leader")
        .unwrap()
        .characters
        .clear();
    assert!(config
        .commit_shared_guild_rank_name(&identity("leader", 0), 2, "Orphaned")
        .is_err());
    assert_eq!(stored(&config), before);
}

#[test]
fn revision_exhaustion_rejects_without_modifying_name_or_epoch() {
    let config = fixture();
    config
        .account_store
        .lock()
        .unwrap()
        .shared_guilds
        .get_mut(GUILD_ID)
        .unwrap()
        .revision = u64::MAX;
    let before = stored(&config);
    let error = config
        .commit_shared_guild_rank_name(&identity("leader", 0), 2, "Overflow")
        .unwrap_err();
    assert!(error.contains("exhausted"), "unexpected refusal: {error}");
    assert_eq!(stored(&config), before);
}

#[cfg(feature = "test-support")]
fn file_fixture() -> (SimulationConfig, std::path::PathBuf) {
    let config = fixture();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "mir2-rank-authority-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("accounts.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&*config.account_store.lock().unwrap()).unwrap(),
    )
    .unwrap();
    eprintln!("isolated rank authority fixture: {}", path.display());
    (config.with_account_store_path(path.clone()), path)
}

#[cfg(feature = "test-support")]
#[test]
fn known_file_publication_refusal_preserves_live_and_file_then_retry_advances_once() {
    use mir2_simulation::AccountStoreTransactionFault;
    let (config, path) = file_fixture();
    let before = stored(&config);
    let bytes = std::fs::read(&path).unwrap();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforeFileRename);
    assert!(config
        .commit_shared_guild_rank_name(&identity("leader", 0), 2, "Committed")
        .is_err());
    assert_eq!(stored(&config), before);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let after = config
        .commit_shared_guild_rank_name(&identity("leader", 0), 2, "Committed")
        .unwrap();
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(after.members, before.members);
    drop(config);
    let reopened = SimulationConfig::default().with_account_store_path(path);
    assert_eq!(stored(&reopened), after);
}

#[cfg(feature = "test-support")]
#[test]
fn unknown_file_publication_has_no_success_and_freezes_rank_and_notice_after_reopen() {
    use mir2_simulation::AccountStoreTransactionFault;
    let (config, path) = file_fixture();
    let before = stored(&config);
    config.inject_account_store_transaction_fault(
        AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );
    let result = config.commit_shared_guild_rank_name(&identity("leader", 0), 2, "Uncertain");
    assert!(result.is_err(), "unknown commit must not report success");
    assert_eq!(
        stored(&config),
        before,
        "unconfirmed staged state must not become live authority"
    );
    let uncertain_bytes = std::fs::read(&path).unwrap();
    let uncertain: serde_json::Value = serde_json::from_slice(&uncertain_bytes).unwrap();
    assert_eq!(
        uncertain["sharedGuilds"][GUILD_ID]["ranks"][2]["name"],
        "Uncertain"
    );
    assert!(config
        .commit_shared_guild_rank_name(&identity("leader", 0), 2, "GuessedRetry")
        .is_err());
    assert!(config
        .commit_shared_guild_notice(&identity("leader", 0), &["GuessedNotice".into()])
        .is_err());
    assert_eq!(std::fs::read(&path).unwrap(), uncertain_bytes);
    assert_eq!(stored(&config), before);
    drop(config);
    let reopened = SimulationConfig::default().with_account_store_path(path.clone());
    assert!(reopened
        .commit_shared_guild_rank_name(&identity("leader", 0), 2, "RestartRetry")
        .is_err());
    assert!(reopened
        .commit_shared_guild_notice(&identity("leader", 0), &["RestartNotice".into()])
        .is_err());
    assert_eq!(std::fs::read(&path).unwrap(), uncertain_bytes);
    // No automatic reconciliation or rollback is exercised; uncertain files are retained.
}
