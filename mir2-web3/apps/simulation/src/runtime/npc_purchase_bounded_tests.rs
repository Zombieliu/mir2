//! Fixed companion supply scope over the real durable File purchase path.
//! Synthetic fixtures retain their own files under guarded runner TEMP; no
//! public packet, QA command, network connection or production account is used.
use super::super::npc::{NpcBuyBackItemState, NpcBuyBackState};
use super::super::resources::{NpcStateResource, PlayerRuntimeResource};
use super::*;
use crate::{
    NpcPurchaseOwnerAction as Action, NpcPurchaseOwnerReply as Reply, SimulationConfig,
    VisibleNpcRecord,
};
use mir2_protocol::{ClientPacket, MirDirection, Point, UserItem};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const GROCERY: &str = "BichonProvince/BorderVillage/Grocery";
const OWNER: [u8; 32] = [117; 32];
static NEXT: AtomicU64 = AtomicU64::new(0);

fn limit(max_gold: u32) -> NpcPurchaseGoldLimit {
    NpcPurchaseGoldLimit {
        max_gold,
        max_count: 1,
        item_template_index: 658,
        npc_index: 9,
    }
}

struct Fixture {
    session: SimulationSession,
    path: PathBuf,
    request: NpcPurchaseRequest,
    goods: Vec<UserItem>,
}
impl Fixture {
    fn new(label: &str, script: &str) -> Self {
        let temporary = std::env::temp_dir()
            .canonicalize()
            .expect("runner TEMP exists");
        let directory = temporary.join(format!(
            "mir2-bounded-npc-{label}-{}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(directory.starts_with(&temporary));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("accounts.json");
        let mut config = SimulationConfig::default().with_account_store_path(&path);
        assert!(config.account_store_database_url.is_none());
        config.visible_npcs.push(VisibleNpcRecord {
            object_id: 4990,
            name: "Bounded Supply Merchant".into(),
            image: 5,
            colour_argb: -1,
            position: Point { x: 331, y: 271 },
            direction: MirDirection::Left,
            quest_ids: vec![],
            script_key: Some(script.into()),
        });
        let mut session = SimulationSession::new(config);
        assert!(session
            .handle_packet(ClientPacket::Login {
                account_id: "demo".into(),
                password: "demo".into()
            })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        session.interact(4990);
        let packets = session.select_npc_dialog_target("@BuySell");
        let goods = packets
            .into_iter()
            .find_map(|packet| match packet {
                ServerPacket::NPCGoods { list, .. } => Some(list),
                _ => None,
            })
            .expect("real buy service goods");
        let uid = goods
            .iter()
            .find(|item| item.item_index == 658 && item.count == 1)
            .expect("real small HP potion")
            .unique_id;
        {
            let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
            inventory.inventory_capacity = 86;
            inventory.inventory_items.clear();
            inventory.belt_items.clear();
            inventory.equipment_items.clear();
            inventory.storage_items.clear();
            inventory.reserved_item_unique_ids.clear();
        }
        session
            .app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold = 100;
        session
            .app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .intelligent_creature_pearls = 1000;
        session.save_active_character().unwrap();
        Self {
            session,
            path,
            request: NpcPurchaseRequest {
                item_index: uid,
                count: 1,
                panel_type: 0,
            },
            goods,
        }
    }
    fn operation(&mut self, sequence: u64) -> NpcPurchaseOperation {
        let begun = self
            .session
            .execute_npc_purchase_owner(true, OWNER, Action::Begin)
            .unwrap();
        let Reply::Producer { producer } = begun.reply else {
            panic!("real producer");
        };
        let quote = self
            .session
            .execute_npc_purchase_owner(
                true,
                OWNER,
                Action::Quote {
                    request: self.request,
                },
            )
            .unwrap();
        let Reply::Quote { intent } = quote.reply else {
            panic!("real quote");
        };
        NpcPurchaseOperation {
            actor: producer.actor,
            request_scope: producer.producer_scope,
            sequence,
            intent,
        }
    }
    fn live(&self) -> serde_json::Value {
        let world = self.session.app.world();
        let inventory = world.resource::<InventoryResource>();
        let npc = world.resource::<NpcStateResource>();
        let session = world.resource::<SessionResource>();
        serde_json::json!({ "gold": world.resource::<PlayerRuntimeResource>().gold,
            "pearls": world.resource::<Stage5SystemsResource>().stage5_systems.intelligent_creature_pearls,
            "items": inventory.inventory_items, "belt": inventory.belt_items, "buyBack": npc.npc_buy_back_items,
            "used": npc.npc_used_goods_items, "revision": session.active_save_revision(), "journal": session.npc_purchase_journal })
    }
    fn purchase(
        &mut self,
        operation: NpcPurchaseOperation,
        limit: NpcPurchaseGoldLimit,
    ) -> Result<crate::NpcPurchaseOwnerExecution, NpcPurchaseDurableError> {
        self.session.execute_npc_purchase_owner(
            true,
            OWNER,
            Action::PurchaseBounded { operation, limit },
        )
    }
    fn assert_refused_unchanged(
        &mut self,
        operation: NpcPurchaseOperation,
        limit: NpcPurchaseGoldLimit,
    ) {
        let live = self.live();
        let file = fs::read(&self.path).unwrap();
        assert!(matches!(
            self.purchase(operation, limit),
            Err(NpcPurchaseDurableError::BeforeExecution { .. })
        ));
        assert_eq!(self.live(), live);
        assert_eq!(fs::read(&self.path).unwrap(), file);
        assert_eq!(self.session.query_npc_purchase(operation).unwrap(), None);
    }
}

#[test]
fn bounded_supply_budget_39_refuses_actual_40_without_wallet_inventory_or_receipt_write() {
    let mut f = Fixture::new("budget39", GROCERY);
    let operation = f.operation(1);
    f.assert_refused_unchanged(operation, limit(39));
}

#[test]
fn bounded_supply_budget_50_charges_40_once_and_replays_the_original_operation() {
    let mut f = Fixture::new("budget50", GROCERY);
    let operation = f.operation(1);
    let bought = f.purchase(operation, limit(50)).unwrap();
    let Reply::Purchase { receipt, replayed } = bought.reply else {
        panic!("real receipt");
    };
    assert!(!replayed);
    assert!(matches!(
        receipt.entry.outcome,
        NpcPurchaseProcessingOutcome::Committed {
            currency: NpcPurchaseCurrency::Gold,
            source: NpcPurchaseSource::Trade,
            charged: 40,
            admitted_count: 1,
            ..
        }
    ));
    assert_eq!(
        f.session
            .app
            .world()
            .resource::<PlayerRuntimeResource>()
            .gold,
        60
    );
    let inventory = f.session.app.world().resource::<InventoryResource>();
    let count: u32 = inventory
        .inventory_items
        .iter()
        .chain(inventory.belt_items.iter())
        .filter(|item| {
            item.user_item_metadata
                .as_ref()
                .and_then(|metadata| metadata.item_index)
                == Some(658)
        })
        .map(|item| item.quantity)
        .sum();
    assert_eq!(count, 1);
    let live = f.live();
    let file = fs::read(&f.path).unwrap();
    let replay = f.purchase(operation, limit(50)).unwrap();
    assert!(matches!(
        replay.reply,
        Reply::Purchase { replayed: true, .. }
    ));
    assert!(replay.packets.is_empty());
    assert_eq!(f.live(), live);
    assert_eq!(fs::read(&f.path).unwrap(), file);
    // A changed limit never describes an already committed higher charge as permitted.
    assert!(matches!(
        f.purchase(operation, limit(39)),
        Err(NpcPurchaseDurableError::BeforeExecution { .. })
    ));
    assert_eq!(f.live(), live);
    assert_eq!(fs::read(&f.path).unwrap(), file);
}

#[test]
fn bounded_supply_rejects_wrong_live_vendor_template_currency_and_quantity() {
    let mut wrong_vendor = Fixture::new("vendor", "BichonProvince/NaturalCave/WickedTrader");
    let operation = wrong_vendor.operation(1);
    wrong_vendor.assert_refused_unchanged(operation, limit(50));
    let mut wrong_template = Fixture::new("template", GROCERY);
    wrong_template.request.item_index = wrong_template
        .goods
        .iter()
        .find(|item| item.item_index != 658 && item.count == 1)
        .expect("another real grocery good")
        .unique_id;
    let operation = wrong_template.operation(1);
    wrong_template.assert_refused_unchanged(operation, limit(50));
    let mut wrong_currency = Fixture::new("currency", GROCERY);
    wrong_currency
        .session
        .app
        .world_mut()
        .resource_mut::<NpcStateResource>()
        .active_npc_service
        .as_mut()
        .unwrap()
        .label_key = "PEARLBUY".into();
    let operation = wrong_currency.operation(1);
    assert_eq!(operation.intent.currency, NpcPurchaseCurrency::Pearls);
    wrong_currency.assert_refused_unchanged(operation, limit(50));
    let mut wrong_count = Fixture::new("count", GROCERY);
    wrong_count.request.count = 2;
    let operation = wrong_count.operation(1);
    wrong_count.assert_refused_unchanged(operation, limit(50));
}

#[test]
fn bounded_supply_rejects_actual_buyback_source_and_limits_outside_the_fixed_scope() {
    let mut f = Fixture::new("source", GROCERY);
    let mut item = f
        .goods
        .iter()
        .find(|item| item.item_index == 658)
        .unwrap()
        .clone();
    item.unique_id = 443_399;
    let name = f.session.active_identity().unwrap().character_name;
    {
        let mut npc = f.session.app.world_mut().resource_mut::<NpcStateResource>();
        npc.active_npc_service.as_mut().unwrap().label_key = "BUYBACK".into();
        npc.npc_buy_back_items.push(NpcBuyBackState {
            script_key: GROCERY.into(),
            player_name: name,
            items: vec![NpcBuyBackItemState {
                item,
                expires_at_binary_datetime: super::super::inventory::future_binary_datetime_minutes(
                    60,
                ),
            }],
        });
    }
    f.session.save_active_character().unwrap();
    f.request.item_index = 443_399;
    let operation = f.operation(1);
    assert_eq!(operation.intent.source, NpcPurchaseSource::BuyBack);
    f.assert_refused_unchanged(operation, limit(50));
    let mut g = Fixture::new("limits", GROCERY);
    let operation = g.operation(1);
    for bad in [
        NpcPurchaseGoldLimit {
            max_gold: 51,
            ..limit(50)
        },
        NpcPurchaseGoldLimit {
            max_gold: 0,
            ..limit(50)
        },
        NpcPurchaseGoldLimit {
            max_count: 2,
            ..limit(50)
        },
        NpcPurchaseGoldLimit {
            item_template_index: 659,
            ..limit(50)
        },
        NpcPurchaseGoldLimit {
            npc_index: 10,
            ..limit(50)
        },
    ] {
        g.assert_refused_unchanged(operation, bad);
    }
}

#[test]
fn bounded_supply_rejects_a_stale_lower_price_proof_and_changed_prepared_charge() {
    let mut f = Fixture::new("price", GROCERY);
    let mut operation = f.operation(1);
    let world = f.session.app.world();
    let service = npc::current_crystal_npc_service_in_range(world).unwrap();
    let selected =
        npc::crystal_npc_service_item_for_purchase(world, &service, f.request.item_index).unwrap();
    let template = mir2_game_data::crystal_item_by_index(658).unwrap();
    // An old/fabricated proof for half the current price cannot authorize the
    // current authoritative catalogue. This does not mutate global game data.
    operation.intent = catalog_intent(
        &service,
        &selected,
        &template,
        0.5,
        f.request,
        operation.actor,
        operation.request_scope,
    )
    .unwrap();
    f.assert_refused_unchanged(operation, limit(50));
    let outcome = NpcPurchaseProcessingOutcome::Committed {
        request: f.request,
        currency: NpcPurchaseCurrency::Gold,
        source: NpcPurchaseSource::Trade,
        charged: 60,
        admitted_count: 1,
        incoming_unique_id: 100,
    };
    assert!(validate_bounded_purchase_outcome(&outcome, limit(50), Some(40)).is_err());
}

#[test]
fn bounded_supply_is_host_only_and_limit_fields_remain_strict() {
    let limit = limit(50);
    let value = serde_json::to_value(limit).unwrap();
    assert_eq!(
        serde_json::from_value::<NpcPurchaseGoldLimit>(value.clone()).unwrap(),
        limit
    );
    let mut extra = value.clone();
    extra["accountId"] = "invented".into();
    assert!(serde_json::from_value::<NpcPurchaseGoldLimit>(extra).is_err());
    assert!(serde_json::from_str::<NpcPurchaseGoldLimit>(
        r#"{"maxGold":50,"maxGold":1,"maxCount":1,"itemTemplateIndex":658,"npcIndex":9}"#
    )
    .is_err());
    let operation = NpcPurchaseOperation {
        actor: [1; 32],
        request_scope: [2; 32],
        sequence: 1,
        intent: NpcPurchaseIntent {
            request: NpcPurchaseRequest {
                item_index: 1,
                count: 1,
                panel_type: 0,
            },
            currency: NpcPurchaseCurrency::Gold,
            source: NpcPurchaseSource::Trade,
            service_catalog_proof: [3; 32],
        },
    };
    let internal = Action::PurchaseBounded { operation, limit };
    assert!(internal.is_purchase());
    assert!(internal.is_mutation());
    assert_eq!(internal.operation(), Some(operation));
    let encoded = serde_json::to_string(&internal).unwrap();
    assert!(
        serde_json::from_str::<Action>(&encoded).is_err(),
        "normal clients cannot deserialize the internal bounded action"
    );
    assert!(serde_json::from_str::<Action>(&format!(
        r#"{{"kind":"purchase","operation":{},"limit":{}}}"#,
        serde_json::to_string(&operation).unwrap(),
        serde_json::to_string(&limit).unwrap()
    ))
    .is_err());
}
