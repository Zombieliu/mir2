//! A rounded display packet cannot create a corpse in the personal mirror.
//! Authentication and character entry use ordinary packets; the low-HP state
//! is an explicitly trusted shared snapshot, not a client command.

use mir2_protocol::{
    ClientPacket, MirClass, MirDirection, MirGender, ObjectDiedInfo, ObjectHealthInfo,
    ObjectRevivedInfo, Point, ServerPacket,
};
use mir2_simulation::{SimulationConfig, SimulationSession, WorldEntityKind, WorldEntitySnapshot};

fn entered_session(class: MirClass) -> SimulationSession {
    let mut session = SimulationSession::new(SimulationConfig::default());
    let created = session.handle_packet(ClientPacket::NewAccount {
        account_id: "mirror_health".into(),
        password: "MirrorHealth42!".into(),
        birth_date_binary: 0,
        user_name: String::new(),
        secret_question: String::new(),
        secret_answer: String::new(),
        email_address: String::new(),
    });
    assert!(created
        .iter()
        .any(|p| matches!(p, ServerPacket::NewAccount { result: 8 })));
    let login = session.handle_packet(ClientPacket::Login {
        account_id: "mirror_health".into(),
        password: "MirrorHealth42!".into(),
    });
    assert!(login
        .iter()
        .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    let created = session.handle_packet(ClientPacket::NewCharacter {
        name: "MirrorHealth".into(),
        gender: MirGender::Male,
        class,
    });
    let character_index = created
        .iter()
        .find_map(|p| match p {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .expect("ordinary character creation must succeed");
    let started = session.handle_packet(ClientPacket::StartGame { character_index });
    assert!(started
        .iter()
        .any(|p| matches!(p, ServerPacket::StartGame { result: 4, .. })));
    assert!(session.active_zone_join_snapshot("mirror-health").is_some());
    session
}

fn scheduled_monster(session: &SimulationSession) -> WorldEntitySnapshot {
    session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|e| {
            e.kind == WorldEntityKind::Monster
                && !e.dead
                && e.hp.is_some_and(|hp| hp > 0)
                && session
                    .zone_monster_spawn_snapshot(e.object_id)
                    .is_some_and(|spawn| spawn.respawn.is_some())
        })
        .expect("ordinary entry must expose a scheduled live monster")
}

fn monster(session: &SimulationSession, object_id: u32) -> WorldEntitySnapshot {
    session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|e| e.object_id == object_id)
        .expect("the monster mirror must remain present")
}

fn health(object_id: u32, percent: u8) -> ServerPacket {
    ServerPacket::ObjectHealth {
        info: ObjectHealthInfo {
            object_id,
            percent,
            expire: 5,
        },
    }
}

#[test]
fn rounded_zero_keeps_exact_live_monster_vitals_for_all_classes() {
    for class in [MirClass::Warrior, MirClass::Wizard, MirClass::Taoist] {
        let mut session = entered_session(class);
        let mut live = scheduled_monster(&session);
        live.hp = Some(1);
        live.max_hp = Some(1_000);
        assert!(session.apply_shared_entity_snapshot(&live));

        for percent in [0, 1, 52, 100] {
            session.apply_shared_monster_lifecycle_packets(&[health(live.object_id, percent)]);
            let after = monster(&session, live.object_id);
            assert!(
                !after.dead,
                "display{percent} is not a death event for {class:?}"
            );
            assert_eq!(after.hp, Some(1));
            assert_eq!(after.max_hp, Some(1_000));
        }
        let spawn = session
            .zone_monster_spawn_snapshot(live.object_id)
            .expect("display0 must not revoke the live spawn snapshot");
        assert_eq!(spawn.hp, 1);
        assert_eq!(spawn.max_hp, 1_000);
        assert!(spawn.respawn.is_some());
    }
}

#[test]
fn actual_death_survives_health_packets_until_explicit_revive() {
    let mut session = entered_session(MirClass::Taoist);
    let live = scheduled_monster(&session);
    let death = ServerPacket::ObjectDied {
        info: ObjectDiedInfo {
            object_id: live.object_id,
            location: Point {
                x: live.x,
                y: live.y,
            },
            direction: live.direction,
            kind: 0,
        },
    };
    session.apply_shared_monster_lifecycle_packets(&[
        health(live.object_id, 0),
        death,
        health(live.object_id, 100),
    ]);
    let corpse = monster(&session, live.object_id);
    assert!(corpse.dead);
    assert_eq!(corpse.hp, Some(0));
    assert!(session
        .zone_monster_spawn_snapshot(live.object_id)
        .is_none());

    session.apply_shared_monster_lifecycle_packets(&[
        ServerPacket::ObjectRevived {
            info: ObjectRevivedInfo {
                object_id: live.object_id,
                effect: true,
            },
        },
        health(live.object_id, 0),
    ]);
    let revived = monster(&session, live.object_id);
    assert!(!revived.dead);
    assert!(revived.hp.is_some_and(|hp| hp > 0));
    assert!(session
        .zone_monster_spawn_snapshot(live.object_id)
        .is_some());
}

#[test]
fn display_zero_cannot_grant_harvest_but_actual_death_can() {
    let mut session = entered_session(MirClass::Warrior);
    let mut deer = scheduled_monster(&session);
    // Materialize a trusted shared deer, separate from the scheduled personal
    // slot, to exercise the compatibility Harvest path without private ECS.
    deer.object_id = 9_510_005;
    deer.name = "Deer".into();
    deer.ai = Some(2);
    deer.hp = Some(1);
    deer.max_hp = Some(1_000);
    deer.sprite = None;
    session.force_authoritative_player_transform(
        Point {
            x: deer.x + 1,
            y: deer.y,
        },
        MirDirection::Left,
    );
    assert!(session.apply_shared_entity_snapshot(&deer));
    session.apply_shared_monster_lifecycle_packets(&[health(deer.object_id, 0)]);
    let live_attempt = session.handle_packet(ClientPacket::Harvest {
        direction: MirDirection::Left,
    });
    assert!(
        !live_attempt.iter().any(|p| matches!(
            p,
            ServerPacket::ObjectHarvest { .. } | ServerPacket::ObjectHarvested { .. }
        )),
        "a live display0 monster must not be a carcass"
    );
    assert!(!monster(&session, deer.object_id).dead);

    session.apply_shared_monster_lifecycle_packets(&[ServerPacket::ObjectDied {
        info: ObjectDiedInfo {
            object_id: deer.object_id,
            location: Point {
                x: deer.x,
                y: deer.y,
            },
            direction: deer.direction,
            kind: 0,
        },
    }]);
    let dead_attempt = session.handle_packet(ClientPacket::Harvest {
        direction: MirDirection::Left,
    });
    assert!(
        dead_attempt
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectHarvest { .. })),
        "only real death grants the ordinary Harvest action"
    );
}
