//! A destination viewport must not inherit queued source-map combat effects.
//! These fixtures use the real pending queue and authenticated map bootstrap.
use super::*;

fn old_map_combat(owner: u32) -> Vec<ServerPacket> {
    vec![
        ServerPacket::ObjectAttack {
            info: mir2_protocol::ObjectAttackInfo {
                object_id: 9_004,
                location: Point { x: 345, y: 361 },
                direction: MirDirection::Down,
                spell: Spell::None as u8,
                level: 0,
                attack_type: 0,
            },
        },
        ServerPacket::ObjectStruck {
            info: ObjectStruckInfo {
                object_id: owner,
                attacker_id: 9_004,
                location: Point { x: 345, y: 362 },
                direction: MirDirection::UpLeft,
            },
        },
        ServerPacket::DamageIndicator { object_id: owner, damage: 13, damage_type: 0 },
        ServerPacket::ObjectHealth {
            info: ObjectHealthInfo { object_id: owner, percent: 36, expire: 0 },
        },
        ServerPacket::ObjectPoisoned { object_id: owner, poison: 32 },
        ServerPacket::Poisoned { poison: 32 },
    ]
}

#[test]
fn map_bootstrap_discards_old_combat_but_preserves_personal_settlement_receipts() {
    let mut state = SharedInProcessZoneState::new();
    let key = ZonePresenceKey { account_id: "map-combat-epoch".into(), character_index: 0 };
    let personal = vec![
        ServerPacket::CompleteQuest { completed_quests: vec![2_110_019] },
        ServerPacket::UseItem { unique_id: 2, success: true, grid: MirGridType::Inventory },
        ServerPacket::GainExperience { amount: 123 },
    ];
    let old_combat = old_map_combat(1000);
    let mut queued = old_combat.clone();
    queued.extend(personal.clone());
    state.queue_zone_packets(key.clone(), queued);

    state.begin_zone_bootstrap(&key);

    let remaining = state.take_pending_zone_packets(&key);
    assert_eq!(remaining, personal,
        "the new viewport cannot receive old-map Struck/Health/Poisoned notifications");
}

#[test]
fn map_bootstrap_retains_new_destination_combat_queued_after_the_boundary() {
    let mut state = SharedInProcessZoneState::new();
    let key = ZonePresenceKey { account_id: "new-map-combat-epoch".into(), character_index: 0 };
    state.queue_zone_packets(key.clone(), old_map_combat(1000));
    state.begin_zone_bootstrap(&key);
    let fresh = vec![
        ServerPacket::ObjectStruck { info: ObjectStruckInfo {
            object_id: 1000, attacker_id: 9_005,
            location: Point { x: 328, y: 264 }, direction: MirDirection::Right,
        } },
        ServerPacket::ObjectPoisoned { object_id: 1000, poison: 32 },
    ];
    state.queue_zone_packets(key.clone(), fresh.clone());
    assert_eq!(state.take_pending_zone_packets(&key), fresh,
        "a bootstrap fence must not suppress the current destination's new combat");
}

#[test]
fn map_destination_bootstrap_explicitly_calibrates_healthy_owner_poison_to_zero() {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared);
    start_new_runtime_with_class(&mut runtime, "poison-calibration", "PoisonCalibration", MirClass::Wizard);
    runtime.execute(WorldCommand::TransferMap { key: "crystal:0102:3:7".into() }).unwrap();
    let packets = runtime.execute(WorldCommand::TransferMap { key: "crystal:0:328:264".into() }).unwrap();
    let owner = runtime.world_snapshot().player_object_id.unwrap();
    let map_index = packets.iter().position(|packet| matches!(packet,
        ServerPacket::MapInformation { info } if info.file_name == "0")).unwrap();
    assert!(packets[map_index + 1..].iter().any(|packet| matches!(packet,
        ServerPacket::ObjectPoisoned { object_id, poison: 0 } if *object_id == owner)),
        "map bootstrap must calibrate the same living owner's current poison, including explicit zero");
}
