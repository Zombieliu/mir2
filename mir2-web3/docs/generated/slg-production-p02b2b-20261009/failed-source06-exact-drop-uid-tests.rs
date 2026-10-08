//! Controlled File issuance through actual pickup/custody planners. No live server.
use super::*;
use super::super::item_grants::tests::{fill_bag, inventory_image, trusted_session};
use crate::{SimulationSession, UserItemUidAllocator};
use mir2_protocol::{ClientPacket, UserItemExpireInfo, UserItemRentalInformation, UserItemSealedInfo};
use std::fs;

fn payload(name: &str) -> GroundDropItemPayload {
    let template = mir2_game_data::crystal_item_by_name(name).unwrap();
    let mut state = embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
    state.unique_id = 600_001;
    GroundDropItemPayload { item: try_user_item_from_item_state(&state).unwrap(), uid_assigned: false }
}

fn nested_payload() -> GroundDropItemPayload {
    let mut root = payload("RedTiger");
    root.item.current_dura = 22_222;
    root.item.max_dura = 33_333;
    root.item.identified = false;
    root.item.soul_bound_id = 77;
    root.item.cursed = true;
    root.item.awake_type = 2;
    root.item.awake_values = vec![4, 5];
    root.item.added_stats = vec![UserItemStat { stat: 17, value: 6 }];
    root.item.refined_value = 6;
    root.item.refine_added = 7;
    root.item.refine_success_chance = 88;
    root.item.wedding_ring = 23;
    root.item.expire_info = Some(UserItemExpireInfo { expiry_binary_datetime: 123_456 });
    root.item.rental_information = Some(UserItemRentalInformation {
        owner_name: "owner".into(), binding_flags: 3,
        expiry_binary_datetime: 234_567, rental_locked: true,
    });
    root.item.sealed_info = Some(UserItemSealedInfo {
        expiry_binary_datetime: 345_678, next_seal_binary_datetime: 456_789,
    });
    root.item.is_shop_item = true;
    root.item.gm_made = true;
    let mut bell = payload("BronzeBell").item;
    bell.unique_id = 600_002;
    let mut ring_a = payload("CopperRing").item;
    ring_a.unique_id = 600_003;
    let mut ring_b = ring_a.clone();
    ring_b.unique_id = 600_004;
    // Repeated child templates exercise captured socket identity and holes.
    root.item.slots = vec![None, Some(bell), Some(ring_a), Some(ring_b)];
    root
}

fn template_for(payload: &GroundDropItemPayload) -> CrystalItemTemplate {
    crystal_item_by_index(payload.item.item_index).unwrap()
}

fn preview(session: &SimulationSession, payload: &GroundDropItemPayload) -> bool {
    let template = template_for(payload);
    can_gain_exact_ground_drop_item(session.app.world(), ItemContainer::Bag1,
        &crystal_item_key_for_template(&template), &template.name, "drop uid test", 8,
        u32::from(payload.item.count), payload)
}

fn collect(session: &mut SimulationSession, payload: &GroundDropItemPayload) -> Option<Vec<ItemState>> {
    let template = template_for(payload);
    add_exact_ground_drop_items(session.app.world_mut(), ItemContainer::Bag1,
        &crystal_item_key_for_template(&template), &template.name, "drop uid test", 8,
        u32::from(payload.item.count), payload)
}

fn set_ids(item: &mut UserItem, next: &mut u64) {
    item.unique_id = *next;
    *next += 1;
    for child in item.slots.iter_mut().flatten() { set_ids(child, next); }
}

fn clear_ids(item: &mut UserItem) {
    item.unique_id = 0;
    for child in item.slots.iter_mut().flatten() { clear_ids(child); }
}

fn same_authority_session(allocator: &UserItemUidAllocator) -> SimulationSession {
    let config = SimulationConfig::default().with_item_uid_allocator(allocator.clone()).unwrap();
    let mut session = SimulationSession::new(config);
    let packets = session.handle_packet(ClientPacket::Login { account_id: "demo".into(), password: "demo".into() });
    assert!(packets.iter().any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let mut inv = session.app.world_mut().resource_mut::<InventoryResource>();
    inv.inventory_items.clear();
    inv.belt_items.clear();
    drop(inv);
    session
}

fn carried(payload: &GroundDropItemPayload, container: ItemContainer, slot: u8, uid: u64, count: u16) -> ItemState {
    let template = template_for(payload);
    let base = embedded_item_state_from_template(&template, container, slot);
    let mut raw = payload.item.clone();
    raw.unique_id = uid;
    raw.count = count;
    try_item_state_from_user_item(base, &raw).unwrap()
}

#[test]
fn nested_drop_preview_is_pure_and_commit_mints_each_raw_node_once() {
    let (mut session, allocator, path) = trusted_session("drop-preview", 5_000_000);
    let mut payload = nested_payload();
    clear_ids(&mut payload.item);
    let source = payload.item.clone();
    let file_before = fs::read(&path).unwrap();
    let inventory_before = inventory_image(&session);
    for _ in 0..10 { assert!(preview(&session, &payload)); }
    assert_eq!(fs::read(&path).unwrap(), file_before);
    assert_eq!(inventory_image(&session), inventory_before);
    assert_eq!(payload.item, source);
    let changed = collect(&mut session, &payload).unwrap();
    assert_eq!(changed.len(), 1);
    let mut gained = try_user_item_from_item_state(&changed[0]).unwrap();
    assert_eq!(gained.unique_id, 5_000_001);
    assert_eq!(gained.slots[1].as_ref().unwrap().unique_id, 5_000_002);
    assert_eq!(gained.slots[2].as_ref().unwrap().unique_id, 5_000_003);
    assert_eq!(gained.slots[3].as_ref().unwrap().unique_id, 5_000_004);
    assert_eq!(allocator.issued_through().unwrap(), 5_000_004);
    let persisted = session.app.world().resource::<InventoryResource>().inventory_items.iter()
        .find(|item| item.unique_id == gained.unique_id).unwrap();
    assert_eq!(try_user_item_from_item_state(persisted).unwrap(), gained);
    clear_ids(&mut gained);
    let mut expected = source;
    clear_ids(&mut expected);
    assert_eq!(gained, expected);
}

#[test]
fn fresh_drop_sessions_share_authority_without_using_planning_ids_as_history() {
    let (mut first, allocator, _) = trusted_session("drop-two-sessions", 5_000_000);
    let mut second = same_authority_session(&allocator);
    let source = payload("GoldOre");
    let a = collect(&mut first, &source).unwrap()[0].unique_id;
    let b = collect(&mut second, &source).unwrap()[0].unique_id;
    assert_eq!(a, 5_000_001);
    assert_eq!(b, 5_000_002);
    assert_eq!(allocator.issued_through().unwrap(), b);
}

#[test]
fn fresh_raw_preflight_keeps_original_shape_and_committed_carrier_limits() {
    let (mut session, allocator, _) = trusted_session("drop-raw-limits", 5_000_000);
    let good = payload("GoldOre");
    let template = template_for(&good);
    let mut deep = good.clone();
    for _ in 0..9 {
        let mut parent = good.item.clone();
        parent.slots = vec![Some(deep.item)];
        deep.item = parent;
    }
    let mut slots = good.clone();
    slots.item.slots = vec![None; 65];
    let mut stats = good.clone();
    stats.item.added_stats = vec![UserItemStat { stat: 1, value: 1 }; 257];
    let mut awake = good.clone();
    awake.item.awake_values = vec![1; 65];
    let mut child_zero_count = nested_payload();
    child_zero_count.item.slots[1].as_mut().unwrap().count = 0;
    let mut wrong_template = good.clone();
    wrong_template.item.item_index = -1;
    let before = inventory_image(&session);
    let floor = allocator.issued_through().unwrap();
    for invalid in [deep, slots, stats, awake, child_zero_count, wrong_template] {
        let key = if invalid.item.item_index < 0 { crystal_item_key_for_template(&template) }
            else { crystal_item_key_for_template(&template_for(&invalid)) };
        assert!(!can_gain_exact_ground_drop_item(session.app.world(), ItemContainer::Bag1,
            &key, "invalid", "invalid", 8, u32::from(invalid.item.count), &invalid));
        assert!(add_exact_ground_drop_items(session.app.world_mut(), ItemContainer::Bag1,
            &key, "invalid", "invalid", 8, u32::from(invalid.item.count), &invalid).is_none());
        assert_eq!(inventory_image(&session), before);
        assert_eq!(allocator.issued_through().unwrap(), floor);
    }
}

#[test]
fn failed_capacity_discards_earlier_merge_and_never_mints() {
    let (mut session, allocator, _) = trusted_session("drop-full-merge", 5_000_000);
    fill_bag(&mut session);
    let mut source = payload("(HP)DrugSmall");
    let max = template_for(&source).stack_size.max(1);
    assert!(max > 2);
    source.item.count = 2;
    let existing = carried(&source, ItemContainer::Bag1, 0, 400_001, max - 1);
    let belt = (0..6).map(|slot| carried(&source, ItemContainer::Belt, slot, 410_000 + u64::from(slot), max)).collect();
    let mut inv = session.app.world_mut().resource_mut::<InventoryResource>();
    inv.inventory_items[0] = existing;
    inv.belt_items = belt;
    drop(inv);
    let before = inventory_image(&session);
    let floor = allocator.issued_through().unwrap();
    assert!(!preview(&session, &source));
    assert!(collect(&mut session, &source).is_none());
    assert_eq!(inventory_image(&session), before);
    assert_eq!(allocator.issued_through().unwrap(), floor);
}

#[test]
fn local_pickup_uid_failure_keeps_ground_object_and_emits_no_gain() {
    let (mut session, _, path) = trusted_session("drop-pickup-fail", 5_000_000);
    let source = nested_payload();
    let template = template_for(&source);
    let world = session.app.world_mut();
    let player = super::super::components::player_entity(world).unwrap();
    let position = super::super::components::entity_position(world, player).unwrap();
    super::super::drops::spawn_ground_drop(world, position, "uid-test", None,
        super::super::drops::DropLoot::InventoryItem {
            key: crystal_item_key_for_template(&template), name: template.name.clone(),
            description: "frozen".into(), weight: u16::from(template.weight),
            durability_current: None, durability_max: None, added_attack: 0,
            added_defence: 0, added_stats: Vec::new(), cursed: false,
            socket_slots: template.slots, show_group_pickup: false, exact_item: Some(source),
        }, 1, template.name);
    let before = inventory_image(&session);
    let ground_before = session.world_snapshot().ground_drops;
    assert_eq!(ground_before.len(), 1);
    fs::write(path, "corrupt authority").unwrap();
    let packets = session.handle_packet(ClientPacket::PickUp);
    assert_eq!(inventory_image(&session), before);
    assert_eq!(session.world_snapshot().ground_drops, ground_before);
    assert!(!packets.iter().any(|packet| matches!(packet, ServerPacket::GainedItem { .. })));
}

#[test]
fn actual_pickup_emits_final_uid_and_normal_relogin_preserves_it() {
    let (mut session, allocator, _) = trusted_session("drop-pickup-save", 5_000_000);
    let source = payload("GoldOre");
    let template = template_for(&source);
    let world = session.app.world_mut();
    let player = super::super::components::player_entity(world).unwrap();
    let position = super::super::components::entity_position(world, player).unwrap();
    super::super::drops::spawn_ground_drop(world, position, "uid-save-test", None,
        super::super::drops::DropLoot::InventoryItem {
            key: crystal_item_key_for_template(&template), name: template.name.clone(),
            description: "saved".into(), weight: u16::from(template.weight),
            durability_current: None, durability_max: None, added_attack: 0,
            added_defence: 0, added_stats: Vec::new(), cursed: false,
            socket_slots: template.slots, show_group_pickup: false, exact_item: Some(source),
        }, 1, template.name);
    let packets = session.handle_packet(ClientPacket::PickUp);
    let gained = packets.iter().find_map(|packet| match packet {
        ServerPacket::GainedItem { item } => Some(item.clone()), _ => None,
    }).unwrap();
    assert_eq!(gained.unique_id, 5_000_001);
    assert!(session.world_snapshot().ground_drops.is_empty());
    let config = session.app.world().resource::<RuntimeConfigResource>().config.clone();
    session.handle_packet(ClientPacket::LogOut);
    let mut restored = SimulationSession::new(config);
    restored.handle_packet(ClientPacket::Login { account_id: "demo".into(), password: "demo".into() });
    restored.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let inv = restored.app.world().resource::<InventoryResource>();
    let saved = inv.inventory_items.iter().chain(inv.belt_items.iter())
        .find(|item| item.unique_id == gained.unique_id).unwrap();
    assert_eq!(try_user_item_from_item_state(saved).unwrap(), gained);
    assert_eq!(allocator.issued_through().unwrap(), gained.unique_id);
}

#[test]
fn mid_tree_exhaustion_burns_prefix_without_delivering_any_item() {
    let (mut session, allocator, _) = trusted_session("drop-exhausted", u64::MAX - 1);
    let source = nested_payload();
    let before = inventory_image(&session);
    assert!(preview(&session, &source));
    assert_eq!(allocator.issued_through().unwrap(), u64::MAX - 1);
    assert!(collect(&mut session, &source).is_none());
    assert_eq!(allocator.issued_through().unwrap(), u64::MAX);
    assert_eq!(inventory_image(&session), before);
    assert!(collect(&mut session, &source).is_none());
    assert_eq!(inventory_image(&session), before);
}

#[test]
fn assigned_custody_preserves_entire_nested_tree_without_new_issuance() {
    let (mut session, allocator, _) = trusted_session("drop-custody", 5_000_000);
    let mut source = nested_payload();
    set_ids(&mut source.item, &mut 4_000_000);
    source.uid_assigned = true;
    let floor = allocator.issued_through().unwrap();
    assert!(preview(&session, &source));
    let gained = collect(&mut session, &source).unwrap();
    assert_eq!(try_user_item_from_item_state(&gained[0]).unwrap(), source.item);
    assert_eq!(allocator.issued_through().unwrap(), floor);
}

#[test]
fn custody_conflicts_cannot_hide_inside_full_stack_absorption() {
    let (mut session, allocator, _) = trusted_session("drop-custody-conflict", 5_000_000);
    let mut source = payload("(HP)DrugSmall");
    let max = template_for(&source).stack_size.max(1);
    let existing = carried(&source, ItemContainer::Bag1, 0, 400_001, max - 1);
    session.app.world_mut().resource_mut::<InventoryResource>().inventory_items.push(existing);
    source.uid_assigned = true;
    let before = inventory_image(&session);
    let floor = allocator.issued_through().unwrap();
    source.item.unique_id = 400_001;
    assert!(!preview(&session, &source));
    assert!(collect(&mut session, &source).is_none());
    assert_eq!(inventory_image(&session), before);
    source.item.unique_id = 400_555;
    session.app.world_mut().resource_mut::<InventoryResource>().reserved_item_unique_ids.insert(400_555);
    let reserved_before = inventory_image(&session);
    assert!(!preview(&session, &source));
    assert!(collect(&mut session, &source).is_none());
    assert_eq!(inventory_image(&session), reserved_before);
    assert_eq!(allocator.issued_through().unwrap(), floor);
}

#[test]
fn absorbed_custody_source_raises_retired_id_floor_without_minting() {
    let (mut session, allocator, _) = trusted_session("drop-retired-id", 5_000_000);
    let mut source = payload("(HP)DrugSmall");
    let max = template_for(&source).stack_size.max(1);
    let existing = carried(&source, ItemContainer::Bag1, 0, 400_001, max - 1);
    session.app.world_mut().resource_mut::<InventoryResource>().inventory_items.push(existing);
    source.item.unique_id = 6_000_000;
    source.uid_assigned = true;
    assert!(preview(&session, &source));
    assert_eq!(allocator.issued_through().unwrap(), 5_000_000);
    let gained = collect(&mut session, &source).unwrap();
    assert_eq!(gained.len(), 1);
    assert_eq!(gained[0].unique_id, 400_001);
    assert_eq!(gained[0].quantity, u32::from(max));
    assert_eq!(allocator.issued_through().unwrap(), 6_000_000);
    let fresh = collect(&mut session, &payload("GoldOre")).unwrap();
    assert_eq!(fresh[0].unique_id, 6_000_001);
}

#[test]
fn fresh_merge_only_is_fenced_and_full_bag_never_uses_out_of_range_preference() {
    let (mut session, allocator, _) = trusted_session("drop-fence-slot", 5_000_000);
    let source = payload("(HP)DrugSmall");
    let max = template_for(&source).stack_size.max(1);
    let existing = carried(&source, ItemContainer::Bag1, 0, 400_001, max - 1);
    session.app.world_mut().resource_mut::<InventoryResource>().inventory_items.push(existing);
    for policy in [ItemUidIssuance::Unavailable, ItemUidIssuance::Fenced] {
        session.app.world_mut().resource_mut::<InventoryResource>().item_uid_issuance = policy;
        let before = inventory_image(&session);
        assert!(preview(&session, &source));
        assert!(collect(&mut session, &source).is_none());
        assert_eq!(inventory_image(&session), before);
    }
    session.app.world_mut().resource_mut::<InventoryResource>().item_uid_issuance = ItemUidIssuance::Durable(allocator.clone());
    fill_bag(&mut session);
    let source = payload("GoldOre");
    let template = template_for(&source);
    let before = inventory_image(&session);
    assert!(!can_gain_exact_ground_drop_item(session.app.world(), ItemContainer::Bag1,
        &crystal_item_key_for_template(&template), &template.name, "full", 255, 1, &source));
    assert!(add_exact_ground_drop_items(session.app.world_mut(), ItemContainer::Bag1,
        &crystal_item_key_for_template(&template), &template.name, "full", 255, 1, &source).is_none());
    assert_eq!(inventory_image(&session), before);
    assert_eq!(allocator.issued_through().unwrap(), 5_000_000);
}
