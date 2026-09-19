use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn prepared_kill_runtime_rejection_restores_complete_private_source() {
    let mut session = super::tests::session();
    let config = session.app.world().resource::<RuntimeConfigResource>().config.clone()
        .with_account_store_database_url("postgresql://unused.invalid/prepared-test");
    session.app.world_mut().resource_mut::<RuntimeConfigResource>().config = config.clone();
    let before = session.active_character_checkpoint().unwrap();
    let award = crate::runtime::zone::ZoneMonsterKillAward {
        monster_object_id: 912300, killed_at_ms: 1, monster_name:"Scarecrow".into(),
        experience: 5, drops:vec![],boss_audit:None,experience_selection:None,
        source_receipt_key:Some("prepared/runtime/1".into()),
    };
    let result = session.commit_shared_monster_kill_via_postgres::<(),_>("prepared/runtime/1",&award,|source|{
        assert_eq!(source.before().experience,before.experience);
        assert!(source.checkpoint().experience > source.before().experience);
        assert_eq!(source.checkpoint().revision,source.before().revision+1);
        assert_eq!(source.checkpoint().guild_experience_journal.applied_kill_receipts.get(source.key()).map(String::as_str),Some(source.award_hash()));
        Err(crate::PreparedKillPublicationFailure::Rejected("transaction preflight rejected".into()))
    });
    assert!(matches!(result,Err(crate::PreparedKillPublicationFailure::Rejected(_))));
    let after = session.active_character_checkpoint().unwrap();
    assert_eq!(after.experience,before.experience);
    assert_eq!(after.revision,before.revision);
    assert_eq!(after.inventory_items_json,before.inventory_items_json);
    assert_eq!(after.quest_states_json,before.quest_states_json);
    assert!(after.guild_experience_journal.applied_kill_receipts.is_empty());
    config.ensure_account_store_writable().unwrap();
}

#[test]
fn prepared_kill_runtime_missing_commit_receipt_freezes_instead_of_false_success() {
    let mut session = super::tests::session();
    let config = session.app.world().resource::<RuntimeConfigResource>().config.clone()
        .with_account_store_database_url("postgresql://unused.invalid/prepared-test");
    session.app.world_mut().resource_mut::<RuntimeConfigResource>().config = config.clone();
    let award = crate::runtime::zone::ZoneMonsterKillAward {
        monster_object_id: 912300, killed_at_ms: 1, monster_name:"Scarecrow".into(),
        experience: 5, drops:vec![],boss_audit:None,experience_selection:None,
        source_receipt_key:Some("prepared/runtime/2".into()),
    };
    let result = session.commit_shared_monster_kill_via_postgres("prepared/runtime/2",&award,|_|Ok(crate::PreparedKillPublication::Written(())));
    assert!(matches!(result,Err(crate::PreparedKillPublicationFailure::OutcomeUnknown(_))));
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
