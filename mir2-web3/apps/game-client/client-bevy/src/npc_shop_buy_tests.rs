use super::*;
use crate::inventory::{CrystalItemTooltipSourceModel, CrystalUserItemModel, ItemModel};
use crate::shop::{shop_buy_enabled_with_pearls, shop_buy_item_command, shop_buy_quantity_max,
    NpcShopServiceMode, ShopGood};

fn catalog(base_price: u32, rate: f32, displayed_price: u32) -> ShopModel {
    let source = CrystalItemTooltipSourceModel {
        info: CrystalItemInfoModel {
            item_index: 658, price: base_price, stack_size: 99, item_type: 13,
            ..Default::default()
        },
        user_item: Some(CrystalUserItemModel {
            unique_id: 0, item_index: 658, count: 1, is_shop_item: true,
            ..Default::default()
        }),
        ..Default::default()
    };
    ShopModel {
        selected_id: Some(0), service_mode: NpcShopServiceMode::Buy,
        goods: vec![ShopGood {
            unique_id: 0, price: displayed_price, purchase_rate: Some(rate), requires_gold_buy_plan: true, count: 1,
            // The raw ordinary Trade carrier does not describe finite stock.
            stock: -1, tooltip_source: Some(source), ..Default::default()
        }],
        ..Default::default()
    }
}

fn inventory(gold: u32) -> InventoryModel { InventoryModel { gold, ..Default::default() } }
fn row(container: u8, slot: u32, id: u64) -> ItemModel {
    ItemModel { unique_id: Some(id), container, slot, quantity: 1, ..Default::default() }
}
fn source_mut(shop: &mut ShopModel) -> &mut CrystalItemTooltipSourceModel {
    shop.goods[0].tooltip_source.as_mut().unwrap()
}
fn reason(plan: NpcGoldBuyPlan, expected: NpcGoldBuyBlockReason) {
    assert!(!plan.can_buy);
    assert_eq!(plan.block_reason, Some(expected));
    assert!(plan.command.is_none());
}
fn request_json(shop: &ShopModel, inventory: &InventoryModel, quantity: u16) -> Value {
    serde_json::to_value(NpcGoldBuyRequest { shop: shop.clone(), inventory: inventory.clone(), quantity }).unwrap()
}
fn json_plan(value: &Value) -> NpcGoldBuyPlan {
    serde_json::from_str(&plan_npc_gold_buy_json(&value.to_string())).unwrap()
}

#[test]
fn fractional_rate_quotes_the_stack_before_rounding_and_keeps_catalog_uid_zero() {
    let shop = catalog(1, 1.5, 1);
    let plan = plan_npc_gold_buy(&shop, &inventory(3), 2);
    assert!(plan.can_buy);
    assert_eq!(plan.total_gold, Some(3));
    assert_eq!(plan.max_quantity, 99);
    assert_eq!(plan.block_reason, None);
    assert_eq!(plan.command, Some(NpcGoldBuyCommand {
        command_type: NpcGoldBuyCommandType::BuyItem, item_index: 0, count: 2, panel_type: 0,
    }));
    let poor = plan_npc_gold_buy(&shop, &inventory(2), 2);
    assert_eq!(poor.total_gold, Some(3));
    assert_eq!(poor.max_quantity, 99);
    reason(poor, NpcGoldBuyBlockReason::InsufficientGold);
}

#[test]
fn exact_f32_and_u32_saturation_are_observable_quote_semantics() {
    let rounded = catalog(16_777_217, 1.0, 16_777_216);
    assert_eq!(plan_npc_gold_buy(&rounded, &inventory(u32::MAX), 1).total_gold, Some(16_777_216));
    let saturated = catalog(3_000_000_000, 0.5, 1_500_000_000);
    assert_eq!(plan_npc_gold_buy(&saturated, &inventory(u32::MAX), 2).total_gold, Some(2_147_483_648));
    let overflow = catalog(u32::MAX, f32::MAX, u32::MAX);
    assert_eq!(plan_npc_gold_buy(&overflow, &inventory(u32::MAX), 99).total_gold, Some(u32::MAX));
}

#[test]
fn quantity_is_not_reclamped_and_catalog_count_is_not_a_purchase_limit() {
    let mut shop = catalog(10, 1.0, 10);
    source_mut(&mut shop).info.stack_size = 2;
    assert!(plan_npc_gold_buy(&shop, &inventory(100), 2).can_buy);
    for quantity in [0, 3, 100, u16::MAX] {
        let plan = plan_npc_gold_buy(&shop, &inventory(100), quantity);
        assert_eq!(plan.max_quantity, 2);
        reason(plan, NpcGoldBuyBlockReason::InvalidQuantity);
    }
    source_mut(&mut shop).info.stack_size = 0;
    assert_eq!(plan_npc_gold_buy(&shop, &inventory(100), 1).max_quantity, 1);
    reason(plan_npc_gold_buy(&shop, &inventory(100), 2), NpcGoldBuyBlockReason::InvalidQuantity);
    source_mut(&mut shop).info.stack_size = u16::MAX;
    assert_eq!(plan_npc_gold_buy(&shop, &inventory(1000), 99).max_quantity, 99);
    reason(plan_npc_gold_buy(&shop, &inventory(1000), 100), NpcGoldBuyBlockReason::InvalidQuantity);
}

#[test]
fn missing_invalid_rate_and_stale_projected_unit_price_cannot_enter_legacy() {
    for rate in [None, Some(-1.0), Some(f32::NAN), Some(f32::INFINITY)] {
        let mut shop = catalog(1, 1.5, 1);
        shop.goods[0].purchase_rate = rate;
        reason(plan_npc_gold_buy(&shop, &inventory(100), 1), NpcGoldBuyBlockReason::InvalidRate);
        assert!(!shop_buy_enabled_with_pearls(&shop, &inventory(100), 1, 100));
        assert!(shop_buy_item_command(&shop, &inventory(100), 1, 100).is_none());
    }
    let stale = catalog(1, 1.5, 2);
    reason(plan_npc_gold_buy(&stale, &inventory(100), 2), NpcGoldBuyBlockReason::InvalidPrice);
    assert!(plan_npc_gold_buy(&catalog(100, 0.0, 0), &inventory(0), 99).can_buy);
}

#[test]
fn full_raw_uid_template_and_count_must_match_without_substituting_another_row() {
    for change in 0..5 {
        let mut shop = catalog(10, 1.0, 10);
        match change {
            0 => source_mut(&mut shop).user_item.as_mut().unwrap().unique_id = 658,
            1 => source_mut(&mut shop).user_item.as_mut().unwrap().item_index = 659,
            2 => source_mut(&mut shop).user_item.as_mut().unwrap().count = 2,
            3 => source_mut(&mut shop).user_item = None,
            _ => shop.goods[0].tooltip_source = None,
        }
        reason(plan_npc_gold_buy(&shop, &inventory(100), 1), NpcGoldBuyBlockReason::InvalidSource);
    }
    let mut duplicate = catalog(10, 1.0, 10);
    duplicate.goods.push(duplicate.goods[0].clone());
    reason(plan_npc_gold_buy(&duplicate, &inventory(100), 1), NpcGoldBuyBlockReason::InvalidCatalog);
    duplicate.goods.pop();
    duplicate.selected_id = Some(658);
    reason(plan_npc_gold_buy(&duplicate, &inventory(100), 1), NpcGoldBuyBlockReason::InvalidCatalog);
}

#[test]
fn service_currency_panel_and_raw_shop_flag_are_required() {
    let mut shop = catalog(10, 1.0, 10);
    shop.service_mode = NpcShopServiceMode::Closed;
    reason(plan_npc_gold_buy(&shop, &inventory(100), 1), NpcGoldBuyBlockReason::ServiceUnavailable);
    shop.supports_buy = true;
    assert!(plan_npc_gold_buy(&shop, &inventory(100), 1).can_buy);
    shop.selected_id = None;
    reason(plan_npc_gold_buy(&shop, &inventory(100), 1), NpcGoldBuyBlockReason::NoSelection);
    shop.selected_id = Some(0);
    for change in 0..3 {
        let mut other = shop.clone();
        match change {
            0 => other.goods[0].use_pearls = true,
            1 => other.goods[0].panel_type = 1,
            _ => source_mut(&mut other).user_item.as_mut().unwrap().is_shop_item = false,
        }
        reason(plan_npc_gold_buy(&other, &inventory(100), 1), NpcGoldBuyBlockReason::UnsupportedGoods);
    }
}

#[test]
fn full_bag_requires_an_allowed_empty_belt_cell_and_never_existing_stack_room() {
    let mut shop = catalog(10, 1.0, 10);
    let mut inv = inventory(100);
    inv.items = (0..40).map(|slot| row(0, slot, u64::from(slot) + 1)).collect();
    assert!(plan_npc_gold_buy(&shop, &inv, 2).can_buy, "medicine may enter Belt0..4");
    inv.items.extend((0..4).map(|slot| row(1, slot, u64::from(slot) + 101)));
    let full = plan_npc_gold_buy(&shop, &inv, 2);
    assert_eq!(full.total_gold, Some(20));
    reason(full, NpcGoldBuyBlockReason::InventoryFull);
    source_mut(&mut shop).info.item_type = 8;
    assert!(plan_npc_gold_buy(&shop, &inv, 2).can_buy, "scroll may enter Belt4..6");
    inv.items.extend((4..6).map(|slot| row(1, slot, u64::from(slot) + 101)));
    inv.items[0].key = "template-658".to_owned();
    let mut existing_source = shop.goods[0].tooltip_source.clone().unwrap();
    let existing_user = existing_source.user_item.as_mut().unwrap();
    existing_user.unique_id = inv.items[0].unique_id.unwrap();
    existing_user.is_shop_item = false;
    inv.items[0].tooltip_source = Some(existing_source);
    inv.items[0].quantity = 1; // Even a compatible almost-empty stack is occupied.
    reason(plan_npc_gold_buy(&shop, &inv, 2), NpcGoldBuyBlockReason::InventoryFull);
    inv.items.retain(|item| !(item.container == 0 && item.slot == 39));
    assert!(plan_npc_gold_buy(&shop, &inv, 2).can_buy);
}

#[test]
fn all_belt_type_ranges_and_effect_gate_match_the_actual_insert_path() {
    let mut inv = inventory(100);
    inv.items = (0..40).map(|slot| row(0, slot, u64::from(slot) + 1)).collect();
    for (item_type, effect, expected) in [(13, 0, true), (17, 0, true), (21, 1, true),
        (21, 0, false), (8, 0, true), (1, 0, false)] {
        let mut shop = catalog(10, 1.0, 10);
        source_mut(&mut shop).info.item_type = item_type;
        source_mut(&mut shop).info.effect = effect;
        assert_eq!(plan_npc_gold_buy(&shop, &inv, 1).can_buy, expected, "type {item_type}/effect {effect}");
    }
}

#[test]
fn actual_expanded_bag_capacity_and_malformed_layout_are_distinct() {
    let mut shop = catalog(10, 1.0, 10);
    source_mut(&mut shop).info.item_type = 1;
    let mut inv = inventory(100);
    inv.capacity = 54;
    inv.items = (0..48).map(|slot| row(0, slot, u64::from(slot))).collect();
    reason(plan_npc_gold_buy(&shop, &inv, 1), NpcGoldBuyBlockReason::InventoryFull);
    inv.items.pop();
    assert!(plan_npc_gold_buy(&shop, &inv, 1).can_buy, "Bag2 physical cell47 is real");
    for change in 0..6 {
        let mut invalid = inv.clone();
        match change {
            0 => invalid.capacity = 53,
            1 => invalid.items.push(row(0, 0, 1000)),
            2 => invalid.items.push(row(0, 47, 0)),
            3 => invalid.items[0].unique_id = None,
            4 => invalid.items[0].quantity = 0,
            _ => invalid.items.push(row(0, 48, 1000)),
        }
        reason(plan_npc_gold_buy(&shop, &invalid, 1), NpcGoldBuyBlockReason::InvalidInventory);
    }
}

#[test]
fn native_gold_helpers_share_raw_count_quote_and_max_while_legacy_remains_compatible() {
    let mut shop = catalog(1, 1.5, 1);
    source_mut(&mut shop).info.stack_size = 2;
    assert_eq!(shop_buy_quantity_max(&shop, &inventory(3)), 2);
    assert_eq!(shop_buy_item_command(&shop, &inventory(3), 2, 0).unwrap().count, 2);
    assert!(!shop_buy_enabled_with_pearls(&shop, &inventory(2), 2, 0));
    assert!(shop_buy_item_command(&shop, &inventory(1000), 3, 0).is_none());
    shop.goods[0].purchase_rate = None;
    shop.goods[0].tooltip_source = None;
    shop.goods[0].requires_gold_buy_plan = false;
    shop.goods[0].stock = -1;
    assert_eq!(shop_buy_item_command(&shop, &inventory(1000), 100, 0).unwrap().count, 99);
    shop.goods[0].use_pearls = true;
    assert!(!shop_buy_enabled_with_pearls(&shop, &inventory(1000), 1, 0));
    assert!(shop_buy_enabled_with_pearls(&shop, &inventory(0), 1, 1));
}

#[test]
fn json_entry_returns_only_the_camel_case_plan_and_existing_model_schema() {
    let request = request_json(&catalog(1, 1.5, 1), &inventory(3), 2);
    let output: Value = serde_json::from_str(&plan_npc_gold_buy_json(&request.to_string())).unwrap();
    assert_eq!(output, serde_json::json!({
        "maxQuantity":99,"totalGold":3,"canBuy":true,"blockReason":null,
        "command":{"type":"buyItem","itemIndex":0,"count":2,"panelType":0}
    }));
}

#[test]
fn json_missing_input_and_full_raw_members_never_use_serde_defaults() {
    let base = request_json(&catalog(1, 1.5, 1), &inventory(3), 2);
    for key in ["shop", "inventory", "quantity"] {
        let mut value = base.clone(); value.as_object_mut().unwrap().remove(key);
        reason(json_plan(&value), NpcGoldBuyBlockReason::InvalidInput);
    }
    for key in ["capacity", "gold", "items"] {
        let mut value = base.clone(); value["inventory"].as_object_mut().unwrap().remove(key);
        reason(json_plan(&value), NpcGoldBuyBlockReason::InvalidInput);
    }
    let source = &base["shop"]["goods"][0]["tooltip_source"];
    for key in source["info"].as_object().unwrap().keys() {
        let mut value = base.clone();
        value["shop"]["goods"][0]["tooltip_source"]["info"].as_object_mut().unwrap().remove(key);
        reason(json_plan(&value), NpcGoldBuyBlockReason::InvalidInput);
    }
    for key in source["userItem"].as_object().unwrap().keys() {
        let mut value = base.clone();
        value["shop"]["goods"][0]["tooltip_source"]["userItem"].as_object_mut().unwrap().remove(key);
        reason(json_plan(&value), NpcGoldBuyBlockReason::InvalidInput);
    }
}

#[test]
fn json_duplicate_escaped_members_trailing_input_and_bounds_fail_closed() {
    let json = request_json(&catalog(1, 1.5, 1), &inventory(3), 2).to_string();
    for input in [
        json.replacen("\"quantity\":2", "\"quantity\":1,\"quantity\":2", 1),
        json.replacen("\"count\":1", "\"count\":9,\"count\":1", 1),
        json.replacen("\"price\":1", "\"price\":2,\"pr\\u0069ce\":1", 1),
        format!("{json} null"),
        " ".repeat(MAX_INPUT_BYTES + 1),
    ] {
        let plan: NpcGoldBuyPlan = serde_json::from_str(&plan_npc_gold_buy_json(&input)).unwrap();
        reason(plan, NpcGoldBuyBlockReason::InvalidInput);
    }
    let nested = format!("{}0{}", "[".repeat(MAX_JSON_DEPTH + 2), "]".repeat(MAX_JSON_DEPTH + 2));
    let plan: NpcGoldBuyPlan = serde_json::from_str(&plan_npc_gold_buy_json(&nested)).unwrap();
    reason(plan, NpcGoldBuyBlockReason::InvalidInput);
    let mut oversized = request_json(&catalog(1, 1.5, 1), &inventory(3), 2);
    oversized["shop"]["goods"] = Value::Array(vec![oversized["shop"]["goods"][0].clone(); MAX_GOODS + 1]);
    reason(json_plan(&oversized), NpcGoldBuyBlockReason::InvalidInput);
}

#[test]
fn positive_admission_marker_never_downgrades_incomplete_gold_authority_to_legacy() {
    let mut shop = catalog(1, 1.5, 1);
    shop.goods[0].tooltip_source = None;
    shop.goods[0].purchase_rate = None;
    assert!(shop.goods[0].uses_gold_buy_plan());
    reason(plan_npc_gold_buy(&shop, &inventory(100), 1), NpcGoldBuyBlockReason::InvalidSource);
    assert!(!shop_buy_enabled_with_pearls(&shop, &inventory(100), 1, 0));
    assert!(shop_buy_item_command(&shop, &inventory(100), 1, 0).is_none());
    let mut unsupported = catalog(1, 1.5, 1);
    source_mut(&mut unsupported).user_item.as_mut().unwrap().is_shop_item = false;
    reason(plan_npc_gold_buy(&unsupported, &inventory(100), 1), NpcGoldBuyBlockReason::UnsupportedGoods);
}

#[test]
fn finite_stock_is_outside_ordinary_planner_and_keeps_the_native_legacy_path() {
    let mut shop = catalog(1, 1.5, 1);
    shop.goods[0].stock = 2;
    assert!(!shop.goods[0].uses_gold_buy_plan());
    reason(plan_npc_gold_buy(&shop, &inventory(100), 2), NpcGoldBuyBlockReason::UnsupportedGoods);
    assert_eq!(shop_buy_item_command(&shop, &inventory(2), 2, 0).unwrap().count, 2);
    assert!(shop_buy_item_command(&shop, &inventory(100), 3, 0).is_none());
}

#[test]
fn public_raw_presence_checks_and_json_marker_are_required_before_defaults() {
    let mut value = request_json(&catalog(1, 1.5, 1), &inventory(3), 2);
    let source = value["shop"]["goods"][0]["tooltip_source"].clone();
    assert!(full_npc_gold_user_item(&source["userItem"]));
    assert!(full_npc_gold_tooltip_source(&source));
    let mut missing = source.clone();
    missing["userItem"].as_object_mut().unwrap().remove("is_shop_item");
    assert!(!full_npc_gold_user_item(&missing["userItem"]));
    assert!(!full_npc_gold_tooltip_source(&missing));
    missing = source.clone();
    missing["info"].as_object_mut().unwrap().remove("price");
    assert!(!full_npc_gold_tooltip_source(&missing));
    missing = source;
    missing["userItem"] = Value::Null;
    assert!(!full_npc_gold_tooltip_source(&missing));
    value["shop"]["goods"][0].as_object_mut().unwrap().remove("requires_gold_buy_plan");
    reason(json_plan(&value), NpcGoldBuyBlockReason::InvalidInput);
}

// Complete independent fresh-carrier fixture, not a call to the production matcher.
fn capacity_row(slot: u32, id: u64, count: u16, container: u8) -> ItemModel {
    ItemModel { unique_id: Some(id), slot, container, quantity: u32::from(count),
        tooltip_source: Some(CrystalItemTooltipSourceModel {
            info: CrystalItemInfoModel { item_index:658, price:1, stack_size:99, item_type:13, ..Default::default() },
            user_item: Some(CrystalUserItemModel { unique_id:id, item_index:658, count,
                identified:false, soul_bound_id:-1, wedding_ring:-1, ..Default::default() }),
            ..Default::default()
        }), ..Default::default() }
}
fn full_capacity_inventory() -> InventoryModel {
    InventoryModel { gold:100, items:(0..40).map(|slot| row(0,slot,1000+u64::from(slot)))
        .chain((0..6).map(|slot| row(1,slot,2000+u64::from(slot)))).collect(), ..Default::default() }
}
fn attest(inventory: &mut InventoryModel, ids: Vec<u64>) {
    inventory.npc_gold_trade_capacity = Some(crate::inventory::NpcGoldTradeCapacity {
        roster_valid:true, fresh_compatible_unique_ids:ids });
}
#[test]
fn npc_gold_capacity_full_bag_requires_attested_exact_fresh_stack_and_keeps_quote_wire() {
    let shop=catalog(1,1.5,1); let mut inv=full_capacity_inventory();
    inv.items[0]=capacity_row(0,77,97,0);
    reason(plan_npc_gold_buy(&shop,&inv,2),NpcGoldBuyBlockReason::InventoryFull);
    attest(&mut inv,vec![]);
    reason(plan_npc_gold_buy(&shop,&inv,2),NpcGoldBuyBlockReason::InventoryFull);
    attest(&mut inv,vec![77]);let before=serde_json::to_value(&inv).unwrap();
    let plan=plan_npc_gold_buy(&shop,&inv,2);assert!(plan.can_buy);assert_eq!(plan.total_gold,Some(3));
    assert_eq!(plan.command.unwrap().item_index,0);
    assert_eq!(serde_json::to_value(&inv).unwrap(),before);
    reason(plan_npc_gold_buy(&shop,&inv,3),NpcGoldBuyBlockReason::InventoryFull);
}
#[test]
fn npc_gold_capacity_multiple_stacks_full_info_and_own_carrier_bindings() {
    let shop=catalog(1,1.,1);let mut inv=full_capacity_inventory();
    inv.items[0]=capacity_row(0,77,97,0);inv.items[1]=capacity_row(1,78,98,0);attest(&mut inv,vec![77,78]);
    assert!(plan_npc_gold_buy(&shop,&inv,3).can_buy);
    reason(plan_npc_gold_buy(&shop,&inv,4),NpcGoldBuyBlockReason::InventoryFull);
    let mut different=inv.clone();different.items[0].tooltip_source.as_mut().unwrap().info.price=2;
    reason(plan_npc_gold_buy(&shop,&different,2),NpcGoldBuyBlockReason::InventoryFull);
    for mutation in 0..5 {let mut bad=inv.clone();let item=&mut bad.items[0];let source=item.tooltip_source.as_mut().unwrap();
        match mutation {0=>source.user_item.as_mut().unwrap().count=96,
            1=>source.user_item.as_mut().unwrap().unique_id=79,
            2=>source.user_item.as_mut().unwrap().identified=true,
            3=>source.info.stack_size=96,_=>source.user_item.as_mut().unwrap().added_stats.push(Default::default())}
        reason(plan_npc_gold_buy(&shop,&bad,1),NpcGoldBuyBlockReason::InvalidInventory);
    }
}
#[test]
fn npc_gold_capacity_false_attestation_rejects_even_empty_inventory() {
    let shop=catalog(1,1.,1);let mut inv=inventory(100);
    inv.npc_gold_trade_capacity=Some(crate::inventory::NpcGoldTradeCapacity {roster_valid:false,fresh_compatible_unique_ids:vec![]});
    reason(plan_npc_gold_buy(&shop,&inv,1),NpcGoldBuyBlockReason::InvalidInventory);
    inv.npc_gold_trade_capacity=None;assert!(plan_npc_gold_buy(&shop,&inv,1).can_buy);
}
#[test]
fn npc_gold_capacity_cross_grid_unlisted_aliases_do_not_grant_merge_or_ambiguous_listed_uid() {
    let shop=catalog(1,1.,1);let mut inv=inventory(100);
    inv.items=vec![row(0,0,0),row(1,0,0),row(2,0,0),row(3,0,0)];
    reason(plan_npc_gold_buy(&shop,&inv,1),NpcGoldBuyBlockReason::InvalidInventory);
    attest(&mut inv,vec![]);assert!(plan_npc_gold_buy(&shop,&inv,1).can_buy);
    inv.items.push(row(0,1,0));reason(plan_npc_gold_buy(&shop,&inv,1),NpcGoldBuyBlockReason::InvalidInventory);
    inv.items.pop();inv.items[0]=capacity_row(0,0,98,0);attest(&mut inv,vec![0]);
    reason(plan_npc_gold_buy(&shop,&inv,1),NpcGoldBuyBlockReason::InvalidInventory);
    inv.items=vec![capacity_row(0,0,98,0)];assert!(plan_npc_gold_buy(&shop,&inv,1).can_buy);
    attest(&mut inv,vec![42]);reason(plan_npc_gold_buy(&shop,&inv,1),NpcGoldBuyBlockReason::InvalidInventory);
    inv.capacity=86;inv.items=vec![row(0,0,0),row(0,40,0),row(1,0,0)];attest(&mut inv,vec![]);
    reason(plan_npc_gold_buy(&shop,&inv,1),NpcGoldBuyBlockReason::InvalidInventory); // Bag1/Bag2 share one Inventory packet grid.
    inv.items.push(row(0,41,0));reason(plan_npc_gold_buy(&shop,&inv,1),NpcGoldBuyBlockReason::InvalidInventory);
}
#[test]
fn npc_gold_capacity_belt_partition_and_expanded_last_bag_cell_are_authoritative() {
    let shop=catalog(1,1.,1);let mut inv=inventory(100);inv.items=vec![capacity_row(4,77,98,1)];attest(&mut inv,vec![77]);
    reason(plan_npc_gold_buy(&shop,&inv,1),NpcGoldBuyBlockReason::InvalidInventory);
    inv.items[0].slot=3;assert!(plan_npc_gold_buy(&shop,&inv,1).can_buy);
    inv.capacity=86;inv.items[0]=capacity_row(79,77,98,0);assert!(plan_npc_gold_buy(&shop,&inv,1).can_buy);
    inv.items[0].slot=80;reason(plan_npc_gold_buy(&shop,&inv,1),NpcGoldBuyBlockReason::InvalidInventory);
}
#[test]
fn npc_gold_capacity_raw_evidence_is_exact_bounded_and_full_carrier_presence_checked() {
    let shop=catalog(1,1.,1);let mut inv=inventory(100);inv.items=vec![capacity_row(0,0,98,0)];attest(&mut inv,vec![0]);
    let good=request_json(&shop,&inv,1);assert!(json_plan(&good).can_buy);
    for raw in [serde_json::json!({"rosterValid":true}),
        serde_json::json!({"rosterValid":true,"freshCompatibleUniqueIds":[0],"extra":1}),
        serde_json::json!({"rosterValid":true,"freshCompatibleUniqueIds":[0,0]}),
        serde_json::json!({"rosterValid":false,"freshCompatibleUniqueIds":[0]}),
        serde_json::json!({"rosterValid":true,"freshCompatibleUniqueIds":(0..87).collect::<Vec<u64>>()}),serde_json::json!({"rosterValid":true,"freshCompatibleUniqueIds":[-1]})] {
        let mut bad=good.clone();bad["inventory"]["npcGoldTradeCapacity"]=raw;
        reason(json_plan(&bad),NpcGoldBuyBlockReason::InvalidInput);
    }
    let mut bad=good.clone();bad["inventory"]["items"][0]["tooltipSource"]["userItem"].as_object_mut().unwrap().remove("identified");
    reason(json_plan(&bad),NpcGoldBuyBlockReason::InvalidInput);
    let duplicate=good.to_string().replace("\"rosterValid\":true","\"rosterValid\":true,\"rosterValid\":true");
    reason(serde_json::from_str(&plan_npc_gold_buy_json(&duplicate)).unwrap(),NpcGoldBuyBlockReason::InvalidInput);
}


#[test]
fn npc_gold_capacity_native_u64_full_range_and_nullable_legacy_remain_lossless() {
    let shop=catalog(1,1.,1);let mut inv=inventory(100);
    inv.items=vec![capacity_row(0,u64::MAX,98,0)];
    assert!(plan_npc_gold_buy(&shop,&inv,1).can_buy,"Native typed u64 is not JS Number");
    attest(&mut inv,vec![u64::MAX]);
    let round_trip:InventoryModel=serde_json::from_value(serde_json::to_value(&inv).unwrap()).unwrap();
    assert_eq!(round_trip.npc_gold_trade_capacity.as_ref().unwrap().fresh_compatible_unique_ids,vec![u64::MAX]);
    assert!(plan_npc_gold_buy(&shop,&round_trip,1).can_buy);
    let legacy=request_json(&shop,&inventory(100),1);let mut null=legacy.clone();
    null["inventory"]["npcGoldTradeCapacity"]=Value::Null;
    assert_eq!(json_plan(&null),json_plan(&legacy));assert!(json_plan(&null).can_buy);
}


#[test]
fn npc_gold_expiry_protocol_and_common_json_keep_every_signed_tick() {
    use crate::inventory::CrystalUserItemExpireModel;
    use mir2_protocol::UserItemExpireInfo;
    for ticks in [i64::MIN, -1, 0, 42, 9_007_199_254_740_991, 9_007_199_254_740_992,
        9_007_199_254_740_993, 638_000_000_000_000_001, 3_155_378_975_999_999_999,
        7_767_064_994_427_387_903, i64::MAX] {
        let wire = UserItemExpireInfo { expiry_binary_datetime: ticks };
        let encoded = serde_json::to_value(&wire).unwrap();
        assert_eq!(encoded["expiry_binary_datetime"].as_str(), Some(ticks.to_string().as_str()));
        let common: CrystalUserItemExpireModel = serde_json::from_value(encoded.clone()).unwrap();
        assert_eq!(common.expiry_binary_datetime, ticks);
        assert_eq!(serde_json::to_value(common).unwrap(), encoded);
        let legacy = format!("{{\"expiry_binary_datetime\":{ticks}}}");
        assert_eq!(serde_json::from_str::<UserItemExpireInfo>(&legacy).unwrap(), wire);
        assert_eq!(serde_json::from_str::<CrystalUserItemExpireModel>(&legacy).unwrap().expiry_binary_datetime, ticks);
    }
}

#[test]
fn npc_gold_expiry_invalid_decimal_forms_and_float_json_are_rejected() {
    use crate::inventory::CrystalUserItemExpireModel;
    use mir2_protocol::UserItemExpireInfo;
    for raw in [serde_json::json!(""), serde_json::json!("+1"), serde_json::json!("-0"),
        serde_json::json!("01"), serde_json::json!(" 1"), serde_json::json!("1 "),
        serde_json::json!("1.0"), serde_json::json!("1e0"), serde_json::json!("9223372036854775808"),
        serde_json::json!("-9223372036854775809"), serde_json::json!(u64::MAX),
        serde_json::json!(1.0), serde_json::json!(true), Value::Null] {
        let expiry = serde_json::json!({"expiry_binary_datetime":raw});
        assert!(serde_json::from_value::<UserItemExpireInfo>(expiry.clone()).is_err());
        assert!(serde_json::from_value::<CrystalUserItemExpireModel>(expiry).is_err());
    }
    assert!(serde_json::from_str::<UserItemExpireInfo>("{\"expiry_binary_datetime\":1e0}").is_err());
}

#[test]
fn npc_gold_expiry_json_representation_preserves_crystal_binary_packet() {
    use crate::inventory::CrystalUserItemExpireModel;
    for ticks in [i64::MIN, 3_155_378_975_999_999_999, 7_767_064_994_427_387_903, i64::MAX] {
        let common = CrystalUserItemModel { unique_id:77, item_index:658, count:1,
            current_dura:100, max_dura:100,
            expire_info:Some(CrystalUserItemExpireModel {expiry_binary_datetime:ticks}),
            ..Default::default() };
        let current = serde_json::to_value(&common).unwrap();
        let mut old = current.clone();
        old["expire_info"]["expiry_binary_datetime"] = serde_json::json!(ticks);
        let a: mir2_protocol::UserItem = serde_json::from_value(current).unwrap();
        let b: mir2_protocol::UserItem = serde_json::from_value(old).unwrap();
        let packet = mir2_protocol::ServerPacket::GainedItem { item:a };
        let bytes = mir2_protocol::encode_server_packet(&packet).unwrap();
        assert_eq!(bytes, mir2_protocol::encode_server_packet(&mir2_protocol::ServerPacket::GainedItem { item:b }).unwrap());
        assert_eq!(mir2_protocol::decode_server_packet(&bytes).unwrap(), packet);
    }
}

#[test]
fn npc_gold_expiry_strict_carrier_accepts_exact_dates_without_weakening_raw_fields() {
    use crate::inventory::CrystalUserItemExpireModel;
    let shop = catalog(1,1.,1);
    for ticks in [3_155_378_975_999_999_999, 7_767_064_994_427_387_903, i64::MIN] {
        let mut inv = inventory(100);
        inv.items = vec![capacity_row(0,77,1,0)];
        inv.items[0].tooltip_source.as_mut().unwrap().user_item.as_mut().unwrap().expire_info =
            Some(CrystalUserItemExpireModel {expiry_binary_datetime:ticks});
        let good = request_json(&shop,&inv,1);
        assert!(full_npc_gold_buy_inventory(&good["inventory"]));
        assert!(json_plan(&good).can_buy);
        let decoded: NpcGoldBuyRequest = serde_json::from_value(good.clone()).unwrap();
        assert_eq!(decoded.inventory.items[0].tooltip_source.as_ref().unwrap().user_item.as_ref().unwrap().expire_info.unwrap().expiry_binary_datetime, ticks);
        let mut old = good.clone();
        old["inventory"]["items"][0]["tooltipSource"]["userItem"]["expire_info"]["expiry_binary_datetime"] = serde_json::json!(ticks);
        assert!(json_plan(&old).can_buy);
        for invalid in [serde_json::json!("+1"),serde_json::json!("01"),serde_json::json!("-0"),
            serde_json::json!("9223372036854775808"),serde_json::json!(1.0),Value::Null] {
            let mut bad=good.clone();
            bad["inventory"]["items"][0]["tooltipSource"]["userItem"]["expire_info"]["expiry_binary_datetime"]=invalid;
            assert!(!full_npc_gold_buy_inventory(&bad["inventory"]));
            reason(json_plan(&bad),NpcGoldBuyBlockReason::InvalidInput);
        }
        let mut missing=good.clone();
        missing["inventory"]["items"][0]["tooltipSource"]["userItem"]["expire_info"].as_object_mut().unwrap().remove("expiry_binary_datetime");
        assert!(!full_npc_gold_buy_inventory(&missing["inventory"]));
        reason(json_plan(&missing),NpcGoldBuyBlockReason::InvalidInput);
        for (field, invalid) in [("count",65536_i64),("current_dura",65536),("max_dura",-1),("unique_id",-1)] {
            let mut bad=good.clone();
            bad["inventory"]["items"][0]["tooltipSource"]["userItem"][field]=serde_json::json!(invalid);
            reason(json_plan(&bad),NpcGoldBuyBlockReason::InvalidInput);
        }
        let mut missing_raw=good.clone();
        missing_raw["inventory"]["items"][0]["tooltipSource"]["userItem"].as_object_mut().unwrap().remove("gm_made");
        reason(json_plan(&missing_raw),NpcGoldBuyBlockReason::InvalidInput);
    }
}
