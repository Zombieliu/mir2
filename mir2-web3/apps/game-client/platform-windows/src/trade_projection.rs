//! Thin Windows delegate to the shared read-only own-offer projection.
//! No wallet, inventory, reservation, settlement or wire contract is changed.

use mir2_client_bevy::social::SocialModel;
#[cfg(test)]
use mir2_client_bevy::social::TradeItemModel;
use serde_json::Value;

pub(super) fn observe_own_offer(
    snapshot: &Value,
    inventory_json: &Value,
    social: &mut SocialModel,
) -> bool {
    mir2_client_bevy::native_trade_ingress::observe_own_offer(snapshot, inventory_json, social)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> (Value, Value, SocialModel) {
        let snapshot = json!({
            "inventoryCapacity":86, "gold":500,
            "inventoryItems":[
                {"uniqueId":7101,"key":"same","name":"Same name","container":"bag2","slot":2,"quantity":201,
                 "tooltipSource":{"info":{"item_index":27,"image":3662,"item_type":8,"shape":0,"stack_size":500},
                 "userItem":{"unique_id":7101,"item_index":27,"count":201,"identified":true}}},
                {"uniqueId":7102,"key":"same","name":"Same name","container":"bag1","slot":7,"quantity":1}
            ],
            "stage5Systems":{"trade":{
                "settlementNonce":"offer-a","partner":"Guest","offeredSlots":{"1":42,"8":7},
                "offeredUniqueIds":{"1":7101,"8":7102},"offeredGold":125,
                "offeredCurrency":"gold","locked":true,"completed":false
            }}
        });
        let inventory = super::super::transform_inventory_model(&snapshot);
        let mut social = SocialModel::default();
        assert!(social.apply_packet("TradeAccept", &json!({"name":"Guest"})));
        social.trade.partner_gold = 17;
        social.trade.partner_items = vec![
            None,
            Some(TradeItemModel {
                count: 9,
                ..Default::default()
            }),
        ];
        (snapshot, inventory, social)
    }

    #[test]
    fn own_trade_snapshot_keeps_exact_slots_instances_metadata_and_guest_state() {
        let (snapshot, inventory, mut social) = fixture();
        let before = social.clone();
        assert!(observe_own_offer(&snapshot, &inventory, &mut social));
        assert_eq!(social.trade.my_items.len(), 10);
        for i in 0..10 {
            assert_eq!(social.trade.my_items[i].is_some(), [1, 8].contains(&i));
        }
        let first = social.trade.my_items[1].as_ref().unwrap();
        assert_eq!(first.unique_id, Some(7101));
        assert_eq!(first.item_index, Some(27));
        assert_eq!(first.count, 201);
        assert_eq!(
            first
                .tooltip_source
                .as_ref()
                .unwrap()
                .user_item_image(u32::from(first.count)),
            3661
        );
        assert_eq!(
            social.trade.my_items[8].as_ref().unwrap().unique_id,
            Some(7102)
        );
        assert_eq!(social.trade.my_gold, 125);
        assert!(social.trade.my_confirmed);
        assert_eq!(social.trade.partner_gold, before.trade.partner_gold);
        assert_eq!(social.trade.partner_items, before.trade.partner_items);
        assert_eq!(social.trade.my_offer_nonce.as_deref(), Some("offer-a"));
        assert_eq!(social.trade.event_revision, before.trade.event_revision + 1);
        assert_eq!(inventory["gold"], 500);
        assert_eq!(inventory["items"].as_array().unwrap().len(), 2);
        let settled = social.clone();
        assert!(!observe_own_offer(&snapshot, &inventory, &mut social));
        assert_eq!(
            social, settled,
            "equal snapshots are not new acknowledgements"
        );
    }

    #[test]
    fn own_trade_snapshot_never_accepts_or_reopens_an_exchange() {
        for state in ["", "requested", "completed"] {
            let (snapshot, inventory, mut social) = fixture();
            social.trade.state = state.into();
            let before = social.clone();
            assert!(!observe_own_offer(&snapshot, &inventory, &mut social));
            assert_eq!(social, before);
        }
    }

    #[test]
    fn own_trade_snapshot_rejects_missing_malformed_wrong_owner_and_currency() {
        let (snapshot, inventory, social) = fixture();
        let mut controls = vec![json!({}), json!({"stage5Systems":{"trade":null}})];
        for (key, value) in [
            ("partner", json!("Other")),
            ("settlementNonce", json!("")),
            ("offeredCurrency", json!("bichon")),
            ("completed", json!(true)),
            ("locked", json!("true")),
            ("offeredGold", json!(4_294_967_296u64)),
            ("offeredSlots", json!({"10":42})),
            ("offeredUniqueIds", json!({"1":7101})),
        ] {
            let mut wrong = snapshot.clone();
            wrong["stage5Systems"]["trade"][key] = value;
            controls.push(wrong);
        }
        let mut no_items = snapshot.clone();
        no_items.as_object_mut().unwrap().remove("inventoryItems");
        controls.push(no_items);
        for wrong in controls {
            let mut current = social.clone();
            assert!(
                !observe_own_offer(&wrong, &inventory, &mut current),
                "{wrong}"
            );
            assert_eq!(current, social);
        }
        let mut current = social.clone();
        current.trade.my_offer_nonce = Some("different-exchange".into());
        let before = current.clone();
        assert!(!observe_own_offer(&snapshot, &inventory, &mut current));
        assert_eq!(current, before);
    }

    #[test]
    fn own_trade_snapshot_rejects_swapped_duplicate_or_nonbag_items_without_guessing_names() {
        let (snapshot, inventory, social) = fixture();
        let mut controls = Vec::new();
        for (key, value) in [
            ("uniqueId", json!(9999)),
            ("slot", json!(41)),
            ("container", json!(1)),
            ("quantity", json!(65536)),
            (
                "tooltipSource",
                json!({"info":{"item_index":27},"userItem":{"unique_id":9999,"item_index":27}}),
            ),
        ] {
            let mut wrong = inventory.clone();
            wrong["items"][0][key] = value;
            controls.push(wrong);
        }
        let mut duplicate = inventory.clone();
        let extra = duplicate["items"][0].clone();
        duplicate["items"].as_array_mut().unwrap().push(extra);
        controls.push(duplicate);
        for wrong in controls {
            let mut current = social.clone();
            assert!(!observe_own_offer(&snapshot, &wrong, &mut current));
            assert_eq!(current, social);
        }
        for (slots, ids) in [
            (json!({"1":42,"8":42}), json!({"1":7101,"8":7101})),
            (json!({"1":42,"8":7}), json!({"1":7101,"8":7101})),
            (json!({"1":42,"8":7}), json!({"1":0,"8":7102})),
        ] {
            let mut wrong = snapshot.clone();
            wrong["stage5Systems"]["trade"]["offeredSlots"] = slots;
            wrong["stage5Systems"]["trade"]["offeredUniqueIds"] = ids;
            let mut current = social.clone();
            assert!(!observe_own_offer(&wrong, &inventory, &mut current));
            assert_eq!(current, social);
        }
    }

    #[test]
    fn own_trade_snapshot_clears_only_explicit_own_offer_and_does_not_settle() {
        let (mut snapshot, inventory, mut social) = fixture();
        assert!(observe_own_offer(&snapshot, &inventory, &mut social));
        for key in ["offeredSlots", "offeredUniqueIds"] {
            snapshot["stage5Systems"]["trade"][key] = json!({});
        }
        snapshot["stage5Systems"]["trade"]["offeredGold"] = json!(0);
        snapshot["stage5Systems"]["trade"]["locked"] = json!(false);
        assert!(observe_own_offer(&snapshot, &inventory, &mut social));
        assert!(social.trade.my_items.iter().all(Option::is_none));
        assert_eq!(social.trade.my_gold, 0);
        assert!(!social.trade.my_confirmed);
        assert_eq!(social.trade.state, "open");
        assert_eq!(social.last_event.as_ref().unwrap().success, None);
        assert_eq!(social.trade.partner_gold, 17);
    }
}
