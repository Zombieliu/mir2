use super::*;
use crate::config::{GroundDropLootSnapshot, GroundDropSnapshot, SimulationConfig};
use crate::runtime::equipment::{equipment_slot_unique_id, equipment_state_from_item_state};
use crate::runtime::inventory::{
    add_exact_ground_drop_items, can_gain_exact_ground_drop_item,
    empty_slots_for_inventory_container,
};
use crate::runtime::items::{
    crystal_item_template_for_item_key, embedded_item_state_from_template,
    try_item_state_from_user_item,
};
use crate::runtime::resources::InventoryResource;
use crate::runtime::session::SimulationSession;
use crate::runtime::zone::{
    GroundDropClaimTicket, ZoneCollision, ZoneCommand, ZoneKey, ZoneOutbound, ZoneRuntime,
};
use bevy_ecs::world::World;
use mir2_game_data::crystal_item_by_name;
use mir2_protocol::{ClientPacket, ServerPacket};
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

fn started(config: SimulationConfig) -> SimulationSession {
    let mut session = SimulationSession::new(config);
    let character_index = session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        })
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::LoginSuccess { characters } => characters.first().map(|item| item.index),
            _ => None,
        })
        .expect("normal account login succeeds");
    let packets = session.handle_packet(ClientPacket::StartGame { character_index });
    assert!(
        session
            .active_zone_join_snapshot("custody-player")
            .is_some(),
        "{packets:?}"
    );
    session
}

fn instance(name: &str, uid: u64, quantity: u32) -> ItemState {
    let template = crystal_item_by_name(name).expect("canonical fixture template");
    let mut item = embedded_item_state_from_template(&template, ItemContainer::Bag1, 8);
    item.unique_id = uid;
    item.quantity = quantity;
    // All of these fields belong to the server instance, not just UserItem.
    item.name = format!("custody-{name}");
    item.description = "private full-instance description".to_string();
    item.attack += 31;
    item.defence += 17;
    item.heal_hp += 9;
    item.heal_mp += 11;
    item
}

fn recursive_instance() -> ItemState {
    let root = instance("Dagger", 81_001, 1);
    let child = instance("BronzeBell", 81_002, 1);
    let grandchild = instance("Dagger", 81_003, 1);
    let mut root_wire = try_user_item_from_item_state(&root).unwrap();
    let mut child_wire = try_user_item_from_item_state(&child).unwrap();
    child_wire.slots = vec![
        Some(try_user_item_from_item_state(&grandchild).unwrap()),
        None,
    ];
    root_wire.slots = vec![None, Some(child_wire), None];
    let mut item = try_item_state_from_user_item(root, &root_wire).expect("hydrated socket tree");
    item.socketed[0].attack += 211;
    item.socketed[0].description = "socket server-only description".to_string();
    item.socketed[0].socketed[0].heal_hp += 113;
    validate_committed_item_state_carrier(&item).unwrap();
    item
}

fn functional_value(item: &ItemState) -> Value {
    let mut value = serde_json::to_value(item).unwrap();
    let fields = value.as_object_mut().unwrap();
    fields.remove("slot");
    fields.remove("container");
    value
}

fn install(session: &mut SimulationSession, item: ItemState) {
    let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
    inventory.inventory_items.clear();
    inventory.belt_items.clear();
    inventory.inventory_items.push(item);
}

fn dropped(session: &mut SimulationSession, uid: u64, quantity: u16) -> GroundDropSnapshot {
    let packets = session.handle_packet(ClientPacket::DropItem {
        unique_id: uid,
        count: quantity,
        hero_inventory: false,
    });
    assert!(
        packets.iter().any(|packet| matches!(packet,
        ServerPacket::DropItem { success: true, unique_id, count, .. }
            if *unique_id == uid && *count == quantity)),
        "{packets:?}"
    );
    session
        .world_snapshot()
        .ground_drops
        .into_iter()
        .find(|drop| {
            matches!(
                &drop.loot, GroundDropLootSnapshot::InventoryItem { exact_item: Some(payload), .. }
                    if payload.server_state.is_some()
            )
        })
        .expect("ordinary DropItem captures full server custody")
}

fn payload(drop: &GroundDropSnapshot) -> &GroundDropItemPayload {
    let GroundDropLootSnapshot::InventoryItem {
        exact_item: Some(payload),
        ..
    } = &drop.loot
    else {
        panic!("exact inventory ground drop");
    };
    payload
}

fn payload_mut(drop: &mut GroundDropSnapshot) -> &mut GroundDropItemPayload {
    let GroundDropLootSnapshot::InventoryItem {
        exact_item: Some(payload),
        ..
    } = &mut drop.loot
    else {
        panic!("exact inventory ground drop");
    };
    payload
}

fn shared_claim(
    session: &SimulationSession,
    drop: GroundDropSnapshot,
) -> (ZoneRuntime, GroundDropClaimTicket) {
    let join = session.active_zone_join_snapshot("custody-player").unwrap();
    let session_id = join.session_id.clone();
    let mut zone = ZoneRuntime::new_with_collision(
        ZoneKey::for_map(&join.map_file_name),
        ZoneCollision::unbounded(),
    );
    zone.handle(ZoneCommand::Join(join));
    zone.handle(ZoneCommand::SyncGroundDrops {
        session_id: session_id.clone(),
        drops: vec![drop.clone()],
        now_ms: 0,
    });
    assert!(zone.has_ground_drop(drop.object_id));
    let outbounds = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: session_id.clone(),
        object_id: Some(drop.object_id),
        target: mir2_protocol::Point {
            x: drop.x,
            y: drop.y,
        },
        group_members: Vec::new(),
        now_ms: 1,
    });
    let ticket = outbounds
        .into_iter()
        .find_map(|outbound| match outbound {
            ZoneOutbound::GroundDropClaimedWithTicket {
                session_id: owner,
                ticket,
            } if owner == session_id => Some(ticket),
            _ => None,
        })
        .expect("real shared Zone claim capability");
    assert_eq!(ticket.drop, drop);
    assert!(!zone.has_ground_drop(drop.object_id));
    (zone, ticket)
}

fn cancel(zone: &mut ZoneRuntime, ticket: GroundDropClaimTicket) {
    let object_id = ticket.object_id;
    zone.handle(ZoneCommand::CancelGroundDropClaimWithTicket {
        session_id: ticket.session_id.clone(),
        ticket,
        now_ms: 2,
    });
    assert!(
        zone.has_ground_drop(object_id),
        "failed personal pickup restores shared source"
    );
}

struct StorePath(PathBuf);
impl StorePath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "mir2-ground-custody-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn config(&self) -> SimulationConfig {
        SimulationConfig::default().with_account_store_path(self.0.join("accounts.json"))
    }
}

#[test]
fn ground_custody_normal_drop_shared_claim_logout_cold_login_preserves_recursive_state() {
    let store = StorePath::new();
    let original = recursive_instance();
    let expected = functional_value(&original);
    {
        let mut session = started(store.config());
        install(&mut session, original.clone());
        let drop = dropped(&mut session, original.unique_id, 1);
        assert_eq!(
            payload(&drop).item,
            try_user_item_from_item_state(&original).unwrap()
        );
        let (mut zone, ticket) = shared_claim(&session, drop);
        let receipt = session.commit_shared_ground_drop_pickup_transaction(&ticket.drop);
        assert!(receipt.committed, "{:?}", receipt.packets);
        zone.handle(ZoneCommand::CommitGroundDropClaimWithTicket {
            session_id: ticket.session_id.clone(),
            ticket: ticket.clone(),
        });
        assert!(!zone.has_ground_drop(ticket.object_id));
        assert!(zone.pending_ground_drop_claim_tickets().is_empty());
        let inventory = session.app.world().resource::<InventoryResource>();
        let restored = inventory
            .inventory_items
            .iter()
            .chain(&inventory.belt_items)
            .find(|item| item.unique_id == original.unique_id)
            .unwrap();
        assert_eq!(functional_value(restored), expected);
        let logout = session.handle_packet(ClientPacket::LogOut);
        assert!(
            session
                .active_zone_join_snapshot("custody-player")
                .is_none(),
            "{logout:?}"
        );
    }
    // A fresh config and Session re-open the file source of truth; no serde-only
    // surrogate or retained live Session inventory is used as the oracle.
    let cold = started(store.config());
    let inventory = cold.app.world().resource::<InventoryResource>();
    let restored = inventory
        .inventory_items
        .iter()
        .chain(&inventory.belt_items)
        .find(|item| item.unique_id == original.unique_id)
        .expect("normal cold login restores item");
    assert_eq!(functional_value(restored), expected);
}

#[test]
fn ground_custody_partial_drop_assigns_new_uid_and_preserves_server_fields() {
    let mut session = started(SimulationConfig::default());
    // Use the canonical starter potion alias rather than relying on a display
    // translation in the game-data manifest.
    let template = crystal_item_template_for_item_key("red-potion").unwrap();
    let mut canonical = embedded_item_state_from_template(&template, ItemContainer::Bag1, 8);
    canonical.unique_id = 82_001;
    canonical.quantity = 4;
    canonical.heal_hp += 321;
    canonical.description = "private partial-stack description".to_string();
    let wire = try_user_item_from_item_state(&canonical).unwrap();
    let original = try_item_state_from_user_item(canonical, &wire).unwrap();
    assert!(original.user_item_metadata.is_some());
    install(&mut session, original.clone());
    let drop = dropped(&mut session, original.unique_id, 2);
    assert!(!payload(&drop).uid_assigned);
    assert!(unassigned(&payload(&drop).item));
    let state = decode(payload(&drop), &original.key, 2).unwrap();
    assert_eq!(state.unique_id, 0);
    assert_eq!(state.heal_hp, original.heal_hp);
    let remaining = session
        .app
        .world()
        .resource::<InventoryResource>()
        .inventory_items[0]
        .clone();
    assert_eq!(remaining.unique_id, original.unique_id);
    assert_eq!(remaining.quantity, 2);
    // Receiving into a different personal Session makes fresh allocation
    // observable rather than absorbing the split back into its original stack.
    let mut receiver = started(SimulationConfig::default());
    install(&mut receiver, instance("Dagger", 90_000, 1));
    let (mut zone, ticket) = shared_claim(&receiver, drop);
    assert!(
        receiver
            .commit_shared_ground_drop_pickup_transaction(&ticket.drop)
            .committed
    );
    zone.handle(ZoneCommand::CommitGroundDropClaimWithTicket {
        session_id: ticket.session_id.clone(),
        ticket,
    });
    let inventory = receiver.app.world().resource::<InventoryResource>();
    let received = inventory
        .inventory_items
        .iter()
        .chain(&inventory.belt_items)
        .find(|item| item.key == original.key)
        .unwrap();
    assert_ne!(received.unique_id, 0);
    assert_ne!(received.unique_id, original.unique_id);
    assert_eq!(received.quantity, 2);
    assert_eq!(received.heal_hp, original.heal_hp);
    assert_eq!(received.description, original.description);
}

#[test]
fn ground_custody_invalid_present_carriers_never_fall_back_or_mutate_either_custodian() {
    let original = recursive_instance();
    let valid = capture(&original, 1, true).unwrap();
    let key = original.key.clone();
    let mut variants = Vec::new();
    let mut bad = valid.clone();
    bad.server_state.as_mut().unwrap().version = 2;
    variants.push(bad);
    let mut bad = valid.clone();
    bad.server_state.as_mut().unwrap().state = json!({"notAnItem": true});
    variants.push(bad);
    let mut bad = valid.clone();
    bad.server_state.as_mut().unwrap().state["futureBinding"] = json!(true);
    variants.push(bad);
    let mut bad = valid.clone();
    bad.server_state.as_mut().unwrap().state["socketed"][0]["futureBinding"] = json!(true);
    variants.push(bad);
    let mut bad = valid.clone();
    bad.server_state
        .as_mut()
        .unwrap()
        .state
        .as_object_mut()
        .unwrap()
        .remove("grade");
    variants.push(bad);
    let mut bad = valid.clone();
    bad.server_state.as_mut().unwrap().state["description"] = json!("x".repeat(MAX_STATE_BYTES));
    variants.push(bad);
    let mut bad = valid.clone();
    bad.server_state.as_mut().unwrap().state["key"] = json!("red-potion");
    variants.push(bad);
    let mut bad = valid.clone();
    bad.server_state.as_mut().unwrap().state["quantity"] = json!(2);
    variants.push(bad);
    let mut bad = valid.clone();
    bad.item.unique_id += 1;
    variants.push(bad);
    let mut bad = valid.clone();
    bad.item.item_index += 1;
    variants.push(bad);
    let mut bad = valid.clone();
    bad.item.count += 1;
    variants.push(bad);
    let mut bad = valid.clone();
    bad.uid_assigned = false;
    variants.push(bad);
    for (index, bad) in variants.into_iter().enumerate() {
        let mut session = started(SimulationConfig::default());
        install(&mut session, original.clone());
        let mut drop = dropped(&mut session, original.unique_id, 1);
        *payload_mut(&mut drop) = bad;
        let before = serde_json::to_value(session.world_snapshot()).unwrap();
        let (mut zone, ticket) = shared_claim(&session, drop.clone());
        assert!(
            !session.can_commit_shared_ground_drop_pickup(&ticket.drop),
            "variant {index}"
        );
        assert!(
            !session
                .commit_shared_ground_drop_pickup_transaction(&ticket.drop)
                .committed,
            "variant {index}"
        );
        assert_eq!(
            serde_json::to_value(session.world_snapshot()).unwrap(),
            before,
            "variant {index}"
        );
        assert_eq!(ticket.drop, drop, "variant {index}");
        assert!(
            decode(payload(&ticket.drop), &key, 1).is_none(),
            "variant {index}"
        );
        cancel(&mut zone, ticket);
    }
}

#[test]
fn ground_custody_full_inventory_and_recursive_uid_collision_preserve_shared_source() {
    for collision in [false, true] {
        let mut session = started(SimulationConfig::default());
        let original = recursive_instance();
        install(&mut session, original.clone());
        let drop = dropped(&mut session, original.unique_id, 1);
        {
            let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
            if collision {
                inventory
                    .inventory_items
                    .push(instance("Dagger", 81_003, 1));
            } else {
                let slots = empty_slots_for_inventory_container(
                    &inventory.inventory_items,
                    ItemContainer::Bag1,
                    inventory.inventory_capacity,
                );
                for (index, (container, slot)) in slots.into_iter().enumerate() {
                    let mut filler = instance("Dagger", 100_000 + index as u64, 1);
                    filler.container = container;
                    filler.slot = slot;
                    inventory.inventory_items.push(filler);
                }
            }
        }
        let before = serde_json::to_value(session.world_snapshot()).unwrap();
        let (mut zone, ticket) = shared_claim(&session, drop.clone());
        assert!(
            !session
                .commit_shared_ground_drop_pickup_transaction(&ticket.drop)
                .committed
        );
        assert_eq!(
            serde_json::to_value(session.world_snapshot()).unwrap(),
            before
        );
        assert_eq!(ticket.drop, drop);
        cancel(&mut zone, ticket);
    }
}

#[test]
fn ground_custody_same_wire_different_server_identity_never_merges() {
    let template = crystal_item_template_for_item_key("red-potion").unwrap();
    let mut source = embedded_item_state_from_template(&template, ItemContainer::Bag1, 8);
    source.unique_id = 83_001;
    source.quantity = 1;
    source.heal_hp = 9001;
    let payload = capture(&source, 1, true).unwrap();
    let mut existing = source.clone();
    existing.unique_id = 83_002;
    existing.slot = 0;
    existing.heal_hp = 9002;
    let mut left = try_user_item_from_item_state(&source).unwrap();
    let mut right = try_user_item_from_item_state(&existing).unwrap();
    left.unique_id = 0;
    right.unique_id = 0;
    assert_eq!(left, right, "wire does not carry the differing heal stat");
    let mut world = World::new();
    let mut inventory = InventoryResource::new(40);
    inventory.inventory_items.push(existing.clone());
    world.insert_resource(inventory);
    let result = add_exact_ground_drop_items(
        &mut world,
        ItemContainer::Bag1,
        &source.key,
        "untrusted outer display name",
        "untrusted outer description",
        1,
        1,
        &payload,
    )
    .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].heal_hp, source.heal_hp);
    assert_eq!(result[0].name, source.name);
    assert_eq!(result[0].description, source.description);
    let inventory = world.resource::<InventoryResource>();
    assert_eq!(
        inventory.inventory_items.len() + inventory.belt_items.len(),
        2
    );
    let retained = inventory
        .inventory_items
        .iter()
        .find(|item| item.unique_id == existing.unique_id)
        .unwrap();
    assert_eq!(retained.quantity, 1);
    assert_eq!(retained.heal_hp, existing.heal_hp);
}

#[test]
fn ground_custody_unsafe_nested_partial_drop_rejects_before_inventory_removal() {
    let template = crystal_item_template_for_item_key("red-potion").unwrap();
    let mut base = embedded_item_state_from_template(&template, ItemContainer::Bag1, 8);
    base.unique_id = 84_001;
    base.quantity = 2;
    let mut wire = try_user_item_from_item_state(&base).unwrap();
    wire.slots = vec![Some(
        try_user_item_from_item_state(&instance("BronzeBell", 84_002, 1)).unwrap(),
    )];
    let source = try_item_state_from_user_item(base, &wire).unwrap();
    assert!(capture(&source, 1, false).is_none());
    let mut session = started(SimulationConfig::default());
    install(&mut session, source.clone());
    let before = serde_json::to_value(session.world_snapshot()).unwrap();
    assert!(session
        .shared_inventory_item_drop(source.unique_id, 1, false)
        .is_none());
    let packets = session.handle_packet(ClientPacket::DropItem {
        unique_id: source.unique_id,
        count: 1,
        hero_inventory: false,
    });
    assert!(!packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::DropItem { success: true, .. })));
    assert_eq!(
        serde_json::to_value(session.world_snapshot()).unwrap(),
        before
    );
}

#[test]
fn ground_custody_death_inventory_and_equipment_capture_full_state() {
    for equipped in [false, true] {
        let original = recursive_instance();
        let mut session = started(SimulationConfig::default());
        install(&mut session, original.clone());
        let expected = if equipped {
            let slot = original.equip_slot.expect("Dagger equipment slot");
            let equipment = equipment_state_from_item_state(&original, slot);
            let item = crate::runtime::equipment::item_state_from_equipment_state(
                equipment.clone(),
                ItemContainer::Bag1,
                0,
            );
            {
                let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
                inventory.inventory_items.clear();
                inventory.equipment_items.clear();
                inventory.equipment_items.push(equipment);
            }
            let packets = super::super::drop_player_death_equipment(
                session.app.world_mut(),
                equipment_slot_unique_id(slot).unwrap(),
            );
            assert!(packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::DeleteItem { count: 1, .. })));
            item
        } else {
            let packets = super::super::drop_player_death_inventory_item(
                session.app.world_mut(),
                original.unique_id,
            );
            assert!(packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::DeleteItem { count: 1, .. })));
            original.clone()
        };
        let drop = session
            .world_snapshot()
            .ground_drops
            .into_iter()
            .find(|drop| {
                matches!(
                    drop.loot,
                    GroundDropLootSnapshot::InventoryItem {
                        exact_item: Some(_),
                        ..
                    }
                )
            })
            .unwrap();
        let decoded = decode(payload(&drop), &expected.key, 1).unwrap();
        assert_eq!(functional_value(&decoded), functional_value(&expected));
        assert!(session
            .app
            .world()
            .resource::<InventoryResource>()
            .inventory_items
            .is_empty());
        if equipped {
            assert!(session
                .app
                .world()
                .resource::<InventoryResource>()
                .equipment_items
                .is_empty());
        }
    }
}

#[test]
fn ground_custody_failed_capture_keeps_normal_and_death_sources_unchanged() {
    let mut original = recursive_instance();
    original.description = "x".repeat(MAX_STATE_BYTES);
    assert!(capture(&original, 1, true).is_none());
    let mut session = started(SimulationConfig::default());
    install(&mut session, original.clone());
    assert!(session
        .shared_inventory_item_drop(original.unique_id, 1, false)
        .is_none());
    let before = serde_json::to_value(session.world_snapshot()).unwrap();
    let normal = session.handle_packet(ClientPacket::DropItem {
        unique_id: original.unique_id,
        count: 1,
        hero_inventory: false,
    });
    assert!(!normal
        .iter()
        .any(|packet| matches!(packet, ServerPacket::DropItem { success: true, .. })));
    assert_eq!(
        serde_json::to_value(session.world_snapshot()).unwrap(),
        before
    );
    assert!(super::super::drop_player_death_inventory_item(
        session.app.world_mut(),
        original.unique_id
    )
    .is_empty());
    assert_eq!(
        serde_json::to_value(session.world_snapshot()).unwrap(),
        before
    );

    let slot = original.equip_slot.unwrap();
    let equipment = equipment_state_from_item_state(&original, slot);
    {
        let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
        inventory.inventory_items.clear();
        inventory.equipment_items.clear();
        inventory.equipment_items.push(equipment);
    }
    let before = serde_json::to_value(session.world_snapshot()).unwrap();
    assert!(super::super::drop_player_death_equipment(
        session.app.world_mut(),
        equipment_slot_unique_id(slot).unwrap()
    )
    .is_empty());
    assert_eq!(
        serde_json::to_value(session.world_snapshot()).unwrap(),
        before
    );
}

#[test]
fn ground_custody_none_keeps_legacy_payload_bytes_and_legacy_pickup() {
    let original = instance("Dagger", 85_001, 1);
    let item = try_user_item_from_item_state(&original).unwrap();
    let old = json!({"item": item, "uidAssigned": true});
    let decoded: GroundDropItemPayload = serde_json::from_value(old.clone()).unwrap();
    assert!(decoded.server_state.is_none());
    assert_eq!(serde_json::to_value(&decoded).unwrap(), old);
    let mut world = World::new();
    world.insert_resource(InventoryResource::new(40));
    assert!(can_gain_exact_ground_drop_item(
        &world,
        ItemContainer::Bag1,
        &original.key,
        &original.name,
        &original.description,
        0,
        1,
        &decoded
    ));
    let mut unknown = serde_json::to_value(capture(&original, 1, true).unwrap()).unwrap();
    unknown["serverState"]["unknownVersionField"] = json!(true);
    assert!(serde_json::from_value::<GroundDropItemPayload>(unknown).is_err());
}
