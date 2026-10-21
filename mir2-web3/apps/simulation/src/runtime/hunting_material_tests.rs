//! Main-server material acceptance with actual 2110001 NPC rewards and PG CAS.
//! Position/account setup and funding are explicit isolated fixtures: permits
//! come from an appended historical synthetic burn plus real settlement; fee
//! funds come from a synthetic new bound credit plus real coin conversion.
//! No clock, legacy gold, immutable history, GUI or physical gateway is changed.
use super::*;
use crate::config::{
    AccountRecord, AccountStore, AccountStoreDatabaseMode, AccountStoreRepository,
    CharacterSaveRecord, ItemContainer, PostgresAccountStoreRepository, QuestStage,
    SimulationConfig,
};
use crate::runtime::components::entity_by_object_id;
use crate::runtime::inventory::empty_slots_for_inventory_container;
use crate::runtime::items::hunting_item_binding::has_bound;
use crate::runtime::items::{crystal_item_index_for_item_state, ItemState};
use crate::runtime::quests;
use crate::runtime::resources::{NpcStateResource, PlayerRuntimeResource, QuestResource};
use mir2_economy_service::{
    self as economy, MaterialValuationEntry, MaterialValuationSet, WalletBalance,
};
use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGridType, Point};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

const KIT: i32 = 2_110_001;
const OLD_GOLD: u32 = 1_234;
const UNIT_GOLD: u64 = 125;
const FUND_GROSS: u64 = 100_000;

fn test_url() -> String {
    let url = std::env::var("MIR2_ECONOMY_TEST_DATABASE_URL")
        .expect("explicit isolated loopback PG fixture URL");
    assert!(url.starts_with("postgres://") || url.starts_with("postgresql://"));
    let parsed: postgres::Config = url.parse().unwrap();
    assert!(parsed
        .get_dbname()
        .is_some_and(|name| name.starts_with("mir2_hunting_acceptance_")));
    assert!(!parsed.get_hosts().is_empty() && parsed.get_hosts().iter().all(|host|
        matches!(host, postgres::config::Host::Tcp(name) if name.eq_ignore_ascii_case("localhost")
            || name.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback()))));
    assert!(parsed
        .get_hostaddrs()
        .iter()
        .all(std::net::IpAddr::is_loopback));
    url
}

fn started(config: SimulationConfig, account: &str) -> SimulationSession {
    let mut session = SimulationSession::new(config);
    {
        let mut quests = session.app.world_mut().resource_mut::<QuestResource>();
        quests.newcomer_v1_cadence = false;
        quests.newcomer_v2_cadence = true;
    }
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
            .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })),
        "{login:?}"
    );
    let start = session
        .try_handle_packet(ClientPacket::StartGame { character_index: 0 })
        .unwrap();
    assert!(
        start
            .iter()
            .any(|p| matches!(p, ServerPacket::StartGame { result: 4, .. })),
        "{start:?}"
    );
    assert!(session
        .active_zone_join_snapshot("material-runtime-fixture")
        .is_some());
    session
}

fn roots(session: &SimulationSession) -> Vec<ItemState> {
    let inventory = session.app.world().resource::<InventoryResource>();
    inventory
        .inventory_items
        .iter()
        .chain(&inventory.belt_items)
        .cloned()
        .collect()
}

fn inventory_image(session: &SimulationSession) -> Value {
    let inventory = session.app.world().resource::<InventoryResource>();
    serde_json::to_value((
        &inventory.inventory_items,
        &inventory.belt_items,
        &inventory.storage_items,
        &inventory.equipment_items,
        &inventory.reserved_item_unique_ids,
        inventory.inventory_capacity,
    ))
    .unwrap()
}

struct MaterialPgFixture {
    client: postgres::Client,
    session: SimulationSession,
    url: String,
    account: String,
    realm: String,
    owner: WalletOwner,
}

impl MaterialPgFixture {
    fn new(label: &str) -> Self {
        let base = test_url();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let schema = format!("material_runtime_{}_{}", std::process::id(), nonce);
        assert!(
            schema.len() <= 63
                && schema
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        );
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
        let mut seed = started(
            SimulationConfig::default().with_crystal_world_runtime(),
            "demo",
        );
        seed.force_authoritative_player_transform(Point { x: 285, y: 606 }, MirDirection::Up);
        seed.app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold = OLD_GOLD;
        let mut save = seed.active_character_checkpoint().unwrap();
        // The real Wizard kit includes both configured HP and MP rewards.
        save.character.class = MirClass::Wizard;
        let account = format!("material-runtime-{nonce}");
        let mut record = AccountRecord::new(save.character.clone());
        record.saves.insert(0, save.clone());
        let mut store = AccountStore::new(save.character.clone());
        store.accounts.clear();
        store.accounts.insert(account.clone(), record);
        PostgresAccountStoreRepository::new(&url, AccountStoreDatabaseMode::SourceOfTruth)
            .save(&store)
            .unwrap();
        let realm = format!("material-runtime-{label}-{nonce}");
        let policy = serde_json::from_value(json!({"schema_version":1,"policy_version":"runtime-material-v1","enabled":true,
            "k":{"numerator":1,"denominator":10},"conversion_fee":{"numerator":1,"denominator":50},
            "daily_decay":{"numerator":1,"denominator":150},"baseline_daily_mint_atoms":720000,
            "daily_mint_cap_multiplier":{"numerator":3,"denominator":2},"business_timezone":"Asia/Shanghai","settlement_hour":4})).unwrap();
        economy::initialize_realm(&mut client, &realm, &policy).unwrap();
        let config = SimulationConfig::default()
            .with_crystal_world_runtime()
            .with_postgres_account_store(&url)
            .unwrap()
            .with_hunting_economy_realm(&realm)
            .unwrap();
        let mut session = started(config, &account);
        assert!(
            entity_by_object_id(session.app.world(), 3).is_some(),
            "actual catalog Jane3"
        );
        assert!(crate::runtime::npc::crystal_npc_object_in_data_range(
            session.app.world(),
            3
        ));
        session.handle_packet(ClientPacket::CallNpc {
            object_id: 3,
            key: "@Main".into(),
        });
        assert_eq!(
            session
                .app
                .world()
                .resource::<NpcStateResource>()
                .active_npc_dialog
                .as_ref()
                .map(|d| d.npc_object_id),
            Some(3)
        );
        let accepted = session.handle_packet(ClientPacket::AcceptQuest {
            npc_index: 3,
            quest_index: KIT,
        });
        assert!(
            accepted.iter().any(|p| matches!(
                p,
                ServerPacket::ChangeQuest {
                    quest_id: KIT,
                    taken: true,
                    ..
                }
            )),
            "{accepted:?}"
        );
        assert_eq!(
            quests::quest_stage(session.app.world(), KIT),
            Some(QuestStage::ReadyToTurnIn)
        );
        // Accept closes the ordinary NPC dialog. Reopen it as a player before
        // FinishQuest; a remembered NPC object is not current service authority.
        session.handle_packet(ClientPacket::CallNpc {
            object_id: 3,
            key: "@Main".into(),
        });
        assert_eq!(
            session
                .app
                .world()
                .resource::<NpcStateResource>()
                .active_npc_dialog
                .as_ref()
                .map(|dialog| dialog.npc_object_id),
            Some(3)
        );
        let finished = session.handle_packet(ClientPacket::FinishQuest {
            quest_index: KIT,
            selected_item_index: -1,
        });
        assert!(
            finished
                .iter()
                .any(|p| matches!(p, ServerPacket::CompleteQuest { .. })),
            "{finished:?}"
        );
        assert_eq!(
            quests::quest_stage(session.app.world(), KIT),
            Some(QuestStage::Completed)
        );
        let actual = actor(session.app.world()).unwrap();
        let owner = WalletOwner {
            account_id: actual.account,
            character_index: actual.index,
            incarnation: actual.incarnation,
        };
        let mut f = Self {
            client,
            session,
            url,
            account,
            realm,
            owner,
        };
        f.move_potions_to_bag_and_save();
        f.seed_permits_and_fee_funds();
        f.register_prices("runtime-material-prices-v1", false);
        assert_eq!(
            f.session.active_character_checkpoint().unwrap().gold,
            OLD_GOLD
        );
        f
    }

    fn move_potions_to_bag_and_save(&mut self) {
        let potions: Vec<_> = roots(&self.session)
            .into_iter()
            .filter(|item| {
                has_bound(item)
                    && item.equip_slot.is_none()
                    && mir2_game_data::crystal_item_by_index(crystal_item_index_for_item_state(
                        item,
                    ))
                    .is_some_and(|template| template.item_type == 13)
            })
            .collect();
        assert!(
            potions.len() >= 2,
            "actual kit gives distinct bound HP/MP roots"
        );
        for item in potions {
            if item.container != ItemContainer::Belt {
                continue;
            }
            let inventory = self.session.app.world().resource::<InventoryResource>();
            let (container, slot) = empty_slots_for_inventory_container(
                &inventory.inventory_items,
                ItemContainer::Bag1,
                inventory.inventory_capacity,
            )
            .into_iter()
            .next()
            .unwrap();
            let logical = if container == ItemContainer::Bag1 {
                slot
            } else {
                40 + slot
            };
            let packets = self.session.handle_packet(ClientPacket::MoveItem {
                grid: MirGridType::Belt,
                from: i32::from(item.slot),
                to: i32::from(logical) + i32::from(crate::config::CRYSTAL_BELT_SLOT_COUNT),
            });
            assert!(
                packets
                    .iter()
                    .any(|p| matches!(p, ServerPacket::MoveItem { success: true, .. })),
                "{packets:?}"
            );
            assert!(self
                .session
                .app
                .world()
                .resource::<InventoryResource>()
                .inventory_items
                .iter()
                .any(|root| root.unique_id == item.unique_id
                    && root.container == container
                    && root.slot == slot));
        }
        self.session.save_active_character().unwrap();
        assert_eq!(
            self.durable().inventory_items_json,
            self.session
                .active_character_checkpoint()
                .unwrap()
                .inventory_items_json
        );
    }

    fn seed_permits_and_fee_funds(&mut self) {
        let day = economy::read_realm(&mut self.client, &self.realm)
            .unwrap()
            .genesis_day
            - 1;
        self.client.execute("UPDATE hunting_economy_pools SET genesis_day=$2::bigint,settled_through_day=$2::bigint-1 WHERE realm_id=$1",
            &[&self.realm,&day]).unwrap();
        let mut tx = self.client.transaction().unwrap();
        let clock = economy::lock_realm_and_read_database_clock(&mut tx, &self.realm).unwrap();
        let synthetic = json!({"classification":"synthetic-material-runtime-permit-funding"});
        tx.execute("INSERT INTO hunting_economy_operations(realm_id,operation_key,operation_kind,request_digest,
            receipt_json,receipt_digest,business_day,committed_at_ms) VALUES($1,'synthetic-permit-burn','burn','synthetic',$2,'synthetic',$3,$4)",
            &[&self.realm,&synthetic,&day,&clock.database_time_ms]).unwrap();
        tx.execute("INSERT INTO hunting_economy_burn_events(realm_id,event_id,operation_key,layer,source_kind,source_key,
            gold_equivalent,valuation_version,policy_version,business_day,committed_at_ms)
            VALUES($1,'synthetic-permit-burn','synthetic-permit-burn','siege','synthetic-material-fixture','synthetic-permit-burn',
            100000000,'synthetic-fixture-v1','runtime-material-v1',$2,$3)",&[&self.realm,&day,&clock.database_time_ms]).unwrap();
        tx.commit().unwrap();
        assert_eq!(
            economy::settle_due(&mut self.client, &self.realm, 31)
                .unwrap()
                .len(),
            1
        );
        economy::credit_bound_authorized_with_source(
            &mut self.client,
            &self.realm,
            "explicit-material-runtime-fee-funding",
            "synthetic-test-adapter",
            FUND_GROSS,
            |_, _| Ok(self.owner.clone()),
            |_, _| Ok(()),
        )
        .unwrap();
        let funding = economy::convert(
            &mut self.client,
            &self.realm,
            &self.owner,
            "synthetic-material-fee-funding-conversion",
            FUND_GROSS,
        )
        .unwrap();
        assert_eq!(funding.receipt.quote.fee_unbound_gold, 2000);
        assert_eq!(
            self.wallet().bound_gold,
            100,
            "real kit bound credit remains untouched"
        );
        assert_eq!(self.wallet().unbound_gold, 98000);
    }

    fn register_prices(&mut self, version: &str, empty: bool) {
        let entries = if empty {
            vec![]
        } else {
            ["(HP)DrugSmall", "(MP)DrugSmall"]
                .into_iter()
                .map(|name| MaterialValuationEntry {
                    item_index: mir2_game_data::crystal_item_by_name(name)
                        .unwrap()
                        .item_index,
                    unit_gold: UNIT_GOLD,
                })
                .collect()
        };
        let set = MaterialValuationSet::new(version, entries).unwrap();
        economy::register_material_valuation(&mut self.client, &self.realm, &set).unwrap();
        economy::activate_material_valuation(
            &mut self.client,
            &self.realm,
            set.valuation_version(),
        )
        .unwrap();
    }

    fn potion(&self, name: &str) -> ItemState {
        let index = mir2_game_data::crystal_item_by_name(name)
            .unwrap()
            .item_index;
        self.session
            .app
            .world()
            .resource::<InventoryResource>()
            .inventory_items
            .iter()
            .find(|item| has_bound(item) && crystal_item_index_for_item_state(item) == index)
            .unwrap()
            .clone()
    }
    fn quote(&mut self, item: &ItemState, quantity: u64) -> HuntingMaterialQuoteView {
        match self
            .session
            .execute_hunting_material(HuntingMaterialAction::Quote {
                unique_id: item.unique_id,
                quantity,
            })
            .reply
        {
            HuntingMaterialReply::Quoted { quote } => quote,
            other => panic!("material quote: {other:?}"),
        }
    }
    fn confirm(&mut self, id: &str) -> HuntingMaterialExecution {
        self.session
            .execute_hunting_material(HuntingMaterialAction::Confirm {
                quote_id: id.into(),
            })
    }
    fn query(&mut self, id: &str) -> HuntingMaterialExecution {
        self.session
            .execute_hunting_material(HuntingMaterialAction::Query {
                quote_id: id.into(),
            })
    }
    fn durable(&mut self) -> CharacterSaveRecord {
        serde_json::from_value(
            self.client
                .query_one(
                    "SELECT snapshot_json FROM character_saves
            WHERE account_id=$1 AND character_index=0",
                    &[&self.account],
                )
                .unwrap()
                .get(0),
        )
        .unwrap()
    }
    fn wallet(&mut self) -> WalletBalance {
        economy::read_wallet(&mut self.client, &self.realm, &self.owner)
            .unwrap()
            .unwrap()
    }
    fn counts(&mut self) -> (i64, i64, i64) {
        let row=self.client.query_one("SELECT
            (SELECT count(*)::bigint FROM hunting_economy_material_conversions WHERE realm_id=$1),
            (SELECT count(*)::bigint FROM hunting_economy_burn_events WHERE realm_id=$1 AND source_kind='material_conversion_fee'),
            (SELECT count(*)::bigint FROM hunting_material_conversion_results WHERE realm_id=$1)",&[&self.realm]).unwrap();
        (row.get(0), row.get(1), row.get(2))
    }
    fn financial(&mut self) -> Value {
        let wallet = self.wallet();
        let pool = economy::read_realm(&mut self.client, &self.realm).unwrap();
        let counts = self.counts();
        json!([wallet, pool, counts])
    }
    fn config(&self) -> SimulationConfig {
        self.session
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone()
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

fn committed(execution: HuntingMaterialExecution) -> HuntingMaterialReceiptView {
    assert!(
        execution
            .packets
            .iter()
            .all(|p| !matches!(p, ServerPacket::LoseGold { .. })),
        "legacy gold is not a material fee wallet"
    );
    match execution.reply {
        HuntingMaterialReply::Committed {
            receipt,
            reload_required: false,
            ..
        } => receipt,
        other => panic!("expected locally published material commit: {other:?}"),
    }
}

fn assert_conversion(before: &ItemState, after: &ItemState, receipt: &HuntingMaterialReceiptView) {
    let mut expected = serde_json::to_value(before).unwrap();
    expected.as_object_mut().unwrap().remove("economic_binding");
    let provenance = serde_json::to_value(after.economic_conversion.as_ref().unwrap()).unwrap();
    assert_eq!(provenance["operationId"], receipt.operation_key);
    assert_eq!(provenance["quoteId"], receipt.quote_id);
    assert_eq!(provenance["inputUniqueId"], json!(before.unique_id));
    expected["economic_conversion"] = provenance;
    for field in ["container", "slot", "unique_id", "quantity"] {
        expected[field] = serde_json::to_value(after).unwrap()[field].clone();
    }
    assert_eq!(serde_json::to_value(after).unwrap(), expected);
    assert!(after.economic_binding.is_none());
    assert_eq!(u64::from(after.quantity), receipt.quantity);
    assert_eq!(after.unique_id, receipt.output_unique_id);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_material_postgres_actual_kit_whole_partial_cold_login_and_original_query() {
    for (name, whole) in [("(HP)DrugSmall", true), ("(MP)DrugSmall", false)] {
        let mut db = MaterialPgFixture::new(if whole { "whole" } else { "partial" });
        let source = db.potion(name);
        assert!(source.quantity > 1);
        let quantity = if whole { u64::from(source.quantity) } else { 1 };
        let before = db.durable();
        let wallet = db.wallet();
        let pool = economy::read_realm(&mut db.client, &db.realm)
            .unwrap()
            .pool
            .stock_atoms;
        let quote = db.quote(&source, quantity);
        assert_eq!(quote.item_index, crystal_item_index_for_item_state(&source));
        assert_eq!(quote.gold_equivalent, UNIT_GOLD * quantity);
        assert_eq!(quote.fee_unbound_gold, (UNIT_GOLD * quantity).div_ceil(50));
        let receipt = committed(db.confirm(&quote.quote_id));
        assert_eq!(db.counts(), (1, 1, 1));
        assert_eq!(db.wallet().bound_gold, wallet.bound_gold);
        assert_eq!(
            db.wallet().unbound_gold,
            wallet.unbound_gold - quote.fee_unbound_gold
        );
        assert_eq!(
            economy::read_realm(&mut db.client, &db.realm)
                .unwrap()
                .pool
                .stock_atoms,
            pool - quote.license_atoms_debit
        );
        let durable = db.durable();
        let live = db.session.active_character_checkpoint().unwrap();
        assert_eq!(durable.revision, before.revision + 1);
        assert_eq!(live.revision, durable.revision);
        assert_eq!(live.inventory_items_json, durable.inventory_items_json);
        assert_eq!(live.belt_items_json, before.belt_items_json);
        assert_eq!(live.storage_items_json, before.storage_items_json);
        assert_eq!(live.equipment_items_json, before.equipment_items_json);
        assert_eq!(
            (live.hp, live.mp, live.max_hp),
            (before.hp, before.mp, before.max_hp)
        );
        assert_eq!((live.gold, durable.gold), (before.gold, OLD_GOLD));
        let output = roots(&db.session)
            .into_iter()
            .find(|i| i.unique_id == receipt.output_unique_id)
            .unwrap();
        assert_conversion(&source, &output, &receipt);
        let mut expected_items: Vec<ItemState> = before
            .inventory_items_json
            .iter()
            .map(|item| serde_json::from_str(item).unwrap())
            .collect();
        let original_index = expected_items
            .iter()
            .position(|item| item.unique_id == source.unique_id)
            .unwrap();
        if whole {
            expected_items[original_index] = output.clone();
        } else {
            expected_items[original_index].quantity -= 1;
            expected_items.push(output.clone());
        }
        assert_eq!(
            serde_json::to_value(&expected_items).unwrap(),
            serde_json::to_value(
                &db.session
                    .app
                    .world()
                    .resource::<InventoryResource>()
                    .inventory_items
            )
            .unwrap()
        );
        if whole {
            assert_eq!(output.unique_id, source.unique_id);
        } else {
            assert_ne!(output.unique_id, 0);
            assert_ne!(output.unique_id, source.unique_id);
            let mut expected = source.clone();
            expected.quantity -= 1;
            let remainder = roots(&db.session)
                .into_iter()
                .find(|i| i.unique_id == source.unique_id)
                .unwrap();
            assert_eq!(
                serde_json::to_value(remainder).unwrap(),
                serde_json::to_value(expected).unwrap()
            );
        }
        let fee=db.client.query_one("SELECT gold_equivalent,valuation_version,source_key,operation_key FROM hunting_economy_burn_events
            WHERE realm_id=$1 AND source_kind='material_conversion_fee'",&[&db.realm]).unwrap();
        assert_eq!(
            fee.get::<_, i64>("gold_equivalent"),
            quote.fee_unbound_gold as i64
        );
        assert_eq!(
            fee.get::<_, String>("valuation_version"),
            "gold-face-value-v1"
        );
        assert_eq!(fee.get::<_, String>("source_key"), receipt.operation_key);
        assert_eq!(fee.get::<_, String>("operation_key"), receipt.operation_key);
        let result=db.client.query_one("SELECT receipt_json,receipt_json::text AS bytes,receipt_digest,before_revision,after_revision
            FROM hunting_material_conversion_results WHERE realm_id=$1 AND quote_id=$2",&[&db.realm,&quote.quote_id]).unwrap();
        let immutable_bytes: String = result.get("bytes");
        let immutable_digest: String = result.get("receipt_digest");
        let full_receipt: economy::MaterialConversionReceipt =
            serde_json::from_value(result.get("receipt_json")).unwrap();
        assert_eq!(
            full_receipt.source.checkpoint,
            serde_json::to_value(&durable).unwrap()
        );
        assert_eq!(full_receipt.wallet_after, db.wallet());
        assert_eq!(full_receipt.source.input_unique_id, source.unique_id);
        assert_eq!(
            full_receipt.source.output_unique_id,
            receipt.output_unique_id
        );
        assert_eq!(
            result.get::<_, i64>("before_revision"),
            before.revision as i64
        );
        assert_eq!(
            result.get::<_, i64>("after_revision"),
            durable.revision as i64
        );
        let financial = db.financial();
        let logout = db.session.try_handle_packet(ClientPacket::LogOut).unwrap();
        assert!(db.session.active_identity().is_none(), "{logout:?}");
        db.session = started(db.fresh_config(), &db.account);
        let cold_inventory = inventory_image(&db.session);
        let loaded = roots(&db.session)
            .into_iter()
            .find(|i| i.unique_id == receipt.output_unique_id)
            .unwrap();
        assert_conversion(&source, &loaded, &receipt);
        let replay = db.query(&quote.quote_id);
        assert!(matches!(
            replay.reply,
            HuntingMaterialReply::Committed {
                replayed: true,
                reload_required: false,
                ..
            }
        ));
        assert!(replay.packets.is_empty());
        assert_eq!(inventory_image(&db.session), cold_inventory);
        assert_eq!(db.financial(), financial);
        let replayed_result=db.client.query_one("SELECT receipt_json::text AS bytes,receipt_digest FROM hunting_material_conversion_results
            WHERE realm_id=$1 AND quote_id=$2",&[&db.realm,&quote.quote_id]).unwrap();
        assert_eq!(replayed_result.get::<_, String>("bytes"), immutable_bytes);
        assert_eq!(
            replayed_result.get::<_, String>("receipt_digest"),
            immutable_digest
        );
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_material_postgres_mutation_revision_price_funds_and_quota_reject_without_debit() {
    let mut db = MaterialPgFixture::new("rejections");
    let source = db.potion("(HP)DrugSmall");
    for mutation in 0..3 {
        let original = db
            .session
            .app
            .world()
            .resource::<InventoryResource>()
            .clone();
        let quote = db.quote(&source, 1);
        let financial = db.financial();
        let durable = db.durable();
        match mutation {
            0 => {
                *db.session
                    .app
                    .world_mut()
                    .resource_mut::<InventoryResource>() = original.clone()
            }
            1 => db
                .session
                .app
                .world_mut()
                .resource_mut::<InventoryResource>()
                .inventory_items
                .iter_mut()
                .find(|i| i.unique_id == source.unique_id)
                .unwrap()
                .description
                .push_str(" changed after quote"),
            2 => db.session.save_active_character().unwrap(),
            _ => unreachable!(),
        }
        let changed_inventory = inventory_image(&db.session);
        let result = db.confirm(&quote.quote_id);
        assert!(
            matches!(
                result.reply,
                HuntingMaterialReply::Rejected {
                    code: HuntingMaterialRejection::ItemChanged,
                    ..
                }
            ),
            "{:?}",
            result.reply
        );
        assert_eq!(inventory_image(&db.session), changed_inventory);
        assert_eq!(db.financial(), financial);
        if mutation == 2 {
            assert!(db.durable().revision > durable.revision);
        } else {
            assert_eq!(
                serde_json::to_value(db.durable()).unwrap(),
                serde_json::to_value(durable).unwrap()
            );
        }
        *db.session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>() = original;
    }
    db.register_prices("runtime-material-empty-v2", true);
    let financial = db.financial();
    let durable = db.durable();
    let unpriced = db
        .session
        .execute_hunting_material(HuntingMaterialAction::Quote {
            unique_id: source.unique_id,
            quantity: 1,
        });
    assert!(matches!(
        unpriced.reply,
        HuntingMaterialReply::Rejected {
            code: HuntingMaterialRejection::Unconfigured,
            ..
        }
    ));
    assert_eq!(db.financial(), financial);
    assert_eq!(db.durable().revision, durable.revision);
    db.register_prices("runtime-material-priced-v3", false);
    let quote = db.quote(&source, 1);
    let funded = db.wallet().unbound_gold;
    db.client
        .execute(
            "UPDATE hunting_economy_wallets SET unbound_gold=0 WHERE realm_id=$1",
            &[&db.realm],
        )
        .unwrap();
    let empty_funds = db.financial();
    assert!(matches!(
        db.confirm(&quote.quote_id).reply,
        HuntingMaterialReply::Rejected {
            code: HuntingMaterialRejection::InsufficientUnboundGold,
            ..
        }
    ));
    assert_eq!(db.financial(), empty_funds);
    db.client
        .execute(
            "UPDATE hunting_economy_wallets SET unbound_gold=$2 WHERE realm_id=$1",
            &[&db.realm, &(funded as i64)],
        )
        .unwrap();
    let quote = db.quote(&source, 1);
    db.client
        .execute(
            "UPDATE hunting_economy_pools SET stock_atoms=0 WHERE realm_id=$1",
            &[&db.realm],
        )
        .unwrap();
    let empty_pool = db.financial();
    assert!(matches!(
        db.confirm(&quote.quote_id).reply,
        HuntingMaterialReply::Rejected {
            code: HuntingMaterialRejection::InsufficientPermits,
            ..
        }
    ));
    assert_eq!(db.financial(), empty_pool);
    assert_eq!(db.counts(), (0, 0, 0));
    assert_eq!(db.durable().gold, OLD_GOLD);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_material_postgres_before_commit_rollback_and_known_absent_never_auto_confirm() {
    for fault in [1, 4] {
        let mut db = MaterialPgFixture::new(if fault == 1 { "rollback" } else { "absent" });
        let source = db.potion("(HP)DrugSmall");
        let quote = db.quote(&source, 1);
        let before = db.durable();
        let live = inventory_image(&db.session);
        let financial = db.financial();
        db.session.set_hunting_material_test_fault(fault).unwrap();
        let failed = db.confirm(&quote.quote_id);
        if fault == 1 {
            assert!(matches!(
                failed.reply,
                HuntingMaterialReply::Rejected { .. }
            ));
        } else {
            assert!(matches!(failed.reply, HuntingMaterialReply::Pending { .. }));
            assert!(db.config().ensure_account_store_writable().is_err());
        }
        assert_eq!(db.financial(), financial);
        assert_eq!(inventory_image(&db.session), live);
        assert_eq!(
            serde_json::to_value(db.durable()).unwrap(),
            serde_json::to_value(&before).unwrap()
        );
        let query = db.query(&quote.quote_id);
        assert!(matches!(
            query.reply,
            HuntingMaterialReply::Rejected {
                code: HuntingMaterialRejection::NotSubmitted,
                ..
            }
        ));
        assert!(query.packets.is_empty());
        assert_eq!(db.financial(), financial);
        assert_eq!(db.counts(), (0, 0, 0));
        db.config().ensure_account_store_writable().unwrap();
        assert!(matches!(
            db.confirm(&quote.quote_id).reply,
            HuntingMaterialReply::Rejected {
                code: HuntingMaterialRejection::NotSubmitted,
                ..
            }
        ));
        let fresh = db.quote(&source, 1);
        assert_ne!(fresh.quote_id, quote.quote_id);
        committed(db.confirm(&fresh.quote_id));
        assert_eq!(db.counts(), (1, 1, 1));
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_material_postgres_ack_lost_original_query_publishes_once() {
    for fault in [2, 5] {
        let mut db = MaterialPgFixture::new(if fault == 2 { "ack" } else { "ack-proof" });
        let source = db.potion("(HP)DrugSmall");
        let quote = db.quote(&source, 1);
        let before = db.durable();
        let live = inventory_image(&db.session);
        let wallet = db.wallet();
        db.session.set_hunting_material_test_fault(fault).unwrap();
        assert!(matches!(
            db.confirm(&quote.quote_id).reply,
            HuntingMaterialReply::Pending { .. }
        ));
        assert_eq!(db.counts(), (1, 1, 1));
        assert_eq!(db.durable().revision, before.revision + 1);
        assert_eq!(inventory_image(&db.session), live);
        assert!(db.config().ensure_account_store_writable().is_err());
        let first = committed(db.query(&quote.quote_id));
        assert_eq!(first.quote_id, quote.quote_id);
        assert_eq!(
            db.wallet().unbound_gold,
            wallet.unbound_gold - quote.fee_unbound_gold
        );
        assert_eq!(
            db.session
                .active_character_checkpoint()
                .unwrap()
                .inventory_items_json,
            db.durable().inventory_items_json
        );
        let published = inventory_image(&db.session);
        let financial = db.financial();
        let repeated = db.query(&quote.quote_id);
        assert!(matches!(
            repeated.reply,
            HuntingMaterialReply::Committed {
                replayed: true,
                reload_required: false,
                ..
            }
        ));
        assert!(repeated.packets.is_empty());
        assert_eq!(inventory_image(&db.session), published);
        assert_eq!(db.financial(), financial);
        db.config().ensure_account_store_writable().unwrap();
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_material_postgres_unsafe_commit_refences_retires_and_cold_loads_without_overwrite() {
    for changed_live in [false, true] {
        let mut db = MaterialPgFixture::new(if changed_live {
            "unsafe-changed"
        } else {
            "unsafe-known"
        });
        let source = db.potion("(HP)DrugSmall");
        let quote = db.quote(&source, 1);
        let authority = db.config();
        let original = inventory_image(&db.session);
        db.session
            .set_hunting_material_test_fault(if changed_live { 2 } else { 3 })
            .unwrap();
        let mut execution = db.confirm(&quote.quote_id);
        let expected_live = if changed_live {
            assert!(matches!(
                execution.reply,
                HuntingMaterialReply::Pending { .. }
            ));
            // Explicit adversarial World mutation while ordinary writes are
            // fenced; no claim that a normal client may bypass that fence.
            db.session
                .app
                .world_mut()
                .resource_mut::<InventoryResource>()
                .inventory_items
                .iter_mut()
                .find(|item| item.unique_id == source.unique_id)
                .unwrap()
                .description
                .push_str(" retained adversarial World");
            let changed = inventory_image(&db.session);
            execution = db.query(&quote.quote_id);
            changed
        } else {
            original
        };
        assert!(
            matches!(
                execution.reply,
                HuntingMaterialReply::Committed {
                    reload_required: true,
                    ..
                }
            ),
            "{:?}",
            execution.reply
        );
        assert!(execution
            .packets
            .iter()
            .any(|p| matches!(p, ServerPacket::LogOutSuccess { .. })));
        assert!(db.session.active_identity().is_none());
        assert_eq!(inventory_image(&db.session), expected_live);
        assert!(authority.ensure_account_store_writable().is_err());
        assert_eq!(db.counts(), (1, 1, 1));
        let canonical = db.durable();
        let financial = db.financial();
        // Reuse the original private source seal for ordinary Login recovery.
        db.session = started(authority.clone(), &db.account);
        authority.ensure_account_store_writable().unwrap();
        assert_eq!(
            db.session
                .active_character_checkpoint()
                .unwrap()
                .inventory_items_json,
            canonical.inventory_items_json
        );
        let replay = db.query(&quote.quote_id);
        assert!(matches!(
            replay.reply,
            HuntingMaterialReply::Committed {
                replayed: true,
                reload_required: false,
                ..
            }
        ));
        assert!(replay.packets.is_empty());
        assert_eq!(db.financial(), financial);
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_material_postgres_normal_logout_reconciles_pending_original_before_save() {
    for fault in [2, 4] {
        let mut db = MaterialPgFixture::new(if fault == 2 {
            "teardown-committed"
        } else {
            "teardown-absent"
        });
        let source = db.potion("(HP)DrugSmall");
        let quote = db.quote(&source, 1);
        let wallet = db.wallet();
        db.session.set_hunting_material_test_fault(fault).unwrap();
        assert!(matches!(
            db.confirm(&quote.quote_id).reply,
            HuntingMaterialReply::Pending { .. }
        ));
        db.session
            .force_authoritative_player_vitals(Some(3), Some(1));
        let logout = db.session.try_handle_packet(ClientPacket::LogOut).unwrap();
        assert!(
            logout
                .iter()
                .any(|p| matches!(p, ServerPacket::LogOutSuccess { .. })),
            "{logout:?}"
        );
        assert!(db.session.active_identity().is_none());
        db.config().ensure_account_store_writable().unwrap();
        let saved = db.durable();
        assert_eq!((saved.hp, saved.mp), (3, 1));
        assert_eq!(saved.gold, OLD_GOLD);
        assert_eq!(db.counts(), if fault == 2 { (1, 1, 1) } else { (0, 0, 0) });
        assert_eq!(
            db.wallet().unbound_gold,
            if fault == 2 {
                wallet.unbound_gold - quote.fee_unbound_gold
            } else {
                wallet.unbound_gold
            }
        );
        db.session = started(db.fresh_config(), &db.account);
        assert_eq!(
            db.session
                .active_character_checkpoint()
                .unwrap()
                .inventory_items_json,
            saved.inventory_items_json
        );
        let query = db.query(&quote.quote_id);
        if fault == 2 {
            assert!(matches!(
                query.reply,
                HuntingMaterialReply::Committed {
                    replayed: true,
                    reload_required: false,
                    ..
                }
            ));
        } else {
            assert!(matches!(
                query.reply,
                HuntingMaterialReply::Rejected {
                    code: HuntingMaterialRejection::NotSubmitted,
                    ..
                }
            ));
        }
        assert!(query.packets.is_empty());
    }
}
