//! Public AI68 Football integration: human hits kick, never damage.
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneBounds, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey,
    ZoneMonsterSpawn, ZoneOutbound, ZoneRuntime,
};
const ID: u32 = 9068;
const MAP: &str = "football-fixture";
fn p(x: i32, y: i32) -> Point {
    Point { x, y }
}
fn player(session: &str, id: u32, position: Point) -> ZoneJoin {
    ZoneJoin {
        session_id: SessionId::new(session),
        account_id: session.into(),
        character_index: id as i32,
        object_id: id,
        name: session.into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 40,
        hp: 10000,
        max_hp: 10000,
        mp: 100,
        map_file_name: MAP.into(),
        position,
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }
}
fn fixture(collision: ZoneCollision) -> ZoneRuntime {
    let mut z = ZoneRuntime::new_with_collision(ZoneKey::for_map(MAP), collision);
    z.handle(ZoneCommand::Join(player("kicker", 101, p(19, 20))));
    z.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("kicker"),
        MirClass::Warrior,
        false,
        false,
        true,
        false,
        false,
        false,
    ));
    z.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("kicker"),
        now_ms: 0,
        monster: ZoneMonsterSpawn {
            object_id: ID,
            name: "Football".into(),
            name_colour_argb: -1,
            image: 149,
            ai: 68,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 0,
            max_hp: 1,
            hp: 1,
            experience: 0,
            move_speed_ms: 1800,
            attack_speed_ms: 2500,
            friendly_guild: None,
            position: p(20, 20),
            direction: MirDirection::Down,
            defense: Default::default(),
            respawn: None,
            drops: Vec::new(),
        },
    });
    z
}
fn kick(z: &mut ZoneRuntime, direction: MirDirection) -> Vec<ZoneOutbound> {
    let mut out = z.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("kicker"),
        object_id: ID,
        direction,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 0,
        now_ms: 3000,
    });
    out.extend(z.tick(3000));
    out
}
fn packets<'a>(out: &'a [ZoneOutbound], who: &str) -> Vec<&'a ServerPacket> {
    let who = SessionId::new(who);
    out.iter()
        .flat_map(|o| match o {
            ZoneOutbound::ToSession {
                session_id,
                packets,
            } if *session_id == who => packets.as_slice(),
            ZoneOutbound::ToMany {
                session_ids,
                packets,
            } if session_ids.contains(&who) => packets.as_slice(),
            ZoneOutbound::ToAll { packets } => packets.as_slice(),
            _ => &[],
        })
        .collect()
}
fn location(z: &ZoneRuntime) -> Point {
    z.native_monster_snapshots()
        .iter()
        .find(|m| m.object_id == ID)
        .unwrap()
        .position
        .clone()
}
fn walks(out: &[ZoneOutbound]) -> Vec<Point> {
    packets(out, "kicker")
        .iter()
        .filter_map(|p| match p {
            ServerPacket::ObjectWalk { movement } if movement.object_id == ID => {
                Some(movement.position.clone())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn football_zero_damage_hit_moves_four_tiles_and_preserves_one_hp() {
    let mut z = fixture(ZoneCollision::unbounded());
    let out = kick(&mut z, MirDirection::Right);
    assert_eq!(
        walks(&out),
        vec![p(21, 20), p(22, 20), p(23, 20), p(24, 20)]
    );
    assert_eq!(location(&z), p(24, 20));
    let ball = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == ID)
        .unwrap();
    assert_eq!(ball.hp, 1);
    assert!(!ball.dead);
    assert!(!packets(&out, "kicker").iter().any(|p| matches!(
        p,
        ServerPacket::DamageIndicator { object_id: ID, .. }
            | ServerPacket::ObjectDied {
                info: mir2_protocol::ObjectDiedInfo { object_id: ID, .. }
            }
    )));
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let idle = restored.tick(10000);
    assert_eq!(location(&restored), p(24, 20));
    assert!(!packets(&idle, "kicker")
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectAttack{info} if info.object_id==ID)));
    restored.handle(ZoneCommand::SyncPlayerTransform {
        session_id: SessionId::new("kicker"),
        position: p(23, 20),
        direction: MirDirection::Right,
    });
    let mut heavy = restored.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("kicker"),
        object_id: ID,
        direction: MirDirection::Right,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 99999,
        now_ms: 11000,
    });
    heavy.extend(restored.tick(11000));
    assert_eq!(location(&restored), p(28, 20));
    assert_eq!(
        restored
            .native_monster_snapshots()
            .iter()
            .find(|m| m.object_id == ID)
            .unwrap()
            .hp,
        1
    );
    assert!(!heavy
        .iter()
        .any(|o| matches!(o, ZoneOutbound::MonsterKillAward { .. })));
}

#[test]
fn football_invalid_cell_bounces_and_consumes_attempt_but_map_edge_stops() {
    let mut z = fixture(ZoneCollision::unbounded().with_blocked_cells([p(23, 20)]));
    let out = kick(&mut z, MirDirection::Right);
    assert_eq!(walks(&out), vec![p(21, 20), p(22, 20), p(21, 20)]);
    let mut z = fixture(ZoneCollision::unbounded().with_bounds(ZoneBounds::new(0, 22, 0, 100)));
    let out = kick(&mut z, MirDirection::Right);
    assert_eq!(walks(&out), vec![p(21, 20), p(22, 20)]);
}

#[test]
fn football_occupied_cell_does_not_bounce() {
    let mut z = fixture(ZoneCollision::unbounded());
    z.handle(ZoneCommand::Join(player("blocker", 102, p(22, 20))));
    let out = kick(&mut z, MirDirection::Right);
    assert_eq!(walks(&out), vec![p(21, 20)]);
    assert_eq!(location(&z), p(21, 20));
}

#[test]
fn football_four_steps_update_old_and_new_aoi_observers() {
    let mut z = fixture(ZoneCollision::unbounded());
    let range = mir2_simulation::CRYSTAL_OBJECT_DATA_RANGE;
    z.handle(ZoneCommand::Join(player("old", 102, p(20 - range, 20))));
    z.handle(ZoneCommand::Join(player("new", 103, p(24 + range, 20))));
    let out = kick(&mut z, MirDirection::Right);
    assert!(packets(&out, "old")
        .iter()
        .any(|p| matches!(p, ServerPacket::ObjectRemove { object_id: ID })));
    assert!(!packets(&out, "old")
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectWalk{movement} if movement.object_id==ID)));
    assert!(packets(&out,"new").iter().any(|packet|matches!(packet,ServerPacket::ObjectMonster{info} if info.object_id==ID&&info.location==p(24,20))));
}
