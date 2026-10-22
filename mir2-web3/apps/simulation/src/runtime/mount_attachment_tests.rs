//! Ordinary mount attachment packets and capability restoration; not GUI acceptance.
use super::components::{
    entity_position, player_entity, CharacterBody, DisplayName, Facing, Npc, NpcAgent, ObjectId,
    Position, WorldObject,
};
use super::equipment::{
    equipment_state_from_item_state, refresh_mount_resource_from_equipment, replace_equipment,
    user_item_from_equipment_state, EquipmentState,
};
use super::items::{
    embedded_item_state_from_template, try_item_state_from_user_item, try_user_item_from_item_state,
};
use super::resources::{
    InventoryResource, MountResource, NpcStateResource, PlayerPermissionResource,
    PlayerRuntimeResource, RuntimeConfigResource, SessionResource, Stage5SystemsResource,
};
use crate::config::MapDropRuleRecord;
use crate::{EquipmentSlot, ItemContainer, SimulationConfig, SimulationSession};
use mir2_protocol::{ClientPacket, MirDirection, MirGridType, ServerPacket};

fn mount(slots: &[(usize, i32, u64)]) -> EquipmentState {
    let template = mir2_game_data::crystal_item_by_index(764).unwrap();
    let mut carrier = embedded_item_state_from_template(&template, ItemContainer::Bag1, 13);
    carrier.unique_id = 88_001;
    let mut wire = try_user_item_from_item_state(&carrier).unwrap();
    assert_eq!(wire.slots.len(), 5);
    for &(slot, index, uid) in slots {
        let template = mir2_game_data::crystal_item_by_index(index).unwrap();
        let mut child =
            embedded_item_state_from_template(&template, ItemContainer::Bag1, slot as u8);
        child.unique_id = uid;
        wire.slots[slot] = Some(try_user_item_from_item_state(&child).unwrap());
    }
    let carrier = try_item_state_from_user_item(carrier, &wire).unwrap();
    equipment_state_from_item_state(&carrier, EquipmentSlot::Mount)
}

fn session(no_mount: bool, need_bridle: bool) -> SimulationSession {
    let mut config = SimulationConfig::default();
    config.default_character.level = 55;
    config.map_drop_rules.push(MapDropRuleRecord {
        map_file_name: "0.map".into(),
        no_experience: false,
        no_town_teleport: false,
        no_escape: false,
        no_random: false,
        no_drug: false,
        no_reincarnation: false,
        no_throw_item: false,
        no_drop_player: false,
        no_drop_monster: false,
        no_mount,
        no_hero: false,
        no_intelligent_creatures: false,
        need_bridle,
    });
    let mut session = SimulationSession::new(config);
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    assert!(session
        .handle_packet(ClientPacket::StartGame { character_index: 0 })
        .iter()
        .any(|p| matches!(p, ServerPacket::StartGame { result: 4, .. })));
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .selected_character
        .as_mut()
        .unwrap()
        .level = 55;
    let player = player_entity(session.app.world()).unwrap();
    session
        .app
        .world_mut()
        .entity_mut(player)
        .get_mut::<CharacterBody>()
        .unwrap()
        .level = 55;
    session
}

fn capabilities(session: &SimulationSession) -> (bool, bool, bool) {
    let state = session.app.world().resource::<MountResource>();
    (state.has_reins, state.has_bells, state.has_saddle)
}

fn assert_ride(session: &mut SimulationSession, expected: bool) {
    let packets = session.handle_packet(ClientPacket::UseItem {
        unique_id: 13,
        grid: MirGridType::Equipment,
    });
    assert!(
        packets.iter().any(|packet| matches!(packet,
            ServerPacket::MountUpdate { riding_mount, .. } if *riding_mount == expected
        )),
        "{packets:?}"
    );
    assert_eq!(
        session.app.world().resource::<MountResource>().riding_mount,
        expected
    );
}

fn give(session: &mut SimulationSession, index: i32, uid: u64, slot: u8, storage: bool) {
    let template = mir2_game_data::crystal_item_by_index(index).unwrap();
    let container = if storage {
        ItemContainer::Storage
    } else {
        ItemContainer::Bag1
    };
    let mut item = embedded_item_state_from_template(&template, container, slot);
    item.unique_id = uid;
    let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
    if storage {
        inventory.storage_items.push(item);
    } else {
        inventory.inventory_items.push(item);
    }
}

fn equip_packet(uid: u64, to: i32, host: u64, storage: bool) -> ClientPacket {
    ClientPacket::EquipSlotItem {
        grid: if storage {
            MirGridType::Storage
        } else {
            MirGridType::Inventory
        },
        unique_id: uid,
        to,
        grid_to: MirGridType::Mount,
        to_unique_id: host,
    }
}

fn remove_packet(uid: u64, to: i32, storage: bool) -> ClientPacket {
    ClientPacket::RemoveSlotItem {
        grid: MirGridType::Mount,
        unique_id: uid,
        to,
        grid_to: if storage {
            MirGridType::Storage
        } else {
            MirGridType::Inventory
        },
        from_unique_id: 88_001,
    }
}

fn custody(session: &SimulationSession) -> serde_json::Value {
    let inventory = session.app.world().resource::<InventoryResource>();
    serde_json::json!({
        "bag": inventory.inventory_items,
        "storage": inventory.storage_items,
        "equipment": inventory.equipment_items,
        "capabilities": capabilities(session),
        "riding": session.app.world().resource::<MountResource>().riding_mount,
    })
}

fn reject_unchanged(session: &mut SimulationSession, packet: ClientPacket) {
    let before = custody(session);
    let packets = session.handle_packet(packet);
    assert!(
        packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::EquipSlotItem { success: false, .. }
                | ServerPacket::RemoveSlotItem { success: false, .. }
        )),
        "{packets:?}"
    );
    assert!(
        !packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::EquipSlotItem { success: true, .. }
                | ServerPacket::RemoveSlotItem { success: true, .. }
                | ServerPacket::MountUpdate { .. }
        )),
        "{packets:?}"
    );
    assert_eq!(custody(session), before);
}

fn storage_service(session: &mut SimulationSession) {
    let player = player_entity(session.app.world()).unwrap();
    let position = entity_position(session.app.world(), player).unwrap();
    session.app.world_mut().spawn((
        WorldObject,
        Npc,
        ObjectId(990_101),
        DisplayName::literal("Warehouse Keeper"),
        Position(position),
        Facing(MirDirection::Left),
        NpcAgent {
            image: 5,
            colour_argb: -1,
            quest_ids: vec![],
            script_key: None,
        },
    ));
    session
        .app
        .world_mut()
        .resource_mut::<NpcStateResource>()
        .active_npc_service = Some(super::npc::ActiveNpcServiceState {
        script_key: "00Default".into(),
        label_key: "STORAGE".into(),
        npc_object_id: 990_101,
    });
    assert!(super::npc::active_crystal_storage_service(
        session.app.world()
    ));
}

#[test]
fn mount_attachments_default_and_absent_mount_never_grant_capabilities() {
    let initial = MountResource::new();
    assert!(!initial.has_saddle && !initial.has_reins && !initial.has_bells);
    let mut session = session(false, false);
    replace_equipment(
        session.app.world_mut(),
        mount(&[(0, 776, 88_002), (1, 778, 88_003), (2, 782, 88_004)]),
    );
    assert_eq!(capabilities(&session), (true, true, true));
    session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .equipment_items
        .retain(|item| item.slot != EquipmentSlot::Mount);
    refresh_mount_resource_from_equipment(session.app.world_mut());
    assert_eq!(capabilities(&session), (false, false, false));
    assert_eq!(
        session.app.world().resource::<MountResource>().mount_type,
        -1
    );
    assert!(!session.app.world().resource::<MountResource>().riding_mount);
}

#[test]
fn mount_attachments_use_exact_sparse_slots_not_compact_child_order() {
    let mut session = session(false, false);
    let mut worn = mount(&[(2, 782, 88_004), (4, 786, 88_005)]);
    assert_eq!(worn.socketed.len(), 2);
    worn.socketed.reverse();
    replace_equipment(session.app.world_mut(), worn);
    assert_eq!(capabilities(&session), (false, false, true));
    assert_ride(&mut session, true);
    replace_equipment(session.app.world_mut(), mount(&[]));
    assert_eq!(capabilities(&session), (false, false, false));
}

#[test]
fn mount_attachments_wrong_types_or_invalid_carrier_cannot_grant_capabilities() {
    let mut session = session(false, false);
    replace_equipment(
        session.app.world_mut(),
        mount(&[(0, 782, 88_004), (1, 778, 88_003), (2, 776, 88_002)]),
    );
    assert_eq!(capabilities(&session), (false, true, false));
    assert_ride(&mut session, false);
    let mut invalid = mount(&[(0, 776, 88_002), (1, 778, 88_003), (2, 782, 88_004)]);
    invalid.user_item_metadata.as_mut().unwrap().item_index = Some(i32::MAX);
    // Bypass normal replacement's stats validation to exercise the projection's fail-closed path.
    {
        let mut resources = session.app.world_mut().resource_mut::<InventoryResource>();
        *resources
            .equipment_items
            .iter_mut()
            .find(|item| item.slot == EquipmentSlot::Mount)
            .unwrap() = invalid;
    }
    refresh_mount_resource_from_equipment(session.app.world_mut());
    assert_eq!(capabilities(&session), (false, false, false));
}

#[test]
fn mount_attachments_saddle_and_bridle_preserve_existing_map_ride_gates() {
    let mut bare = session(false, false);
    replace_equipment(bare.app.world_mut(), mount(&[]));
    assert_ride(&mut bare, false);
    let mut bridle_map = session(false, true);
    replace_equipment(bridle_map.app.world_mut(), mount(&[(2, 782, 88_004)]));
    assert_ride(&mut bridle_map, false);
    replace_equipment(
        bridle_map.app.world_mut(),
        mount(&[(0, 776, 88_002), (2, 782, 88_004)]),
    );
    assert_ride(&mut bridle_map, true);
    assert_ride(&mut bridle_map, false);
    let mut no_mount_map = session(true, false);
    replace_equipment(
        no_mount_map.app.world_mut(),
        mount(&[(0, 776, 88_002), (2, 782, 88_004)]),
    );
    assert_ride(&mut no_mount_map, false);
}

#[test]
fn mount_attachments_save_json_restore_rebuilds_capabilities_and_exact_uids() {
    let mut original = session(false, true);
    replace_equipment(
        original.app.world_mut(),
        mount(&[(0, 776, 88_002), (1, 778, 88_003), (2, 782, 88_004)]),
    );
    let save = super::save::snapshot_active_character_save(original.app.world()).unwrap();
    let encoded = serde_json::to_vec(&save).unwrap();
    let saved = serde_json::from_slice(&encoded).unwrap();
    let mut restored = session(false, true);
    super::save::apply_character_save(restored.app.world_mut(), &saved).unwrap();
    assert_eq!(capabilities(&restored), (true, true, true));
    let resources = restored.app.world().resource::<InventoryResource>();
    let worn = resources
        .equipment_items
        .iter()
        .find(|item| item.slot == EquipmentSlot::Mount)
        .unwrap();
    let wire = user_item_from_equipment_state(worn).unwrap();
    assert_eq!(wire.unique_id, 88_001);
    for (slot, index, uid) in [(0, 776, 88_002), (1, 778, 88_003), (2, 782, 88_004)] {
        let child = wire.slots[slot].as_ref().unwrap();
        assert_eq!((child.item_index, child.unique_id), (index, uid));
    }
    assert!(wire.slots[3].is_none() && wire.slots[4].is_none());
    assert_ride(&mut restored, true);
}

#[test]
fn mount_attachments_ordinary_packets_equip_and_remove_all_five_types() {
    let mut session = session(false, false);
    replace_equipment(session.app.world_mut(), mount(&[]));
    for (slot, index) in [(0, 776), (1, 778), (2, 782), (3, 783), (4, 786)] {
        let uid = 89_100 + slot as u64;
        give(&mut session, index, uid, 20 + slot as u8, false);
        let packets = session.handle_packet(equip_packet(uid, slot, 88_001, false));
        assert!(
            matches!(
                packets.first(),
                Some(ServerPacket::EquipSlotItem { success: true, .. })
            ),
            "{packets:?}"
        );
        let inventory = session.app.world().resource::<InventoryResource>();
        assert!(!inventory
            .inventory_items
            .iter()
            .any(|item| item.unique_id == uid));
        let worn = inventory
            .equipment_items
            .iter()
            .find(|item| item.slot == EquipmentSlot::Mount)
            .unwrap();
        let wire = user_item_from_equipment_state(worn).unwrap();
        let child = wire.slots[slot as usize].as_ref().unwrap();
        assert_eq!((child.item_index, child.unique_id), (index, uid));
    }
    assert_eq!(capabilities(&session), (true, true, true));
    assert_ride(&mut session, true);
    for slot in [4, 3, 1, 2, 0] {
        let uid = 89_100 + slot as u64;
        let packets = session.handle_packet(remove_packet(uid, 20 + slot, false));
        assert!(
            matches!(
                packets.first(),
                Some(ServerPacket::RemoveSlotItem { success: true, .. })
            ),
            "{packets:?}"
        );
        if slot == 2 {
            assert!(
                packets.iter().any(|packet| matches!(
                    packet,
                    ServerPacket::MountUpdate {
                        riding_mount: false,
                        ..
                    }
                )),
                "{packets:?}"
            );
        }
        assert_eq!(
            session
                .app
                .world()
                .resource::<InventoryResource>()
                .inventory_items
                .iter()
                .filter(|item| item.unique_id == uid)
                .count(),
            1
        );
    }
    assert_eq!(capabilities(&session), (false, false, false));
    assert!(!session.app.world().resource::<MountResource>().riding_mount);
}

#[test]
fn mount_attachments_equip_rejects_wrong_slot_host_level_binding_and_reservation() {
    for case in [
        "negative",
        "overflow",
        "wrong-type",
        "host",
        "occupied",
        "level",
        "soul-bound",
        "source-reserved",
        "host-reserved",
        "legacy-host-reserved",
        "duplicate-source",
    ] {
        let mut session = session(false, false);
        replace_equipment(
            session.app.world_mut(),
            if case == "occupied" {
                mount(&[(2, 782, 88_004)])
            } else {
                mount(&[])
            },
        );
        give(&mut session, 782, 89_201, 20, false);
        let to = match case {
            "negative" => -1,
            "overflow" => 5,
            "wrong-type" => 0,
            _ => 2,
        };
        let host = if case == "host" {
            88_999
        } else if case == "legacy-host-reserved" {
            13
        } else {
            88_001
        };
        match case {
            "level" => {
                session
                    .app
                    .world_mut()
                    .resource_mut::<SessionResource>()
                    .selected_character
                    .as_mut()
                    .unwrap()
                    .level = 21
            }
            "soul-bound" => {
                session
                    .app
                    .world_mut()
                    .resource_mut::<InventoryResource>()
                    .inventory_items
                    .iter_mut()
                    .find(|item| item.unique_id == 89_201)
                    .unwrap()
                    .soul_bound_id = Some(i32::MAX)
            }
            "source-reserved" | "host-reserved" | "legacy-host-reserved" => {
                let id = if case == "source-reserved" {
                    89_201
                } else {
                    88_001
                };
                session
                    .app
                    .world_mut()
                    .resource_mut::<InventoryResource>()
                    .reserved_item_unique_ids
                    .insert(id);
            }
            "duplicate-source" => give(&mut session, 782, 89_201, 21, false),
            _ => {}
        }
        reject_unchanged(&mut session, equip_packet(89_201, to, host, false));
    }
}

#[test]
fn mount_attachments_storage_requires_service_range_unlock_and_valid_index() {
    for case in ["no-service", "range", "locked", "index"] {
        let mut session = session(false, false);
        replace_equipment(session.app.world_mut(), mount(&[]));
        if case != "no-service" {
            storage_service(&mut session);
        }
        if case == "range" {
            let entity =
                super::components::entity_by_object_id(session.app.world(), 990_101).unwrap();
            session
                .app
                .world_mut()
                .get_mut::<Position>(entity)
                .unwrap()
                .0
                .x += 100;
        }
        if case == "locked" {
            session
                .app
                .world_mut()
                .resource_mut::<RuntimeConfigResource>()
                .config
                .require_storage_password = true;
            let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
            inventory.storage_has_password = true;
            inventory.storage_unlocked = false;
        }
        give(
            &mut session,
            782,
            89_301,
            if case == "index" { 200 } else { 0 },
            true,
        );
        reject_unchanged(&mut session, equip_packet(89_301, 2, 88_001, true));
    }
    let mut session = session(false, false);
    storage_service(&mut session);
    replace_equipment(session.app.world_mut(), mount(&[]));
    {
        let inventory = session.app.world().resource::<InventoryResource>();
        // Demo seed storage occupies slots0/1; preserve those existing items.
        for slot in [2, 3] {
            assert!(super::inventory::is_valid_storage_slot(inventory, slot));
            assert!(!super::inventory::collection_slot_occupied(
                inventory,
                ItemContainer::Storage,
                slot,
            ));
        }
    }
    give(&mut session, 782, 89_301, 2, true);
    assert!(session
        .handle_packet(equip_packet(89_301, 2, 88_001, true))
        .iter()
        .any(|packet| matches!(packet, ServerPacket::EquipSlotItem { success: true, .. })));
    assert_eq!(capabilities(&session), (false, false, true));
    assert!(super::npc::active_crystal_storage_service(
        session.app.world()
    ));
    assert!(!super::inventory::storage_locked(session.app.world()));
    let packets = session.handle_packet(remove_packet(89_301, 3, true));
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::RemoveSlotItem { success: true, .. })),
        "Storage attachment removal failed: {packets:?}"
    );
    assert_eq!(capabilities(&session), (false, false, false));
    assert_eq!(
        session
            .app
            .world()
            .resource::<InventoryResource>()
            .storage_items
            .iter()
            .filter(|item| item.unique_id == 89_301 && item.slot == 3)
            .count(),
        1
    );
}

#[test]
fn mount_attachments_remove_rejects_binding_curse_wedding_reservation_and_occupied_target() {
    for case in [
        "dont-store",
        "curse",
        "wedding",
        "child-reserved",
        "host-reserved",
        "occupied",
        "index",
        "host",
    ] {
        let mut session = session(false, false);
        storage_service(&mut session);
        let mut worn = mount(&[(2, 782, 88_004)]);
        match case {
            "dont-store" => {
                worn.socketed[0].rental_binding_flags =
                    super::crystal_compat::CRYSTAL_BIND_DONT_STORE
            }
            "curse" => worn.socketed[0].cursed = true,
            "wedding" => {
                let mut wire = user_item_from_equipment_state(&worn).unwrap();
                wire.slots[2].as_mut().unwrap().wedding_ring = 0;
                let carrier = super::equipment::item_state_from_equipment_state(
                    worn,
                    ItemContainer::Bag1,
                    13,
                );
                worn = equipment_state_from_item_state(
                    &try_item_state_from_user_item(carrier, &wire).unwrap(),
                    EquipmentSlot::Mount,
                );
            }
            _ => {}
        }
        replace_equipment(session.app.world_mut(), worn);
        if case == "occupied" {
            give(&mut session, 782, 89_401, 20, true);
        }
        if case == "child-reserved" || case == "host-reserved" {
            session
                .app
                .world_mut()
                .resource_mut::<InventoryResource>()
                .reserved_item_unique_ids
                .insert(if case == "child-reserved" {
                    88_004
                } else {
                    88_001
                });
        }
        let mut packet = remove_packet(88_004, if case == "index" { 200 } else { 20 }, true);
        if case == "host" {
            if let ClientPacket::RemoveSlotItem { from_unique_id, .. } = &mut packet {
                *from_unique_id = 88_999;
            }
        }
        reject_unchanged(&mut session, packet);
    }
    let mut session = session(false, false);
    let mut worn = mount(&[(2, 782, 88_004)]);
    worn.socketed[0].cursed = true;
    replace_equipment(session.app.world_mut(), worn);
    session
        .app
        .world_mut()
        .resource_mut::<PlayerPermissionResource>()
        .unlock_curse = true;
    assert!(session
        .handle_packet(remove_packet(88_004, 20, false))
        .iter()
        .any(|packet| matches!(packet, ServerPacket::RemoveSlotItem { success: true, .. })));
    assert!(
        !session
            .app
            .world()
            .resource::<PlayerPermissionResource>()
            .unlock_curse
    );
}

#[test]
fn mount_attachments_removing_required_reins_dismounts_and_emits_owner_update() {
    let mut session = session(false, true);
    replace_equipment(
        session.app.world_mut(),
        mount(&[(0, 776, 88_002), (2, 782, 88_004)]),
    );
    assert_ride(&mut session, true);
    let packets = session.handle_packet(remove_packet(88_002, 20, false));
    assert!(matches!(
        packets.first(),
        Some(ServerPacket::RemoveSlotItem { success: true, .. })
    ));
    assert!(
        packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::MountUpdate {
                mount_type: 0,
                riding_mount: false,
                ..
            }
        )),
        "{packets:?}"
    );
    assert_eq!(capabilities(&session), (false, false, true));
    assert_ride(&mut session, false);
}

#[test]
fn mount_attachments_removing_mount_while_riding_emits_absent_mount_update() {
    let mut session = session(false, false);
    replace_equipment(session.app.world_mut(), mount(&[(2, 782, 88_004)]));
    assert_ride(&mut session, true);
    let packets = session.handle_packet(ClientPacket::RemoveItem {
        grid: MirGridType::Inventory,
        unique_id: 88_001,
        to: 20,
    });
    assert!(
        matches!(
            packets.first(),
            Some(ServerPacket::RemoveItem { success: true, .. })
        ),
        "{packets:?}"
    );
    assert!(
        packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::MountUpdate {
                mount_type: -1,
                riding_mount: false,
                ..
            }
        )),
        "{packets:?}"
    );
    assert_eq!(capabilities(&session), (false, false, false));
}

#[test]
fn mount_attachments_trade_reserved_source_and_child_cannot_move() {
    let mut session = session(false, false);
    replace_equipment(session.app.world_mut(), mount(&[]));
    give(&mut session, 782, 89_501, 20, false);
    session.trade_request("Partner");
    session
        .app
        .world_mut()
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .trade
        .as_mut()
        .unwrap()
        .offered_unique_ids
        .insert(0, 89_501);
    reject_unchanged(&mut session, equip_packet(89_501, 2, 88_001, false));

    replace_equipment(session.app.world_mut(), mount(&[(2, 782, 88_004)]));
    session
        .app
        .world_mut()
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .trade
        .as_mut()
        .unwrap()
        .offered_unique_ids
        .insert(1, 88_004);
    reject_unchanged(&mut session, remove_packet(88_004, 21, false));
}

#[test]
fn mount_attachments_refresh_on_no_mount_map_revokes_riding_with_packet() {
    let mut session = session(false, false);
    replace_equipment(session.app.world_mut(), mount(&[(2, 782, 88_004)]));
    assert_ride(&mut session, true);
    session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config
        .map_drop_rules[0]
        .no_mount = true;
    give(&mut session, 783, 89_601, 20, false);
    let packets = session.handle_packet(equip_packet(89_601, 3, 88_001, false));
    assert!(
        matches!(
            packets.first(),
            Some(ServerPacket::EquipSlotItem { success: true, .. })
        ),
        "{packets:?}"
    );
    assert!(
        packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::MountUpdate {
                mount_type: 0,
                riding_mount: false,
                ..
            }
        )),
        "{packets:?}"
    );
    assert!(!session.app.world().resource::<MountResource>().riding_mount);
}

#[test]
fn mount_attachments_zero_durability_cannot_ride_and_preserves_custody() {
    let mut session = session(false, false);
    let mut worn = mount(&[(0, 776, 88_002), (1, 778, 88_003), (2, 782, 88_004)]);
    worn.durability_current = 0;
    replace_equipment(session.app.world_mut(), worn);
    assert_eq!(
        session.app.world().resource::<MountResource>().mount_type,
        -1
    );
    assert_eq!(capabilities(&session), (false, false, false));
    let before = custody(&session);
    assert_ride(&mut session, false);
    assert_eq!(
        session.app.world().resource::<MountResource>().mount_type,
        -1
    );
    assert_eq!(custody(&session), before);
    let packets = session.handle_packet(ClientPacket::Chat {
        message: "@ride".into(),
        linked_items: vec![],
    });
    assert!(
        packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::MountUpdate {
                mount_type: -1,
                riding_mount: false,
                ..
            }
        )),
        "{packets:?}"
    );
    assert_eq!(custody(&session), before);
}

#[test]
fn mount_attachments_non_mount_template_cannot_supply_mount_type_or_ride() {
    let mut session = session(false, false);
    let template = mir2_game_data::crystal_item_by_index(221).unwrap();
    assert_ne!(
        template.item_type,
        super::crystal_compat::CRYSTAL_ITEM_TYPE_MOUNT
    );
    let carrier = embedded_item_state_from_template(&template, ItemContainer::Bag1, 13);
    let mut worn = equipment_state_from_item_state(&carrier, EquipmentSlot::Mount);
    worn.shape = Some(12);
    replace_equipment(session.app.world_mut(), worn);
    assert_eq!(
        session.app.world().resource::<MountResource>().mount_type,
        -1
    );
    assert_eq!(capabilities(&session), (false, false, false));
    let before = custody(&session);
    assert_ride(&mut session, false);
    assert_eq!(
        session.app.world().resource::<MountResource>().mount_type,
        -1
    );
    assert_eq!(custody(&session), before);
}

fn destination_rule(
    session: &mut SimulationSession,
    file: &str,
    no_mount: bool,
    need_bridle: bool,
) {
    let mut config = session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>();
    let mut rule = config.config.map_drop_rules[0].clone();
    rule.map_file_name = file.into();
    rule.no_mount = no_mount;
    rule.need_bridle = need_bridle;
    config.config.map_drop_rules.push(rule);
}

#[test]
fn mount_attachments_cross_map_refresh_requires_reins_and_preserves_other_mount_rules() {
    for (no_mount, need_bridle, with_reins, expected) in [
        (false, true, false, false),
        (false, true, true, true),
        (true, true, true, false),
        (false, false, false, true),
    ] {
        let mut session = session(false, false);
        destination_rule(&mut session, "1", no_mount, need_bridle);
        let attachments = if with_reins {
            vec![(0, 776, 88_002), (2, 782, 88_004)]
        } else {
            vec![(2, 782, 88_004)]
        };
        replace_equipment(session.app.world_mut(), mount(&attachments));
        assert_ride(&mut session, true);
        let before = serde_json::to_value(
            &session
                .app
                .world()
                .resource::<InventoryResource>()
                .equipment_items,
        )
        .unwrap();
        let packets = session.transfer_map("crystal:1:30:30");
        assert!(
            packets.iter().any(
                |packet| matches!(packet, ServerPacket::MapInformation { info }
            if super::map::normalize_map_file_name(&info.file_name) == "1")
            ),
            "{packets:?}"
        );
        assert_eq!(
            session
                .app
                .world()
                .resource::<PlayerRuntimeResource>()
                .player_position,
            mir2_protocol::Point { x: 30, y: 30 }
        );
        assert_eq!(
            session.app.world().resource::<MountResource>().riding_mount,
            expected
        );
        assert_eq!(
            serde_json::to_value(
                &session
                    .app
                    .world()
                    .resource::<InventoryResource>()
                    .equipment_items
            )
            .unwrap(),
            before
        );
        if expected {
            assert!(
                !packets.iter().any(|packet| matches!(
                    packet,
                    ServerPacket::MountUpdate {
                        riding_mount: false,
                        ..
                    }
                )),
                "{packets:?}"
            );
        } else {
            assert!(
                packets.iter().any(|packet| matches!(
                    packet,
                    ServerPacket::MountUpdate {
                        mount_type: 0,
                        riding_mount: false,
                        ..
                    }
                )),
                "{packets:?}"
            );
        }
    }
}

#[test]
fn mount_attachments_unmounted_ordinary_and_mining_transfers_remain_available() {
    for (file, x, y) in [("1", 30, 30), ("D401", 24, 181)] {
        let mut session = session(false, false);
        destination_rule(&mut session, file, true, true);
        let before = custody(&session);
        let packets = session.transfer_map(&format!("crystal:{file}:{x}:{y}"));
        assert!(
            packets.iter().any(
                |packet| matches!(packet, ServerPacket::MapInformation { info }
            if super::map::normalize_map_file_name(&info.file_name) == file.to_ascii_lowercase())
            ),
            "{packets:?}"
        );
        assert_eq!(
            session
                .app
                .world()
                .resource::<PlayerRuntimeResource>()
                .player_position,
            mir2_protocol::Point { x, y }
        );
        assert!(
            !packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::MountUpdate { .. })),
            "{packets:?}"
        );
        assert_eq!(custody(&session), before);
    }
}

fn equip_real_riding_mount_fixture(session: &mut SimulationSession, durability: u16) {
    give(session, 764, 88_001, 20, false);
    session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .inventory_items
        .iter_mut()
        .find(|item| item.unique_id == 88_001)
        .unwrap()
        .durability_current = Some(durability);
    let packets = session.handle_packet(ClientPacket::EquipItem {
        grid: MirGridType::Inventory,
        unique_id: 88_001,
        to: 13,
    });
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::EquipItem { success: true, .. })),
        "{packets:?}"
    );
    for (slot, index, uid, bag_slot) in [(2, 782, 88_004, 21), (1, 778, 88_003, 22)] {
        give(session, index, uid, bag_slot, false);
        let packets = session.handle_packet(equip_packet(uid, slot, 88_001, false));
        assert!(
            packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::EquipSlotItem { success: true, .. })),
            "{packets:?}"
        );
    }
    assert_eq!(capabilities(session), (false, true, true));
    assert_ride(session, true);
}

fn worn_mount_wire(session: &SimulationSession) -> mir2_protocol::UserItem {
    let inventory = session.app.world().resource::<InventoryResource>();
    let mount = inventory
        .equipment_items
        .iter()
        .find(|item| item.slot == EquipmentSlot::Mount)
        .unwrap();
    user_item_from_equipment_state(mount).unwrap()
}

#[test]
fn mount_attachments_worn_durability_zero_revokes_riding_and_preserves_item_uids() {
    let mut session = session(false, false);
    equip_real_riding_mount_fixture(&mut session, 1);
    let before = worn_mount_wire(&session);
    assert_eq!(before.current_dura, 1);
    let packets = super::equipment::damage_worn_durability(session.app.world_mut(), 0);
    assert!(
        packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::DuraChanged {
                unique_id: 13,
                current_dura: 0
            }
        )),
        "{packets:?}"
    );
    assert_eq!(
        packets
            .iter()
            .filter(|packet| matches!(
                packet,
                ServerPacket::MountUpdate {
                    mount_type: -1,
                    riding_mount: false,
                    ..
                }
            ))
            .count(),
        1,
        "{packets:?}"
    );
    let state = session.app.world().resource::<MountResource>();
    assert_eq!(state.mount_type, -1);
    assert!(!state.riding_mount);
    assert_eq!(capabilities(&session), (false, false, false));
    let after = worn_mount_wire(&session);
    assert_eq!(
        (after.unique_id, after.item_index, after.count),
        (before.unique_id, before.item_index, before.count)
    );
    assert_eq!(after.unique_id, 88_001);
    assert_eq!(after.current_dura, 0);
    assert_eq!(after.max_dura, before.max_dura);
    assert_eq!(after.slots, before.slots);
    assert_eq!(after.slots[1].as_ref().unwrap().unique_id, 88_003);
    assert_eq!(after.slots[2].as_ref().unwrap().unique_id, 88_004);
}

#[test]
fn mount_attachments_non_mount_break_and_nonzero_mount_wear_do_not_emit_mount_update() {
    let mut session = session(false, false);
    equip_real_riding_mount_fixture(&mut session, 2);
    let helmet = mir2_game_data::crystal_item_by_name("BronzeHelmet").unwrap();
    give(&mut session, helmet.item_index, 89_701, 23, false);
    session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .inventory_items
        .iter_mut()
        .find(|item| item.unique_id == 89_701)
        .unwrap()
        .durability_current = Some(1);
    let packets = session.handle_packet(ClientPacket::EquipItem {
        grid: MirGridType::Inventory,
        unique_id: 89_701,
        to: super::equipment::equipment_slot_index(EquipmentSlot::Helmet).unwrap() as i32,
    });
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::EquipItem { success: true, .. })),
        "{packets:?}"
    );
    let before = worn_mount_wire(&session);
    let packets = super::equipment::damage_worn_durability(session.app.world_mut(), 0);
    let helmet_id = super::equipment::equipment_slot_unique_id(EquipmentSlot::Helmet).unwrap();
    assert!(
        packets.iter().any(|packet| matches!(packet,
            ServerPacket::DuraChanged { unique_id, current_dura: 0 } if *unique_id == helmet_id
        )),
        "{packets:?}"
    );
    assert!(
        !packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::MountUpdate { .. })),
        "{packets:?}"
    );
    let after = worn_mount_wire(&session);
    assert_eq!(after.current_dura, 1);
    assert_eq!(
        (after.unique_id, after.item_index),
        (before.unique_id, before.item_index)
    );
    assert_eq!(after.slots, before.slots);
    let state = session.app.world().resource::<MountResource>();
    assert_eq!(state.mount_type, 0);
    assert!(state.riding_mount);
    assert_eq!(capabilities(&session), (false, true, true));
}
