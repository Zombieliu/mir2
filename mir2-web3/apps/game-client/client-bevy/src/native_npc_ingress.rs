//! Windows3d735745f NPC packet/catalogue projection shared with Android.
//! Hosts own authentication, scene lifetime, FIFO/backpressure and Exit gates.
//! No purchase, repair, quest, wallet or custody rules live in this adapter.
use crate::{
    native_inventory_ingress::{
        native_tooltip_source_for_user_item, native_user_item_icon, NativeItemFrameGeometry,
        NativeItemLibrary,
    },
    native_player_ingress::NativeUiPlayerCursor,
    shop::{NpcShopServiceMode, NpcShopServiceSignal},
};
use serde_json::{json, Value};

/// Only a real, well-shaped NPCResponse page retires service children.
/// Passive snapshots do not synthesize this ordered packet boundary.
pub fn npc_shop_service_from_packet(packet: &str, payload: &Value) -> Option<NpcShopServiceSignal> {
    let signal = match packet {
        "NPCResponse"
            if payload
                .get("page")
                .and_then(Value::as_array)
                .is_some_and(|page| page.iter().all(Value::is_string)) =>
        {
            NpcShopServiceSignal::default()
        }
        "NPCGoods" | "NPCPearlGoods" => NpcShopServiceSignal {
            mode: NpcShopServiceMode::Buy,
            repair_rate: None,
        },
        "NPCSell" => NpcShopServiceSignal {
            mode: NpcShopServiceMode::Sell,
            repair_rate: None,
        },
        "NPCRepair" | "NPCSRepair" => NpcShopServiceSignal {
            mode: if packet == "NPCRepair" {
                NpcShopServiceMode::Repair
            } else {
                NpcShopServiceMode::SpecialRepair
            },
            repair_rate: payload
                .get("rate")
                .and_then(Value::as_f64)
                .filter(|rate| rate.is_finite() && *rate >= 0.0)
                .map(|rate| rate as f32),
        },
        _ => return None,
    };
    signal.is_valid().then_some(signal)
}

pub fn transform_shop_model_from_packet(
    payload: &Value,
    cursor: &NativeUiPlayerCursor,
    mut geometry: impl FnMut(NativeItemLibrary, u16) -> Option<NativeItemFrameGeometry>,
) -> Value {
    let goods: Vec<Value> = payload
        .get("list")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .enumerate()
                .filter_map(|(slot, item)| shop_good_json(item, slot, cursor, &mut geometry))
                .collect()
        })
        .unwrap_or_default();
    json!({
        "goods": goods,
        "selected_id": Value::Null,
        "hide_added_stats": if ["hideAddedStats", "hide_added_stats", "shopHideAddedStats", "shop_hide_added_stats"].iter().any(|key| payload.get(*key).is_some()) { shop_hide_added_stats(payload) } else { cursor.npc_shop_hide_added_stats },
        "selected_bag_slot_for_sell": Value::Null,
        "selected_bag_slot_for_repair": Value::Null,
    })
}

pub fn transform_shop_model_from_snapshot(
    payload: &Value,
    cursor: &NativeUiPlayerCursor,
    mut geometry: impl FnMut(NativeItemLibrary, u16) -> Option<NativeItemFrameGeometry>,
) -> Value {
    let list = ["shopGoods", "shop_goods", "npcGoods", "npc_goods"]
        .iter()
        .find_map(|key| payload.get(*key))
        .and_then(Value::as_array);
    let goods: Vec<Value> = list
        .map(|list| {
            list.iter()
                .enumerate()
                .filter_map(|(slot, item)| shop_good_json(item, slot, cursor, &mut geometry))
                .collect()
        })
        .unwrap_or_default();
    json!({
        "goods": goods,
        "selected_id": Value::Null,
        "hide_added_stats": shop_hide_added_stats(payload),
        "selected_bag_slot_for_sell": Value::Null,
        "selected_bag_slot_for_repair": Value::Null,
    })
}

pub fn shop_good_json(
    item: &Value,
    fallback: usize,
    cursor: &NativeUiPlayerCursor,
    mut geometry: impl FnMut(NativeItemLibrary, u16) -> Option<NativeItemFrameGeometry>,
) -> Option<Value> {
    let id = value_u64(
        item.get("uniqueId")
            .or_else(|| item.get("unique_id"))
            .or_else(|| item.get("id"))
            .or_else(|| item.get("itemIndex"))
            .or_else(|| item.get("item_index")),
    )?;
    let tooltip_source = item
        .get("tooltipSource")
        .cloned()
        .or_else(|| native_tooltip_source_for_user_item(item, cursor).map(|source| json!(source)));
    let count = value_u32(item.get("count").or_else(|| item.get("quantity"))).unwrap_or(1);
    let icon = native_user_item_icon(item, count).or_else(|| {
        value_u32(item.get("icon"))
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| *value != 0)
    });
    let icon_geometry = icon.and_then(|index| geometry(NativeItemLibrary::Items, index));
    Some(json!({
        "unique_id": id,
        "use_pearls": cursor.npc_shop_uses_pearls,
        "name": value_string(item.get("name")).unwrap_or_else(|| format!("Item #{id}")),
        "price": value_u32(item.get("price")).unwrap_or_default(),
        "count": u16::try_from(count).unwrap_or(1),
        "stock": value_i32(item.get("stock")).unwrap_or(-1),
        "panel_type": value_u32(item.get("panelType").or_else(|| item.get("panel_type")))
            .and_then(|value| u8::try_from(value).ok()).unwrap_or(u8::try_from(fallback).unwrap_or_default()),
        "icon": icon.unwrap_or_default(),
        "icon_width": icon_geometry.map(|frame| frame.width).unwrap_or_default(),
        "icon_height": icon_geometry.map(|frame| frame.height).unwrap_or_default(),
        "description": value_string(item.get("description")).unwrap_or_default(),
        "tooltip_source": tooltip_source,
    }))
}

pub fn shop_hide_added_stats(payload: &Value) -> bool {
    [
        "hideAddedStats",
        "hide_added_stats",
        "shopHideAddedStats",
        "shop_hide_added_stats",
    ]
    .iter()
    .find_map(|key| payload.get(*key))
    .and_then(Value::as_bool)
    .unwrap_or(false)
}

/// Goods and currency are one projection. Invalid input must not change the
/// currency of the old catalogue; Pearl packets retain HideAddedStats.
pub fn transform_npc_catalog_packet(
    packet: &str,
    payload: &Value,
    cursor: &mut NativeUiPlayerCursor,
    geometry: impl FnMut(NativeItemLibrary, u16) -> Option<NativeItemFrameGeometry>,
) -> Option<Value> {
    if !matches!(packet, "NPCGoods" | "NPCPearlGoods") {
        return None;
    }
    let previous = cursor.npc_shop_uses_pearls;
    cursor.npc_shop_uses_pearls = packet == "NPCPearlGoods";
    let mut model = try_transform_shop_model_from_packet(payload, cursor, geometry);
    if let Some(model) = model.as_mut() {
        if packet == "NPCPearlGoods" {
            model["hide_added_stats"] = json!(cursor.npc_shop_hide_added_stats);
        } else {
            cursor.npc_shop_hide_added_stats = shop_hide_added_stats(payload);
        }
    } else {
        cursor.npc_shop_uses_pearls = previous;
    }
    model
}

pub fn try_transform_shop_model_from_packet(
    payload: &Value,
    cursor: &NativeUiPlayerCursor,
    mut geometry: impl FnMut(NativeItemLibrary, u16) -> Option<NativeItemFrameGeometry>,
) -> Option<Value> {
    let list = payload.get("list")?.as_array()?;
    if list
        .iter()
        .enumerate()
        .any(|(slot, item)| shop_good_json(item, slot, cursor, &mut geometry).is_none())
    {
        return None;
    }
    Some(transform_shop_model_from_packet(payload, cursor, geometry))
}

pub fn payload_has_valid_shop_array(payload: &Value) -> bool {
    ["shopGoods", "shop_goods", "npcGoods", "npc_goods"]
        .iter()
        .find_map(|key| payload.get(*key))
        .is_some_and(Value::is_array)
}

fn value_u32(value: Option<&Value>) -> Option<u32> {
    value.and_then(|value| {
        value
            .as_u64()
            .and_then(|number| u32::try_from(number).ok())
            .or_else(|| value.as_str()?.parse().ok())
    })
}
fn value_i32(value: Option<&Value>) -> Option<i32> {
    value.and_then(|value| {
        value
            .as_i64()
            .and_then(|number| i32::try_from(number).ok())
            .or_else(|| value.as_str()?.parse().ok())
    })
}
fn value_u64(value: Option<&Value>) -> Option<u64> {
    value.and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()))
}
fn value_string(value: Option<&Value>) -> Option<String> {
    value.and_then(|value| match value {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shop::ShopModel;

    #[test]
    fn only_a_real_npc_response_is_a_close_boundary() {
        for page in [json!([]), json!(["Welcome", "<Back/@main>"])] {
            assert_eq!(
                npc_shop_service_from_packet("NPCResponse", &json!({"page":page})),
                Some(NpcShopServiceSignal::default())
            );
        }
        for payload in [
            Value::Null,
            json!({}),
            json!({"page":null}),
            json!({"page":""}),
            json!({"page":[42]}),
        ] {
            assert!(npc_shop_service_from_packet("NPCResponse", &payload).is_none());
        }
        assert!(
            npc_shop_service_from_packet("worldSnapshot", &json!({"activeNpcDialog":null}))
                .is_none()
        );
    }

    #[test]
    fn ordinary_goods_sell_and_repair_keep_independent_capabilities() {
        let mut shop = ShopModel::default();
        for (packet, payload) in [("NPCGoods", json!({"list":[]})), ("NPCSell", Value::Null)] {
            assert!(
                shop.apply_service_signal(npc_shop_service_from_packet(packet, &payload).unwrap())
            );
        }
        assert!(shop.allows_buy() && shop.allows_sell());
        assert!(shop.apply_service_signal(
            npc_shop_service_from_packet("NPCRepair", &json!({"rate":1.5})).unwrap()
        ));
        assert!(shop.allows_repair());
        assert!(!shop.allows_buy() && !shop.allows_sell());
        assert_eq!(shop.repair_rate, Some(1.5));
        for payload in [
            json!({}),
            json!({"rate":-1}),
            json!({"rate":"1.5"}),
            json!({"rate":1e100}),
        ] {
            assert!(npc_shop_service_from_packet("NPCSRepair", &payload).is_none());
        }
    }

    #[test]
    fn catalogue_keeps_exact_identity_original_geometry_and_currency() {
        let mut cursor = NativeUiPlayerCursor::default();
        transform_npc_catalog_packet(
            "NPCGoods",
            &json!({"list":[],"hideAddedStats":true}),
            &mut cursor,
            |_, _| None,
        )
        .unwrap();
        let payload = json!({"list":[{"uniqueId":"9007199254740993","name":"Potion","price":50,"count":20,"icon":7}]});
        let model = transform_npc_catalog_packet(
            "NPCPearlGoods",
            &payload,
            &mut cursor,
            |library, index| {
                assert_eq!((library, index), (NativeItemLibrary::Items, 7));
                Some(NativeItemFrameGeometry {
                    width: 36,
                    height: 26,
                    ..Default::default()
                })
            },
        )
        .unwrap();
        let shop: ShopModel = serde_json::from_value(model).unwrap();
        assert_eq!(shop.goods[0].unique_id, 9007199254740993);
        assert_eq!(
            (shop.goods[0].icon_width, shop.goods[0].icon_height),
            (36, 26)
        );
        assert!(shop.goods[0].use_pearls && shop.hide_added_stats);
        assert!(transform_npc_catalog_packet(
            "NPCGoods",
            &json!({"list":[{}]}),
            &mut cursor,
            |_, _| None
        )
        .is_none());
        assert!(cursor.npc_shop_uses_pearls && cursor.npc_shop_hide_added_stats);
        let gold =
            transform_npc_catalog_packet("NPCGoods", &payload, &mut cursor, |_, _| None).unwrap();
        assert_eq!(gold["goods"][0]["use_pearls"], false);
        assert!(!cursor.npc_shop_hide_added_stats);
        assert!(
            transform_npc_catalog_packet("GameShopInfo", &payload, &mut cursor, |_, _| None)
                .is_none()
        );
    }

    #[test]
    fn snapshot_catalogue_is_passive_and_absent_geometry_is_not_guessed() {
        let cursor = NativeUiPlayerCursor {
            npc_shop_uses_pearls: true,
            ..Default::default()
        };
        for field in ["shopGoods", "shop_goods", "npcGoods", "npc_goods"] {
            let mut payload = json!({});
            payload[field] = json!([{"unique_id":10,"name":"Blade","icon":7}]);
            assert!(payload_has_valid_shop_array(&payload));
            let shop: ShopModel = serde_json::from_value(transform_shop_model_from_snapshot(
                &payload,
                &cursor,
                |_, _| None,
            ))
            .unwrap();
            assert_eq!(shop.goods[0].icon_width, 0);
            assert!(shop.goods[0].use_pearls);
            assert_eq!(shop.service_mode, NpcShopServiceMode::Closed);
            assert!(!shop.allows_buy() && !shop.allows_sell());
        }
    }
}
