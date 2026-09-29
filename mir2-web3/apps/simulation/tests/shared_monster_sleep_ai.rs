use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCommand, ZoneJoin, ZoneKey, ZoneMonsterSpawn,
    ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};
use serde_json::Value;
const ID: u32 = 9034;
fn p(x: i32, y: i32) -> Point {
    Point { x, y }
}
fn spawn(ai: u8) -> ZoneMonsterSpawn {
    ZoneMonsterSpawn {
        crystal_drop_seed: None,
        object_id: ID,
        name: if ai == 34 { "FrostTiger" } else { "MirStatue" }.into(),
        name_colour_argb: -1,
        image: 0,
        ai,
        disposition: Some(WorldEntityDisposition::Hostile),
        level: 60,
        max_hp: if ai == 34 { 100 } else { 10 },
        hp: if ai == 34 { 100 } else { 10 },
        experience: 777,
        move_speed_ms: 600,
        attack_speed_ms: 1000,
        friendly_guild: None,
        defense: Default::default(),
        position: p(10, 10),
        direction: MirDirection::Down,
        respawn: None,
        drops: Vec::new(),
    }
}
fn fixture(ai: u8) -> ZoneRuntime {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map("sleep-fixture"));
    for (name, id, pos) in [("a", 101, p(10, 11)), ("b", 102, p(15, 15))] {
        zone.handle(ZoneCommand::Join(ZoneJoin {
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
            map_file_name: "sleep-fixture".into(),
            position: pos,
            direction: MirDirection::Up,
            chat_profile: Default::default(),
            combat_stats: ZonePlayerCombatStats {
                min_dc: 10,
                max_dc: 10,
                accuracy: 100,
                min_ac: 100000,
                max_ac: 100000,
                ..Default::default()
            },
        }));
    }
    zone.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("a"),
        MirClass::Warrior,
        false,
        false,
        true,
        false,
        false,
        false,
    ));
    assert!(zone.spawn_world_event_monster(&spawn(ai), 0).0);
    zone
}
fn state(zone: &ZoneRuntime) -> Value {
    let all: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    all["native_monsters"][ID.to_string()].clone()
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
fn hit(zone: &mut ZoneRuntime, now: u64) -> Vec<ZoneOutbound> {
    let mut out = zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("a"),
        object_id: ID,
        direction: MirDirection::Up,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: now,
    });
    out.extend(zone.tick(now));
    out
}

#[test]
fn frost_tiger_does_not_aggro_nearby_players_and_sits_until_attacked() {
    let mut zone = fixture(34);
    let first = zone.tick(1);
    assert!(!packets(&first).iter().any(
        |p| matches!(p,ServerPacket::ObjectAttack{info} if info.object_id==ID)
            || matches!(p,ServerPacket::ObjectRangeAttack{info} if info.object_id==ID)
    ));
    let deadline = state(&zone)["special_ai"]["sleep"]["sit_down_at_ms"]
        .as_u64()
        .unwrap();
    zone.tick(deadline);
    assert_eq!(state(&zone)["special_ai"]["sleep"]["sitting"], false);
    let sat = zone.tick(deadline + 1);
    assert!(packets(&sat).iter().any(
        |p| matches!(p,ServerPacket::ObjectSitDown{movement,sitting:true} if movement.object_id==ID)
    ));
    assert_eq!(state(&zone)["position"], serde_json::json!({"x":10,"y":10}));
    let angry = hit(&mut zone, deadline + 2);
    assert_eq!(state(&zone)["hp"], 90);
    assert_eq!(state(&zone)["special_ai"]["sleep"]["sitting"], false);
    assert_eq!(state(&zone)["special_ai"]["sleep"]["target_object_id"], 101);
    assert!(packets(&angry).iter().any(|p|matches!(p,ServerPacket::ObjectSitDown{movement,sitting:false} if movement.object_id==ID)));
}

#[test]
fn statue_lethal_damage_sleeps_without_kill_rewards_and_metadata_cannot_wake_it() {
    let mut zone = fixture(54);
    let out = hit(&mut zone, 1);
    assert_eq!(state(&zone)["hp"], 0);
    assert_eq!(state(&zone)["dead"], false);
    assert_eq!(state(&zone)["special_ai"]["sleep"]["sleeping"], true);
    assert!(!out
        .iter()
        .any(|o| matches!(o, ZoneOutbound::MonsterKillAward { .. })));
    assert!(!packets(&out)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectDied{info} if info.object_id==ID)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("a"),
        monster: spawn(54),
        now_ms: 2,
    });
    assert_eq!(state(&zone)["hp"], 0);
    assert_eq!(state(&zone)["special_ai"]["sleep"]["wake_at_ms"], 900001);
    hit(&mut zone, 1001);
    assert_eq!(state(&zone)["special_ai"]["sleep"]["wake_at_ms"], 900001);
}

#[test]
fn statue_checkpoint_preserves_sleep_and_wakes_strictly_after_fifteen_minutes() {
    let mut zone = fixture(54);
    hit(&mut zone, 1);
    let mut restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(state(&zone), state(&restored));
    assert_eq!(zone.tick(900001), restored.tick(900001));
    assert_eq!(state(&restored)["hp"], 0);
    let wake = restored.tick(900002);
    assert_eq!(state(&restored)["hp"], 10);
    assert_eq!(state(&restored)["dead"], false);
    assert_eq!(state(&restored)["special_ai"]["sleep"]["sleeping"], false);
    assert_eq!(
        state(&restored)["position"],
        serde_json::json!({"x":10,"y":10})
    );
    assert!(!packets(&wake).iter().any(|p| matches!(
        p,
        ServerPacket::ObjectRevived { .. } | ServerPacket::ObjectDied { .. }
    )));
    assert!(!wake
        .iter()
        .any(|o| matches!(o, ZoneOutbound::MonsterKillAward { .. })));
    assert!(packets(&wake).iter().any(
        |p| matches!(p,ServerPacket::ObjectHealth{info} if info.object_id==ID&&info.percent==100)
    ));
}

#[test]
fn sleeping_statue_ignores_stale_harvest_remove_and_ai_metadata() {
    let mut zone = fixture(54);
    hit(&mut zone, 1);
    let before = zone.checkpoint_bytes().unwrap();
    let mut stale = spawn(0);
    stale.hp = 0;
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("a"), monster: stale, now_ms: 2,
    });
    let out = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: SessionId::new("a"), owner_local_object_id: 101, now_ms: 2,
        packets: vec![ServerPacket::ObjectHarvested {
            movement: mir2_protocol::ObjectMovement {
                object_id: ID, position: Point { x: 99, y: 99 }, direction: MirDirection::Left,
            },
        }, ServerPacket::ObjectRemove { object_id: ID }],
    });
    assert!(out.is_empty());
    assert_eq!(zone.checkpoint_bytes().unwrap(), before);
}

fn late_join(zone: &mut ZoneRuntime, id: u32) -> Vec<ZoneOutbound> {
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new(format!("late{id}")), account_id: format!("late{id}"),
        character_index: 1, object_id: id, name: format!("late{id}"),
        class: MirClass::Warrior, gender: MirGender::Male, level: 50,
        hp: 1000, max_hp: 1000, mp: 100, map_file_name: "sleep-fixture".into(),
        position: p(12, 12), direction: MirDirection::Up,
        chat_profile: Default::default(), combat_stats: Default::default(),
    }))
}

#[test]
fn sleeping_and_awakened_statue_remain_alive_for_late_observers_after_checkpoint() {
    let mut zone = fixture(54);
    hit(&mut zone, 1);
    let mut restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    let sleeping = late_join(&mut restored, 103);
    assert!(packets(&sleeping).iter().any(|p| matches!(p,
        ServerPacket::ObjectMonster { info } if info.object_id == ID && !info.dead)));
    restored.tick(900002);
    let awake = late_join(&mut restored, 104);
    assert!(packets(&awake).iter().any(|p| matches!(p,
        ServerPacket::ObjectMonster { info } if info.object_id == ID && !info.dead)));
    assert!(packets(&awake).iter().any(|p| matches!(p,
        ServerPacket::ObjectHealth { info } if info.object_id == ID && info.percent == 100)));
}
