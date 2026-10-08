use super::*;
use crate::{SimulationConfig, SimulationSession, UserItemUidAllocator};
use mir2_protocol::{ClientPacket, ServerPacket};
use std::{fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub(in crate::runtime) fn trusted_session(label: &str, high: u64) -> (SimulationSession, UserItemUidAllocator, PathBuf) {
    let root = std::env::temp_dir().canonicalize().unwrap();
    let dir = root.join(format!("mir2-grants-{label}-{}-{}", std::process::id(), SEQUENCE.fetch_add(1, Ordering::Relaxed)));
    assert!(dir.starts_with(&root));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("uid.json");
    let allocator = UserItemUidAllocator::initialize_file(&path, high).unwrap();
    let config = SimulationConfig::default().with_item_uid_allocator(allocator.clone()).unwrap();
    let mut session = SimulationSession::new(config);
    let login = session.handle_packet(ClientPacket::Login { account_id: "demo".into(), password: "demo".into() });
    assert!(login.iter().any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    session.app.world_mut().resource_mut::<InventoryResource>().inventory_items.clear();
    session.app.world_mut().resource_mut::<InventoryResource>().belt_items.clear();
    (session, allocator, path)
}

pub(in crate::runtime) fn plain(name: &str, quantity: u32, container: ItemContainer) -> FreshItemGrant {
    let template = mir2_game_data::crystal_item_by_name(name).unwrap();
    FreshItemGrant::plain(container, &super::super::items::crystal_item_key_for_template(&template),
        &template.name, "grant test", 0, quantity, u16::from(template.weight), UserItemUidReason::QuestReward).unwrap()
}

pub(in crate::runtime) fn inventory_image(session: &SimulationSession) -> String {
    let inv = session.app.world().resource::<InventoryResource>();
    format!("{:?}|{:?}|{:?}|{:?}|{:?}", inv.inventory_items, inv.belt_items, inv.storage_items, inv.equipment_items, inv.reserved_item_unique_ids)
}

pub(in crate::runtime) fn fill_bag(session: &mut SimulationSession) {
    let template = mir2_game_data::crystal_item_by_name("CopperRing").unwrap();
    let inv = &mut *session.app.world_mut().resource_mut::<InventoryResource>();
    inv.inventory_capacity = 46;
    inv.inventory_items.clear();
    for slot in 0..40 {
        let mut item = embedded_item_state_from_template(&template, ItemContainer::Bag1, slot);
        item.unique_id = 100_000 + u64::from(slot);
        inv.inventory_items.push(item);
    }
}

#[test]
fn batch_mints_unique_roots_and_projects_final_wire_items() {
    let (mut session, allocator, _) = trusted_session("batch", 5_000_000);
    let changed = publish_world_grants(session.app.world_mut(), &[plain("GoldOre", 3, ItemContainer::Bag1)]).unwrap();
    assert_eq!(changed.len(), 3);
    let uids: BTreeSet<_> = changed.iter().map(|item| item.unique_id).collect();
    assert_eq!(uids.len(), 3);
    assert_eq!(uids, BTreeSet::from([5_000_001, 5_000_002, 5_000_003]));
    assert_eq!(allocator.issued_through().unwrap(), 5_000_003);
    for item in &changed {
        assert_eq!(super::super::items::try_user_item_from_item_state(item).unwrap().unique_id, item.unique_id);
    }
}

#[test]
fn batch_capacity_failure_does_not_keep_an_earlier_merge_or_mint() {
    let (mut session, allocator, _) = trusted_session("capacity", 5_000_000);
    fill_bag(&mut session);
    let mut potion = plain("(HP)DrugSmall", 1, ItemContainer::Bag1).prototype;
    potion.container = ItemContainer::Belt;
    potion.unique_id = 234_000;
    session.app.world_mut().resource_mut::<InventoryResource>().belt_items.push(potion);
    let before = inventory_image(&session);
    assert!(publish_world_grants(session.app.world_mut(), &[plain("(HP)DrugSmall", 1, ItemContainer::Bag1), plain("GoldOre", 1, ItemContainer::Bag1)]).is_err());
    assert_eq!(inventory_image(&session), before);
    assert_eq!(allocator.issued_through().unwrap(), 5_000_000);
}

#[test]
fn uid_exhaustion_after_one_mint_burns_id_without_partial_delivery() {
    let (mut session, allocator, _) = trusted_session("exhaustion", u64::MAX - 1);
    let before = inventory_image(&session);
    assert!(publish_world_grants(session.app.world_mut(), &[plain("GoldOre", 2, ItemContainer::Bag1)]).is_err());
    assert_eq!(inventory_image(&session), before);
    assert_eq!(allocator.issued_through().unwrap(), u64::MAX);
}

#[test]
fn fresh_batch_cannot_import_custody_or_nested_objects() {
    let (mut session, allocator, _) = trusted_session("custody", 5_000_000);
    let before = inventory_image(&session);
    let mut exact = plain("CopperRing", 1, ItemContainer::Bag1);
    exact.prototype.unique_id = 42;
    assert!(publish_world_grants(session.app.world_mut(), &[exact]).is_err());
    let mut nested = plain("CopperRing", 1, ItemContainer::Bag1);
    nested.prototype.socketed.push(plain("CopperRing", 1, ItemContainer::Bag1).prototype);
    assert!(publish_world_grants(session.app.world_mut(), &[nested]).is_err());
    assert_eq!(inventory_image(&session), before);
    assert_eq!(allocator.issued_through().unwrap(), 5_000_000);
}

#[test]
fn shadow_and_fenced_batches_cannot_grow_an_existing_stack() {
    let (mut session, allocator, _) = trusted_session("fenced", 5_000_000);
    let grant = plain("(HP)DrugSmall", 1, ItemContainer::Bag1);
    publish_world_grants(session.app.world_mut(), &[grant.clone()]).unwrap();
    let before = allocator.issued_through().unwrap();
    for policy in [ItemUidIssuance::Unavailable, ItemUidIssuance::Fenced] {
        let mut inv = session.app.world().resource::<InventoryResource>().clone();
        inv.item_uid_issuance = policy;
        assert!(plan_fresh_item_grants(&inv, &[grant.clone()]).is_err());
    }
    assert_eq!(allocator.issued_through().unwrap(), before);
}

#[test]
fn same_batch_plain_grants_share_new_stack_and_emit_its_final_count_once() {
    let (mut session, allocator, _) = trusted_session("same-stack", 5_000_000);
    let grant = plain("(HP)DrugSmall", 1, ItemContainer::Bag1);
    let changed = publish_world_grants(session.app.world_mut(), &[grant.clone(), grant]).unwrap();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].quantity, 2);
    assert_eq!(changed[0].unique_id, 5_000_001);
    assert_eq!(allocator.issued_through().unwrap(), 5_000_001);
}

#[test]
fn bound_or_cursed_stack_never_absorbs_plain_reward() {
    let (mut session, _, _) = trusted_session("binding", 5_000_000);
    let grant = plain("(HP)DrugSmall", 1, ItemContainer::Bag1);
    let mut existing = grant.prototype.clone();
    existing.unique_id = 199_001;
    existing.container = ItemContainer::Belt;
    existing.soul_bound_id = Some(0);
    existing.cursed = true;
    session.app.world_mut().resource_mut::<InventoryResource>().belt_items.push(existing);
    let changed = publish_world_grants(session.app.world_mut(), &[grant]).unwrap();
    assert_eq!(changed.len(), 1);
    assert_ne!(changed[0].unique_id, 199_001);
    assert_eq!(session.app.world().resource::<InventoryResource>().belt_items[0].quantity, 1);
    assert!(changed[0].soul_bound_id.is_none() && !changed[0].cursed);
}

#[test]
fn quest_slots_are_independent_and_fresh_grant_never_overwrites_a_full_slot() {
    let (mut session, allocator, _) = trusted_session("quest-grid", 5_000_000);
    fill_bag(&mut session);
    let grant = plain("CopperRing", 40, ItemContainer::Quest);
    let changed = publish_world_grants(session.app.world_mut(), &[grant]).unwrap();
    assert_eq!(changed.len(), 40);
    let before = inventory_image(&session);
    let high = allocator.issued_through().unwrap();
    assert!(publish_world_grants(session.app.world_mut(), &[plain("CopperRing", 1, ItemContainer::Quest)]).is_err());
    assert_eq!(inventory_image(&session), before);
    assert_eq!(allocator.issued_through().unwrap(), high);
}
