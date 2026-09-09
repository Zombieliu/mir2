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
fn state(z: &ZoneRuntime) -> serde_json::Value {
    serde_json::from_slice(&z.checkpoint_bytes().unwrap()).unwrap()
}
fn add_attacker(z: &mut ZoneRuntime, trap: u32) {add_attacker_at(z,trap,9001,1)}
fn add_attacker_at(z: &mut ZoneRuntime, trap: u32, actor:u32, dx:i32) {
    let p = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == trap)
        .unwrap()
        .position;
    z.spawn_world_event_monster(
        &ZoneMonsterSpawn {
            object_id: actor,
            name: "Ancient_WoomaGuardian".into(),
            name_colour_argb: -1,
            image: 0,
            ai: 0,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 30,
            max_hp: 10000,
            hp: 10000,
            experience: 100,
            move_speed_ms: 300,
            attack_speed_ms: 300,
            friendly_guild: None,
            position: Point { x: p.x + dx, y: p.y },
            direction: MirDirection::Left,
            defense: Default::default(),
            respawn: None,
            drops: Vec::new(),
        },
        1210,
    );
}
#[test]
fn monster_decoy_swing_reduces_trap_hp_after_500ms_without_player_credit() {
    let (mut z, id, _) = fixture();
    add_attacker(&mut z, id);
    let before = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == id)
        .unwrap()
        .hp;
    let out = z.tick(2000);
    assert!(packets(&out)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==9001)));
    assert_eq!(
        z.native_monster_snapshots()
            .iter()
            .find(|m| m.object_id == id)
            .unwrap()
            .hp,
        before
    );
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let out = z.tick(2500);
    assert_eq!(out, restored.tick(2500));
    let after = z
        .native_monster_snapshots()
        .iter()
        .find(|m| m.object_id == id)
        .unwrap()
        .hp;
    assert!(
        after < before,
        "ordinary Monster.Attacked is not StoneTrap.Struck immunity"
    );
    assert!(!out
        .iter()
        .any(|o| matches!(o, ZoneOutbound::MonsterKillAward { .. })));
}
#[test]
fn monsters_can_kill_a_stone_trap_without_awarding_its_owner_xp_or_loot() {
    let (mut z, id, due) = fixture();
    add_attacker(&mut z, id);
    add_attacker_at(&mut z,id,9002,-1);
    let mut died = false;
    for now in (2000..due).step_by(100) {
        let out = z.tick(now);
        assert!(!out
            .iter()
            .any(|o| matches!(o, ZoneOutbound::MonsterKillAward { .. })));
        assert!(!packets(&out)
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectItem { .. })));
        if z.native_monster_snapshots()
            .iter()
            .find(|m| m.object_id == id)
            .unwrap()
            .dead
        {
            assert!(packets(&out)
                .iter()
                .any(|p| matches!(p,ServerPacket::ObjectDied{info}if info.object_id==id)));
            died = true;
            break;
        }
    }
    assert!(
        died,
        "real incoming attacks must kill before the lifetime timer"
    );
}
fn fixture() -> (ZoneRuntime, u32, u64) {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("stone-trap-fixture"));
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
        map_file_name: "stone-trap-fixture".into(),
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
        spell: Spell::Stonetrap,
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
        .expect("real Stonetrap summon must spawn");
    let saved = state(&z);
    let due = saved["native_monsters"][id.to_string()]["special_ai"]["stone_trap"]["die_at_ms"]
        .as_u64()
        .unwrap();
    (z, id, due)
}
#[test]
fn stone_trap_expires_through_death_then_retains_source_corpse_interval() {
    let (mut z, id, due) = fixture();
    z.tick(due);
    assert!(
        !z.native_monster_snapshots()
            .iter()
            .find(|m| m.object_id == id)
            .unwrap()
            .dead
    );
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let out = z.tick(due + 1);
    assert_eq!(out, restored.tick(due + 1));
    assert!(packets(&out)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectDied{info}if info.object_id==id)));
    assert!(
        z.native_monster_snapshots()
            .iter()
            .find(|m| m.object_id == id)
            .unwrap()
            .dead
    );
    let later = z.tick(due + 2);
    assert!(!packets(&later)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectDied{info}if info.object_id==id)));
    z.tick(due + 180000);
    assert!(z
        .native_monster_snapshots()
        .iter()
        .any(|m| m.object_id == id));
    let out = z.tick(due + 180001);
    assert!(packets(&out)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectRemove{object_id}if *object_id==id)));
}
#[test]
fn stone_trap_has_source_image_extra_and_owner_name() {
    let (mut z, id, _) = fixture();
    let saved = state(&z);
    let m = &saved["native_monsters"][id.to_string()];
    let out = z.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new("viewer"),
        account_id: "viewer".into(),
        character_index: 2,
        object_id: 102,
        name: "viewer".into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 50,
        hp: 1000,
        max_hp: 1000,
        mp: 100,
        map_file_name: "stone-trap-fixture".into(),
        position: Point { x: 102, y: 102 },
        direction: MirDirection::Up,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
    assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectMonster{info} if info.object_id==id && info.image==358 && info.extra && info.name=="StoneTrap(owner)")));
    assert_eq!(m["visible_extra"], true);
}
#[test]
fn stone_trap_dies_when_owner_exceeds_fifteen_cells_without_chasing() {
    let (mut z, id, _) = fixture();
    let origin = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == id)
        .unwrap()
        .position;
    let mut died = false;
    for step in 1..=12 {
        let now = 1210 + step * 1000;
        z.handle(ZoneCommand::Walk {
            session_id: SessionId::new("owner"),
            direction: MirDirection::Left,
            seq: step,
            now_ms: now,
        });
        let out = z.tick(now);
        assert_eq!(
            z.native_monster_snapshots()
                .iter()
                .find(|m| m.object_id == id)
                .unwrap()
                .position,
            origin
        );
        if packets(&out)
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectDied{info}if info.object_id==id))
        {
            died = true;
            break;
        }
    }
    assert!(died);
}
