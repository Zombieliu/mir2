use super::*;
use crate::config::{CurrencyKind, Stage5TradeState, CRYSTAL_BASE_INVENTORY_CAPACITY};
use std::collections::BTreeMap;

#[test]
fn prepared_trade_reopen_preserves_custody_until_the_entire_offer_fits() {
    let config = SimulationConfig::default();
    let mut session = SimulationSession::new(config.clone());
    let world = session.app.world_mut();
    {
        let mut owner = world.resource_mut::<SessionResource>();
        owner.account_id = Some("owner".into());
        owner.selected_character = Some(config.default_character);
    }
    world.resource_mut::<PlayerRuntimeResource>().gold = 981;
    let filler = {
        let mut inventory = world.resource_mut::<InventoryResource>();
        inventory.inventory_items.truncate(2);
        inventory.inventory_capacity = CRYSTAL_BASE_INVENTORY_CAPACITY;
        let mut filler = inventory.inventory_items[0].clone();
        filler.unique_id = 600_001;
        filler
    };
    world
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .trade = Some(Stage5TradeState {
        settlement_nonce: "a".repeat(32),
        partner: "Partner".into(),
        offered_items: vec!["red-potion".into(), "blue-potion".into()],
        offered_slots: BTreeMap::from([(0, 0), (1, 1)]),
        offered_unique_ids: BTreeMap::from([(0, 0), (1, 1)]),
        offered_gold: 19,
        held_gold: Some(19),
        offered_currency: CurrencyKind::Gold,
        accepted: false,
        locked: false,
        escrow_prepared: false,
        completed: false,
    });
    let offer = build_shared_trade_offer(world).expect("original offer");
    assert!(matches!(
        prepare_shared_trade_escrow(world),
        SharedTradePreparation::Prepared(_)
    ));
    let prepared = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .trade
        .clone();

    // A received item occupied the legacy bag slot while the offer was held.
    // Only slot 1 is free, so restoring two items must fail atomically.
    let capacity = crate::config::crystal_bag_slot_capacity(CRYSTAL_BASE_INVENTORY_CAPACITY);
    for slot in (0..u8::try_from(capacity).unwrap()).filter(|&slot| slot != 1) {
        let mut item = filler.clone();
        item.slot = slot;
        item.unique_id += u64::from(slot);
        world
            .resource_mut::<InventoryResource>()
            .inventory_items
            .push(item);
    }
    let packets = reopen_shared_trade_offer(world, &offer);
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::Chat { .. })));
    assert!(!packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::TradeCancel { .. })));
    assert_eq!(world.resource::<PlayerRuntimeResource>().gold, 981);
    assert_eq!(
        world
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .trade,
        prepared
    );
    assert_eq!(
        world.resource::<InventoryResource>().inventory_items.len(),
        usize::from(capacity) - 1
    );

    {
        let mut inventory = world.resource_mut::<InventoryResource>();
        inventory.inventory_items.pop();
        inventory.inventory_items[0].unique_id = 0;
    }
    assert!(reopen_shared_trade_offer(world, &offer).is_empty());
    assert_eq!(
        world
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .trade,
        prepared
    );
    world.resource_mut::<InventoryResource>().inventory_items[0].unique_id = 600_001;
    let packets = reopen_shared_trade_offer(world, &offer);
    assert_eq!(packets, vec![ServerPacket::TradeCancel { unlock: true }]);
    assert!(reopen_shared_trade_offer(world, &offer).is_empty());
    let inventory = world.resource::<InventoryResource>();
    let trade = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .trade
        .as_ref()
        .unwrap();
    assert_eq!(inventory.inventory_items.len(), usize::from(capacity));
    assert_eq!(inventory.inventory_items[0].unique_id, 600_001);
    for (index, offered) in offer.items.iter().enumerate() {
        let index = index as u8;
        let item = inventory
            .inventory_items
            .iter()
            .find(|item| inventory_index_for_item(item) == Some(trade.offered_slots[&index]))
            .unwrap();
        assert_eq!(item_unique_id(item), offered.unique_id);
        assert_eq!(item.key, offered.key);
    }
    assert_eq!(trade.held_gold, Some(19));
    assert_eq!(world.resource::<PlayerRuntimeResource>().gold, 981);
}
