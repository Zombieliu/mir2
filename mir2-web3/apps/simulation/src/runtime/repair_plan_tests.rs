use super::*;
use crate::config::SimulationConfig;
use crate::runtime::components::{entity_by_object_id, player_entity, PlayerVitals, Position};
use crate::runtime::resources::NpcStateResource;
use crate::runtime::session::SimulationSession;
use mir2_protocol::{ClientPacket, MirDirection, Point, UserItemRentalInformation};

fn repair_session(special: bool) -> SimulationSession {
    let mut config = SimulationConfig::default();
    let (object_id, script_key) = if special {
        (4988, "BichonProvince/BichonWall/Blacksmith-0")
    } else {
        (4990, "BichonProvince/NaturalCave/WickedTrader")
    };
    config.visible_npcs.push(crate::VisibleNpcRecord {
        object_id,
        name: "Repair plan fixture".into(),
        image: 5,
        colour_argb: -1,
        position: Point { x: 331, y: 271 },
        direction: MirDirection::Left,
        quest_ids: Vec::new(),
        script_key: Some(script_key.into()),
    });
    let mut session = SimulationSession::new(config);
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        })
        .iter()
        .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    session.interact(object_id);
    session.select_npc_dialog_target(if special { "@SRepair" } else { "@Repair" });
    session
        .app
        .world_mut()
        .resource_mut::<PlayerRuntimeResource>()
        .gold = 1_000_000;
    session
}

fn rich_item(unique_id: u64) -> ItemState {
    let mut item =
        item_state_from_equipment_state(seed_equipment_items()[0].clone(), ItemContainer::Bag1, 30);
    item.key = "wooden-sword".into();
    item.unique_id = unique_id;
    item.quantity = 3;
    item.durability_current = Some(100);
    item.durability_max = Some(1000);
    item.added_attack = 2;
    item.added_defence = 3;
    item.added_stats = vec![UserItemStat {
        stat: 99,
        value: -5,
    }];
    item.cursed = true;
    item.socket_slots = 3;
    item.gem_count = 7;
    item.soul_bound_id = Some(23);
    item.sealed_expiry_time_binary_datetime = 124;
    item.sealed_next_time_binary_datetime = 125;
    item.rental_owner_name.clear();
    item.rental_binding_flags = 0;
    item.rental_expiry_binary_datetime = 0;
    item.rental_locked = false;
    item.user_item_metadata = None;
    item.socketed.clear();
    let mut child = item.clone();
    child.unique_id = unique_id + 100;
    child.description = "first recursive socket".into();
    let mut grandchild = child.clone();
    grandchild.unique_id += 100;
    grandchild.description = "nested socket metadata must survive".into();
    child.socketed.push(grandchild);
    item.socketed.push(child.clone());
    // Include legacy sparse wire slots as well as live recursive children.
    item.user_item_metadata = Some(ItemStateUserItemMetadata {
        item_index: Some(
            crystal_item_template_for_item_key(&item.key)
                .unwrap()
                .item_index,
        ),
        awake_type: 2,
        awake_values: vec![4, 5],
        refined_value: 6,
        refine_added: 7,
        refine_success_chance: 88,
        wedding_ring: 23,
        expire_info: Some(mir2_protocol::UserItemExpireInfo {
            expiry_binary_datetime: 123_456,
        }),
        rental_information: Some(UserItemRentalInformation {
            owner_name: String::new(),
            binding_flags: 0,
            expiry_binary_datetime: 0,
            rental_locked: false,
        }),
        sealed_info: Some(mir2_protocol::UserItemSealedInfo {
            expiry_binary_datetime: 124,
            next_seal_binary_datetime: 125,
        }),
        slots: vec![None, Some(user_item_from_item_state(&child)), None],
        is_shop_item: true,
        gm_made: true,
        live_socketed_at_capture: false,
        socket_layout_hydrated: false,
        captured_socket_positions: None,
        captured_socket_position: Some(2),
    });
    item
}

fn place_item(session: &mut SimulationSession, equipped: bool, unique_id: u64) {
    let item = rich_item(unique_id);
    let mut resources = session.app.world_mut().resource_mut::<InventoryResource>();
    resources.inventory_items.clear();
    resources.equipment_items.clear();
    if equipped {
        let mut equipment = equipment_state_from_item_state(&item, EquipmentSlot::Weapon);
        equipment.awake_type = 2;
        equipment.awake_values = vec![4, 5];
        equipment.added_luck = 9;
        resources.equipment_items.push(equipment);
        // Exact legacy slot collision must still choose worn equipment first.
        let mut colliding = item.clone();
        colliding.unique_id = 0;
        resources.inventory_items.push(colliding);
    } else {
        resources.inventory_items.push(item);
    }
}

fn state(session: &SimulationSession) -> (u32, String) {
    (
        session.app.world().resource::<PlayerRuntimeResource>().gold,
        format!("{:?}", session.app.world().resource::<InventoryResource>()),
    )
}

#[test]
fn repair_plan_preparation_is_pure_and_does_not_require_legacy_gold() {
    for special in [false, true] {
        let mut session = repair_session(special);
        place_item(&mut session, false, 9030);
        session
            .app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold = 0;
        let before_state = state(&session);
        let plan = prepare_repair_plan(session.app.world(), 9030, special).unwrap();
        assert_eq!(state(&session), before_state);
        assert!(plan.cost() > 0);
        assert_eq!(plan.special(), special);
        assert_eq!(
            plan.service().npc_object_id,
            if special { 4988 } else { 4990 }
        );
        assert_eq!(
            plan.service().label_key,
            if special { "SREPAIR" } else { "REPAIR" }
        );
        assert_eq!(
            plan.entry_acknowledgement(),
            ServerPacket::RepairItem { unique_id: 9030 }
        );
        let RepairPlanTarget::Inventory { before, after, .. } = plan.target() else {
            panic!("inventory plan")
        };
        let template = crystal_item_template_for_item_key(&before.key).unwrap();
        let configured_rate =
            crystal_npc_info_by_script_key(&plan.service().script_key).map(|npc| npc.price_rate);
        assert_eq!(
            configured_rate,
            if special { None } else { Some(2.0) },
            "Blacksmith-0 uses the fallback; WickedTrader has the manifest rate"
        );
        let rate = configured_rate.unwrap_or(1.0);
        if configured_rate.is_none() {
            assert_eq!(
                rate, 1.0,
                "missing NPC info retains the legacy default rate"
            );
        }
        assert_eq!(
            plan.cost(),
            crystal_npc_repair_cost(before, &template, rate, special)
        );
        let mut expected = before.clone();
        let max = if special { 1000 } else { 970 };
        expected.durability_current = Some(max);
        expected.durability_max = Some(max);
        assert_eq!(
            serde_json::to_value(after).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
    }
}

#[test]
fn legacy_repair_reuses_plan_for_normal_special_and_complete_snapshots() {
    for special in [false, true] {
        for equipped in [false, true] {
            let mut session = repair_session(special);
            let unique_id = if equipped { 0 } else { 9030 };
            place_item(&mut session, equipped, unique_id);
            let gold = state(&session).0;
            let before_preparation = state(&session);
            let plan = prepare_repair_plan(session.app.world(), unique_id, special).unwrap();
            assert_eq!(state(&session), before_preparation);
            let mut expected = session.app.world().resource::<InventoryResource>().clone();
            let max = if special { 1000 } else { 970 };
            if equipped {
                assert!(matches!(plan.target(), RepairPlanTarget::Equipment { .. }));
                expected.equipment_items[0].durability_current = max;
                expected.equipment_items[0].durability_max = max;
            } else {
                expected.inventory_items[0].durability_current = Some(max);
                expected.inventory_items[0].durability_max = Some(max);
            }
            let packets = repair_item_impl(session.app.world_mut(), unique_id, special);
            assert_eq!(
                packets,
                vec![
                    ServerPacket::LoseGold { gold: plan.cost() },
                    ServerPacket::ItemRepaired {
                        unique_id,
                        max_dura: max,
                        current_dura: max
                    }
                ]
            );
            assert_eq!(
                state(&session),
                (gold - plan.cost(), format!("{expected:?}"))
            );
        }
    }
}

#[test]
fn repair_plan_rejects_invalid_npc_context_and_death_without_mutation() {
    for case in 0..6 {
        let mut session = repair_session(false);
        place_item(&mut session, false, 9030);
        match case {
            0 => {
                session
                    .app
                    .world_mut()
                    .resource_mut::<NpcStateResource>()
                    .active_npc_service = None
            }
            1 => {
                let npc = entity_by_object_id(session.app.world(), 4990).unwrap();
                session.app.world_mut().despawn(npc);
            }
            2 => {
                session
                    .app
                    .world_mut()
                    .resource_mut::<NpcStateResource>()
                    .active_npc_service
                    .as_mut()
                    .unwrap()
                    .label_key = "SREPAIR".into()
            }
            3 => {
                session
                    .app
                    .world_mut()
                    .resource_mut::<NpcStateResource>()
                    .active_npc_service
                    .as_mut()
                    .unwrap()
                    .script_key = "missing-repair-script".into()
            }
            4 => {
                let player = player_entity(session.app.world()).unwrap();
                session
                    .app
                    .world_mut()
                    .entity_mut(player)
                    .insert(Position(Point { x: 400, y: 271 }));
                session
                    .app
                    .world_mut()
                    .resource_mut::<PlayerRuntimeResource>()
                    .player_position = Point { x: 400, y: 271 };
            }
            5 => {
                let player = player_entity(session.app.world()).unwrap();
                let vitals = *session
                    .app
                    .world()
                    .entity(player)
                    .get::<PlayerVitals>()
                    .unwrap();
                session
                    .app
                    .world_mut()
                    .entity_mut(player)
                    .insert(PlayerVitals { hp: 0, ..vitals });
                session
                    .app
                    .world_mut()
                    .resource_mut::<PlayerRuntimeResource>()
                    .player_vitals
                    .hp = 0;
            }
            _ => unreachable!(),
        }
        let before = state(&session);
        assert!(matches!(
            prepare_repair_plan(session.app.world(), 9030, false),
            Err(RepairPlanRejection::Silent)
        ));
        assert!(repair_item_impl(session.app.world_mut(), 9030, false).is_empty());
        assert_eq!(state(&session), before, "NPC/death rejection case {case}");
    }
}

#[test]
fn repair_plan_preserves_type_bind_order_and_legacy_insufficient_gold_behavior() {
    let mut session = repair_session(false);
    place_item(&mut session, false, 9030);
    let cost = prepare_repair_plan(session.app.world(), 9030, false)
        .unwrap()
        .cost();
    assert!(cost > 0);
    session
        .app
        .world_mut()
        .resource_mut::<PlayerRuntimeResource>()
        .gold = cost - 1;
    let before = state(&session);
    assert!(repair_item_impl(session.app.world_mut(), 9030, false).is_empty());
    assert_eq!(state(&session), before);
    for dont_repair in [false, true] {
        let (key, rejection) = if dont_repair {
            let template = mir2_game_data::crystal_item_by_index(578).unwrap();
            (
                crate::runtime::items::crystal_item_key_for_template(&template),
                RepairPlanRejection::CannotRepairItem,
            )
        } else {
            (
                "red-potion".to_string(),
                RepairPlanRejection::CannotRepairItemHere,
            )
        };
        session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_items[0]
            .key = key;
        let before = state(&session);
        assert!(
            matches!(prepare_repair_plan(session.app.world(), 9030, false), Err(actual) if actual == rejection)
        );
        let packets = repair_item_impl(session.app.world_mut(), 9030, false);
        assert_eq!(packets.len(), 1);
        assert!(matches!(packets[0], ServerPacket::Chat { .. }));
        assert_eq!(state(&session), before);
    }
}

#[test]
fn repair_plan_keeps_equipment_rental_bind_and_special_repair_restrictions() {
    for (special, flags) in [
        (false, CRYSTAL_BIND_DONT_REPAIR),
        (true, CRYSTAL_BIND_NO_SREPAIR),
    ] {
        let mut session = repair_session(special);
        place_item(&mut session, true, 0);
        session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>()
            .equipment_items[0]
            .rental_binding_flags = flags;
        let before = state(&session);
        assert!(matches!(
            prepare_repair_plan(session.app.world(), 0, special),
            Err(RepairPlanRejection::CannotRepairItem)
        ));
        let packets = repair_item_impl(session.app.world_mut(), 0, special);
        assert_eq!(packets.len(), 1);
        assert!(matches!(packets[0], ServerPacket::Chat { .. }));
        assert_eq!(state(&session), before);
    }
}

#[test]
fn repair_plan_rejects_stale_recursive_metadata_and_changed_reference_resolution() {
    for equipped in [false, true] {
        let mut session = repair_session(false);
        let uid = if equipped { 0 } else { 9030 };
        place_item(&mut session, equipped, uid);
        let plan = prepare_repair_plan(session.app.world(), uid, false).unwrap();
        let mut staged = session.app.world().resource::<InventoryResource>().clone();
        let sockets = if equipped {
            &mut staged.equipment_items[0].socketed
        } else {
            &mut staged.inventory_items[0].socketed
        };
        sockets[0].socketed[0].description.push_str(" changed");
        let before = format!("{staged:?}");
        assert!(!plan.apply_to_inventory(&mut staged));
        assert_eq!(format!("{staged:?}"), before);
        let mut staged = session.app.world().resource::<InventoryResource>().clone();
        let metadata = if equipped {
            &mut staged.equipment_items[0].user_item_metadata
        } else {
            &mut staged.inventory_items[0].user_item_metadata
        };
        metadata.as_mut().unwrap().slots[1]
            .as_mut()
            .unwrap()
            .refined_value += 1;
        let before = format!("{staged:?}");
        assert!(!plan.apply_to_inventory(&mut staged));
        assert_eq!(format!("{staged:?}"), before);
        let mut staged = session.app.world().resource::<InventoryResource>().clone();
        assert!(plan.apply_to_inventory(&mut staged));
        let once = format!("{staged:?}");
        assert!(!plan.apply_to_inventory(&mut staged));
        assert_eq!(format!("{staged:?}"), once);
    }
    let mut session = repair_session(false);
    place_item(&mut session, false, 0);
    let plan = prepare_repair_plan(session.app.world(), 0, false).unwrap();
    let mut staged = session.app.world().resource::<InventoryResource>().clone();
    staged.equipment_items.push(equipment_state_from_item_state(
        &rich_item(9),
        EquipmentSlot::Weapon,
    ));
    let before = format!("{staged:?}");
    assert!(
        !plan.apply_to_inventory(&mut staged),
        "new equipment shadows the old bag reference"
    );
    assert_eq!(format!("{staged:?}"), before);
}
