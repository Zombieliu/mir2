use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCommand, ZoneJoin, ZoneKey, ZoneMonsterSpawn,
    ZoneOutbound, ZoneRuntime,
};
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
fn saved(z: &ZoneRuntime) -> serde_json::Value {
    serde_json::from_slice(&z.checkpoint_bytes().unwrap()).unwrap()
}
fn fixture() -> (ZoneRuntime, u32) {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("vampire-fixture"));
    z.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new("owner"),
        account_id: "owner".into(),
        character_index: 1,
        object_id: 101,
        name: "owner".into(),
        class: MirClass::Archer,
        gender: MirGender::Male,
        level: 50,
        hp: 100,
        max_hp: 1000,
        mp: 100,
        map_file_name: "vampire-fixture".into(),
        position: Point { x: 100, y: 100 },
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
    z.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("owner"),
        MirClass::Archer,
        true,
        false,
        false,
        false,
        false,
        false,
    ));
    z.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("owner"),
        object_id: 0,
        spell: Spell::SummonVampire,
        direction: MirDirection::Right,
        target: Point { x: 104, y: 100 },
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1000,
        now_ms: 10,
    });
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
fn enemy(z: &mut ZoneRuntime, spider: u32, mac: i32) {
    let p = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == spider)
        .unwrap()
        .position;
    z.spawn_world_event_monster(
        &ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: 9001,
            name: "Scarecrow".into(),
            name_colour_argb: -1,
            image: 0,
            ai: 0,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 1,
            max_hp: 1000,
            hp: 1000,
            experience: 5,
            move_speed_ms: 1000000,
            attack_speed_ms: 1000000,
            friendly_guild: None,
            position: Point { x: p.x + 1, y: p.y },
            direction: MirDirection::Left,
            defense: mir2_simulation::ZoneMonsterDefense {
                agility: 0,
                min_ac: 10000,
                max_ac: 10000,
                min_mac: mac,
                max_mac: mac,
            },
            respawn: None,
            drops: Vec::new(),
        },
        1210,
    );
}
fn hp(z: &ZoneRuntime) -> i32 {
    z.native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == 9001)
        .unwrap()
        .hp
}
#[test]
fn vampire_bite_settles_mac_damage_in_attack_update_and_queues_vamp_once() {
    let (mut z, id) = fixture();
    enemy(&mut z, id, 0);
    let out = z.tick(2210);
    assert!(packets(&out)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==id)));
    assert!(
        hp(&z) < 1000,
        "DC bite ignores physical AC but resolves MACAgility immediately"
    );
    let after = hp(&z);
    let s = saved(&z);
    assert!(s["pet_special_world"]["vampire"]
        .as_array()
        .is_some_and(|p| !p.is_empty()));
    z.tick(2810);
    assert_eq!(hp(&z), after, "no obsolete 600ms secondary strike");
}
#[test]
fn vampire_fully_armoured_hit_does_not_create_vampire_pool() {
    let (mut z, id) = fixture();
    enemy(&mut z, id, 10000);
    z.tick(2210);
    assert_eq!(hp(&z), 1000);
    assert!(saved(&z)["pet_special_world"]["vampire"]
        .as_array()
        .is_none_or(|p| p.is_empty()));
}
#[test]
fn vampire_expiry_explodes_once_with_exact_pet_level_damage_and_checkpoint() {
    let (mut z, id) = fixture();
    let s = saved(&z);
    let due = s["objects"][id.to_string()]["expires_at_ms"]
        .as_u64()
        .unwrap();
    z.tick(due);
    assert!(
        !z.native_monster_snapshots()
            .iter()
            .find(|m| m.object_id == id)
            .unwrap()
            .dead
    );
    enemy(&mut z, id, 0);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let out = z.tick(due + 1);
    assert_eq!(out, restored.tick(due + 1));
    assert_eq!(
        hp(&z),
        980,
        "PetLevel2 explodes for 20 MAC damage in the death update"
    );
    assert!(packets(&out)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectDied{info}if info.object_id==id)));
    z.tick(due + 2);
    assert_eq!(hp(&z), 980);
}
