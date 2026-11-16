use super::*;
fn fixture() -> (
    crate::SimulationSession,
    SimulationConfig,
    Stage5FriendIdentity,
    CharacterSaveRecord,
    InventoryResource,
) {
    let mut session = super::super::tests::session();
    session.grant_shared_guild_creation_from_npc();
    session.submit_shared_guild_name("ItemBank").unwrap();
    let mut resources = session.app.world_mut().resource_mut::<InventoryResource>();
    let horn = resources
        .inventory_items
        .iter_mut()
        .find(|item| item.name == "WoomaHorn")
        .unwrap();
    horn.identified = Some(true);
    horn.cursed = true;
    horn.gem_count = 2;
    horn.description = "exact state survives bank".into();
    let mut second = horn.clone();
    second.unique_id = 8_999_991;
    second.slot = 31;
    second.description = "second distinct carrier".into();
    second.user_item_metadata = None;
    resources.inventory_items.push(second);
    drop(resources);
    let config = session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .clone();
    let identity = world_identity(session.app.world()).unwrap();
    let save = session.active_character_checkpoint().unwrap();
    let baseline = session.app.world().resource::<InventoryResource>().clone();
    (session, config, identity, save, baseline)
}
#[test]
fn guild_bank_items_preserve_full_carrier_and_swap_both_depositors() {
    let (_session, config, identity, save, mut baseline) = fixture();
    let original = baseline
        .inventory_items
        .iter()
        .find(|item| item.name == "WoomaHorn")
        .unwrap()
        .clone();
    let slot = i32::from(inventory::inventory_index_for_item(&original).unwrap());
    let first = config
        .commit_shared_guild_item(&identity, &save, &baseline, 0, slot, 111)
        .unwrap();
    let guild = config
        .shared_guild_for_identity(&identity)
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&guild.storage[&111].item_state_json).unwrap(),
        serde_json::to_value(&original).unwrap()
    );
    assert_eq!(
        guild.storage[&111].depositor_character_index,
        identity.character_index
    );
    assert!(matches!(
        first.packets.last(),
        Some(ServerPacket::GuildStorageItemChange {
            change_type: 0,
            to: 111,
            item: Some(_),
            ..
        })
    ));
    baseline.inventory_items = first.inventory;
    let second = config
        .commit_shared_guild_item(&identity, &first.save, &baseline, 0, 31, 0)
        .unwrap();
    baseline.inventory_items = second.inventory;
    let moved = config
        .commit_shared_guild_item(&identity, &second.save, &baseline, 2, 111, 0)
        .unwrap();
    let guild = config
        .shared_guild_for_identity(&identity)
        .unwrap()
        .unwrap();
    assert_eq!(
        stored_item(&guild.storage[&0]).unwrap().0.description,
        original.description
    );
    assert_eq!(
        stored_item(&guild.storage[&111]).unwrap().0.description,
        "second distinct carrier"
    );
    let retrieved = config
        .commit_shared_guild_item(&identity, &moved.save, &baseline, 1, 0, slot)
        .unwrap();
    assert_eq!(
        serde_json::to_value(
            retrieved
                .inventory
                .iter()
                .find(|item| items::item_unique_id(item) == items::item_unique_id(&original))
                .unwrap()
        )
        .unwrap(),
        serde_json::to_value(&original).unwrap()
    );
    assert_eq!(
        config
            .shared_guild_for_identity(&identity)
            .unwrap()
            .unwrap()
            .storage
            .len(),
        1
    );
}
#[test]
fn guild_bank_items_failed_commit_invalid_slots_occupied_target_and_uid_collision_are_atomic() {
    let (_session, config, identity, save, baseline) = fixture();
    let item = baseline
        .inventory_items
        .iter()
        .find(|item| item.name == "WoomaHorn")
        .unwrap();
    let slot = i32::from(inventory::inventory_index_for_item(item).unwrap());
    for (from, to) in [(slot, 112), (-1, 0), (80, 0)] {
        assert!(config
            .commit_shared_guild_item(&identity, &save, &baseline, 0, from, to)
            .is_err());
    }
    config
        .inject_account_store_transaction_fault(crate::AccountStoreTransactionFault::BeforePersist);
    assert!(config
        .commit_shared_guild_item(&identity, &save, &baseline, 0, slot, 0)
        .is_err());
    assert!(config
        .shared_guild_for_identity(&identity)
        .unwrap()
        .unwrap()
        .storage
        .is_empty());
    let deposit = config
        .commit_shared_guild_item(&identity, &save, &baseline, 0, slot, 0)
        .unwrap();
    assert!(
        config
            .commit_shared_guild_item(&identity, &save, &baseline, 0, slot, 1)
            .is_err(),
        "stale replay cannot debit twice"
    );
    assert!(
        config
            .commit_shared_guild_item(&identity, &deposit.save, &baseline, 1, 0, slot)
            .is_err(),
        "identity collision is rejected rather than silently renumbered"
    );
    let mut current = baseline.clone();
    current.inventory_items = deposit.inventory;
    assert!(
        config
            .commit_shared_guild_item(&identity, &deposit.save, &current, 0, 31, 0)
            .is_err(),
        "bank deposit does not merge or overwrite"
    );
    assert!(
        config
            .commit_shared_guild_item(&identity, &deposit.save, &current, 1, 0, 31)
            .is_err(),
        "retrieval does not overwrite or merge"
    );
    assert_eq!(
        config
            .shared_guild_for_identity(&identity)
            .unwrap()
            .unwrap()
            .storage
            .len(),
        1
    );
}
#[test]
fn guild_bank_list_is_once_per_session_and_contains_all_112_slots() {
    let (mut session, _, _, _, _) = fixture();
    let packets = session.change_shared_guild_item(3, 0, 0).unwrap();
    assert!(packets
        .iter()
        .any(|packet| matches!(packet,ServerPacket::GuildStorageList{items} if items.len()==112)));
    assert!(session
        .change_shared_guild_item(3, 0, 0)
        .unwrap()
        .is_empty());
    session.clear_shared_guild_grant();
    assert!(!session
        .change_shared_guild_item(3, 0, 0)
        .unwrap()
        .is_empty());
}
