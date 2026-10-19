//! Real membership, event epoch, Guild XP, and current server buff coverage.
//! Every test retains the parent's loopback/database-name/schema isolation guard.
use super::*;
use crate::config::{
    SharedGuildBuff, SharedGuildMember, SharedGuildRank, SharedGuildRecord, Stage5FriendIdentity,
};
use std::collections::{BTreeMap, BTreeSet};

const MEMBER_EPOCH: u64 = 2;
const PERSONAL_EXP_RATE: i32 = 400;
const FINAL_EXPERIENCE: u32 = 241;
const GUILD_EXPERIENCE: u64 = 2;

fn login_guild_member(config: SimulationConfig, account: &str) -> SimulationSession {
    let mut session = SimulationSession::new(config);
    // Gateway enables this authority before Login/StartGame. Hydration must see
    // the actual membership and active Guild pool buffs, not a later flag flip.
    session.enable_shared_guild_authority();
    let login = session
        .try_handle_packet(ClientPacket::Login {
            account_id: account.into(),
            password: "demo".into(),
        })
        .unwrap();
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let entered = session
        .try_handle_packet(ClientPacket::StartGame { character_index: 0 })
        .unwrap();
    assert!(
        entered
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })),
        "member StartGame: {entered:?}"
    );
    assert!(session
        .app
        .world()
        .resource::<SessionResource>()
        .hunting_character_incarnation
        .is_some());
    session
}

struct GuildFixture {
    db: PgFixture,
    id: String,
    identity: Stage5FriendIdentity,
}
impl GuildFixture {
    fn new() -> Self {
        let mut db = PgFixture::new();
        let config = db
            .session
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        let identity = Stage5FriendIdentity {
            account_id: db.account.clone(),
            character_index: 0,
        };
        let id = format!(
            "{:032x}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let leader_account = format!("guild-leader-{}", db.account);
        let mut leader = config.default_character.clone();
        leader.index = 1;
        leader.name = "GuildLeader".into();
        leader.level = 22;
        let recruiter = Stage5FriendIdentity {
            account_id: leader_account.clone(),
            character_index: 1,
        };
        let settings = mir2_game_data::crystal_guild_settings();
        assert_eq!(settings.experience_rate, 0.01_f32);
        assert_eq!(settings.experience_levels[0], 45_000);
        // Real shipped definitions: Field Experience (+4%), Max Health (+20),
        // Max Mana (+20). The source rule set, not a name-only guild blob, owns
        // these stats. No Guild clock process is started by this fixture.
        let buffs = [100, 12, 13]
            .into_iter()
            .map(|stat| {
                let definition = mir2_game_data::crystal_guild_buff_definitions()
                    .iter()
                    .find(|definition| {
                        definition
                            .stats
                            .iter()
                            .any(|entry| entry.stat == stat && entry.value > 0)
                    })
                    .unwrap();
                assert_eq!(definition.level_requirement, 0);
                assert!(definition.time_limit > 0);
                (
                    definition.id,
                    SharedGuildBuff {
                        id: definition.id,
                        active: true,
                        remaining_minutes: definition.time_limit,
                    },
                )
            })
            .collect();
        let guild = SharedGuildRecord {
            id: id.clone(),
            name: "RewardGuild".into(),
            revision: 1,
            level: 0,
            experience: settings.experience_levels[0] as u64 - 1,
            spare_points: 0,
            gold: 44,
            ranks: vec![
                SharedGuildRank {
                    index: 0,
                    name: "Leader".into(),
                    options: 255,
                },
                SharedGuildRank {
                    index: 1,
                    name: "Members".into(),
                    options: 0,
                },
            ],
            members: vec![SharedGuildMember {
                membership_epoch: 1,
                identity: recruiter.clone(),
                name: leader.name.clone(),
                rank_index: 0,
            }],
            notice: vec![],
            storage: BTreeMap::new(),
            buffs,
            last_buff_tick_ms: 0,
            experience_receipts: BTreeSet::new(),
            experience_receipt_payloads: BTreeMap::new(),
        };
        // Fixture provisioning itself uses the real PG account/Guild writer,
        // including member FK rows and source versions, before any quest claim.
        config
            .commit_account_store_transaction_with_guilds(
                &[db.account.clone(), leader_account.clone()],
                std::slice::from_ref(&id),
                |store| {
                    store
                        .accounts
                        .insert(leader_account, AccountRecord::new(leader));
                    store.shared_guilds.insert(id.clone(), guild);
                    Ok(())
                },
            )
            .unwrap();
        // Exercise the real membership domain writer: the recruit receives its
        // own epoch and FK row; a mere session flag or legacy guild name cannot
        // produce the journal evidence verified by these tests.
        let joined = config
            .commit_shared_guild_join(&recruiter, &identity, &id)
            .unwrap();
        assert_eq!(
            joined.member(&identity).unwrap().membership_epoch,
            MEMBER_EPOCH
        );
        db.session = login_guild_member(config, &db.account);
        db.session
            .app
            .world_mut()
            .resource_mut::<BuffResource>()
            .buffs
            .push(BuffState {
                key: "hunting-guild-exp-context".into(),
                name: "Server EXP buff fixture".into(),
                description: String::new(),
                expires_at_tick: u64::MAX,
                real_time_duration: Some(RealTimeBuffDuration::new(120_000)),
                attack_bonus: 0,
                defence_bonus: 0,
                stats: vec![mir2_protocol::UserItemStat {
                    stat: 100,
                    value: PERSONAL_EXP_RATE,
                }],
            });
        let stats = super::super::super::stats::player_stats(db.session.app.world());
        assert_eq!(stats.get(100), 404);
        let selected = db
            .session
            .app
            .world()
            .resource::<SessionResource>()
            .selected_character
            .as_ref()
            .unwrap();
        let (base_hp, base_mp) = crate::config::crystal_base_vitals(selected.class, selected.level);
        assert_eq!(
            (stats.max_hp(), stats.max_mp()),
            (base_hp + 20, base_mp + 20)
        );
        let member_count:i64=db.client.query_one("SELECT count(*) FROM shared_guild_members WHERE guild_id=$1 AND account_id=$2 AND character_index=0",&[&id,&db.account]).unwrap().get(0);
        assert_eq!(member_count, 1);
        let fixture = Self { db, id, identity };
        let memory = fixture
            .config()
            .shared_guild_for_identity(&fixture.identity)
            .unwrap()
            .unwrap();
        assert_eq!(
            memory.member(&fixture.identity).unwrap().membership_epoch,
            MEMBER_EPOCH
        );
        assert_eq!(memory.member(&fixture.identity).unwrap().rank_index, 1);
        fixture
    }

    fn config(&self) -> SimulationConfig {
        self.db
            .session
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone()
    }

    fn durable_guild(&mut self) -> (SharedGuildRecord, i64) {
        let row = self
            .db
            .client
            .query_one(
                "SELECT raw_json,store_version FROM shared_guilds WHERE guild_id=$1",
                &[&self.id],
            )
            .unwrap();
        (serde_json::from_value(row.get(0)).unwrap(), row.get(1))
    }

    fn expected_event(&self) -> crate::config::GuildExperienceEvent {
        let prepared = prepare(self.db.session.app.world(), QUEST).unwrap();
        let events = &prepared.planned.guild_experience_journal.pending;
        assert_eq!(
            events.len(),
            1,
            "the isolated real quest must journal its own complete EXP gain"
        );
        let event = events[0].clone();
        assert_eq!(event.identity, self.identity);
        assert_eq!(event.guild_id, self.id);
        assert_eq!(event.membership_epoch, MEMBER_EPOCH);
        assert_eq!(event.final_amount, FINAL_EXPERIENCE);
        assert!(
            event.capture_hash.is_none(),
            "an ordinary quest is not a shared-kill permit"
        );
        assert_eq!(event.source_revision, prepared.before.revision);
        assert_eq!(
            prepared.planned.character.level,
            prepared.before.character.level + 1
        );
        assert_eq!(prepared.planned.experience, 240);
        event
    }

    fn assert_committed(
        &mut self,
        before: &CharacterSaveRecord,
        guild_before: &SharedGuildRecord,
        version_before: i64,
        event: &crate::config::GuildExperienceEvent,
        fresh: bool,
    ) -> CharacterSaveRecord {
        let durable = self.db.durable();
        let live = self.db.session.active_character_checkpoint().unwrap();
        let (guild, version) = self.durable_guild();
        let hash = crate::config::guild_experience::source_event_hash(event).unwrap();
        let mut expected_guild = guild_before.clone();
        expected_guild.experience += GUILD_EXPERIENCE;
        expected_guild.level += 1;
        expected_guild.spare_points += mir2_game_data::crystal_guild_settings().points_per_level;
        expected_guild.revision += 1;
        expected_guild
            .experience_receipts
            .insert(event.event_id.clone());
        expected_guild
            .experience_receipt_payloads
            .insert(event.event_id.clone(), hash.clone());
        assert_eq!(guild, expected_guild);
        assert_eq!(
            version,
            version_before + 1,
            "outer Guild XP guard must not publish a second transaction"
        );
        assert_eq!(
            self.config()
                .shared_guild_for_identity(&self.identity)
                .unwrap()
                .unwrap(),
            guild
        );
        assert_eq!(durable.revision, before.revision + 1);
        assert_eq!(live.revision, durable.revision);
        assert_eq!(durable.character.level, before.character.level + 1);
        assert_eq!(live.character, durable.character);
        assert_eq!((durable.experience, durable.max_experience), (240, 600));
        assert_eq!(live.experience, durable.experience);
        assert_eq!((durable.gold, live.gold), (before.gold, before.gold));
        assert_eq!(durable.credit, before.credit);
        assert_eq!(live.quest_states_json, durable.quest_states_json);
        assert_eq!(
            quests::quest_stage(self.db.session.app.world(), QUEST),
            Some(QuestStage::Completed)
        );
        assert_eq!(
            durable.guild_experience_journal.next_sequence,
            event.sequence
        );
        assert!(durable.guild_experience_journal.pending.is_empty());
        assert_eq!(durable.guild_experience_journal.event_payloads.len(), 1);
        assert_eq!(
            durable.guild_experience_journal.event_payloads[&event.event_id],
            hash
        );
        assert_eq!(
            live.guild_experience_journal,
            durable.guild_experience_journal
        );
        assert!(durable
            .guild_experience_journal
            .applied_kill_receipts
            .is_empty());
        let stats = super::super::super::stats::player_stats(self.db.session.app.world());
        assert_eq!((live.max_hp, live.max_mp), (stats.max_hp(), stats.max_mp()));
        if fresh {
            assert_eq!((live.hp, live.mp), (live.max_hp, live.max_mp));
        }
        let receipt=self.db.client.query_one("SELECT checkpoint_json,bound_gold,before_revision,after_revision FROM hunting_quest_source_checkpoints WHERE realm_id=$1 AND account_id=$2 AND quest_id=$3",&[&self.db.realm,&self.db.account,&QUEST]).unwrap();
        assert_eq!(
            receipt.get::<_, serde_json::Value>(0),
            serde_json::to_value(&durable).unwrap()
        );
        assert_eq!(receipt.get::<_, i64>(1), 60);
        assert_eq!(receipt.get::<_, i64>(2), before.revision as i64);
        assert_eq!(receipt.get::<_, i64>(3), durable.revision as i64);
        let bound:i64=self.db.client.query_one("SELECT bound_gold FROM hunting_economy_wallets WHERE realm_id=$1 AND account_id=$2",&[&self.db.realm,&self.db.account]).unwrap().get(0);
        assert_eq!(bound, 60);
        assert_eq!(self.db.counts(), (1, 1, 1));
        durable
    }
}

fn quest_request() -> ClientPacket {
    ClientPacket::FinishQuest {
        quest_index: QUEST,
        selected_item_index: -1,
    }
}

fn completed(packets: &[ServerPacket]) -> bool {
    packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::CompleteQuest { .. }))
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_quest_postgres_real_guild_member_commits_exp_level_buffs_and_bound_gold() {
    let mut fixture = GuildFixture::new();
    let before = fixture.db.session.active_character_checkpoint().unwrap();
    assert_eq!(
        (
            before.character.level,
            before.experience,
            before.max_experience
        ),
        (4, 399, 400)
    );
    let (guild_before, version_before) = fixture.durable_guild();
    let event = fixture.expected_event();
    assert_eq!(
        fixture.durable_guild(),
        (guild_before.clone(), version_before),
        "planning must not publish Guild XP"
    );
    dialog(&mut fixture.db.session, QUEST, false, false);
    let packets = fixture
        .db
        .session
        .try_handle_packet(quest_request())
        .unwrap();
    assert!(completed(&packets), "ordinary guild quest: {packets:?}");
    let committed = fixture.assert_committed(&before, &guild_before, version_before, &event, true);
    fixture
        .db
        .session
        .try_handle_packet(quest_request())
        .unwrap();
    assert_eq!(fixture.durable_guild().0.experience, 45_001);
    assert_eq!(fixture.db.durable().revision, committed.revision);

    fixture
        .db
        .session
        .try_handle_packet(ClientPacket::LogOut)
        .unwrap();
    let config = fixture.config();
    let url = config.account_store_database_url.as_ref().unwrap();
    let fresh = SimulationConfig::default()
        .with_postgres_account_store(url)
        .unwrap()
        .with_hunting_economy_realm(&fixture.db.realm)
        .unwrap();
    fixture.db.session = login_guild_member(fresh, &fixture.db.account);
    let reloaded = fixture.db.session.active_character_checkpoint().unwrap();
    assert_eq!(reloaded.character.level, 5);
    assert_eq!(reloaded.experience, 240);
    assert_eq!(
        reloaded.guild_experience_journal,
        committed.guild_experience_journal
    );
    assert_eq!(
        fixture
            .config()
            .shared_guild_for_identity(&fixture.identity)
            .unwrap()
            .unwrap()
            .experience,
        45_001
    );
    assert_eq!(
        super::super::super::stats::player_stats(fixture.db.session.app.world()).get(100),
        404
    );
    dialog(&mut fixture.db.session, QUEST, false, false);
    fixture
        .db
        .session
        .try_handle_packet(quest_request())
        .unwrap();
    assert_eq!(fixture.durable_guild().0.experience_receipts.len(), 1);
    assert_eq!(fixture.db.counts(), (1, 1, 1));
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_quest_postgres_real_guild_member_unknown_replays_one_exp_and_wallet_source() {
    let mut fixture = GuildFixture::new();
    let before = fixture.db.session.active_character_checkpoint().unwrap();
    let (guild_before, version_before) = fixture.durable_guild();
    let event = fixture.expected_event();
    let npc = dialog(&mut fixture.db.session, QUEST, false, false);
    fixture
        .db
        .session
        .app
        .world_mut()
        .insert_resource(HuntingQuestTestFault(2));
    let request = ClientPacket::CallNpc {
        object_id: npc,
        key: format!("@quest:finish:{QUEST}"),
    };
    let first = fixture
        .db
        .session
        .try_handle_packet(request.clone())
        .unwrap();
    assert!(!completed(&first));
    assert!(fixture.config().ensure_account_store_writable().is_err());
    assert_eq!(fixture.db.counts(), (1, 1, 1));
    assert_eq!(fixture.durable_guild().0.experience, 45_001);
    let unchanged = fixture.db.session.active_character_checkpoint().unwrap();
    assert_eq!(unchanged.experience, before.experience);
    assert_eq!(unchanged.revision, before.revision);
    assert_eq!(
        unchanged.guild_experience_journal,
        before.guild_experience_journal
    );
    assert_eq!(
        fixture.config().account_store.lock().unwrap().shared_guilds[&fixture.id],
        guild_before
    );
    fixture
        .db
        .session
        .app
        .world_mut()
        .resource_mut::<BuffResource>()
        .buffs
        .iter_mut()
        .find(|buff| buff.key == "hunting-guild-exp-context")
        .unwrap()
        .stats[0]
        .value = 800;
    let recovered = fixture
        .db
        .session
        .try_handle_packet(request.clone())
        .unwrap();
    assert!(
        completed(&recovered),
        "original Guild source recovery: {recovered:?}"
    );
    assert!(fixture.config().ensure_account_store_writable().is_ok());
    let durable = fixture.assert_committed(&before, &guild_before, version_before, &event, false);
    assert_eq!(
        super::super::super::stats::player_stats(fixture.db.session.app.world()).get(100),
        804
    );
    fixture.db.session.try_handle_packet(request).unwrap();
    assert_eq!(fixture.durable_guild().0.experience_receipts.len(), 1);
    assert_eq!(fixture.db.durable().revision, durable.revision);
    assert_eq!(fixture.db.counts(), (1, 1, 1));
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_quest_postgres_real_guild_member_precommit_failure_rolls_back_complete_reward() {
    let mut fixture = GuildFixture::new();
    let before = fixture.db.session.active_character_checkpoint().unwrap();
    let durable_before = fixture.db.durable();
    let (guild_before, version_before) = fixture.durable_guild();
    let event = fixture.expected_event();
    let memory_before =
        serde_json::to_value(&*fixture.config().account_store.lock().unwrap()).unwrap();
    dialog(&mut fixture.db.session, QUEST, false, false);
    fixture
        .db
        .session
        .app
        .world_mut()
        .insert_resource(HuntingQuestTestFault(1));
    let failed = fixture
        .db
        .session
        .try_handle_packet(quest_request())
        .unwrap();
    assert!(!completed(&failed));
    assert!(fixture.config().ensure_account_store_writable().is_ok());
    assert_eq!(fixture.db.counts(), (0, 0, 0));
    assert_eq!(
        fixture.durable_guild(),
        (guild_before.clone(), version_before)
    );
    assert_eq!(
        serde_json::to_value(fixture.db.durable()).unwrap(),
        serde_json::to_value(durable_before).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&*fixture.config().account_store.lock().unwrap()).unwrap(),
        memory_before
    );
    let live = fixture.db.session.active_character_checkpoint().unwrap();
    assert_eq!(live.revision, before.revision);
    assert_eq!(live.character, before.character);
    assert_eq!(live.experience, before.experience);
    assert_eq!(
        (live.hp, live.max_hp, live.mp, live.max_mp),
        (before.hp, before.max_hp, before.mp, before.max_mp)
    );
    assert_eq!(live.gold, before.gold);
    assert_eq!(live.quest_states_json, before.quest_states_json);
    assert_eq!(live.inventory_items_json, before.inventory_items_json);
    assert_eq!(
        live.guild_experience_journal,
        before.guild_experience_journal
    );
    let retried = fixture
        .db
        .session
        .try_handle_packet(quest_request())
        .unwrap();
    assert!(
        completed(&retried),
        "rolled-back original source remains claimable: {retried:?}"
    );
    fixture.assert_committed(&before, &guild_before, version_before, &event, true);
}
