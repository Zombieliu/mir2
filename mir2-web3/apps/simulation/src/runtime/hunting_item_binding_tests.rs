use super::*;
use crate::config::{ItemContainer, SimulationConfig};
use crate::runtime::components::{
    entity_position, player_entity, DisplayName, Facing, Npc, NpcAgent, ObjectId, Position,
    WorldObject,
};
use crate::runtime::equipment::{
    equip_item_impl, equipment_slot_index, equipment_state_from_item_state,
    item_state_from_equipment_state, remove_equipped_item_impl,
};
use crate::runtime::inventory::{
    add_or_increment_item, empty_slots_for_inventory_container, gain_hunting_bound_item,
    item_stack_identity_compatible, split_item_impl, store_item_impl, take_back_item_impl,
};
use crate::runtime::items::{
    crystal_item_template_for_item_key, embedded_item_state_from_template,
    try_item_state_from_user_item, try_user_item_from_item_state, use_item,
};
use crate::runtime::npc::ActiveNpcServiceState;
use crate::runtime::resources::{NpcStateResource, PotionRecoveryResource};
use crate::runtime::session::SimulationSession;
use mir2_protocol::{ClientPacket, MirDirection, MirGridType, ServerPacket};
use serde_json::{json, Value};

const INCARNATION: &str = "pg-character-v1:12345678-1234-4234-9234-123456789abc";

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
    assert!(session.active_zone_join_snapshot("binding-core").is_some());
    // Only the core owner witness is controlled here. These tests neither
    // issue a real PG incarnation nor claim a quest; Root tests the PG source.
    session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config
        .hunting_economy_realm = Some("binding-core".into());
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .hunting_character_incarnation = Some(INCARNATION.into());
    let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
    inventory.inventory_items.clear();
    inventory.belt_items.clear();
    inventory.storage_items.clear();
    inventory.equipment_items.clear();
    drop(inventory);
    session
}

fn marker(world: &World) -> HuntingItemBinding {
    HuntingItemBinding::for_current_owner(world, "quest-once-v1:2110001").unwrap()
}

fn item(key: &str, uid: u64, slot: u8, quantity: u32) -> ItemState {
    let template = crystal_item_template_for_item_key(key).unwrap();
    let mut item = embedded_item_state_from_template(&template, ItemContainer::Bag1, slot);
    item.key = key.into();
    item.unique_id = uid;
    item.quantity = quantity;
    item
}

fn grant(
    world: &mut World,
    key: &str,
    quantity: u32,
    binding: &HuntingItemBinding,
) -> Option<Vec<ItemState>> {
    let template = crystal_item_template_for_item_key(key).unwrap();
    gain_hunting_bound_item(
        world,
        key,
        &template.name,
        template.tooltip.as_deref().unwrap_or(""),
        quantity,
        u16::from(template.weight),
        binding,
    )
}

fn inventory_json(world: &World) -> Value {
    let inventory = world.resource::<InventoryResource>();
    json!({"bag": inventory.inventory_items, "belt": inventory.belt_items,
        "storage": inventory.storage_items, "equipment": inventory.equipment_items,
        "reserved": inventory.reserved_item_unique_ids})
}

fn storage(session: &mut SimulationSession) {
    let player = player_entity(session.app.world()).unwrap();
    let position = entity_position(session.app.world(), player).unwrap();
    session.app.world_mut().spawn((
        WorldObject,
        Npc,
        ObjectId(98_005),
        DisplayName::literal("Binding storage fixture"),
        Position(position),
        Facing(MirDirection::Left),
        NpcAgent {
            image: 5,
            colour_argb: -1,
            quest_ids: Vec::new(),
            script_key: Some("00Default".into()),
        },
    ));
    session
        .app
        .world_mut()
        .resource_mut::<NpcStateResource>()
        .active_npc_service = Some(ActiveNpcServiceState {
        npc_object_id: 98_005,
        script_key: "00Default".into(),
        label_key: "STORAGE".into(),
    });
}

#[test]
fn hunting_item_binding_strict_marker_and_legacy_shape() {
    let session = session();
    let good = serde_json::to_value(marker(session.app.world())).unwrap();
    for (field, value) in [
        ("version", json!(2)),
        ("version", json!(0)),
        ("index", json!(-1)),
        ("realm", json!("")),
        ("account", json!(" demo")),
        ("incarnation", json!("12345678-1234-4234-9234-123456789abc")),
        (
            "incarnation",
            json!("pg-character-v1:00000000-0000-0000-0000-000000000000"),
        ),
        (
            "incarnation",
            json!("pg-character-v1:12345678-1234-4234-9234-123456789ABc"),
        ),
        ("sourceRef", json!("client:2110001")),
        ("sourceRef", json!("quest-once-v1:0")),
        ("sourceRef", json!("quest-once-v1:1::x")),
        ("sourceRef", json!("quest-once-v1:1/../../")),
        ("sourceRef", json!("x".repeat(513))),
        ("futureField", json!(true)),
    ] {
        let mut value_good = good.clone();
        value_good[field] = value;
        assert!(
            serde_json::from_value::<HuntingItemBinding>(value_good).is_err(),
            "{field}"
        );
    }
    for field in [
        "version",
        "realm",
        "account",
        "index",
        "incarnation",
        "sourceRef",
    ] {
        let mut missing = good.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<HuntingItemBinding>(missing).is_err());
    }
    let mut duplicate = serde_json::to_string(&good).unwrap();
    duplicate.insert_str(1, "\"version\":1,");
    assert!(serde_json::from_str::<HuntingItemBinding>(&duplicate).is_err());
    let old = item("red-potion", 100, 8, 3);
    let old_value = serde_json::to_value(&old).unwrap();
    assert!(old_value.get("economic_binding").is_none());
    let decoded: ItemState = serde_json::from_value(old_value.clone()).unwrap();
    assert!(decoded.economic_binding.is_none());
    assert_eq!(serde_json::to_value(decoded).unwrap(), old_value);
}

#[test]
fn hunting_item_binding_owner_witness_is_required_even_when_policy_is_disabled() {
    let mut session = session();
    let binding = marker(session.app.world());
    let mut bound = item("red-potion", 101, 8, 1);
    bound.economic_binding = Some(binding.clone());
    assert!(belongs_to_current_owner(session.app.world(), &bound));
    // The default simulation has no enabled economic policy; the identity
    // nevertheless remains enforced without consulting that policy.
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .hunting_character_incarnation = None;
    assert!(!belongs_to_current_owner(session.app.world(), &bound));
    assert!(
        HuntingItemBinding::for_current_owner(session.app.world(), "quest-once-v1:2110001")
            .is_err()
    );
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .hunting_character_incarnation =
        Some("pg-character-v1:12345678-1234-4234-9234-123456789abd".into());
    assert!(!belongs_to_current_owner(session.app.world(), &bound));
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .hunting_character_incarnation = Some(INCARNATION.into());
    session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config
        .hunting_economy_realm = Some("other-realm".into());
    assert!(!belongs_to_current_owner(session.app.world(), &bound));
    assert!(belongs_to_current_owner(
        session.app.world(),
        &item("red-potion", 102, 9, 1)
    ));
}

#[test]
fn hunting_item_binding_grants_separate_legacy_and_exact_binding_identities() {
    let mut session = session();
    let binding = marker(session.app.world());
    grant(session.app.world_mut(), "red-potion", 3, &binding).unwrap();
    grant(session.app.world_mut(), "red-potion", 2, &binding).unwrap();
    let template = crystal_item_template_for_item_key("red-potion").unwrap();
    add_or_increment_item(
        session.app.world_mut(),
        ItemContainer::Bag1,
        "red-potion",
        &template.name,
        template.tooltip.as_deref().unwrap_or(""),
        0,
        4,
        u16::from(template.weight),
    );
    let other = HuntingItemBinding::for_current_owner(
        session.app.world(),
        "quest-once-v1:2110001:reward:other",
    )
    .unwrap();
    grant(session.app.world_mut(), "red-potion", 1, &other).unwrap();
    let inventory = session.app.world().resource::<InventoryResource>();
    let all = inventory
        .inventory_items
        .iter()
        .chain(&inventory.belt_items)
        .collect::<Vec<_>>();
    assert_eq!(
        all.iter()
            .filter(|item| item.economic_binding == Some(binding.clone()))
            .map(|item| item.quantity)
            .sum::<u32>(),
        5
    );
    assert_eq!(
        all.iter()
            .filter(|item| item.economic_binding.is_none())
            .map(|item| item.quantity)
            .sum::<u32>(),
        4
    );
    assert_eq!(
        all.iter()
            .filter(|item| item.economic_binding == Some(other.clone()))
            .map(|item| item.quantity)
            .sum::<u32>(),
        1
    );
    let a = all
        .iter()
        .find(|item| item.economic_binding == Some(binding.clone()))
        .unwrap();
    let b = all
        .iter()
        .find(|item| item.economic_binding == Some(other.clone()))
        .unwrap();
    let mut aw = try_user_item_from_item_state(a).unwrap();
    let mut bw = try_user_item_from_item_state(b).unwrap();
    aw.unique_id = 0;
    bw.unique_id = 0;
    aw.count = 0;
    bw.count = 0;
    assert_eq!(aw, bw);
    assert!(!item_stack_identity_compatible(a, b));
}

#[test]
fn hunting_item_binding_full_capacity_rolls_back_staged_partial_merges() {
    let mut session = session();
    let binding = marker(session.app.world());
    let template = crystal_item_template_for_item_key("red-potion").unwrap();
    let limit = u32::from(template.stack_size.max(1));
    assert!(limit > 2);
    grant(session.app.world_mut(), "red-potion", limit - 1, &binding).unwrap();
    {
        let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
        let slots = empty_slots_for_inventory_container(
            &inventory.inventory_items,
            ItemContainer::Bag1,
            inventory.inventory_capacity,
        );
        for (i, (container, slot)) in slots.into_iter().enumerate() {
            let mut filler = item("dagger", 5000 + i as u64, slot, 1);
            filler.container = container;
            inventory.inventory_items.push(filler);
        }
        for slot in 0..6 {
            if inventory.belt_items.iter().all(|item| item.slot != slot) {
                let mut filler = item("red-potion", 6000 + u64::from(slot), slot, limit);
                filler.container = ItemContainer::Belt;
                inventory.belt_items.push(filler);
            }
        }
    }
    let before = inventory_json(session.app.world());
    assert!(grant(session.app.world_mut(), "red-potion", 2, &binding).is_none());
    assert_eq!(inventory_json(session.app.world()), before);
}

#[test]
fn hunting_item_binding_equipment_roundtrip_keeps_tag_and_rejects_wrong_owner() {
    let mut session = session();
    let binding = marker(session.app.world());
    let sword = grant(session.app.world_mut(), "wooden-sword", 1, &binding)
        .unwrap()
        .pop()
        .unwrap();
    let slot = sword.equip_slot.unwrap();
    let to = equipment_slot_index(slot).unwrap() as i32;
    let packets = equip_item_impl(
        session.app.world_mut(),
        MirGridType::Inventory,
        sword.unique_id,
        to,
    );
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::EquipItem { success: true, .. })),
        "{packets:?}"
    );
    let equipped = session
        .app
        .world()
        .resource::<InventoryResource>()
        .equipment_items[0]
        .clone();
    assert_eq!(equipped.economic_binding, Some(binding.clone()));
    assert!(equipment_has_bound(&equipped));
    let display = crate::runtime::equipment::user_item_from_equipment_state(&equipped).unwrap();
    assert_eq!(display.unique_id, sword.unique_id);
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .hunting_character_incarnation = None;
    let before = inventory_json(session.app.world());
    assert!(remove_equipped_item_impl(
        session.app.world_mut(),
        MirGridType::Inventory,
        sword.unique_id,
        8
    )
    .iter()
    .all(|packet| !matches!(packet, ServerPacket::RemoveItem { success: true, .. })));
    assert_eq!(inventory_json(session.app.world()), before);
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .hunting_character_incarnation = Some(INCARNATION.into());
    assert!(remove_equipped_item_impl(
        session.app.world_mut(),
        MirGridType::Inventory,
        sword.unique_id,
        8
    )
    .iter()
    .any(|packet| matches!(packet, ServerPacket::RemoveItem { success: true, .. })));
    let restored = &session
        .app
        .world()
        .resource::<InventoryResource>()
        .inventory_items[0];
    assert_eq!(restored.economic_binding, Some(binding));
}

#[test]
fn hunting_item_binding_split_and_personal_storage_preserve_tag_with_unique_ids() {
    let mut session = session();
    let binding = marker(session.app.world());
    let mut potion = grant(session.app.world_mut(), "red-potion", 6, &binding)
        .unwrap()
        .pop()
        .unwrap();
    potion.container = ItemContainer::Bag1;
    potion.slot = 8;
    {
        let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
        inventory.belt_items.clear();
        inventory.inventory_items = vec![potion.clone()];
    }
    assert!(split_item_impl(
        session.app.world_mut(),
        MirGridType::Inventory,
        potion.unique_id,
        2
    )
    .iter()
    .any(|packet| matches!(packet, ServerPacket::SplitItem1 { success: true, .. })));
    storage(&mut session);
    assert!(store_item_impl(session.app.world_mut(), 8, 0)
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StoreItem { success: true, .. })));
    let stored = session
        .app
        .world()
        .resource::<InventoryResource>()
        .storage_items[0]
        .clone();
    assert!(split_item_impl(
        session.app.world_mut(),
        MirGridType::Storage,
        stored.unique_id,
        1
    )
    .iter()
    .any(|packet| matches!(packet, ServerPacket::SplitItem1 { success: true, .. })));
    assert!(take_back_item_impl(session.app.world_mut(), 0, 12)
        .iter()
        .any(|packet| matches!(packet, ServerPacket::TakeBackItem { success: true, .. })));
    let inventory = session.app.world().resource::<InventoryResource>();
    let all = inventory
        .inventory_items
        .iter()
        .chain(&inventory.belt_items)
        .chain(&inventory.storage_items)
        .collect::<Vec<_>>();
    assert_eq!(all.iter().map(|item| item.quantity).sum::<u32>(), 6);
    assert!(all
        .iter()
        .all(|item| item.economic_binding == Some(binding.clone())));
    let ids = all
        .iter()
        .map(|item| item.unique_id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(ids.len(), all.len());
}

#[test]
fn hunting_item_binding_ordinary_potion_self_use_keeps_enforcement_without_policy() {
    let mut session = session();
    let binding = marker(session.app.world());
    let potion = grant(session.app.world_mut(), "red-potion", 2, &binding)
        .unwrap()
        .pop()
        .unwrap();
    let grid = if potion.container == ItemContainer::Belt {
        MirGridType::Belt
    } else {
        MirGridType::Inventory
    };
    let packets = use_item(
        session.app.world_mut(),
        &potion.key,
        Some((potion.unique_id, grid)),
    );
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::UseItem { success: true, .. })),
        "{packets:?}"
    );
    assert!(
        session
            .app
            .world()
            .resource::<PotionRecoveryResource>()
            .pending_pot_health_amount
            > 0
    );
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .hunting_character_incarnation = None;
    let before = inventory_json(session.app.world());
    let packets = use_item(
        session.app.world_mut(),
        &potion.key,
        Some((potion.unique_id, grid)),
    );
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::UseItem { success: false, .. })));
    assert_eq!(inventory_json(session.app.world()), before);
}

#[test]
fn hunting_item_binding_wire_hydration_preserves_root_but_refuses_bound_children() {
    let session = session();
    let binding = marker(session.app.world());
    let mut root = item("dagger", 7001, 8, 1);
    root.economic_binding = Some(binding.clone());
    let wire = try_user_item_from_item_state(&root).unwrap();
    let hydrated = try_item_state_from_user_item(root.clone(), &wire).unwrap();
    assert_eq!(hydrated.economic_binding, Some(binding.clone()));
    let child = item("dagger", 7002, 9, 1);
    let mut wire = wire;
    wire.slots = vec![Some(try_user_item_from_item_state(&child).unwrap())];
    let mut tree = try_item_state_from_user_item(root, &wire).unwrap();
    tree.socketed[0].economic_binding = Some(binding);
    assert!(has_bound(&tree));
    assert_eq!(
        try_user_item_from_item_state(&tree).unwrap(),
        wire,
        "display conversion remains available"
    );
    assert!(try_item_state_from_user_item(tree.clone(), &wire).is_err());
    let equipment = equipment_state_from_item_state(&tree, tree.equip_slot.unwrap());
    assert!(equipment_has_bound(&equipment));
    let returned = item_state_from_equipment_state(equipment, ItemContainer::Bag1, 8);
    assert!(has_bound(&returned));
}

#[test]
fn hunting_item_binding_same_owner_hero_moves_keep_tag_and_wrong_owner_is_refused() {
    let mut session = session();
    // Establish a real ordinary Hero before installing a controlled PG owner
    // witness for the core-only marked-instance tests.
    session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config
        .hunting_economy_realm = None;
    let created = session.handle_packet(ClientPacket::NewHero {
        name: "BindingAide".into(),
        class: mir2_protocol::MirClass::Warrior,
        gender: mir2_protocol::MirGender::Male,
    });
    assert!(
        crate::runtime::components::hero_entity(session.app.world()).is_some(),
        "{created:?}"
    );
    session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config
        .hunting_economy_realm = Some("binding-core".into());
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .hunting_character_incarnation = Some(INCARNATION.into());
    let binding = marker(session.app.world());
    let mut sword = item("wooden-sword", 8001, 2, 1);
    sword.economic_binding = Some(binding.clone());
    session
        .app
        .world_mut()
        .resource_mut::<crate::runtime::resources::HeroInventoryResource>()
        .items = vec![sword];
    let moved = crate::runtime::hero_inventory::move_item(session.app.world_mut(), 2, 3);
    assert!(moved
        .iter()
        .any(|packet| matches!(packet, ServerPacket::MoveItem { success: true, .. })));
    let hero = session
        .app
        .world()
        .resource::<crate::runtime::resources::HeroInventoryResource>();
    assert_eq!(hero.items[0].economic_binding, Some(binding));
    let before = serde_json::to_value(&hero.items).unwrap();
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .hunting_character_incarnation = None;
    let refused = crate::runtime::hero_inventory::move_item(session.app.world_mut(), 3, 4);
    assert!(refused
        .iter()
        .any(|packet| matches!(packet, ServerPacket::MoveItem { success: false, .. })));
    assert_eq!(
        serde_json::to_value(
            &session
                .app
                .world()
                .resource::<crate::runtime::resources::HeroInventoryResource>()
                .items
        )
        .unwrap(),
        before
    );
}

fn registry_hero(item: &ItemState) -> crate::config::SharedHeroRecord {
    crate::config::SharedHeroRecord {
        id: 1,
        revision: 1,
        custody: crate::config::SharedHeroCustody::Attached {
            account_id: "demo".into(),
            character_index: 0,
            slot: 0,
            attachment_revision: 1,
        },
        state: crate::config::SharedHeroState {
            name: "Binding registry fixture".into(),
            class: mir2_protocol::MirClass::Warrior,
            gender: mir2_protocol::MirGender::Male,
            hair: 0,
            grade: 0,
            level: 1,
            experience: 0,
            inventory_items_json: vec![serde_json::to_string(item).unwrap()],
            equipment_items_json: Vec::new(),
            inventory_capacity: 10,
            inventory_legacy_40: false,
            vitals: None,
            magics: Vec::new(),
            seal_count: 0,
            auto_pot: false,
            auto_hp_percent: 0,
            auto_mp_percent: 0,
            hp_item_index: 0,
            mp_item_index: 0,
        },
    }
}

#[test]
fn hunting_item_binding_registry_refuses_sealed_shell_with_bound_inventory_or_equipment() {
    let session = session();
    let binding = marker(session.app.world());
    let mut item = item("wooden-sword", 9001, 2, 1);
    let mut hero = registry_hero(&item);
    hero.custody = crate::config::SharedHeroCustody::Sealed { carrier_uid: 9901 };
    crate::runtime::validate_shared_hero_carriers(&hero.state).unwrap();
    crate::runtime::validate_hunting_hero_binding_custody(&hero).unwrap();
    item.economic_binding = Some(binding);
    for equipment in [false, true] {
        hero.state.inventory_items_json.clear();
        hero.state.equipment_items_json.clear();
        let mut physical = item.clone();
        if equipment {
            physical.slot = 0;
            hero.state
                .equipment_items_json
                .push(serde_json::to_string(&physical).unwrap());
        } else {
            hero.state
                .inventory_items_json
                .push(serde_json::to_string(&physical).unwrap());
        }
        crate::runtime::validate_shared_hero_carriers(&hero.state).unwrap();
        let before = serde_json::to_value(&hero).unwrap();
        assert!(crate::runtime::validate_hunting_hero_binding_custody(&hero)
            .unwrap_err()
            .contains("sealed Hero"));
        assert_eq!(serde_json::to_value(&hero).unwrap(), before);
    }
}

#[test]
fn hunting_item_binding_registry_refuses_hidden_bound_child_and_cross_character_attach() {
    let session = session();
    let mut child = item("dagger", 9102, 0, 1);
    let root = item("dagger", 9101, 2, 1);
    let mut wire = try_user_item_from_item_state(&root).unwrap();
    wire.slots = vec![Some(try_user_item_from_item_state(&child).unwrap())];
    let mut root = try_item_state_from_user_item(root, &wire).unwrap();
    child.economic_binding = Some(marker(session.app.world()));
    root.socketed[0].economic_binding = child.economic_binding;
    let mut hero = registry_hero(&root);
    crate::runtime::validate_shared_hero_carriers(&hero.state).unwrap();
    crate::runtime::validate_hunting_hero_binding_custody(&hero).unwrap();
    for (account, index) in [("peer", 0), ("demo", 1)] {
        hero.custody = crate::config::SharedHeroCustody::Attached {
            account_id: account.into(),
            character_index: index,
            slot: 0,
            attachment_revision: 1,
        };
        assert!(crate::runtime::validate_hunting_hero_binding_custody(&hero).is_err());
    }
    hero.custody = crate::config::SharedHeroCustody::Sealed { carrier_uid: 9902 };
    assert!(crate::runtime::validate_hunting_hero_binding_custody(&hero).is_err());
    hero.custody = crate::config::SharedHeroCustody::Released;
    crate::runtime::validate_hunting_hero_binding_custody(&hero).unwrap();
}

#[test]
fn hunting_item_binding_registry_attachment_is_not_a_realm_or_incarnation_grant() {
    let session = session();
    let mut tagged = item("red-potion", 9201, 2, 1);
    let mut binding = serde_json::to_value(marker(session.app.world())).unwrap();
    binding["realm"] = json!("other-realm");
    binding["incarnation"] = json!("pg-character-v1:12345678-1234-4234-9234-123456789abd");
    tagged.economic_binding = Some(serde_json::from_value(binding).unwrap());
    let hero = registry_hero(&tagged);
    crate::runtime::validate_shared_hero_carriers(&hero.state).unwrap();
    // Registry storage knows only Attached account/index, not the live PG
    // realm or UUID. A successful structural check never grants live use.
    crate::runtime::validate_hunting_hero_binding_custody(&hero).unwrap();
    assert!(!belongs_to_current_owner(session.app.world(), &tagged));
}

fn load_snapshot_with_item(
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
        0 => save.inventory_items_json = vec![encoded],
        1 => save.belt_items_json = vec![encoded],
        2 => save.storage_items_json = vec![encoded],
        3 => save.hero_inventory_items_json = vec![encoded],
        4 => save.hero_equipment_items_json = vec![encoded],
        5 => {
            save.equipment_items_json = vec![serde_json::to_string(
                &equipment_state_from_item_state(item, crate::EquipmentSlot::Weapon),
            )
            .unwrap()]
        }
        _ => panic!("unknown load fixture collection"),
    }
    save
}

#[test]
fn hunting_item_binding_character_load_uses_verified_actor_without_selected_session() {
    let mut session = session();
    let mut bound = item("wooden-sword", 9301, 8, 1);
    bound.economic_binding = Some(marker(session.app.world()));
    let saves: Vec<_> = (0..6)
        .map(|collection| load_snapshot_with_item(&session, &bound, collection))
        .collect();
    // The core fixture has no enabled DB policy. Identity checks must remain
    // independent of it and of the previous actor still held by the World.
    session.app.world_mut().remove_resource::<SessionResource>();
    for save in saves {
        validate_character_load(session.app.world(), &save, "demo", Some(INCARNATION)).unwrap();
    }
}

#[test]
fn hunting_item_binding_character_load_refuses_foreign_or_missing_realm_for_every_collection() {
    let mut session = session();
    let mut bound = item("wooden-sword", 9401, 8, 1);
    bound.economic_binding = Some(marker(session.app.world()));
    let saves: Vec<_> = (0..6)
        .map(|collection| load_snapshot_with_item(&session, &bound, collection))
        .collect();
    for realm in [Some("other-realm".to_string()), None] {
        session
            .app
            .world_mut()
            .resource_mut::<RuntimeConfigResource>()
            .config
            .hunting_economy_realm = realm;
        for save in &saves {
            let before = serde_json::to_value(save).unwrap();
            assert!(
                validate_character_load(session.app.world(), save, "demo", Some(INCARNATION))
                    .is_err()
            );
            assert_eq!(serde_json::to_value(save).unwrap(), before);
        }
    }
}

#[test]
fn hunting_item_binding_character_load_refuses_wrong_identity_and_hidden_bound_children() {
    let session = session();
    let mut bound = item("wooden-sword", 9501, 8, 1);
    bound.economic_binding = Some(marker(session.app.world()));
    for collection in 0..6 {
        let mut save = load_snapshot_with_item(&session, &bound, collection);
        for (account, witness) in [
            ("peer", Some(INCARNATION)),
            ("demo", None),
            (
                "demo",
                Some("pg-character-v1:12345678-1234-4234-9234-123456789abd"),
            ),
        ] {
            assert!(validate_character_load(session.app.world(), &save, account, witness).is_err());
        }
        save.character.index = 1;
        assert!(
            validate_character_load(session.app.world(), &save, "demo", Some(INCARNATION)).is_err()
        );
    }
    let root = item("dagger", 9510, 8, 1);
    let child = item("dagger", 9511, 0, 1);
    let mut wire = try_user_item_from_item_state(&root).unwrap();
    wire.slots = vec![Some(try_user_item_from_item_state(&child).unwrap())];
    let mut root = try_item_state_from_user_item(root, &wire).unwrap();
    let mut foreign = serde_json::to_value(marker(session.app.world())).unwrap();
    foreign["account"] = json!("peer");
    root.socketed[0].economic_binding = Some(serde_json::from_value(foreign).unwrap());
    assert!(root.economic_binding.is_none());
    for collection in 0..6 {
        let save = load_snapshot_with_item(&session, &root, collection);
        assert!(
            validate_character_load(session.app.world(), &save, "demo", Some(INCARNATION)).is_err()
        );
    }
}

#[test]
fn hunting_item_binding_character_load_keeps_legacy_none_and_rejects_overbudget_trees() {
    let mut session = session();
    let legacy = item("wooden-sword", 9601, 8, 1);
    let saves: Vec<_> = (0..6)
        .map(|collection| load_snapshot_with_item(&session, &legacy, collection))
        .collect();
    session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config
        .hunting_economy_realm = None;
    session.app.world_mut().remove_resource::<SessionResource>();
    for save in &saves {
        validate_character_load(session.app.world(), save, "legacy-account", None).unwrap();
    }
    let mut deep = legacy.clone();
    for _ in 0..9 {
        let mut parent = legacy.clone();
        parent.socketed = vec![deep];
        deep = parent;
    }
    let mut save = saves[0].clone();
    save.inventory_items_json = vec![serde_json::to_string(&deep).unwrap()];
    assert!(
        validate_character_load(session.app.world(), &save, "legacy-account", None)
            .unwrap_err()
            .contains("budget")
    );
    let mut wide = legacy.clone();
    wide.socketed = vec![legacy; 256];
    save.inventory_items_json = vec![serde_json::to_string(&wide).unwrap()];
    assert!(
        validate_character_load(session.app.world(), &save, "legacy-account", None)
            .unwrap_err()
            .contains("budget")
    );
}
