use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCommand, ZoneJoin, ZoneKey, ZoneMonsterSpawn,
    ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};
use serde_json::Value;
fn p(x: i32, y: i32) -> Point {
    Point { x, y }
}
fn join(z: &mut ZoneRuntime, name: &str, id: u32, pos: Point) {
    z.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new(name),
        account_id: name.into(),
        character_index: 1,
        object_id: id,
        name: name.into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 50,
        hp: 100000,
        max_hp: 100000,
        mp: 100,
        map_file_name: "horned-fixture".into(),
        position: pos,
        direction: MirDirection::Up,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats {
            min_dc: 100,
            max_dc: 100,
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
fn spawn(id: u32, ai: u8, name: &str, pos: Point, hp: i32) -> ZoneMonsterSpawn {
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
fn fixture(id: u32, ai: u8, pos: Point, hp: i32) -> ZoneRuntime {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("horned-fixture"));
    join(&mut z, "a", 101, pos);
    z.spawn_world_event_monster(
        &spawn(
            id,
            ai,
            // Explicit populated stat fixture: shipped Horned templates are zero.
            "ArcherGuard",
            p(10, 10),
            hp,
        ),
        0,
    );
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
#[test]
fn mage_retreats_then_uses_delayed_close_range_magic_on_shared_players() {
    let mut z = fixture(9163, 163, p(10, 11), 1000);
    join(&mut z, "b", 102, p(12, 10));
    let moved = z.tick(1);
    assert!(packets(&moved)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectWalk{movement}if movement.object_id==9163)));
    let cast = z.tick(502);
    assert!(packets(&cast)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==9163)));
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(1001), restored.tick(1001));
    let out = z.tick(1002);
    assert_eq!(out, restored.tick(1002));
    for name in ["a", "b"] {
        assert!(out.iter().any(|o|matches!(o,ZoneOutbound::PlayerDamaged{session_id,..}if *session_id==SessionId::new(name))));
    }
}
#[test]
fn mage_ranged_projectile_has_eight_hundred_ms_delay_and_does_not_cross_lives() {
    let mut found = None;
    for id in 9163..9195 {
        let mut z = fixture(id, 163, p(10, 16), 1000);
        z.tick(1);
        let out = z.tick(502);
        if packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectRangeAttack{info}if info.object_id==id&&info.attack_type==0)){found=Some((z,id));break;}
    }
    let (mut z, id) = found.expect("natural persisted RNG fixture must cast its ranged attack");
    let value = state(&z);
    assert_eq!(
        value["native_monsters"][id.to_string()]["special_ai"]["horned"]["pending"][0]["due"],
        1302
    );
    assert!(!z
        .tick(1301)
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
    z.handle(ZoneCommand::Leave {
        session_id: SessionId::new("a"),
    });
    join(&mut z, "a", 101, p(10, 16));
    assert!(!z
        .tick(1302)
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
}
#[test]
fn mage_target_teleport_moves_authoritative_player_with_source_effect() {
    let mut found = false;
    for id in 9163..9195 {
        let mut z = fixture(id, 163, p(10, 16), 1000);
        z.tick(1);
        let out = z.tick(502);
        if let Some(pos) = out.iter().find_map(|o| match o {
            ZoneOutbound::SaveTransform {
                session_id,
                position,
                ..
            } if *session_id == SessionId::new("a") => Some(position),
            _ => None,
        }) {
            assert_eq!(z.player_position(&SessionId::new("a")).as_ref(), Some(pos));
            let monster = z
                .native_monster_snapshots()
                .into_iter()
                .find(|m| m.object_id == id)
                .unwrap();
            assert!(
                (pos.x - monster.position.x).abs() <= 4 && (pos.y - monster.position.y).abs() <= 4
            );
            assert!(packets(&out)
                .iter()
                .any(|p| matches!(p, ServerPacket::ObjectTeleportIn { object_id: 101, .. })));
            found = true;
            break;
        }
    }
    assert!(found);
}
#[test]
fn warrior_shield_adds_real_five_hundred_ac_then_expires_and_retreats() {
    let mut z = fixture(9165, 165, p(10, 11), 400);
    let mut unshielded = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let strike = ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("a"),
        object_id: 9165,
        direction: MirDirection::Up,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 9999,
        now_ms: 2,
    };
    unshielded.handle(strike.clone());
    unshielded.tick(2);
    assert_eq!(state(&unshielded)["native_monsters"]["9165"]["hp"], 300);
    let shield = z.tick(1);
    assert!(packets(&shield).iter().any(
        |p| matches!(p,ServerPacket::AddBuff{buff}if buff.object_id==9165&&buff.buff_type==55)
    ));
    z.handle(strike);
    z.tick(2);
    assert_eq!(state(&z)["native_monsters"]["9165"]["hp"], 400);
    // The explicit ArcherGuard stat fixture also contributes its 2,000 ms
    // AttackSpeed. Crystal CanAttack uses Time > AttackTime, so 2,001 is
    // still blocked after the shield cast at 1; retreat starts at 2,002.
    assert_eq!(
        state(&z)["native_monsters"]["9165"]["attack_speed_ms"],
        2000
    );
    for now in [1002, 2001] {
        assert!(!packets(&z.tick(now)).iter().any(
            |p| matches!(p, ServerPacket::ObjectWalk { movement } if movement.object_id == 9165)
        ));
    }
    let out = z.tick(2002);
    assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectWalk{movement}if movement.object_id==9165&&movement.position.y<10)));
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let out = z.tick(10001);
    assert_eq!(out, restored.tick(10001));
    assert!(packets(&out).iter().any(|p| matches!(
        p,
        ServerPacket::RemoveBuff {
            object_id: 9165,
            buff_type: 55
        }
    )));
}
#[test]
fn warrior_wide_line_hits_multiple_cells_with_distance_based_delay() {
    // HornedWarrior.cs InAttackRange accepts equal dx/dy parity. (0, 2)
    // is a valid ranged target; the former (0, 3) correctly caused a walk.
    let mut z = fixture(9165, 165, p(10, 12), 1000);
    join(&mut z, "b", 102, p(11, 14));
    let cast = z.tick(1);
    assert!(packets(&cast).iter().any(
        |p| matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==9165&&info.attack_type==1)
    ));
    assert!(!z
        .tick(600)
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let out = z.tick(601);
    assert_eq!(out, restored.tick(601));
    assert!(out.iter().any(|o|matches!(o,ZoneOutbound::PlayerDamaged{session_id,..}if *session_id==SessionId::new("a"))));
    let out = z.tick(701);
    assert_eq!(out, restored.tick(701));
    assert!(out.iter().any(|o|matches!(o,ZoneOutbound::PlayerDamaged{session_id,..}if *session_id==SessionId::new("b"))));
    assert!(!z
        .tick(701)
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
}

#[test]
fn warrior_odd_axis_range_requires_chasing_before_a_wide_line_attack() {
    let mut z = fixture(9165, 165, p(10, 13), 1000);
    let out = z.tick(1);
    assert!(!packets(&out)
        .iter()
        .any(|p| matches!(p, ServerPacket::ObjectAttack { info } if info.object_id == 9165)));
    assert!(packets(&out)
        .iter()
        .any(|p| matches!(p, ServerPacket::ObjectWalk { movement }
            if movement.object_id == 9165 && movement.position == Point { x: 10, y: 11 })));
    let cast = z.tick(502);
    assert!(packets(&cast).iter().any(
        |p| matches!(p, ServerPacket::ObjectAttack { info } if info.object_id == 9165 && info.attack_type == 1)
    ));
}
#[test]
fn archer_buff_is_delayed_retained_and_does_not_invent_base_boulder_stats() {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("horned-fixture"));
    join(&mut z, "a", 101, p(10, 16));
    // ArcherGuard is an explicit nonzero-MC fixture for the Archer buff action.
    z.spawn_world_event_monster(&spawn(9164, 164, "ArcherGuard", p(10, 10), 1000), 0);
    z.spawn_world_event_monster(&spawn(9170, 170, "BoulderSpirit", p(10, 6), 1000), 0);
    let cast = z.tick(1);
    assert!(packets(&cast).iter().any(|p|matches!(p,ServerPacket::ObjectRangeAttack{info}if info.object_id==9164&&info.attack_type==1&&info.target_id==9170)));
    assert!(state(&z)["native_monsters"]["9170"]["special_ai"]["horned"].is_null());
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(700), restored.tick(700));
    let out = z.tick(701);
    assert_eq!(out, restored.tick(701));
    assert!(packets(&out).iter().any(
        |p| matches!(p,ServerPacket::AddBuff{buff}if buff.object_id==9170&&buff.buff_type==50)
    ));
    let buffs = state(&z)["native_monsters"]["9170"]["special_ai"]["horned"]["buffs"].clone();
    assert_eq!(buffs[0]["min"], 255);
    assert_eq!(buffs[0]["max"], 255);
    assert_eq!(buffs[0]["expires"], 10701);
}

#[test]
fn shipped_zero_stat_mage_does_not_invent_damage_or_a_melee_cast() {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("horned-fixture"));
    join(&mut z, "a", 101, p(10, 11));
    z.spawn_world_event_monster(&spawn(9163, 163, "HornedMage", p(10, 10), 1000), 0);
    z.tick(1);
    let out = z.tick(502);
    assert!(!packets(&out)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==9163)));
    assert!(
        state(&z)["native_monsters"]["9163"]["special_ai"]["horned"]["pending"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(!z
        .tick(1002)
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
}

fn owned_horned_fixture(ai: u8, id: u32, distance: i32) -> (ZoneRuntime, u32, Point) {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("horned-owned-fixture"));
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
        map_file_name: "horned-owned-fixture".into(),
        position: p(20, 24),
        direction: MirDirection::Up,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats::default(),
    }));
    z.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("owner"),
        object_id: 0,
        spell: mir2_protocol::Spell::SummonToad,
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
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info } if info.name == "SpittingToad" => {
                Some((info.object_id, info.location.clone()))
            }
            _ => None,
        })
        .expect("owned pet from public spell");
    z.handle(ZoneCommand::SyncPlayerTransform {
        session_id: SessionId::new("owner"),
        position: p(pos.x + 10, pos.y),
        direction: MirDirection::Up,
    });
    let mut enemy = spawn(id, ai, "Guard", p(pos.x, pos.y - distance), 100000);
    enemy.max_hp = 100000;
    enemy.attack_speed_ms = 5000;
    assert!(z.spawn_world_event_monster(&enemy, 1500).0);
    (z, pet, pos)
}
fn pet_hp(z: &ZoneRuntime, id: u32) -> i64 {
    state(z)["native_monsters"][id.to_string()]["hp"]
        .as_i64()
        .unwrap()
}
fn wait_native_horned_cast(z: &mut ZoneRuntime, id: u32) -> u64 {
    for now in (1501..7000).step_by(100) {
        z.tick(now);
        if let Some(due) = state(z)["native_monsters"][id.to_string()]["special_ai"]["horned"]
            ["pending"]
            .as_array()
            .and_then(|a| a.iter().find(|h| h["buff"] == false))
            .and_then(|h| h["due"].as_u64())
        {
            return due;
        }
    }
    panic!("real owned target must receive a dedicated pending attack");
}
#[test]
fn horned_three_attackers_schedule_real_owned_targets_and_restore_impacts() {
    for ai in [163, 164, 165] {
        let id = 9300 + u32::from(ai);
        let (mut z, pet, _) = owned_horned_fixture(ai, id, 1);
        let hp = pet_hp(&z, pet);
        let due = wait_native_horned_cast(&mut z, id);
        assert_eq!(pet_hp(&z, pet), hp);
        let pending =
            &state(&z)["native_monsters"][id.to_string()]["special_ai"]["horned"]["pending"][0];
        assert!(pending["session"].is_null());
        assert_eq!(pending["target"], pet);
        assert!(
            pending["target_ref"]["Monster"]["incarnation"]
                .as_u64()
                .unwrap()
                > 0
        );
        let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        assert_eq!(z.tick(due - 1), restored.tick(due - 1));
        assert_eq!(pet_hp(&z, pet), hp);
        let out = z.tick(due);
        assert_eq!(out, restored.tick(due));
        assert!(pet_hp(&z, pet) < hp, "ai {ai}");
        assert!(!out
            .iter()
            .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
    }
}
#[test]
fn horned_native_pending_does_not_cross_retirement_and_id_reuse() {
    let (mut z, pet, pos) = owned_horned_fixture(164, 9464, 1);
    let due = wait_native_horned_cast(&mut z, 9464);
    z.despawn_world_event_monster(pet, due - 1);
    let mut replacement = spawn(pet, 2, "Deer", pos, 1000);
    replacement.disposition = Some(WorldEntityDisposition::Neutral);
    assert!(z.spawn_world_event_monster(&replacement, due - 1).0);
    z.tick(due);
    assert_eq!(pet_hp(&z, pet), 1000);
}
#[test]
fn mage_teleports_owned_target_with_authoritative_object_packets() {
    let mut found = false;
    for id in 9400..9440 {
        let (mut z, pet, pos) = owned_horned_fixture(163, id, 6);
        for now in (1501..3500).step_by(100) {
            let out = z.tick(now);
            if packets(&out).iter().any(
                |p| matches!(p,ServerPacket::ObjectTeleportOut{object_id,..}if *object_id==pet),
            ) {
                let next = &state(&z)["native_monsters"][pet.to_string()]["position"];
                assert!(next["x"] != pos.x || next["y"] != pos.y);
                assert!(packets(&out)
                    .iter()
                    .any(|p| matches!(p,ServerPacket::ObjectMonster{info}if info.object_id==pet)));
                assert!(packets(&out)
                    .iter()
                    .any(|p| matches!(p,ServerPacket::ObjectHealth{info}if info.object_id==pet)));
                found = true;
                break;
            }
        }
        if found {
            break;
        }
    }
    assert!(
        found,
        "vary legal source IDs to exercise real 1/5 teleport branch"
    );
}
