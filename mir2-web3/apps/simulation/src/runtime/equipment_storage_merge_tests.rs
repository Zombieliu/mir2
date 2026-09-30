use super::components::{
    CharacterBody, DisplayName, Facing, Npc, NpcAgent, ObjectId, Position, WorldObject,
    entity_by_object_id, entity_position, player_entity,
};
use super::crystal_compat::{BASE_STORAGE_SLOTS, CRYSTAL_BIND_DONT_STORE};
use super::equipment::{equipment_slot_index, user_item_from_equipment_state};
use super::items::{
    crystal_item_key_for_template, crystal_stack_size_for_item_key,
    embedded_item_state_from_template, item_unique_id,
};
use super::npc::{ActiveNpcServiceState, active_crystal_storage_service};
use super::resources::{InventoryResource, NpcStateResource, SessionResource};
use super::session::SimulationSession;
use super::stats::{player_stats, refresh_player_stats};
use crate::{EquipmentSlot, ItemContainer, SimulationConfig};
use mir2_protocol::{ClientPacket, MirDirection, MirGridType, ServerPacket};

const STORAGE_NPC_OBJECT_ID: u32 = 4_991;

fn authenticated_session() -> SimulationSession {
    let mut session = SimulationSession::new(SimulationConfig::default());
    let login = session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    let character_index = login
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::LoginSuccess { characters } => {
                characters.first().map(|character| character.index)
            }
            _ => None,
        })
        .expect("demo login should expose a character");
    assert!(
        session
            .handle_packet(ClientPacket::StartGame { character_index })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. }))
    );
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .selected_character
        .as_mut()
        .expect("started character")
        .level = 55;
    let player = player_entity(session.app.world()).expect("player entity");
    session
        .app
        .world_mut()
        .entity_mut(player)
        .get_mut::<CharacterBody>()
        .expect("player character body")
        .level = 55;
    refresh_player_stats(session.app.world_mut());
    session
}

fn activate_storage_service(session: &mut SimulationSession) {
    if entity_by_object_id(session.app.world(), STORAGE_NPC_OBJECT_ID).is_none() {
        let player = player_entity(session.app.world()).expect("player entity");
        let player_position =
            entity_position(session.app.world(), player).expect("player position");
        session.app.world_mut().spawn((
            WorldObject,
            Npc,
            ObjectId(STORAGE_NPC_OBJECT_ID),
            DisplayName::literal("Warehouse Keeper"),
            Position(player_position),
            Facing(MirDirection::Left),
            NpcAgent {
                image: 5,
                colour_argb: -1,
                quest_ids: Vec::new(),
                script_key: Some("BichonProvince/Warehouse-D002".to_string()),
            },
        ));
    }
    session
        .app
        .world_mut()
        .resource_mut::<NpcStateResource>()
        .active_npc_service = Some(ActiveNpcServiceState {
        script_key: "00Default".to_string(),
        label_key: "STORAGE".to_string(),
        npc_object_id: STORAGE_NPC_OBJECT_ID,
    });
    assert!(active_crystal_storage_service(session.app.world()));
}

fn merge_succeeded(packets: &[ServerPacket]) -> bool {
    packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::MergeItem { success: true, .. }))
}

fn merge_failed(packets: &[ServerPacket]) -> bool {
    packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::MergeItem { success: false, .. }))
}

fn equipped_amulet_and_target(
    session: &mut SimulationSession,
    target_container: ItemContainer,
    target_slot: u8,
    equipped_quantity: u32,
    target_quantity: u32,
    added_attack: i32,
) -> (u64, u64, u32) {
    let template = mir2_game_data::crystal_item_by_name("Amulet")
        .expect("Crystal Amulet template should exist");
    let key = crystal_item_key_for_template(&template);
    let mut source = embedded_item_state_from_template(&template, ItemContainer::Bag1, 35);
    source.unique_id = 91_001;
    source.quantity = equipped_quantity;
    source.added_attack = added_attack;
    let source_unique_id = source.unique_id;
    let max_stack = crystal_stack_size_for_item_key(&key);

    {
        let mut resources = session.app.world_mut().resource_mut::<InventoryResource>();
        let mut target = source.clone();
        target.container = target_container;
        target.slot = target_slot;
        target.unique_id = 90_000 + u64::from(target_slot);
        target.quantity = target_quantity;
        resources.inventory_items.push(source);
        match target_container {
            ItemContainer::Bag1 | ItemContainer::Bag2 => resources.inventory_items.push(target),
            ItemContainer::Storage => resources.storage_items.push(target),
            _ => panic!("test target must be inventory or storage"),
        }
    }

    let equipment_slot =
        equipment_slot_index(EquipmentSlot::Amulet).expect("Amulet slot index") as i32;
    assert!(
        session
            .handle_packet(ClientPacket::EquipItem {
                grid: MirGridType::Inventory,
                unique_id: source_unique_id,
                to: equipment_slot,
            })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::EquipItem { success: true, .. }))
    );

    let resources = session.app.world().resource::<InventoryResource>();
    let equipment = resources
        .equipment_items
        .iter()
        .find(|item| item.slot == EquipmentSlot::Amulet)
        .expect("equipped Amulet");
    let equipment_unique_id = user_item_from_equipment_state(equipment)
        .map(|item| item.unique_id)
        .expect("equipped Amulet should retain its exact UID");
    (
        equipment_unique_id,
        90_000 + u64::from(target_slot),
        max_stack,
    )
}

fn item_quantity(
    session: &SimulationSession,
    container: ItemContainer,
    unique_id: u64,
) -> Option<u32> {
    let resources = session.app.world().resource::<InventoryResource>();
    let items = match container {
        ItemContainer::Bag1 | ItemContainer::Bag2 => &resources.inventory_items,
        ItemContainer::Storage => &resources.storage_items,
        _ => return None,
    };
    items
        .iter()
        .find(|item| item.container == container && item_unique_id(item) == unique_id)
        .map(|item| item.quantity)
}

fn equipped_amulet_quantity(session: &SimulationSession) -> Option<u32> {
    session
        .app
        .world()
        .resource::<InventoryResource>()
        .equipment_items
        .iter()
        .find(|item| item.slot == EquipmentSlot::Amulet)
        .map(|item| item.quantity)
}

#[test]
fn equipped_amulet_partially_merges_into_inventory_without_retiring_equipment() {
    let mut session = authenticated_session();
    let (equipment_id, inventory_id, max_stack) =
        equipped_amulet_and_target(&mut session, ItemContainer::Bag1, 2, 5, 3, 0);
    {
        let mut resources = session.app.world_mut().resource_mut::<InventoryResource>();
        let target = resources
            .inventory_items
            .iter_mut()
            .find(|item| item_unique_id(item) == inventory_id)
            .expect("target inventory Amulet");
        target.quantity = max_stack - 2;
    }

    let packets = session.handle_packet(ClientPacket::MergeItem {
        grid_from: MirGridType::Equipment,
        grid_to: MirGridType::Inventory,
        id_from: equipment_id,
        id_to: inventory_id,
    });

    assert!(merge_succeeded(&packets));
    assert_eq!(equipped_amulet_quantity(&session), Some(3));
    assert_eq!(
        item_quantity(&session, ItemContainer::Bag1, inventory_id),
        Some(max_stack)
    );
}

#[test]
fn inventory_amulet_fully_merges_into_equipment() {
    let mut session = authenticated_session();
    let (equipment_id, inventory_id, _) =
        equipped_amulet_and_target(&mut session, ItemContainer::Bag1, 2, 2, 3, 0);

    let packets = session.handle_packet(ClientPacket::MergeItem {
        grid_from: MirGridType::Inventory,
        grid_to: MirGridType::Equipment,
        id_from: inventory_id,
        id_to: equipment_id,
    });

    assert!(merge_succeeded(&packets));
    assert_eq!(equipped_amulet_quantity(&session), Some(5));
    assert_eq!(
        item_quantity(&session, ItemContainer::Bag1, inventory_id),
        None
    );
}

#[test]
fn equipped_amulet_fully_merges_into_storage_and_refreshes_equipment_stats() {
    let mut session = authenticated_session();
    activate_storage_service(&mut session);
    let (equipment_id, storage_id, _) =
        equipped_amulet_and_target(&mut session, ItemContainer::Storage, 2, 2, 3, 17);
    let before = player_stats(session.app.world()).max_dc();

    let packets = session.handle_packet(ClientPacket::MergeItem {
        grid_from: MirGridType::Equipment,
        grid_to: MirGridType::Storage,
        id_from: equipment_id,
        id_to: storage_id,
    });

    assert!(merge_succeeded(&packets));
    assert_eq!(equipped_amulet_quantity(&session), None);
    assert_eq!(
        item_quantity(&session, ItemContainer::Storage, storage_id),
        Some(5)
    );
    assert_eq!(player_stats(session.app.world()).max_dc(), before - 17);
}

#[test]
fn storage_amulet_partially_merges_into_equipment() {
    let mut session = authenticated_session();
    activate_storage_service(&mut session);
    let (equipment_id, storage_id, max_stack) =
        equipped_amulet_and_target(&mut session, ItemContainer::Storage, 2, 5, 3, 0);
    {
        let mut resources = session.app.world_mut().resource_mut::<InventoryResource>();
        let equipment = resources
            .equipment_items
            .iter_mut()
            .find(|item| item.slot == EquipmentSlot::Amulet)
            .expect("equipped Amulet");
        equipment.quantity = max_stack - 2;
    }

    let packets = session.handle_packet(ClientPacket::MergeItem {
        grid_from: MirGridType::Storage,
        grid_to: MirGridType::Equipment,
        id_from: storage_id,
        id_to: equipment_id,
    });

    assert!(merge_succeeded(&packets));
    assert_eq!(equipped_amulet_quantity(&session), Some(max_stack));
    assert_eq!(
        item_quantity(&session, ItemContainer::Storage, storage_id),
        Some(1)
    );
}

#[test]
fn equipment_merge_rejects_stale_uid_without_mutating_stacks() {
    let mut session = authenticated_session();
    let (equipment_id, inventory_id, _) =
        equipped_amulet_and_target(&mut session, ItemContainer::Bag1, 2, 4, 3, 0);

    let packets = session.handle_packet(ClientPacket::MergeItem {
        grid_from: MirGridType::Equipment,
        grid_to: MirGridType::Inventory,
        id_from: equipment_id + 1,
        id_to: inventory_id,
    });

    assert!(merge_failed(&packets));
    assert_eq!(equipped_amulet_quantity(&session), Some(4));
    assert_eq!(
        item_quantity(&session, ItemContainer::Bag1, inventory_id),
        Some(3)
    );
}

#[test]
fn equipment_merge_canonicalizes_only_inert_amulet_durability() {
    let mut session = authenticated_session();
    let (equipment_id, inventory_id, _) =
        equipped_amulet_and_target(&mut session, ItemContainer::Bag1, 2, 2, 3, 0);
    {
        let resources = session.app.world().resource::<InventoryResource>();
        let equipment = resources
            .equipment_items
            .iter()
            .find(|item| item.slot == EquipmentSlot::Amulet)
            .expect("equipped Amulet");
        let target = resources
            .inventory_items
            .iter()
            .find(|item| item_unique_id(item) == inventory_id)
            .expect("target inventory Amulet");
        assert_eq!(equipment.durability_current, 10);
        assert_eq!(target.durability_current, None);
    }
    assert!(merge_succeeded(&session.handle_packet(
        ClientPacket::MergeItem {
            grid_from: MirGridType::Equipment,
            grid_to: MirGridType::Inventory,
            id_from: equipment_id,
            id_to: inventory_id,
        }
    )));

    let mut mismatch = authenticated_session();
    let (equipment_id, inventory_id, _) =
        equipped_amulet_and_target(&mut mismatch, ItemContainer::Bag1, 2, 2, 3, 0);
    mismatch
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .inventory_items
        .iter_mut()
        .find(|item| item_unique_id(item) == inventory_id)
        .expect("target inventory Amulet")
        .added_attack = 1;
    assert!(merge_failed(&mismatch.handle_packet(
        ClientPacket::MergeItem {
            grid_from: MirGridType::Equipment,
            grid_to: MirGridType::Inventory,
            id_from: equipment_id,
            id_to: inventory_id,
        }
    )));
    assert_eq!(equipped_amulet_quantity(&mismatch), Some(2));
    assert_eq!(
        item_quantity(&mismatch, ItemContainer::Bag1, inventory_id),
        Some(3)
    );
}

#[test]
fn equipment_merge_rejects_reserved_and_cursed_sources_without_mutation() {
    let mut reserved = authenticated_session();
    let (equipment_id, inventory_id, _) =
        equipped_amulet_and_target(&mut reserved, ItemContainer::Bag1, 2, 2, 3, 0);
    reserved
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .reserved_item_unique_ids
        .insert(equipment_id);
    assert!(merge_failed(&reserved.handle_packet(
        ClientPacket::MergeItem {
            grid_from: MirGridType::Equipment,
            grid_to: MirGridType::Inventory,
            id_from: equipment_id,
            id_to: inventory_id,
        }
    )));
    assert_eq!(equipped_amulet_quantity(&reserved), Some(2));

    let mut cursed = authenticated_session();
    let (equipment_id, inventory_id, _) =
        equipped_amulet_and_target(&mut cursed, ItemContainer::Bag1, 2, 2, 3, 0);
    {
        let mut resources = cursed.app.world_mut().resource_mut::<InventoryResource>();
        resources
            .equipment_items
            .iter_mut()
            .find(|item| item.slot == EquipmentSlot::Amulet)
            .expect("equipped Amulet")
            .cursed = true;
        resources
            .inventory_items
            .iter_mut()
            .find(|item| item_unique_id(item) == inventory_id)
            .expect("target inventory Amulet")
            .cursed = true;
    }
    assert!(merge_failed(&cursed.handle_packet(
        ClientPacket::MergeItem {
            grid_from: MirGridType::Equipment,
            grid_to: MirGridType::Inventory,
            id_from: equipment_id,
            id_to: inventory_id,
        }
    )));
    assert_eq!(equipped_amulet_quantity(&cursed), Some(2));
    assert_eq!(
        item_quantity(&cursed, ItemContainer::Bag1, inventory_id),
        Some(3)
    );
}

#[test]
fn equipment_storage_merge_requires_unlocked_active_storage_and_accessible_slot() {
    let mut session = authenticated_session();
    activate_storage_service(&mut session);
    let (equipment_id, storage_id, _) =
        equipped_amulet_and_target(&mut session, ItemContainer::Storage, 2, 4, 3, 0);
    {
        let mut resources = session.app.world_mut().resource_mut::<InventoryResource>();
        resources.storage_has_password = true;
        resources.storage_unlocked = false;
    }
    let locked = session.handle_packet(ClientPacket::MergeItem {
        grid_from: MirGridType::Equipment,
        grid_to: MirGridType::Storage,
        id_from: equipment_id,
        id_to: storage_id,
    });
    assert!(merge_failed(&locked));
    assert_eq!(equipped_amulet_quantity(&session), Some(4));

    {
        let mut resources = session.app.world_mut().resource_mut::<InventoryResource>();
        resources.storage_unlocked = true;
        resources.has_expanded_storage = false;
        resources
            .storage_items
            .iter_mut()
            .find(|item| item_unique_id(item) == storage_id)
            .expect("target storage Amulet")
            .slot = BASE_STORAGE_SLOTS as u8;
    }
    let expired = session.handle_packet(ClientPacket::MergeItem {
        grid_from: MirGridType::Equipment,
        grid_to: MirGridType::Storage,
        id_from: equipment_id,
        id_to: storage_id,
    });
    assert!(merge_failed(&expired));
    assert_eq!(equipped_amulet_quantity(&session), Some(4));
    assert_eq!(
        item_quantity(&session, ItemContainer::Storage, storage_id),
        Some(3)
    );
}

#[test]
fn equipment_merge_rejects_non_amulet_and_dont_store_amulet() {
    let mut session = authenticated_session();
    let template = mir2_game_data::crystal_item_by_name("WoodenSword")
        .expect("Crystal WoodenSword template should exist");
    let mut source = embedded_item_state_from_template(&template, ItemContainer::Bag1, 35);
    source.unique_id = 92_001;
    source.quantity = 1;
    let source_unique_id = source.unique_id;
    {
        let mut resources = session.app.world_mut().resource_mut::<InventoryResource>();
        let mut target = source.clone();
        target.slot = 2;
        target.unique_id = 90_002;
        resources.inventory_items.push(source);
        resources.inventory_items.push(target);
    }
    let weapon_slot =
        equipment_slot_index(EquipmentSlot::Weapon).expect("Weapon slot index") as i32;
    assert!(
        session
            .handle_packet(ClientPacket::EquipItem {
                grid: MirGridType::Inventory,
                unique_id: source_unique_id,
                to: weapon_slot,
            })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::EquipItem { success: true, .. }))
    );
    let equipment_id = session
        .app
        .world()
        .resource::<InventoryResource>()
        .equipment_items
        .iter()
        .find(|item| item.slot == EquipmentSlot::Weapon)
        .and_then(user_item_from_equipment_state)
        .map(|item| item.unique_id)
        .expect("equipped WoodenSword should retain its exact UID");
    let non_amulet = session.handle_packet(ClientPacket::MergeItem {
        grid_from: MirGridType::Equipment,
        grid_to: MirGridType::Inventory,
        id_from: equipment_id,
        id_to: 90_002,
    });
    assert!(merge_failed(&non_amulet));
    assert_eq!(
        session
            .app
            .world()
            .resource::<InventoryResource>()
            .equipment_items
            .iter()
            .find(|item| item.slot == EquipmentSlot::Weapon)
            .map(|item| item.quantity),
        Some(1)
    );

    let mut storage_session = authenticated_session();
    activate_storage_service(&mut storage_session);
    let (equipment_id, storage_id, _) =
        equipped_amulet_and_target(&mut storage_session, ItemContainer::Storage, 2, 4, 3, 0);
    {
        let mut resources = storage_session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>();
        resources
            .equipment_items
            .iter_mut()
            .find(|item| item.slot == EquipmentSlot::Amulet)
            .expect("equipped Amulet")
            .rental_binding_flags = CRYSTAL_BIND_DONT_STORE;
        resources
            .storage_items
            .iter_mut()
            .find(|item| item_unique_id(item) == storage_id)
            .expect("target storage Amulet")
            .rental_binding_flags = CRYSTAL_BIND_DONT_STORE;
    }
    let dont_store = storage_session.handle_packet(ClientPacket::MergeItem {
        grid_from: MirGridType::Equipment,
        grid_to: MirGridType::Storage,
        id_from: equipment_id,
        id_to: storage_id,
    });
    assert!(merge_failed(&dont_store));
    assert_eq!(equipped_amulet_quantity(&storage_session), Some(4));
    assert_eq!(
        item_quantity(&storage_session, ItemContainer::Storage, storage_id),
        Some(3)
    );
}
