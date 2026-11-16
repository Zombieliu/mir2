//! Contract check for the actual refreshed ordinary-browser projection in
//! `fixtures/web-bag-projection.json`. The fixture contains only the projected
//! `InventoryModel`, with no account, character or full world snapshot.

use mir2_client_bevy::bag_ui::{
    plan_equipment, selection_at_slot, BagEquipmentKind, BagPage, BagSelection,
};
use mir2_client_bevy::inventory::InventoryModel;
use mir2_client_bevy::pending_operations::PendingOperations;
use serde_json::{json, Value};

const PROJECTED: &str = include_str!("fixtures/web-bag-projection.json");

fn projected_inventory() -> InventoryModel {
    serde_json::from_str(PROJECTED).expect("real Web model must decode as shared InventoryModel")
}

#[test]
fn actual_browser_projection_decodes_with_original_source_and_images() {
    let raw: Value = serde_json::from_str(PROJECTED).unwrap();
    let keys: Vec<_> = raw
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["capacity", "gold", "items"]);

    let inventory = projected_inventory();
    assert_eq!(inventory.capacity, 46);
    assert_eq!(inventory.bag_slot_capacity(), 40);
    assert_eq!(inventory.gold, 200);
    assert_eq!(inventory.items.len(), 5);

    let bag_sword = inventory
        .items
        .iter()
        .find(|item| item.container == 0 && item.slot == 4)
        .unwrap();
    assert_eq!(bag_sword.unique_id, Some(4));
    assert_eq!(bag_sword.key, "crystal-item-221");
    assert_eq!(bag_sword.quantity, 1);
    assert_eq!(bag_sword.icon, 30);
    assert_eq!(bag_sword.equip_slot.as_deref(), Some("weapon"));
    assert_eq!(bag_sword.grade.as_deref(), Some("common"));
    assert_eq!(bag_sword.added_attack, 0);
    let bag_source = bag_sword
        .tooltip_source
        .as_ref()
        .expect("wire source preserved");
    assert_eq!(bag_source.info.item_index, 221);
    assert_eq!(bag_source.info.image, 30);
    assert_eq!(bag_source.info.shape, 0);
    assert_eq!(bag_source.user_item.as_ref().unwrap().unique_id, 4);
    assert_eq!(bag_source.user_item.as_ref().unwrap().item_index, 221);
    assert!(bag_source.real_info.is_some());
    assert_eq!(bag_sword.user_item_image_index(), Some(30));

    let worn_sword = inventory
        .items
        .iter()
        .find(|item| item.container == 2 && item.slot == 0)
        .unwrap();
    assert_eq!(worn_sword.unique_id, Some(0), "zero is a real instance id");
    assert_eq!(worn_sword.key, "crystal-item-221");
    assert_eq!(worn_sword.state_image, 30);
    assert_eq!(worn_sword.icon, 30);
    assert_eq!(worn_sword.quantity, 1);
    let worn_source = worn_sword
        .tooltip_source
        .as_ref()
        .expect("wire source preserved");
    assert_eq!(worn_source.info.item_index, 221);
    assert_eq!(worn_source.info.image, 30);
    assert_eq!(worn_source.user_item.as_ref().unwrap().unique_id, 0);
    assert_eq!(worn_sword.user_item_image_index(), Some(30));
}

#[test]
fn actual_ids_produce_exact_remove_and_equip_plans() {
    let inventory = projected_inventory();
    let pending = PendingOperations::default();

    let worn = BagSelection {
        container: 2,
        slot: 0,
        unique_id: 0,
    };
    let remove = plan_equipment(&inventory, worn, &pending).expect("UID 0 can be removed");
    assert_eq!(remove.kind, BagEquipmentKind::Remove);
    assert_eq!(remove.unique_id, 0);
    assert_eq!(remove.grid, "inventory");
    assert_eq!(remove.to, 0, "the first unoccupied normalized bag slot");

    let bag = selection_at_slot(&inventory, BagPage::First, 4).unwrap();
    assert_eq!(bag.unique_id, 4);
    let equip = plan_equipment(&inventory, bag, &pending).expect("UID 4 can be equipped");
    assert_eq!(equip.kind, BagEquipmentKind::Equip);
    assert_eq!(equip.unique_id, 4);
    assert_eq!(equip.grid, "inventory");
    assert_eq!(
        equip.to, 0,
        "weapon destination from authoritative equipSlot"
    );
}

#[test]
fn tooltip_user_item_id_cannot_repair_missing_outer_identity() {
    let mut raw: Value = serde_json::from_str(PROJECTED).unwrap();
    for item in raw["items"].as_array_mut().unwrap() {
        if (item["container"] == 0 && item["slot"] == 4)
            || (item["container"] == 2 && item["slot"] == 0)
        {
            item["uniqueId"] = Value::Null;
        }
    }
    let inventory: InventoryModel = serde_json::from_value(raw).unwrap();
    let bag = inventory
        .items
        .iter()
        .find(|item| item.container == 0 && item.slot == 4)
        .unwrap();
    let worn = inventory
        .items
        .iter()
        .find(|item| item.container == 2 && item.slot == 0)
        .unwrap();
    assert_eq!(
        bag.tooltip_source
            .as_ref()
            .unwrap()
            .user_item
            .as_ref()
            .unwrap()
            .unique_id,
        4
    );
    assert_eq!(
        worn.tooltip_source
            .as_ref()
            .unwrap()
            .user_item
            .as_ref()
            .unwrap()
            .unique_id,
        0
    );
    assert_eq!(bag.unique_id, None);
    assert_eq!(worn.unique_id, None);
    assert!(BagSelection::from_item(bag).is_none());
    assert!(BagSelection::from_item(worn).is_none());
    assert!(selection_at_slot(&inventory, BagPage::First, 4).is_none());
}

#[test]
fn malformed_deep_source_u16_does_not_deserialize() {
    let original: Value = serde_json::from_str(PROJECTED).unwrap();
    let sword = original["items"]
        .as_array()
        .unwrap()
        .iter()
        .position(|item| item["container"] == 0 && item["slot"] == 4)
        .unwrap();

    let mut bad_info_image = original.clone();
    bad_info_image["items"][sword]["tooltipSource"]["info"]["image"] = json!(65_536);
    assert!(serde_json::from_value::<InventoryModel>(bad_info_image).is_err());

    let mut bad_user_durability = original;
    bad_user_durability["items"][sword]["tooltipSource"]["userItem"]["current_dura"] =
        json!(65_536);
    assert!(serde_json::from_value::<InventoryModel>(bad_user_durability).is_err());
}
