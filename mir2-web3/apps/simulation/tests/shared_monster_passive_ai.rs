use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCommand, ZoneJoin, ZoneKey, ZoneMonsterSpawn,
    ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};
use serde_json::Value;
fn deer_base_speed() -> u64 {
    u64::from(mir2_game_data::crystal_monster_by_name("Deer").unwrap().move_speed).max(900)
}
fn p(x: i32, y: i32) -> Point {
    Point { x, y }
}
fn state(zone: &ZoneRuntime, id: u32) -> Value {
    let value: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    value["native_monsters"][id.to_string()].clone()
}
fn fixture(timid: bool) -> (ZoneRuntime, u32) {
    for id in 9000..9128 {
        let mut zone = ZoneRuntime::new(ZoneKey::for_map("deer-shared-fixture"));
        assert!(
            zone.spawn_world_event_monster(
                &ZoneMonsterSpawn {
                    object_id: id,
                    name: "Deer".into(),
                    name_colour_argb: -1,
                    image: 1,
                    ai: 2,
                    disposition: Some(WorldEntityDisposition::Neutral),
                    level: 5,
                    max_hp: 100,
                    hp: 100,
                    experience: 0,
                    move_speed_ms: 900,
                    attack_speed_ms: 900,
                    friendly_guild: None,
                    defense: Default::default(),
                    position: p(10, 10),
                    direction: MirDirection::Down,
                    respawn: None,
                    drops: Vec::new()
                },
                0
            )
            .0
        );
        zone.tick(1);
        if state(&zone, id)["special_ai"]["passive"]["timid"] != timid {
            continue;
        }
        zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: SessionId::new("a"),
            account_id: "a".into(),
            character_index: 1,
            object_id: 101,
            name: "a".into(),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 30,
            hp: 10000,
            max_hp: 10000,
            mp: 100,
            map_file_name: "deer-shared-fixture".into(),
            position: p(10, 11),
            direction: MirDirection::Up,
            chat_profile: Default::default(),
            combat_stats: ZonePlayerCombatStats {
                min_dc: 10,
                max_dc: 10,
                accuracy: 100,
                ..Default::default()
            },
        }));
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
        return (zone, id);
    }
    panic!("saved seeded births did not cover requested temperament");
}
fn hit(zone: &mut ZoneRuntime, id: u32, now: u64) -> Vec<ZoneOutbound> {
    zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("a"),
        object_id: id,
        direction: MirDirection::Up,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: now,
    });
    zone.tick(now)
}
fn attacks(out: &[ZoneOutbound], id: u32) -> bool {
    out.iter().any(|o| match o {
        ZoneOutbound::ToSession { packets, .. }
        | ZoneOutbound::ToMany { packets, .. }
        | ZoneOutbound::ToAll { packets } => packets.iter().any(|p| {
            matches!(p,ServerPacket::ObjectAttack{info} if info.object_id==id)
                || matches!(p,ServerPacket::ObjectRangeAttack{info} if info.object_id==id)
        }),
        _ => false,
    })
}
#[test]
fn timid_deer_flees_direct_attacker_without_turning_hostile_and_respects_speed() {
    let (mut zone, id) = fixture(true);
    let out = hit(&mut zone, id, 2);
    assert_eq!(state(&zone, id)["hp"], 90);
    assert_eq!(state(&zone, id)["hostile_to_player"], false);
    assert_eq!(state(&zone, id)["move_speed_ms"], deer_base_speed() - 300);
    assert_eq!(
        state(&zone, id)["position"],
        serde_json::json!({"x":10,"y":9})
    );
    assert!(!attacks(&out, id));
    zone.tick(2 + deer_base_speed() - 300 - 1);
    assert_eq!(
        state(&zone, id)["position"],
        serde_json::json!({"x":10,"y":9})
    );
    zone.tick(2 + deer_base_speed() - 300);
    assert_eq!(
        state(&zone, id)["position"],
        serde_json::json!({"x":10,"y":8})
    );
}
#[test]
fn ordinary_deer_only_retaliates_after_a_real_hit() {
    let (mut zone, id) = fixture(false);
    assert!(!attacks(&zone.tick(2), id));
    assert_eq!(
        state(&zone, id)["position"],
        serde_json::json!({"x":10,"y":10})
    );
    let out = hit(&mut zone, id, 3);
    assert!(attacks(&out, id));
    assert_eq!(state(&zone, id)["hostile_to_player"], false);
    assert_eq!(state(&zone, id)["move_speed_ms"], deer_base_speed());
}
#[test]
fn saved_temperament_does_not_reroll_or_double_speed_discount_and_leaving_clears_target() {
    let (mut zone, id) = fixture(true);
    hit(&mut zone, id, 2);
    let mut restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(state(&zone, id), state(&restored, id));
    zone.handle(ZoneCommand::Leave {
        session_id: SessionId::new("a"),
    });
    restored.handle(ZoneCommand::Leave {
        session_id: SessionId::new("a"),
    });
    assert_eq!(zone.tick(2 + deer_base_speed() - 300), restored.tick(2 + deer_base_speed() - 300));
    assert_eq!(
        state(&restored, id)["special_ai"]["passive"]["target_object_id"],
        Value::Null
    );
    assert_eq!(state(&restored, id)["move_speed_ms"], deer_base_speed() - 300);
    assert_eq!(
        state(&restored, id)["position"],
        serde_json::json!({"x":10,"y":9})
    );
}
