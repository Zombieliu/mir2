use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    world_entity_sprite_from_object_player, SessionId, ZoneCollision, ZoneCommand, ZoneJoin,
    ZoneKey, ZoneManager, ZoneOutbound, ZonePlayerAppearance, ZoneRuntime,
};

fn join(id: &str, object_id: u32, class: MirClass, gender: MirGender) -> ZoneJoin {
    ZoneJoin {
        session_id: SessionId::new(id),
        account_id: format!("{id}-account"),
        character_index: object_id as i32,
        object_id,
        name: id.into(),
        class,
        gender,
        level: 12,
        hp: 60,
        max_hp: 60,
        mp: 30,
        map_file_name: "0".into(),
        position: Point {
            x: 10 + (object_id % 2) as i32,
            y: 10,
        },
        direction: MirDirection::Down,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }
}

fn player_info(
    outbounds: &[ZoneOutbound],
    recipient: &str,
    name: &str,
) -> mir2_protocol::ObjectPlayerInfo {
    let recipient = SessionId::new(recipient);
    outbounds
        .iter()
        .filter_map(|outbound| match outbound {
            ZoneOutbound::ToSession {
                session_id,
                packets,
            } if *session_id == recipient => Some(packets),
            ZoneOutbound::ToMany {
                session_ids,
                packets,
            } if session_ids.contains(&recipient) => Some(packets),
            ZoneOutbound::ToAll { packets } => Some(packets),
            _ => None,
        })
        .flatten()
        .find_map(|packet| match packet {
            ServerPacket::ObjectPlayer { info } if info.name == name => Some(info.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing {name} appearance for {recipient:?}: {outbounds:?}"))
}

#[test]
fn normal_three_class_aoi_join_and_reentry_use_player_body_for_both_genders() {
    for class in [MirClass::Warrior, MirClass::Wizard, MirClass::Taoist] {
        for gender in [MirGender::Male, MirGender::Female] {
            let mut zone =
                ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
            zone.handle(ZoneCommand::Join(join(
                "observer",
                100,
                MirClass::Warrior,
                MirGender::Male,
            )));
            let outbounds = zone.handle(ZoneCommand::Join(join("subject", 101, class, gender)));
            let subject = player_info(&outbounds, "observer", "subject");
            let observer = player_info(&outbounds, "subject", "observer");
            for info in [&subject, &observer] {
                assert_eq!(
                    info.transform_type, -1,
                    "ordinary join must not select a transformation"
                );
                let sprite = world_entity_sprite_from_object_player(info);
                assert_eq!(sprite.body_library, "CArmour/00");
                assert_eq!(sprite.hair_library.as_deref(), Some("CHair/00"));
                assert_eq!(
                    sprite.frame_base_offset,
                    if info.gender == MirGender::Female {
                        808
                    } else {
                        0
                    }
                );
            }
            assert_eq!(subject.class, class);
            assert_eq!(subject.gender, gender);
            zone.handle(ZoneCommand::SyncPlayerTransform {
                session_id: SessionId::new("subject"),
                position: Point { x: 100, y: 100 },
                direction: MirDirection::Right,
            });
            let outbounds = zone.handle(ZoneCommand::SyncPlayerTransform {
                session_id: SessionId::new("subject"),
                position: Point { x: 11, y: 10 },
                direction: MirDirection::Left,
            });
            let subject = player_info(&outbounds, "observer", "subject");
            assert_eq!(subject.transform_type, -1);
            assert_eq!(
                world_entity_sprite_from_object_player(&subject).body_library,
                "CArmour/00"
            );
        }
    }
}

#[test]
fn equipment_and_real_transformation_survive_aoi_reentry() {
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
    zone.handle(ZoneCommand::Join(join(
        "subject",
        101,
        MirClass::Wizard,
        MirGender::Female,
    )));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: SessionId::new("subject"),
        owner_local_object_id: 1000,
        now_ms: 1,
        packets: vec![ServerPacket::PlayerUpdate {
            object_id: 1000,
            light: 3,
            weapon: 4,
            weapon_effect: 0,
            armour: 5,
            wing_effect: 0,
        }],
    });
    let outbounds = zone.handle(ZoneCommand::Join(join(
        "observer",
        100,
        MirClass::Warrior,
        MirGender::Male,
    )));
    let subject = player_info(&outbounds, "observer", "subject");
    assert_eq!(subject.transform_type, -1);
    let sprite = world_entity_sprite_from_object_player(&subject);
    assert_eq!(sprite.body_library, "CArmour/05");
    assert_eq!(sprite.weapon_library.as_deref(), Some("CWeapon/04"));
    assert_eq!(sprite.weapon_frame_offset, Some(416));

    for (transform_type, expected_library) in [(4, "Transform/04"), (-1, "CArmour/05")] {
        zone.handle(ZoneCommand::BroadcastPackets {
            session_id: SessionId::new("subject"),
            owner_local_object_id: 1000,
            now_ms: 2,
            packets: vec![ServerPacket::TransformUpdate {
                object_id: 1000,
                transform_type,
            }],
        });
        zone.handle(ZoneCommand::SyncPlayerTransform {
            session_id: SessionId::new("subject"),
            position: Point { x: 100, y: 100 },
            direction: MirDirection::Right,
        });
        let outbounds = zone.handle(ZoneCommand::SyncPlayerTransform {
            session_id: SessionId::new("subject"),
            position: Point { x: 11, y: 10 },
            direction: MirDirection::Left,
        });
        let subject = player_info(&outbounds, "observer", "subject");
        assert_eq!(subject.transform_type, transform_type);
        assert_eq!(
            world_entity_sprite_from_object_player(&subject).body_library,
            expected_library
        );
    }
}

fn appearance() -> ZonePlayerAppearance {
    ZonePlayerAppearance {
        hair: 2,
        light: 3,
        weapon: 4,
        weapon_effect: 0,
        armour: 5,
        wing_effect: 0,
        mount_type: -1,
        riding_mount: false,
        fishing: false,
    }
}

#[test]
fn first_join_sends_equipped_appearance_once_and_late_observer_sees_it() {
    let mut manager = ZoneManager::new();
    let mut observer = join("observer", 100, MirClass::Warrior, MirGender::Male);
    observer.position = Point { x: 330, y: 270 };
    manager.join(observer);
    let mut subject = join("subject", 101, MirClass::Taoist, MirGender::Male);
    subject.position = Point { x: 331, y: 270 };
    let outbounds = manager.join_with_appearance(subject, appearance());
    let info = player_info(&outbounds, "observer", "subject");
    assert_eq!(
        (info.hair, info.weapon, info.armour, info.transform_type),
        (2, 4, 5, -1)
    );
    let introductions = outbounds.iter().filter_map(|outbound| match outbound {
        ZoneOutbound::ToSession { session_id, packets } if *session_id == SessionId::new("observer") => Some(packets),
        ZoneOutbound::ToMany { session_ids, packets } if session_ids.contains(&SessionId::new("observer")) => Some(packets),
        _ => None,
    }).flatten().filter(|packet| matches!(packet, ServerPacket::ObjectPlayer { info } if info.name == "subject")).count();
    assert_eq!(
        introductions, 1,
        "do not introduce a naked actor before its equipped replacement"
    );
    let mut late = join("late", 102, MirClass::Wizard, MirGender::Female);
    late.position = Point { x: 332, y: 270 };
    let late_packets = manager.join(late);
    let info = player_info(&late_packets, "late", "subject");
    assert_eq!((info.hair, info.weapon, info.armour), (2, 4, 5));
}

#[test]
fn appearance_refresh_is_aoi_only_idempotent_and_preserves_authoritative_state() {
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
    zone.handle(ZoneCommand::Join(join(
        "observer",
        100,
        MirClass::Warrior,
        MirGender::Male,
    )));
    zone.handle(ZoneCommand::Join(join(
        "subject",
        101,
        MirClass::Wizard,
        MirGender::Female,
    )));
    let mut far = join("far", 102, MirClass::Taoist, MirGender::Male);
    far.position = Point { x: 100, y: 100 };
    zone.handle(ZoneCommand::Join(far));
    zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
        session_id: SessionId::new("subject"),
        hp: 0,
        max_hp: 60,
        mp: 17,
        dead: true,
    });
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: SessionId::new("subject"),
        owner_local_object_id: 1000,
        now_ms: 1,
        packets: vec![
            ServerPacket::ObjectPoisoned {
                object_id: 1000,
                poison: 3,
            },
            ServerPacket::ObjectHidden {
                object_id: 1000,
                hidden: true,
            },
            ServerPacket::TransformUpdate {
                object_id: 1000,
                transform_type: 4,
            },
        ],
    });
    let outbounds = zone.handle(ZoneCommand::SyncPlayerAppearance {
        session_id: SessionId::new("subject"),
        appearance: appearance(),
    });
    assert_eq!(outbounds.len(), 1);
    assert!(
        matches!(&outbounds[0], ZoneOutbound::ToMany { session_ids, .. }
        if session_ids == &vec![SessionId::new("observer")])
    );
    let info = player_info(&outbounds, "observer", "subject");
    assert_eq!(info.location, Point { x: 11, y: 10 });
    assert_eq!(info.direction, MirDirection::Down);
    assert!(info.dead && info.hidden);
    assert_eq!((info.poison, info.transform_type), (3, 4));
    assert_eq!(
        zone.player_vitals(&SessionId::new("subject")),
        Some((0, 60, 17))
    );
    assert!(zone
        .handle(ZoneCommand::SyncPlayerAppearance {
            session_id: SessionId::new("subject"),
            appearance: appearance(),
        })
        .is_empty());
    let mut empty_handed = appearance();
    empty_handed.weapon = -1;
    let packets = zone.handle(ZoneCommand::SyncPlayerAppearance {
        session_id: SessionId::new("subject"),
        appearance: empty_handed,
    });
    assert_eq!(player_info(&packets, "observer", "subject").weapon, -1);
}

#[test]
fn checkpoints_preserve_new_hair_and_legacy_default_without_rewriting_valid_transformation() {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map("0"));
    let mut subject = join("subject", 101, MirClass::Wizard, MirGender::Female);
    subject.position = Point { x: 330, y: 270 };
    zone.handle(ZoneCommand::Join(subject));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: SessionId::new("subject"),
        owner_local_object_id: 1000,
        now_ms: 1,
        packets: vec![ServerPacket::TransformUpdate {
            object_id: 1000,
            transform_type: 0,
        }],
    });
    let legacy_bytes = zone.checkpoint_bytes().unwrap();
    let legacy: serde_json::Value = serde_json::from_slice(&legacy_bytes).unwrap();
    assert!(
        legacy["players"]["subject"].get("hair").is_none(),
        "adding zero hair must not change previously committed player serialization"
    );
    let mut restored = ZoneRuntime::restore_checkpoint(&legacy_bytes).unwrap();
    let mut observer = join("observer", 100, MirClass::Warrior, MirGender::Male);
    observer.position = Point { x: 331, y: 270 };
    let packets = restored.handle(ZoneCommand::Join(observer.clone()));
    let info = player_info(&packets, "observer", "subject");
    assert_eq!(
        (info.hair, info.transform_type),
        (0, 0),
        "a real retained transformation stays valid"
    );
    zone.handle(ZoneCommand::SyncPlayerAppearance {
        session_id: SessionId::new("subject"),
        appearance: appearance(),
    });
    let equipped_bytes = zone.checkpoint_bytes().unwrap();
    let mut restored = ZoneRuntime::restore_checkpoint(&equipped_bytes).unwrap();
    let packets = restored.handle(ZoneCommand::Join(observer));
    let info = player_info(&packets, "observer", "subject");
    assert_eq!(
        (info.hair, info.weapon, info.armour, info.transform_type),
        (2, 4, 5, 0)
    );
}
