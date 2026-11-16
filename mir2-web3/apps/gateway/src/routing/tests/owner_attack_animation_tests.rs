//! Owner-facing combat animation identity must not leak into shared Zone state.
use super::*;

#[test]
fn public_melee_attack_uses_local_actor_for_owner_and_zone_actor_for_observer() {
    let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut owner = shared_session_runtime(zone_state.clone());
    let mut observer = shared_session_runtime(zone_state.clone());
    start_new_runtime(&mut owner, "swing-owner", "SwingOwner");
    start_new_runtime(&mut observer, "swing-observer", "SwingObserver");
    let owner_key = owner.current_presence_key().expect("owner presence");
    let observer_key = observer.current_presence_key().expect("observer presence");
    let mut target = shared_monster_entity(260_905);
    target.ai = Some(2);
    target.x = 331;
    target.y = 270;
    let owner_position = Point { x: 330, y: 270 };
    let observer_position = Point { x: 330, y: 271 };
    {
        let mut state = zone_state.lock().unwrap();
        state.update_player_transform(&owner_key, owner_position.clone(), MirDirection::Right);
        state.update_player_transform(&observer_key, observer_position.clone(), MirDirection::Up);
        state.sync_map_layer(
            "0".to_owned(),
            vec![target.clone()],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
    }
    owner
        .inner
        .force_authoritative_player_transform(owner_position, MirDirection::Right);
    observer
        .inner
        .force_authoritative_player_transform(observer_position, MirDirection::Up);
    owner.sync_zone_snapshot();
    observer.sync_zone_snapshot();
    let owner_local_id = owner
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .expect("owner snapshot self")
        .object_id;
    let owner_zone_id = owner.current_zone_player_object_id().expect("Zone actor");
    assert_ne!(owner_local_id, owner_zone_id);

    let owner_packets = owner
        .execute(WorldCommand::Attack {
            object_id: target.object_id,
        })
        .expect("public attack command");
    assert!(
        owner_packets.iter().any(|packet| matches!(packet,
            ServerPacket::ObjectAttack { info } if info.object_id == owner_local_id
        )),
        "owner animation must address its rendered SelfPlayer: {owner_packets:?}"
    );
    assert!(
        !owner_packets.iter().any(|packet| matches!(packet,
            ServerPacket::ObjectAttack { info } if info.object_id == owner_zone_id
        )),
        "owner must not receive a second orphaned Zone actor animation"
    );

    let observer_packets = observer.execute(WorldCommand::Tick).expect("observer tick");
    assert!(
        observer_packets.iter().any(|packet| matches!(packet,
            ServerPacket::ObjectAttack { info } if info.object_id == owner_zone_id
        )),
        "observer must animate the shared player identity: {observer_packets:?}"
    );
    assert!(
        !observer_packets.iter().any(|packet| matches!(packet,
            ServerPacket::ObjectAttack { info } if info.object_id == owner_local_id
        )),
        "owner presentation ID must not enter observer broadcasts"
    );
    let observer_snapshot = observer.world_snapshot();
    let visible_owner = observer_snapshot
        .entities
        .iter()
        .find(|entity| entity.kind == WorldEntityKind::Player && entity.name == "SwingOwner")
        .expect("observer sees owner");
    assert_eq!(visible_owner.object_id, owner_zone_id);

    let state = zone_state.lock().unwrap();
    let presence = state
        .players
        .get(&owner_key)
        .expect("authoritative presence");
    assert_eq!(presence.zone_object_id, owner_zone_id);
    assert_eq!(presence.entity.object_id, owner_zone_id);
    assert!(state
        .zone_manager
        .zone(&ZoneKey::for_map("0"))
        .is_some_and(|zone| zone.retains_object_id(owner_zone_id)));
    assert!(
        state
            .map_layer(Some("0"))
            .expect("shared map")
            .entities
            .values()
            .all(|entity| {
                !matches!(
                    entity.kind,
                    WorldEntityKind::Player | WorldEntityKind::SelfPlayer
                ) || entity.object_id != owner_local_id
            }),
        "owner packet projection must not create a personal-ID player in shared map state"
    );
}
