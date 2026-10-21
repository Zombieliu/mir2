//! Pure carrier/session checks. The source marker is constructed by trusted
//! test setup; these tests do not claim an online PG conversion or valuation.
use super::*;
use crate::config::{CharacterSaveRecord, EquipmentSlot, ItemContainer, SimulationConfig};
use crate::runtime::equipment::{
    equip_item_impl, equipment_slot_index, equipment_state_from_item_state,
    item_state_from_equipment_state, remove_equipped_item_impl, EquipmentState,
};
use crate::runtime::inventory::{
    add_or_increment_item, additional_slots_needed_for_item_quantity,
    item_stack_identity_compatible, merge_item_within_collection, split_item_impl,
};
use crate::runtime::items::hunting_item_binding::{
    belongs_to_current_owner, equipment_has_bound, equipment_has_economic_provenance, has_bound,
    has_economic_provenance, validate_character_load, HuntingItemBinding,
};
use crate::runtime::items::{
    crystal_item_template_for_item_key, embedded_item_state_from_template,
    try_item_state_from_user_item, try_user_item_from_item_state,
};
use crate::runtime::resources::{InventoryResource, RuntimeConfigResource, SessionResource};
use crate::runtime::session::SimulationSession;
use mir2_protocol::{ClientPacket, MirGridType, ServerPacket};
use serde_json::json;

const ID: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const QUOTE: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const DIGEST: &str = "3333333333333333333333333333333333333333333333333333333333333333";
const LOT: &str = "4444444444444444444444444444444444444444444444444444444444444444";

fn identity() -> ConversionIdentity<'static> {
    ConversionIdentity {
        operation_id: ID,
        quote_id: QUOTE,
        valuation_version: "fixed-v1",
        valuation_digest: DIGEST,
        lot_id: LOT,
    }
}

fn session() -> SimulationSession {
    let mut session = SimulationSession::new(SimulationConfig::default());
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into()
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(session
        .active_zone_join_snapshot("conversion-carrier")
        .is_some());
    session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config
        .hunting_economy_realm = Some("conversion-carrier".into());
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .hunting_character_incarnation =
        Some("pg-character-v1:12345678-1234-4234-9234-123456789abc".into());
    let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
    inventory.inventory_items.clear();
    inventory.belt_items.clear();
    inventory.storage_items.clear();
    inventory.equipment_items.clear();
    drop(inventory);
    session
}

fn plain(key: &str, uid: u64, slot: u8, quantity: u32) -> ItemState {
    let template = crystal_item_template_for_item_key(key).unwrap();
    let mut item = embedded_item_state_from_template(&template, ItemContainer::Bag1, slot);
    item.key = key.into();
    item.unique_id = uid;
    item.quantity = quantity;
    item
}

fn bound(session: &SimulationSession, key: &str, uid: u64, slot: u8, quantity: u32) -> ItemState {
    let mut item = plain(key, uid, slot, quantity);
    item.economic_binding = Some(
        HuntingItemBinding::for_current_owner(
            session.app.world(),
            "quest-once-v1:2110001:reward:0",
        )
        .unwrap(),
    );
    item
}

fn converted(
    session: &SimulationSession,
    key: &str,
    uid: u64,
    slot: u8,
    quantity: u32,
) -> ItemState {
    convert_bound_root(
        session.app.world(),
        &bound(session, key, uid, slot, quantity),
        identity(),
    )
    .unwrap()
}

fn save_with_item(
    session: &SimulationSession,
    item: &ItemState,
    collection: u8,
) -> CharacterSaveRecord {
    let mut save = session.active_character_checkpoint().unwrap();
    save.inventory_items_json.clear();
    save.belt_items_json.clear();
    save.storage_items_json.clear();
    save.equipment_items_json.clear();
    save.hero_inventory_items_json.clear();
    save.hero_equipment_items_json.clear();
    let encoded = serde_json::to_string(item).unwrap();
    match collection {
        0 => save.inventory_items_json.push(encoded),
        1 => save.belt_items_json.push(encoded),
        2 => save.storage_items_json.push(encoded),
        3 => save.hero_inventory_items_json.push(encoded),
        4 => save.hero_equipment_items_json.push(encoded),
        5 => save.equipment_items_json.push(
            serde_json::to_string(&equipment_state_from_item_state(
                item,
                EquipmentSlot::Weapon,
            ))
            .unwrap(),
        ),
        _ => unreachable!(),
    }
    save
}

#[test]
fn hunting_item_conversion_strict_provenance_and_unchanged_legacy_bound_shapes() {
    let session = session();
    let old = plain("red-potion", 100, 8, 1);
    let old_json = serde_json::to_value(&old).unwrap();
    assert!(old_json.get("economic_binding").is_none());
    assert!(old_json.get("economic_conversion").is_none());
    assert_eq!(
        serde_json::to_value(serde_json::from_value::<ItemState>(old_json.clone()).unwrap())
            .unwrap(),
        old_json
    );
    let source = bound(&session, "red-potion", 101, 8, 1);
    let source_json = serde_json::to_value(&source).unwrap();
    assert!(source_json.get("economic_conversion").is_none());
    assert_eq!(source_json["economic_binding"]["version"], json!(1));
    assert_eq!(
        serde_json::to_value(serde_json::from_value::<ItemState>(source_json.clone()).unwrap())
            .unwrap(),
        source_json
    );
    let result = convert_bound_root(session.app.world(), &source, identity()).unwrap();
    let good = serde_json::to_value(result.economic_conversion.as_ref().unwrap()).unwrap();
    let full = serde_json::to_value(&result).unwrap();
    assert_eq!(
        serde_json::to_value(serde_json::from_value::<ItemState>(full.clone()).unwrap()).unwrap(),
        full
    );
    for (field, value) in [
        ("version", json!(0)),
        ("version", json!(2)),
        ("realm", json!("")),
        ("realm", json!(" realm")),
        ("realm", json!("x".repeat(129))),
        ("operationId", json!("a".repeat(63))),
        ("quoteId", json!("A".repeat(64))),
        ("lotId", json!("0".repeat(64))),
        ("valuationVersion", json!("v\n1")),
        ("valuationDigest", json!(format!("sha256:{DIGEST}"))),
        ("sourceBindingDigest", json!("z".repeat(64))),
        ("inputUniqueId", json!(0)),
        ("future", json!(true)),
    ] {
        let mut invalid = good.clone();
        invalid[field] = value;
        assert!(
            serde_json::from_value::<HuntingItemConversionProvenance>(invalid).is_err(),
            "{field}"
        );
    }
    for field in good.as_object().unwrap().keys() {
        let mut missing = good.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<HuntingItemConversionProvenance>(missing).is_err(),
            "{field}"
        );
    }
    let mut duplicate = serde_json::to_string(&good).unwrap();
    duplicate.insert_str(1, "\"version\":1,");
    assert!(serde_json::from_str::<HuntingItemConversionProvenance>(&duplicate).is_err());
}

#[test]
fn hunting_item_conversion_only_replaces_a_verified_plain_bound_root() {
    let session = session();
    let source = bound(&session, "red-potion", 102, 8, 3);
    let before = serde_json::to_value(&source).unwrap();
    let result = convert_bound_root(session.app.world(), &source, identity()).unwrap();
    assert_eq!(serde_json::to_value(&source).unwrap(), before);
    assert!(result.economic_binding.is_none());
    assert!(!has_bound(&result));
    assert!(has_economic_provenance(&result));
    let provenance = result.economic_conversion.as_ref().unwrap();
    assert_eq!(
        provenance.source_binding_digest,
        source_binding_digest(&source).unwrap()
    );
    assert_eq!(provenance.input_unique_id, source.unique_id);
    assert_eq!(provenance.operation_id, ID);
    let mut expected = before.clone();
    expected.as_object_mut().unwrap().remove("economic_binding");
    expected["economic_conversion"] = serde_json::to_value(provenance).unwrap();
    assert_eq!(serde_json::to_value(&result).unwrap(), expected);
    for invalid in [
        plain("red-potion", 103, 8, 1),
        result.clone(),
        {
            let mut zero = source.clone();
            zero.unique_id = 0;
            zero
        },
        {
            let mut mixed = source.clone();
            mixed.economic_conversion = result.economic_conversion.clone();
            mixed
        },
    ] {
        let before = serde_json::to_value(&invalid).unwrap();
        assert!(convert_bound_root(session.app.world(), &invalid, identity()).is_err());
        assert_eq!(serde_json::to_value(&invalid).unwrap(), before);
    }
    let mut bad_identity = identity();
    bad_identity.quote_id = "client-quote";
    assert!(convert_bound_root(session.app.world(), &source, bad_identity).is_err());
    let mut changed_source = source.clone();
    changed_source.economic_binding = Some(
        HuntingItemBinding::for_current_owner(session.app.world(), "quest-once-v1:2110002")
            .unwrap(),
    );
    assert_ne!(
        source_binding_digest(&source).unwrap(),
        source_binding_digest(&changed_source).unwrap()
    );
}

#[test]
fn hunting_item_conversion_mixed_roots_and_hidden_children_fail_before_reconstruction() {
    let session = session();
    let result = converted(&session, "dagger", 110, 8, 1);
    let mut mixed = result.clone();
    mixed.economic_binding = bound(&session, "dagger", 110, 8, 1).economic_binding;
    assert!(serde_json::from_value::<ItemState>(serde_json::to_value(&mixed).unwrap()).is_err());
    assert!(validate_committed_item_state_carrier(&mixed).is_err());
    assert!(!belongs_to_current_owner(session.app.world(), &mixed));
    let equipment = equipment_state_from_item_state(&result, EquipmentSlot::Weapon);
    let mut mixed_equipment = serde_json::to_value(&equipment).unwrap();
    mixed_equipment["economic_binding"] = serde_json::to_value(mixed.economic_binding).unwrap();
    assert!(serde_json::from_value::<EquipmentState>(mixed_equipment).is_err());

    let root = plain("dagger", 111, 8, 1);
    let child = converted(&session, "dagger", 112, 0, 1);
    let mut wire = try_user_item_from_item_state(&root).unwrap();
    wire.slots = vec![Some(try_user_item_from_item_state(&child).unwrap())];
    let mut tree = try_item_state_from_user_item(root, &wire).unwrap();
    tree.socketed[0].economic_conversion = child.economic_conversion.clone();
    let before = serde_json::to_value(&tree).unwrap();
    assert!(!has_bound(&tree));
    assert!(has_economic_provenance(&tree));
    assert!(try_item_state_from_user_item(tree.clone(), &wire).is_err());
    assert_eq!(serde_json::to_value(&tree).unwrap(), before);
    tree.economic_binding = bound(&session, "dagger", 111, 8, 1).economic_binding;
    assert!(convert_bound_root(session.app.world(), &tree, identity()).is_err());
    let mut retained = bound(&session, "dagger", 113, 8, 1);
    let mut retained_wire = try_user_item_from_item_state(&retained).unwrap();
    retained_wire.slots = wire.slots.clone();
    retained = try_item_state_from_user_item(retained, &retained_wire).unwrap();
    let metadata = retained.user_item_metadata.as_mut().unwrap();
    metadata.slots = retained_wire.slots;
    retained.socketed.clear();
    assert!(convert_bound_root(session.app.world(), &retained, identity()).is_err());
}

#[test]
fn hunting_item_conversion_same_realm_new_owner_can_use_and_load_all_carriers() {
    let mut session = session();
    let source = bound(&session, "wooden-sword", 120, 8, 1);
    let result = convert_bound_root(session.app.world(), &source, identity()).unwrap();
    let saves: Vec<_> = (0..6)
        .map(|collection| save_with_item(&session, &result, collection))
        .collect();
    {
        let mut actor = session.app.world_mut().resource_mut::<SessionResource>();
        actor.account_id = Some("peer".into());
        actor.hunting_character_incarnation = None;
    }
    assert!(!belongs_to_current_owner(session.app.world(), &source));
    assert!(convert_bound_root(session.app.world(), &source, identity()).is_err());
    assert!(belongs_to_current_owner(session.app.world(), &result));
    for save in &saves {
        validate_character_load(session.app.world(), save, "peer", None).unwrap();
    }
    for realm in [Some("other-realm".into()), None] {
        session
            .app
            .world_mut()
            .resource_mut::<RuntimeConfigResource>()
            .config
            .hunting_economy_realm = realm;
        assert!(!belongs_to_current_owner(session.app.world(), &result));
        for save in &saves {
            assert!(validate_character_load(session.app.world(), save, "peer", None).is_err());
        }
    }
}

#[test]
fn hunting_item_conversion_equip_remove_and_wire_identity_keep_provenance() {
    let mut session = session();
    let result = converted(&session, "wooden-sword", 130, 8, 1);
    let marker = result.economic_conversion.clone();
    let wire = try_user_item_from_item_state(&result).unwrap();
    assert_eq!(
        try_item_state_from_user_item(result.clone(), &wire)
            .unwrap()
            .economic_conversion,
        marker
    );
    for changed in [
        {
            let mut changed = wire.clone();
            changed.unique_id += 1;
            changed
        },
        {
            let mut changed = wire.clone();
            changed.count += 1;
            changed
        },
    ] {
        assert!(try_item_state_from_user_item(result.clone(), &changed).is_err());
    }
    session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .inventory_items = vec![result.clone()];
    let packets = equip_item_impl(
        session.app.world_mut(),
        MirGridType::Inventory,
        result.unique_id,
        equipment_slot_index(EquipmentSlot::Weapon).unwrap() as i32,
    );
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::EquipItem { success: true, .. })),
        "{packets:?}"
    );
    let equipment = session
        .app
        .world()
        .resource::<InventoryResource>()
        .equipment_items[0]
        .clone();
    assert!(!equipment_has_bound(&equipment));
    assert!(equipment_has_economic_provenance(&equipment));
    assert_eq!(equipment.economic_conversion, marker);
    assert_eq!(
        item_state_from_equipment_state(equipment, ItemContainer::Bag1, 8).economic_conversion,
        marker
    );
    let packets = remove_equipped_item_impl(
        session.app.world_mut(),
        MirGridType::Inventory,
        result.unique_id,
        8,
    );
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::RemoveItem { success: true, .. })),
        "{packets:?}"
    );
    assert_eq!(
        session
            .app
            .world()
            .resource::<InventoryResource>()
            .inventory_items[0]
            .economic_conversion,
        marker
    );
}

#[test]
fn hunting_item_conversion_merge_only_matches_the_same_lot_and_split_keeps_its_origin() {
    let mut session = session();
    let first = converted(&session, "red-potion", 140, 8, 2);
    let mut same = first.clone();
    same.unique_id = 141;
    same.slot = 9;
    same.quantity = 3;
    assert!(item_stack_identity_compatible(&first, &same));
    let legacy = plain("red-potion", 142, 9, 1);
    let mut other_identity = identity();
    other_identity.lot_id = QUOTE;
    let mut other = convert_bound_root(
        session.app.world(),
        &bound(&session, "red-potion", 140, 8, 1),
        other_identity,
    )
    .unwrap();
    other.unique_id = 143;
    other.slot = 9;
    for incompatible in [legacy, other] {
        let mut items = vec![first.clone(), incompatible.clone()];
        let before = serde_json::to_value(&items).unwrap();
        assert!(!merge_item_within_collection(
            &mut items,
            MirGridType::Inventory,
            first.unique_id,
            incompatible.unique_id,
            None
        ));
        assert_eq!(serde_json::to_value(&items).unwrap(), before);
    }
    let mut items = vec![first.clone(), same];
    assert!(merge_item_within_collection(
        &mut items,
        MirGridType::Inventory,
        140,
        141,
        None
    ));
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].quantity, 5);
    let provenance = items[0].economic_conversion.clone();
    session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .inventory_items = items;
    let packets = split_item_impl(session.app.world_mut(), MirGridType::Inventory, 141, 2);
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::SplitItem1 { success: true, .. })),
        "{packets:?}"
    );
    let inventory = session.app.world().resource::<InventoryResource>();
    let all: Vec<_> = inventory
        .inventory_items
        .iter()
        .chain(&inventory.belt_items)
        .collect();
    assert_eq!(all.len(), 2);
    assert_eq!(all.iter().map(|item| item.quantity).sum::<u32>(), 5);
    assert_ne!(all[0].unique_id, all[1].unique_id);
    assert!(all
        .iter()
        .all(|item| item.unique_id > 0 && item.economic_conversion == provenance));
    assert_eq!(provenance.unwrap().input_unique_id, 140);
}

#[test]
fn hunting_item_conversion_legacy_grants_do_not_use_converted_stack_capacity() {
    let mut session = session();
    let result = converted(&session, "red-potion", 150, 8, 2);
    let before = serde_json::to_value(&result).unwrap();
    session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .inventory_items = vec![result];
    assert_eq!(
        additional_slots_needed_for_item_quantity(
            session.app.world().resource::<InventoryResource>(),
            ItemContainer::Bag1,
            "red-potion",
            1
        ),
        1
    );
    let template = crystal_item_template_for_item_key("red-potion").unwrap();
    add_or_increment_item(
        session.app.world_mut(),
        ItemContainer::Bag1,
        "red-potion",
        &template.name,
        template.tooltip.as_deref().unwrap_or(""),
        9,
        1,
        u16::from(template.weight),
    );
    let inventory = session.app.world().resource::<InventoryResource>();
    let all: Vec<_> = inventory
        .inventory_items
        .iter()
        .chain(&inventory.belt_items)
        .collect();
    assert_eq!(
        all.iter()
            .filter(|item| item.economic_conversion.is_some())
            .count(),
        1
    );
    assert_eq!(
        serde_json::to_value(
            all.iter()
                .find(|item| item.economic_conversion.is_some())
                .unwrap()
        )
        .unwrap(),
        before
    );
    assert_eq!(
        all.iter()
            .filter(|item| item.economic_conversion.is_none())
            .map(|item| item.quantity)
            .sum::<u32>(),
        1
    );
}
