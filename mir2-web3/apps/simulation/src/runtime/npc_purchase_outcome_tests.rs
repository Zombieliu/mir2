//! Actual local handler tests; no transport, durable journal or player acceptance.
use super::*;
use super::super::resources::{InventoryResource, NpcStateResource, PlayerRuntimeResource, SessionResource, Stage5SystemsResource};
use super::super::npc::{NpcBuyBackItemState, NpcBuyBackState, NpcUsedGoodsState};
use super::super::inventory::{add_or_increment_item_with_durability_and_stats, future_binary_datetime_minutes};
use super::super::items::{crystal_item_key_for_template, embedded_item_state_from_template, user_item_from_item_state};
use crate::{ActiveSessionIdentity, InProcessWorldRuntime, ItemContainer, SimulationConfig,
    SimulationSession, VisibleNpcRecord, WorldCommand, WorldRuntime, WorldSnapshot};
use mir2_protocol::{ClientPacket, MirDirection, Point, UserItem};

const SCRIPT: &str = "BichonProvince/NaturalCave/WickedTrader";
fn config() -> SimulationConfig {
    let mut config = SimulationConfig::default();
    config.visible_npcs.push(VisibleNpcRecord { object_id: 4990, name: "Wicked Trader".into(),
        image: 5, colour_argb: -1, position: Point { x: 331, y: 271 }, direction: MirDirection::Left,
        quest_ids: vec![], script_key: Some(SCRIPT.into()) });
    config
}
fn login(s: &mut SimulationSession) {
    assert!(s.handle_packet(ClientPacket::Login { account_id: "demo".into(), password: "demo".into() })
        .iter().any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
}
fn session() -> (SimulationSession, NpcPurchaseRequest) {
    let mut s = SimulationSession::new(config()); login(&mut s);
    s.handle_packet(ClientPacket::StartGame { character_index: 0 }); s.interact(4990);
    let goods = s.select_npc_dialog_target("@BuySell");
    let uid = goods.iter().find_map(|p| match p {
        ServerPacket::NPCGoods { list, .. } => list.iter().find(|i| i.item_index == 658 && i.count == 1).map(|i| i.unique_id),
        _ => None,
    }).expect("real trade selector");
    {
        let mut i = s.app.world_mut().resource_mut::<InventoryResource>();
        i.inventory_capacity = 86; i.inventory_items.clear(); i.belt_items.clear();
        i.equipment_items.clear(); i.storage_items.clear(); i.reserved_item_unique_ids.clear();
    }
    s.app.world_mut().resource_mut::<PlayerRuntimeResource>().gold = 100_000;
    s.app.world_mut().resource_mut::<Stage5SystemsResource>().stage5_systems.intelligent_creature_pearls = 1_000;
    (s, NpcPurchaseRequest { item_index: uid, count: 2, panel_type: 0 })
}
fn source_item(uid: u64, count: u16) -> UserItem {
    let template = mir2_game_data::crystal_item_by_index(658).unwrap();
    UserItem { unique_id: uid, item_index: 658, current_dura: template.durability,
        max_dura: template.durability, count, soul_bound_id: -1, identified: false, cursed: false,
        slots: vec![None; usize::from(template.slots)], gem_count: 0, added_stats: vec![], awake_type: 0,
        awake_values: vec![], refined_value: 0, refine_added: 0, refine_success_chance: 0, wedding_ring: -1,
        expire_info: None, rental_information: None, is_shop_item: false, sealed_info: None, gm_made: false }
}
fn resale(s: &mut SimulationSession, label: &str, item: UserItem) -> NpcPurchaseRequest {
    let name = s.app.world().resource::<SessionResource>().selected_character.as_ref().unwrap().name.clone();
    let request = NpcPurchaseRequest { item_index: item.unique_id, count: 2, panel_type: 0 };
    let mut npc = s.app.world_mut().resource_mut::<NpcStateResource>();
    npc.active_npc_service.as_mut().unwrap().label_key = label.into();
    if label == "BUYBACK" {
        npc.npc_buy_back_items.push(NpcBuyBackState { script_key: SCRIPT.into(), player_name: name,
            items: vec![NpcBuyBackItemState { item, expires_at_binary_datetime: future_binary_datetime_minutes(60) }] });
    } else {
        npc.npc_used_goods_items.push(NpcUsedGoodsState { script_key: SCRIPT.into(), items: vec![item] });
    }
    request
}
fn snapshot(s: &SimulationSession) -> serde_json::Value {
    let i = s.app.world().resource::<InventoryResource>(); let n = s.app.world().resource::<NpcStateResource>();
    serde_json::json!({ "gold": s.app.world().resource::<PlayerRuntimeResource>().gold,
        "pearls": s.app.world().resource::<Stage5SystemsResource>().stage5_systems.intelligent_creature_pearls,
        "inventory": i.inventory_items, "belt": i.belt_items, "equipment": i.equipment_items,
        "storage": i.storage_items, "capacity": i.inventory_capacity, "reserved": i.reserved_item_unique_ids,
        "used": n.npc_used_goods_items, "buyBack": n.npc_buy_back_items })
}
fn expect_commit(e: &NpcPurchaseProcessingExecution, request: NpcPurchaseRequest,
    currency: NpcPurchaseCurrency, source: NpcPurchaseSource, count: u16, charged: u32) -> UserItem {
    let item = e.packets.iter().find_map(|p| match p { ServerPacket::GainedItem { item } => Some(item.clone()), _ => None }).unwrap();
    assert_eq!(e.outcome, NpcPurchaseProcessingOutcome::Committed { request, currency, source,
        charged, admitted_count: count, incoming_unique_id: item.unique_id });
    assert_eq!(item.count, count);
    assert_eq!(e.packets.iter().filter(|p| matches!(p, ServerPacket::LoseGold { gold } if *gold == charged)).count(),
        usize::from(currency == NpcPurchaseCurrency::Gold));
    item
}
fn expect_rejection(s: &mut SimulationSession, request: NpcPurchaseRequest, reason: NpcPurchaseRejection) {
    let before = snapshot(s); let e = s.try_npc_purchase_with_outcome(request).unwrap();
    assert_eq!(e.outcome, NpcPurchaseProcessingOutcome::Rejected { request, reason });
    assert!(e.packets.iter().all(|p| !matches!(p, ServerPacket::LoseGold { .. } | ServerPacket::GainedItem { .. })));
    assert_eq!(snapshot(s), before);
}

#[test]
fn npc_purchase_all_primary_branches_capture_actual_currency_source_and_delivery() {
    for (label, currency, source) in [("BUYSELL", NpcPurchaseCurrency::Gold, NpcPurchaseSource::Trade),
        ("PEARLBUY", NpcPurchaseCurrency::Pearls, NpcPurchaseSource::Trade),
        ("BUYBACK", NpcPurchaseCurrency::Gold, NpcPurchaseSource::BuyBack),
        ("BUYUSED", NpcPurchaseCurrency::Gold, NpcPurchaseSource::Used)] {
        let (mut s, mut request) = session();
        if source == NpcPurchaseSource::Trade {
            s.app.world_mut().resource_mut::<NpcStateResource>().active_npc_service.as_mut().unwrap().label_key = label.into();
        } else { request = resale(&mut s, label, source_item(444_000, 2)); }
        let e = s.try_npc_purchase_with_outcome(request).unwrap();
        expect_commit(&e, request, currency, source, 2, 160);
        assert_eq!(s.app.world().resource::<PlayerRuntimeResource>().gold,
            if currency == NpcPurchaseCurrency::Gold { 99_840 } else { 100_000 });
        assert_eq!(s.app.world().resource::<Stage5SystemsResource>().stage5_systems.intelligent_creature_pearls,
            if currency == NpcPurchaseCurrency::Pearls { 840 } else { 1_000 });
        let n = s.app.world().resource::<NpcStateResource>();
        assert!(n.npc_buy_back_items.iter().all(|entry| entry.items.is_empty()));
        assert!(n.npc_used_goods_items.iter().all(|entry| entry.items.is_empty()));
    }
}

#[test]
fn npc_purchase_pearl_used_fallback_keeps_currency_and_source_orthogonal() {
    let (mut s, _) = session(); let request = resale(&mut s, "PEARLBUY", source_item(444_001, 2));
    let e = s.try_npc_purchase_with_outcome(request).unwrap();
    expect_commit(&e, request, NpcPurchaseCurrency::Pearls, NpcPurchaseSource::Used, 2, 160);
    assert_eq!(s.app.world().resource::<PlayerRuntimeResource>().gold, 100_000);
    assert_eq!(s.app.world().resource::<Stage5SystemsResource>().stage5_systems.intelligent_creature_pearls, 840);
    assert!(s.app.world().resource::<NpcStateResource>().npc_used_goods_items[0].items.is_empty());
}

#[test]
fn npc_purchase_resale_admitted_count_is_clamped_and_removes_legacy_whole_entry() {
    for label in ["BUYBACK", "BUYUSED"] {
        for (requested, stock, admitted) in [(3, 2, 2), (1, 5, 1)] {
            let (mut s, _) = session(); let mut request = resale(&mut s, label, source_item(444_002, stock));
            request.count = requested;
            let e = s.try_npc_purchase_with_outcome(request).unwrap();
            expect_commit(&e, request, NpcPurchaseCurrency::Gold,
                if label == "BUYBACK" { NpcPurchaseSource::BuyBack } else { NpcPurchaseSource::Used }, admitted, u32::from(admitted) * 80);
            let n = s.app.world().resource::<NpcStateResource>();
            assert!(n.npc_buy_back_items.iter().all(|entry| entry.items.is_empty()));
            assert!(n.npc_used_goods_items.iter().all(|entry| entry.items.is_empty()));
        }
    }
}

#[test]
fn npc_purchase_zero_resale_selector_is_not_reconstructed_from_template_index() {
    let (mut s, _) = session(); let request = resale(&mut s, "BUYUSED", source_item(0, 2));
    let e = s.try_npc_purchase_with_outcome(request).unwrap();
    expect_commit(&e, request, NpcPurchaseCurrency::Gold, NpcPurchaseSource::Used, 2, 160);
}

#[test]
fn npc_purchase_resale_delivery_matches_valid_old_gain_carrier() {
    for label in ["BUYBACK", "BUYUSED"] {
        let (mut s, _) = session(); let (mut oracle, _) = session(); let source = source_item(444_003, 2);
        let template = mir2_game_data::crystal_item_by_index(source.item_index).unwrap();
        let key = crystal_item_key_for_template(&template);
        // Execute the old valid per-item delivery helper, rather than comparing
        // two new entry points which share the same staged implementation.
        let old_state = add_or_increment_item_with_durability_and_stats(oracle.app.world_mut(), ItemContainer::Bag1,
            &key, &template.name, template.tooltip.as_deref().unwrap_or("Crystal NPC shop item."), 8, 2,
            u16::from(template.weight.max(1)), Some(source.current_dura), Some(source.max_dura), 0, 0);
        let mut old = user_item_from_item_state(&old_state);
        assert_eq!(old.unique_id, old_state.unique_id);
        let request = resale(&mut s, label, source);
        let e = s.try_npc_purchase_with_outcome(request).unwrap();
        let mut new = expect_commit(&e, request, NpcPurchaseCurrency::Gold,
            if label == "BUYBACK" { NpcPurchaseSource::BuyBack } else { NpcPurchaseSource::Used }, 2, 160);
        let inventory = s.app.world().resource::<InventoryResource>();
        assert!(inventory.inventory_items.iter().chain(inventory.belt_items.iter()).any(|item|
            item.unique_id == new.unique_id && item.quantity == u32::from(new.count)),
            "per-item resale carrier must identify the actual committed delivery, including UID0");
        old.unique_id = 0; new.unique_id = 0; assert_eq!(old, new);
    }
}

#[test]
fn npc_purchase_resale_invalid_source_or_roster_never_debits_or_removes_stock() {
    for label in ["BUYBACK", "BUYUSED", "PEARLBUY"] {
        for invalid_source in [false, true] {
            let (mut s, _) = session(); let mut source = source_item(444_004, 2);
            if invalid_source { source.count = 0; }
            let request = resale(&mut s, label, source);
            if !invalid_source { s.app.world_mut().resource_mut::<InventoryResource>().inventory_capacity = 0; }
            expect_rejection(&mut s, request, NpcPurchaseRejection::InvalidDelivery);
        }
    }
}

#[test]
fn npc_purchase_resale_stack_slack_without_free_cells_cannot_overwrite_inventory() {
    for label in ["BUYBACK", "BUYUSED"] {
        let (mut s, _) = session(); let request = resale(&mut s, label, source_item(444_005, 2));
        let template = mir2_game_data::crystal_item_by_index(658).unwrap();
        let i = &mut *s.app.world_mut().resource_mut::<InventoryResource>(); i.inventory_capacity = 46;
        for slot in 0..40u8 {
            let mut item = embedded_item_state_from_template(&template, ItemContainer::Bag1, slot);
            item.unique_id = 1_000_000 + u64::from(slot); item.quantity = 1; i.inventory_items.push(item);
        }
        for slot in 0..6u8 {
            let template = mir2_game_data::crystal_item_manifest_ref().items.iter().find(|t|
                super::super::items::crystal_belt_slot_range_for_item_key(&crystal_item_key_for_template(t))
                    .is_some_and(|(start, end)| (start..end).contains(&slot))).unwrap();
            let mut item = embedded_item_state_from_template(template, ItemContainer::Belt, slot);
            item.unique_id = 2_000_000 + u64::from(slot); i.belt_items.push(item);
        }
        expect_rejection(&mut s, request, NpcPurchaseRejection::InvalidDelivery);
    }
}

#[test]
fn npc_purchase_insufficient_currency_is_rejected_without_reusing_previous_commit() {
    let (mut s, request) = session();
    let e = s.try_npc_purchase_with_outcome(request).unwrap();
    expect_commit(&e, request, NpcPurchaseCurrency::Gold, NpcPurchaseSource::Trade, 2, 160);
    s.app.world_mut().resource_mut::<NpcStateResource>().active_npc_service.as_mut().unwrap().label_key = "PEARLBUY".into();
    s.app.world_mut().resource_mut::<Stage5SystemsResource>().stage5_systems.intelligent_creature_pearls = 0;
    expect_rejection(&mut s, request, NpcPurchaseRejection::InsufficientCurrency);
}

#[test]
fn npc_purchase_known_resale_commit_survives_postprocessing_failure_or_panic() {
    for label in ["BUYBACK", "BUYUSED", "PEARLBUY"] {
        for panic in [false, true] {
            let (mut s, _) = session(); let request = resale(&mut s, label, source_item(444_006, 2));
            let error = if panic { s.npc_purchase_with_postprocessing_panic(request) }
                else { s.npc_purchase_with_postprocessing_failure(request) }.unwrap_err();
            let NpcPurchaseProcessingError::PostProcessing { outcome, .. } = error else { panic!("known commit must survive"); };
            assert!(matches!(outcome, NpcPurchaseProcessingOutcome::Committed { request: r, charged: 160,
                admitted_count: 2, .. } if r == request));
            assert_eq!(s.app.world().resource::<PlayerRuntimeResource>().gold, if label == "PEARLBUY" { 100_000 } else { 99_840 });
            assert_eq!(s.app.world().resource::<Stage5SystemsResource>().stage5_systems.intelligent_creature_pearls,
                if label == "PEARLBUY" { 840 } else { 1_000 });
            let n = s.app.world().resource::<NpcStateResource>();
            assert!(n.npc_buy_back_items.iter().all(|entry| entry.items.is_empty()));
            assert!(n.npc_used_goods_items.iter().all(|entry| entry.items.is_empty()));
        }
    }
}

#[test]
fn npc_purchase_missing_capture_or_precommit_panic_is_unknown_and_does_not_debit() {
    assert!(matches!(finish_npc_purchase_processing(Ok(vec![]), None), Err(NpcPurchaseProcessingError::Unknown { .. })));
    let (mut s, _) = session(); let request = resale(&mut s, "BUYUSED", source_item(444_007, 2));
    let before = snapshot(&s); let inventory = s.app.world_mut().remove_resource::<InventoryResource>().unwrap();
    assert!(matches!(s.try_npc_purchase_with_outcome(request), Err(NpcPurchaseProcessingError::Unknown { .. })));
    s.app.world_mut().insert_resource(inventory); assert_eq!(snapshot(&s), before);
}

#[test]
fn npc_purchase_session_and_runtime_require_authentication_and_start_game() {
    let request = NpcPurchaseRequest { item_index: 0, count: 2, panel_type: 0 };
    let mut s = SimulationSession::new(config()); let before = snapshot(&s);
    assert_eq!(s.try_npc_purchase_with_outcome(request).unwrap_err(),
        NpcPurchaseProcessingError::BeforeExecution(NpcPurchaseBeforeExecution::NotAuthenticated));
    assert_eq!(snapshot(&s), before); login(&mut s);
    assert_eq!(s.try_npc_purchase_with_outcome(request).unwrap_err(),
        NpcPurchaseProcessingError::BeforeExecution(NpcPurchaseBeforeExecution::NotInGame));
    let mut runtime = InProcessWorldRuntime::new(config()); assert!(runtime.supports_typed_npc_purchase_outcome());
    assert_eq!(runtime.execute_production_npc_purchase_requiring_typed_outcome(true, request).unwrap_err(),
        NpcPurchaseProcessingError::BeforeExecution(NpcPurchaseBeforeExecution::NotAuthenticated));
    runtime.execute(WorldCommand::ClientPacket(ClientPacket::Login { account_id: "demo".into(), password: "demo".into() })).unwrap();
    runtime.execute(WorldCommand::ClientPacket(ClientPacket::StartGame { character_index: 0 })).unwrap();
    assert_eq!(runtime.execute_production_npc_purchase_requiring_typed_outcome(false, request).unwrap_err(),
        NpcPurchaseProcessingError::BeforeExecution(NpcPurchaseBeforeExecution::NotAuthenticated));
}

struct Unsupported { session: SimulationSession, executions: usize }
impl WorldRuntime for Unsupported {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn on_connect(&self) -> Vec<ServerPacket> { vec![] }
    fn execute(&mut self, _: WorldCommand) -> Result<Vec<ServerPacket>, String> { self.executions += 1; Ok(vec![]) }
    fn world_snapshot(&self) -> WorldSnapshot { self.session.world_snapshot() }
    fn active_identity(&self) -> Option<ActiveSessionIdentity> { self.session.active_identity() }
    fn save_active_character(&mut self) -> Result<(), String> { panic!("must not save") }
    fn refresh_active_external_mail(&mut self) -> bool { false }
}
#[test]
fn npc_purchase_unsupported_runtime_never_executes_legacy_fallback() {
    let mut runtime = Unsupported { session: SimulationSession::new(config()), executions: 0 };
    assert!(!runtime.supports_typed_npc_purchase_outcome());
    assert_eq!(runtime.execute_production_npc_purchase_requiring_typed_outcome(true,
        NpcPurchaseRequest { item_index: 0, count: 2, panel_type: 0 }).unwrap_err(),
        NpcPurchaseProcessingError::BeforeExecution(NpcPurchaseBeforeExecution::UnsupportedRuntime));
    assert_eq!(runtime.executions, 0);
}
