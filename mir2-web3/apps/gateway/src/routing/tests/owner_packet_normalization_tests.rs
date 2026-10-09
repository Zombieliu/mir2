use super::*;

#[test]
fn owner_combat_results_rebase_only_the_exact_owner_zone_identity() {
    let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(zone_state);
    start_demo_runtime(&mut runtime);
    let local_id = runtime
        .local_self_object_id()
        .expect("started session should expose its local player id");
    let zone_id = runtime
        .current_zone_player_object_id()
        .expect("started session should expose its Zone player id");
    assert_ne!(zone_id, local_id);

    let remote_id = zone_id.saturating_add(77);
    let remote_attacker_id = zone_id.saturating_add(88);
    let mut packets = vec![
        ServerPacket::ObjectStruck {
            info: ObjectStruckInfo {
                object_id: zone_id,
                attacker_id: remote_attacker_id,
                location: Point { x: 330, y: 270 },
                direction: MirDirection::Down,
            },
        },
        ServerPacket::DamageIndicator {
            damage: 9,
            damage_type: 0,
            object_id: zone_id,
        },
        ServerPacket::ObjectDied {
            info: ObjectDiedInfo {
                object_id: zone_id,
                location: Point { x: 330, y: 270 },
                direction: MirDirection::Down,
                kind: 0,
            },
        },
        ServerPacket::ObjectRevived {
            info: ObjectRevivedInfo {
                object_id: zone_id,
                effect: true,
            },
        },
        ServerPacket::ObjectPoisoned {
            object_id: zone_id,
            poison: 2,
        },
        ServerPacket::DamageIndicator {
            damage: 4,
            damage_type: 0,
            object_id: remote_id,
        },
        ServerPacket::ObjectStruck {
            info: ObjectStruckInfo {
                object_id: remote_id,
                attacker_id: zone_id,
                location: Point { x: 331, y: 270 },
                direction: MirDirection::Left,
            },
        },
    ];

    runtime.normalize_owner_state_packets(&mut packets);

    assert!(matches!(
        &packets[0],
        ServerPacket::ObjectStruck { info }
            if info.object_id == local_id && info.attacker_id == remote_attacker_id
    ));
    assert!(matches!(
        &packets[1],
        ServerPacket::DamageIndicator { object_id, .. } if *object_id == local_id
    ));
    assert!(matches!(
        &packets[2],
        ServerPacket::ObjectDied { info } if info.object_id == local_id
    ));
    assert!(matches!(
        &packets[3],
        ServerPacket::ObjectRevived { info } if info.object_id == local_id
    ));
    assert!(matches!(
        &packets[4],
        ServerPacket::ObjectPoisoned { object_id, poison }
            if *object_id == local_id && *poison == 2
    ));
    assert!(matches!(
        &packets[5],
        ServerPacket::DamageIndicator { object_id, .. } if *object_id == remote_id
    ));
    assert!(
        matches!(
            &packets[6],
            ServerPacket::ObjectStruck { info }
                if info.object_id == remote_id && info.attacker_id == local_id
        ),
        "the owner-facing hit identifies the rendered local attacker; remote targets keep their ids"
    );
}

#[test]
fn owner_magic_packets_rebase_caster_and_self_targets_without_rewriting_remote_actors() {
    let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(zone_state);
    start_demo_runtime(&mut runtime);
    let local_id = runtime.local_self_object_id().unwrap();
    let zone_id = runtime.current_zone_player_object_id().unwrap();
    let remote_id = zone_id + 77;
    let mut packets = vec![
        ServerPacket::ObjectMagic {
            object_id: zone_id,
            location: Point { x: 330, y: 270 },
            direction: MirDirection::Down,
            spell: Spell::Healing,
            target_id: zone_id,
            target: Point { x: 330, y: 270 },
            cast: true,
            level: 0,
            self_broadcast: false,
            secondary_target_ids: vec![zone_id, remote_id],
        },
        ServerPacket::Magic {
            spell: Spell::Healing,
            target_id: zone_id,
            target: Point { x: 330, y: 270 },
            cast: true,
            level: 0,
            secondary_target_ids: vec![remote_id, zone_id],
        },
        ServerPacket::ObjectMagic {
            object_id: remote_id,
            location: Point { x: 331, y: 270 },
            direction: MirDirection::Left,
            spell: Spell::FireBall,
            target_id: zone_id,
            target: Point { x: 330, y: 270 },
            cast: true,
            level: 0,
            self_broadcast: false,
            secondary_target_ids: vec![remote_id],
        },
    ];
    runtime.normalize_owner_state_packets(&mut packets);
    assert!(matches!(&packets[0], ServerPacket::ObjectMagic { object_id, target_id, secondary_target_ids, .. }
        if *object_id == local_id && *target_id == local_id && secondary_target_ids == &vec![local_id, remote_id]));
    assert!(matches!(&packets[1], ServerPacket::Magic { target_id, secondary_target_ids, .. }
        if *target_id == local_id && secondary_target_ids == &vec![remote_id, local_id]));
    assert!(matches!(&packets[2], ServerPacket::ObjectMagic { object_id, target_id, secondary_target_ids, .. }
        if *object_id == remote_id && *target_id == local_id && secondary_target_ids == &vec![remote_id]));
}
