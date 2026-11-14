use super::observe_own_offer;
use crate::social::{SocialModel, MAX_TRADE_ITEMS};
use serde_json::{json, Value};

fn legacy_offer() -> (Value, Value, SocialModel) {
    let snapshot = json!({
        "inventoryItems": [{"container": "bag1", "slot": 0, "uniqueId": 0}],
        "stage5Systems": {"trade": {
            "settlementNonce": "legacy-offer",
            "partner": "Guest",
            "offeredSlots": {"3": 0},
            "offeredUniqueIds": {"3": 0},
            "offeredGold": 7,
            "offeredCurrency": "gold",
            "locked": false,
            "completed": false
        }}
    });
    let inventory = json!({
        "gold": 50,
        "items": [{
            "container": 0, "slot": 0, "uniqueId": 0,
            "key": "potion", "name": "Potion", "quantity": 5,
            "tooltipSource": {
                "info": {"item_index": 27},
                "userItem": {"unique_id": 0, "item_index": 27, "count": 5}
            }
        }]
    });
    let mut social = SocialModel::default();
    assert!(social.apply_packet("TradeAccept", &json!({"name": "Guest"})));
    (snapshot, inventory, social)
}

#[test]
fn explicit_legacy_id_zero_is_an_occupied_trade_slot() {
    let (mut snapshot, inventory, mut social) = legacy_offer();
    assert!(observe_own_offer(&snapshot, &inventory, &mut social));
    assert_eq!(social.trade.my_items.iter().flatten().count(), 1);
    let item = social.trade.my_items[3].as_ref().unwrap();
    assert_eq!(item.unique_id, Some(0));
    assert_eq!(item.item_index, Some(27));
    assert_eq!(item.count, 5);
    assert_eq!(social.trade.my_gold, 7);

    let offer = &mut snapshot["stage5Systems"]["trade"];
    offer["offeredSlots"] = json!({});
    offer["offeredUniqueIds"] = json!({});
    assert!(observe_own_offer(&snapshot, &inventory, &mut social));
    assert!(social.trade.my_items.iter().all(Option::is_none));
    assert_eq!(social.trade.state, "open");
}

#[test]
fn legacy_offer_requires_exact_identity_and_positive_quantity() {
    let (snapshot, inventory, social) = legacy_offer();
    for (field, value) in [
        ("uniqueId", json!(null)),
        ("uniqueId", json!(42)),
        ("quantity", json!(0)),
        ("quantity", json!(65536)),
        (
            "tooltipSource",
            json!({"info": {"item_index": 27}, "userItem": {"unique_id": 42, "item_index": 27}}),
        ),
    ] {
        let mut invalid = inventory.clone();
        invalid["items"][0][field] = value;
        let mut current = social.clone();
        assert!(
            !observe_own_offer(&snapshot, &invalid, &mut current),
            "{field}"
        );
        assert_eq!(current, social);
    }

    let mut invalid = snapshot.clone();
    invalid["stage5Systems"]["trade"]["offeredUniqueIds"] = json!({});
    let mut current = social.clone();
    assert!(!observe_own_offer(&invalid, &inventory, &mut current));
    assert_eq!(current, social);
}

#[test]
fn legacy_offer_rejects_duplicate_identity_in_inventory_or_quote() {
    let (mut snapshot, mut inventory, mut social) = legacy_offer();
    let before = social.clone();
    let mut duplicate = inventory["items"][0].clone();
    duplicate["slot"] = json!(1);
    inventory["items"].as_array_mut().unwrap().push(duplicate);
    assert!(!observe_own_offer(&snapshot, &inventory, &mut social));
    assert_eq!(social, before);

    inventory["items"][1]["uniqueId"] = json!(42);
    inventory["items"][1]["tooltipSource"]["userItem"]["unique_id"] = json!(42);
    let offer = &mut snapshot["stage5Systems"]["trade"];
    offer["offeredSlots"]["4"] = json!(1);
    offer["offeredUniqueIds"]["4"] = json!(0);
    assert!(!observe_own_offer(&snapshot, &inventory, &mut social));
    assert_eq!(social, before);
}

fn detached_offer() -> (Value, Value, SocialModel) {
    let (mut snapshot, mut inventory, social) = legacy_offer();
    snapshot["inventoryItems"][0]["uniqueId"] = json!(19);
    inventory["items"][0]["uniqueId"] = json!(19);
    inventory["items"][0]["tooltipSource"]["userItem"]["unique_id"] = json!(19);
    let mut slots = vec![Value::Null; MAX_TRADE_ITEMS];
    slots[3] = json!({
        "slot": 3, "uniqueId": 0, "container": "bag1",
        "key": "potion", "name": "Potion", "icon": 123, "quantity": 5,
        "tooltipSource": {
            "info": {"item_index": 27, "image": 123},
            "userItem": {"unique_id": 0, "item_index": 27, "count": 5}
        }
    });
    snapshot["stage5Systems"]["trade"]["ownItems"] = json!(slots);
    (snapshot, inventory, social)
}

#[test]
fn detached_offer_renders_exact_slots_after_historical_origin_is_reused() {
    let (mut snapshot, inventory, mut social) = detached_offer();
    let offer = &mut snapshot["stage5Systems"]["trade"];
    offer["offeredSlots"]["8"] = json!(0);
    offer["offeredUniqueIds"]["8"] = json!(33);
    offer["ownItems"][8] = json!({
        "slot": 8, "uniqueId": 33, "container": "bag1",
        "key": "candle", "name": "Candle", "icon": 130, "quantity": 1
    });
    offer["locked"] = json!(true);

    assert!(observe_own_offer(&snapshot, &inventory, &mut social));
    assert_eq!(social.trade.my_items.iter().flatten().count(), 2);
    let first = social.trade.my_items[3].as_ref().unwrap();
    assert_eq!(first.unique_id, Some(0));
    assert_eq!(first.item_index, Some(27));
    assert_eq!(first.count, 5);
    assert_eq!(
        social.trade.my_items[8].as_ref().unwrap().unique_id,
        Some(33)
    );
    assert_eq!(social.trade.my_gold, 7);
    assert!(social.trade.my_confirmed);
    assert!(!observe_own_offer(&snapshot, &inventory, &mut social));

    let offer = &mut snapshot["stage5Systems"]["trade"];
    offer["offeredSlots"] = json!({});
    offer["offeredUniqueIds"] = json!({});
    offer["ownItems"] = json!(vec![Value::Null; MAX_TRADE_ITEMS]);
    offer["locked"] = json!(false);
    assert!(observe_own_offer(&snapshot, &inventory, &mut social));
    assert!(social.trade.my_items.iter().all(Option::is_none));
    assert!(!social.trade.my_confirmed);
    assert_eq!(social.trade.state, "open");
}

#[test]
fn present_null_or_malformed_detached_array_never_falls_back_to_bag() {
    let (mut snapshot, inventory, social) = legacy_offer();
    for items in [
        Value::Null,
        json!({}),
        json!([]),
        json!(vec![Value::Null; MAX_TRADE_ITEMS - 1]),
        json!(vec![Value::Null; MAX_TRADE_ITEMS]),
        json!(vec![Value::Null; MAX_TRADE_ITEMS + 1]),
    ] {
        snapshot["stage5Systems"]["trade"]["ownItems"] = items;
        let mut current = social.clone();
        assert!(!observe_own_offer(&snapshot, &inventory, &mut current));
        assert_eq!(current, social);
    }
}

#[test]
fn malformed_detached_items_leave_the_last_valid_offer_intact() {
    let (snapshot, inventory, mut social) = detached_offer();
    assert!(observe_own_offer(&snapshot, &inventory, &mut social));
    for (field, bad) in [
        ("slot", json!(8)),
        ("uniqueId", json!(19)),
        ("uniqueId", Value::Null),
        ("container", json!("bag2")),
        ("quantity", json!(0)),
        ("quantity", json!(65536)),
        ("key", json!("")),
        ("name", json!("")),
        ("icon", json!("123")),
        ("tooltipSource", json!({})),
        ("tooltipSource", json!({"info": {"item_index": 27}})),
        (
            "tooltipSource",
            json!({"info": {"item_index": 27}, "userItem": {"item_index": 27, "count": 5}}),
        ),
        (
            "tooltipSource",
            json!({"info": {"item_index": 27}, "userItem": {"unique_id": 19, "item_index": 27, "count": 5}}),
        ),
        (
            "tooltipSource",
            json!({"info": {"item_index": 27}, "userItem": {"unique_id": 0, "item_index": 19, "count": 5}}),
        ),
        (
            "tooltipSource",
            json!({"info": {"item_index": 27}, "userItem": {"unique_id": 0, "item_index": 27, "count": 4}}),
        ),
    ] {
        let mut invalid = snapshot.clone();
        let offer = &mut invalid["stage5Systems"]["trade"];
        offer["offeredGold"] = json!(9);
        offer["ownItems"][3][field] = bad;
        let mut current = social.clone();
        assert!(
            !observe_own_offer(&invalid, &inventory, &mut current),
            "{field}"
        );
        assert_eq!(current, social);
    }
}

#[test]
fn detached_offer_rejects_extra_slots_duplicate_ids_and_simultaneous_bag_custody() {
    let (snapshot, inventory, social) = detached_offer();
    let mut wrong = snapshot.clone();
    let offer = &mut wrong["stage5Systems"]["trade"];
    offer["ownItems"][8] = offer["ownItems"][3].clone();
    offer["ownItems"][8]["slot"] = json!(8);
    let mut current = social.clone();
    assert!(!observe_own_offer(&wrong, &inventory, &mut current));
    assert_eq!(current, social);
    let offer = &mut wrong["stage5Systems"]["trade"];
    offer["offeredSlots"]["8"] = json!(0);
    offer["offeredUniqueIds"]["8"] = json!(0);
    assert!(!observe_own_offer(&wrong, &inventory, &mut current));
    assert_eq!(current, social);

    let (_, bag_with_same_uid, _) = legacy_offer();
    assert!(!observe_own_offer(
        &snapshot,
        &bag_with_same_uid,
        &mut current
    ));
    assert_eq!(current, social);
}
