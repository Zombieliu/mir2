use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    CellCoord, CellSize, MapEffectOwnerRef, PartitionId, PartitionKey, SessionId, TopologyRevision,
    ZoneCommand, ZoneJoin, ZoneKey, ZoneManager, ZoneOutbound,
};

fn join(key: &ZoneKey, session: &str, object_id: u32, x: i32) -> ZoneJoin {
    ZoneJoin {
        session_id: SessionId::new(session),
        account_id: format!("{session}-account"),
        character_index: object_id as i32,
        object_id,
        name: session.to_string(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 7,
        hp: 60,
        max_hp: 60,
        mp: 100,
        map_file_name: key.map_file_name.clone(),
        position: Point { x, y: 65 },
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }
}

#[test]
fn cell_geometry_validates_deserialization_and_uses_euclidean_boundaries() {
    for invalid in [i32::MIN, -64, -1, 0] {
        assert!(CellSize::new(invalid).is_err());
        assert!(serde_json::from_value::<CellSize>(invalid.into()).is_err());
    }
    let size = CellSize::new(64).unwrap();
    for (position, expected) in [
        (Point { x: 0, y: 63 }, CellCoord { x: 0, y: 0 }),
        (Point { x: 64, y: 127 }, CellCoord { x: 1, y: 1 }),
        (Point { x: -1, y: -64 }, CellCoord { x: -1, y: -1 }),
        (Point { x: -65, y: 128 }, CellCoord { x: -2, y: 2 }),
        (
            Point {
                x: i32::MIN,
                y: i32::MAX,
            },
            CellCoord {
                x: -33_554_432,
                y: 33_554_431,
            },
        ),
    ] {
        assert_eq!(CellCoord::at(&position, size), expected);
    }
    assert_eq!(
        CellCoord::at(&Point { x: 64, y: 0 }, CellSize::new(32).unwrap()).x,
        2
    );
    assert_eq!(CellSize::new(i32::MAX).unwrap().tiles(), i32::MAX);
    assert_eq!(serde_json::from_str::<CellSize>("64").unwrap(), size);
}

#[test]
fn effect_owner_and_topology_versions_reject_unsupported_internal_data() {
    assert!(TopologyRevision::new(0).is_err());
    assert!(serde_json::from_str::<TopologyRevision>("0").is_err());
    let key = ZoneKey::for_map("p1-arena");
    let owner = MapEffectOwnerRef::for_zone(key.clone());
    let bytes = serde_json::to_vec(&owner).unwrap();
    assert_eq!(
        serde_json::from_slice::<MapEffectOwnerRef>(&bytes).unwrap(),
        owner
    );
    assert_eq!(owner.version(), 1);
    assert_eq!(owner.zone(), &key);
    let mut invalid: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    invalid["version"] = 2.into();
    assert!(serde_json::from_value::<MapEffectOwnerRef>(invalid).is_err());
}

#[test]
fn partition_identity_preserves_all_logical_zone_namespaces() {
    let keys = [
        ZoneKey::new("primary", "p1-arena", 0, "main"),
        ZoneKey::new("other", "p1-arena", 0, "main"),
        ZoneKey::new("primary", "other-map", 0, "main"),
        ZoneKey::new("primary", "p1-arena", 1, "main"),
        ZoneKey::new("primary", "p1-arena", 0, "instance"),
    ];
    let mut manager = ZoneManager::new();
    for (index, key) in keys.iter().enumerate() {
        manager.handle_for_key(
            key.clone(),
            ZoneCommand::Join(join(key, &format!("player-{index}"), index as u32 + 1, 63)),
        );
    }
    let directory = manager.partition_directory();
    assert_eq!(directory.len(), keys.len());
    for key in &keys {
        let partition = manager.partition_for_key(key).unwrap();
        assert_eq!(partition, PartitionKey::whole_zone(key.clone()));
        assert_eq!(manager.zone_for_partition(&partition).unwrap().key(), key);
        let low = manager.partition_at(key, &Point { x: -65, y: 0 }).unwrap();
        let high = manager.partition_at(key, &Point { x: 128, y: 0 }).unwrap();
        assert_ne!(low.cell, high.cell);
        assert_eq!(low.partition, high.partition);
        assert_eq!(low.effect_owner, high.effect_owner);
        assert_eq!(low.effect_owner.zone(), key);
        assert_eq!(low.topology_revision, TopologyRevision::WHOLE_ZONE);
    }
}

#[test]
fn authoritative_cell_crossing_preserves_actor_and_map_effect_owner_until_leave() {
    // Unknown terrain is intentionally not used as a movement acceptance arena.
    // This exercises the trusted authoritative-transform and save contracts.
    let key = ZoneKey::for_map("p1-transform-contract");
    let session = SessionId::new("walker");
    let mut manager = ZoneManager::new();
    assert!(manager.partition_for_key(&key).is_none());
    assert!(manager.partition_for_session(&session).is_none());
    manager.handle_for_key(
        key.clone(),
        ZoneCommand::Join(join(&key, "walker", 101, 63)),
    );
    let partition = manager.partition_for_session(&session).unwrap();
    let before = manager
        .partition_at(&key, &manager.player_transform(&session).unwrap().0)
        .unwrap();
    let actor_id = manager
        .zone_for_partition(&partition)
        .unwrap()
        .player_object_id(&session);
    let life = manager.player_life_generation(&session);
    let out = manager.handle(ZoneCommand::SyncPlayerTransform {
        session_id: session.clone(),
        position: Point { x: 64, y: 65 },
        direction: MirDirection::Right,
    });
    let transform = manager.player_transform(&session).unwrap();
    assert_eq!(transform, (Point { x: 64, y: 65 }, MirDirection::Right));
    let after = manager.partition_at(&key, &transform.0).unwrap();
    assert_ne!(before.cell, after.cell);
    assert_eq!(before.partition, after.partition);
    assert_eq!(before.effect_owner, after.effect_owner);
    assert_eq!(
        actor_id,
        manager
            .zone_for_partition(&partition)
            .unwrap()
            .player_object_id(&session)
    );
    assert_eq!(life, manager.player_life_generation(&session));
    assert!(out.iter().any(|event| matches!(event,
        ZoneOutbound::SaveTransform { session_id, position, direction }
        if session_id == &session && position == &transform.0 && direction == &transform.1
    )));
    let left = manager.handle(ZoneCommand::Leave {
        session_id: session.clone(),
    });
    assert!(left.iter().any(|event| matches!(event,
        ZoneOutbound::SaveTransform { session_id, position, direction }
        if session_id == &session && position == &transform.0 && direction == &transform.1
    )));
    assert!(manager.partition_for_session(&session).is_none());
    assert!(manager.player_transform(&session).is_none());
    assert_eq!(
        manager.partition_directory().len(),
        1,
        "persistent map remains after its last player leaves"
    );
    assert_eq!(
        manager
            .zone_for_partition(&partition)
            .unwrap()
            .player_count(),
        0
    );
}

#[test]
#[cfg(feature = "test-support")]
fn partition_dispatch_matches_existing_key_dispatch_including_visibility_and_save() {
    let key = ZoneKey::for_map("p1-dispatch-contract");
    let partition = PartitionKey::whole_zone(key.clone());
    let mut keyed = ZoneManager::new();
    keyed.handle_for_key(key.clone(), ZoneCommand::Join(join(&key, "alice", 101, 63)));
    let mut partitioned = keyed.fork_for_deterministic_reference();
    let mut observed_player = false;
    for command in [
        ZoneCommand::Join(join(&key, "bob", 102, 65)),
        ZoneCommand::SyncPlayerTransform {
            session_id: SessionId::new("alice"),
            position: Point { x: 64, y: 65 },
            direction: MirDirection::Down,
        },
        ZoneCommand::Turn {
            session_id: SessionId::new("bob"),
            direction: MirDirection::Left,
            now_ms: 10,
        },
        ZoneCommand::Leave {
            session_id: SessionId::new("alice"),
        },
    ] {
        let expected = keyed.handle_for_key(key.clone(), command.clone());
        let actual = partitioned
            .handle_for_partition(partition.clone(), command)
            .unwrap();
        assert_eq!(actual, expected);
        observed_player |= actual.iter().any(|event| match event {
            ZoneOutbound::ToSession { packets, .. }
            | ZoneOutbound::ToMany { packets, .. }
            | ZoneOutbound::ToAll { packets } => packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::ObjectPlayer { .. })),
            _ => false,
        });
        assert_eq!(
            partitioned.checkpoint_bytes().unwrap(),
            keyed.checkpoint_bytes().unwrap()
        );
    }
    assert!(
        observed_player,
        "two nearby actors must be visible in the actual outbound stream"
    );
}

#[test]
fn unknown_partition_is_rejected_before_zone_or_session_mutation() {
    let mut manager = ZoneManager::new();
    let key = ZoneKey::for_map("p1-unknown-partition");
    let unknown = PartitionKey {
        zone: key.clone(),
        partition_id: PartitionId(7),
    };
    let before = manager.checkpoint_bytes().unwrap();
    assert!(manager
        .handle_for_partition(
            unknown.clone(),
            ZoneCommand::Join(join(&key, "alice", 1, 63))
        )
        .is_err());
    assert!(manager.zone_for_partition(&unknown).is_none());
    assert!(manager.partition_directory().is_empty());
    assert_eq!(manager.checkpoint_bytes().unwrap(), before);
}

#[test]
fn directory_is_derived_after_v2_and_v3_restore_without_checkpoint_changes() {
    let key = ZoneKey::for_map("p1-checkpoint-contract");
    let session = SessionId::new("restored-player");
    let mut manager = ZoneManager::new();
    manager.handle_for_key(
        key.clone(),
        ZoneCommand::Join(join(&key, "restored-player", 101, 63)),
    );
    let original = manager.checkpoint_bytes().unwrap();
    let directory = manager.partition_directory();
    let _ = manager.partition_at(&key, &Point { x: 64, y: 65 }).unwrap();
    assert_eq!(
        manager.checkpoint_bytes().unwrap(),
        original,
        "directory queries are read-only projections"
    );
    let checkpoint: serde_json::Value = serde_json::from_slice(&original).unwrap();
    assert_eq!(checkpoint["version"], 3);
    assert!(checkpoint.get("partitions").is_none());
    assert!(checkpoint.get("topology_revision").is_none());
    for version in [2, 3] {
        let mut fixture = checkpoint.clone();
        fixture["version"] = version.into();
        if version == 2 {
            fixture.as_object_mut().unwrap().remove("state_root");
        }
        let restored =
            ZoneManager::restore_checkpoint(&serde_json::to_vec(&fixture).unwrap()).unwrap();
        assert_eq!(restored.partition_directory(), directory);
        assert_eq!(
            restored.partition_for_session(&session),
            Some(PartitionKey::whole_zone(key.clone()))
        );
        assert_eq!(
            restored.player_transform(&session),
            manager.player_transform(&session)
        );
        assert_eq!(
            restored.player_life_generation(&session),
            manager.player_life_generation(&session)
        );
    }
}

#[test]
#[cfg(feature = "test-support")]
fn partition_directory_preserves_parallel_and_sequential_tick_equivalence() {
    fn populate() -> ZoneManager {
        let mut manager = ZoneManager::new();
        for index in 0..8 {
            let key = ZoneKey::for_map(format!("p1-tick-{index}"));
            let partition = PartitionKey::whole_zone(key.clone());
            for (name, object, x) in [("alice", 101, 63), ("bob", 102, 65)] {
                let session = format!("{index}-{name}");
                manager
                    .handle_for_partition(
                        partition.clone(),
                        ZoneCommand::Join(join(&key, &session, object, x)),
                    )
                    .unwrap();
                // Terrain is unavailable in this fixture, so exercise actual
                // per-Zone timed regeneration rather than rejected movement.
                manager.handle(ZoneCommand::SyncPlayerVitals {
                    session_id: SessionId::new(session),
                    hp: 40,
                    max_hp: 100,
                    mp: 100,
                });
            }
        }
        manager
    }
    let mut parallel = populate();
    let mut sequential = parallel.fork_for_deterministic_reference();
    assert_eq!(parallel.partition_directory().len(), 8);
    let mut healed_sessions = std::collections::BTreeSet::new();
    let mut health_owner_sessions = std::collections::BTreeSet::new();
    let mut healing_pulses = 0;
    for time in [10, 100, 1_000, 4_011, 8_012] {
        let actual = parallel.tick_all(time);
        let reference = sequential.tick_all_sequential(time);
        assert_eq!(actual, reference, "complete outbound order at {time}");
        let mut pulse_heals = 0;
        for event in &actual {
            match event {
                ZoneOutbound::PlayerHealed {
                    session_id,
                    amount,
                    settlement: Some(settlement),
                } => {
                    assert!(*amount > 0);
                    assert_eq!(settlement.hp_after - settlement.hp_before, *amount);
                    assert!(!settlement.death_transition);
                    healed_sessions.insert(session_id.clone());
                    pulse_heals += 1;
                }
                ZoneOutbound::OwnerHealthChanged { change } => {
                    assert!(change.hp > change.hp_before);
                    health_owner_sessions.insert(change.cursor.session_id.clone());
                }
                _ => {}
            }
        }
        if pulse_heals > 0 {
            assert_eq!(
                pulse_heals, 16,
                "every actor in all eight Zones regenerates"
            );
            healing_pulses += 1;
        }
        assert_eq!(
            parallel.checkpoint_bytes().unwrap(),
            sequential.checkpoint_bytes().unwrap()
        );
    }
    assert_eq!(healing_pulses, 2, "both real timed regeneration pulses ran");
    assert_eq!(healed_sessions.len(), 16);
    assert_eq!(health_owner_sessions, healed_sessions);
}
