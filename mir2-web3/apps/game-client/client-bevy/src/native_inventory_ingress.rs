//! Shared Windows/Android inventory wire projection. No item mutations,
//! gameplay rules, operation matching or filesystem/platform dependencies.
//! Geometry is supplied by the host from the approved original asset pack.
use crate::inventory::{CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel};
use crate::native_player_ingress::NativeUiPlayerCursor;
use crate::pending_operations::InventoryOperationAck;
use mir2_game_data::{crystal_item_manifest, crystal_real_item_for_player, CrystalItemTemplate};
use mir2_protocol::MirClass;
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeItemLibrary {
    Items,
    StateItem,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeItemFrameGeometry {
    pub width: u16,
    pub height: u16,
    pub x: i32,
    pub y: i32,
}

/// Public ACK parser; callers must separately enforce their packet allowlist.
/// In particular, frozen Windows routes seven kinds but not DeleteItem.
pub fn project_native_inventory_operation_ack(
    packet: &str,
    payload: &Value,
) -> Option<InventoryOperationAck> {
    if packet == "DeleteItem" {
        // Crystal's S.DeleteItem receipt carries the exact instance/count but
        // no Success field. Receiving the packet itself is the authoritative
        // success acknowledgement.
        return Some(InventoryOperationAck::Delete {
            unique_id: value_u64(payload.get("uniqueId"))?,
            count: value_u32(payload.get("count")).and_then(|value| u16::try_from(value).ok())?,
            success: true,
        });
    }
    let success = payload.get("success")?.as_bool()?;
    match packet {
        "EquipItem" => Some(InventoryOperationAck::Equip {
            grid: payload.get("grid")?.as_str()?.to_owned(),
            unique_id: value_u64(payload.get("uniqueId"))?,
            to: value_i32(payload.get("to"))?,
            success,
        }),
        "RemoveItem" => Some(InventoryOperationAck::Remove {
            grid: payload.get("grid")?.as_str()?.to_owned(),
            unique_id: value_u64(payload.get("uniqueId"))?,
            to: value_i32(payload.get("to"))?,
            success,
        }),
        "DropItem" => Some(InventoryOperationAck::Drop {
            unique_id: value_u64(payload.get("uniqueId"))?,
            count: value_u32(payload.get("count")).and_then(|value| u16::try_from(value).ok())?,
            hero_inventory: payload.get("heroInventory")?.as_bool()?,
            success,
        }),
        "MoveItem" => Some(InventoryOperationAck::Move {
            grid: payload.get("grid")?.as_str()?.to_owned(),
            from: value_i32(payload.get("from"))?,
            to: value_i32(payload.get("to"))?,
            success,
        }),
        "MergeItem" => Some(InventoryOperationAck::Merge {
            grid_from: payload.get("gridFrom")?.as_str()?.to_owned(),
            grid_to: payload.get("gridTo")?.as_str()?.to_owned(),
            id_from: value_u64(payload.get("idFrom"))?,
            id_to: value_u64(payload.get("idTo"))?,
            success,
        }),
        "SplitItem1" => Some(InventoryOperationAck::Split {
            grid: payload.get("grid")?.as_str()?.to_owned(),
            unique_id: value_u64(payload.get("uniqueId"))?,
            count: value_u32(payload.get("count")).and_then(|value| u16::try_from(value).ok())?,
            success,
        }),
        "SellItem" => Some(InventoryOperationAck::Sell {
            unique_id: value_u64(payload.get("uniqueId"))?,
            count: value_u32(payload.get("count")).and_then(|value| u16::try_from(value).ok())?,
            success,
        }),
        _ => None,
    }
}

/// Retain explicit Crystal capacity and bag/belt/equipment/quest grouping.
pub fn project_native_inventory_model(
    payload: &Value,
    mut geometry: impl FnMut(NativeItemLibrary, u16) -> Option<NativeItemFrameGeometry>,
) -> Value {
    let gold = value_u32_or(payload.get("gold"), 0);
    // Only an explicit Crystal-array length can unlock page two. Occupied
    // item count and the runtime's broader maxBagSlots value are not evidence
    // that this character purchased inventory expansion.
    let capacity = value_u32(payload.get("inventoryCapacity"))
        .and_then(|value| u16::try_from(value).ok())
        .map(crate::inventory::InventoryModel::canonical_capacity)
        .unwrap_or(crate::inventory::CRYSTAL_BASE_INVENTORY_CAPACITY);

    let mut map_items = |items: Option<&Value>, default_container: u8| -> Vec<Value> {
        items
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let fallback_slot = u32::try_from(index).unwrap_or(0);
                        let unique_id = item
                            .get("uniqueId")
                            .or_else(|| item.get("unique_id"))
                            .and_then(value_u64_ref);
                        let key = value_string(item.get("key"))
                            .or_else(|| value_string(item.get("itemIndex")))
                            .or_else(|| value_string(item.get("item_index")))
                            .or_else(|| unique_id.map(|id| id.to_string()))
                            .unwrap_or_else(|| index.to_string());
                        let local_slot = native_inventory_slot(item.get("slot"), fallback_slot);
                        let source_container = value_string(item.get("container"))
                            .unwrap_or_default()
                            .to_ascii_lowercase();
                        let (container, slot) = if default_container == 0 {
                            match source_container.as_str() {
                                "bag2" => (0, 40u32.saturating_add(local_slot)),
                                "quest" => (3, local_slot),
                                _ => (0, local_slot),
                            }
                        } else {
                            (default_container, local_slot)
                        };
                        let mut mapped = json!({
                            "uniqueId": unique_id,
                            "key": key,
                            "name": value_string(item.get("name")).unwrap_or_default(),
                            "quantity": value_u32(item.get("quantity").or_else(|| item.get("count"))).unwrap_or(1),
                            "slot": slot,
                            "container": container,
                        });
                        extend_native_item_metadata(&mut mapped, item, &mut geometry);
                        mapped
                    })
                    .collect()
            })
            .unwrap_or_default()
    };

    let mut items = Vec::new();
    items.extend(map_items(payload.get("inventoryItems"), 0));
    items.extend(map_items(payload.get("beltItems"), 1));
    items.extend(map_items(payload.get("equipmentItems"), 2));

    json!({ "capacity": capacity, "gold": gold, "items": items })
}

/// Lossless public metadata and current-count image; absent geometry stays absent.
pub fn extend_native_item_metadata(
    mapped: &mut Value,
    item: &Value,
    mut geometry: impl FnMut(NativeItemLibrary, u16) -> Option<NativeItemFrameGeometry>,
) {
    let metadata = [
        ("icon", &["icon"][..]),
        ("stateImage", &["stateImage", "state_image"][..]),
        ("description", &["description"][..]),
        (
            "durabilityCurrent",
            &[
                "durabilityCurrent",
                "durability_current",
                "currentDura",
                "current_dura",
            ][..],
        ),
        (
            "durabilityMax",
            &["durabilityMax", "durability_max", "maxDura", "max_dura"][..],
        ),
        ("sellValue", &["sellValue", "sell_value", "price"][..]),
        ("equipSlot", &["equipSlot", "equip_slot"][..]),
        ("grade", &["grade"][..]),
        ("attack", &["attack"][..]),
        ("defence", &["defence", "defense"][..]),
        ("addedAttack", &["addedAttack", "added_attack"][..]),
        (
            "addedDefence",
            &[
                "addedDefence",
                "added_defence",
                "addedDefense",
                "added_defense",
            ][..],
        ),
        ("addedLuck", &["addedLuck", "added_luck"][..]),
        ("shape", &["shape"][..]),
        ("socketSlots", &["socketSlots", "socket_slots"][..]),
        ("tooltipSource", &["tooltipSource", "tooltip_source"][..]),
    ];
    let Some(target) = mapped.as_object_mut() else {
        return;
    };
    for (target_name, candidates) in metadata {
        if let Some(value) = candidates.iter().find_map(|name| item.get(*name)).cloned() {
            target.insert(target_name.to_owned(), value);
        }
    }
    let source_icon = value_u32(target.get("quantity"))
        .and_then(|quantity| native_user_item_icon(item, quantity));
    if let Some(icon) = source_icon {
        target.insert("icon".to_owned(), json!(icon));
    }
    let icon = source_icon.or_else(|| {
        value_u32(target.get("icon"))
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| *value != 0)
    });
    if let Some(frame) = icon.and_then(|index| geometry(NativeItemLibrary::Items, index)) {
        target.insert("iconWidth".to_owned(), json!(frame.width));
        target.insert("iconHeight".to_owned(), json!(frame.height));
    }
    let state_image = value_u32(target.get("stateImage"))
        .and_then(|value| u16::try_from(value).ok())
        .filter(|value| *value != 0);
    if let Some(frame) = state_image.and_then(|index| geometry(NativeItemLibrary::StateItem, index))
    {
        target.insert("stateImageX".to_owned(), json!(frame.x));
        target.insert("stateImageY".to_owned(), json!(frame.y));
        target.insert("stateImageWidth".to_owned(), json!(frame.width));
        target.insert("stateImageHeight".to_owned(), json!(frame.height));
    }
}

pub fn native_user_item_icon(item: &Value, count: u32) -> Option<u16> {
    if let Some(info) = item
        .get("tooltipSource")
        .or_else(|| item.get("tooltip_source"))
        .and_then(|source| source.get("info"))
    {
        return Some(mir2_game_data::crystal_user_item_image(
            u8::try_from(value_u32(info.get("item_type"))?).ok()?,
            i16::try_from(value_i32(info.get("shape"))?).ok()?,
            u16::try_from(value_u32(info.get("stack_size"))?).ok()?,
            u16::try_from(value_u32(info.get("image"))?).ok()?,
            count,
        ));
    }
    let index = value_i32(item.get("item_index").or_else(|| item.get("itemIndex")))?;
    let info = unique_crystal_item_template(index)?;
    Some(mir2_game_data::crystal_user_item_image(
        info.item_type,
        info.shape,
        info.stack_size,
        info.image,
        count,
    ))
}

pub fn unique_crystal_item_template(item_index: i32) -> Option<CrystalItemTemplate> {
    let mut matches = crystal_item_manifest()
        .items
        .into_iter()
        .filter(|item| item.item_index == item_index);
    let item = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(item)
}

/// Pure viewer/item tooltip adapters extracted from Windows3d735745f.
/// A missing/ambiguous database row stays partial; this never grants an item.
pub fn native_tooltip_viewer(cursor: &NativeUiPlayerCursor) -> Option<(u16, MirClass)> {
    let level = u16::try_from(cursor.level?).ok()?;
    let class = match cursor
        .class_name
        .as_deref()?
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "warrior" => MirClass::Warrior,
        "wizard" => MirClass::Wizard,
        "taoist" => MirClass::Taoist,
        "assassin" => MirClass::Assassin,
        "archer" => MirClass::Archer,
        _ => return None,
    };
    Some((level, class))
}

pub fn native_tooltip_info(item_index: i32) -> Option<CrystalItemInfoModel> {
    serde_json::from_value(serde_json::to_value(unique_crystal_item_template(item_index)?).ok()?)
        .ok()
}

pub fn native_real_tooltip_info(
    info: &CrystalItemInfoModel,
    viewer: Option<(u16, MirClass)>,
) -> Option<CrystalItemInfoModel> {
    let (level, class) = viewer?;
    if !info.class_based && !info.level_based {
        return Some(info.clone());
    }
    let origin = unique_crystal_item_template(info.item_index)?;
    let origin_model =
        serde_json::from_value::<CrystalItemInfoModel>(serde_json::to_value(&origin).ok()?).ok()?;
    if origin_model != *info {
        return None;
    }
    serde_json::from_value(
        serde_json::to_value(crystal_real_item_for_player(&origin, level, class)).ok()?,
    )
    .ok()
}

pub fn native_tooltip_source_for_user_item(
    value: &Value,
    cursor: &NativeUiPlayerCursor,
) -> Option<CrystalItemTooltipSourceModel> {
    let user_item = serde_json::from_value::<CrystalUserItemModel>(value.clone()).ok()?;
    let info = native_tooltip_info(user_item.item_index)?;
    let viewer = native_tooltip_viewer(cursor);
    let socket_infos = user_item
        .slots
        .iter()
        .map(|slot| {
            slot.as_ref()
                .and_then(|socket| native_tooltip_info(socket.item_index))
        })
        .collect::<Vec<_>>();
    let real_socket_infos = if viewer.is_some() {
        socket_infos
            .iter()
            .map(|socket| {
                socket
                    .as_ref()
                    .and_then(|socket| native_real_tooltip_info(socket, viewer))
            })
            .collect()
    } else {
        Vec::new()
    };
    Some(CrystalItemTooltipSourceModel {
        real_info: native_real_tooltip_info(&info, viewer),
        info,
        user_item: Some(user_item),
        socket_infos,
        real_socket_infos,
    })
}

pub fn native_inventory_slot(value: Option<&Value>, fallback: u32) -> u32 {
    if let Some(slot) = value_u32(value) {
        return slot;
    }

    let Some(name) = value.and_then(Value::as_str) else {
        return fallback;
    };
    match name.trim().to_ascii_lowercase().replace('_', "-").as_str() {
        "weapon" => 0,
        "armour" | "armor" => 1,
        "helmet" => 2,
        "torch" => 3,
        "necklace" => 4,
        "bracelet-left" | "braceletleft" | "braceletl" => 5,
        "bracelet-right" | "braceletright" | "braceletr" => 6,
        "ring-left" | "ringleft" | "ringl" => 7,
        "ring-right" | "ringright" | "ringr" => 8,
        "amulet" => 9,
        "belt" => 10,
        "boots" => 11,
        "stone" => 12,
        "mount" => 13,
        _ => fallback,
    }
}

fn value_u32(value: Option<&Value>) -> Option<u32> {
    value.and_then(|value| {
        value
            .as_u64()
            .and_then(|number| u32::try_from(number).ok())
            .or_else(|| value.as_str()?.parse::<u32>().ok())
    })
}
fn value_u32_or(value: Option<&Value>, fallback: u32) -> u32 {
    value_u32(value).unwrap_or(fallback)
}
fn value_i32(value: Option<&Value>) -> Option<i32> {
    value.and_then(|value| {
        value
            .as_i64()
            .and_then(|number| i32::try_from(number).ok())
            .or_else(|| value.as_str()?.parse::<i32>().ok())
    })
}
fn value_u64(value: Option<&Value>) -> Option<u64> {
    value.and_then(value_u64_ref)
}
fn value_u64_ref(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str()?.parse::<u64>().ok())
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
    use crate::inventory::{InventoryModel, CRYSTAL_BASE_INVENTORY_CAPACITY};

    #[test]
    fn explicit_capacity_containers_and_u64_identity_match_windows_projection() {
        let value = project_native_inventory_model(
            &json!({"gold":"123", "inventoryCapacity":86,
            "inventoryItems":[{"unique_id":"9007199254740993","container":"bag2","slot":"3","count":2,"name":"Exact"},
                {"uniqueId":99,"container":"quest","slot":1}],
            "beltItems":[{"uniqueId":100,"slot":0}],
            "equipmentItems":[{"uniqueId":101,"slot":"bracelet_right"}]}),
            |_, _| None,
        );
        let model: InventoryModel = serde_json::from_value(value).unwrap();
        assert_eq!(model.capacity, 86);
        assert_eq!(model.gold, 123);
        assert_eq!(model.items[0].unique_id, Some(9007199254740993));
        assert_eq!((model.items[0].container, model.items[0].slot), (0, 43));
        assert_eq!((model.items[1].container, model.items[1].slot), (3, 1));
        assert_eq!((model.items[2].container, model.items[2].slot), (1, 0));
        assert_eq!((model.items[3].container, model.items[3].slot), (2, 6));
    }

    #[test]
    fn occupancy_and_max_bag_slots_never_grant_expansion() {
        for capacity in [Value::Null, json!("bad"), json!(-1), json!(999999)] {
            let value = project_native_inventory_model(
                &json!({"inventoryCapacity":capacity,
                "maxBagSlots":999,"inventoryItems":[{"container":"bag2","slot":3}]}),
                |_, _| None,
            );
            assert_eq!(value["capacity"], CRYSTAL_BASE_INVENTORY_CAPACITY);
        }
        for capacity in [0, 46, 47, 85, 86, 100] {
            let value =
                project_native_inventory_model(&json!({"inventoryCapacity":capacity}), |_, _| None);
            assert_eq!(
                value["capacity"],
                InventoryModel::canonical_capacity(capacity)
            );
        }
    }

    #[test]
    fn named_equipment_slots_and_unknown_fallbacks_are_shared() {
        for (index, name) in [
            "weapon",
            "armour",
            "helmet",
            "torch",
            "necklace",
            "bracelet-left",
            "bracelet_right",
            "ringl",
            "ringr",
            "amulet",
            "belt",
            "boots",
            "stone",
            "mount",
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(native_inventory_slot(Some(&json!(name)), 88), index as u32);
        }
        assert_eq!(native_inventory_slot(Some(&json!("9")), 88), 9);
        assert_eq!(native_inventory_slot(Some(&json!("unknown")), 88), 88);
        assert_eq!(native_inventory_slot(Some(&Value::Null), 88), 88);
    }

    #[test]
    fn metadata_aliases_and_host_geometry_preserve_original_offsets() {
        let mut model = json!({"quantity":2});
        let source = json!({"icon":12,"state_image":30,"description":"Source", "current_dura":20,
            "maxDura":50,"sell_value":7,"equip_slot":"weapon","socket_slots":2,
            "added_defense":3,"tooltip_source":null});
        let mut calls = Vec::new();
        extend_native_item_metadata(&mut model, &source, |library, index| {
            calls.push((library, index));
            Some(NativeItemFrameGeometry {
                width: 18,
                height: 25,
                x: -3,
                y: 4,
            })
        });
        assert_eq!(
            calls,
            vec![
                (NativeItemLibrary::Items, 12),
                (NativeItemLibrary::StateItem, 30)
            ]
        );
        assert_eq!(model["durabilityCurrent"], 20);
        assert_eq!(model["durabilityMax"], 50);
        assert_eq!(model["addedDefence"], 3);
        assert_eq!(model["stateImageX"], -3);
        assert_eq!(model["stateImageY"], 4);
        assert_eq!(model["iconWidth"], 18);
        let mut absent = json!({"quantity":2});
        extend_native_item_metadata(&mut absent, &source, |_, _| None);
        assert!(absent.get("stateImageX").is_none());
        assert!(absent.get("iconWidth").is_none());
    }

    #[test]
    fn concrete_source_info_owns_zero_image_and_current_stack_count() {
        let source = json!({"icon":999,"tooltipSource":{"info":{
            "item_type":0,"shape":0,"stack_size":1,"image":0},
            "userItem":{"count":99}}});
        let mut model = json!({"quantity":2});
        extend_native_item_metadata(&mut model, &source, |_, _| None);
        assert_eq!(model["icon"], 0);
        let source = json!({"tooltip_source":{"info":{
            "item_type":3,"shape":1,"stack_size":100,"image":24},
            "userItem":{"count":1}}});
        assert_eq!(
            native_user_item_icon(&source, 300),
            Some(mir2_game_data::crystal_user_item_image(3, 1, 100, 24, 300))
        );
        assert_eq!(
            native_user_item_icon(&json!({"itemIndex":i32::MIN}), 2),
            None
        );
    }

    #[test]
    fn seven_ack_kinds_preserve_complete_fields_and_failure() {
        let payload = json!({"success":false,"grid":"Inventory","uniqueId":"9007199254740993",
            "count":"2","heroInventory":false,"from":"6","to":7,"gridFrom":"Inventory",
            "gridTo":"Belt","idFrom":"9007199254740993","idTo":99});
        for (packet, required) in [
            ("DropItem", vec!["uniqueId", "count", "heroInventory"]),
            ("MoveItem", vec!["grid", "from", "to"]),
            ("MergeItem", vec!["gridFrom", "gridTo", "idFrom", "idTo"]),
            ("SplitItem1", vec!["grid", "uniqueId", "count"]),
            ("SellItem", vec!["uniqueId", "count"]),
            ("EquipItem", vec!["grid", "uniqueId", "to"]),
            ("RemoveItem", vec!["grid", "uniqueId", "to"]),
        ] {
            let ack = project_native_inventory_operation_ack(packet, &payload).unwrap();
            assert!(!ack.success());
            for field in required.into_iter().chain(["success"]) {
                let mut bad = payload.clone();
                bad.as_object_mut().unwrap().remove(field);
                assert!(
                    project_native_inventory_operation_ack(packet, &bad).is_none(),
                    "{packet}/{field}"
                );
            }
        }
    }

    #[test]
    fn uncorrelatable_or_malformed_receipts_are_not_success() {
        assert!(
            project_native_inventory_operation_ack("SplitItem", &json!({"success":true})).is_none()
        );
        assert!(project_native_inventory_operation_ack(
            "DropItem",
            &json!({"success":true,
            "uniqueId":1,"count":65536,"heroInventory":false})
        )
        .is_none());
        assert!(project_native_inventory_operation_ack(
            "SellItem",
            &json!({"success":"true",
            "uniqueId":1,"count":1})
        )
        .is_none());
        // Decoding this public packet is not enabling it in a host allowlist.
        assert!(matches!(
            project_native_inventory_operation_ack("DeleteItem", &json!({"uniqueId":1,"count":2})),
            Some(InventoryOperationAck::Delete {
                unique_id: 1,
                count: 2,
                success: true
            })
        ));
    }
}
