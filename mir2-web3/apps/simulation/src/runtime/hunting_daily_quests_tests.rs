//! Daily source tests use the real board and kill-progress implementation.
//! Calendar boundary tests are pure; no test changes the OS or PostgreSQL clock.
use super::*;

const DAILY_IDS: [i32; 3] = [2_100_001, 2_100_002, 2_100_003];
const BONUS: i32 = 2_100_004;
const DAY_MS: u64 = 86_400_000;
const MIDNIGHT_UTC_MS: u64 = 16 * 60 * 60 * 1_000 + 20 * DAY_MS;

fn set_daily_profile(session: &mut SimulationSession) {
    let mut quests = session.app.world_mut().resource_mut::<QuestResource>();
    quests.newcomer_v1_cadence = true;
    quests.newcomer_v2_cadence = false;
}

fn daily_seed() -> SimulationSession {
    // Board24 belongs to the real per-map Crystal manifest, not the default
    // StarterScenario's small hand-authored NPC roster.
    let mut session = started(
        SimulationConfig::default().with_crystal_world_runtime(),
        "demo",
    );
    set_daily_profile(&mut session);
    super::super::super::leveling::apply_level_change(session.app.world_mut(), 20);
    // Offline synthetic starting position, beside the actual catalog Board.
    session.force_authoritative_player_transform(Point { x: 334, y: 260 }, MirDirection::Up);
    session
        .app
        .world_mut()
        .resource_mut::<QuestResource>()
        .quests
        .clear();
    let mut player = session
        .app
        .world_mut()
        .resource_mut::<PlayerRuntimeResource>();
    player.gold = 123;
    player.experience = 0;
    drop(player);
    session
}

fn ready_daily(quest_id: i32) -> SimulationSession {
    let mut session = daily_seed();
    quests::ensure_runtime_quest(session.app.world_mut(), quest_id);
    quests::set_quest_stage(session.app.world_mut(), quest_id, QuestStage::ReadyToTurnIn);
    session
}

fn state(save: &CharacterSaveRecord, quest_id: i32) -> quests::QuestState {
    save.quest_states_json
        .iter()
        .filter_map(|encoded| serde_json::from_str::<quests::QuestState>(encoded).ok())
        .find(|quest| quest.quest_id == quest_id)
        .expect("saved daily quest")
}

fn complete_option_state(session: &mut SimulationSession, quest_id: i32, period: u64) {
    quests::ensure_runtime_quest(session.app.world_mut(), quest_id);
    let mut resource = session.app.world_mut().resource_mut::<QuestResource>();
    let quest = resource
        .quests
        .iter_mut()
        .find(|quest| quest.quest_id == quest_id)
        .unwrap();
    quest.stage = QuestStage::Completed;
    quest.current = quest.required;
    quest.cadence_last_claimed_period = Some(period);
    quest.cadence_high_watermark_period = Some(period);
}

#[test]
fn hunting_daily_quest_definition_supports_only_the_real_item_free_board() {
    let mut session = daily_seed();
    for (quest_id, gold) in DAILY_IDS
        .into_iter()
        .map(|id| (id, 3_000))
        .chain([(BONUS, 5_000)])
    {
        assert!(quests::is_hunting_newcomer_daily_quest(
            session.app.world(),
            quest_id
        ));
        assert_eq!(
            source_definition(session.app.world(), quest_id).unwrap().1,
            gold
        );
    }
    for quest_id in [140, 141, 142] {
        assert!(
            source_definition(session.app.world(), quest_id).is_err(),
            "other recurring quest {quest_id}"
        );
    }
    session
        .app
        .world_mut()
        .resource_mut::<QuestResource>()
        .newcomer_v1_cadence = false;
    for quest_id in DAILY_IDS.into_iter().chain([BONUS]) {
        assert!(source_definition(session.app.world(), quest_id).is_err());
    }
}

#[test]
fn hunting_daily_quest_shadow_uses_exact_utc8_midnight_without_live_mutation() {
    let session = ready_daily(DAILY_IDS[0]);
    let before = session.active_character_checkpoint().unwrap();
    let digest = source_definition(session.app.world(), DAILY_IDS[0])
        .unwrap()
        .0;
    assert!(
        prepare(session.app.world(), DAILY_IDS[0]).is_err(),
        "daily cannot use the once/SystemTime planner"
    );
    let left =
        prepare_with_daily_time(session.app.world(), DAILY_IDS[0], Some(MIDNIGHT_UTC_MS - 1))
            .unwrap();
    let right =
        prepare_with_daily_time(session.app.world(), DAILY_IDS[0], Some(MIDNIGHT_UTC_MS)).unwrap();
    let after_four = prepare_with_daily_time(
        session.app.world(),
        DAILY_IDS[0],
        Some(MIDNIGHT_UTC_MS + 4 * 60 * 60 * 1_000),
    )
    .unwrap();
    let left_state = state(&left.planned, DAILY_IDS[0]);
    let right_state = state(&right.planned, DAILY_IDS[0]);
    assert_eq!(
        left_state.cadence_last_claimed_period.unwrap() + 1,
        right_state.cadence_last_claimed_period.unwrap()
    );
    assert_eq!(
        right_state.cadence_last_claimed_period,
        right_state.cadence_high_watermark_period
    );
    assert_eq!(
        right_state.cadence_last_claimed_period,
        state(&after_four.planned, DAILY_IDS[0]).cadence_last_claimed_period
    );
    assert_eq!(right.planned.experience, before.experience + 5_000);
    assert_eq!(right.planned.gold, before.gold);
    assert_eq!(
        right.planned.inventory_items_json,
        before.inventory_items_json
    );
    assert_eq!(
        right.planned.equipment_items_json,
        before.equipment_items_json
    );
    assert_eq!(
        source_definition(session.app.world(), DAILY_IDS[0])
            .unwrap()
            .0,
        digest,
        "period is not reward-definition identity"
    );
    let live = session.active_character_checkpoint().unwrap();
    assert_eq!(live.quest_states_json, before.quest_states_json);
    assert_eq!(
        (live.revision, live.gold, live.experience),
        (before.revision, before.gold, before.experience)
    );
}

#[test]
fn hunting_daily_quest_bonus_replans_its_two_claim_prerequisite_at_midnight() {
    let mut session = ready_daily(BONUS);
    let old_period = quests::quest_recurrence::daily_period_at(MIDNIGHT_UTC_MS - 1);
    for quest_id in &DAILY_IDS[..2] {
        complete_option_state(&mut session, *quest_id, old_period);
    }
    let before = session.active_character_checkpoint().unwrap();
    assert!(prepare_with_daily_time(session.app.world(), BONUS, Some(MIDNIGHT_UTC_MS - 1)).is_ok());
    assert!(
        prepare_with_daily_time(session.app.world(), BONUS, Some(MIDNIGHT_UTC_MS)).is_err(),
        "yesterday's two options cannot pay today's bonus"
    );
    assert_eq!(
        session
            .active_character_checkpoint()
            .unwrap()
            .quest_states_json,
        before.quest_states_json
    );
}

#[test]
fn hunting_daily_quest_shadow_rejects_group_future_and_legacy_calendar_evidence() {
    let now = MIDNIGHT_UTC_MS;
    let period = quests::quest_recurrence::daily_period_at(now);
    for kind in 0..4 {
        let mut session = ready_daily(DAILY_IDS[0]);
        let other = if kind == 3 {
            DAILY_IDS[0]
        } else {
            DAILY_IDS[2]
        };
        quests::ensure_runtime_quest(session.app.world_mut(), other);
        {
            let mut resource = session.app.world_mut().resource_mut::<QuestResource>();
            let quest = resource
                .quests
                .iter_mut()
                .find(|quest| quest.quest_id == other)
                .unwrap();
            match kind {
                0 => quest.cadence_high_watermark_period = Some(period + 1),
                1 => quest.cadence_last_claimed_period = Some(period + 1),
                2 => {
                    quest.stage = QuestStage::Completed;
                    quest.cadence_last_claimed_period = None;
                }
                _ => quest.cadence_last_claimed_period = Some(period),
            }
        }
        let before = session.active_character_checkpoint().unwrap();
        assert!(
            prepare_with_daily_time(session.app.world(), DAILY_IDS[0], Some(now)).is_err(),
            "calendar case {kind}"
        );
        assert_eq!(
            session
                .active_character_checkpoint()
                .unwrap()
                .quest_states_json,
            before.quest_states_json
        );
    }
}

fn daily_started(config: SimulationConfig, account: &str) -> SimulationSession {
    let mut session = SimulationSession::new(config);
    set_daily_profile(&mut session);
    session.enable_shared_guild_authority();
    let login = session
        .try_handle_packet(ClientPacket::Login {
            account_id: account.into(),
            password: "demo".into(),
        })
        .unwrap();
    assert!(
        login
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })),
        "daily Login: {login:?}"
    );
    let entered = session
        .try_handle_packet(ClientPacket::StartGame { character_index: 0 })
        .unwrap();
    assert!(
        entered
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })),
        "daily StartGame: {entered:?}"
    );
    session
}

struct DailyPgFixture {
    client: postgres::Client,
    session: SimulationSession,
    account: String,
    realm: String,
    url: String,
}

impl DailyPgFixture {
    fn new() -> Self {
        let base = test_url();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let schema = format!("hunting_daily_runtime_{}_{}", std::process::id(), nonce);
        postgres::Client::connect(&base, postgres::NoTls)
            .unwrap()
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .unwrap();
        let separator = if base.contains('?') { '&' } else { '?' };
        let url = format!("{base}{separator}options=-csearch_path%3D{schema}");
        let mut client = postgres::Client::connect(&url, postgres::NoTls).unwrap();
        assert_eq!(
            client
                .query_one("SELECT current_schema()", &[])
                .unwrap()
                .get::<_, String>(0),
            schema
        );
        crate::apply_migrations(&mut client).unwrap();
        let save = daily_seed().active_character_checkpoint().unwrap();
        let account = format!("daily-runtime-{nonce}");
        let mut record = AccountRecord::new(save.character.clone());
        record.saves.insert(0, save);
        let mut store = AccountStore::new(record.characters[0].clone());
        store.accounts.clear();
        store.accounts.insert(account.clone(), record);
        PostgresAccountStoreRepository::new(&url, AccountStoreDatabaseMode::SourceOfTruth)
            .save(&store)
            .unwrap();
        let realm = format!("daily-runtime-{nonce}");
        let policy = serde_json::from_value(json!({"schema_version":1,"policy_version":"daily-runtime-v1","enabled":true,
            "k":{"numerator":1,"denominator":10},"conversion_fee":{"numerator":2,"denominator":100},
            "daily_decay":{"numerator":1,"denominator":150},"baseline_daily_mint_atoms":720000,
            "daily_mint_cap_multiplier":{"numerator":3,"denominator":2},"business_timezone":"Asia/Shanghai","settlement_hour":4})).unwrap();
        mir2_economy_service::initialize_realm(&mut client, &realm, &policy).unwrap();
        let config = SimulationConfig::default()
            .with_crystal_world_runtime()
            .with_postgres_account_store(&url)
            .unwrap()
            .with_hunting_economy_realm(&realm)
            .unwrap();
        let session = daily_started(config, &account);
        Self {
            client,
            session,
            account,
            realm,
            url,
        }
    }

    fn board(&mut self) {
        let board = entity_by_object_id(self.session.app.world(), 24)
            .expect("CrystalWorld must load the real catalog Board24");
        assert!(self.session.app.world().entity(board).contains::<Npc>());
        assert!(
            super::super::super::npc::crystal_npc_object_in_data_range(
                self.session.app.world(),
                24,
            ),
            "the saved synthetic start must remain beside the actual visible Board"
        );
        self.session.handle_packet(ClientPacket::CallNpc {
            object_id: 24,
            key: "@Main".into(),
        });
        assert_eq!(
            self.session
                .app
                .world()
                .resource::<NpcStateResource>()
                .active_npc_dialog
                .as_ref()
                .map(|dialog| dialog.npc_object_id),
            Some(24),
            "real Board dialog must open"
        );
    }

    fn accept(&mut self, quest_id: i32, npc_target: bool) -> Vec<ServerPacket> {
        self.board();
        if npc_target {
            self.session.handle_packet(ClientPacket::CallNpc {
                object_id: 24,
                key: format!("@quest:accept:{quest_id}"),
            })
        } else {
            self.session.handle_packet(ClientPacket::AcceptQuest {
                npc_index: 24,
                quest_index: quest_id,
            })
        }
    }

    fn ready_by_kills(&mut self, quest_id: i32) {
        let (name, count) = match quest_id {
            2_100_001 => ("OmaFighter", 10),
            2_100_002 => ("Skeleton", 12),
            2_100_003 => ("ForestYeti", 8),
            _ => panic!("not a daily kill task"),
        };
        assert_eq!(
            quests::quest_stage(self.session.app.world(), quest_id),
            Some(QuestStage::InProgress)
        );
        for _ in 0..count {
            let _ = quests::advance_crystal_quest_kill(self.session.app.world_mut(), name);
        }
        assert_eq!(
            quests::quest_stage(self.session.app.world(), quest_id),
            Some(QuestStage::ReadyToTurnIn)
        );
    }

    fn finish(&mut self, quest_id: i32, npc_target: bool) -> Vec<ServerPacket> {
        self.board();
        self.finish_open(quest_id, npc_target)
    }

    fn finish_open(&mut self, quest_id: i32, npc_target: bool) -> Vec<ServerPacket> {
        self.session.handle_packet(if npc_target {
            ClientPacket::CallNpc {
                object_id: 24,
                key: format!("@quest:finish:{quest_id}"),
            }
        } else {
            ClientPacket::FinishQuest {
                quest_index: quest_id,
                selected_item_index: -1,
            }
        })
    }

    fn counts(&mut self) -> (i64, i64, i64, i64) {
        let row = self.client.query_one("SELECT (SELECT count(*) FROM hunting_economy_wallets),(SELECT count(*) FROM hunting_economy_operations),(SELECT count(*) FROM hunting_daily_quest_source_checkpoints),(SELECT count(*) FROM hunting_quest_source_checkpoints)", &[]).unwrap();
        (row.get(0), row.get(1), row.get(2), row.get(3))
    }

    fn bound(&mut self) -> i64 {
        self.client.query_opt("SELECT bound_gold FROM hunting_economy_wallets WHERE realm_id=$1 AND account_id=$2", &[&self.realm, &self.account]).unwrap().map(|row| row.get(0)).unwrap_or(0)
    }

    fn durable(&mut self) -> CharacterSaveRecord {
        serde_json::from_value(self.client.query_one("SELECT snapshot_json FROM character_saves WHERE account_id=$1 AND character_index=0", &[&self.account]).unwrap().get(0)).unwrap()
    }

    fn fresh_config(&self) -> SimulationConfig {
        SimulationConfig::default()
            .with_crystal_world_runtime()
            .with_postgres_account_store(&self.url)
            .unwrap()
            .with_hunting_economy_realm(&self.realm)
            .unwrap()
    }
}

fn completed(packets: &[ServerPacket], quest_id: i32) -> bool {
    packets.iter().any(|packet| matches!(packet, ServerPacket::CompleteQuest { completed_quests } if completed_quests.contains(&quest_id)))
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_daily_quest_postgres_normal_board_two_options_bonus_and_cold_login() {
    for pair in [[0, 1], [0, 2], [1, 2]] {
        let mut db = DailyPgFixture::new();
        let baseline = db.session.active_character_checkpoint().unwrap();
        assert!(baseline.max_experience > 20_000);
        assert!(!db.accept(BONUS, false).is_empty());
        for (offset, index) in pair.into_iter().enumerate() {
            let id = DAILY_IDS[index];
            assert!(!db.accept(id, offset == 1).is_empty());
            db.ready_by_kills(id);
            assert!(completed(&db.finish(id, offset == 1), id));
        }
        assert_eq!(
            quests::quest_stage(db.session.app.world(), BONUS),
            Some(QuestStage::ReadyToTurnIn)
        );
        let third = DAILY_IDS
            .into_iter()
            .find(|id| {
                !pair
                    .into_iter()
                    .map(|index| DAILY_IDS[index])
                    .any(|picked| picked == *id)
            })
            .unwrap();
        let denied = db.accept(third, false);
        assert!(!denied.iter().any(|packet| matches!(packet, ServerPacket::ChangeQuest { quest_id, taken: true, .. } if *quest_id == third)));
        assert!(completed(&db.finish(BONUS, false), BONUS));
        assert_eq!(db.counts(), (1, 3, 3, 0));
        assert_eq!(db.bound(), 11_000);
        let durable = db.durable();
        let live = db.session.active_character_checkpoint().unwrap();
        assert_eq!(live.experience, baseline.experience + 20_000);
        assert_eq!(durable.experience, live.experience);
        assert_eq!(live.gold, baseline.gold);
        assert_eq!(durable.gold, baseline.gold);
        assert_eq!(live.quest_states_json, durable.quest_states_json);
        assert_eq!(
            state(&durable, BONUS).cadence_last_claimed_period,
            state(&durable, DAILY_IDS[pair[0]]).cadence_last_claimed_period
        );
        assert!(!completed(&db.finish(BONUS, true), BONUS));
        assert_eq!(db.bound(), 11_000);
        db.session.try_handle_packet(ClientPacket::LogOut).unwrap();
        db.session = daily_started(db.fresh_config(), &db.account);
        assert_eq!(
            quests::quest_stage(db.session.app.world(), BONUS),
            Some(QuestStage::Completed)
        );
        assert_eq!(
            db.session.active_character_checkpoint().unwrap().experience,
            live.experience
        );
        assert_eq!(db.counts(), (1, 3, 3, 0));
        assert_eq!(db.bound(), 11_000);
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_daily_quest_postgres_source_rollback_publishes_no_partial_reward() {
    let mut db = DailyPgFixture::new();
    db.accept(DAILY_IDS[0], false);
    db.ready_by_kills(DAILY_IDS[0]);
    // Ordinary Board entry initializes cadence metadata before the source
    // starts. Compare the reward transaction against that legitimate state.
    db.board();
    let live_before = db.session.active_character_checkpoint().unwrap();
    let durable_before = db.durable();
    db.session
        .app
        .world_mut()
        .insert_resource(HuntingQuestTestFault(1));
    assert!(!completed(
        &db.finish_open(DAILY_IDS[0], false),
        DAILY_IDS[0]
    ));
    assert_eq!(db.counts(), (0, 0, 0, 0));
    assert_eq!(db.durable().revision, durable_before.revision);
    assert_eq!(
        db.session
            .active_character_checkpoint()
            .unwrap()
            .quest_states_json,
        live_before.quest_states_json
    );
    assert_eq!(
        db.session.active_character_checkpoint().unwrap().experience,
        live_before.experience
    );
    assert!(completed(&db.finish(DAILY_IDS[0], true), DAILY_IDS[0]));
    assert_eq!(db.bound(), 3_000);
    assert_eq!(db.counts(), (1, 1, 1, 0));
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_daily_quest_postgres_original_unknown_request_keeps_reward_and_vitals() {
    for fault in [2, 5, 9] {
        let mut db = DailyPgFixture::new();
        db.accept(DAILY_IDS[0], false);
        db.ready_by_kills(DAILY_IDS[0]);
        let before = db.session.active_character_checkpoint().unwrap();
        db.board();
        db.session
            .app
            .world_mut()
            .insert_resource(HuntingQuestTestFault(fault));
        let first = db.finish_open(DAILY_IDS[0], false);
        assert!(first.iter().any(|packet| matches!(packet, ServerPacket::Chat { message, .. } if message.contains("requires recovery"))));
        assert_eq!(
            db.session.active_character_checkpoint().unwrap().experience,
            before.experience
        );
        db.session.apply_zone_player_damage(7);
        let hurt = db.session.active_character_checkpoint().unwrap();
        let replay = db.finish_open(DAILY_IDS[0], false);
        assert!(
            completed(&replay, DAILY_IDS[0]),
            "daily original recovery: {replay:?}"
        );
        let after = db.session.active_character_checkpoint().unwrap();
        assert_eq!(after.hp, hurt.hp.min(after.max_hp));
        assert_eq!(after.experience, before.experience + 5_000);
        assert_eq!(after.gold, before.gold);
        assert_eq!(db.bound(), 3_000);
        assert_eq!(db.counts(), (1, 1, 1, 0));
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_daily_quest_postgres_lost_session_ordinary_login_resolves_original_slot() {
    let mut db = DailyPgFixture::new();
    db.accept(DAILY_IDS[2], true);
    db.ready_by_kills(DAILY_IDS[2]);
    db.board();
    db.session
        .app
        .world_mut()
        .insert_resource(HuntingQuestTestFault(5));
    assert!(!completed(
        &db.finish_open(DAILY_IDS[2], true),
        DAILY_IDS[2]
    ));
    let config = db
        .session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .clone();
    assert!(config.ensure_account_store_writable().is_err());
    // Drop the old World before the ordinary new Login. The shared config's
    // private fence, not a reconstructed current-day source, authorizes lookup.
    db.session = SimulationSession::new(config.clone());
    db.session = daily_started(config.clone(), &db.account);
    assert!(config.ensure_account_store_writable().is_ok());
    assert_eq!(
        quests::quest_stage(db.session.app.world(), DAILY_IDS[2]),
        Some(QuestStage::Completed)
    );
    assert_eq!(db.bound(), 3_000);
    assert_eq!(db.counts(), (1, 1, 1, 0));
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_daily_quest_postgres_controlled_period_change_replans_full_source_once() {
    let mut db = DailyPgFixture::new();
    db.accept(DAILY_IDS[1], true);
    db.ready_by_kills(DAILY_IDS[1]);
    let before = db.session.active_character_checkpoint().unwrap();
    db.board();
    // Test-only first callback refusal, not an actual midnight or clock change.
    db.session
        .app
        .world_mut()
        .insert_resource(HuntingQuestTestFault(11));
    assert!(completed(
        &db.finish_open(DAILY_IDS[1], false),
        DAILY_IDS[1]
    ));
    assert_eq!(db.bound(), 3_000);
    assert_eq!(db.counts(), (1, 1, 1, 0));
    assert_eq!(db.durable().revision, before.revision + 1);
    assert_eq!(
        db.session.active_character_checkpoint().unwrap().experience,
        before.experience + 5_000
    );
}
