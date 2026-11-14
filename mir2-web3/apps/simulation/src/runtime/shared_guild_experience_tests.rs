use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn checkpoint_with_held_account_store(force: bool) -> (bool, bool) {
    use std::sync::mpsc;
    use std::time::Duration;

    let mut session = super::tests::session();
    let store = session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .account_store
        .clone();
    let (held_tx, held_rx) = mpsc::sync_channel(0);
    let (release_tx, release_rx) = mpsc::sync_channel(0);
    let hold_limit = if force {
        Duration::from_secs(5)
    } else {
        Duration::from_millis(250)
    };
    let holder = std::thread::spawn(move || {
        let guard = store.lock().unwrap();
        held_tx.send(()).unwrap();
        // The bounded fallback releases the Store even against old code or a
        // panic. Successful forced admission must finish before this fallback.
        let explicitly_released = release_rx.recv_timeout(hold_limit).is_ok();
        drop(guard);
        explicitly_released
    });
    let held = held_rx.recv_timeout(Duration::from_secs(5));
    if held.is_err() {
        drop(release_tx);
        holder.join().unwrap();
        panic!("AccountStore holder did not reach the synchronous barrier");
    }
    // Keep the session on this thread: no private runtime is moved across lanes.
    let attempt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if force {
            session.begin_default_npc_source_command()
        } else {
            session.begin_guild_experience_command(false)
        }
    }));
    let released_by_test = release_tx.send(()).is_ok();
    let held_until_explicit_release = holder.join().unwrap();
    // Assertions and unwraps are deliberately after the Store release/join.
    let checkpoint = attempt.expect("checkpoint admission panicked").unwrap();
    let captured = checkpoint.is_some();
    session
        .finish_guild_experience_command(checkpoint, Vec::new())
        .unwrap();
    (captured, released_by_test && held_until_explicit_release)
}

#[test]
fn forced_checkpoint_captures_before_unrelated_account_store_is_released() {
    let (captured, completed_while_store_held) = checkpoint_with_held_account_store(true);
    assert!(
        captured,
        "forced personal Source checkpoint is still complete"
    );
    assert!(
        completed_while_store_held,
        "forced checkpoint must not wait for unused mentor/guild eligibility lookups"
    );
}

#[test]
fn unforced_checkpoint_still_checks_store_and_skips_absent_eligibility() {
    let (captured, completed_while_store_held) = checkpoint_with_held_account_store(false);
    assert!(!completed_while_store_held);
    assert!(
        !captured,
        "no mentor, guild or pet admission needs no checkpoint"
    );
}

#[test]
fn unforced_checkpoint_still_captures_an_existing_guild_member() {
    let mut session = super::tests::session();
    session.grant_shared_guild_creation_from_npc();
    session.submit_shared_guild_name("CheckpointGuild").unwrap();
    let checkpoint = session.begin_guild_experience_command(false).unwrap();
    assert!(checkpoint.is_some());
    session
        .finish_guild_experience_command(checkpoint, Vec::new())
        .unwrap();
}

#[test]
fn prepared_kill_runtime_rejection_restores_complete_private_source() {
    let mut session = super::tests::session();
    let config = session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .clone()
        .with_account_store_database_url("postgresql://unused.invalid/prepared-test");
    session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config = config.clone();
    let before = session.active_character_checkpoint().unwrap();
    let award = crate::runtime::zone::ZoneMonsterKillAward {
        monster_object_id: 912300,
        killed_at_ms: 1,
        monster_name: "Scarecrow".into(),
        experience: 5,
        drops: vec![],
        boss_audit: None,
        mentor_bank: None,
        experience_selection: None,
        source_receipt_key: Some("prepared/runtime/1".into()),
    };
    let result = session.commit_shared_monster_kill_via_postgres::<(), _>(
        "prepared/runtime/1",
        &award,
        |source| {
            assert_eq!(source.before().experience, before.experience);
            assert!(source.checkpoint().experience > source.before().experience);
            assert_eq!(source.checkpoint().revision, source.before().revision + 1);
            assert_eq!(
                source
                    .checkpoint()
                    .guild_experience_journal
                    .applied_kill_receipts
                    .get(source.key())
                    .map(String::as_str),
                Some(source.award_hash())
            );
            Err(crate::PreparedKillPublicationFailure::Rejected(
                "transaction preflight rejected".into(),
            ))
        },
    );
    assert!(matches!(
        result,
        Err(crate::PreparedKillPublicationFailure::Rejected(_))
    ));
    let after = session.active_character_checkpoint().unwrap();
    assert_eq!(after.experience, before.experience);
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.inventory_items_json, before.inventory_items_json);
    assert_eq!(after.quest_states_json, before.quest_states_json);
    assert!(after
        .guild_experience_journal
        .applied_kill_receipts
        .is_empty());
    config.ensure_account_store_writable().unwrap();
}

#[test]
fn prepared_kill_runtime_missing_commit_receipt_freezes_instead_of_false_success() {
    let mut session = super::tests::session();
    let config = session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .clone()
        .with_account_store_database_url("postgresql://unused.invalid/prepared-test");
    session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config = config.clone();
    let award = crate::runtime::zone::ZoneMonsterKillAward {
        monster_object_id: 912300,
        killed_at_ms: 1,
        monster_name: "Scarecrow".into(),
        experience: 5,
        drops: vec![],
        boss_audit: None,
        mentor_bank: None,
        experience_selection: None,
        source_receipt_key: Some("prepared/runtime/2".into()),
    };
    let result =
        session.commit_shared_monster_kill_via_postgres("prepared/runtime/2", &award, |_| {
            Ok(crate::PreparedKillPublication::Written(()))
        });
    assert!(matches!(
        result,
        Err(crate::PreparedKillPublicationFailure::OutcomeUnknown(_))
    ));
    assert!(config.ensure_account_store_writable().is_err());
}

#[test]
fn guild_xp_shared_kill_full_checkpoint_receipt_survives_failure_and_relogin() {
    let mut session = super::tests::session();
    super::super::leveling::apply_level_change(session.app.world_mut(), 22);
    session.grant_shared_guild_creation_from_npc();
    session.submit_shared_guild_name("CapturedGuild").unwrap();
    let identity = world_identity(session.app.world()).unwrap();
    let config = session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .clone();
    let guild = config
        .shared_guild_for_identity(&identity)
        .unwrap()
        .unwrap();
    let mut award = crate::runtime::zone::ZoneMonsterKillAward {
        monster_object_id: 987001,
        killed_at_ms: 400,
        monster_name: "Scarecrow".into(),
        experience: 1,
        drops: vec![],
        boss_audit: None,
        source_receipt_key: None,
        mentor_bank: None,
        experience_selection: Some(crate::runtime::zone::ZoneExperienceSelection {
            source_zone: "primary/0/0/main".into(),
            monster_object_id: 987001,
            killed_at_ms: 400,
            account_id: identity.account_id.clone(),
            character_index: identity.character_index,
            guild: Some(crate::runtime::zone::ZoneGuildExperienceMembership {
                guild_id: guild.id.clone(),
                membership_epoch: guild.member(&identity).unwrap().membership_epoch,
            }),
            final_amount: 200,
            guild_amount: 7,
        }),
    };
    let before = session.active_character_checkpoint().unwrap();
    config
        .inject_account_store_transaction_fault(crate::AccountStoreTransactionFault::BeforePersist);
    assert!(matches!(
        session.try_commit_shared_monster_kill_award_with_receipt("test-zone/kill/400", &award),
        Err(crate::SharedMonsterKillCommitFailure::Deferred(_))
    ));
    assert_eq!(
        session.active_character_checkpoint().unwrap().experience,
        before.experience
    );
    assert!(session
        .active_character_checkpoint()
        .unwrap()
        .guild_experience_journal
        .is_empty());
    assert_eq!(
        config
            .shared_guild_for_identity(&identity)
            .unwrap()
            .unwrap()
            .experience,
        0
    );
    assert!(
        session
            .commit_shared_monster_kill_award_with_receipt("test-zone/kill/400", &award)
            .committed
    );
    assert_eq!(
        session.active_character_checkpoint().unwrap().experience,
        before.experience + 200
    );
    assert_eq!(
        config
            .shared_guild_for_identity(&identity)
            .unwrap()
            .unwrap()
            .experience,
        7
    );
    session.handle_packet(mir2_protocol::ClientPacket::LogOut);
    session.handle_packet(mir2_protocol::ClientPacket::StartGame {
        character_index: identity.character_index,
    });
    assert!(
        session
            .commit_shared_monster_kill_award_with_receipt("test-zone/kill/400", &award)
            .committed
    );
    assert_eq!(
        session.active_character_checkpoint().unwrap().experience,
        before.experience + 200
    );
    award.experience += 1;
    assert!(
        !session
            .commit_shared_monster_kill_award_with_receipt("test-zone/kill/400", &award)
            .committed
    );
    assert_eq!(
        config
            .shared_guild_for_identity(&identity)
            .unwrap()
            .unwrap()
            .experience,
        7
    );
}

#[test]
fn guild_xp_npc_gain_file_failure_rolls_back_source_and_reuses_only_uncommitted_sequence() {
    let mut session = super::tests::session();
    super::super::leveling::apply_level_change(session.app.world_mut(), 22);
    session.grant_shared_guild_creation_from_npc();
    session.submit_shared_guild_name("ExperienceGuild").unwrap();
    let old_config = session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .clone();
    let old_store = old_config.account_store.lock().unwrap().clone();
    let path = std::env::temp_dir()
        .join(format!(
            "mir2-guild-xp-npc-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
        .join("accounts.json");
    let config = old_config.with_account_store_path(&path);
    *config.account_store.lock().unwrap() = old_store;
    config.save_account_store().unwrap();
    session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config = config.clone();
    let identity = world_identity(session.app.world()).unwrap();
    session.handle_packet(mir2_protocol::ClientPacket::NewHero {
        name: "XpCompanion".into(),
        gender: mir2_protocol::MirGender::Female,
        class: mir2_protocol::MirClass::Wizard,
    });
    {
        let world = session.app.world_mut();
        super::super::hero_ai::hero_buffs::capture(
            world,
            &[mir2_protocol::ServerPacket::AddBuff {
                buff: mir2_protocol::ClientBuff {
                    buff_type: 21,
                    object_id: 1001,
                    expire_time: 60000,
                    infinite: false,
                    visible: true,
                    paused: false,
                    stats: vec![mir2_protocol::UserItemStat { stat: 4, value: 6 }],
                    values: vec![],
                },
            }],
        );
        super::super::hero_ai::hero_cadence::cast(world);
    }
    let before = session.active_character_checkpoint().unwrap();
    let bytes = std::fs::read(&path).unwrap();

    let checkpoint = session.begin_guild_experience_command(false).unwrap();
    super::super::npc_script::crystal_npc_give_exp(session.app.world_mut(), &["200"]);
    assert_eq!(
        session.active_character_checkpoint().unwrap().experience,
        before.experience + 200
    );
    config.inject_account_store_transaction_fault(
        crate::AccountStoreTransactionFault::BeforeFileRename,
    );
    assert!(session
        .finish_guild_experience_command(checkpoint, vec![])
        .is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let rolled_back = session.active_character_checkpoint().unwrap();
    assert_eq!(rolled_back.experience, before.experience);
    assert_eq!(
        rolled_back.guild_experience_journal,
        before.guild_experience_journal
    );
    let hero_buffs = super::super::hero_ai::hero_buffs::active(session.app.world());
    assert_eq!(hero_buffs.len(), 1);
    assert_eq!(hero_buffs[0].stats[0].value, 6);
    assert!(hero_buffs[0].expire_time > 59000 && hero_buffs[0].expire_time <= 60000);
    assert!(!super::super::hero_ai::hero_cadence::action_ready(
        session.app.world()
    ));

    let checkpoint = session.begin_guild_experience_command(false).unwrap();
    super::super::npc_script::crystal_npc_give_exp(session.app.world_mut(), &["200"]);
    session
        .finish_guild_experience_command(checkpoint, vec![])
        .unwrap();
    let guild = config
        .shared_guild_for_identity(&identity)
        .unwrap()
        .unwrap();
    assert_eq!(guild.experience, 2);
    assert_eq!(guild.experience_receipts.len(), 1);
    let store = config.account_store.lock().unwrap();
    let saved = &store.accounts[&identity.account_id].saves[&identity.character_index];
    assert_eq!(saved.experience, before.experience + 200);
    assert_eq!(saved.guild_experience_journal.next_sequence, 1);
    assert!(saved.guild_experience_journal.pending.is_empty());
    drop(store);
    session.handle_packet(mir2_protocol::ClientPacket::LogOut);
    session.handle_packet(mir2_protocol::ClientPacket::StartGame {
        character_index: identity.character_index,
    });
    assert_eq!(
        session.active_character_checkpoint().unwrap().experience,
        before.experience + 200
    );
    assert_eq!(
        config
            .shared_guild_for_identity(&identity)
            .unwrap()
            .unwrap()
            .experience,
        2
    );
    let uncertain = config
        .clone()
        .with_account_store_database_url("postgresql://xp-probe@127.0.0.1:1/xp_probe");
    session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config = uncertain.clone();
    uncertain.inject_guild_xp_unknown_publication_probe();
    let guild = config
        .shared_guild_for_identity(&identity)
        .unwrap()
        .unwrap();
    let award = crate::runtime::zone::ZoneMonsterKillAward {
        monster_object_id: 987002,
        killed_at_ms: 500,
        monster_name: "Scarecrow".into(),
        experience: 100,
        drops: vec![],
        boss_audit: None,
        source_receipt_key: None,
        mentor_bank: None,
        experience_selection: Some(crate::runtime::zone::ZoneExperienceSelection {
            source_zone: "primary/0/0/main".into(),
            monster_object_id: 987002,
            killed_at_ms: 500,
            account_id: identity.account_id.clone(),
            character_index: identity.character_index,
            guild: Some(crate::runtime::zone::ZoneGuildExperienceMembership {
                guild_id: guild.id,
                membership_epoch: 1,
            }),
            final_amount: 100,
            guild_amount: 1,
        }),
    };
    let before_unknown = std::fs::read(&path).unwrap();
    assert!(matches!(
        session.try_commit_shared_monster_kill_award_with_receipt("unknown/500", &award),
        Err(crate::SharedMonsterKillCommitFailure::OutcomeUnknown(_))
    ));
    assert_eq!(uncertain.guild_xp_publication_probe_attempts(), 1);
    assert_eq!(std::fs::read(&path).unwrap(), before_unknown);
    assert!(matches!(
        session.try_commit_shared_monster_kill_award_with_receipt("unknown/500", &award),
        Err(crate::SharedMonsterKillCommitFailure::Deferred(_))
    ));
    assert_eq!(uncertain.guild_xp_publication_probe_attempts(), 1);
    assert!(config.save_account_store().is_err());
    drop(uncertain);
    drop(session);
    drop(config);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

// Finish-only: fixture/checkpoint construction precedes the real Store barrier.
// No Tick, billing, admission or has_uncommitted Store read is timed here.
fn finish_only_with_held_account_store(
    session: &mut crate::SimulationSession,
    before: Option<crate::runtime::shared_guild_experience::GuildExperienceCheckpoint>,
    packets: Vec<mir2_protocol::ServerPacket>,
) -> (Result<Vec<mir2_protocol::ServerPacket>, String>, bool) {
    use std::sync::mpsc;
    use std::time::Duration;

    let store = session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .account_store
        .clone();
    let (held_tx, held_rx) = mpsc::sync_channel(0);
    let (release_tx, release_rx) = mpsc::sync_channel(0);
    let holder = std::thread::spawn(move || {
        let guard = store.lock().unwrap();
        held_tx.send(()).unwrap();
        // Old code terminates through this fallback; no mutex is leaked.
        let explicitly_released = release_rx.recv_timeout(Duration::from_secs(5)).is_ok();
        drop(guard);
        explicitly_released
    });
    let held = held_rx.recv_timeout(Duration::from_secs(5));
    if held.is_err() {
        drop(release_tx);
        let joined = holder.join();
        assert!(joined.is_ok(), "Store holder failed before readiness");
        panic!("Store holder did not reach the ownership handshake");
    }
    // The session stays on its owner thread. Only AccountStore is shared.
    let attempt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        session.finish_guild_experience_command(before, packets)
    }));
    let explicitly_released = release_tx.send(()).is_ok();
    let joined = holder.join();
    // Every assertion/unwrap below runs after release and join, even on panic.
    let held_until_explicit_release = joined.expect("Store holder panicked");
    let result = attempt.expect("finish panicked");
    (result, explicitly_released && held_until_explicit_release)
}

#[test]
fn quiet_finish_completes_before_held_account_store_is_released() {
    let mut session = super::tests::session();
    let config = session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .clone();
    let identity = session.active_identity().unwrap();
    let journal_before = crate::runtime::shared_guild_experience::snapshot(session.app.world());
    assert!(
        journal_before.is_empty(),
        "all six journal fields must be empty"
    );
    let source_before =
        serde_json::to_value(session.active_character_checkpoint().unwrap()).unwrap();
    let durable_before = {
        let store = config.account_store.lock().unwrap();
        serde_json::to_value(&store.accounts[&identity.account_id].saves[&identity.character_index])
            .unwrap()
    };
    // This forced Source checkpoint has force_save=false by its existing API.
    let before = session.begin_default_npc_source_command().unwrap();
    assert!(before.is_some());
    let packets = vec![mir2_protocol::ServerPacket::LoseGold { gold: 7 }];
    let packet_image = format!("{packets:?}");
    let (result, completed_while_store_held) =
        finish_only_with_held_account_store(&mut session, before, packets);
    let packets = result.unwrap();
    assert_eq!(format!("{packets:?}"), packet_image);
    assert_eq!(
        serde_json::to_value(session.active_character_checkpoint().unwrap()).unwrap(),
        source_before,
        "quiet finish must preserve the complete owner state and save revision"
    );
    assert_eq!(
        crate::runtime::shared_guild_experience::snapshot(session.app.world()),
        journal_before
    );
    let durable_after = {
        let store = config.account_store.lock().unwrap();
        serde_json::to_value(&store.accounts[&identity.account_id].saves[&identity.character_index])
            .unwrap()
    };
    assert_eq!(durable_after, durable_before);
    assert!(
        completed_while_store_held,
        "quiet finish must not wait for a mentor lookup whose XP/selected-level term is false"
    );
}

#[test]
fn invalid_before_stage5_json_still_errors_before_quiet_finish_filter() {
    let mut session = super::tests::session();
    let source_before =
        serde_json::to_value(session.active_character_checkpoint().unwrap()).unwrap();
    assert!(crate::runtime::shared_guild_experience::snapshot(session.app.world()).is_empty());
    let mut before = session.begin_default_npc_source_command().unwrap();
    let invalid = "{";
    let expected = format!(
        "mentor source checkpoint invalid: {}",
        serde_json::from_str::<crate::Stage5SystemsState>(invalid).unwrap_err()
    );
    before.as_mut().unwrap().save_mut().stage5_systems_json = Some(invalid.into());
    let (result, completed_while_store_held) =
        finish_only_with_held_account_store(&mut session, before, Vec::new());
    assert!(
        completed_while_store_held,
        "eager JSON error precedes any Store lookup"
    );
    assert_eq!(result.unwrap_err(), expected);
    assert_eq!(
        serde_json::to_value(session.active_character_checkpoint().unwrap()).unwrap(),
        source_before
    );
}

fn changed_finish_with_new_mentor_still_checks_source(level_only: bool) {
    let mut session = super::tests::session();
    // Warm a real complete source before measuring finish or taking the Store.
    crate::runtime::save::persist_active_character_save(session.app.world()).unwrap();
    let config = session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .clone();
    let active = session.active_identity().unwrap();
    let actor = crate::Stage5FriendIdentity {
        account_id: active.account_id.clone(),
        character_index: active.character_index,
    };
    let teacher = crate::Stage5FriendIdentity {
        account_id: "finish-teacher".into(),
        character_index: 7,
    };
    let owner_before = session.active_character_checkpoint().unwrap();
    assert!(owner_before.guild_experience_journal.is_empty());
    let mut teacher_character = config.default_character.clone();
    teacher_character.index = teacher.character_index;
    teacher_character.name = "FinishTeacher".into();
    teacher_character.level = owner_before.character.level.checked_add(10).unwrap();
    {
        let mut store = config.account_store.lock().unwrap();
        assert!(store
            .accounts
            .insert(
                teacher.account_id.clone(),
                crate::AccountRecord::new(teacher_character)
            )
            .is_none());
    }
    config
        .commit_shared_mentor_mutation(&teacher, crate::SharedMentorMutation::TogglePermission, 1)
        .unwrap();
    let before = session.begin_default_npc_source_command().unwrap();
    assert!(before.is_some());
    // Publish after begin; deliberately do not refresh the owner's local social
    // projection. The original mentor clause must inspect this NEW authority.
    config
        .commit_shared_mentor_mutation(
            &teacher,
            crate::SharedMentorMutation::Accept {
                student: actor.clone(),
            },
            10,
        )
        .unwrap();
    assert!(session
        .app
        .world()
        .resource::<super::super::resources::Stage5SystemsResource>()
        .stage5_systems
        .mentor
        .partner_identity
        .is_none());
    if level_only {
        session
            .app
            .world_mut()
            .resource_mut::<super::super::resources::SessionResource>()
            .selected_character
            .as_mut()
            .unwrap()
            .level = owner_before.character.level.checked_add(1).unwrap();
    } else {
        session
            .app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .experience = owner_before.experience.checked_add(1).unwrap();
    }
    let changed = session.active_character_checkpoint().unwrap();
    if level_only {
        assert_eq!(changed.experience, owner_before.experience);
        assert_ne!(changed.character.level, owner_before.character.level);
    } else {
        assert_ne!(changed.experience, owner_before.experience);
        assert_eq!(changed.character.level, owner_before.character.level);
    }
    config
        .inject_account_store_transaction_fault(crate::AccountStoreTransactionFault::BeforePersist);
    let (result, completed_while_store_held) =
        finish_only_with_held_account_store(&mut session, before, Vec::new());
    assert!(
        !completed_while_store_held,
        "positive scalar change keeps the fresh mentor Store read"
    );
    assert_eq!(
        result.unwrap_err(),
        "injected account-store failure before persist"
    );
    let rolled_back = session.active_character_checkpoint().unwrap();
    assert_eq!(rolled_back.experience, owner_before.experience);
    assert_eq!(rolled_back.character.level, owner_before.character.level);
    assert_eq!(rolled_back.revision, owner_before.revision);
    assert_eq!(
        config
            .shared_mentor_profile_for(&actor)
            .unwrap()
            .mentor
            .partner_identity,
        Some(teacher)
    );
}

#[test]
fn xp_only_changed_finish_rechecks_mentor_published_after_begin_and_rejects_source_failure() {
    changed_finish_with_new_mentor_still_checks_source(false);
}

#[test]
fn selected_level_only_changed_finish_rechecks_mentor_published_after_begin_and_rejects_source_failure(
) {
    changed_finish_with_new_mentor_still_checks_source(true);
}

#[test]
fn changed_selected_identity_keeps_original_finish_mentor_clause() {
    let mut session = super::tests::session();
    assert!(crate::runtime::shared_guild_experience::snapshot(session.app.world()).is_empty());
    let before = session.begin_default_npc_source_command().unwrap();
    session
        .app
        .world_mut()
        .resource_mut::<super::super::resources::SessionResource>()
        .selected_character
        .as_mut()
        .unwrap()
        .index += 1000;
    let (result, completed_while_store_held) =
        finish_only_with_held_account_store(&mut session, before, Vec::new());
    assert!(result.unwrap().is_empty());
    assert!(
        !completed_while_store_held,
        "uncertain selected identity takes the original fresh branch"
    );
}
