//! ObjectHealth is display information; only real lifecycle events mark death.
use super::*;
use mir2_protocol::MonsterInfo;

const ID: u32 = 8_010_001;

fn spawn() -> ServerPacket {
    ServerPacket::ObjectMonster {
        info: MonsterInfo {
            object_id: ID,
            name: "Scarecrow".into(),
            name_colour_argb: -1,
            location: Point { x: 10, y: 10 },
            image: 0,
            direction: MirDirection::Down,
            effect: 0,
            ai: 0,
            light: 0,
            dead: false,
            skeleton: false,
            poison: 0,
            hidden: false,
            shock_time: 0,
            binding_shot_center: false,
            extra: false,
            extra_byte: 0,
            master_object_id: 0,
            rarity: 0,
            buffs: Vec::new(),
        },
    }
}

fn health(percent: u8) -> ServerPacket {
    ServerPacket::ObjectHealth {
        info: ObjectHealthInfo {
            object_id: ID,
            percent,
            expire: 5,
        },
    }
}

fn walk() -> ServerPacket {
    ServerPacket::ObjectWalk {
        movement: ObjectMovement {
            object_id: ID,
            position: Point { x: 11, y: 10 },
            direction: MirDirection::Right,
        },
    }
}

fn died() -> ServerPacket {
    ServerPacket::ObjectDied {
        info: ObjectDiedInfo {
            object_id: ID,
            location: Point { x: 12, y: 10 },
            direction: MirDirection::Left,
            kind: 0,
        },
    }
}

fn zone() -> ZoneRuntime {
    ZoneRuntime::new_with_collision(ZoneKey::for_map("health-zero"), ZoneCollision::unbounded())
}

fn assert_live(zone: &ZoneRuntime, position: Point) {
    let object = &zone.objects[&ID];
    assert!(matches!(&object.packet, ServerPacket::ObjectMonster { info } if !info.dead));
    assert_eq!(object.position, position);
    assert!(!zone.dead_object_ids.contains_key(&ID));
    assert!(!zone.harvested_object_ids.contains(&ID));
    assert!(!zone.removed_object_ids.contains(&ID));
}

#[test]
fn health_zero_live_object_keeps_walk_and_visibility_cache() {
    let mut zone = zone();
    zone.apply_zone_object_packets(&[spawn(), health(0), walk()], 100);
    assert_live(&zone, Point { x: 11, y: 10 });
    let packets = retained_zone_object_visibility_packets(&zone.objects[&ID]);
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::ObjectMonster { info } if !info.dead)));
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::ObjectHealth { info } if info.percent == 0)));
    assert!(!packets
        .iter()
        .any(|p| matches!(p, ServerPacket::ObjectDied { .. })));
}

#[test]
fn health_zero_before_spawn_creates_no_death_marker() {
    let mut zone = zone();
    zone.apply_zone_object_packets(&[health(0)], 100);
    assert!(!zone.dead_object_ids.contains_key(&ID));
    zone.apply_zone_object_packets(&[spawn(), walk()], 200);
    assert_live(&zone, Point { x: 11, y: 10 });
}

#[test]
fn health_zero_then_real_death_rejects_stale_positive_health_and_walk() {
    let mut zone = zone();
    zone.apply_zone_object_packets(&[spawn(), health(0), died(), health(100), walk()], 100);
    let object = &zone.objects[&ID];
    assert!(matches!(&object.packet, ServerPacket::ObjectMonster { info } if info.dead));
    assert_eq!(object.position, Point { x: 12, y: 10 });
    assert_eq!(object.health.as_ref().unwrap().percent, 0);
    assert!(zone.dead_object_ids.contains_key(&ID));
}

#[test]
fn health_zero_does_not_supersede_death_before_spawn() {
    let mut zone = zone();
    zone.apply_zone_object_packets(&[died(), health(0), spawn(), health(100), walk()], 100);
    let object = &zone.objects[&ID];
    assert!(matches!(&object.packet, ServerPacket::ObjectMonster { info } if info.dead));
    assert_eq!(object.position, Point { x: 12, y: 10 });
    assert_eq!(object.health.as_ref().unwrap().percent, 0);
}

#[test]
fn health_zero_live_fork_preserves_state_without_death_authority() {
    let mut zone = zone();
    zone.apply_zone_object_packets(&[spawn(), health(0)], 100);
    let before = zone.checkpoint_bytes().unwrap();
    let mut fork = zone.transaction_fork();
    assert_eq!(before, fork.checkpoint_bytes().unwrap());
    fork.apply_zone_object_packets(&[walk()], 200);
    assert_live(&fork, Point { x: 11, y: 10 });
    let mut restored = ZoneRuntime::restore_checkpoint(&before).unwrap();
    assert_live(&restored, Point { x: 10, y: 10 });
    restored.apply_zone_object_packets(&[walk()], 200);
    assert_live(&restored, Point { x: 11, y: 10 });
}

#[test]
fn health_zero_real_dead_marker_without_location_stays_authoritative() {
    let mut zone = zone();
    zone.dead_object_ids.insert(
        ID,
        ZoneObjectDeadState {
            position: None,
            direction: None,
        },
    );
    zone.apply_zone_object_packets(&[spawn(), health(100), walk()], 100);
    let object = &zone.objects[&ID];
    assert!(matches!(&object.packet, ServerPacket::ObjectMonster { info } if info.dead));
    assert_eq!(object.health.as_ref().unwrap().percent, 0);
    assert_eq!(object.position, Point { x: 10, y: 10 });
}
