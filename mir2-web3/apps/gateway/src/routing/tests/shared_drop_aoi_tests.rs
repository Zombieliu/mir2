//! Recipient-local AOI removal must not retire a shared ground item.
use super::*;

fn item_fixture() -> (
    SharedInProcessZoneSessionRuntime,
    SharedInProcessZoneSessionRuntime,
    Arc<Mutex<SharedInProcessZoneState>>,
    GroundDropSnapshot,
) {
    let state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut owner = shared_session_runtime(state.clone());
    start_demo_runtime(&mut owner);
    let item = owner
        .world_snapshot()
        .inventory_items
        .into_iter()
        .find(|item| item.key == "red-potion" && item.quantity > 0)
        .expect("ordinary demo inventory contains a droppable potion");
    let packets = owner
        .execute(WorldCommand::ClientPacket(ClientPacket::DropItem {
            unique_id: item.unique_id,
            count: 1,
            hero_inventory: false,
        }))
        .expect("ordinary item drop");
    let object_id = packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectItem { info } => Some(info.object_id),
            _ => None,
        })
        .expect("drop creates an actual ObjectItem");
    let drop = owner
        .world_snapshot()
        .ground_drops
        .into_iter()
        .find(|drop| drop.object_id == object_id)
        .expect("drop enters shared map snapshot");
    let mut observer = shared_session_runtime(state.clone());
    start_new_runtime(&mut observer, "drop-aoi-observer", "DropObserver");
    assert!(observer
        .world_snapshot()
        .ground_drops
        .iter()
        .any(|candidate| candidate.object_id == object_id));
    {
        let state = state.lock().unwrap();
        let zone = state.zone_manager.zone(&ZoneKey::for_map("0")).unwrap();
        assert!(zone.retains_object_id(object_id));
        assert!(zone.has_ground_drop(object_id));
    }
    (owner, observer, state, drop)
}

#[test]
fn shared_item_aoi_leave_preserves_global_drop_and_reentry_projection() {
    let (owner, observer, state, drop) = item_fixture();
    let key = observer.current_presence_key().expect("observer presence");
    let session = observer
        .current_zone_session_id()
        .expect("observer Zone session");
    let (return_position, return_direction) = state
        .lock()
        .unwrap()
        .zone_manager
        .player_transform(&session)
        .expect("observer transform");
    {
        let mut state = state.lock().unwrap();
        let out = state.zone_manager.handle(ZoneCommand::SyncPlayerTransform {
            session_id: session.clone(),
            position: Point {
                x: drop.x + 64,
                y: drop.y + 64,
            },
            direction: MirDirection::Down,
        });
        assert!(out.iter().any(|outbound| matches!(
            outbound,
            ZoneOutbound::ToSession { session_id, packets }
                if session_id == &session && packets.iter().any(|packet| matches!(
                    packet, ServerPacket::ObjectRemove { object_id } if *object_id == drop.object_id
                ))
        )), "the real Zone must emit the recipient's AOI remove");
        assert!(state
            .zone_manager
            .zone(&ZoneKey::for_map("0"))
            .unwrap()
            .retains_object_id(drop.object_id));
        state.dispatch_zone_outbounds(out, Some(&key));
        let map = state.maps.get("0").unwrap();
        assert_eq!(map.ground_drops.get(&drop.object_id), Some(&drop));
        assert!(!map.removed_drop_ids.contains(&drop.object_id));
        assert!(map.drop_expires_at_ms.contains_key(&drop.object_id));
    }
    assert!(owner
        .world_snapshot()
        .ground_drops
        .iter()
        .any(|candidate| candidate.object_id == drop.object_id));
    assert!(!observer
        .world_snapshot()
        .ground_drops
        .iter()
        .any(|candidate| candidate.object_id == drop.object_id));
    {
        let mut state = state.lock().unwrap();
        let out = state.zone_manager.handle(ZoneCommand::SyncPlayerTransform {
            session_id: session,
            position: return_position,
            direction: return_direction,
        });
        let (packets, ..) = state.dispatch_zone_outbounds(out, Some(&key));
        assert!(
            packets.iter().any(|packet| matches!(
                packet, ServerPacket::ObjectItem { info } if info.object_id == drop.object_id
            )),
            "reentering AOI must show the same shared item"
        );
    }
    assert!(observer
        .world_snapshot()
        .ground_drops
        .iter()
        .any(|candidate| candidate.object_id == drop.object_id));
}

#[test]
fn shared_item_real_pickup_removes_zone_object_and_global_drop() {
    let (mut owner, mut observer, state, drop) = item_fixture();
    // The observer joins after the drop. Its spawn collision search can choose
    // that same adjacent cell, so vacate it before moving the owner onto it.
    // PickUp intentionally has no object ID and requires the exact shared cell.
    let observer_session = observer
        .current_zone_session_id()
        .expect("observer Zone session");
    observer.dispatch_zone_player_command(
        ZoneCommand::SyncPlayerTransform {
            session_id: observer_session.clone(),
            position: Point {
                x: drop.x + 4,
                y: drop.y + 4,
            },
            direction: MirDirection::Down,
        },
        false,
    );
    assert_ne!(
        state
            .lock()
            .unwrap()
            .zone_manager
            .player_transform(&observer_session)
            .map(|(position, _)| position),
        Some(Point {
            x: drop.x,
            y: drop.y
        }),
        "observer must vacate the drop cell before the owner approaches"
    );
    let session = owner.current_zone_session_id().expect("owner Zone session");
    let relocation = owner.dispatch_zone_player_command(
        ZoneCommand::SyncPlayerTransform {
            session_id: session.clone(),
            position: Point {
                x: drop.x,
                y: drop.y,
            },
            direction: MirDirection::Down,
        },
        false,
    );
    assert_eq!(
        state
            .lock()
            .unwrap()
            .zone_manager
            .player_transform(&session)
            .map(|(position, _)| position),
        Some(Point {
            x: drop.x,
            y: drop.y
        }),
        "owner must occupy the authoritative pickup cell: {relocation:?}"
    );
    let packets = owner
        .execute(WorldCommand::ClientPacket(ClientPacket::PickUp))
        .expect("ordinary pickup on the actual shared drop cell");
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedItem { .. })),
        "the actual pickup must grant the item: {packets:?}"
    );
    let state = state.lock().unwrap();
    let zone = state.zone_manager.zone(&ZoneKey::for_map("0")).unwrap();
    assert!(!zone.retains_object_id(drop.object_id));
    assert!(!zone.has_ground_drop(drop.object_id));
    let map = state.maps.get("0").unwrap();
    assert!(!map.ground_drops.contains_key(&drop.object_id));
    assert!(map.removed_drop_ids.contains(&drop.object_id));
    assert!(!map.drop_expires_at_ms.contains_key(&drop.object_id));
    std::mem::drop(state);
    assert!(!observer
        .world_snapshot()
        .ground_drops
        .iter()
        .any(|candidate| candidate.object_id == drop.object_id));
}
