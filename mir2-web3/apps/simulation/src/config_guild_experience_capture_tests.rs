use super::*;
use crate::runtime::zone::{ZoneExperienceSelection, ZoneGuildExperienceMembership};

#[test]
fn guild_xp_no_guild_kill_source_still_freezes_on_unknown_account_only_publication() {
    let (config, path, id, identity) = fixture();
    let mut source = pending(&config, &id, &identity, 100);
    source.guild_experience_journal.pending.clear();
    source.guild_experience_journal.next_sequence = 0;
    let permit = GuildExperienceCommitPermit::from_verified_shared_kill(
        identity.clone(), "legacy/no-guild/kill".into(), "b".repeat(64), None,
    ).unwrap();
    source.guild_experience_journal.applied_kill_receipts.insert(permit.kill_key.clone(), permit.kill_payload_hash.clone());
    config.account_store.lock().unwrap().shared_guilds.clear();
    config.save_account_store().unwrap();
    let original = fs::read(&path).unwrap();
    let uncertain = config.clone().with_account_store_database_url("postgresql://xp-probe@127.0.0.1:1/xp_probe");
    uncertain.inject_guild_xp_unknown_publication_probe();
    assert!(authorized_commit(&uncertain, &permit, source).unwrap_err().contains(GUILD_COMMIT_OUTCOME_UNKNOWN));
    assert_eq!(uncertain.guild_xp_publication_probe_attempts(), 1);
    assert!(config.save_account_store().is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    drop(uncertain);
    drop(config);
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

fn captured(
    config: &SimulationConfig,
    id: &str,
    identity: &Stage5FriendIdentity,
) -> (CharacterSaveRecord, GuildExperienceCommitPermit) {
    let mut source = pending(config, id, identity, 100);
    let selection = ZoneExperienceSelection {
        source_zone: "crystal:0/channel:0".into(),
        monster_object_id: 700,
        killed_at_ms: 1234,
        account_id: identity.account_id.clone(),
        character_index: identity.character_index,
        guild: Some(ZoneGuildExperienceMembership {
            guild_id: id.into(),
            membership_epoch: 1,
        }),
        final_amount: 100,
        guild_amount: 7,
    };
    let permit = GuildExperienceCommitPermit::from_verified_shared_kill(
        identity.clone(),
        "zone/kill/700/1234".into(),
        "a".repeat(64),
        Some(selection),
    )
    .unwrap();
    source.guild_experience_journal.pending[0].capture_hash = permit.selection_hash.clone();
    source
        .guild_experience_journal
        .applied_kill_receipts
        .insert(permit.kill_key.clone(), permit.kill_payload_hash.clone());
    (source, permit)
}
fn authorized_commit(
    config: &SimulationConfig,
    permit: &GuildExperienceCommitPermit,
    source: CharacterSaveRecord,
) -> Result<(), String> {
    config.commit_account_store_transaction_with_guild_experience_permit(
        &[permit.identity.account_id.clone()],
        permit,
        |store| {
            let account = store.accounts.get_mut(&permit.identity.account_id).unwrap();
            if account.saves[&permit.identity.character_index].revision + 1 != source.revision {
                return Err("capture test source CAS".into());
            }
            account
                .saves
                .insert(permit.identity.character_index, source);
            Ok(())
        },
    )
}

#[test]
fn guild_xp_captured_retry_uses_original_epoch_and_rounded_amount_once() {
    let (config, path, id, identity) = fixture();
    let (source, permit) = captured(&config, &id, &identity);
    let before = fs::read(&path).unwrap();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforeFileRename);
    assert!(authorized_commit(&config, &permit, source.clone()).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    // A later rejoin must not discard or reprice the already selected reward.
    {
        let mut store = config.account_store.lock().unwrap();
        let guild = store.shared_guilds.get_mut(&id).unwrap();
        guild.members[0].membership_epoch = 2;
        guild.revision += 1;
    }
    config.save_account_store().unwrap();
    authorized_commit(&config, &permit, source.clone()).unwrap();
    let mut replay = source;
    replay.revision += 1;
    authorized_commit(&config, &permit, replay).unwrap();
    let saved = FileAccountStoreRepository::new(&path)
        .load(config.default_character.clone())
        .unwrap();
    assert_eq!(saved.shared_guilds[&id].experience, 7);
    let journal = &saved.accounts[&identity.account_id].saves[&identity.character_index]
        .guild_experience_journal;
    assert_eq!(
        journal.kill_outcomes[&permit.kill_key],
        GuildExperienceKillOutcome::Credited { guild_amount: 7 }
    );
    assert_eq!(journal.event_payloads.len(), 1);
    assert!(journal.pending.is_empty());
    drop(config);
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn guild_xp_captured_disband_records_skip_without_recreating_guild() {
    let (config, path, id, identity) = fixture();
    let (source, permit) = captured(&config, &id, &identity);
    config
        .account_store
        .lock()
        .unwrap()
        .shared_guilds
        .remove(&id);
    config.save_account_store().unwrap();
    let expected_exp = source.experience;
    authorized_commit(&config, &permit, source).unwrap();
    let saved = FileAccountStoreRepository::new(&path)
        .load(config.default_character.clone())
        .unwrap();
    assert!(!saved.shared_guilds.contains_key(&id));
    let source = &saved.accounts[&identity.account_id].saves[&identity.character_index];
    assert_eq!(source.experience, expected_exp);
    assert_eq!(
        source.guild_experience_journal.kill_outcomes[&permit.kill_key],
        GuildExperienceKillOutcome::SkippedDisbanded
    );
    drop(config);
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn guild_xp_captured_receipt_and_journal_require_exact_ephemeral_permit() {
    let (config, path, id, identity) = fixture();
    let (source, permit) = captured(&config, &id, &identity);
    let before = fs::read(&path).unwrap();
    assert!(commit(&config, &identity, source.clone()).is_err());
    let mut wrong = source.clone();
    wrong.guild_experience_journal.pending[0].final_amount += 1;
    assert!(authorized_commit(&config, &permit, wrong).is_err());
    let mut omitted = source.clone();
    omitted.guild_experience_journal.pending.clear();
    omitted.guild_experience_journal.next_sequence -= 1;
    assert!(authorized_commit(&config, &permit, omitted).is_err());
    let mut forged = source;
    forged.guild_experience_journal.kill_outcomes.insert(
        permit.kill_key.clone(),
        GuildExperienceKillOutcome::SkippedDisbanded,
    );
    assert!(authorized_commit(&config, &permit, forged).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    drop(config);
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
