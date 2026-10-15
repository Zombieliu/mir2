use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, WorldEntityKind, WorldEntitySnapshot, ZoneCollision,
    ZoneCommand, ZoneJoin, ZoneKey, ZoneOutbound, ZoneRuntime,
};

fn join(session: &str, object_id: u32, x: i32) -> ZoneJoin {
    ZoneJoin {
        session_id: SessionId::new(session),
        account_id: format!("{session}-account"),
        character_index: 0,
        object_id,
        name: session.to_string(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 7,
        hp: 60,
        max_hp: 60,
        mp: 100,
        map_file_name: "0".to_string(),
        position: Point { x, y: 270 },
        direction: MirDirection::Down,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }
}

fn snapshot() -> WorldEntitySnapshot {
    WorldEntitySnapshot {
        object_id: 101,
        kind: WorldEntityKind::SelfPlayer,
        name: "Scout".to_string(),
        owner_name: None,
        ai: None,
        x: 330,
        y: 270,
        direction: MirDirection::Down,
        class: Some(MirClass::Warrior),
        gender: Some(MirGender::Male),
        level: Some(7),
        hp: Some(60),
        max_hp: Some(60),
        light: 0,
        wing_effect: None,
        name_colour_argb: -1,
        dead: false,
        riding_mount: None,
        can_mount_attack: None,
        has_class_weapon: None,
        dazed: None,
        fishing: None,
        transform_type: None,
        disposition: WorldEntityDisposition::Friendly,
        sprite: None,
        quest_ids: Vec::new(),
        quest_icon: None,
    }
}

fn has_packet(
    outbounds: &[ZoneOutbound],
    session: &SessionId,
    expected: impl Fn(&ServerPacket) -> bool,
) -> bool {
    outbounds.iter().any(|outbound| match outbound {
        ZoneOutbound::ToSession { session_id, packets } if session_id == session => {
            packets.iter().any(&expected)
        }
        ZoneOutbound::ToMany { session_ids, packets } if session_ids.contains(session) => {
            packets.iter().any(&expected)
        }
        ZoneOutbound::ToAll { packets } => packets.iter().any(&expected),
        _ => false,
    })
}

#[test]
fn fishing_transform_snapshot_preserves_missing_and_signed_raw_values() {
    let mut entity = snapshot();
    let legacy = serde_json::to_value(&entity).unwrap();
    assert!(legacy.get("transformType").is_none());
    let decoded: WorldEntitySnapshot = serde_json::from_value(legacy).unwrap();
    assert_eq!(decoded.transform_type, None);
    assert_eq!(decoded, entity);
    for raw in [i16::MIN, -1, 0, 4, i16::MAX] {
        entity.transform_type = Some(raw);
        let value = serde_json::to_value(&entity).unwrap();
        assert_eq!(value["transformType"], serde_json::json!(raw));
        let decoded: WorldEntitySnapshot = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, entity);
    }
}

#[test]
fn fishing_transform_zone_getter_distinguishes_absence_join_zero_and_leave() {
    let mut zone = ZoneRuntime::new_with_collision(
        ZoneKey::for_map("0"),
        ZoneCollision::unbounded(),
    );
    let session = SessionId::new("owner");
    assert_eq!(zone.player_transform_type(&session), None);
    zone.handle(ZoneCommand::Join(join("owner", 101, 330)));
    // This observes the existing retained join value; no normal form is inferred.
    assert_eq!(zone.player_transform_type(&session), Some(0));
    zone.handle(ZoneCommand::Leave {
        session_id: session.clone(),
    });
    assert_eq!(zone.player_transform_type(&session), None);
}

#[test]
fn fishing_transform_zone_reads_updates_and_remote_spawn_from_the_same_raw_state() {
    let mut zone = ZoneRuntime::new_with_collision(
        ZoneKey::for_map("0"),
        ZoneCollision::unbounded(),
    );
    let owner = SessionId::new("owner");
    let observer = SessionId::new("observer");
    zone.handle(ZoneCommand::Join(join("owner", 101, 330)));
    zone.handle(ZoneCommand::Join(join("observer", 102, 332)));
    // The personal actor id and authoritative Zone object id may differ.
    for (index, raw) in [i16::MIN, -1, 0, 4, i16::MAX].into_iter().enumerate() {
        let updates = zone.handle(ZoneCommand::BroadcastPackets {
            session_id: owner.clone(),
            owner_local_object_id: 1001,
            packets: vec![ServerPacket::TransformUpdate {
                object_id: 1001,
                transform_type: raw,
            }],
            now_ms: index as u64,
        });
        assert_eq!(zone.player_transform_type(&owner), Some(raw));
        assert!(has_packet(&updates, &observer, |packet| matches!(packet,
            ServerPacket::TransformUpdate { object_id, transform_type }
                if *object_id == 101 && *transform_type == raw)));
    }
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: owner.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::TransformUpdate {
            object_id: 102,
            transform_type: -1,
        }],
        now_ms: 10,
    });
    assert_eq!(zone.player_transform_type(&owner), Some(i16::MAX));
    assert_eq!(zone.player_transform_type(&observer), Some(0));
    let late = SessionId::new("late");
    let spawns = zone.handle(ZoneCommand::Join(join("late", 103, 334)));
    assert!(has_packet(&spawns, &late, |packet| matches!(packet,
        ServerPacket::ObjectPlayer { info }
            if info.object_id == 101 && info.transform_type == i16::MAX)));
}
