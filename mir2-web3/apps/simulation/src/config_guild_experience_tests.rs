use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "config_guild_experience_capture_tests.rs"]
mod capture_tests;

#[test]
fn guild_xp_unknown_repository_commit_freezes_all_file_writers_without_compensation() {
    let (config, path, id, identity) = fixture();
    let second = SimulationConfig::default().with_account_store_path(&path);
    let source = pending(&config, &id, &identity, 100);
    let original = fs::read(&path).unwrap();
    let config =
        config.with_account_store_database_url("postgresql://guild-probe@127.0.0.1:1/guild_probe");
    config.inject_account_store_repository_writer_probe(Err(format!(
        "{GUILD_COMMIT_OUTCOME_UNKNOWN}: response lost after COMMIT"
    )));
    assert!(commit(&config, &identity, source)
        .unwrap_err()
        .contains(GUILD_COMMIT_OUTCOME_UNKNOWN));
    assert_eq!(
        config.account_store_repository_writer_probe_invocations(),
        1
    );
    assert!(config.ensure_account_store_writable().is_err());
    assert!(second.save_account_store().is_err());
    assert!(second
        .commit_account_store_transaction(&[identity.account_id.clone()], |store| {
            store
                .accounts
                .get_mut(&identity.account_id)
                .unwrap()
                .password = "must-not-persist".into();
            Ok(())
        })
        .is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(
        config.account_store_repository_writer_probe_invocations(),
        1
    );
    drop(config);
    drop(second);
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
fn fixture() -> (SimulationConfig, PathBuf, String, Stage5FriendIdentity) {
    let path = std::env::temp_dir()
        .join(format!(
            "mir2-guild-xp-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
        .join("accounts.json");
    let config = SimulationConfig::default().with_account_store_path(&path);
    let mut store = config.account_store.lock().unwrap();
    let mut guild = guild_clock::schema_tests::fixture_guild(&store);
    guild.members[0].membership_epoch = 1;
    let identity = guild.members[0].identity.clone();
    store
        .accounts
        .get_mut(&identity.account_id)
        .unwrap()
        .saves
        .entry(identity.character_index)
        .or_insert_with(|| CharacterSaveRecord::new(config.default_character.clone()));
    let id = guild.id.clone();
    store.shared_guilds.insert(id.clone(), guild);
    drop(store);
    config.save_account_store().unwrap();
    (config, path, id, identity)
}
fn pending(
    config: &SimulationConfig,
    id: &str,
    identity: &Stage5FriendIdentity,
    amount: u32,
) -> CharacterSaveRecord {
    let store = config.account_store.lock().unwrap();
    let mut save = store.accounts[&identity.account_id].saves[&identity.character_index].clone();
    let sequence = save.guild_experience_journal.next_sequence + 1;
    let event = GuildExperienceEvent {
        sequence,
        source_revision: save.revision,
        event_id: serde_json::to_string(&(
            "guild-xp-v1",
            &identity.account_id,
            identity.character_index,
            sequence,
        ))
        .unwrap(),
        identity: identity.clone(),
        guild_id: id.into(),
        membership_epoch: store.shared_guilds[id].members[0].membership_epoch,
        final_amount: amount,
        capture_hash: None,
    };
    save.guild_experience_journal.next_sequence = sequence;
    save.guild_experience_journal.pending.push(event);
    save.experience += i64::from(amount);
    save.gold -= 1;
    save.revision += 1;
    save
}
fn commit(
    config: &SimulationConfig,
    identity: &Stage5FriendIdentity,
    save: CharacterSaveRecord,
) -> Result<(), String> {
    config.commit_account_store_transaction(&[identity.account_id.clone()], |store| {
        let old = store.accounts[&identity.account_id].saves[&identity.character_index].revision;
        if save.revision != old + 1 {
            return Err("test source CAS rejected".into());
        }
        store
            .accounts
            .get_mut(&identity.account_id)
            .unwrap()
            .saves
            .insert(identity.character_index, save);
        Ok(())
    })
}
#[test]
fn guild_xp_file_transaction_commits_source_and_receipt_atomically_and_rejects_payload_reuse() {
    let (config, path, id, identity) = fixture();
    let source = pending(&config, &id, &identity, 100);
    let before = fs::read(&path).unwrap();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforeFileRename);
    assert!(commit(&config, &identity, source.clone()).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(
        config.account_store.lock().unwrap().shared_guilds[&id].experience,
        0
    );
    commit(&config, &identity, source.clone()).unwrap();
    let mut replay = source.clone();
    replay.revision += 1;
    // An old pending projection with an identical payload is cleared, not paid twice.
    commit(&config, &identity, replay.clone()).unwrap();
    replay.revision += 1;
    replay.guild_experience_journal.pending[0].final_amount += 100;
    assert!(commit(&config, &identity, replay)
        .unwrap_err()
        .contains("payload mismatch"));
    let file = FileAccountStoreRepository::new(&path)
        .load(config.default_character.clone())
        .unwrap();
    assert_eq!(file.shared_guilds[&id].experience, 1);
    assert_eq!(file.shared_guilds[&id].experience_receipt_payloads.len(), 1);
    let saved = &file.accounts[&identity.account_id].saves[&identity.character_index];
    assert_eq!(saved.experience, source.experience);
    assert!(saved.guild_experience_journal.pending.is_empty());
    assert_eq!(saved.guild_experience_journal.event_payloads.len(), 1);
    drop(config);
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
#[test]
fn guild_xp_original_scope_cannot_escape_via_journal_and_import_cannot_award() {
    let (config, path, id, identity) = fixture();
    let source = pending(&config, &id, &identity, 100);
    let before = fs::read(&path).unwrap();
    let error = config
        .commit_account_store_transaction(&[identity.account_id.clone()], |store| {
            store.shared_guilds.get_mut(&id).unwrap().gold += 999;
            store
                .accounts
                .get_mut(&identity.account_id)
                .unwrap()
                .saves
                .insert(identity.character_index, source.clone());
            Ok(())
        })
        .unwrap_err();
    assert!(error.contains("outside the authorized scope"));
    assert_eq!(fs::read(&path).unwrap(), before);
    let mut backup = config.account_store.lock().unwrap().clone();
    backup
        .accounts
        .get_mut(&identity.account_id)
        .unwrap()
        .saves
        .insert(identity.character_index, source);
    let backup_path = path.parent().unwrap().join("backup.json");
    backup.save_to_path(&backup_path).unwrap();
    assert!(config
        .restore_account_store_from_backup(&backup_path)
        .unwrap_err()
        .contains("unsettled guild XP"));
    assert_eq!(fs::read(&path).unwrap(), before);
    drop(config);
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
#[test]
fn guild_xp_membership_epoch_change_rejects_old_pending_source_without_asset_change() {
    let (config, path, id, identity) = fixture();
    let source = pending(&config, &id, &identity, 100);
    config
        .commit_account_store_transaction_with_guilds(
            &[identity.account_id.clone()],
            &[id.clone()],
            |store| {
                let guild = store.shared_guilds.get_mut(&id).unwrap();
                guild.revision += 1;
                guild.members[0].membership_epoch += 1;
                Ok(())
            },
        )
        .unwrap();
    let before = fs::read(&path).unwrap();
    assert!(commit(&config, &identity, source)
        .unwrap_err()
        .contains("membership epoch"));
    assert_eq!(fs::read(&path).unwrap(), before);
    drop(config);
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
