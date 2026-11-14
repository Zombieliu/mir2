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
        map_file_name: "horned-encounter-fixture".into(),
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
        crystal_drop_seed: None,
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
    let mut z = ZoneRuntime::new(ZoneKey::for_map("horned-encounter-fixture"));
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
fn pick(ai: u8, kind: u8, hp: i32, name: &str) -> (ZoneRuntime, u32, Vec<ZoneOutbound>) {
    for id in 91690..91946 {
        let mut z = fixture(id, ai, hp, name);
        let out = z.tick(1);
        if packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==id&&info.attack_type==kind)){return(z,id,out);}
    }
    panic!("natural RNG fixture failed to select source attack {kind}");
}
fn attack(z: &mut ZoneRuntime, session: &str, id: u32, now: u64) {
    z.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new(session),
        object_id: id,
        direction: MirDirection::Up,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 999999,
        now_ms: now,
    });
}
#[test]
fn sorceror_charged_stomp_has_real_shared_damage_and_immunity_until_completion() {
    // ArcherGuard is an explicit populated DC/MC/SC fixture; shipped Sorceror is zero.
    let (mut z, id, out) = pick(169, 2, 800, "ArcherGuard");
    join(&mut z, "b", 102, p(102, 100), 100);
    let info = packets(&out)
        .into_iter()
        .find_map(|p| match p {
            ServerPacket::ObjectAttack { info } if info.object_id == id => Some(info),
            _ => None,
        })
        .unwrap();
    assert!((5..10).contains(&info.level));
    let due = state(&z)["native_monsters"][id.to_string()]["special_ai"]["horned_encounter"]
        ["hits"][0]["due"]
        .as_u64()
        .unwrap();
    assert_eq!(due, 1 + u64::from(info.level) * 500 + 500);
    attack(&mut z, "a", id, 2);
    assert_eq!(state(&z)["native_monsters"][id.to_string()]["hp"], 800);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(due - 1), restored.tick(due - 1));
    let out = z.tick(due);
    assert_eq!(out, restored.tick(due));
    for who in ["a", "b"] {
        assert!(out.iter().any(|o|matches!(o,ZoneOutbound::PlayerDamaged { session_id, damage, .. }if *session_id==SessionId::new(who)&&*damage>0)));
    }
    assert_eq!(
        state(&z)["native_monsters"][id.to_string()]["special_ai"]["horned_encounter"]["immune"],
        false
    );
}
#[test]
fn invalidated_stomp_does_not_hit_new_life_but_still_releases_immunity() {
    let (mut z, id, _) = pick(169, 2, 800, "ArcherGuard");
    let due = state(&z)["native_monsters"][id.to_string()]["special_ai"]["horned_encounter"]
        ["hits"][0]["due"]
        .as_u64()
        .unwrap();
    z.handle(ZoneCommand::Leave {
        session_id: SessionId::new("a"),
    });
    join(&mut z, "a", 101, p(100, 101), 100);
    let out = z.tick(due);
    assert!(!out
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
    assert_eq!(
        state(&z)["native_monsters"][id.to_string()]["special_ai"]["horned_encounter"]["immune"],
        false
    );
}
#[test]
fn source_zero_sc_stomp_preserves_no_pending_damage_instead_of_inventing_dc() {
    let (mut z, id, _) = pick(169, 2, 800, "HornedSorceror");
    let saved = state(&z);
    assert!(
        saved["native_monsters"][id.to_string()]["special_ai"]["horned_encounter"]["hits"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        saved["native_monsters"][id.to_string()]["special_ai"]["horned_encounter"]["immune"],
        true
    );
    let out = z.tick(1000);
    assert!(!out
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
}
#[test]
fn tornado_is_twenty_five_real_cells_one_visual_and_delayed_environment_struck() {
    let (mut z, id, _) = pick(169, 3, 800, "ArcherGuard");
    let saved = state(&z);
    let fields = saved["horned_encounter_world"]["spells"]
        .as_array()
        .unwrap();
    assert_eq!(fields.len(), 25);
    assert_eq!(fields.iter().filter(|s| s["show"] == true).count(), 1);
    assert!(fields
        .iter()
        .all(|s| s["start"] == 1001 && s["expires"] == 16001 && s["tick_speed"] == 1000));
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let out = z.tick(1001);
    assert_eq!(out, restored.tick(1001));
    assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectSpell{info}if info.spell==Spell::HornedSorcererDustTornado)));
    assert!(packets(&out).iter().any(
        |p| matches!(p,ServerPacket::ObjectStruck{info}if info.object_id==101&&info.attacker_id==0)
    ));
    let visible_id = fields.iter().find(|s| s["show"] == true).unwrap()["object_id"]
        .as_u64()
        .unwrap();
    let out = z.tick(16002);
    assert!(packets(&out).iter().any(
        |p| matches!(p,ServerPacket::ObjectRemove{object_id}if u64::from(*object_id)==visible_id)
    ));
    let _ = id;
}
#[test]
fn commander_rock_fall_keeps_all_source_cells_and_nine_visible_centers() {
    let (z, id, out) = pick(171, 3, 990, "ArcherGuard");
    let saved = state(&z);
    let fields = saved["horned_encounter_world"]["spells"]
        .as_array()
        .unwrap();
    // Nine 21x21 areas, with the caster's own cell excluded from four overlaps.
    assert_eq!(fields.len(), 9 * 21 * 21 - 4);
    assert_eq!(fields.iter().filter(|s| s["show"] == true).count(), 9);
    assert!(fields
        .iter()
        .all(|s| s["tick_speed"] == 2000 && s["value"] == 255));
    assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==id&&(5..10).contains(&info.level))));
    let bytes = z.checkpoint_bytes().unwrap();
    let restored = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
    assert_eq!(
        state(&restored)["horned_encounter_world"],
        saved["horned_encounter_world"]
    );
    let mut corrupt = saved.clone();
    corrupt["horned_encounter_world"]["spells"][0]["value"] = serde_json::json!(999);
    assert!(ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&corrupt).unwrap()).is_err());
}
#[test]
fn commander_boulder_ring_uses_source_offsets_without_parent_stat_inheritance() {
    let mut z = fixture(9171, 171, 790, "ArcherGuard");
    z.tick(1);
    let saved = state(&z);
    let children: Vec<_> = saved["native_monsters"]
        .as_object()
        .unwrap()
        .values()
        .filter(|m| m["special_ai"]["horned_encounter"]["parent"] == 9171)
        .collect();
    assert_eq!(children.len(), 8);
    for child in children {
        assert_eq!(child["name"], "BoulderSpirit");
        let x = child["position"]["x"].as_i64().unwrap();
        let y = child["position"]["y"].as_i64().unwrap();
        let dx = (x - 100).abs();
        let dy = (y - 100).abs();
        assert!((dx == 9 && dy == 0) || (dx == 0 && dy == 9) || (dx == 7 && dy == 7));
    }
}
#[test]
fn commander_shield_expires_and_death_kills_its_real_sorceror_slave() {
    let mut z = fixture(9171, 171, 90, "ArcherGuard");
    let out = z.tick(1);
    assert!(packets(&out).iter().any(
        |p| matches!(p,ServerPacket::AddBuff{buff}if buff.object_id==9171&&buff.buff_type==56)
    ));
    let saved = state(&z);
    let slave = saved["native_monsters"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(_, m)| m["name"] == "HornedSorceror")
        .map(|(id, _)| id.parse::<u32>().unwrap())
        .unwrap();
    let slave_position = &saved["native_monsters"][slave.to_string()]["position"];
    assert_ne!(
        slave_position,
        &serde_json::json!({"x":100,"y":101}),
        "the player occupying Front must not overlap the shield slave"
    );
    assert_ne!(
        slave_position,
        &serde_json::json!({"x":100,"y":100}),
        "shared occupancy also forbids Crystal's stacked parent fallback"
    );
    assert!((slave_position["x"].as_i64().unwrap() - 100).abs() <= 3);
    assert!((slave_position["y"].as_i64().unwrap() - 100).abs() <= 3);
    z.tick(20001);
    assert_eq!(
        state(&z)["native_monsters"]["9171"]["special_ai"]["horned_encounter"]["immune"],
        false
    );
    let boss = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == 9171)
        .unwrap();
    join(
        &mut z,
        "killer",
        105,
        p(boss.position.x + 1, boss.position.y),
        1000000,
    );
    attack(&mut z, "killer", 9171, 20002);
    z.tick(20002);
    let saved = state(&z);
    assert_eq!(saved["native_monsters"]["9171"]["dead"], true);
    assert!(
        saved["native_monsters"][slave.to_string()].is_null()
            || saved["native_monsters"][slave.to_string()]["dead"] == true
    );
}

fn owned_encounter_fixture(id: u32, ai: u8, hp: i32) -> (ZoneRuntime, u32, Point) {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("horned-owned-encounter"));
    z.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new("owner"),
        account_id: "owner".into(),
        character_index: 1,
        object_id: 101,
        name: "owner".into(),
        class: MirClass::Archer,
        gender: MirGender::Male,
        level: 60,
        hp: 100000,
        max_hp: 100000,
        mp: 100,
        map_file_name: "horned-owned-encounter".into(),
        position: p(20, 24),
        direction: MirDirection::Up,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats::default(),
    }));
    z.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("owner"),
        object_id: 0,
        spell: Spell::SummonToad,
        direction: MirDirection::Up,
        target: p(20, 20),
        cast: true,
        level: 3,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1000,
        now_ms: 10,
    });
    let born = z.tick(1500);
    let (pet, pos) = packets(&born)
        .into_iter()
        .find_map(|p| match p {
            ServerPacket::ObjectMonster { info } if info.name == "SpittingToad" => {
                Some((info.object_id, info.location.clone()))
            }
            _ => None,
        })
        .expect("real owned toad");
    z.handle(ZoneCommand::SyncPlayerTransform {
        session_id: SessionId::new("owner"),
        position: p(pos.x + 10, pos.y),
        direction: MirDirection::Up,
    });
    let mut source = spawn(id, ai, hp, "Guard", p(pos.x, pos.y - 1));
    source.attack_speed_ms = 10000;
    assert!(z.spawn_world_event_monster(&source, 1500).0);
    (z, pet, pos)
}
fn native_hp(z: &ZoneRuntime, id: u32) -> i64 {
    state(z)["native_monsters"][id.to_string()]["hp"]
        .as_i64()
        .unwrap()
}
fn pick_owned_sorceror(kind: u8) -> (ZoneRuntime, u32, u32, Point) {
    for id in 95000..95100 {
        let (mut z, pet, pos) = owned_encounter_fixture(id, 169, 800);
        let out = z.tick(1501);
        if packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==id&&info.attack_type==kind)){return(z,id,pet,pos);}
    }
    panic!("legal source IDs must exercise source branch {kind}");
}
#[test]
fn sorceror_owned_target_stomp_restores_real_hit_and_releases_immunity() {
    let (mut z, id, pet, _) = pick_owned_sorceror(2);
    let before = native_hp(&z, pet);
    let s = state(&z);
    let h = &s["native_monsters"][id.to_string()]["special_ai"]["horned_encounter"]["hits"][0];
    let due = h["due"].as_u64().unwrap();
    assert!(h["target"].is_null());
    assert_eq!(h["target_ref"]["Monster"]["object_id"], pet);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(due - 1), restored.tick(due - 1));
    assert_eq!(native_hp(&z, pet), before);
    assert_eq!(z.tick(due), restored.tick(due));
    assert!(native_hp(&z, pet) < before);
    assert_eq!(
        state(&z)["native_monsters"][id.to_string()]["special_ai"]["horned_encounter"]["immune"],
        false
    );
}
#[test]
fn commander_basic_attack_hits_owned_target_at_five_hundred_ms() {
    let (mut z, pet, _) = owned_encounter_fixture(95171, 171, 1000);
    let before = native_hp(&z, pet);
    z.tick(1501);
    assert_eq!(
        state(&z)["native_monsters"]["95171"]["special_ai"]["horned_encounter"]["hits"][0]["due"],
        2001
    );
    z.tick(2000);
    assert_eq!(native_hp(&z, pet), before);
    let out = z.tick(2001);
    assert!(native_hp(&z, pet) < before);
    assert!(!out
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
}
#[test]
fn sorceror_native_tornado_is_environment_struck_and_source_retirement_stops_it() {
    let (mut z, id, pet, _) = pick_owned_sorceror(3);
    let before = native_hp(&z, pet);
    assert_eq!(
        state(&z)["horned_encounter_world"]["spells"]
            .as_array()
            .unwrap()
            .len(),
        25
    );
    let out = z.tick(2501);
    assert!(native_hp(&z, pet) < before);
    assert!(packets(&out).iter().any(
        |p| matches!(p,ServerPacket::ObjectStruck{info}if info.object_id==pet&&info.attacker_id==0)
    ));
    z.despawn_world_event_monster(id, 2502);
    let hp = native_hp(&z, pet);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(3501), restored.tick(3501));
    assert_eq!(native_hp(&z, pet), hp);
}
#[test]
fn sorceror_captured_owned_target_retirement_cancels_damage_but_not_release() {
    let (mut z, id, pet, pos) = pick_owned_sorceror(2);
    let due = state(&z)["native_monsters"][id.to_string()]["special_ai"]["horned_encounter"]
        ["hits"][0]["due"]
        .as_u64()
        .unwrap();
    z.despawn_world_event_monster(pet, 1502);
    let mut replacement = spawn(pet, 2, 1000, "Deer", pos);
    replacement.disposition = Some(WorldEntityDisposition::Neutral);
    assert!(z.spawn_world_event_monster(&replacement, 1502).0);
    z.tick(due);
    assert_eq!(native_hp(&z, pet), 1000);
    assert_eq!(
        state(&z)["native_monsters"][id.to_string()]["special_ai"]["horned_encounter"]["immune"],
        false
    );
}
