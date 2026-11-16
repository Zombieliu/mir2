//! The browser and Native use the same bounded, whole-batch catalog rules.

pub(super) use mir2_protocol::catalog_envelope::decode_validated_catalog_batch;
#[cfg(test)]
use serde_json::Value;

#[cfg(test)]
pub(super) mod tests {
    use super::super::{process_connected_server_frame, GameShopReceiptGate};
    use super::*;
    use crate::native_protocol::parse_inbound_event;
    use mir2_client_bevy::game_shop::{GameShopReceipt, GameShopRequest};
    use mir2_protocol::catalog_transport::encode_catalog_batch;
    use serde_json::json;
    use tokio_tungstenite::tungstenite::Message;

    pub(crate) fn catalog_fixture_texts() -> Vec<String> {
        let item = json!({"index":7,"name":"药品 Potion","item_type":0,"grade":0,"required_type":0,"required_class":0,
            "required_gender":0,"item_set":0,"shape":0,"weight":1,"light":0,"required_amount":0,"image":1,"durability":0,
            "stack_size":99,"price":1,"start_item":false,"effect":0,"need_identify":false,"show_group_pickup":false,
            "class_based":false,"level_based":false,"can_mine":false,"global_drop_notify":false,"bind":0,"unique":0,
            "random_stats_id":0,"can_fast_run":false,"can_awakening":false,"slots":0,"stats":[],"tooltip":null});
        let user_item = json!({"unique_id":0,"item_index":7,"current_dura":0,"max_dura":0,"count":1,"soul_bound_id":0,
            "identified":true,"cursed":false,"slots":[],"gem_count":0,"added_stats":[],"awake_type":0,"awake_values":[],
            "refined_value":0,"refine_added":0,"refine_success_chance":0,"wedding_ring":0,"expire_info":null,
            "rental_information":null,"is_shop_item":false,"sealed_info":null,"gm_made":false});
        let info = json!({"index":3,"npc_index":1,"name":"骷髅试炼","group":"","description":["ASCII and 中文"],
            "task_description":[],"return_description":[],"completion_description":[],"min_level_needed":1,"max_level_needed":30,
            "quest_needed":0,"class_needed":0,"quest_type":0,"time_limit_in_seconds":0,"reward_gold":0,"reward_exp":0,"reward_credit":0,
            "rewards_fixed_item":[],"rewards_select_item":[],"finish_npc_index":1});
        vec![
            json!({"type":"packet","packet":"NewItemInfo","payload":{"info":item}}).to_string(),
            json!({"type":"packet","packet":"NewRecipeInfo","payload":{"info":{"gold":1,"chance":100,"item":user_item,"tools":[],"ingredients":[]}}}).to_string(),
            json!({"type":"packet","packet":"GameShopInfo","payload":{"item":{"item_index":7,"g_index":8,"info":item,
                "gold_price":1,"credit_price":0,"count":1,"class":"Warrior","category":"Potion","stock":3,"i_stock":true,
                "deal":false,"top_item":false,"date_binary_datetime":0,"can_buy_credit":false,"can_buy_gold":true},"stockLevel":3}}).to_string(),
            json!({"type":"packet","packet":"NewQuestInfo","payload":{"info":info,"id":3,"name":"骷髅试炼","minLevelNeeded":1,"descriptionLines":["ASCII and 中文"]}}).to_string(),
        ]
    }

    #[test]
    fn native_catalog_all_four_shapes_keep_exact_order_and_original_text() {
        let texts = catalog_fixture_texts();
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        assert_eq!(
            decode_validated_catalog_batch(&encode_catalog_batch(&refs).unwrap()).unwrap(),
            texts
        );
    }

    #[test]
    fn native_catalog_rejects_non_catalog_bad_shapes_and_invalid_later_envelopes() {
        let good = catalog_fixture_texts().remove(0);
        for invalid in [
            r#"{"type":"resumeCredential","packet":"NewItemInfo","payload":{}}"#,
            r#"{"type":"packet","packet":"UserLocation","payload":{"x":1,"y":2}}"#,
            r#"{"type":"packet","packet":"NewItemInfo","payload":[]}"#,
            r#"{"type":"packet","packet":"NewItemInfo","payload":{"info":{}}}"#,
            r#"{"type":"packet","packet":"NewQuestInfo","payload":{"info":{}}}"#,
            r#"{"type":"packet","packet":"GameShopInfo","payload":{"item":{},"stockLevel":null}}"#,
            "{private-invalid-json",
        ] {
            let encoded = encode_catalog_batch(&[&good, invalid]).unwrap();
            assert_eq!(
                decode_validated_catalog_batch(&encoded).unwrap_err(),
                "invalid catalog packet envelope"
            );
        }
        let mut quest: Value = serde_json::from_str(&catalog_fixture_texts()[3]).unwrap();
        quest["payload"]["id"] = json!(999);
        assert!(decode_validated_catalog_batch(
            &encode_catalog_batch(&[&quest.to_string()]).unwrap()
        ).is_err());
    }

    #[test]
    fn native_catalog_dispatch_validates_whole_batch_before_any_callback() {
        let texts = catalog_fixture_texts();
        let invalid = r#"{"type":"packet","packet":"UserLocation","payload":{"x":99,"y":99}}"#;
        let frame = Message::Binary(encode_catalog_batch(&[&texts[0], invalid]).unwrap().into());
        let mut gate = GameShopReceiptGate::default();
        let mut applied = Vec::new();
        let result = process_connected_server_frame(
            &frame,
            &mut gate,
            |text, _| {
                applied.push(text.to_owned());
                Ok(())
            },
            || panic!("no purchase was pending"),
        );
        assert!(result.is_err());
        assert!(
            applied.is_empty(),
            "the valid first envelope must not leak before a rejected second one"
        );
    }

    #[test]
    fn native_catalog_dispatch_keeps_order_and_legacy_text_path() {
        let texts = catalog_fixture_texts();
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        let frame = Message::Binary(encode_catalog_batch(&refs).unwrap().into());
        let mut gate = GameShopReceiptGate::default();
        let mut applied = Vec::new();
        let events = process_connected_server_frame(
            &frame,
            &mut gate,
            |text, _| {
                applied.push(text.to_owned());
                parse_inbound_event(text).map_err(|error| error.to_string())
            },
            || panic!("valid catalog must not reset"),
        )
        .unwrap();
        assert_eq!(applied, texts);
        assert_eq!(events.len(), 4);

        // An old server can continue to send any legacy text envelope. The
        // binary catalog allowlist must never filter the ordinary text path.
        let legacy = r#" {"type":"packet","packet":"UserLocation","payload":{"x":11,"y":22,"direction":"Up"}} "#;
        let events = process_connected_server_frame(
            &Message::Text(legacy.into()),
            &mut gate,
            |text, _| {
                assert_eq!(text, legacy);
                parse_inbound_event(text).map_err(|error| error.to_string())
            },
            || panic!("legacy text must not reset"),
        )
        .unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn native_catalog_invalid_binary_resets_only_written_pending_purchase_once() {
        let request = GameShopRequest::new("gs-catalog".to_owned(), 31, 1, 1).unwrap();
        let mut gate = GameShopReceiptGate::default();
        assert!(gate.record_successful_send(request));
        let invalid = Message::Binary(vec![0; 17].into());
        let mut resets = 0;
        for _ in 0..2 {
            assert!(process_connected_server_frame::<(), _, _>(
                &invalid,
                &mut gate,
                |_, _| { panic!("malformed binary must not be applied") },
                || {
                    resets += 1;
                    true
                }
            )
            .is_err());
        }
        assert_eq!(resets, 1);
        assert!(gate.pending.is_none());

        let receipt = GameShopReceipt {
            protocol: "nativeGameShopReceiptV1".to_owned(),
            request_id: "gs-exact-catalog".to_owned(),
            success: true,
            g_index: 31,
            quantity: 1,
            price_type: 1,
            recipient_name: None,
            new_stock_level: Some(9),
            mail_id: Some(77),
            code: None,
        };
        gate.reserved = Some(receipt.clone());
        assert!(process_connected_server_frame::<(), _, _>(
            &invalid,
            &mut gate,
            |_, _| { panic!("malformed binary must not be applied") },
            || {
                resets += 1;
                true
            }
        )
        .is_err());
        assert_eq!(
            resets, 1,
            "an accepted exact receipt must not become unknown"
        );
        assert_eq!(gate.reserved, Some(receipt));
    }
}
