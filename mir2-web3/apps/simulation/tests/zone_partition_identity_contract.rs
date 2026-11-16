use mir2_protocol::{MirClass, MirDirection, MirGender, Point};
use mir2_simulation::{
    EffectEnvelope, EffectReplayReceipt, EntityLifeRef, LogicalWorldId, PartitionKey,
    PayloadDigest, SessionId, StableActionId, TopologyRevision, WorldMapEffectOwner,
    WorldPartitionKey, ZoneCommand, ZoneJoin, ZoneKey, ZoneManager,
};

fn world(line: &str) -> LogicalWorldId {
    LogicalWorldId::legacy_execution_domain(line).unwrap()
}
fn action(sequence: u64) -> StableActionId {
    StableActionId::from_source_sequence(world("map:0:line:1"), ZoneKey::for_map("0"), sequence)
        .unwrap()
}
fn entity(life: u64) -> EntityLifeRef {
    EntityLifeRef::player(
        world("map:0:line:1"),
        ZoneKey::for_map("0"),
        101,
        "online-namespace".into(),
        3,
        life,
    )
    .unwrap()
}
fn effect(target: EntityLifeRef, payload: &[u8], topology: u64, epoch: u64) -> EffectEnvelope {
    EffectEnvelope::new(
        action(7),
        Some(entity(0)),
        Some(target),
        100,
        600,
        0,
        PayloadDigest::of_bytes(payload),
        TopologyRevision::new(topology).unwrap(),
        epoch,
    )
    .unwrap()
}

#[test]
fn equal_legacy_zone_keys_on_distinct_lines_have_distinct_global_identities() {
    let legacy = PartitionKey::whole_zone(ZoneKey::for_map("0"));
    let old_bytes = serde_json::to_vec(&legacy).unwrap();
    let a = WorldPartitionKey::new(world("map:0:line:1"), legacy.clone());
    let b = WorldPartitionKey::new(world("map:0:line:2"), legacy.clone());
    assert_ne!(a, b, "lines are separate worlds despite equal old ZoneKeys");
    assert_eq!(serde_json::to_vec(a.partition()).unwrap(), old_bytes);
    assert_eq!(serde_json::to_vec(b.partition()).unwrap(), old_bytes);
    assert_ne!(
        WorldMapEffectOwner::for_map(a.world().clone(), legacy.zone.clone()),
        WorldMapEffectOwner::for_map(b.world().clone(), legacy.zone)
    );
    for invalid in ["", " map:0", "map:0 ", "line\n1"] {
        assert!(LogicalWorldId::legacy_execution_domain(invalid).is_err());
    }
    assert!(serde_json::from_str::<LogicalWorldId>("\"arbitrary-unversioned-world\"").is_err());
}

#[test]
fn server_sequences_round_trip_and_exhaustion_never_recycles_an_action() {
    assert!(
        StableActionId::from_source_sequence(world("map:0"), ZoneKey::for_map("0"), 0).is_err()
    );
    let max = action(u64::MAX);
    assert!(max.checked_successor().is_err());
    assert_eq!(max.source_sequence(), u64::MAX);
    assert_eq!(action(9).checked_successor().unwrap(), action(10));
    let bytes = serde_json::to_vec(&action(9)).unwrap();
    assert_eq!(
        serde_json::from_slice::<StableActionId>(&bytes).unwrap(),
        action(9)
    );
    let original: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for (field, value) in [("version", 2), ("source_sequence", 0)] {
        let mut corrupt = original.clone();
        corrupt[field] = value.into();
        assert!(serde_json::from_value::<StableActionId>(corrupt).is_err());
    }
    let mut unknown = original;
    unknown["host_id"] = "physical-host-a".into();
    assert!(serde_json::from_value::<StableActionId>(unknown).is_err());
}

#[test]
fn every_new_versioned_reference_rejects_a_future_schema() {
    let value = effect(entity(0), b"damage:7", 1, 4);
    let mut partition = serde_json::to_value(WorldPartitionKey::new(
        world("map:0"),
        PartitionKey::whole_zone(ZoneKey::for_map("0")),
    ))
    .unwrap();
    partition["version"] = 99.into();
    assert!(serde_json::from_value::<WorldPartitionKey>(partition).is_err());
    let mut life = serde_json::to_value(entity(0)).unwrap();
    life["version"] = 99.into();
    assert!(serde_json::from_value::<EntityLifeRef>(life).is_err());
    let mut owner = serde_json::to_value(value.effect_owner()).unwrap();
    owner["version"] = 99.into();
    assert!(serde_json::from_value::<WorldMapEffectOwner>(owner).is_err());
    let mut receipt = serde_json::to_value(EffectReplayReceipt::for_effect(&value)).unwrap();
    receipt["version"] = 99.into();
    assert!(serde_json::from_value::<EffectReplayReceipt>(receipt).is_err());
    let mut nested = serde_json::to_value(action(7)).unwrap();
    nested["zone"]["physical_host"] = "host-a".into();
    assert!(serde_json::from_value::<StableActionId>(nested).is_err());
    let mut nested = serde_json::to_value(value.effect_owner()).unwrap();
    nested["map_owner"]["unknown_effect_epoch"] = 7.into();
    assert!(serde_json::from_value::<WorldMapEffectOwner>(nested).is_err());
}

#[test]
fn routing_and_fencing_changes_do_not_make_a_new_business_effect() {
    let first = effect(entity(0), b"damage:7", 1, 4);
    let moved = effect(entity(0), b"damage:7", 2, 8);
    assert_ne!(
        first, moved,
        "routing observations must remain available for fresh fencing"
    );
    assert_eq!(first.action(), moved.action());
    assert_eq!(first.effect_owner(), moved.effect_owner());
    let receipt = EffectReplayReceipt::for_effect(&first);
    receipt.validate_replay(&moved).unwrap();
    let bytes = serde_json::to_vec(&moved).unwrap();
    let restored: EffectEnvelope = serde_json::from_slice(&bytes).unwrap();
    receipt.validate_replay(&restored).unwrap();
    assert_eq!(restored.placement_epoch(), 8);
    assert_eq!(restored.topology_revision().value(), 2);
    // Receipt comparison does not validate a lease or publish a second effect.
}

#[test]
fn same_action_cannot_replay_different_damage_target_life_or_due_time() {
    let first = effect(entity(0), b"damage:7", 1, 4);
    let receipt = EffectReplayReceipt::for_effect(&first);
    for conflict in [
        effect(entity(0), b"damage:70", 1, 4),
        effect(entity(1), b"damage:7", 1, 4),
    ] {
        assert!(receipt.validate_replay(&conflict).is_err());
    }
    let mut later = serde_json::to_value(&first).unwrap();
    later["due_at_ms"] = 601.into();
    let later: EffectEnvelope = serde_json::from_value(later).unwrap();
    assert!(receipt.validate_replay(&later).is_err());
    assert!(first.validate_payload(b"damage:70").is_err());
    first.validate_payload(b"damage:7").unwrap();
}

#[test]
fn foreign_world_reused_object_and_unknown_effect_schema_fail_closed() {
    let foreign =
        EntityLifeRef::monster(world("map:0:line:2"), ZoneKey::for_map("0"), 101, 1).unwrap();
    assert!(EffectEnvelope::new(
        action(7),
        Some(entity(0)),
        Some(foreign),
        100,
        600,
        0,
        PayloadDigest::of_bytes(b"damage:7"),
        TopologyRevision::WHOLE_ZONE,
        4
    )
    .is_err());
    let first = effect(entity(0), b"damage:7", 1, 4);
    let original = serde_json::to_value(&first).unwrap();
    for (field, value) in [("version", 2), ("placement_epoch", 0), ("due_at_ms", 99)] {
        let mut corrupt = original.clone();
        corrupt[field] = value.into();
        assert!(serde_json::from_value::<EffectEnvelope>(corrupt).is_err());
    }
    for malformed in ["", "a", &"G".repeat(64), &"A".repeat(64), &"0".repeat(65)] {
        assert!(serde_json::from_value::<PayloadDigest>(malformed.into()).is_err());
    }
    let old = EntityLifeRef::monster(world("map:0:line:1"), ZoneKey::for_map("0"), 200, 1).unwrap();
    let respawn =
        EntityLifeRef::monster(world("map:0:line:1"), ZoneKey::for_map("0"), 200, 2).unwrap();
    assert!(old.validate_current(&respawn).is_err());
    assert!(EntityLifeRef::monster(world("map:0"), ZoneKey::for_map("0"), 200, 0).is_err());
    assert!(EntityLifeRef::player(
        world("map:0"),
        ZoneKey::for_map("0"),
        101,
        "ns".into(),
        0,
        0
    )
    .is_err());
}

fn join() -> ZoneJoin {
    ZoneJoin {
        session_id: SessionId::new("owner"),
        account_id: "account".into(),
        character_index: 1,
        object_id: 101,
        name: "Owner".into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 7,
        hp: 60,
        max_hp: 60,
        mp: 30,
        map_file_name: "identity-contract".into(),
        position: Point { x: 10, y: 10 },
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }
}
fn current(manager: &ZoneManager) -> EntityLifeRef {
    let session = SessionId::new("owner");
    let proof: serde_json::Value =
        serde_json::from_str(&manager.online_owner_proof_for_session(&session).unwrap()).unwrap();
    EntityLifeRef::player(
        world("actual-owner-domain"),
        manager.zone_key_for_session(&session).unwrap(),
        proof["object_id"].as_u64().unwrap() as u32,
        proof["namespace"].as_str().unwrap().into(),
        proof["epoch"].as_u64().unwrap(),
        manager.player_life_generation(&session).unwrap(),
    )
    .unwrap()
}

#[test]
fn actual_owner_death_revive_and_fresh_join_reject_delayed_old_life() {
    let mut manager = ZoneManager::new();
    manager.handle(ZoneCommand::Join(join()));
    let initial = current(&manager);
    let admitted = EffectEnvelope::new(
        StableActionId::from_source_sequence(initial.world().clone(), initial.zone().clone(), 1)
            .unwrap(),
        Some(initial.clone()),
        Some(initial.clone()),
        10,
        510,
        0,
        PayloadDigest::of_bytes(b"heal:5"),
        TopologyRevision::WHOLE_ZONE,
        1,
    )
    .unwrap();
    admitted
        .validate_current(Some(&initial), Some(&initial), b"heal:5")
        .unwrap();
    for (hp, dead) in [(0, true), (60, false)] {
        manager.handle(ZoneCommand::SyncPlayerVitalsAndLife {
            session_id: SessionId::new("owner"),
            hp,
            max_hp: 60,
            mp: 30,
            dead,
        });
    }
    let revived = current(&manager);
    assert_ne!(
        initial, revived,
        "real revival must advance life without reusing effect rights"
    );
    let before = manager.checkpoint_bytes().unwrap();
    assert!(admitted
        .validate_current(Some(&revived), Some(&revived), b"heal:5")
        .is_err());
    assert_eq!(
        before,
        manager.checkpoint_bytes().unwrap(),
        "rejected description cannot change HP or receipts"
    );
    manager.handle(ZoneCommand::Leave {
        session_id: SessionId::new("owner"),
    });
    manager.handle(ZoneCommand::Join(join()));
    let relogin = current(&manager);
    assert!(
        initial.validate_current(&relogin).is_err(),
        "same object and initial life must not reuse an old admission"
    );
    assert!(revived.validate_current(&relogin).is_err());
}

#[test]
fn cold_world_bytes_cannot_reconstitute_a_current_life_reference() {
    let mut manager = ZoneManager::new();
    manager.handle(ZoneCommand::Join(join()));
    let old = current(&manager);
    let bytes = manager.checkpoint_bytes().unwrap();
    let restored = ZoneManager::restore_checkpoint(&bytes).unwrap();
    assert_eq!(
        restored.player_transform(&SessionId::new("owner")),
        manager.player_transform(&SessionId::new("owner"))
    );
    assert!(restored
        .online_owner_proof_for_session(&SessionId::new("owner"))
        .is_none());
    let encoded = serde_json::to_vec(&old).unwrap();
    let description: EntityLifeRef = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(
        description, old,
        "an imported description still needs a current owner query"
    );
}
