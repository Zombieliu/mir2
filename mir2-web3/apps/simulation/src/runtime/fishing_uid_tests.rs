use super::*;
use super::super::item_grants::tests::{trusted_session, inventory_image};
use super::super::resources::set_runtime_tick;
use crate::SimulationSession;
use std::fs;

fn reel_tick(session: &mut SimulationSession) {
    let mut fishing = session.app.world_mut().resource_mut::<FishingResource>();
    fishing.fishing_attribute = 0;
    fishing.progress_percent = 100;
    fishing.chance_percent = 100;
    fishing.chance_counter = 3;
    drop(fishing);
    for tick in 1..10_001 {
        set_runtime_tick(session.app.world_mut(), tick);
        if fishing_reel_succeeds(session.app.world()) && matches!(resolved_fishing_drop(session.app.world()),
            Some(ResolvedDropTemplate::Item { name, .. }) if name == "SwordFish") { return; }
    }
    panic!("source fishing table must yield SwordFish");
}

#[test]
fn fishing_source_loot_has_same_authority_uid_and_final_packet_count() {
    let (mut session, allocator, _) = trusted_session("fish-good", 5_000_000);
    reel_tick(&mut session);
    let outcome = complete_fishing_reel(session.app.world_mut());
    let items = outcome.packets.iter().filter_map(|packet| if let ServerPacket::GainedItem { item } = packet { Some(item) } else { None }).collect::<Vec<_>>();
    assert!(!items.is_empty());
    assert!(items.iter().all(|item| item.unique_id > 5_000_000 && item.count > 0));
    assert!(allocator.issued_through().unwrap() > 5_000_000);
    assert_eq!(session.app.world().resource::<FishingResource>().chance_counter, 0);
}

#[test]
fn fishing_issuance_failure_cancels_autocast_without_success_event_or_reel_cost() {
    let (mut session, _, path) = trusted_session("fish-fail", 5_000_000);
    reel_tick(&mut session);
    let before = inventory_image(&session);
    let chance = session.app.world().resource::<FishingResource>().chance_counter;
    fs::write(path, "broken").unwrap();
    let outcome = complete_fishing_reel(session.app.world_mut());
    assert!(outcome.cancel_autocast);
    assert_eq!(inventory_image(&session), before);
    assert_eq!(session.app.world().resource::<FishingResource>().chance_counter, chance + 1);
    assert!(!outcome.packets.iter().any(|packet| matches!(packet, ServerPacket::GainedItem { .. } | ServerPacket::DuraChanged { .. })));
    assert!(outcome.packets.iter().any(|packet| matches!(packet, ServerPacket::Chat { message, .. } if message.contains("temporarily unavailable"))));
}
