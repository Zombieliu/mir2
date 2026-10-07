//! Actual Session/File-source tests. Root is the sole runner. Every fixture
//! owns a fresh directory under the runner's guarded TEMP and retains evidence;
//! no environment mutation, network, external account file or recursive cleanup.
use super::*;
use super::super::inventory::future_binary_datetime_minutes;
use super::super::items::ItemState;
use crate::config::ItemContainer;
use super::super::npc::{NpcBuyBackItemState, NpcBuyBackState, NpcUsedGoodsState};
use super::super::resources::{InventoryResource, NpcStateResource, PlayerRuntimeResource, Stage5SystemsResource};
use crate::config::{deliver_stage5_system_mail, AccountStoreTransactionFault,
    Stage5MailDelivery, Stage5MailTargetKind};
use crate::npc_purchase_journal::NPC_PURCHASE_JOURNAL_ENTRY_CAP;
use crate::{SimulationConfig, VisibleNpcRecord};
use mir2_protocol::{ClientPacket, MirDirection, Point, UserItem};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const SCRIPT: &str = "BichonProvince/NaturalCave/WickedTrader";
static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    config: SimulationConfig,
    path: PathBuf,
    session: SimulationSession,
    request: NpcPurchaseRequest,
}

fn fresh_path(label: &str) -> PathBuf {
    let temporary = std::env::temp_dir().canonicalize().expect("runner TEMP must exist");
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let directory = temporary.join(format!("mir2-source27-npc-purchase-{label}-{timestamp}-{sequence}"));
    assert!(directory.starts_with(&temporary));
    fs::create_dir(&directory).expect("unique fixture directory must not overwrite an existing path");
    directory.join("accounts.json")
}

fn login_start(config: SimulationConfig, index: i32) -> SimulationSession {
    assert!(config.account_store_path.is_some(), "durable fixture requires File source");
    assert!(config.account_store_database_url.is_none(), "fixture must never contact a database");
    let mut session = SimulationSession::new(config);
    assert!(session.handle_packet(ClientPacket::Login { account_id: "demo".into(), password: "demo".into() })
        .iter().any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    session.handle_packet(ClientPacket::StartGame { character_index: index });
    assert_eq!(session.active_identity().unwrap().character_index, index);
    session
}

fn source_item(uid: u64, count: u16) -> UserItem {
    let template = mir2_game_data::crystal_item_by_index(658).unwrap();
    UserItem { unique_id: uid, item_index: 658, current_dura: template.durability,
        max_dura: template.durability, count, soul_bound_id: -1, identified: false, cursed: false,
        slots: vec![None; usize::from(template.slots)], gem_count: 0, added_stats: vec![], awake_type: 0,
        awake_values: vec![], refined_value: 0, refine_added: 0, refine_success_chance: 0, wedding_ring: -1,
        expire_info: None, rental_information: None, is_shop_item: false, sealed_info: None, gm_made: false }
}

impl Fixture {
    fn new(label: &str, service: &str, source: NpcPurchaseSource) -> Self {
        let path = fresh_path(label);
        let mut config = SimulationConfig::default().with_account_store_path(&path);
        config.visible_npcs.push(VisibleNpcRecord { object_id: 4990, name: "Wicked Trader".into(), image: 5,
            colour_argb: -1, position: Point { x: 331, y: 271 }, direction: MirDirection::Left,
            quest_ids: vec![], script_key: Some(SCRIPT.into()) });
        let mut session = login_start(config.clone(), 0);
        session.interact(4990);
        let packets = session.select_npc_dialog_target("@BuySell");
        let trade_uid = packets.iter().find_map(|packet| match packet {
            ServerPacket::NPCGoods { list, .. } => list.iter().find(|item| item.item_index == 658 && item.count == 1)
                .map(|item| item.unique_id),
            _ => None,
        }).expect("actual WickedTrader goods selector");
        {
            let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
            inventory.inventory_capacity = 86;
            inventory.inventory_items.clear(); inventory.belt_items.clear(); inventory.equipment_items.clear();
            inventory.storage_items.clear(); inventory.reserved_item_unique_ids.clear();
        }
        session.app.world_mut().resource_mut::<PlayerRuntimeResource>().gold = 100_000;
        session.app.world_mut().resource_mut::<Stage5SystemsResource>().stage5_systems.intelligent_creature_pearls = 1_000;
        let player_name = session.active_identity().unwrap().character_name;
        let uid = if source == NpcPurchaseSource::Trade { trade_uid } else { 444_000 };
        {
            let mut npc = session.app.world_mut().resource_mut::<NpcStateResource>();
            npc.active_npc_service.as_mut().unwrap().label_key = service.into();
            match source {
                NpcPurchaseSource::Trade => {}
                NpcPurchaseSource::BuyBack => npc.npc_buy_back_items.push(NpcBuyBackState {
                    script_key: SCRIPT.into(), player_name,
                    items: vec![NpcBuyBackItemState { item: source_item(uid, 2),
                        expires_at_binary_datetime: future_binary_datetime_minutes(60) }],
                }),
                NpcPurchaseSource::Used => npc.npc_used_goods_items.push(NpcUsedGoodsState {
                    script_key: SCRIPT.into(), items: vec![source_item(uid, 2)],
                }),
            }
        }
        // Commit the actual baseline wallet, inventory and resale carrier before
        // enabling an actor, so transaction CAS compares a real File checkpoint.
        session.save_active_character().unwrap();
        let fixture = Self { config, path, session,
            request: NpcPurchaseRequest { item_index: uid, count: 2, panel_type: 0 } };
        fixture.assert_checkpoint();
        fixture
    }

    fn operation(&mut self, sequence: u64) -> NpcPurchaseOperation {
        let producer = self.session.begin_npc_purchase_producer().unwrap();
        let intent = self.session.npc_purchase_intent(self.request).unwrap();
        assert!(producer.actor.iter().any(|byte| *byte != 0));
        assert!(producer.producer_scope.iter().any(|byte| *byte != 0));
        assert!(intent.service_catalog_proof.iter().any(|byte| *byte != 0));
        NpcPurchaseOperation { actor: producer.actor, request_scope: producer.producer_scope, sequence, intent }
    }

    fn store_from_file(&self) -> AccountStore {
        serde_json::from_slice(&fs::read(&self.path).expect("owned File checkpoint must exist")).unwrap()
    }

    fn save_from_file(&self) -> CharacterSaveRecord {
        let index = self.session.active_identity().unwrap().character_index;
        self.store_from_file().accounts["demo"].saves[&index].clone()
    }

    fn live(&self) -> serde_json::Value {
        let inventory = self.session.app.world().resource::<InventoryResource>();
        let npc = self.session.app.world().resource::<NpcStateResource>();
        let session = self.session.app.world().resource::<SessionResource>();
        serde_json::json!({
            "gold": self.session.app.world().resource::<PlayerRuntimeResource>().gold,
            "experience": self.session.app.world().resource::<PlayerRuntimeResource>().experience,
            "maxExperience": self.session.app.world().resource::<PlayerRuntimeResource>().max_experience,
            "pearls": self.session.app.world().resource::<Stage5SystemsResource>().stage5_systems.intelligent_creature_pearls,
            "stage5": self.session.app.world().resource::<Stage5SystemsResource>().stage5_systems,
            "character": session.selected_character,
            "inventory": inventory.inventory_items, "belt": inventory.belt_items,
            "equipment": inventory.equipment_items, "storage": inventory.storage_items,
            "capacity": inventory.inventory_capacity, "reserved": inventory.reserved_item_unique_ids,
            "buyBack": npc.npc_buy_back_items, "used": npc.npc_used_goods_items,
            "revision": session.active_save_revision(), "journal": session.npc_purchase_journal,
        })
    }

    fn assert_checkpoint(&self) {
        let durable = self.save_from_file();
        let active = snapshot_active_character_save(self.session.app.world()).unwrap();
        assert_eq!(economic_checkpoint(&active), economic_checkpoint(&durable));
        assert_eq!(active.revision, durable.revision);
        assert_eq!(active.npc_purchase_journal, durable.npc_purchase_journal);
        if let Some(journal) = &durable.npc_purchase_journal {
            journal.validate_for("demo", durable.character.index, &durable.character.name, durable.revision).unwrap();
        }
    }

    fn install_checkpoint<F: FnOnce(&mut CharacterSaveRecord)>(&mut self, change: F) {
        let index = self.session.active_identity().unwrap().character_index;
        self.config.commit_account_store_transaction(&["demo".into()], |store| {
            let save = store.accounts.get_mut("demo").unwrap().saves.get_mut(&index).unwrap();
            change(save);
            validate_character_save_record(save)?;
            Ok(())
        }).unwrap();
        let actual_file_checkpoint = self.save_from_file();
        self.session.restore_active_character_checkpoint(&actual_file_checkpoint).unwrap();
        self.session.interact(4990); self.session.select_npc_dialog_target("@BuySell");
    }
}

fn economic_checkpoint(save: &CharacterSaveRecord) -> serde_json::Value {
    let systems: Stage5SystemsState = serde_json::from_str(save.stage5_systems_json.as_deref().unwrap()).unwrap();
    let mut stage5 = serde_json::to_value(&systems).unwrap();
    // Refine deadlines have a runtime clock domain and a durable wall-clock
    // representation. Compare every other Stage5 field without coercing clocks.
    stage5.as_object_mut().unwrap().remove("refine");
    serde_json::json!({ "gold": save.gold, "stage5": stage5,
        "character": save.character, "experience": save.experience, "maxExperience": save.max_experience,
        "inventory": save.inventory_items_json, "belt": save.belt_items_json,
        "equipment": save.equipment_items_json, "storage": save.storage_items_json,
        "capacity": save.inventory_capacity, "buyBack": save.npc_buy_back_items_json,
        "used": save.npc_used_goods_items_json })
}

fn assert_commit(execution: &NpcPurchaseDurableExecution, operation: NpcPurchaseOperation,
    currency: NpcPurchaseCurrency, source: NpcPurchaseSource) {
    let incoming = execution.packets.iter().find_map(|packet| match packet {
        ServerPacket::GainedItem { item } => Some(item), _ => None,
    }).expect("actual gained carrier");
    assert!(!execution.replayed);
    assert_eq!(execution.receipt.entry.operation, operation);
    assert_eq!(execution.receipt.entry.outcome, NpcPurchaseProcessingOutcome::Committed {
        request: operation.intent.request, currency, source, charged: 160, admitted_count: 2,
        incoming_unique_id: incoming.unique_id,
    });
    assert_eq!(incoming.count, 2);
}

#[test]
fn durable_purchase_five_actual_currency_source_branches_share_full_file_checkpoint() {
    for (label, currency, source) in [("BUYSELL", NpcPurchaseCurrency::Gold, NpcPurchaseSource::Trade),
        ("PEARLBUY", NpcPurchaseCurrency::Pearls, NpcPurchaseSource::Trade),
        ("BUYBACK", NpcPurchaseCurrency::Gold, NpcPurchaseSource::BuyBack),
        ("BUYUSED", NpcPurchaseCurrency::Gold, NpcPurchaseSource::Used),
        ("PEARLBUY", NpcPurchaseCurrency::Pearls, NpcPurchaseSource::Used)] {
        let mut fixture = Fixture::new("five-branches", label, source);
        let operation = fixture.operation(1);
        assert_eq!(operation.intent.currency, currency); assert_eq!(operation.intent.source, source);
        let before = fixture.save_from_file();
        let execution = fixture.session.try_durable_npc_purchase(operation).unwrap();
        assert_commit(&execution, operation, currency, source);
        let after = fixture.save_from_file();
        assert_eq!(after.revision, before.revision + 1);
        assert_eq!(execution.receipt.entry.server_revision, after.revision);
        assert_eq!(after.gold, if currency == NpcPurchaseCurrency::Gold { 99_840 } else { 100_000 });
        let systems: Stage5SystemsState = serde_json::from_str(after.stage5_systems_json.as_deref().unwrap()).unwrap();
        assert_eq!(systems.intelligent_creature_pearls, if currency == NpcPurchaseCurrency::Pearls { 840 } else { 1_000 });
        assert_eq!(after.npc_purchase_journal.as_ref().unwrap().entries, vec![execution.receipt.entry.clone()]);
        let npc = fixture.session.app.world().resource::<NpcStateResource>();
        assert!(npc.npc_buy_back_items.iter().all(|stock| stock.items.is_empty()));
        assert!(npc.npc_used_goods_items.iter().all(|stock| stock.items.is_empty()));
        assert_ne!((&after.inventory_items_json, &after.belt_items_json),
            (&before.inventory_items_json, &before.belt_items_json));
        fixture.assert_checkpoint();
        assert_eq!(fixture.session.query_npc_purchase(operation).unwrap(), Some(execution.receipt));
    }
}

#[test]
fn durable_duplicate_original_id_has_empty_packets_without_second_wallet_delivery_or_stock_change() {
    for (label, source) in [("BUYSELL", NpcPurchaseSource::Trade), ("BUYBACK", NpcPurchaseSource::BuyBack),
        ("BUYUSED", NpcPurchaseSource::Used), ("PEARLBUY", NpcPurchaseSource::Used)] {
        let mut fixture = Fixture::new("duplicate", label, source); let operation = fixture.operation(1);
        let first = fixture.session.try_durable_npc_purchase(operation).unwrap();
        let live = fixture.live(); let file = fs::read(&fixture.path).unwrap();
        let replay = fixture.session.try_durable_npc_purchase(operation).unwrap();
        assert!(replay.replayed); assert!(replay.packets.is_empty()); assert_eq!(replay.receipt, first.receipt);
        assert_eq!(fixture.live(), live); assert_eq!(fs::read(&fixture.path).unwrap(), file);
        fixture.assert_checkpoint();
    }
}

#[test]
fn durable_zero_raw_resale_selector_survives_actual_file_journal_and_recovery() {
    let mut fixture = Fixture::new("zero-selector", "BUYUSED", NpcPurchaseSource::Used);
    fixture.session.app.world_mut().resource_mut::<NpcStateResource>().npc_used_goods_items[0].items[0].unique_id = 0;
    fixture.request.item_index = 0;
    fixture.session.save_active_character().unwrap();
    let operation = fixture.operation(1);
    assert_eq!(operation.intent.request.item_index, 0);
    let execution = fixture.session.try_durable_npc_purchase(operation).unwrap();
    assert_commit(&execution, operation, NpcPurchaseCurrency::Gold, NpcPurchaseSource::Used);
    fixture.assert_checkpoint();
    assert_eq!(fixture.save_from_file().npc_purchase_journal.unwrap().lookup(&operation).unwrap().unwrap(),
        &execution.receipt.entry);
    assert_eq!(fixture.session.query_npc_purchase(operation).unwrap(), Some(execution.receipt));
}

#[test]
fn actual_external_mail_is_merged_into_complete_purchase_checkpoint_and_published_live() {
    let mut fixture = Fixture::new("external-mail-merge", "PEARLBUY", NpcPurchaseSource::Used);
    let operation = fixture.operation(1);
    let baseline = fixture.save_from_file();
    assert!(fixture.session.app.world().resource::<Stage5SystemsResource>().stage5_systems.mail.is_empty());
    let delivery = deliver_stage5_system_mail(&fixture.config, Stage5MailDelivery {
        target_kind: Stage5MailTargetKind::Character, target_id: baseline.character.name.clone(),
        from: "Source27 fixture".into(), subject: "Durable mailbox merge".into(),
        body: "Actual server delivery before purchase checkpoint".into(), gold: 77, items: vec![],
    }).unwrap();
    assert_eq!(delivery.delivered_count, 1);
    // The actual mail path retains the full-checkpoint revision and writes an
    // external mailbox delta, which the purchase transaction must merge.
    assert_eq!(fixture.save_from_file().revision, baseline.revision);
    assert!(fixture.session.app.world().resource::<Stage5SystemsResource>().stage5_systems.mail.is_empty());
    let execution = fixture.session.try_durable_npc_purchase(operation).unwrap();
    assert_commit(&execution, operation, NpcPurchaseCurrency::Pearls, NpcPurchaseSource::Used);
    let durable = fixture.save_from_file();
    let systems: Stage5SystemsState = serde_json::from_str(durable.stage5_systems_json.as_deref().unwrap()).unwrap();
    let live = &fixture.session.app.world().resource::<Stage5SystemsResource>().stage5_systems;
    assert_eq!(live.mail, systems.mail);
    assert_eq!(live.mentor, systems.mentor); assert_eq!(live.relationship, systems.relationship);
    assert_eq!(live.economy_projection_event_ids, systems.economy_projection_event_ids);
    assert_eq!(live.intelligent_creature_pearls, 840);
    assert_eq!(systems.mail.len(), 1); assert_eq!(systems.mail[0].gold, 77);
    assert!(!systems.mail[0].claimed); assert!(!systems.mail[0].delivery_nonce.is_empty());
    fixture.assert_checkpoint();
    assert_eq!(fixture.session.query_npc_purchase(operation).unwrap(), Some(execution.receipt));
}

#[test]
fn actual_durable_mentor_credit_merge_publishes_level_xp_roster_and_complete_stage5() {
    let mut fixture = Fixture::new("mentor-checkpoint-merge", "BUYSELL", NpcPurchaseSource::Trade);
    let operation = fixture.operation(1); let before = fixture.save_from_file();
    assert!(before.max_experience > before.experience);
    let credit = u64::try_from(before.max_experience - before.experience).unwrap();
    assert_eq!(fixture.session.app.world().resource::<Stage5SystemsResource>().stage5_systems.mentor.ledger.leveling_credit, 0);
    // Model an actual trusted durable credit awaiting live projection: update
    // only this owned actor's File record through the real source transaction,
    // keeping its complete revision and World unchanged before purchase merge.
    fixture.config.commit_account_store_transaction(&["demo".into()], |store| {
        let save = store.accounts.get_mut("demo").unwrap().saves.get_mut(&0).unwrap();
        let mut systems: Stage5SystemsState = serde_json::from_str(save.stage5_systems_json.as_deref().unwrap()).unwrap();
        systems.mentor.ledger.leveling_credit = credit;
        save.stage5_systems_json = Some(serde_json::to_string(&systems).unwrap());
        validate_character_save_record(save)?;
        Ok(())
    }).unwrap();
    assert_eq!(fixture.save_from_file().revision, before.revision);
    let execution = fixture.session.try_durable_npc_purchase(operation).unwrap();
    assert_commit(&execution, operation, NpcPurchaseCurrency::Gold, NpcPurchaseSource::Trade);
    let durable = fixture.save_from_file();
    assert_eq!(durable.character.level, before.character.level + 1);
    assert_eq!(durable.experience, 0);
    let runtime = fixture.session.app.world().resource::<PlayerRuntimeResource>();
    assert_eq!(runtime.experience, durable.experience); assert_eq!(runtime.max_experience, durable.max_experience);
    let session = fixture.session.app.world().resource::<SessionResource>();
    assert_eq!(session.selected_character.as_ref().unwrap().level, durable.character.level);
    assert_eq!(session.characters.iter().find(|record| record.index == 0).unwrap().level, durable.character.level);
    let player = super::super::components::player_entity(fixture.session.app.world()).unwrap();
    assert_eq!(fixture.session.app.world().entity(player).get::<super::super::components::CharacterBody>().unwrap().level,
        durable.character.level);
    let live = &fixture.session.app.world().resource::<Stage5SystemsResource>().stage5_systems;
    assert_eq!(live.mentor.ledger.leveling_applied, credit);
    let systems: Stage5SystemsState = serde_json::from_str(durable.stage5_systems_json.as_deref().unwrap()).unwrap();
    assert_eq!(live.mentor, systems.mentor);
    fixture.assert_checkpoint();
    assert_eq!(fixture.session.query_npc_purchase(operation).unwrap(), Some(execution.receipt));
}

#[test]
fn conflicting_nonce_tuple_foreign_actor_and_missing_query_never_write_or_debit() {
    let mut fixture = Fixture::new("key-fences", "BUYSELL", NpcPurchaseSource::Trade);
    let original = fixture.operation(1); fixture.session.try_durable_npc_purchase(original).unwrap();
    let before = fixture.live(); let bytes = fs::read(&fixture.path).unwrap();
    let changed = NpcPurchaseOperation { intent: NpcPurchaseIntent {
        request: NpcPurchaseRequest { count: 1, ..original.intent.request }, ..original.intent }, ..original };
    assert!(fixture.session.query_npc_purchase(changed).is_err());
    assert!(fixture.session.try_durable_npc_purchase(changed).is_err());
    let foreign = NpcPurchaseOperation { actor: [9; 32], ..original };
    assert!(fixture.session.query_npc_purchase(foreign).is_err());
    assert!(fixture.session.try_durable_npc_purchase(foreign).is_err());
    assert_eq!(fixture.session.query_npc_purchase(NpcPurchaseOperation { sequence: 2, ..original }).unwrap(), None);
    assert_eq!(fixture.live(), before); assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
}

#[test]
fn genuine_new_session_producer_queries_original_scope_but_cannot_execute_it() {
    let mut fixture = Fixture::new("new-producer", "BUYUSED", NpcPurchaseSource::Used);
    let original = fixture.operation(1); let first = fixture.session.try_durable_npc_purchase(original).unwrap();
    let file = fs::read(&fixture.path).unwrap();
    let mut reopened = login_start(fixture.config.clone(), 0);
    let producer = reopened.begin_npc_purchase_producer().unwrap();
    assert_eq!(producer.actor, original.actor); assert_ne!(producer.producer_scope, original.request_scope);
    let recovered = reopened.query_npc_purchase(original).unwrap().unwrap();
    assert_eq!(recovered.entry, first.receipt.entry); assert_eq!(recovered.producer_scope, producer.producer_scope);
    assert!(matches!(reopened.try_durable_npc_purchase(original), Err(NpcPurchaseDurableError::BeforeExecution { .. })));
    assert_eq!(fs::read(&fixture.path).unwrap(), file);
}

#[test]
fn committed_empty_checkpoint_reopens_without_demo_wallet_or_container_reseeding_before_actor() {
    let mut fixture = Fixture::new("empty-committed-checkpoint", "BUYSELL", NpcPurchaseSource::Trade);
    {
        let mut inventory = fixture.session.app.world_mut().resource_mut::<InventoryResource>();
        inventory.inventory_capacity = 46;
        inventory.inventory_items.clear(); inventory.belt_items.clear();
        inventory.storage_items.clear(); inventory.equipment_items.clear();
    }
    fixture.session.app.world_mut().resource_mut::<PlayerRuntimeResource>().gold = 0;
    fixture.session.save_active_character().unwrap();
    let durable = fixture.save_from_file();
    assert!(durable.revision > 0); assert!(durable.npc_purchase_journal.is_none());
    let file = fs::read(&fixture.path).unwrap();
    let reopened = login_start(fixture.config.clone(), 0);
    let restored = snapshot_active_character_save(reopened.app.world()).unwrap();
    assert_eq!(restored.gold, 0); assert_eq!(restored.inventory_capacity, 46);
    assert!(restored.inventory_items_json.is_empty()); assert!(restored.belt_items_json.is_empty());
    assert!(restored.storage_items_json.is_empty()); assert!(restored.equipment_items_json.is_empty());
    assert_eq!(economic_checkpoint(&restored), economic_checkpoint(&durable));
    assert_eq!(restored.revision, durable.revision); assert!(restored.npc_purchase_journal.is_none());
    assert_eq!(fs::read(&fixture.path).unwrap(), file);
}

#[test]
fn legacy_revision_zero_demo_checkpoint_still_seeds_missing_wallet_and_containers() {
    let fixture = Fixture::new("legacy-uncommitted-checkpoint", "BUYSELL", NpcPurchaseSource::Trade);
    fixture.config.commit_account_store_transaction(&["demo".into()], |store| {
        let save = store.accounts.get_mut("demo").unwrap().saves.get_mut(&0).unwrap();
        assert!(save.npc_purchase_journal.is_none());
        save.revision = 0; save.gold = 0; save.inventory_capacity = 46;
        save.inventory_items_json.clear(); save.belt_items_json.clear(); save.storage_items_json.clear();
        validate_character_save_record(save)?;
        Ok(())
    }).unwrap();
    let reopened = login_start(fixture.config.clone(), 0);
    let durable = fixture.save_from_file();
    let restored = snapshot_active_character_save(reopened.app.world()).unwrap();
    assert_eq!(durable.gold, 1280); assert_eq!(durable.inventory_capacity, 86);
    assert!(!durable.inventory_items_json.is_empty()); assert!(!durable.belt_items_json.is_empty());
    assert!(!durable.storage_items_json.is_empty()); assert!(durable.npc_purchase_journal.is_none());
    // Legacy demo Bag2 stores a slot-local UID0, which load has always normalized
    // to the global bag slot40. Preserve every other checkpoint field exactly.
    let mut expected = durable.clone();
    let encoded = expected.inventory_items_json.iter_mut().find(|encoded|
        serde_json::from_str::<ItemState>(encoded).unwrap().key == "town-teleport").unwrap();
    let mut legacy_bag2: ItemState = serde_json::from_str(encoded).unwrap();
    assert_eq!(legacy_bag2.container, ItemContainer::Bag2);
    assert_eq!(legacy_bag2.slot, 0); assert_eq!(legacy_bag2.unique_id, 0);
    assert!(legacy_bag2.user_item_metadata.is_none());
    legacy_bag2.unique_id = 40;
    *encoded = serde_json::to_string(&legacy_bag2).unwrap();
    assert_eq!(economic_checkpoint(&restored), economic_checkpoint(&expected));
    let file = fs::read(&fixture.path).unwrap();
    let again = login_start(fixture.config.clone(), 0);
    assert_eq!(economic_checkpoint(&snapshot_active_character_save(again.app.world()).unwrap()),
        economic_checkpoint(&expected));
    assert_eq!(fs::read(&fixture.path).unwrap(), file);
}

#[test]
fn durable_terminal_rejection_advances_revision_without_economic_debit() {
    let mut fixture = Fixture::new("rejection", "PEARLBUY", NpcPurchaseSource::Trade);
    fixture.session.app.world_mut().resource_mut::<Stage5SystemsResource>().stage5_systems.intelligent_creature_pearls = 0;
    fixture.session.save_active_character().unwrap();
    let operation = fixture.operation(1); let before = fixture.save_from_file();
    let execution = fixture.session.try_durable_npc_purchase(operation).unwrap();
    assert_eq!(execution.receipt.entry.outcome, NpcPurchaseProcessingOutcome::Rejected {
        request: operation.intent.request, reason: NpcPurchaseRejection::InsufficientCurrency,
    });
    assert!(execution.packets.iter().all(|packet| !matches!(packet, ServerPacket::LoseGold { .. } | ServerPacket::GainedItem { .. })));
    let after = fixture.save_from_file(); assert_eq!(after.revision, before.revision + 1);
    assert_eq!(economic_checkpoint(&after), economic_checkpoint(&before));
    fixture.assert_checkpoint();
    assert_eq!(fixture.session.query_npc_purchase(operation).unwrap(), Some(execution.receipt));
}

#[test]
fn prepublication_faults_preserve_live_file_stock_carrier_and_buyback_expiry() {
    for fault in [AccountStoreTransactionFault::BeforePersist, AccountStoreTransactionFault::BeforeFileRename] {
        let mut fixture = Fixture::new("prepublication", "BUYBACK", NpcPurchaseSource::BuyBack);
        let operation = fixture.operation(1); let before = fixture.live(); let bytes = fs::read(&fixture.path).unwrap();
        fixture.config.inject_account_store_transaction_fault(fault);
        assert!(matches!(fixture.session.try_durable_npc_purchase(operation), Err(NpcPurchaseDurableError::Unknown { .. })));
        assert_eq!(fixture.live(), before); assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
        assert_eq!(fixture.session.query_npc_purchase(operation).unwrap(), None);
        fixture.assert_checkpoint();
    }
}

#[test]
fn ambiguous_rename_freezes_stale_cache_and_query_but_actual_file_retains_terminal_result() {
    let mut fixture = Fixture::new("ambiguous-publication", "BUYUSED", NpcPurchaseSource::Used);
    let operation = fixture.operation(1); let before = fixture.live();
    let cache_before = serde_json::to_value(&*fixture.config.account_store.lock().unwrap()).unwrap();
    fixture.config.inject_account_store_transaction_fault(AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync);
    assert!(matches!(fixture.session.try_durable_npc_purchase(operation), Err(NpcPurchaseDurableError::Unknown { .. })));
    assert_eq!(fixture.live(), before);
    assert_eq!(serde_json::to_value(&*fixture.config.account_store.lock().unwrap()).unwrap(), cache_before);
    let freeze = fixture.config.ensure_account_store_writable().unwrap_err();
    assert!(freeze.contains("frozen")); assert!(freeze.contains("ACCOUNT_STORE_COMMIT_OUTCOME_UNKNOWN"));
    assert!(matches!(fixture.session.query_npc_purchase(operation), Err(NpcPurchaseDurableError::BeforeExecution { .. })));
    assert!(fixture.session.try_durable_npc_purchase(operation).is_err());
    // Read the owned File source independently. A shared cache reopen cannot
    // clear the durable freeze and is deliberately not called recovery here.
    let file = fixture.save_from_file(); let journal = file.npc_purchase_journal.as_ref().unwrap();
    let recorded = journal.lookup(&operation).unwrap().unwrap();
    assert!(matches!(recorded.outcome, NpcPurchaseProcessingOutcome::Committed { charged: 160, admitted_count: 2, .. }));
    assert_eq!(recorded.server_revision, file.revision); assert_eq!(file.gold, 99_840);
    let used: Vec<NpcUsedGoodsState> = file.npc_used_goods_items_json.iter()
        .map(|json| serde_json::from_str(json).unwrap()).collect();
    assert!(used.iter().all(|stock| stock.items.is_empty()));
    journal.validate_for("demo", file.character.index, &file.character.name, file.revision).unwrap();
}

#[test]
fn postcommit_error_and_panic_preserve_exact_receipt_for_original_id_query() {
    for panic in [false, true] {
        let mut fixture = Fixture::new("postcommit", "PEARLBUY", NpcPurchaseSource::Used);
        let operation = fixture.operation(1);
        let error = if panic { fixture.session.durable_npc_purchase_post_panic(operation) }
            else { fixture.session.durable_npc_purchase_post_failure(operation) }.unwrap_err();
        let NpcPurchaseDurableError::PostCommit { receipt, .. } = error else { panic!("committed evidence must survive postprocessing"); };
        assert!(matches!(receipt.entry.outcome, NpcPurchaseProcessingOutcome::Committed {
            currency: NpcPurchaseCurrency::Pearls, source: NpcPurchaseSource::Used, charged: 160, ..
        }));
        fixture.assert_checkpoint();
        assert_eq!(fixture.session.query_npc_purchase(operation).unwrap(), Some(receipt.clone()));
        let before = fixture.live(); let file = fs::read(&fixture.path).unwrap();
        let replay = fixture.session.try_durable_npc_purchase(operation).unwrap();
        assert!(replay.replayed); assert!(replay.packets.is_empty()); assert_eq!(replay.receipt, receipt);
        assert_eq!(fixture.live(), before); assert_eq!(fs::read(&fixture.path).unwrap(), file);
    }
}

#[test]
fn legacy_missing_journal_enables_persistent_actor_and_ordinary_save_keeps_history() {
    let mut fixture = Fixture::new("legacy-save", "BUYSELL", NpcPurchaseSource::Trade);
    let legacy = fixture.save_from_file(); assert!(legacy.npc_purchase_journal.is_none());
    let encoded = serde_json::to_value(&legacy).unwrap(); assert!(encoded.get("npc_purchase_journal").is_none());
    let decoded: CharacterSaveRecord = serde_json::from_value(encoded).unwrap(); assert!(decoded.npc_purchase_journal.is_none());
    let operation = fixture.operation(1); fixture.session.try_durable_npc_purchase(operation).unwrap();
    let retained = fixture.save_from_file().npc_purchase_journal.unwrap();
    fixture.session.save_active_character().unwrap();
    assert_eq!(fixture.save_from_file().npc_purchase_journal, Some(retained.clone()));
    // An older World mirror with this optional field absent cannot erase the
    // authoritative journal through the genuine ordinary full-save path.
    fixture.session.app.world_mut().resource_mut::<SessionResource>().npc_purchase_journal = None;
    fixture.session.save_active_character().unwrap();
    assert_eq!(fixture.save_from_file().npc_purchase_journal, Some(retained.clone()));
    let actual_checkpoint = fixture.save_from_file();
    fixture.session.restore_active_character_checkpoint(&actual_checkpoint).unwrap();
    fixture.session.begin_npc_purchase_producer().unwrap();
    assert_eq!(fixture.session.query_npc_purchase(operation).unwrap().unwrap().entry,
        retained.lookup(&operation).unwrap().unwrap().clone());
}

#[test]
fn wellformed_wrong_catalog_currency_or_source_rejects_without_removing_resale_stock() {
    for (label, source) in [("BUYBACK", NpcPurchaseSource::BuyBack), ("BUYUSED", NpcPurchaseSource::Used),
        ("PEARLBUY", NpcPurchaseSource::Used)] {
        for changed_field in ["proof", "currency", "source"] {
            let mut fixture = Fixture::new("wrong-service-intent", label, source);
            let original = fixture.operation(1); let mut operation = original;
            match changed_field {
                "proof" => operation.intent.service_catalog_proof = [8; 32],
                "currency" => operation.intent.currency = if original.intent.currency == NpcPurchaseCurrency::Gold {
                    NpcPurchaseCurrency::Pearls
                } else { NpcPurchaseCurrency::Gold },
                "source" => operation.intent.source = NpcPurchaseSource::Trade,
                _ => unreachable!(),
            }
            let before = fixture.save_from_file();
            let execution = fixture.session.try_durable_npc_purchase(operation).unwrap();
            assert_eq!(execution.receipt.entry.operation, operation);
            assert_eq!(execution.receipt.entry.outcome, NpcPurchaseProcessingOutcome::Rejected {
                request: operation.intent.request, reason: NpcPurchaseRejection::ServiceUnavailable,
            });
            assert!(execution.packets.iter().all(|packet| !matches!(packet, ServerPacket::LoseGold { .. } | ServerPacket::GainedItem { .. })));
            let after = fixture.save_from_file();
            assert_eq!(economic_checkpoint(&after), economic_checkpoint(&before));
            assert_eq!(after.revision, before.revision + 1);
            fixture.assert_checkpoint();
            assert_eq!(fixture.session.query_npc_purchase(operation).unwrap(), Some(execution.receipt));
        }
    }
}

#[test]
fn failed_logout_or_disconnect_save_retires_authority_while_preserving_original_recovery_state() {
    for packet in [ClientPacket::LogOut, ClientPacket::Disconnect] {
        let mut fixture = Fixture::new("failed-retirement-save", "BUYSELL", NpcPurchaseSource::Trade);
        let operation = fixture.operation(1); let execution = fixture.session.try_durable_npc_purchase(operation).unwrap();
        let before = fixture.live(); let bytes = fs::read(&fixture.path).unwrap();
        fixture.config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
        fixture.session.handle_packet(packet);
        assert_eq!(fixture.live(), before);
        assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
        let session = fixture.session.app.world().resource::<SessionResource>();
        assert_eq!(session.account_id.as_deref(), Some("demo")); assert!(session.selected_character.is_some());
        assert_eq!(session.ranking_inspect_generation(), None);
        assert!(matches!(fixture.session.begin_npc_purchase_producer(), Err(NpcPurchaseDurableError::BeforeExecution { .. })));
        assert!(matches!(fixture.session.npc_purchase_intent(operation.intent.request), Err(NpcPurchaseDurableError::BeforeExecution { .. })));
        assert!(matches!(fixture.session.query_npc_purchase(operation), Err(NpcPurchaseDurableError::BeforeExecution { .. })));
        assert!(matches!(fixture.session.try_durable_npc_purchase(operation), Err(NpcPurchaseDurableError::BeforeExecution { .. })));
        assert_eq!(fixture.live(), before);
        let mut actual_new_owner = login_start(fixture.config.clone(), 0);
        let producer = actual_new_owner.begin_npc_purchase_producer().unwrap();
        assert_eq!(producer.actor, operation.actor); assert_ne!(producer.producer_scope, operation.request_scope);
        assert_eq!(actual_new_owner.query_npc_purchase(operation).unwrap().unwrap().entry, execution.receipt.entry);
    }
}

#[test]
fn older_complete_checkpoint_cannot_roll_back_committed_purchase_or_actor() {
    let mut fixture = Fixture::new("restore-fence", "BUYSELL", NpcPurchaseSource::Trade);
    let operation = fixture.operation(1); let old = fixture.save_from_file();
    fixture.session.try_durable_npc_purchase(operation).unwrap();
    let before = fixture.live(); let file = fs::read(&fixture.path).unwrap();
    assert!(fixture.session.restore_active_character_checkpoint(&old).is_err());
    assert_eq!(fixture.live(), before); assert_eq!(fs::read(&fixture.path).unwrap(), file);
    let mut erased = fixture.save_from_file(); erased.npc_purchase_journal = None;
    assert!(fixture.session.restore_active_character_checkpoint(&erased).is_err());
    assert_eq!(fixture.live(), before);
}

#[test]
fn revision_and_journal_capacity_exhaustion_fail_closed_before_live_purchase() {
    for capacity in [false, true] {
        let mut fixture = Fixture::new("exhaustion", "BUYSELL", NpcPurchaseSource::Trade);
        let original = fixture.operation(1);
        fixture.install_checkpoint(|save| {
            if capacity {
                let base = save.revision;
                let journal = save.npc_purchase_journal.as_mut().unwrap();
                journal.entries = (1..=NPC_PURCHASE_JOURNAL_ENTRY_CAP as u64).map(|sequence| {
                    let operation = NpcPurchaseOperation { sequence, ..original };
                    NpcPurchaseJournalEntry { operation, server_revision: base + sequence,
                        outcome: NpcPurchaseProcessingOutcome::Rejected { request: operation.intent.request,
                            reason: NpcPurchaseRejection::InsufficientCurrency } }
                }).collect();
                save.revision = base + NPC_PURCHASE_JOURNAL_ENTRY_CAP as u64;
            } else { save.revision = u64::MAX - 1; }
        });
        let operation = fixture.operation(NPC_PURCHASE_JOURNAL_ENTRY_CAP as u64 + 1);
        let before = fixture.live(); let file = fs::read(&fixture.path).unwrap();
        assert!(matches!(fixture.session.try_durable_npc_purchase(operation), Err(NpcPurchaseDurableError::BeforeExecution { .. })));
        assert_eq!(fixture.live(), before); assert_eq!(fs::read(&fixture.path).unwrap(), file);
        assert_eq!(fixture.session.query_npc_purchase(operation).unwrap(), None);
    }
}

#[test]
fn actual_delete_and_recreate_retires_old_actor_and_recovery_key() {
    let mut fixture = Fixture::new("character-incarnation", "BUYSELL", NpcPurchaseSource::Trade);
    let old = fixture.operation(1); fixture.session.try_durable_npc_purchase(old).unwrap();
    let character = fixture.save_from_file().character;
    fixture.session.handle_packet(ClientPacket::LogOut);
    fixture.session.handle_packet(ClientPacket::DeleteCharacter { character_index: character.index });
    assert!(!fixture.config.account_store.lock().unwrap().accounts["demo"].characters.iter()
        .any(|record| record.index == character.index));
    let packets = fixture.session.handle_packet(ClientPacket::NewCharacter {
        name: character.name.clone(), gender: character.gender, class: character.class,
    });
    assert!(packets.iter().any(|packet| matches!(packet, ServerPacket::NewCharacterSuccess { .. })));
    let replacement = fixture.config.account_store.lock().unwrap().accounts["demo"].characters.iter()
        .find(|record| record.name == character.name).unwrap().clone();
    // The actual allocator does not reuse a deleted index. Do not claim this
    // fixture tests unsupported same-index recreation or fabricate that path.
    assert_ne!(replacement.index, character.index);
    fixture.session.handle_packet(ClientPacket::StartGame { character_index: replacement.index });
    let producer = fixture.session.begin_npc_purchase_producer().unwrap();
    assert_ne!(producer.actor, old.actor); assert_ne!(producer.producer_scope, old.request_scope);
    assert!(fixture.session.query_npc_purchase(old).is_err());
    assert!(fixture.session.try_durable_npc_purchase(old).is_err());
    assert!(fixture.save_from_file().npc_purchase_journal.unwrap().entries.is_empty());
}

#[path = "npc_purchase_owner_tests.rs"]
mod owner_tests;
