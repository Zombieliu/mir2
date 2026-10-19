use super::*;
use crate::config::{
    AccountRecord, AccountStore, AccountStoreDatabaseMode, AccountStoreRepository, EquipmentSlot,
    PostgresAccountStoreRepository, SimulationConfig,
};
use crate::runtime::equipment::{equipment_slot_unique_id, seed_equipment_items};
use crate::runtime::resources::PlayerRuntimeResource;
use mir2_economy_service::{
    BoundDebitBurnReceipt, BurnLayer, Idempotent, WalletBalance, WalletOwner,
};
use mir2_protocol::{ClientPacket, MirDirection, Point};
use std::time::{SystemTime, UNIX_EPOCH};

fn config() -> SimulationConfig {
    let mut config = SimulationConfig::default();
    for (object_id, script_key) in [
        (4990, "BichonProvince/NaturalCave/WickedTrader"),
        (4988, "BichonProvince/BichonWall/Blacksmith-0"),
    ] {
        config.visible_npcs.push(crate::VisibleNpcRecord {
            object_id,
            name: "Repair source fixture".into(),
            image: 5,
            colour_argb: -1,
            position: Point { x: 331, y: 271 },
            direction: MirDirection::Left,
            quest_ids: Vec::new(),
            script_key: Some(script_key.into()),
        });
    }
    config
}

fn started(config: SimulationConfig, account: &str) -> SimulationSession {
    let mut session = SimulationSession::new(config);
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: account.into(),
            password: "demo".into()
        })
        .iter()
        .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    let entered = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(
        entered
            .iter()
            .any(|p| matches!(p, ServerPacket::StartGame { result: 4, .. })),
        "{entered:?}"
    );
    session
}

fn service(session: &mut SimulationSession, special: bool) {
    session.interact(if special { 4988 } else { 4990 });
    session.select_npc_dialog_target(if special { "@SRepair" } else { "@Repair" });
}

fn setup(free: bool) -> SimulationSession {
    let mut session = started(config(), "demo");
    let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
    if !inventory
        .equipment_items
        .iter()
        .any(|item| item.slot == EquipmentSlot::Weapon)
    {
        inventory
            .equipment_items
            .push(seed_equipment_items().remove(0));
    }
    let weapon = inventory
        .equipment_items
        .iter_mut()
        .find(|item| item.slot == EquipmentSlot::Weapon)
        .unwrap();
    weapon.durability_max = 1_000;
    weapon.durability_current = if free { 1_000 } else { 100 };
    drop(inventory);
    service(&mut session, false);
    session
}

fn uid() -> u64 {
    equipment_slot_unique_id(EquipmentSlot::Weapon).unwrap()
}

fn local_quote(session: &mut SimulationSession) -> LocalQuote {
    session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config
        .hunting_economy_realm = Some("unit-repair".into());
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .hunting_character_incarnation = Some("pg-character-v1:unit-repair".into());
    // This is an explicit projection-unit fixture only. PG tests below obtain
    // the scope through the actual authenticated StartGame hook.
    bind_start_game_authority(session.app.world_mut());
    let actor = actor(session.app.world()).unwrap();
    let plan = prepare_repair_plan(session.app.world(), uid(), false).unwrap();
    LocalQuote {
        actor,
        view: HuntingRepairQuoteView {
            quote_id: "a".repeat(64),
            unique_id: uid(),
            special: false,
            cost_bound_gold: u64::from(plan.cost()),
            created_at_ms: 1,
            expires_at_ms: i64::MAX,
        },
        source_json: quote_json(&plan, uid(), false).unwrap(),
        witness: session.app.repair_mutation_witness().unwrap(),
        before_revision: session.active_character_checkpoint().unwrap().revision,
        plan,
        phase: Phase::Pending,
        authority: RepairAuthority::capture(
            &session
                .app
                .world()
                .resource::<RuntimeConfigResource>()
                .config,
        ),
    }
}

fn commit(session: &SimulationSession, quote: &LocalQuote) -> HuntingRepairCommit {
    let before = session.active_character_checkpoint().unwrap();
    let mut checkpoint = planned_save(session.app.world(), &quote.plan, &before).unwrap();
    checkpoint.revision = before.revision + 1;
    HuntingRepairCommit {
        current_revision: Some(checkpoint.revision),
        checkpoint,
        receipt: Idempotent {
            replayed: false,
            receipt: HuntingRepairReceipt::Charged(BoundDebitBurnReceipt {
                realm_id: "unit-repair".into(),
                operation_key: "unit-operation".into(),
                request_digest: "unit-digest".into(),
                owner: WalletOwner {
                    account_id: quote.actor.account.clone(),
                    character_index: quote.actor.index,
                    incarnation: quote.actor.incarnation.clone(),
                },
                source_kind: "npc-repair".into(),
                source_key: quote.view.quote_id.clone(),
                bound_gold_debit: quote.view.cost_bound_gold,
                burn_event_id: "unit-burn".into(),
                layer: BurnLayer::Daily,
                valuation_version: "bound-gold-face-v1".into(),
                policy_version: "unit-v1".into(),
                business_day: 1,
                committed_at_ms: 1,
                wallet_after: WalletBalance {
                    bound_gold: 987_654,
                    unbound_gold: 222,
                    revision: 9,
                },
            }),
        },
    }
}

#[test]
fn hunting_repair_action_rejects_client_fee_and_old_uid_only_confirmation() {
    assert!(serde_json::from_value::<HuntingRepairAction>(
        json!({"kind":"quote","uniqueId":0,"special":false,"amount":1})
    )
    .is_err());
    assert!(
        serde_json::from_value::<HuntingRepairAction>(json!({"kind":"confirm","uniqueId":0}))
            .is_err()
    );
    let request = serde_json::from_value::<HuntingRepairAction>(
        json!({"kind":"confirm","quoteId":"a".repeat(64)}),
    )
    .unwrap();
    assert!(matches!(request,HuntingRepairAction::Confirm{quote_id} if quote_id=="a".repeat(64)));
}

#[test]
fn hunting_repair_default_disabled_keeps_real_legacy_repair() {
    let mut session = setup(false);
    assert!(matches!(
        session
            .execute_hunting_repair(HuntingRepairAction::Quote {
                unique_id: uid(),
                special: false
            })
            .reply,
        HuntingRepairReply::Rejected {
            code: HuntingRepairRejection::Disabled,
            ..
        }
    ));
    session
        .app
        .world_mut()
        .resource_mut::<PlayerRuntimeResource>()
        .gold = 1_000_000;
    let before = session.active_character_checkpoint().unwrap();
    let packets = session.handle_packet(ClientPacket::RepairItem { unique_id: uid() });
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::ItemRepaired { .. })));
    assert!(session.active_character_checkpoint().unwrap().gold < before.gold);
}

#[test]
fn hunting_repair_enabled_raw_normal_and_special_packets_cannot_debit_or_repair() {
    for special in [false, true] {
        let mut session = setup(false);
        service(&mut session, special);
        session
            .app
            .world_mut()
            .resource_mut::<RuntimeConfigResource>()
            .config
            .hunting_economy_realm = Some("unit-repair".into());
        let before = session.active_character_checkpoint().unwrap();
        let packets = session.handle_packet(if special {
            ClientPacket::SRepairItem { unique_id: uid() }
        } else {
            ClientPacket::RepairItem { unique_id: uid() }
        });
        assert!(!packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ItemRepaired { .. } | ServerPacket::LoseGold { .. }
        )));
        let after = session.active_character_checkpoint().unwrap();
        assert_eq!(before.gold, after.gold);
        assert_eq!(before.equipment_items_json, after.equipment_items_json);
        assert_eq!(before.revision, after.revision);
    }
}

#[test]
fn hunting_repair_plan_isolated_and_uses_repaired_equipment_caps_without_healing() {
    let mut session = setup(false);
    {
        let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
        let weapon = inventory
            .equipment_items
            .iter_mut()
            .find(|item| item.slot == EquipmentSlot::Weapon)
            .unwrap();
        weapon.durability_current = 0;
        weapon.added_stats.push(mir2_protocol::UserItemStat {
            stat: 12,
            value: 1_000,
        });
    }
    super::super::stats::refresh_player_stats(session.app.world_mut());
    session.force_authoritative_player_vitals(Some(7), Some(3));
    let quote = local_quote(&mut session);
    assert_eq!(
        quote.source_json["costBoundGold"],
        json!(quote.view.cost_bound_gold.to_string())
    );
    let before = session.active_character_checkpoint().unwrap();
    let witness = session.app.repair_mutation_witness().unwrap();
    let planned = planned_save(session.app.world(), &quote.plan, &before).unwrap();
    assert!(planned.max_hp > before.max_hp);
    assert_eq!(planned.hp, before.hp);
    assert_eq!(planned.mp, before.mp);
    assert_eq!(planned.gold, before.gold);
    let live = session.active_character_checkpoint().unwrap();
    assert_eq!(live.equipment_items_json, before.equipment_items_json);
    assert_eq!(
        (live.hp, live.max_hp, live.revision),
        (before.hp, before.max_hp, before.revision)
    );
    assert!(session.app.repair_mutation_witness_matches(&witness));
}

#[test]
fn hunting_repair_projection_keeps_zone_vitals_and_emits_no_legacy_wallet_debit() {
    let mut session = setup(false);
    let quote = local_quote(&mut session);
    let commit = commit(&session, &quote);
    session.force_authoritative_player_vitals(Some(4), Some(2));
    let outcome = session.publish_hunting_repair_result(
        &quote.actor,
        &quote.view.quote_id,
        Some(&quote),
        commit,
    );
    assert!(matches!(
        outcome.reply,
        HuntingRepairReply::Committed {
            reload_required: false,
            ..
        }
    ));
    assert!(!outcome
        .packets
        .iter()
        .any(|p| matches!(p, ServerPacket::LoseGold { .. })));
    let live = session.active_character_checkpoint().unwrap();
    assert_eq!((live.hp, live.mp), (4, 2));
    assert_eq!(live.revision, quote.before_revision + 1);
}

#[test]
fn hunting_repair_projection_same_value_aba_actually_retires_old_authority() {
    let mut session = setup(false);
    let quote = local_quote(&mut session);
    let commit = commit(&session, &quote);
    let same = session.app.world().resource::<InventoryResource>().clone();
    *session.app.world_mut().resource_mut::<InventoryResource>() = same;
    let outcome = session.publish_hunting_repair_result(
        &quote.actor,
        &quote.view.quote_id,
        Some(&quote),
        commit,
    );
    assert!(matches!(
        outcome.reply,
        HuntingRepairReply::Committed {
            reload_required: true,
            ..
        }
    ));
    assert!(outcome
        .packets
        .iter()
        .any(|p| matches!(p, ServerPacket::LogOutSuccess { .. })));
    let state = session.app.world().resource::<SessionResource>();
    assert!(state.account_id.is_none() && state.selected_character.is_none());
    assert!(
        state.active_save_revision().is_none() && state.hunting_character_incarnation.is_none()
    );
    assert!(session.active_character_checkpoint().is_none());
    assert!(session.prepare_hunting_repair_teardown().unwrap());
    assert!(session.prepare_hunting_repair_teardown().unwrap());
}

#[test]
fn hunting_repair_empty_teardown_is_not_proof_of_retirement() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    assert!(!session.prepare_hunting_repair_teardown().unwrap());
    assert!(session.reconcile_hunting_repair_login_authority().is_ok());
    session.set_hunting_repair_test_fault(2).unwrap();
    assert_eq!(session.app.world().resource::<RepairTestFault>().0, 2);
    assert!(session.set_hunting_repair_test_fault(6).is_err());
    session.set_hunting_repair_test_fault(0).unwrap();
    assert!(!session.app.world().contains_resource::<RepairTestFault>());
}

#[test]
fn hunting_repair_login_reconciliation_is_a_noop_for_existing_world() {
    let mut session = setup(false);
    session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config
        .hunting_economy_realm = Some("must-not-open-a-repository".into());
    let before = session.active_character_checkpoint().unwrap();
    let witness = session.app.repair_mutation_witness().unwrap();
    assert!(session.reconcile_hunting_repair_login_authority().is_ok());
    let after = session.active_character_checkpoint().unwrap();
    assert_eq!(before.revision, after.revision);
    assert_eq!(before.equipment_items_json, after.equipment_items_json);
    assert!(session.app.repair_mutation_witness_matches(&witness));
}

#[test]
fn hunting_repair_projection_later_checkpoint_cannot_bless_stale_inventory() {
    let mut session = setup(false);
    let quote = local_quote(&mut session);
    let mut commit = commit(&session, &quote);
    commit.current_revision = Some(commit.checkpoint.revision + 1);
    let outcome = session.publish_hunting_repair_result(
        &quote.actor,
        &quote.view.quote_id,
        Some(&quote),
        commit,
    );
    assert!(matches!(
        outcome.reply,
        HuntingRepairReply::Committed {
            reload_required: true,
            ..
        }
    ));
    assert!(session.active_identity().is_none());
}

#[test]
fn hunting_repair_authority_rebind_cannot_publish_old_local_quote() {
    let mut session = setup(false);
    let quote = local_quote(&mut session);
    let commit = commit(&session, &quote);
    session.rebind_account_store(&SimulationConfig::default());
    let result = session.publish_hunting_repair_result(
        &quote.actor,
        &quote.view.quote_id,
        Some(&quote),
        commit,
    );
    assert!(matches!(
        result.reply,
        HuntingRepairReply::Committed {
            reload_required: true,
            ..
        }
    ));
    assert!(session.active_identity().is_none());
}

#[test]
fn hunting_repair_applied_query_does_not_repair_new_damage_or_emit_historical_balance() {
    let mut session = setup(false);
    let mut quote = local_quote(&mut session);
    let first = commit(&session, &quote);
    let mut commit = commit(&session, &quote);
    let applied = session.publish_hunting_repair_result(
        &quote.actor,
        &quote.view.quote_id,
        Some(&quote),
        first,
    );
    assert!(matches!(
        applied.reply,
        HuntingRepairReply::Committed {
            reload_required: false,
            ..
        }
    ));
    commit.receipt.replayed = true;
    quote.phase = Phase::Applied;
    session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .equipment_items
        .iter_mut()
        .find(|item| item.slot == EquipmentSlot::Weapon)
        .unwrap()
        .durability_current = 1;
    let before = session.active_character_checkpoint().unwrap();
    let outcome = session.publish_hunting_repair_result(
        &quote.actor,
        &quote.view.quote_id,
        Some(&quote),
        commit,
    );
    assert!(outcome.packets.is_empty());
    let after = session.active_character_checkpoint().unwrap();
    assert_eq!(after.equipment_items_json, before.equipment_items_json);
    let reply = serde_json::to_string(&outcome.reply).unwrap();
    assert!(!reply.contains("wallet") && !reply.contains("987654"));
}

fn applied_fixture() -> (SimulationSession, LocalQuote, HuntingRepairCommit) {
    let mut session = setup(false);
    let mut quote = local_quote(&mut session);
    let first = commit(&session, &quote);
    let mut replay = commit(&session, &quote);
    let applied = session.publish_hunting_repair_result(
        &quote.actor,
        &quote.view.quote_id,
        Some(&quote),
        first,
    );
    assert!(matches!(
        applied.reply,
        HuntingRepairReply::Committed {
            reload_required: false,
            ..
        }
    ));
    quote.phase = Phase::Applied;
    replay.receipt.replayed = true;
    (session, quote, replay)
}

#[test]
fn hunting_repair_applied_query_later_durable_revision_retires_stale_world() {
    let (mut session, quote, mut replay) = applied_fixture();
    replay.current_revision = Some(replay.checkpoint.revision + 1);
    let result = session.publish_hunting_repair_result(
        &quote.actor,
        &quote.view.quote_id,
        Some(&quote),
        replay,
    );
    assert!(matches!(
        result.reply,
        HuntingRepairReply::Committed {
            reload_required: true,
            ..
        }
    ));
    assert!(session.active_identity().is_none());
    assert!(session
        .app
        .world()
        .get_resource::<StartGameRepairAuthority>()
        .is_none());
}

#[test]
fn hunting_repair_applied_query_rebound_authority_retires_even_at_same_revision() {
    let (mut session, quote, replay) = applied_fixture();
    let mut replacement = SimulationConfig::default();
    replacement.hunting_economy_realm = Some("unit-repair".into());
    session.rebind_account_store(&replacement);
    let result = session.publish_hunting_repair_result(
        &quote.actor,
        &quote.view.quote_id,
        Some(&quote),
        replay,
    );
    assert!(matches!(
        result.reply,
        HuntingRepairReply::Committed {
            reload_required: true,
            ..
        }
    ));
    assert!(session.active_identity().is_none());
}

#[test]
fn hunting_repair_no_local_quote_same_revision_without_startgame_proof_retires() {
    let (mut session, quote, replay) = applied_fixture();
    session
        .app
        .world_mut()
        .remove_resource::<StartGameRepairAuthority>();
    let result =
        session.publish_hunting_repair_result(&quote.actor, &quote.view.quote_id, None, replay);
    assert!(matches!(
        result.reply,
        HuntingRepairReply::Committed {
            reload_required: true,
            ..
        }
    ));
    assert!(session.active_identity().is_none());
}

#[test]
fn hunting_repair_no_local_quote_with_bound_canonical_scope_only_returns_receipt() {
    let (mut session, quote, replay) = applied_fixture();
    let result =
        session.publish_hunting_repair_result(&quote.actor, &quote.view.quote_id, None, replay);
    assert!(matches!(
        result.reply,
        HuntingRepairReply::Committed {
            reload_required: false,
            ..
        }
    ));
    assert!(result.packets.is_empty());
    assert!(session.active_identity().is_some());
}

#[test]
fn hunting_repair_quote_requires_startgame_scope_before_database_or_recurrence() {
    let mut session = setup(false);
    bind_start_game_authority(session.app.world_mut());
    assert!(
        session
            .app
            .world()
            .get_resource::<StartGameRepairAuthority>()
            .is_none(),
        "disabled does not create scope"
    );
    let quote = local_quote(&mut session);
    let mut replacement = SimulationConfig::default();
    replacement.hunting_economy_realm = Some("unit-repair".into());
    session.rebind_account_store(&replacement);
    let result = session.execute_hunting_repair(HuntingRepairAction::Quote {
        unique_id: uid(),
        special: false,
    });
    assert!(matches!(
        result.reply,
        HuntingRepairReply::Rejected {
            code: HuntingRepairRejection::Unauthenticated,
            ..
        }
    ));
    assert!(!quote.authority.matches(session.app.world()));
}

fn test_url() -> String {
    let url =
        std::env::var("MIR2_ECONOMY_TEST_DATABASE_URL").expect("explicit isolated loopback PG URL");
    assert!(url.starts_with("postgres://") || url.starts_with("postgresql://"));
    let parsed: postgres::Config = url.parse().unwrap();
    assert!(parsed
        .get_dbname()
        .is_some_and(|name| name.starts_with("mir2_hunting_acceptance_")));
    assert!(!parsed.get_hosts().is_empty() && parsed.get_hosts().iter().all(|host|matches!(host,postgres::config::Host::Tcp(name) if name.eq_ignore_ascii_case("localhost") || name.parse::<std::net::IpAddr>().is_ok_and(|ip|ip.is_loopback()))));
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
    fn new(free: bool, fund: bool) -> Self {
        let base = test_url();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let schema = format!("repair_runtime_{}_{}", std::process::id(), nonce);
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
        let save = setup(free).active_character_checkpoint().unwrap();
        let account = format!("runtime-repair-{nonce}");
        let mut record = AccountRecord::new(save.character.clone());
        record.saves.insert(0, save.clone());
        let mut store = AccountStore::new(save.character.clone());
        store.accounts.clear();
        store.accounts.insert(account.clone(), record);
        PostgresAccountStoreRepository::new(&url, AccountStoreDatabaseMode::SourceOfTruth)
            .save(&store)
            .unwrap();
        let realm = format!("runtime-repair-{nonce}");
        let policy=serde_json::from_value(json!({"schema_version":1,"policy_version":"runtime-repair-v1","enabled":true,
            "k":{"numerator":1,"denominator":10},"conversion_fee":{"numerator":2,"denominator":100},"daily_decay":{"numerator":1,"denominator":150},
            "baseline_daily_mint_atoms":720000,"daily_mint_cap_multiplier":{"numerator":3,"denominator":2},"business_timezone":"Asia/Shanghai","settlement_hour":4})).unwrap();
        mir2_economy_service::initialize_realm(&mut client, &realm, &policy).unwrap();
        let config = config()
            .with_postgres_account_store(&url)
            .unwrap()
            .with_hunting_economy_realm(&realm)
            .unwrap();
        let mut session = started(config, &account);
        session.enable_shared_guild_authority();
        service(&mut session, false);
        if fund {
            let actor = actor(session.app.world()).unwrap();
            mir2_economy_service::credit_bound_authorized_with_source(
                &mut client,
                &realm,
                "runtime-test-funding",
                "runtime-test-funding-v1",
                1_000_000,
                |_, _| {
                    Ok(WalletOwner {
                        account_id: actor.account,
                        character_index: actor.index,
                        incarnation: actor.incarnation,
                    })
                },
                |_, _| Ok(()),
            )
            .unwrap();
        }
        Self {
            client,
            session,
            account,
            realm,
        }
    }
    fn quote(&mut self) -> HuntingRepairQuoteView {
        match self
            .session
            .execute_hunting_repair(HuntingRepairAction::Quote {
                unique_id: uid(),
                special: false,
            })
            .reply
        {
            HuntingRepairReply::Quoted { quote } => quote,
            other => panic!("quote: {other:?}"),
        }
    }
    fn durable(&mut self) -> CharacterSaveRecord {
        serde_json::from_value(self.client.query_one("SELECT snapshot_json FROM character_saves WHERE account_id=$1 AND character_index=0",&[&self.account]).unwrap().get(0)).unwrap()
    }
    fn balance(&mut self) -> Option<i64> {
        self.client.query_opt("SELECT bound_gold FROM hunting_economy_wallets WHERE realm_id=$1 AND account_id=$2",&[&self.realm,&self.account]).unwrap().map(|row|row.get(0))
    }
    fn burns(&mut self) -> i64 {
        self.client
            .query_one(
                "SELECT count(*) FROM hunting_economy_burn_events WHERE realm_id=$1",
                &[&self.realm],
            )
            .unwrap()
            .get(0)
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_real_npc_quote_confirm_query_debits_and_repairs_once() {
    let mut db = PgFixture::new(false, true);
    let before = db.durable();
    let quote = db.quote();
    assert!(quote.cost_bound_gold > 0);
    assert_eq!(db.balance(), Some(1_000_000));
    assert_eq!(db.burns(), 0);
    let confirmed = db
        .session
        .execute_hunting_repair(HuntingRepairAction::Confirm {
            quote_id: quote.quote_id.clone(),
        });
    assert!(
        matches!(
            confirmed.reply,
            HuntingRepairReply::Committed {
                reload_required: false,
                ..
            }
        ),
        "{:?}",
        confirmed.reply
    );
    assert!(confirmed
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::ItemRepaired { .. })));
    assert_eq!(db.balance(), Some(1_000_000 - quote.cost_bound_gold as i64));
    assert_eq!(db.burns(), 1);
    let durable = db.durable();
    let live = db.session.active_character_checkpoint().unwrap();
    assert_eq!(durable.revision, before.revision + 1);
    assert_eq!(live.revision, durable.revision);
    assert_eq!(live.equipment_items_json, durable.equipment_items_json);
    assert_eq!(live.gold, before.gold);
    db.session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .equipment_items
        .iter_mut()
        .find(|item| item.slot == EquipmentSlot::Weapon)
        .unwrap()
        .durability_current = 1;
    let damaged = db
        .session
        .active_character_checkpoint()
        .unwrap()
        .equipment_items_json;
    for action in [
        HuntingRepairAction::Confirm {
            quote_id: quote.quote_id.clone(),
        },
        HuntingRepairAction::Query {
            quote_id: quote.quote_id.clone(),
        },
    ] {
        let retried = db.session.execute_hunting_repair(action);
        assert!(matches!(
            retried.reply,
            HuntingRepairReply::Committed {
                reload_required: false,
                replayed: true,
                ..
            }
        ));
        assert!(retried.packets.is_empty());
        assert_eq!(
            db.session
                .active_character_checkpoint()
                .unwrap()
                .equipment_items_json,
            damaged
        );
    }
    assert_eq!(db.balance(), Some(1_000_000 - quote.cost_bound_gold as i64));
    assert_eq!(db.burns(), 1);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_zero_cost_commits_source_without_wallet_or_burn() {
    let mut db = PgFixture::new(true, false);
    let before = db.durable();
    let quote = db.quote();
    assert_eq!(quote.cost_bound_gold, 0);
    let result = db
        .session
        .execute_hunting_repair(HuntingRepairAction::Confirm {
            quote_id: quote.quote_id.clone(),
        });
    assert!(
        matches!(
            result.reply,
            HuntingRepairReply::Committed {
                receipt: HuntingRepairReceiptView {
                    free: true,
                    cost_bound_gold: 0,
                    ..
                },
                reload_required: false,
                ..
            }
        ),
        "{:?}",
        result.reply
    );
    assert!(db.balance().is_none());
    assert_eq!(db.burns(), 0);
    assert_eq!(db.durable().revision, before.revision + 1);
    assert!(matches!(
        db.session
            .execute_hunting_repair(HuntingRepairAction::Query {
                quote_id: quote.quote_id
            })
            .reply,
        HuntingRepairReply::Committed { replayed: true, .. }
    ));
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_real_special_service_charges_quoted_cost_without_max_loss() {
    let mut db = PgFixture::new(false, true);
    service(&mut db.session, true);
    let original = db
        .session
        .app
        .world()
        .resource::<InventoryResource>()
        .equipment_items
        .iter()
        .find(|item| item.slot == EquipmentSlot::Weapon)
        .unwrap()
        .clone();
    let expected = prepare_repair_plan(db.session.app.world(), uid(), true)
        .unwrap()
        .cost();
    let quote = match db
        .session
        .execute_hunting_repair(HuntingRepairAction::Quote {
            unique_id: uid(),
            special: true,
        })
        .reply
    {
        HuntingRepairReply::Quoted { quote } => quote,
        reply => panic!("{reply:?}"),
    };
    assert_eq!(quote.cost_bound_gold, u64::from(expected));
    let result = db
        .session
        .execute_hunting_repair(HuntingRepairAction::Confirm {
            quote_id: quote.quote_id,
        });
    assert!(
        matches!(
            result.reply,
            HuntingRepairReply::Committed {
                reload_required: false,
                ..
            }
        ),
        "{:?}",
        result.reply
    );
    let repaired = &db
        .session
        .app
        .world()
        .resource::<InventoryResource>()
        .equipment_items
        .iter()
        .find(|item| item.slot == EquipmentSlot::Weapon)
        .unwrap();
    assert_eq!(repaired.durability_max, original.durability_max);
    assert_eq!(repaired.durability_current, repaired.durability_max);
    assert_eq!(db.balance(), Some(1_000_000 - i64::from(expected)));
    assert_eq!(db.burns(), 1);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_unknown_query_preserves_zone_damage_and_retries_no_debit() {
    let mut db = PgFixture::new(false, true);
    let before = db.session.active_character_checkpoint().unwrap();
    let quote = db.quote();
    db.session
        .app
        .world_mut()
        .insert_resource(RepairTestFault(2));
    let result = db
        .session
        .execute_hunting_repair(HuntingRepairAction::Confirm {
            quote_id: quote.quote_id.clone(),
        });
    assert!(
        matches!(result.reply, HuntingRepairReply::Pending { .. }),
        "{:?}",
        result.reply
    );
    assert_eq!(
        db.session.active_character_checkpoint().unwrap().revision,
        before.revision
    );
    assert_eq!(db.balance(), Some(1_000_000 - quote.cost_bound_gold as i64));
    db.session
        .force_authoritative_player_vitals(Some(3), Some(1));
    db.session
        .app
        .world_mut()
        .resource_mut::<NpcStateResource>()
        .active_npc_service = None;
    let query = db
        .session
        .execute_hunting_repair(HuntingRepairAction::Query {
            quote_id: quote.quote_id.clone(),
        });
    assert!(
        matches!(
            query.reply,
            HuntingRepairReply::Committed {
                reload_required: false,
                replayed: true,
                ..
            }
        ),
        "{:?}",
        query.reply
    );
    assert_eq!(
        (
            db.session.active_character_checkpoint().unwrap().hp,
            db.session.active_character_checkpoint().unwrap().mp
        ),
        (3, 1)
    );
    assert_eq!(db.burns(), 1);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_same_value_aba_and_far_npc_reject_before_debit() {
    for aba in [true, false] {
        let mut db = PgFixture::new(false, true);
        let before = db.durable();
        let quote = db.quote();
        if aba {
            let same = db
                .session
                .app
                .world()
                .resource::<InventoryResource>()
                .clone();
            *db.session
                .app
                .world_mut()
                .resource_mut::<InventoryResource>() = same;
        } else {
            db.session
                .force_authoritative_player_transform(Point { x: 10, y: 10 }, MirDirection::Up);
        }
        let result = db
            .session
            .execute_hunting_repair(HuntingRepairAction::Confirm {
                quote_id: quote.quote_id,
            });
        assert!(matches!(
            result.reply,
            HuntingRepairReply::Rejected {
                code: HuntingRepairRejection::ItemChanged,
                ..
            }
        ));
        assert_eq!(db.balance(), Some(1_000_000));
        assert_eq!(db.burns(), 0);
        assert_eq!(db.durable().revision, before.revision);
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_precommit_rollback_and_unknown_changed_item_actual_logout() {
    for fault in [1, 2] {
        let mut db = PgFixture::new(false, true);
        let before = db.durable();
        let quote = db.quote();
        db.session
            .app
            .world_mut()
            .insert_resource(RepairTestFault(fault));
        let result = db
            .session
            .execute_hunting_repair(HuntingRepairAction::Confirm {
                quote_id: quote.quote_id.clone(),
            });
        if fault == 1 {
            assert!(matches!(result.reply, HuntingRepairReply::Rejected { .. }));
            assert_eq!(db.balance(), Some(1_000_000));
            assert_eq!(db.burns(), 0);
            assert_eq!(db.durable().revision, before.revision);
        } else {
            assert!(matches!(result.reply, HuntingRepairReply::Pending { .. }));
            let same = db
                .session
                .app
                .world()
                .resource::<InventoryResource>()
                .clone();
            *db.session
                .app
                .world_mut()
                .resource_mut::<InventoryResource>() = same;
            let result = db
                .session
                .execute_hunting_repair(HuntingRepairAction::Query {
                    quote_id: quote.quote_id,
                });
            assert!(matches!(
                result.reply,
                HuntingRepairReply::Committed {
                    reload_required: true,
                    ..
                }
            ));
            assert!(db.session.active_identity().is_none());
            assert!(db
                .session
                .app
                .world()
                .resource::<SessionResource>()
                .active_save_revision()
                .is_none());
            assert_eq!(db.burns(), 1);
        }
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_lost_session_login_loads_committed_source_with_new_scope() {
    for fault in [2, 5] {
        let mut db = PgFixture::new(false, true);
        let before = db.session.active_character_checkpoint().unwrap();
        let quote = db.quote();
        let old_witness = db.session.app.repair_mutation_witness().unwrap();
        let authority = db
            .session
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        db.session.set_hunting_repair_test_fault(fault).unwrap();
        assert!(matches!(
            db.session
                .execute_hunting_repair(HuntingRepairAction::Confirm {
                    quote_id: quote.quote_id.clone()
                })
                .reply,
            HuntingRepairReply::Pending { .. }
        ));
        assert!(authority.ensure_account_store_writable().is_err());
        let after = db.durable();
        assert_eq!(after.revision, before.revision + 1);
        assert_eq!(
            db.session.active_character_checkpoint().unwrap().revision,
            before.revision
        );
        // Preserve the exact authority Arcs, then actually drop the original
        // World. No explicit Query or reconciliation precedes the new Login.
        let PgFixture {
            client,
            session,
            account,
            realm,
        } = db;
        drop(session);
        let session = started(authority.clone(), &account);
        let mut db = PgFixture {
            client,
            session,
            account,
            realm,
        };
        assert!(authority.ensure_account_store_writable().is_ok());
        let loaded = db.session.active_character_checkpoint().unwrap();
        assert_eq!(loaded.revision, after.revision);
        assert_eq!(loaded.inventory_items_json, after.inventory_items_json);
        assert_eq!(loaded.equipment_items_json, after.equipment_items_json);
        assert_eq!(loaded.gold, before.gold);
        let resumed_actor = actor(db.session.app.world()).unwrap();
        assert!(start_game_authority_matches(
            db.session.app.world(),
            &resumed_actor
        ));
        assert!(!db.session.app.world().contains_resource::<LocalQuotes>());
        assert_ne!(
            db.session.app.repair_mutation_witness().unwrap(),
            old_witness
        );
        let result = db
            .session
            .execute_hunting_repair(HuntingRepairAction::Query {
                quote_id: quote.quote_id.clone(),
            });
        assert!(matches!(
            result.reply,
            HuntingRepairReply::Committed {
                replayed: true,
                reload_required: false,
                ..
            }
        ));
        assert!(result.packets.is_empty());
        assert_eq!(db.balance(), Some(1_000_000 - quote.cost_bound_gold as i64));
        assert_eq!(db.burns(), 1);
        assert_eq!(db.durable().revision, after.revision);
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_lost_session_login_proves_rolled_back_source_without_debit() {
    let mut db = PgFixture::new(false, true);
    let before = db.durable();
    let quote = db.quote();
    let authority = db
        .session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .clone();
    db.session.set_hunting_repair_test_fault(4).unwrap();
    assert!(matches!(
        db.session
            .execute_hunting_repair(HuntingRepairAction::Confirm {
                quote_id: quote.quote_id.clone()
            })
            .reply,
        HuntingRepairReply::Pending { .. }
    ));
    assert!(authority.ensure_account_store_writable().is_err());
    let PgFixture {
        client,
        session,
        account,
        realm,
    } = db;
    drop(session);
    let session = started(authority.clone(), &account);
    let mut db = PgFixture {
        client,
        session,
        account,
        realm,
    };
    assert!(authority.ensure_account_store_writable().is_ok());
    let loaded = db.session.active_character_checkpoint().unwrap();
    assert_eq!(loaded.revision, before.revision);
    assert_eq!(loaded.equipment_items_json, before.equipment_items_json);
    assert_eq!(loaded.inventory_items_json, before.inventory_items_json);
    assert_eq!(db.balance(), Some(1_000_000));
    assert_eq!(db.burns(), 0);
    let result = db
        .session
        .execute_hunting_repair(HuntingRepairAction::Query {
            quote_id: quote.quote_id,
        });
    assert!(matches!(
        result.reply,
        HuntingRepairReply::Rejected {
            code: HuntingRepairRejection::NotSubmitted,
            ..
        }
    ));
    assert!(result.packets.is_empty());
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_teardown_queries_original_before_capturing_or_saving() {
    for fault in [2, 4, 5] {
        let mut db = PgFixture::new(false, true);
        let before = db.durable();
        let quote = db.quote();
        db.session.set_hunting_repair_test_fault(fault).unwrap();
        assert!(matches!(
            db.session
                .execute_hunting_repair(HuntingRepairAction::Confirm {
                    quote_id: quote.quote_id.clone()
                })
                .reply,
            HuntingRepairReply::Pending { .. }
        ));
        db.session
            .force_authoritative_player_vitals(Some(3), Some(1));
        db.session
            .app
            .world_mut()
            .resource_mut::<NpcStateResource>()
            .active_npc_service = None;
        assert!(!db.session.prepare_hunting_repair_teardown().unwrap());
        let prepared = db.session.active_character_checkpoint().unwrap();
        let canonical = db.durable();
        assert_eq!(prepared.revision, canonical.revision);
        assert_eq!(
            prepared.equipment_items_json,
            canonical.equipment_items_json
        );
        assert_eq!(
            prepared.inventory_items_json,
            canonical.inventory_items_json
        );
        assert_eq!((prepared.hp, prepared.mp), (3, 1));
        assert_eq!(prepared.gold, before.gold);
        db.session.save_active_character_for_logout().unwrap();
        let saved = db.durable();
        assert_eq!(saved.revision, prepared.revision + 1);
        assert_eq!(saved.equipment_items_json, prepared.equipment_items_json);
        assert_eq!((saved.hp, saved.mp), (3, 1));
        assert_eq!(db.burns(), i64::from(fault != 4));
        assert_eq!(
            db.balance(),
            Some(if fault == 4 {
                1_000_000
            } else {
                1_000_000 - quote.cost_bound_gold as i64
            })
        );
        assert!(!db.session.prepare_hunting_repair_teardown().unwrap());
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_teardown_unsafe_projection_retires_idempotently_until_startgame() {
    let mut db = PgFixture::new(false, true);
    let quote = db.quote();
    db.session.set_hunting_repair_test_fault(2).unwrap();
    assert!(matches!(
        db.session
            .execute_hunting_repair(HuntingRepairAction::Confirm {
                quote_id: quote.quote_id
            })
            .reply,
        HuntingRepairReply::Pending { .. }
    ));
    let after = db.durable();
    let same = db
        .session
        .app
        .world()
        .resource::<InventoryResource>()
        .clone();
    *db.session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>() = same;
    assert!(db.session.prepare_hunting_repair_teardown().unwrap());
    assert!(db.session.active_identity().is_none());
    assert!(db.session.active_character_checkpoint().is_none());
    assert!(db.session.prepare_hunting_repair_teardown().unwrap());
    assert_eq!(db.durable().revision, after.revision);
    assert_eq!(db.burns(), 1);
    assert!(db
        .session
        .handle_packet(ClientPacket::Login {
            account_id: db.account.clone(),
            password: "demo".into()
        })
        .iter()
        .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    assert!(db
        .session
        .handle_packet(ClientPacket::StartGame { character_index: 0 })
        .iter()
        .any(|p| matches!(p, ServerPacket::StartGame { result: 4, .. })));
    assert!(!db
        .session
        .app
        .world()
        .contains_resource::<RepairAuthorityRetired>());
    assert!(!db.session.prepare_hunting_repair_teardown().unwrap());
    assert_eq!(
        db.session
            .active_character_checkpoint()
            .unwrap()
            .equipment_items_json,
        after.equipment_items_json
    );
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_teardown_after_new_ban_or_creation_never_saves_old_world() {
    for lifecycle in [
        "banned",
        "recreated",
        "deleted-character",
        "deleted-account",
    ] {
        let mut db = PgFixture::new(true, false);
        let quote = db.quote();
        db.session.set_hunting_repair_test_fault(2).unwrap();
        assert!(matches!(
            db.session
                .execute_hunting_repair(HuntingRepairAction::Confirm {
                    quote_id: quote.quote_id
                })
                .reply,
            HuntingRepairReply::Pending { .. }
        ));
        if lifecycle == "recreated" {
            db.client.execute("UPDATE characters SET hunting_incarnation=gen_random_uuid() WHERE account_id=$1 AND character_index=0",&[&db.account]).unwrap();
        } else if lifecycle == "deleted-account" {
            db.client
                .execute("DELETE FROM accounts WHERE account_id=$1", &[&db.account])
                .unwrap();
        } else {
            let mut account: AccountRecord = serde_json::from_value(
                db.client
                    .query_one(
                        "SELECT raw_json FROM accounts WHERE account_id=$1",
                        &[&db.account],
                    )
                    .unwrap()
                    .get(0),
            )
            .unwrap();
            if lifecycle == "banned" {
                account.is_banned = true;
                account.ban_reason = "moderation after uncertain repair".into();
                account.ban_until_ms = None;
                db.client.execute("UPDATE accounts SET is_banned=true,ban_until_ms=NULL,raw_json=$2,store_version=store_version+1 WHERE account_id=$1",&[&db.account,&serde_json::to_value(account).unwrap()]).unwrap();
            } else {
                account.characters.clear();
                account.saves.clear();
                let mut tx = db.client.transaction().unwrap();
                tx.execute("UPDATE accounts SET raw_json=$2,store_version=store_version+1 WHERE account_id=$1",&[&db.account,&serde_json::to_value(account).unwrap()]).unwrap();
                tx.execute("DELETE FROM characters WHERE account_id=$1", &[&db.account])
                    .unwrap();
                tx.commit().unwrap();
            }
        }
        assert!(
            db.session.prepare_hunting_repair_teardown().unwrap(),
            "{lifecycle}"
        );
        assert!(db.session.active_identity().is_none());
        assert!(db.session.active_character_checkpoint().is_none());
        assert!(db.session.prepare_hunting_repair_teardown().unwrap());
        assert!(db
            .session
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .ensure_account_store_writable()
            .is_ok());
        assert_eq!(db.burns(), 0);
        assert_eq!(db.balance(), None);
        let count: i64 = db
            .client
            .query_one(
                "SELECT count(*) FROM hunting_repair_results WHERE realm_id=$1",
                &[&db.realm],
            )
            .unwrap()
            .get(0);
        assert_eq!(
            count, 1,
            "the original immutable outcome must survive {lifecycle}"
        );
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_teardown_does_not_release_foreign_freeze_or_capture_old_source() {
    let mut db = PgFixture::new(false, true);
    let before = db.session.active_character_checkpoint().unwrap();
    let quote = db.quote();
    db.session.set_hunting_repair_test_fault(2).unwrap();
    assert!(matches!(
        db.session
            .execute_hunting_repair(HuntingRepairAction::Confirm {
                quote_id: quote.quote_id
            })
            .reply,
        HuntingRepairReply::Pending { .. }
    ));
    let config = &db
        .session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config;
    config.freeze_prepared_kill_source("unrelated publication requires recovery".into());
    assert!(db.session.prepare_hunting_repair_teardown().is_err());
    assert!(db.session.active_identity().is_some());
    assert_eq!(
        db.session.active_character_checkpoint().unwrap().revision,
        before.revision
    );
    assert_eq!(db.burns(), 1);
    assert_eq!(db.durable().revision, before.revision + 1);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_repeated_startgame_cannot_release_pending_world_fence() {
    let mut db = PgFixture::new(false, true);
    let before = db.session.active_character_checkpoint().unwrap();
    let quote = db.quote();
    let authority = db
        .session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .clone();
    db.session.set_hunting_repair_test_fault(2).unwrap();
    assert!(matches!(
        db.session
            .execute_hunting_repair(HuntingRepairAction::Confirm {
                quote_id: quote.quote_id.clone()
            })
            .reply,
        HuntingRepairReply::Pending { .. }
    ));
    assert!(authority.ensure_account_store_writable().is_err());
    assert!(db
        .session
        .reconcile_hunting_repair_login_authority()
        .is_ok());
    assert!(authority.ensure_account_store_writable().is_err());
    assert!(db
        .session
        .handle_packet(ClientPacket::StartGame { character_index: 0 })
        .is_empty());
    assert!(authority.ensure_account_store_writable().is_err());
    let retained = db.session.active_character_checkpoint().unwrap();
    assert_eq!(retained.revision, before.revision);
    assert_eq!(retained.equipment_items_json, before.equipment_items_json);
    assert!(matches!(
        db.session
            .local_repair_quote(&actor(db.session.app.world()).unwrap(), &quote.quote_id)
            .unwrap()
            .phase,
        Phase::Pending
    ));
    assert_eq!(db.durable().revision, before.revision + 1);
    assert_eq!(db.burns(), 1);
    assert!(!db.session.prepare_hunting_repair_teardown().unwrap());
    assert!(authority.ensure_account_store_writable().is_ok());
    assert_eq!(
        db.session.active_character_checkpoint().unwrap().revision,
        before.revision + 1
    );
    assert_eq!(db.burns(), 1);
}
