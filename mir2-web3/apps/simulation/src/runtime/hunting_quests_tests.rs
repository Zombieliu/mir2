use super::*;
use crate::config::{
    AccountRecord, AccountStore, AccountStoreDatabaseMode, AccountStoreRepository,
    PostgresAccountStoreRepository, SimulationConfig,
};
use crate::runtime::buffs::{BuffState, RealTimeBuffDuration};
use crate::runtime::components::{entity_by_object_id, Npc, ObjectId, Position};
use crate::runtime::npc::{
    canonical_crystal_quest_npc_object_id, ActiveNpcDialogState, NpcDialogLinkState,
};
use crate::runtime::resources::{
    mark_crystal_player_move, PlayerActionTimingResource, PlayerMovementTimingResource,
};
use mir2_protocol::{ClientPacket, MirDirection, Point};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

const QUEST: i32 = 7;

fn started(config: SimulationConfig, account: &str) -> SimulationSession {
    let mut session = SimulationSession::new(config);
    let login = session.handle_packet(ClientPacket::Login {
        account_id: account.into(),
        password: "demo".into(),
    });
    assert!(
        login
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })),
        "login fixture: {login:?}"
    );
    let entered = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(
        entered
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })),
        "StartGame fixture: {entered:?}"
    );
    session
}

/// Only fixture setup writes Ready. The requests below still traverse the real
/// packet dispatcher, active NPC dialog, exact owner and distance checks.
fn ready_session() -> SimulationSession {
    let mut session = started(SimulationConfig::default(), "demo");
    let world = session.app.world_mut();
    {
        let mut quests = world.resource_mut::<QuestResource>();
        quests.newcomer_v1_cadence = false;
        quests.newcomer_v2_cadence = false;
    }
    let info = quests::effective_crystal_quest_info_by_id(world, QUEST).unwrap();
    super::super::leveling::apply_level_change(world, info.min_level_needed.max(1) as u16);
    // Complete the actual prerequisite chain, rather than weakening eligibility.
    let mut prerequisite = info.quest_needed;
    for _ in 0..32 {
        if prerequisite <= 0 {
            break;
        }
        quests::ensure_runtime_quest(world, prerequisite);
        quests::set_quest_stage(world, prerequisite, QuestStage::Completed);
        prerequisite = quests::effective_crystal_quest_info_by_id(world, prerequisite)
            .map(|info| info.quest_needed)
            .unwrap_or(0);
    }
    quests::ensure_runtime_quest(world, QUEST);
    quests::set_quest_stage(world, QUEST, QuestStage::ReadyToTurnIn);
    let threshold = world.resource::<PlayerRuntimeResource>().max_experience;
    world.resource_mut::<PlayerRuntimeResource>().experience = threshold - 1;
    world.resource_mut::<PlayerRuntimeResource>().gold = 123;
    world.resource_mut::<PlayerRuntimeResource>().credit = 9;
    session
}

fn dialog(session: &mut SimulationSession, quest: i32, wrong_npc: bool, far: bool) -> u32 {
    let world = session.app.world_mut();
    let info = quests::effective_crystal_quest_info_by_id(world, quest).unwrap();
    let npc_id = if wrong_npc {
        4_000_000_000
    } else {
        canonical_crystal_quest_npc_object_id(info.finish_npc_index)
    };
    let position = world
        .resource::<PlayerRuntimeResource>()
        .player_position
        .clone();
    let npc_position = Point {
        x: position.x + if far { 100 } else { 1 },
        y: position.y,
    };
    if let Some(entity) = entity_by_object_id(world, npc_id) {
        world
            .entity_mut(entity)
            .insert((Npc, Position(npc_position)));
    } else {
        world.spawn((Npc, ObjectId(npc_id), Position(npc_position)));
    }
    world.resource_mut::<NpcStateResource>().active_npc_dialog = Some(ActiveNpcDialogState {
        npc_object_id: npc_id,
        npc_name: "Quest instructor".into(),
        npc_name_key: None,
        stage: Some(QuestStage::ReadyToTurnIn),
        current: 1,
        required: 1,
        title: "Quest".into(),
        body: vec![],
        footer: String::new(),
        input: None,
        links: vec![NpcDialogLinkState {
            text: "Finish".into(),
            target: format!("@quest:finish:{quest}"),
        }],
    });
    npc_id
}

fn finish_packet(session: &mut SimulationSession) -> Vec<ServerPacket> {
    session.handle_packet(ClientPacket::FinishQuest {
        quest_index: QUEST,
        selected_item_index: -1,
    })
}

#[test]
fn hunting_quest_shadow_preparation_has_no_live_projection_or_authority() {
    let mut session = ready_session();
    set_runtime_tick(session.app.world_mut(), 71);
    let tick_before_transform = runtime_tick(session.app.world());
    session.force_authoritative_player_transform(Point { x: 20, y: 21 }, MirDirection::Left);
    assert_eq!(
        runtime_tick(session.app.world()),
        tick_before_transform + 1,
        "authoritative transform setup advances the session clock before preparation"
    );
    dialog(&mut session, QUEST, false, false);
    {
        let world = session.app.world_mut();
        mark_crystal_player_move(world, false);
        world.resource_mut::<BuffResource>().buffs.push(BuffState {
            key: "bound-preparation-test".into(),
            name: "Timer".into(),
            description: String::new(),
            expires_at_tick: 200,
            real_time_duration: Some(RealTimeBuffDuration::new(60_000)),
            attack_bonus: 0,
            defence_bonus: 0,
            stats: vec![],
        });
        if let Some(skill) = world.resource_mut::<SkillResource>().skills.first_mut() {
            skill.cooldown_ends_at = 999;
        }
    }
    let world = session.app.world();
    let before = session.active_character_checkpoint().unwrap();
    let tick_before_preparation = runtime_tick(world);
    let revision = world.resource::<SessionResource>().active_save_revision();
    let config = world.resource::<RuntimeConfigResource>().config.clone();
    let store_before = serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
    let context = format!(
        "{:?}|{:?}|{:?}|{:?}|{:?}",
        world.resource::<BuffResource>(),
        world.resource::<NpcStateResource>(),
        world.resource::<RuntimeQueueResource>(),
        world.resource::<PlayerActionTimingResource>(),
        world.resource::<PlayerMovementTimingResource>()
    );
    let prepared = prepare(world, QUEST).expect("server once quest prepares");
    assert_eq!(prepared.before.gold, 123);
    assert_eq!(prepared.planned.gold, 123);
    assert!(prepared.planned.character.level > before.character.level);
    assert_eq!(prepared.planned.hp, prepared.planned.max_hp);
    assert_eq!(prepared.planned.mp, prepared.planned.max_mp);
    assert_eq!(prepared.planned.credit, before.credit);
    assert_eq!(
        prepared.planned.skill_states_json,
        prepared.before.skill_states_json
    );
    assert_eq!(
        prepared.planned.buff_states_json,
        prepared.before.buff_states_json
    );
    assert_eq!(prepared.planned.position, before.position);
    assert_eq!(prepared.planned.direction, before.direction);
    assert_eq!(
        revision,
        world.resource::<SessionResource>().active_save_revision()
    );
    let after = session.active_character_checkpoint().unwrap();
    assert_eq!(after.character, before.character);
    assert_eq!(after.experience, before.experience);
    assert_eq!(after.gold, before.gold);
    assert_eq!(after.credit, before.credit);
    assert_eq!(after.quest_states_json, before.quest_states_json);
    assert_eq!(after.position, before.position);
    assert_eq!(after.direction, before.direction);
    assert_eq!(after.skill_states_json, before.skill_states_json);
    assert_eq!(runtime_tick(world), tick_before_preparation);
    assert_eq!(
        runtime_tick(prepared.shadow.app.world()),
        tick_before_preparation
    );
    assert_eq!(
        context,
        format!(
            "{:?}|{:?}|{:?}|{:?}|{:?}",
            world.resource::<BuffResource>(),
            world.resource::<NpcStateResource>(),
            world.resource::<RuntimeQueueResource>(),
            world.resource::<PlayerActionTimingResource>(),
            world.resource::<PlayerMovementTimingResource>()
        )
    );
    assert_eq!(
        store_before,
        serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap()
    );
    prepared
        .shadow
        .app
        .world()
        .resource::<SessionResource>()
        .bind_active_save_revision(900);
    assert_eq!(
        revision,
        world.resource::<SessionResource>().active_save_revision()
    );
    let shadow_config = &prepared
        .shadow
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config;
    assert!(shadow_config.hunting_economy_realm.is_none());
    assert!(shadow_config.account_store_database_url.is_none());
    shadow_config.freeze_hunting_quest_publication("owned shadow refusal".into());
    assert!(shadow_config.ensure_account_store_writable().is_err());
    assert!(config.ensure_account_store_writable().is_ok());
}

#[test]
fn hunting_quest_definition_supports_real_once_templates_and_stable_digest() {
    let mut session = ready_session();
    for quest in [7, 10, 13, 16, 19] {
        let (_, amount) = source_definition(session.app.world(), quest).unwrap();
        assert_eq!(amount, 60);
    }
    assert!(
        source_definition(session.app.world(), 1).is_err(),
        "carry items remain unsupported"
    );
    let definition = source_definition(session.app.world(), QUEST).unwrap();
    set_runtime_tick(session.app.world_mut(), 1000);
    session
        .app
        .world()
        .resource::<SessionResource>()
        .bind_active_save_revision(99);
    session
        .app
        .world_mut()
        .resource_mut::<PlayerRuntimeResource>()
        .experience = 0;
    assert_eq!(
        definition,
        source_definition(session.app.world(), QUEST).unwrap()
    );
}

#[test]
fn hunting_quest_missing_incarnation_refuses_without_legacy_gold_or_bag_full() {
    let mut session = ready_session();
    session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config
        .hunting_economy_realm = Some("unbound-fixture".into());
    dialog(&mut session, QUEST, false, false);
    let before = session.active_character_checkpoint().unwrap();
    let packets = finish_packet(&mut session);
    assert!(packets.iter().any(|packet| matches!(packet,ServerPacket::Chat{message,..} if message.contains("economy settlement"))));
    assert!(!format!("{packets:?}")
        .to_ascii_lowercase()
        .contains("bag is full"));
    let after = session.active_character_checkpoint().unwrap();
    assert_eq!(after.gold, before.gold);
    assert_eq!(after.experience, before.experience);
    assert_eq!(after.quest_states_json, before.quest_states_json);
    assert_eq!(after.revision, before.revision);
}

#[test]
fn hunting_quest_projection_cannot_bless_a_later_complete_checkpoint_revision() {
    let mut session = ready_session();
    let before = session.active_character_checkpoint().unwrap();
    let mut later = prepare(session.app.world(), QUEST).unwrap().planned;
    later.revision = before.revision + 2;
    let projection = PersonalProjection::decode(session.app.world(), &later).unwrap();
    assert!(projection
        .apply(session.app.world_mut(), before.revision, None)
        .is_err());
    let after = session.active_character_checkpoint().unwrap();
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.quest_states_json, before.quest_states_json);
    assert_eq!(after.character, before.character);
    assert_eq!(after.experience, before.experience);
    assert_eq!(after.gold, before.gold);
}

#[test]
fn hunting_quest_shadow_preserves_existing_pool_context_when_reward_does_not_level() {
    let mut session = ready_session();
    let world = session.app.world_mut();
    let existing = PlayerVitals {
        hp: 555,
        max_hp: 777,
        mp: 333,
        max_mp: 444,
    };
    let player = player_entity(world).unwrap();
    *world.entity_mut(player).get_mut::<PlayerVitals>().unwrap() = existing;
    let mut runtime = world.resource_mut::<PlayerRuntimeResource>();
    runtime.player_vitals = existing;
    runtime.experience = 0;
    runtime.max_experience = 1_000_000;
    drop(runtime);
    let before = session.active_character_checkpoint().unwrap();
    let planned = prepare(session.app.world(), QUEST).unwrap().planned;
    assert_eq!(planned.character.level, before.character.level);
    assert_eq!(
        (planned.hp, planned.max_hp, planned.mp, planned.max_mp),
        (555, 777, 333, 444)
    );
    let live = session.active_character_checkpoint().unwrap();
    assert_eq!(
        (live.hp, live.max_hp, live.mp, live.max_mp),
        (555, 777, 333, 444)
    );
}

fn add_pool_buff(session: &mut SimulationSession) {
    session
        .app
        .world_mut()
        .resource_mut::<BuffResource>()
        .buffs
        .push(BuffState {
            key: "hunting-short-pool-buff".into(),
            name: "Pool buff".into(),
            description: String::new(),
            expires_at_tick: u64::MAX,
            real_time_duration: Some(RealTimeBuffDuration::new(60_000)),
            attack_bonus: 0,
            defence_bonus: 0,
            stats: vec![
                mir2_protocol::UserItemStat {
                    stat: 12,
                    value: 3_000,
                },
                mir2_protocol::UserItemStat {
                    stat: 13,
                    value: 2_000,
                },
            ],
        });
    super::super::stats::refresh_player_stats(session.app.world_mut());
}

fn expire_pool_buff(session: &mut SimulationSession) {
    session
        .app
        .world_mut()
        .resource_mut::<BuffResource>()
        .buffs
        .iter_mut()
        .find(|buff| buff.key == "hunting-short-pool-buff")
        .unwrap()
        .real_time_duration
        .as_mut()
        .unwrap()
        .elapse_for_test(60_000);
}

#[test]
fn hunting_quest_fresh_level_up_refills_current_caps_after_real_time_buff_expires() {
    let mut session = ready_session();
    add_pool_buff(&mut session);
    session.force_authoritative_player_vitals(Some(17), Some(11));
    let before = session.active_character_checkpoint().unwrap();
    let mut planned = prepare(session.app.world(), QUEST).unwrap().planned;
    assert!(planned.character.level > before.character.level);
    planned.revision = before.revision + 1;
    let source_caps = (planned.max_hp, planned.max_mp);
    expire_pool_buff(&mut session);
    let projection = PersonalProjection::decode(session.app.world(), &planned).unwrap();
    projection
        .apply(session.app.world_mut(), before.revision, Some(&before))
        .unwrap();
    let live = session.active_character_checkpoint().unwrap();
    let stats = super::super::stats::player_stats(session.app.world());
    assert_eq!((live.max_hp, live.max_mp), (stats.max_hp(), stats.max_mp()));
    assert!(live.max_hp < source_caps.0 && live.max_mp < source_caps.1);
    assert_eq!(
        (live.hp, live.mp),
        (live.max_hp, live.max_mp),
        "fresh ordinary LevelUp still refills"
    );
}

#[test]
fn hunting_quest_projection_preserves_zone_damage_mana_and_death_without_replaying_heal() {
    for fresh in [false, true] {
        for damage in [7, 10_000] {
            let mut session = ready_session();
            let before = session.active_character_checkpoint().unwrap();
            let mut planned = prepare(session.app.world(), QUEST).unwrap().planned;
            planned.revision = before.revision + 1;
            session.apply_zone_player_damage(damage);
            session.force_authoritative_player_vitals(None, Some(3));
            let authority = session.active_character_checkpoint().unwrap();
            assert_eq!(
                authority.revision, before.revision,
                "Zone vitals do not advance save CAS"
            );
            let projection = PersonalProjection::decode(session.app.world(), &planned).unwrap();
            projection
                .apply(
                    session.app.world_mut(),
                    before.revision,
                    fresh.then_some(&before),
                )
                .unwrap();
            let live = session.active_character_checkpoint().unwrap();
            assert_eq!(live.hp, authority.hp.min(live.max_hp));
            assert_eq!(live.mp, authority.mp.min(live.max_mp));
            if damage == 10_000 {
                assert_eq!(live.hp, 0, "recovery must not resurrect a Zone death");
            }
            assert_eq!(live.character.level, planned.character.level);
            assert_eq!(live.experience, planned.experience);
        }
    }
}

fn test_url() -> String {
    let url =
        std::env::var("MIR2_ECONOMY_TEST_DATABASE_URL").expect("explicit isolated PG fixture URL");
    assert!(url.starts_with("postgres://") || url.starts_with("postgresql://"));
    let parsed: postgres::Config = url.parse().unwrap();
    assert!(parsed
        .get_dbname()
        .is_some_and(|name| name.starts_with("mir2_hunting_acceptance_")));
    assert!(!parsed.get_hosts().is_empty() && parsed.get_hosts().iter().all(|host| matches!(host,postgres::config::Host::Tcp(name) if name.eq_ignore_ascii_case("localhost") || name.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback()))));
    assert!(parsed
        .get_hostaddrs()
        .iter()
        .all(std::net::IpAddr::is_loopback));
    url
}

struct PgFixture {
    client: postgres::Client,
    session: SimulationSession,
    account: String,
    realm: String,
}
impl PgFixture {
    fn new() -> Self {
        let base = test_url();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let schema = format!("hunting_runtime_{}_{}", std::process::id(), nonce);
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
        client
            .batch_execute(include_str!(
                "../../../../infra/postgres/migrations/0021_hunting_quest_sources.sql"
            ))
            .unwrap();
        let ready = ready_session().active_character_checkpoint().unwrap();
        let account = format!("runtime-quest-{nonce}");
        let mut record = AccountRecord::new(ready.character.clone());
        record.saves.insert(0, ready);
        let mut store = AccountStore::new(record.characters[0].clone());
        store.accounts.clear();
        store.accounts.insert(account.clone(), record);
        PostgresAccountStoreRepository::new(&url, AccountStoreDatabaseMode::SourceOfTruth)
            .save(&store)
            .unwrap();
        let realm = format!("runtime-quest-{nonce}");
        let policy = serde_json::from_value(json!({"schema_version":1,"policy_version":"runtime-quest-v1","enabled":true,
            "k":{"numerator":1,"denominator":10},"conversion_fee":{"numerator":2,"denominator":100},
            "daily_decay":{"numerator":1,"denominator":150},"baseline_daily_mint_atoms":720000,
            "daily_mint_cap_multiplier":{"numerator":3,"denominator":2},"business_timezone":"Asia/Shanghai","settlement_hour":4})).unwrap();
        mir2_economy_service::initialize_realm(&mut client, &realm, &policy).unwrap();
        let config = SimulationConfig::default()
            .with_postgres_account_store(&url)
            .unwrap()
            .with_hunting_economy_realm(&realm)
            .unwrap();
        let mut session = started(config, &account);
        session.enable_shared_guild_authority();
        assert!(session
            .app
            .world()
            .resource::<SessionResource>()
            .hunting_character_incarnation
            .is_some());
        Self {
            client,
            session,
            account,
            realm,
        }
    }
    fn counts(&mut self) -> (i64, i64, i64) {
        let row = self.client.query_one("SELECT (SELECT count(*) FROM hunting_economy_wallets),(SELECT count(*) FROM hunting_economy_operations),(SELECT count(*) FROM hunting_quest_source_checkpoints)",&[]).unwrap();
        (row.get(0), row.get(1), row.get(2))
    }
    fn durable(&mut self) -> CharacterSaveRecord {
        serde_json::from_value(self.client.query_one("SELECT snapshot_json FROM character_saves WHERE account_id=$1 AND character_index=0", &[&self.account]).unwrap().get(0)).unwrap()
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_quest_postgres_real_finish_and_call_npc_share_atomic_once_source() {
    for npc_target in [false, true] {
        let mut db = PgFixture::new();
        let before = db.session.active_character_checkpoint().unwrap();
        let expected = prepare(db.session.app.world(), QUEST).unwrap().planned;
        let npc_id = dialog(&mut db.session, QUEST, false, false);
        let packets = if npc_target {
            db.session.handle_packet(ClientPacket::CallNpc {
                object_id: npc_id,
                key: format!("@quest:finish:{QUEST}"),
            })
        } else {
            finish_packet(&mut db.session)
        };
        assert!(
            packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::CompleteQuest { .. })),
            "normal quest completion: {packets:?}"
        );
        assert_eq!(db.counts(), (1, 1, 1));
        let bound: i64 = db.client.query_one("SELECT bound_gold FROM hunting_economy_wallets WHERE realm_id=$1 AND account_id=$2", &[&db.realm,&db.account]).unwrap().get(0);
        assert_eq!(bound, 60);
        let durable = db.durable();
        let live = db.session.active_character_checkpoint().unwrap();
        assert_eq!(durable.revision, before.revision + 1);
        assert_eq!(live.revision, durable.revision);
        assert_eq!(durable.gold, before.gold);
        assert_eq!(live.gold, before.gold);
        assert_eq!(durable.character, expected.character);
        assert_eq!(live.character, expected.character);
        assert_eq!(live.experience, expected.experience);
        assert_eq!(durable.experience, expected.experience);
        assert_eq!(live.credit, expected.credit);
        assert_eq!(
            (live.hp, live.mp, live.max_hp, live.max_mp),
            (expected.hp, expected.mp, expected.max_hp, expected.max_mp)
        );
        assert_eq!(durable.quest_states_json, expected.quest_states_json);
        assert_eq!(live.quest_states_json, expected.quest_states_json);
        assert_eq!(live.position, before.position);
        assert_eq!(live.direction, before.direction);
        // A retransmitted real packet cannot make a second wallet credit or XP gain.
        dialog(&mut db.session, QUEST, false, false);
        finish_packet(&mut db.session);
        assert_eq!(db.counts(), (1, 1, 1));
        assert_eq!(db.durable().revision, durable.revision);
        assert_eq!(
            db.session.active_character_checkpoint().unwrap().experience,
            live.experience
        );
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_quest_postgres_normal_routes_reject_wrong_npc_and_distance() {
    let mut db = PgFixture::new();
    let before = db.session.active_character_checkpoint().unwrap();
    for npc_target in [false, true] {
        for (wrong_npc, far) in [(true, false), (false, true)] {
            let npc = dialog(&mut db.session, QUEST, wrong_npc, far);
            let packets = if npc_target {
                db.session.handle_packet(ClientPacket::CallNpc {
                    object_id: npc,
                    key: format!("@quest:finish:{QUEST}"),
                })
            } else {
                finish_packet(&mut db.session)
            };
            assert!(!packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::CompleteQuest { .. })));
            assert_eq!(db.counts(), (0, 0, 0));
            let live = db.session.active_character_checkpoint().unwrap();
            assert_eq!(live.revision, before.revision);
            assert_eq!(live.gold, before.gold);
            assert_eq!(live.experience, before.experience);
            assert_eq!(live.quest_states_json, before.quest_states_json);
        }
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_quest_postgres_original_packet_recovers_unknown_and_worker_loss_with_shared_guild() {
    for (npc_target, fault) in [(false, 2), (true, 5), (false, 9)] {
        let mut db = PgFixture::new();
        let before = db.session.active_character_checkpoint().unwrap();
        let original_plan = prepare(db.session.app.world(), QUEST).unwrap().planned;
        let npc_id = dialog(&mut db.session, QUEST, false, false);
        db.session
            .app
            .world_mut()
            .insert_resource(HuntingQuestTestFault(fault));
        let request = if npc_target {
            ClientPacket::CallNpc {
                object_id: npc_id,
                key: format!("@quest:finish:{QUEST}"),
            }
        } else {
            ClientPacket::FinishQuest {
                quest_index: QUEST,
                selected_item_index: -1,
            }
        };
        let first = db.session.try_handle_packet(request.clone()).unwrap();
        assert!(first.iter().any(|packet| matches!(packet,ServerPacket::Chat{message,..} if message.contains("requires recovery"))));
        assert!(!first
            .iter()
            .any(|packet| matches!(packet, ServerPacket::CompleteQuest { .. })));
        let config = db
            .session
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        assert!(config.ensure_account_store_writable().is_err());
        let untouched = db.session.active_character_checkpoint().unwrap();
        assert_eq!(untouched.revision, before.revision);
        assert_eq!(untouched.gold, before.gold);
        assert_eq!(untouched.experience, before.experience);
        assert_eq!(untouched.quest_states_json, before.quest_states_json);
        assert_eq!(db.counts(), if fault == 9 { (0, 0, 0) } else { (1, 1, 1) });

        // An unrelated XP-producing request cannot borrow the quest's freeze.
        assert!(db
            .session
            .try_handle_packet(ClientPacket::CallNpc {
                object_id: npc_id,
                key: "@Main".into(),
            })
            .is_err());
        assert!(config.ensure_account_store_writable().is_err());
        // Nor can another quest, wrong NPC owner or out-of-range request clear it.
        dialog(&mut db.session, 10, false, false);
        assert!(db
            .session
            .try_handle_packet(ClientPacket::FinishQuest {
                quest_index: 10,
                selected_item_index: -1
            })
            .is_err());
        assert!(config.ensure_account_store_writable().is_err());
        dialog(&mut db.session, QUEST, true, false);
        assert!(db.session.try_handle_packet(request.clone()).is_err());
        dialog(&mut db.session, QUEST, false, true);
        assert!(db.session.try_handle_packet(request.clone()).is_err());
        assert!(config.ensure_account_store_writable().is_err());

        let witness = db
            .session
            .app
            .world()
            .resource::<SessionResource>()
            .hunting_character_incarnation
            .clone();
        dialog(&mut db.session, QUEST, false, false);
        db.session
            .app
            .world_mut()
            .resource_mut::<SessionResource>()
            .hunting_character_incarnation = Some("pg-character-v1:another-creation".into());
        let wrong_generation = db.session.try_handle_packet(request.clone());
        if let Ok(packets) = wrong_generation {
            assert!(!packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::CompleteQuest { .. })));
        }
        assert!(config.ensure_account_store_writable().is_err());
        db.session
            .app
            .world_mut()
            .resource_mut::<SessionResource>()
            .hunting_character_incarnation = witness;

        dialog(&mut db.session, QUEST, false, false);
        // New transient EXP arithmetic cannot change a known original reward.
        if fault != 9 {
            db.session
                .app
                .world_mut()
                .resource_mut::<BuffResource>()
                .buffs
                .push(BuffState {
                    key: "late-exp-context".into(),
                    name: "Late EXP context".into(),
                    description: String::new(),
                    expires_at_tick: u64::MAX,
                    real_time_duration: None,
                    attack_bonus: 0,
                    defence_bonus: 0,
                    stats: vec![mir2_protocol::UserItemStat {
                        stat: 100,
                        value: 100,
                    }],
                });
        }
        let replay = db.session.try_handle_packet(request).unwrap();
        assert!(
            replay
                .iter()
                .any(|packet| matches!(packet, ServerPacket::CompleteQuest { .. })),
            "original recovery: {replay:?}"
        );
        assert!(config.ensure_account_store_writable().is_ok());
        assert_eq!(db.counts(), (1, 1, 1));
        let durable = db.durable();
        let live = db.session.active_character_checkpoint().unwrap();
        assert_eq!(live.revision, before.revision + 1);
        assert_eq!(live.revision, durable.revision);
        assert_eq!(live.gold, before.gold);
        assert_eq!(live.experience, durable.experience);
        if fault != 9 {
            assert_eq!(live.experience, original_plan.experience);
        }
        assert_eq!(live.quest_states_json, durable.quest_states_json);
        assert_eq!(db.client.query_one("SELECT bound_gold FROM hunting_economy_wallets WHERE realm_id=$1 AND account_id=$2", &[&db.realm,&db.account]).unwrap().get::<_,i64>(0),60);
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_quest_postgres_high_level_npc_wrapper_recovers_only_original_source() {
    use crate::world_runtime::{InProcessWorldRuntime, WorldCommand, WorldRuntime};
    let mut db = PgFixture::new();
    let before = db.session.active_character_checkpoint().unwrap();
    dialog(&mut db.session, QUEST, false, false);
    db.session
        .app
        .world_mut()
        .insert_resource(HuntingQuestTestFault(5));
    let config = db
        .session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .clone();
    let mut runtime = InProcessWorldRuntime::from_test_session(db.session);
    let original = WorldCommand::SelectNpcDialog {
        target: format!("@quest:finish:{QUEST}"),
    };
    let first = runtime.execute(original.clone()).unwrap();
    assert!(first.iter().any(|packet| matches!(packet,ServerPacket::Chat{message,..} if message.contains("requires recovery"))));
    assert!(config.ensure_account_store_writable().is_err());
    assert!(runtime
        .execute(WorldCommand::SelectNpcDialog {
            target: "@Main".into()
        })
        .is_err());
    assert!(config.ensure_account_store_writable().is_err());
    let recovered = runtime.execute(original).unwrap();
    assert!(
        recovered
            .iter()
            .any(|packet| matches!(packet, ServerPacket::CompleteQuest { .. })),
        "high-level original recovery: {recovered:?}"
    );
    assert!(config.ensure_account_store_writable().is_ok());
    db.session = runtime.into_session();
    assert_eq!(db.counts(), (1, 1, 1));
    let live = db.session.active_character_checkpoint().unwrap();
    assert_eq!(live.revision, before.revision + 1);
    assert_eq!(live.gold, before.gold);
    assert_eq!(live.quest_states_json, db.durable().quest_states_json);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_quest_postgres_unknown_recovery_preserves_zone_vitals_and_current_caps() {
    for damage in [7, 10_000] {
        let mut db = PgFixture::new();
        add_pool_buff(&mut db.session);
        db.session
            .force_authoritative_player_vitals(Some(100), Some(20));
        let before = db.session.active_character_checkpoint().unwrap();
        dialog(&mut db.session, QUEST, false, false);
        db.session
            .app
            .world_mut()
            .insert_resource(HuntingQuestTestFault(2));
        let request = ClientPacket::FinishQuest {
            quest_index: QUEST,
            selected_item_index: -1,
        };
        let first = db.session.try_handle_packet(request.clone()).unwrap();
        assert!(first.iter().any(|packet| matches!(packet,ServerPacket::Chat{message,..} if message.contains("requires recovery"))));
        let committed = db.durable();
        assert!(committed.character.level > before.character.level);
        assert!(committed.max_hp > 3_000 && committed.max_mp > 2_000);
        expire_pool_buff(&mut db.session);
        db.session.apply_zone_player_damage(damage);
        db.session.force_authoritative_player_vitals(None, Some(3));
        let zone = db.session.active_character_checkpoint().unwrap();
        assert_eq!(zone.revision, before.revision);
        let recovered = db.session.try_handle_packet(request).unwrap();
        assert!(
            recovered
                .iter()
                .any(|packet| matches!(packet, ServerPacket::CompleteQuest { .. })),
            "original recovery: {recovered:?}"
        );
        let live = db.session.active_character_checkpoint().unwrap();
        let stats = super::super::stats::player_stats(db.session.app.world());
        assert_eq!((live.max_hp, live.max_mp), (stats.max_hp(), stats.max_mp()));
        assert!(live.max_hp < committed.max_hp && live.max_mp < committed.max_mp);
        assert_eq!(live.hp, zone.hp.min(live.max_hp));
        assert_eq!(live.mp, zone.mp.min(live.max_mp));
        if damage == 10_000 {
            assert_eq!(live.hp, 0);
        }
        assert_eq!(live.revision, committed.revision);
        assert_eq!(live.experience, committed.experience);
        assert_eq!(live.quest_states_json, committed.quest_states_json);
        assert_eq!(db.counts(), (1, 1, 1));
    }
}

#[path = "hunting_quests_guild_tests.rs"]
mod guild_tests;

#[path = "hunting_daily_quests_tests.rs"]
mod daily_tests;
