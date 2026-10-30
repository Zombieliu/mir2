use super::super::items::{embedded_item_state_from_template, try_item_state_from_user_item};
use super::*;

fn item(key: &str, uid: u64, slot: u8) -> ItemState {
    let template = mir2_game_data::crystal_item_by_name(key).unwrap();
    let mut item = embedded_item_state_from_template(&template, ItemContainer::Bag1, slot);
    item.unique_id = uid;
    item
}

fn manifest(items: &[ItemState]) -> Result<AttachmentManifest, String> {
    attachment_manifest(
        &items
            .iter()
            .map(|item| serde_json::to_string(item).unwrap())
            .collect::<Vec<_>>(),
    )
}

#[test]
fn mail_transfer_manifest_preserves_sidecarless_nested_live_tree_and_opaque_text() {
    let mut root = item("BronzeHelmet", 100, 0);
    root.description = r#"{"unique_id":18446744073709551615}"#.into();
    root.attack = 42;
    root.durability_current = Some(7);
    let mut child = item("BronzeBell", 101, 1);
    child.socket_slots = 2;
    child.socketed.push(item("CopperRing", 102, 0));
    root.socket_slots = 2;
    root.socketed.push(child);
    let expected = serde_json::to_value(vec![root.clone()]).unwrap();
    let result = manifest(&[root]).unwrap();
    assert_eq!(result.original_items, expected);
    assert_eq!(result.root_uids, vec![100]);
    assert_eq!(result.live_uids, BTreeSet::from([100, 101, 102]));
}

#[test]
fn mail_transfer_manifest_retired_captured_reference_stays_frozen_but_is_not_physical() {
    let original = item("BronzeHelmet", 200, 0);
    let mut wire = try_user_item_from_item_state(&original).unwrap();
    wire.slots = vec![
        None,
        Some(try_user_item_from_item_state(&item("BronzeBell", u64::MAX - 1, 1)).unwrap()),
    ];
    let mut root = try_item_state_from_user_item(original, &wire).unwrap();
    root.socketed.clear();
    let projection = try_user_item_from_item_state(&root).unwrap();
    assert!(projection.slots.iter().all(Option::is_none));
    let expected = serde_json::to_value(vec![root.clone()]).unwrap();
    let result = manifest(&[root]).unwrap();
    assert_eq!(result.original_items, expected);
    assert_eq!(
        result.original_items[0]["user_item_metadata"]["captured_socket_positions"][1]["unique_id"],
        serde_json::json!(u64::MAX - 1)
    );
    assert_eq!(result.live_uids, BTreeSet::from([200]));
    assert_eq!(result.root_uids, vec![200]);
}

#[test]
fn mail_transfer_manifest_rejects_original_zero_in_root_live_child_and_legacy_metadata() {
    assert!(manifest(&[item("BronzeHelmet", 0, 0)]).is_err());
    let mut root = item("BronzeHelmet", 300, 0);
    root.socket_slots = 2;
    root.socketed.push(item("BronzeBell", 0, 1));
    assert!(manifest(&[root]).is_err());
    let source = item("BronzeHelmet", 301, 0);
    let mut wire = try_user_item_from_item_state(&source).unwrap();
    wire.slots = vec![Some(
        try_user_item_from_item_state(&item("BronzeBell", 302, 0)).unwrap(),
    )];
    let mut root = try_item_state_from_user_item(source, &wire).unwrap();
    let metadata = root.user_item_metadata.as_mut().unwrap();
    metadata.slots = wire.slots;
    metadata.slots[0].as_mut().unwrap().unique_id = 0;
    metadata.live_socketed_at_capture = false;
    metadata.socket_layout_hydrated = false;
    metadata.captured_socket_positions = None;
    root.socketed.clear();
    assert_eq!(
        try_user_item_from_item_state(&root).unwrap().slots[0]
            .as_ref()
            .unwrap()
            .unique_id,
        0,
        "this fixture must exercise a current legacy slot, not retained history"
    );
    assert!(manifest(&[root]).is_err());
}

#[test]
fn mail_transfer_manifest_rejects_duplicate_live_identity_across_roots_and_nested_nodes() {
    let a = item("BronzeHelmet", 400, 0);
    let b = item("BronzeHelmet", 400, 1);
    assert!(manifest(&[a.clone(), b]).is_err());
    let mut nested = item("BronzeHelmet", 401, 1);
    nested.socket_slots = 2;
    nested.socketed.push(item("BronzeBell", 400, 1));
    assert!(manifest(&[a, nested]).is_err());
    let mut nested = item("BronzeHelmet", 402, 0);
    nested.socket_slots = 2;
    nested.socketed.push(item("BronzeBell", 402, 1));
    assert!(manifest(&[nested]).is_err());
}

#[test]
fn mail_transfer_manifest_rejects_monthly_uid_and_monthly_template_in_physical_nodes() {
    let (_, reserved) = crate::monthly_card::monthly_card_item_identity("owner", 0, "request", 0);
    assert!(manifest(&[item("BronzeHelmet", reserved, 0)]).is_err());
    let source = item("BronzeHelmet", 500, 0);
    let mut wire = try_user_item_from_item_state(&source).unwrap();
    let mut child = try_user_item_from_item_state(&item("BronzeBell", 501, 0)).unwrap();
    child.item_index = mir2_game_data::BILLING_MONTHLY_CARD_ITEM_INDEX;
    wire.slots = vec![Some(child)];
    let root = try_item_state_from_user_item(source, &wire).unwrap();
    assert!(manifest(&[root]).is_err());
}

#[test]
fn mail_transfer_manifest_rejects_corrupt_metadata_and_non_bag_roots() {
    let source = item("BronzeHelmet", 600, 0);
    let mut wire = try_user_item_from_item_state(&source).unwrap();
    wire.slots = vec![Some(
        try_user_item_from_item_state(&item("BronzeBell", 601, 0)).unwrap(),
    )];
    let mut root = try_item_state_from_user_item(source, &wire).unwrap();
    root.user_item_metadata
        .as_mut()
        .unwrap()
        .captured_socket_positions
        .as_mut()
        .unwrap()[0]
        .as_mut()
        .unwrap()
        .item_index = i32::MAX;
    assert!(manifest(&[root]).is_err());
    let mut non_bag = item("BronzeHelmet", 602, 0);
    non_bag.container = ItemContainer::Belt;
    assert!(manifest(&[non_bag]).is_err());
    assert!(attachment_manifest(&["{bad ItemState".into()]).is_err());
}

#[test]
fn mail_transfer_manifest_enforces_five_roots_and_preserves_input_order() {
    let roots = (0..5)
        .map(|i| item("BronzeHelmet", 700 + u64::from(i), i))
        .collect::<Vec<_>>();
    assert_eq!(
        manifest(&roots).unwrap().root_uids,
        vec![700, 701, 702, 703, 704]
    );
    let mut too_many = roots;
    too_many.push(item("BronzeHelmet", 705, 5));
    assert!(manifest(&too_many).is_err());
    let empty = attachment_manifest(&[]).unwrap();
    assert_eq!(empty.original_items, serde_json::json!([]));
    assert!(empty.live_uids.is_empty());
    assert!(empty.root_uids.is_empty());
}
