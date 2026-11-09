use super::observe_own_offer;
use crate::social::SocialModel;
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
