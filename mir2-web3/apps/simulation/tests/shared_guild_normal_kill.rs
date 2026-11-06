//! Prepared account/Guild authority with a real shared native monster death,
//! trusted personal profile and normal durable award consumer. No public-play
//! or native-client acceptance is implied by these isolated integration checks.
use mir2_protocol::{ClientPacket, MirDirection, Point, ServerPacket, Spell};
use mir2_simulation::{
    AccountRecord, CharacterSaveRecord, SessionId, SharedGuildMember, SharedGuildRank,
    SharedGuildRecord, SimulationConfig, SimulationSession, Stage5FriendIdentity,
    WorldEntityDisposition, ZoneChatProfile, ZoneCommand, ZoneManager, ZoneMonsterKillAward,
    ZoneMonsterSpawn, ZoneOutbound,
};
use std::collections::BTreeMap;

const GUILD: &str = "0123456789abcdef0123456789abcdef";
const OTHER: &str = "1123456789abcdef0123456789abcdef";

fn actor(config: &SimulationConfig) -> Stage5FriendIdentity {
    Stage5FriendIdentity {
        account_id: "demo".into(),
        character_index: config.default_character.index,
    }
}
fn guild(config: &SimulationConfig, id: &str, name: &str) -> SharedGuildRecord {
    SharedGuildRecord {
        id: id.into(),
        name: name.into(),
        revision: 1,
        level: 0,
        experience: 0,
        spare_points: 0,
        gold: 0,
        ranks: vec![SharedGuildRank {
            index: 0,
            name: "Leader".into(),
            options: 255,
        }],
        members: vec![SharedGuildMember {
            identity: actor(config),
            name: config.default_character.name.clone(),
            rank_index: 0,
            membership_epoch: 7,
        }],
        notice: Vec::new(),
        storage: BTreeMap::new(),
        buffs: BTreeMap::new(),
        last_buff_tick_ms: 0,
        experience_receipts: Default::default(),
        experience_receipt_payloads: Default::default(),
        active_wars: Default::default(),
    }
}
fn fixture(guild_name: Option<&str>) -> SimulationConfig {
    let mut config = SimulationConfig::default();
    config.default_character.level = 22;
    {
        let mut store = config.account_store.lock().unwrap();
        let record = store.accounts.get_mut("demo").unwrap();
        record
            .characters
            .iter_mut()
            .find(|character| character.index == config.default_character.index)
            .unwrap()
            .level = 22;
        let save = record
            .saves
            .get_mut(&config.default_character.index)
            .unwrap();
        save.character.level = 22;
        // This is an isolated prepared buff, not an earned public-player buff.
        save.buff_states_json.push(
            serde_json::json!({
                "key":"exp", "name":"Exp", "description":"prepared source-rate input",
                "expires_at_tick":u64::MAX, "attack_bonus":0, "defence_bonus":0,
                "stats":[{"stat":100,"value":20}]
            })
            .to_string(),
        );
        if let Some(name) = guild_name {
            store
                .shared_guilds
                .insert(GUILD.into(), guild(&config, GUILD, name));
        }
    }
    config
}
fn login(config: &SimulationConfig) -> SimulationSession {
    let mut session = SimulationSession::new(config.clone());
    session.enable_shared_guild_authority();
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into()
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert!(session
        .handle_packet(ClientPacket::StartGame {
            character_index: config.default_character.index
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    session
}
fn prepare_native_death(session: &SimulationSession) -> (ZoneManager, SessionId) {
    let mut manager = ZoneManager::new();
    let mut join = session.active_zone_join_snapshot("normal-kill").unwrap();
    join.map_file_name = "xp-test".into();
    join.position = Point { x: 330, y: 270 };
    join.direction = MirDirection::Right;
    let class = join.class;
    manager.handle(ZoneCommand::Join(join));
    let sid = SessionId::new("normal-kill");
    manager.handle(ZoneCommand::sync_player_combat_state(
        sid.clone(),
        class,
        false,
        false,
        false,
        false,
        false,
        false,
    ));
    manager.handle(ZoneCommand::SpawnMonster {
        session_id: sid.clone(),
        now_ms: 0,
        monster: ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: 800,
            name: "Scarecrow".into(),
            name_colour_argb: -1,
            image: 0,
            ai: 0,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 20,
            max_hp: 10,
            hp: 10,
            experience: 1000,
            move_speed_ms: 60_000,
            attack_speed_ms: 60_000,
            friendly_guild: None,
            position: Point { x: 331, y: 270 },
            direction: MirDirection::Left,
            defense: Default::default(),
            respawn: None,
            drops: Vec::new(),
        },
    });
    (manager, sid)
}

fn finish_native_death(manager: &mut ZoneManager, sid: &SessionId) -> ZoneMonsterKillAward {
    let mut launches = Vec::new();
    let mut out = Vec::new();
    // The real personal DC is authoritative: the supplied scalar does not
    // choose the damage. Complete accepted attacks at their normal cadence.
    for swing in 0..4 {
        let now_ms = 10 + swing * 610;
        launches.extend(manager.handle(ZoneCommand::PlayerAttackObject {
            session_id: sid.clone(),
            object_id: 800,
            direction: MirDirection::Right,
            spell: Spell::None as u8,
            level: 0,
            attack_type: 0,
            damage: 10,
            now_ms,
        }));
        out.extend(manager.tick_all(now_ms));
        if out
            .iter()
            .any(|outbound| matches!(outbound, ZoneOutbound::OwnedMonsterKillAward { .. }))
        {
            break;
        }
    }
    let diagnostics = format!(
        "normal native death must issue a selected reward; launches={launches:?}; out={out:?}; monsters={:?}",
        manager.native_monster_snapshots(&mir2_simulation::ZoneKey::for_map("xp-test"))
    );
    let (source, sid, proof, award) = out
        .into_iter()
        .find_map(|outbound| match outbound {
            ZoneOutbound::OwnedMonsterKillAward {
                source,
                session_id,
                online_owner,
                award,
            } => Some((source, session_id, online_owner, award)),
            _ => None,
        })
        .expect(&diagnostics);
    assert!(manager.issued_monster_award_is_current(&source, &sid, &proof, &award));
    assert!(award.experience_selection.is_some());
    award
}

fn capture(session: &SimulationSession) -> (ZoneManager, ZoneMonsterKillAward) {
    let source = session.active_zone_experience_profile().unwrap().unwrap();
    let (mut manager, sid) = prepare_native_death(session);
    assert!(manager.update_experience_profile(&sid, Some(source)));
    let award = finish_native_death(&mut manager, &sid);
    (manager, award)
}

fn prepare_authority_leader(config: &SimulationConfig) -> Stage5FriendIdentity {
    let leader = Stage5FriendIdentity {
        account_id: "admission-leader".into(),
        character_index: config.default_character.index + 100,
    };
    let mut character = config.default_character.clone();
    character.index = leader.character_index;
    character.name = "Admission Leader".into();
    let mut store = config.account_store.lock().unwrap();
    store
        .accounts
        .insert(leader.account_id.clone(), AccountRecord::new(character));
    let guild = store.shared_guilds.get_mut(GUILD).unwrap();
    guild.ranks.push(SharedGuildRank {
        index: 1,
        name: "Member".into(),
        options: 0,
    });
    guild.members[0].rank_index = 1;
    guild.members.push(SharedGuildMember {
        identity: leader.clone(),
        name: "Admission Leader".into(),
        rank_index: 0,
        membership_epoch: 8,
    });
    leader
}

fn rebase_admission_recipient(
    manager: &mut ZoneManager,
    sid: &SessionId,
    actor: &Stage5FriendIdentity,
    rebase: &dyn Fn(
        &Stage5FriendIdentity,
        &mut Option<mir2_simulation::ZoneExperienceProfile>,
        &mut ZoneChatProfile,
    ) -> Result<(), String>,
) {
    let zone = manager.zone_key_for_session(sid).unwrap();
    let mut chat = manager
        .zone(&zone)
        .unwrap()
        .player_chat_profile(sid)
        .unwrap();
    let mut profile = manager.player_experience_profile(sid);
    rebase(actor, &mut profile, &mut chat).unwrap();
    assert!(manager.update_experience_profile(sid, profile));
    manager.handle(ZoneCommand::UpdateChatProfile {
        session_id: sid.clone(),
        profile: chat,
    });
}

fn obtain_actual_guild_experience_buff(config: &SimulationConfig, session: &mut SimulationSession) {
    let definition = mir2_game_data::crystal_guild_buff_definitions()
        .iter()
        .find(|definition| definition.id == 7)
        .unwrap();
    assert_eq!(definition.time_limit, 60);
    assert_eq!(
        definition
            .stats
            .iter()
            .find(|stat| stat.stat == 100)
            .unwrap()
            .value,
        4,
        "the imported GuildSettings.ini Experience buff remains authoritative"
    );
    {
        // Isolated authority preparation grants the real buy prerequisites;
        // the normal reducer still rechecks permission, level, cost and points.
        let mut store = config.account_store.lock().unwrap();
        let guild = store.shared_guilds.get_mut(GUILD).unwrap();
        guild.level = definition.level_requirement;
        guild.spare_points = definition.points_requirement;
        guild.gold = definition.activation_cost as u32;
    }
    assert!(session
        .change_shared_guild_buff(1, definition.id)
        .unwrap()
        .iter()
        .any(
            |packet| matches!(packet, ServerPacket::GuildBuffList { active_buffs, .. }
            if active_buffs.iter().any(|buff| buff.id == definition.id && buff.active))
        ));
    let guild = config
        .shared_guild_for_identity(&actor(config))
        .unwrap()
        .unwrap();
    assert_eq!(guild.gold, 0);
    assert_eq!(guild.spare_points, 0);
    assert_eq!(guild.buffs[&definition.id].remaining_minutes, 60);
}

#[test]
fn normal_guild_exp_buff_uses_imported_rate_and_prior_death_keeps_it_after_leave() {
    let config = fixture(Some("Rate Guild"));
    let mut session = login(&config);
    obtain_actual_guild_experience_buff(&config, &mut session);
    let source = session.active_zone_experience_profile().unwrap().unwrap();
    assert_eq!(source.experience_rate_percent, 24);
    assert_eq!(
        source
            .rate_source
            .as_ref()
            .unwrap()
            .base_experience_rate_percent,
        0
    );
    assert_eq!(source.rate_source.as_ref().unwrap().guild_stats.len(), 1);
    assert_eq!(source.rate_source.as_ref().unwrap().guild_stats[0].value, 4);
    let (_, award) = capture(&session);
    assert_eq!(
        award.experience_selection.as_ref().unwrap().final_amount,
        1240
    );
    let leader = prepare_authority_leader(&config);
    config
        .commit_shared_guild_remove_member(&leader, &config.default_character.name, false)
        .unwrap();
    assert_eq!(
        session
            .active_zone_experience_profile()
            .unwrap()
            .unwrap()
            .experience_rate_percent,
        20
    );
    let receipt = session
        .try_commit_shared_monster_kill_award_with_receipt("rate-before-leave", &award)
        .unwrap()
        .0;
    assert!(receipt
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GainExperience { amount: 1240 })));
    assert_eq!(
        config.account_store.lock().unwrap().shared_guilds[GUILD].experience,
        12
    );
}

#[test]
fn source_admission_removes_stale_guild_and_newbie_rates_after_real_membership_leave() {
    for name in ["Rate Guild", "NewbieGuild"] {
        let config = fixture(Some(name));
        let mut session = login(&config);
        obtain_actual_guild_experience_buff(&config, &mut session);
        session
            .refresh_shared_social_buffs(&Default::default())
            .unwrap();
        let source = session.active_zone_experience_profile().unwrap().unwrap();
        let newbie = name == config.crystal_newbie_guild_name;
        assert_eq!(source.experience_rate_percent, if newbie { 29 } else { 24 });
        assert_eq!(source.guild.is_none(), newbie);
        assert_eq!(
            source
                .rate_source
                .as_ref()
                .unwrap()
                .buffs
                .iter()
                .any(|buff| buff.newbie_buff),
            newbie
        );
        let (mut manager, sid) = prepare_native_death(&session);
        assert!(manager.update_experience_profile(&sid, Some(source)));
        let leader = prepare_authority_leader(&config);
        config
            .commit_shared_guild_remove_member(&leader, &config.default_character.name, false)
            .unwrap();
        // Deliberately leave the peer's private Newbie buff intact. Admission
        // uses current typed membership, not the stale published projection.
        let actor = actor(&config);
        let award = config
            .with_shared_zone_experience_admission(|rebase| {
                rebase_admission_recipient(&mut manager, &sid, &actor, rebase);
                let profile = manager.player_experience_profile(&sid).unwrap();
                assert_eq!(profile.experience_rate_percent, 20);
                assert!(profile.rate_source.as_ref().unwrap().guild_stats.is_empty());
                assert!(profile
                    .rate_source
                    .as_ref()
                    .unwrap()
                    .buffs
                    .iter()
                    .all(|buff| !buff.newbie_buff));
                Ok(finish_native_death(&mut manager, &sid))
            })
            .unwrap();
        let selection = award.experience_selection.as_ref().unwrap();
        assert_eq!(selection.final_amount, 1200);
        assert!(selection.guild.is_none());
        assert_eq!(selection.guild_amount, 0);
    }
}

#[test]
fn source_authority_admission_orders_real_native_death_before_concurrent_guild_leave() {
    let config = fixture(Some("Admission Guild"));
    let leader = prepare_authority_leader(&config);
    let mut session = login(&config);
    let source = session.active_zone_experience_profile().unwrap().unwrap();
    let (mut manager, sid) = prepare_native_death(&session);
    assert!(manager.update_experience_profile(&sid, Some(source)));
    let actor = actor(&config);
    let target_name = config.default_character.name.clone();
    let (attempted_tx, attempted_rx) = std::sync::mpsc::channel();
    let (finished_tx, finished_rx) = std::sync::mpsc::channel();
    let mut worker = None;
    let award = config
        .with_shared_zone_experience_admission(|rebase| {
            rebase_admission_recipient(&mut manager, &sid, &actor, rebase);
            let peer_config = config.clone();
            worker = Some(std::thread::spawn(move || {
                // Establish that the actual publication mutex is unavailable,
                // then attempt the real hierarchy-checked membership commit.
                attempted_tx
                    .send(peer_config.account_store.try_lock().is_err())
                    .unwrap();
                let result =
                    peer_config.commit_shared_guild_remove_member(&leader, &target_name, false);
                finished_tx.send(result).unwrap();
            }));
            assert!(attempted_rx
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap());
            let award = finish_native_death(&mut manager, &sid);
            assert!(matches!(
                finished_rx.try_recv(),
                Err(std::sync::mpsc::TryRecvError::Empty)
            ));
            assert_eq!(
                award
                    .experience_selection
                    .as_ref()
                    .unwrap()
                    .guild
                    .as_ref()
                    .unwrap()
                    .membership_epoch,
                7
            );
            Ok(award)
        })
        .unwrap();
    finished_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap()
        .unwrap();
    worker.unwrap().join().unwrap();
    assert!(config.shared_guild_for_identity(&actor).unwrap().is_none());
    // A valid selection issued before leave still credits its original guild.
    session
        .try_commit_shared_monster_kill_award_with_receipt("admitted-before-leave", &award)
        .unwrap();
    assert_eq!(
        config.account_store.lock().unwrap().shared_guilds[GUILD].experience,
        12
    );
}

#[test]
fn source_authority_admission_rebases_stale_profile_after_committed_guild_leave() {
    let config = fixture(Some("Admission Guild"));
    let leader = prepare_authority_leader(&config);
    let mut session = login(&config);
    let old = session.active_zone_experience_profile().unwrap().unwrap();
    let (mut manager, sid) = prepare_native_death(&session);
    assert!(manager.update_experience_profile(&sid, Some(old)));
    config
        .commit_shared_guild_remove_member(&leader, &config.default_character.name, false)
        .unwrap();
    let actor = actor(&config);
    let award = config
        .with_shared_zone_experience_admission(|rebase| {
            rebase_admission_recipient(&mut manager, &sid, &actor, rebase);
            let zone = manager.zone_key_for_session(&sid).unwrap();
            assert!(manager
                .zone(&zone)
                .unwrap()
                .player_chat_profile(&sid)
                .unwrap()
                .guild_name
                .is_none());
            Ok(finish_native_death(&mut manager, &sid))
        })
        .unwrap();
    assert!(award.experience_selection.as_ref().unwrap().guild.is_none());
    session
        .try_commit_shared_monster_kill_award_with_receipt("admitted-after-leave", &award)
        .unwrap();
    assert_eq!(
        config.account_store.lock().unwrap().shared_guilds[GUILD].experience,
        0
    );
}

#[test]
fn normal_native_death_captures_actual_final_exp_and_credits_guild_once_across_relogin() {
    let config = fixture(Some("Earned Guild"));
    let mut session = login(&config);
    let profile = session.active_zone_experience_profile().unwrap().unwrap();
    assert_eq!(profile.experience_rate_percent, 20);
    assert_eq!(profile.guild.as_ref().unwrap().membership_epoch, 7);
    let (_manager, award) = capture(&session);
    let selection = award.experience_selection.as_ref().unwrap();
    assert_eq!(selection.final_amount, 1200);
    assert_eq!(selection.guild_amount, 12);
    let before = session.active_character_checkpoint().unwrap();
    let (receipt, replay) = session
        .try_commit_shared_monster_kill_award_with_receipt("normal/800/10", &award)
        .unwrap();
    assert!(receipt.committed && !replay);
    assert!(receipt
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GainExperience { amount: 1200 })));
    assert_eq!(
        config.account_store.lock().unwrap().shared_guilds[GUILD].experience,
        12
    );
    let after = session.active_character_checkpoint().unwrap();
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(
        after.guild_experience_journal.applied_kill_receipts.len(),
        1
    );
    let durable = config.account_store.lock().unwrap().accounts["demo"].saves
        [&config.default_character.index]
        .clone();
    assert_eq!(durable.revision, after.revision);
    assert_eq!(durable.experience, after.experience);
    assert_eq!(durable.character.level, after.character.level);
    assert_eq!(durable.guild_experience_journal.event_payloads.len(), 1);
    assert_eq!(
        serde_json::to_value(&durable.guild_experience_journal.kill_outcomes["normal/800/10"])
            .unwrap(),
        serde_json::json!({"outcome": "credited", "guild_amount": 12})
    );
    session.handle_packet(ClientPacket::LogOut);
    session.handle_packet(ClientPacket::StartGame {
        character_index: config.default_character.index,
    });
    let (receipt, replay) = session
        .try_commit_shared_monster_kill_award_with_receipt("normal/800/10", &award)
        .unwrap();
    assert!(receipt.committed && replay);
    assert!(receipt.packets.is_empty());
    assert_eq!(
        config.account_store.lock().unwrap().shared_guilds[GUILD].experience,
        12
    );
}

#[test]
fn normal_native_death_keeps_captured_guild_when_membership_changes_before_delivery() {
    let config = fixture(Some("Captured Guild"));
    let mut session = login(&config);
    let (_, award) = capture(&session);
    {
        let mut store = config.account_store.lock().unwrap();
        let old = store.shared_guilds.get_mut(GUILD).unwrap();
        // Retain another authoritative prepared leader before actor leaves.
        let mut other_leader = old.members[0].clone();
        other_leader.identity.account_id = "unrelated-prepared-leader".into();
        other_leader.identity.character_index = 999;
        other_leader.name = "Other Leader".into();
        old.members = vec![other_leader];
        old.revision += 1;
        let mut character = config.default_character.clone();
        character.index = 999;
        character.name = "Other Leader".into();
        let mut account = AccountRecord::empty();
        account.characters.push(character.clone());
        account
            .saves
            .insert(999, CharacterSaveRecord::new(character));
        store
            .accounts
            .insert("unrelated-prepared-leader".into(), account);
        store
            .shared_guilds
            .insert(OTHER.into(), guild(&config, OTHER, "Later Guild"));
    }
    assert!(
        session
            .try_commit_shared_monster_kill_award_with_receipt("original-guild/800/10", &award)
            .unwrap()
            .0
            .committed
    );
    let store = config.account_store.lock().unwrap();
    assert_eq!(store.shared_guilds[GUILD].experience, 12);
    assert_eq!(store.shared_guilds[OTHER].experience, 0);
}

#[test]
fn normal_no_guild_capture_does_not_infer_a_guild_joined_before_delivery() {
    let config = fixture(None);
    let mut session = login(&config);
    let (_, award) = capture(&session);
    assert!(award.experience_selection.as_ref().unwrap().guild.is_none());
    config
        .account_store
        .lock()
        .unwrap()
        .shared_guilds
        .insert(GUILD.into(), guild(&config, GUILD, "Later Guild"));
    assert!(
        session
            .try_commit_shared_monster_kill_award_with_receipt("no-guild/800/10", &award)
            .unwrap()
            .0
            .committed
    );
    let saved = config.account_store.lock().unwrap().accounts["demo"].saves
        [&config.default_character.index]
        .clone();
    assert!(saved.guild_experience_journal.event_payloads.is_empty());
    assert_eq!(
        config.account_store.lock().unwrap().shared_guilds[GUILD].experience,
        0
    );
    assert_eq!(
        saved.guild_experience_journal.applied_kill_receipts.len(),
        1
    );
    assert_eq!(
        serde_json::to_value(&saved.guild_experience_journal.kill_outcomes["no-guild/800/10"])
            .unwrap(),
        serde_json::json!({"outcome": "noGuild"})
    );
}

#[test]
fn source_newbie_guild_is_excluded_at_capture_and_trusted_config_controls_its_name() {
    for name in ["NewbieGuild", "Configured Novice"] {
        let mut config = fixture(Some(name));
        config.crystal_newbie_guild_name = name.into();
        let mut session = login(&config);
        let (_, award) = capture(&session);
        assert!(award.experience_selection.as_ref().unwrap().guild.is_none());
        assert_eq!(award.experience_selection.as_ref().unwrap().guild_amount, 0);
        assert!(
            session
                .try_commit_shared_monster_kill_award_with_receipt("newbie/800/10", &award)
                .unwrap()
                .0
                .committed
        );
        assert_eq!(
            config.account_store.lock().unwrap().shared_guilds[GUILD].experience,
            0
        );
        assert!(session
            .active_character_checkpoint()
            .unwrap()
            .guild_experience_journal
            .event_payloads
            .is_empty());
    }
}

#[cfg(feature = "test-support")]
#[test]
fn normal_selected_kill_file_failure_rolls_back_full_character_and_guild_then_retries_once() {
    use mir2_simulation::AccountStoreTransactionFault;
    use std::time::{SystemTime, UNIX_EPOCH};
    let directory = std::env::temp_dir().join(format!(
        "mir2-normal-earned-guild-xp-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = directory.join("accounts.json");
    let prepared = fixture(Some("Atomic Guild"));
    let prepared_store = prepared.account_store.lock().unwrap().clone();
    let config = prepared.with_account_store_path(&path);
    *config.account_store.lock().unwrap() = prepared_store;
    config.save_account_store().unwrap();
    let mut session = login(&config);
    let (_, award) = capture(&session);
    let before = session.active_character_checkpoint().unwrap();
    let disk = std::fs::read(&path).unwrap();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforeFileRename);
    assert!(session
        .try_commit_shared_monster_kill_award_with_receipt("atomic/800/10", &award)
        .is_err());
    let rolled_back = session.active_character_checkpoint().unwrap();
    assert_eq!(
        serde_json::to_value(&rolled_back).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    assert_eq!(std::fs::read(&path).unwrap(), disk);
    assert_eq!(
        config.account_store.lock().unwrap().shared_guilds[GUILD].experience,
        0
    );
    assert!(
        session
            .try_commit_shared_monster_kill_award_with_receipt("atomic/800/10", &award)
            .unwrap()
            .0
            .committed
    );
    assert_eq!(
        config.account_store.lock().unwrap().shared_guilds[GUILD].experience,
        12
    );
    drop(session);
    drop(config);
    std::fs::remove_dir_all(directory).unwrap();
}
