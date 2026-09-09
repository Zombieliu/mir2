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
        map_file_name: "pet-special-fixture".into(),
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
    let mut z = ZoneRuntime::new(ZoneKey::for_map("pet-special-fixture"));
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

fn cast_pet(z: &mut ZoneRuntime, spell: Spell) {
    z.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("owner"),
        object_id: 9001,
        spell,
        direction: MirDirection::Right,
        target: p(104, 100),
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1000,
        now_ms: 10,
    });
}
fn pet_fixture(spell: Spell) -> (ZoneRuntime, u32) {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("pet-special-fixture"));
    z.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new("owner"),
        account_id: "owner".into(),
        character_index: 1,
        object_id: 101,
        name: "owner".into(),
        class: MirClass::Archer,
        gender: MirGender::Male,
        level: 50,
        hp: 30,
        max_hp: 1000,
        mp: 100,
        map_file_name: "pet-special-fixture".into(),
        position: p(100, 100),
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
    let mut enemy = spawn(9001, 0, 1000, "Scarecrow", p(104, 100));
    enemy.max_hp = 100000;
    enemy.hp = 100000;
    z.spawn_world_event_monster(&enemy, 0);
    cast_pet(&mut z, spell);
    let out = z.tick(1210);
    let id = packets(&out)
        .into_iter()
        .find_map(|p| match p {
            ServerPacket::ObjectMonster { info } if info.master_object_id == 101 => {
                Some(info.object_id)
            }
            _ => None,
        })
        .unwrap();
    (z, id)
}
#[test]
fn toad_spawn_lock_and_exact_projectile_timer_survive_checkpoint() {
    let (mut z, id) = pet_fixture(Spell::SummonToad);
    let s = state(&z);
    assert_eq!(
        s["native_monsters"][id.to_string()]["next_ai_ready_at_ms"],
        2210
    );
    let out = z.tick(2209);
    assert!(!packets(&out)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectRangeAttack{info}if info.object_id==id)));
    let mut fired = None;
    for now in [2210, 4210, 5210] {
        let out = z.tick(now);
        if let Some((origin, target)) = packets(&out).iter().find_map(|p| match p {
            ServerPacket::ObjectRangeAttack { info } if info.object_id == id => {
                Some((info.location.clone(), info.target.clone()))
            }
            _ => None,
        }) {
            fired = Some((now, origin, target));
            break;
        }
    }
    let (now, origin, target) = fired.unwrap();
    let s = state(&z);
    let h = &s["pet_special_world"]["toad_hits"][0];
    let due = h["due"].as_u64().unwrap();
    let d = (origin.x - target.x).abs().max((origin.y - target.y).abs());
    assert_eq!(due, now + d as u64 * 50 + 500);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(due - 1), restored.tick(due - 1));
    let out = z.tick(due);
    assert_eq!(out, restored.tick(due));
    assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::DamageIndicator{object_id,damage,..}if *object_id==9001&&*damage>0)));
}
#[test]
fn vampire_healing_arrives_after_one_second_in_ten_hp_slices() {
    let (mut z, spider) = pet_fixture(Spell::SummonVampire);
    let mut hit_at = None;
    for now in [2210, 3210, 4210, 5210] {
        z.tick(now);
        if state(&z)["pet_special_world"]["vampire"]
            .as_array()
            .is_some_and(|v| !v.is_empty())
        {
            hit_at = Some(now);
            break;
        }
    }
    let hit = hit_at.unwrap();
    let s = state(&z);
    let due = s["pet_special_world"]["vampire"][0]["due"]
        .as_u64()
        .unwrap();
    assert_eq!(due, hit + 1000);
    let reserve = s["pet_special_world"]["vampire"][0]["amount"]
        .as_u64()
        .unwrap();
    // Stop fresh attacks without touching the player's accumulated reserve.
    // Natural regeneration may legitimately emit PlayerHealed on these ticks.
    z.despawn_world_event_monster(spider, hit);
    z.tick(due);
    assert_eq!(
        state(&z)["pet_special_world"]["vampire"][0]["amount"],
        reserve
    );
    let out = z.tick(due + 1);
    let remaining = state(&z)["pet_special_world"]["vampire"]
        .as_array()
        .unwrap()
        .first()
        .map_or(0, |p| p["amount"].as_u64().unwrap());
    assert_eq!(remaining, reserve.saturating_sub(10));
    assert!(out
        .iter()
        .any(|o| matches!(o,ZoneOutbound::PlayerHealed{amount,..}if *amount>0&&*amount<=10)));
}
#[test]
fn vampire_pool_is_cleared_before_same_id_owner_rejoins() {
    let (mut z, _) = pet_fixture(Spell::SummonVampire);
    for now in [2210, 3210, 4210, 5210] {
        z.tick(now);
        if state(&z)["pet_special_world"]["vampire"]
            .as_array()
            .is_some_and(|v| !v.is_empty())
        {
            break;
        }
    }
    assert!(!state(&z)["pet_special_world"]["vampire"]
        .as_array()
        .unwrap()
        .is_empty());
    z.handle(ZoneCommand::Leave {
        session_id: SessionId::new("owner"),
    });
    join(&mut z, "owner", 101, p(100, 100), 1);
    assert!(state(&z)["pet_special_world"]["vampire"]
        .as_array()
        .unwrap()
        .is_empty());
}
