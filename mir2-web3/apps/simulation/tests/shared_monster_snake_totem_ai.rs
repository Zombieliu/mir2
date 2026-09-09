use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCommand, ZoneJoin, ZoneKey, ZoneMonsterSpawn,
    ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};
use serde_json::Value;
fn p(x: i32, y: i32) -> Point {
    Point { x, y }
}
fn join(z: &mut ZoneRuntime, name: &str, id: u32, pos: Point, dc: i32) {
    z.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new(name),
        account_id: name.into(),
        character_index: 1,
        object_id: id,
        name: name.into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 50,
        hp: 1000000,
        max_hp: 1000000,
        mp: 100,
        map_file_name: "snake-totem-fixture".into(),
        position: pos,
        direction: MirDirection::Up,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats {
            min_dc: dc,
            max_dc: dc,
            accuracy: 100,
            ..Default::default()
        },
    }));
    z.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new(name),
        MirClass::Warrior,
        false,
        false,
        true,
        false,
        false,
        false,
    ));
}
fn spawn(id: u32, ai: u8, hp: i32, name: &str, pos: Point) -> ZoneMonsterSpawn {
    ZoneMonsterSpawn {
        object_id: id,
        name: name.into(),
        name_colour_argb: -1,
        image: 950,
        ai,
        disposition: Some(WorldEntityDisposition::Hostile),
        level: 30,
        max_hp: 1000,
        hp,
        experience: 100,
        move_speed_ms: 500,
        attack_speed_ms: 1000,
        friendly_guild: None,
        defense: Default::default(),
        position: pos,
        direction: MirDirection::Down,
        respawn: None,
        drops: Vec::new(),
    }
}
fn fixture(id: u32, ai: u8, hp: i32, name: &str) -> ZoneRuntime {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("snake-totem-fixture"));
    join(&mut z, "a", 101, p(100, 101), 100);
    z.spawn_world_event_monster(&spawn(id, ai, hp, name, p(100, 100)), 0);
    z
}
fn state(z: &ZoneRuntime) -> Value {
    serde_json::from_slice(&z.checkpoint_bytes().unwrap()).unwrap()
}
fn packets(out: &[ZoneOutbound]) -> Vec<&ServerPacket> {
    out.iter()
        .flat_map(|o| match o {
            ZoneOutbound::ToSession { packets, .. }
            | ZoneOutbound::ToMany { packets, .. }
            | ZoneOutbound::ToAll { packets } => packets.iter().collect(),
            _ => Vec::new(),
        })
        .collect()
}

fn empty_room() -> ZoneRuntime {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("snake-totem-fixture"));
    z.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new("owner"),
        account_id: "owner".into(),
        character_index: 1,
        object_id: 101,
        name: "owner".into(),
        class: MirClass::Archer,
        gender: MirGender::Male,
        level: 50,
        hp: 1000,
        max_hp: 1000,
        mp: 100,
        map_file_name: "snake-totem-fixture".into(),
        position: p(100, 100),
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
    z.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("owner"),
        object_id: 0,
        spell: Spell::SummonSnakes,
        direction: MirDirection::Right,
        target: p(104, 100),
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1000,
        now_ms: 10,
    });
    z
}
#[test]
fn totem_produces_without_enemies_and_does_not_fake_an_attack() {
    let mut z = empty_room();
    z.tick(1210);
    let totem = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.name == "SnakeTotem")
        .unwrap();
    for now in [2210, 4210, 7210, 10210] {
        let out = z.tick(now);
        assert!(!packets(&out).iter().any(
            |p| matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==totem.object_id)
        ));
    }
    let s = state(&z);
    let children: Vec<_> = s["native_monsters"]
        .as_object()
        .unwrap()
        .values()
        .filter(|m| m["master_object_id"] == totem.object_id && m["dead"] == false)
        .collect();
    assert_eq!(children.len(), 3);
    assert!(children.iter().all(|m| m["owner_player_object_id"] == 101));
    assert_eq!(
        s["native_monsters"][totem.object_id.to_string()]["direction"],
        serde_json::to_value(MirDirection::Up).unwrap()
    );
}
#[test]
fn totem_search_clock_and_minion_ownership_restore_without_duplicate_wave() {
    let mut z = empty_room();
    z.tick(1210);
    z.tick(4210);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    for now in [4211, 7210, 10210] {
        assert_eq!(z.tick(now), restored.tick(now));
    }
    assert_eq!(
        z.checkpoint_bytes().unwrap(),
        restored.checkpoint_bytes().unwrap()
    );
}

#[test]
fn full_totem_keeps_redirecting_real_hostiles_away_from_nearer_player() {
    let mut z = empty_room();
    for now in [1210, 4210, 7210, 10210] {
        z.tick(now);
    }
    let before = state(&z);
    let totem = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.name == "SnakeTotem")
        .unwrap()
        .object_id;
    let due = before["native_monsters"][totem.to_string()]["special_ai"]["snake_totem"]
        ["search_at"]
        .as_u64()
        .unwrap();
    assert_eq!(
        before["native_monsters"]
            .as_object()
            .unwrap()
            .values()
            .filter(|m| m["master_object_id"] == totem && m["dead"] == false)
            .count(),
        3
    );
    let enemy = 9900;
    let mut enemy_spawn = spawn(enemy, 0, 1000, "Scarecrow", p(101, 102));
    enemy_spawn.move_speed_ms = 300;
    assert!(z.spawn_world_event_monster(&enemy_spawn, due - 1).0);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let out = z.tick(due);
    assert_eq!(out, restored.tick(due));
    let after = state(&z);
    assert_eq!(
        after["entity_combat"]["targets"][enemy.to_string()]["target"]["Monster"]["object_id"],
        totem
    );
    assert_eq!(
        after["entity_combat"]["targets"][enemy.to_string()]["target"]["Monster"]["incarnation"],
        after["native_monsters"][totem.to_string()]["incarnation"]
    );
    // Actors update in stable ID order. This enemy ran before the higher-ID
    // totem assigned Target; its next eligible decision must consume that target.
    let ready = after["native_monsters"][enemy.to_string()]["next_ai_ready_at_ms"]
        .as_u64()
        .unwrap();
    let next = z.tick(ready);
    assert_eq!(next, restored.tick(ready));
    assert!(
        packets(&next).iter().any(
            |packet| matches!(packet, ServerPacket::ObjectWalk { movement }
        if movement.object_id == enemy && movement.direction == MirDirection::UpRight
            && movement.position == p(101, 100))
        ),
        "next={next:?}"
    );
}
