use super::*;
use crate::runtime::zone::types::{ZoneMonsterRespawnPolicy, ZonePlayerCombatStats};
use mir2_protocol::{MirClass, MirGender, ObjectMovement};

fn join_observer(zone: &mut ZoneRuntime) -> SessionId {
    let session_id = SessionId::new("harvest-owner");
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: session_id.clone(),
        account_id: "harvest-account".into(),
        character_index: 0,
        object_id: 101,
        name: "Harvester".into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 10,
        hp: 100,
        max_hp: 100,
        mp: 20,
        map_file_name: "harvest-result-anchor".into(),
        position: Point { x: 10, y: 10 },
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats::default(),
    }));
    session_id
}

fn scheduled_monster(object_id: u32, ai: u8) -> ZoneMonsterSpawn {
    ZoneMonsterSpawn {
        crystal_drop_seed: None,
        object_id,
        name: "Harvest fixture".into(),
        name_colour_argb: -1,
        image: 0,
        ai,
        disposition: Some(crate::WorldEntityDisposition::Hostile),
        level: 1,
        max_hp: 10,
        hp: 10,
        experience: 1,
        move_speed_ms: 600,
        attack_speed_ms: 1_200,
        friendly_guild: None,
        position: Point { x: 11, y: 10 },
        direction: MirDirection::Left,
        defense: Default::default(),
        respawn: Some(ZoneMonsterRespawnPolicy {
            minimum_delay_ms: 100,
            base_delay_ms: 100,
            random_delay_step_ms: 0,
            random_delay_steps: 1,
            random_delay_subtract_steps: 0,
            rule_index: 0,
            slot_index: 0,
        }),
        drops: Vec::new(),
    }
}

fn harvest_packet(object_id: u32) -> ServerPacket {
    ServerPacket::ObjectHarvested {
        movement: ObjectMovement {
            object_id,
            position: Point { x: 999, y: 999 },
            direction: MirDirection::Down,
        },
    }
}

fn outbounds_have_revive(outbounds: &[ZoneOutbound], object_id: u32) -> bool {
    outbounds.iter().any(|outbound| {
        let packets = match outbound {
            ZoneOutbound::ToSession { packets, .. }
            | ZoneOutbound::ToMany { packets, .. }
            | ZoneOutbound::ToAll { packets } => packets,
            _ => return false,
        };
        packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectRevived { info } if info.object_id == object_id
            )
        })
    })
}

#[test]
fn valid_harvest_result_anchors_corpse_and_unblocks_scheduled_respawn_once() {
    let mut zone = ZoneRuntime::new_with_collision(
        ZoneKey::for_map("harvest-result-anchor"),
        ZoneCollision::unbounded(),
    );
    let owner = join_observer(&mut zone);
    let object_id = 9_101;
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: owner.clone(),
        now_ms: 0,
        monster: scheduled_monster(object_id, 2),
    });
    assert!(zone
        .apply_native_monster_damage(object_id, 10, Some(&owner), 1_000)
        .is_some_and(|result| result.2));

    assert!(
        !outbounds_have_revive(&zone.tick(1_100), object_id),
        "an unharvested Crystal corpse must still block its scheduled respawn"
    );

    let harvest = harvest_packet(object_id);
    assert!(matches!(
        shared_object_action_packet(&zone.players[&owner], Some(101), &harvest),
        Some(ServerPacket::ObjectHarvested { movement })
            if movement.object_id == object_id
                && movement.position == (Point { x: 999, y: 999 })
    ));
    zone.broadcast_shared_object_packets(&owner, Some(101), &[harvest.clone()], 1_101);
    assert!(zone.harvested_object_ids.contains(&object_id));

    zone.broadcast_shared_object_packets(&owner, Some(101), &[harvest], 1_102);
    assert_eq!(
        zone.harvested_object_ids.len(),
        1,
        "duplicate harvest acknowledgement remains idempotent"
    );

    let revived = zone.tick(1_103);
    assert!(outbounds_have_revive(&revived, object_id));
    assert!(!zone.harvested_object_ids.contains(&object_id));

    assert!(zone
        .apply_native_monster_damage(object_id, 10, Some(&owner), 2_000)
        .is_some_and(|result| result.2));
    zone.broadcast_shared_object_packets(&owner, Some(101), &[harvest_packet(object_id)], 2_001);
    assert!(
        zone.harvested_object_ids.contains(&object_id),
        "the respawned incarnation must accept its own first harvest acknowledgement"
    );
}

#[test]
fn harvest_result_anchor_does_not_bypass_live_or_nonharvestable_guards() {
    let mut zone = ZoneRuntime::new_with_collision(
        ZoneKey::for_map("harvest-result-anchor"),
        ZoneCollision::unbounded(),
    );
    let owner = join_observer(&mut zone);

    zone.handle(ZoneCommand::SpawnMonster {
        session_id: owner.clone(),
        now_ms: 0,
        monster: scheduled_monster(9_102, 2),
    });
    zone.broadcast_shared_object_packets(&owner, Some(101), &[harvest_packet(9_102)], 1);
    assert!(
        !zone.harvested_object_ids.contains(&9_102),
        "a live harvestable monster cannot be harvested"
    );

    zone.handle(ZoneCommand::SpawnMonster {
        session_id: owner.clone(),
        now_ms: 0,
        monster: scheduled_monster(9_103, 0),
    });
    assert!(zone
        .apply_native_monster_damage(9_103, 10, Some(&owner), 2)
        .is_some_and(|result| result.2));
    zone.broadcast_shared_object_packets(&owner, Some(101), &[harvest_packet(9_103)], 3);
    assert!(
        !zone.harvested_object_ids.contains(&9_103),
        "a nonharvestable corpse cannot be harvested"
    );
}
