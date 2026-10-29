use super::super::{items, resources::InventoryResource};
use super::*;
use crate::{ItemContainer, SimulationConfig, SimulationSession};
use mir2_protocol::{ClientPacket, MirClass, MirGender, ServerPacket};

fn session_with(item: ItemState) -> SimulationSession {
    let mut s = SimulationSession::new(SimulationConfig::default());
    s.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    s.handle_packet(ClientPacket::StartGame { character_index: 0 });
    s.handle_packet(ClientPacket::NewHero {
        name: "Aide".into(),
        gender: MirGender::Female,
        class: MirClass::Taoist,
    });
    s.app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .inventory_items = vec![item];
    s
}
fn legacy() -> ItemState {
    let t = mir2_game_data::crystal_item_by_index(658).unwrap();
    let mut item = items::embedded_item_state_from_template(&t, ItemContainer::Bag1, 0);
    item.unique_id = 0;
    item.user_item_metadata = None;
    item
}
fn state(s: &SimulationSession) -> serde_json::Value {
    let i = s.app.world().resource::<InventoryResource>();
    let h = s.app.world().resource::<HeroInventoryResource>();
    serde_json::json!({"inventory": i.inventory_items, "belt": i.belt_items,
      "storage": i.storage_items, "equipment": i.equipment_items,
      "reserved": i.reserved_item_unique_ids, "hero": h.items, "heroEquipment": h.equipment})
}
fn transfer(s: &mut SimulationSession, success: bool) {
    let packets = s.handle_packet(ClientPacket::TransferHeroItem { from: 0, to: 0 });
    assert!(
        packets.contains(&ServerPacket::TransferHeroItem {
            from: 0,
            to: 0,
            success
        }),
        "{packets:?}"
    );
}
#[test]
fn hero_custody_legacy_zero_exact_or_nested_identity_is_never_reassigned() {
    for nested in [false, true] {
        let item = legacy();
        let mut wire = items::try_user_item_from_item_state(&item).unwrap();
        wire.unique_id = 0;
        if nested {
            let mut child = wire.clone();
            child.unique_id = 987654;
            wire.slots = vec![Some(child)];
        }
        let mut exact = items::try_item_state_from_user_item(item, &wire).unwrap();
        if nested {
            exact.user_item_metadata = None;
        }
        let mut s = session_with(exact);
        let before = state(&s);
        transfer(&mut s, false);
        assert_eq!(state(&s), before);
    }
}
#[test]
fn hero_custody_legacy_zero_migration_commits_once_and_survives_relogin() {
    let item = legacy();
    let mut expected = items::try_user_item_from_item_state(&item).unwrap();
    let mut s = session_with(item);
    transfer(&mut s, true);
    let h = s.app.world().resource::<HeroInventoryResource>();
    let actual = items::try_user_item_from_item_state(&h.items[0]).unwrap();
    assert_ne!(actual.unique_id, 0);
    expected.unique_id = actual.unique_id;
    assert_eq!(actual, expected);
    let i = s.app.world().resource::<InventoryResource>();
    assert!(i.inventory_items.is_empty());
    assert!(i.reserved_item_unique_ids.contains(&actual.unique_id));
    let ids = validate_hero_custody(h).unwrap();
    assert_eq!(ids.len(), 1);
    s.handle_packet(ClientPacket::LogOut);
    s.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let h = s.app.world().resource::<HeroInventoryResource>();
    assert_eq!(h.items.len(), 1);
    assert_eq!(
        items::try_user_item_from_item_state(&h.items[0]).unwrap(),
        expected
    );
}
#[test]
fn hero_custody_legacy_zero_post_allocation_failure_preserves_carriers_and_allocator() {
    let mut item = legacy();
    item.quantity = 100_000;
    let mut s = session_with(item);
    let before = state(&s);
    transfer(&mut s, false);
    assert_eq!(state(&s), before);
    // A successful retry must choose exactly the identity a fresh equivalent
    // state chooses: the rejected staged allocation consumed nothing.
    s.app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .inventory_items[0]
        .quantity = 1;
    let mut fresh = session_with(legacy());
    transfer(&mut s, true);
    transfer(&mut fresh, true);
    let uid = |s: &SimulationSession| {
        items::try_user_item_from_item_state(
            &s.app.world().resource::<HeroInventoryResource>().items[0],
        )
        .unwrap()
        .unique_id
    };
    assert_eq!(uid(&s), uid(&fresh));
}
