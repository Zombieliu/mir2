//! Shared peaceful castle gate occupancy is authoritative for every player.
use mir2_protocol::{MirClass, MirDirection, MirGender, Point};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey,
    ZoneMonsterSpawn, ZoneRuntime,
};
const ID: u32 = 9081;
const MAP: &str = "gate-fixture";
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
    z.handle(ZoneCommand::Join(player("kicker", 101, p(19, 18))));
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
            crystal_drop_seed: None,
            object_id: ID,
            name: "SabukGate".into(),
            name_colour_argb: -1,
            image: 950,
            ai: 81,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 0,
            max_hp: 1000,
            hp: 1000,
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

#[test]
fn gate_walls_block_both_sessions_and_survive_checkpoint() {
    let mut z = fixture(ZoneCollision::unbounded());
    z.handle(ZoneCommand::Join(player("other", 102, p(21, 17))));
    for (session, direction) in [
        ("kicker", MirDirection::Right),
        ("other", MirDirection::Down),
    ] {
        z.handle(ZoneCommand::Walk {
            session_id: SessionId::new(session),
            direction,
            seq: 1,
            now_ms: 0,
        });
    }
    z.tick(0);
    assert_eq!(
        z.player_position(&SessionId::new("kicker")),
        Some(p(19, 18))
    );
    assert_eq!(z.player_position(&SessionId::new("other")), Some(p(21, 17)));
    let bytes = z.checkpoint_bytes().unwrap();
    let mut restored = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
    restored.handle(ZoneCommand::Walk {
        session_id: SessionId::new("kicker"),
        direction: MirDirection::Right,
        seq: 2,
        now_ms: 1000,
    });
    restored.tick(1000);
    assert_eq!(
        restored.player_position(&SessionId::new("kicker")),
        Some(p(19, 18))
    );
}

#[test]
fn gate_is_stationary_and_does_not_attack_nearby_players() {
    let mut z = fixture(ZoneCollision::unbounded());
    for now in [1, 3000, 10000, 60000] {
        z.tick(now);
    }
    let m = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == ID)
        .unwrap();
    assert_eq!(m.position, p(20, 20));
    assert_eq!(m.hp, 1000);
    assert_eq!(z.player_vitals(&SessionId::new("kicker")).unwrap().0, 10000);
}
