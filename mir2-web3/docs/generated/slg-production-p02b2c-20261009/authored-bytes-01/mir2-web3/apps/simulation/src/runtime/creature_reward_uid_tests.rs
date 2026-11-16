//! Real UseItem reward publication under a controlled File authority.
use super::*;
use super::super::item_grants::tests::{inventory_image, trusted_session};
use super::super::items::{embedded_item_state_from_template, try_user_item_from_item_state, ItemState};
use super::super::resources::{InventoryResource, RuntimeConfigResource, SessionResource};
use crate::{ItemContainer, SimulationConfig, SimulationSession, UserItemUidAllocator};
use mir2_protocol::{ClientPacket, MirGridType, UserItem};
use std::fs;

fn prepare_box(session: &mut SimulationSession, name: &str, uid: u64) -> (CrystalItemTemplate, ItemState) {
    session.app.world_mut().resource_mut::<SessionResource>()
        .selected_character.as_mut().unwrap().level = 100;
    let player = super::super::components::player_entity(session.app.world()).unwrap();
    session.app.world_mut().entity_mut(player)
        .get_mut::<super::super::components::CharacterBody>().unwrap().level = 100;
    let template = mir2_game_data::crystal_item_by_name(name).unwrap();
    let mut source = embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
    source.unique_id = uid;
    session.app.world_mut().resource_mut::<InventoryResource>().inventory_items.push(source.clone());
    (template, source)
}

fn use_box(session: &mut SimulationSession, uid: u64) -> Vec<ServerPacket> {
    session.handle_packet(ClientPacket::UseItem { unique_id: uid, grid: MirGridType::Inventory })
}

fn gained(packets: &[ServerPacket]) -> UserItem {
    let items: Vec<_> = packets.iter().filter_map(|packet| match packet {
        ServerPacket::GainedItem { item } => Some(item.clone()),
        _ => None,
    }).collect();
    assert_eq!(items.len(), 1, "{packets:?}");
    assert_eq!(packets.iter().filter(|packet| matches!(packet, ServerPacket::UseItem { success: true, .. })).count(), 1);
    items[0].clone()
}

fn assert_failed_use(session: &mut SimulationSession, uid: u64) {
    let before = inventory_image(session);
    let packets = use_box(session, uid);
    assert!(packets.iter().any(|packet| matches!(packet, ServerPacket::UseItem { success: false, .. })), "{packets:?}");
    assert!(!packets.iter().any(|packet| matches!(packet, ServerPacket::UseItem { success: true, .. } | ServerPacket::GainedItem { .. })));
    assert_eq!(inventory_image(session), before);
}

fn second_session(allocator: &UserItemUidAllocator) -> SimulationSession {
    let config = SimulationConfig::default().with_item_uid_allocator(allocator.clone()).unwrap();
    let mut session = SimulationSession::new(config);
    let packets = session.handle_packet(ClientPacket::Login { account_id: "demo".into(), password: "demo".into() });
    assert!(packets.iter().any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
    inventory.inventory_items.clear();
    inventory.belt_items.clear();
    drop(inventory);
    session
}

#[test]
fn actual_box_mints_above_retired_source_and_preserves_complete_reward_on_relogin() {
    let (mut session, allocator, _) = trusted_session("box-retired", 5_000_000);
    prepare_box(&mut session, "WonderBox(L)", 6_000_000);
    let reward = gained(&use_box(&mut session, 6_000_000));
    assert_eq!(reward.unique_id, 6_000_001);
    assert_eq!(allocator.issued_through().unwrap(), reward.unique_id);
    assert_eq!(reward.count, 1);
    let inventory = session.app.world().resource::<InventoryResource>();
    assert!(!inventory.inventory_items.iter().chain(&inventory.belt_items).any(|item| item.unique_id == 6_000_000));
    let item = inventory.inventory_items.iter().chain(&inventory.belt_items)
        .find(|item| item.unique_id == reward.unique_id).unwrap();
    assert_eq!(item.slot, 0, "reuse exactly the source's freed cell");
    assert_eq!(try_user_item_from_item_state(item).unwrap(), reward);
    let config = session.app.world().resource::<RuntimeConfigResource>().config.clone();
    session.handle_packet(ClientPacket::LogOut);
    let mut restored = SimulationSession::new(config);
    restored.handle_packet(ClientPacket::Login { account_id: "demo".into(), password: "demo".into() });
    restored.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let inventory = restored.app.world().resource::<InventoryResource>();
    let item = inventory.inventory_items.iter().chain(&inventory.belt_items)
        .find(|item| item.unique_id == reward.unique_id).unwrap();
    assert_eq!(try_user_item_from_item_state(item).unwrap(), reward);
    assert_eq!(allocator.issued_through().unwrap(), reward.unique_id);
}

#[test]
fn reward_sessions_share_one_authority_even_when_their_source_slots_are_reused() {
    let (mut first, allocator, _) = trusted_session("box-shared", 5_000_000);
    let mut second = second_session(&allocator);
    prepare_box(&mut first, "WonderBox(L)", 400_001);
    prepare_box(&mut second, "WonderBox(L)", 400_002);
    let a = gained(&use_box(&mut first, 400_001));
    let b = gained(&use_box(&mut second, 400_002));
    assert_eq!(a.unique_id, 5_000_001);
    assert_eq!(b.unique_id, 5_000_002);
    assert_eq!(allocator.issued_through().unwrap(), b.unique_id);
}

#[test]
fn corrupt_authority_keeps_box_and_emits_no_success_or_reward() {
    let (mut session, _, path) = trusted_session("box-corrupt", 5_000_000);
    prepare_box(&mut session, "WonderBox(L)", 400_001);
    fs::write(&path, b"corrupt UID state").unwrap();
    assert_failed_use(&mut session, 400_001);
    assert_eq!(fs::read(&path).unwrap(), b"corrupt UID state");
}

#[test]
fn exhausted_authority_never_falls_back_to_the_removed_source_or_slot() {
    let (mut session, allocator, _) = trusted_session("box-exhausted", u64::MAX);
    prepare_box(&mut session, "WonderBox(L)", 400_001);
    assert_failed_use(&mut session, 400_001);
    assert_eq!(allocator.issued_through().unwrap(), u64::MAX);
}

#[test]
fn unavailable_and_fenced_rewards_cannot_consume_the_box() {
    for policy in [super::super::item_uid_issuance::ItemUidIssuance::Unavailable,
        super::super::item_uid_issuance::ItemUidIssuance::Fenced] {
        let (mut session, allocator, _) = trusted_session("box-fenced", 5_000_000);
        prepare_box(&mut session, "WonderBox(L)", 400_001);
        session.app.world_mut().resource_mut::<InventoryResource>().item_uid_issuance = policy;
        assert_failed_use(&mut session, 400_001);
        assert_eq!(allocator.issued_through().unwrap(), 5_000_000);
    }
}

#[test]
fn wrong_configured_authority_cannot_mutate_either_store_or_consume_the_box() {
    let (mut session, allocator, path) = trusted_session("box-bound", 5_000_000);
    let (_, other, other_path) = trusted_session("box-other", 8_000_000);
    prepare_box(&mut session, "WonderBox(L)", 400_001);
    session.app.world_mut().resource_mut::<InventoryResource>().item_uid_issuance =
        super::super::item_uid_issuance::ItemUidIssuance::Durable(other.clone());
    let before = fs::read(&path).unwrap();
    let other_before = fs::read(&other_path).unwrap();
    assert_failed_use(&mut session, 400_001);
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(fs::read(&other_path).unwrap(), other_before);
    assert_eq!(allocator.issued_through().unwrap(), 5_000_000);
    assert_eq!(other.issued_through().unwrap(), 8_000_000);
}

#[test]
fn empty_blackstone_retires_high_source_in_history_without_minting_a_reward() {
    let (mut session, allocator, _) = trusted_session("box-empty", 5_000_000);
    let template = mir2_game_data::crystal_item_manifest().items.into_iter()
        .find(|item| item.item_type == 36 && item.shape == 21).unwrap();
    prepare_box(&mut session, &template.name, 6_000_000);
    assert!(select_creature_box_reward("00Blackstone", 1.0, |_| 0).is_none(), "the shipped empty table fixture must remain empty");
    let packets = use_box(&mut session, 6_000_000);
    assert_eq!(packets.iter().filter(|packet| matches!(packet, ServerPacket::UseItem { success: true, .. })).count(), 1);
    assert!(!packets.iter().any(|packet| matches!(packet, ServerPacket::GainedItem { .. })));
    let inventory = session.app.world().resource::<InventoryResource>();
    assert!(!inventory.inventory_items.iter().chain(&inventory.belt_items).any(|item| item.unique_id == 6_000_000));
    assert_eq!(allocator.issued_through().unwrap(), 6_000_000);
}

#[test]
fn stale_source_reference_is_rejected_before_history_floor_or_inventory_changes() {
    let (mut session, allocator, path) = trusted_session("box-reference", 5_000_000);
    let (template, mut source) = prepare_box(&mut session, "WonderBox(L)", 6_000_000);
    source.unique_id = 400_001;
    let before = inventory_image(&session);
    let file_before = fs::read(&path).unwrap();
    let (success, packets) = use_creature_reward_box(session.app.world_mut(), &template, &source,
        super::super::inventory::UseItemLocation::Inventory(0));
    assert!(!success);
    assert!(packets.is_empty());
    assert_eq!(inventory_image(&session), before);
    assert_eq!(fs::read(&path).unwrap(), file_before);
    assert_eq!(allocator.issued_through().unwrap(), 5_000_000);
}
