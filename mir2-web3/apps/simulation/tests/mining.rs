use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    SessionId, ZoneBounds, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey, ZoneMiningTool,
    ZoneOutbound, ZoneRuntime,
};

fn tool() -> ZoneMiningTool {
    ZoneMiningTool {
        unique_id: 10_000,
        durability: 10_000,
        accuracy: 0,
        strong: 0,
        mine_rate_percent: 0,
        attack_delay_ms: 1030,
    }
}

fn join(id: &str, object: u32, x: i32, class: MirClass) -> ZoneJoin {
    ZoneJoin {
        session_id: SessionId::new(id),
        account_id: format!("{id}-account"),
        character_index: object as i32,
        object_id: object,
        name: id.to_string(),
        class,
        gender: MirGender::Male,
        level: 30,
        hp: 200,
        max_hp: 200,
        mp: 100,
        map_file_name: "D401".to_string(),
        position: Point { x, y: 5 },
        direction: MirDirection::Up,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }
}

fn sync(
    zone: &mut ZoneRuntime,
    id: &str,
    class: MirClass,
    riding: bool,
    dead: bool,
    blocked: bool,
    fishing: bool,
) {
    zone.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new(id),
        class,
        false,
        riding,
        false,
        dead,
        blocked,
        fishing,
    ));
}

fn fixture() -> ZoneRuntime {
    let mut zone = ZoneRuntime::new_with_collision(
        ZoneKey::for_map("D401"),
        ZoneCollision::unbounded()
            .with_bounds(ZoneBounds::new(0, 20, 0, 20))
            .with_blocked_cells([Point { x: 5, y: 4 }]),
    );
    for (id, obj, x, class) in [
        ("first", 1, 5, MirClass::Warrior),
        ("second", 2, 6, MirClass::Wizard),
    ] {
        zone.handle(ZoneCommand::Join(join(id, obj, x, class)));
        sync(&mut zone, id, class, false, false, false, false);
    }
    zone
}

fn state(zone: &ZoneRuntime) -> serde_json::Value {
    serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap()
}
fn stones(zone: &ZoneRuntime) -> u64 {
    state(zone)["mining"]["spots"]["5,4"]["stones_left"]
        .as_u64()
        .unwrap()
}
fn mine(zone: &mut ZoneRuntime, id: &str, dir: MirDirection, t: u64) -> Vec<ZoneOutbound> {
    let swing = zone
        .prepare_mining_swing(&SessionId::new(id), dir, &tool(), t)
        .unwrap();
    zone.commit_mining_swing(&swing).unwrap()
}
fn packets(out: &[ZoneOutbound]) -> Vec<&ServerPacket> {
    out.iter()
        .flat_map(|o| match o {
            ZoneOutbound::ToSession { packets, .. }
            | ZoneOutbound::ToMany { packets, .. }
            | ZoneOutbound::ToAll { packets } => packets.as_slice(),
            _ => &[],
        })
        .collect()
}

#[test]
fn mining_only_admits_live_players_with_tools_and_real_in_bounds_walls() {
    let mut zone = fixture();
    let id = SessionId::new("first");
    assert!(zone
        .prepare_mining_swing(&id, MirDirection::Down, &tool(), 1_000_000)
        .is_none());
    assert!(zone
        .prepare_mining_swing(
            &SessionId::new("missing"),
            MirDirection::Up,
            &tool(),
            1_000_000
        )
        .is_none());
    for bad in [
        ZoneMiningTool {
            unique_id: 0,
            ..tool()
        },
        ZoneMiningTool {
            durability: 0,
            ..tool()
        },
    ] {
        assert!(zone
            .prepare_mining_swing(&id, MirDirection::Up, &bad, 1_000_000)
            .is_none());
    }
    for (riding, dead, blocked, fishing) in [
        (true, false, false, false),
        (false, true, false, false),
        (false, false, true, false),
        (false, false, false, true),
    ] {
        sync(
            &mut zone,
            "first",
            MirClass::Warrior,
            riding,
            dead,
            blocked,
            fishing,
        );
        assert!(zone
            .prepare_mining_swing(&id, MirDirection::Up, &tool(), 1_000_000)
            .is_none());
    }
    let unbounded =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("D401"), ZoneCollision::unbounded());
    assert!(unbounded
        .prepare_mining_swing(&id, MirDirection::Up, &tool(), 1_000_000)
        .is_none());
    assert!(state(&zone)["mining"]["spots"]
        .as_object()
        .unwrap()
        .is_empty());
}

#[test]
fn mining_refill_swing_and_shared_consumption_survive_leave_and_rejoin() {
    let mut zone = fixture();
    let first = SessionId::new("first");
    let initial = zone
        .prepare_mining_swing(&first, MirDirection::Up, &tool(), 1_000_000)
        .unwrap();
    assert_eq!(initial.tool_damage(), 0);
    assert!(initial.ore().is_none());
    zone.commit_mining_swing(&initial).unwrap();
    let before = stones(&zone);
    assert!(before > 0 && before < 80);
    mine(&mut zone, "second", MirDirection::UpLeft, 1_001_030);
    assert_eq!(stones(&zone), before - 1);
    zone.handle(ZoneCommand::Leave {
        session_id: first.clone(),
    });
    zone.handle(ZoneCommand::Join(join("first", 1, 5, MirClass::Warrior)));
    sync(
        &mut zone,
        "first",
        MirClass::Warrior,
        false,
        false,
        false,
        false,
    );
    mine(&mut zone, "first", MirDirection::Up, 1_002_060);
    assert_eq!(stones(&zone), before - 2);
}

#[test]
fn mining_uses_exact_five_minute_refill_deadline_and_swing_clock() {
    let mut zone = fixture();
    let id = SessionId::new("first");
    mine(&mut zone, "first", MirDirection::Up, 1_000_000);
    let initial = stones(&zone);
    assert!(zone
        .prepare_mining_swing(&id, MirDirection::Up, &tool(), 1_001_029)
        .is_none());
    for i in 0..initial {
        mine(&mut zone, "first", MirDirection::Up, 1_001_030 + i * 1_030);
    }
    assert_eq!(stones(&zone), 0);
    assert_eq!(
        state(&zone)["mining"]["spots"]["5,4"]["regen_at_ms"],
        1_300_000
    );
    mine(&mut zone, "first", MirDirection::Up, 1_300_000);
    assert_eq!(
        stones(&zone),
        0,
        "Crystal refills strictly after its deadline"
    );
    let refill = zone
        .prepare_mining_swing(&id, MirDirection::Up, &tool(), 1_301_030)
        .unwrap();
    assert_eq!(refill.tool_damage(), 0);
    assert!(refill.ore().is_none());
    zone.commit_mining_swing(&refill).unwrap();
    assert!(stones(&zone) < 80);
    assert_eq!(
        state(&zone)["mining"]["spots"]["5,4"]["regen_at_ms"],
        1_601_030
    );
}

#[test]
fn mining_hit_dust_is_delayed_and_only_observed_inside_the_same_zone_aoi() {
    let mut zone = fixture();
    mine(&mut zone, "first", MirDirection::Up, 1_000_000);
    let mut pick = tool();
    pick.accuracy = 10;
    pick.strong = 40;
    pick.mine_rate_percent = 100;
    let swing = zone
        .prepare_mining_swing(&SessionId::new("first"), MirDirection::Up, &pick, 1_001_030)
        .unwrap();
    assert_eq!(swing.tool_damage(), 1);
    let out = zone.commit_mining_swing(&swing).unwrap();
    assert!(packets(&out)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectAttack {info} if info.spell==Spell::None as u8)));
    assert!(!packets(&zone.tick(1_001_429))
        .iter()
        .any(|p| matches!(p, ServerPacket::MapEffect { effect: 12, .. })));
    let out = zone.tick(1_001_430);
    assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::MapEffect{effect:12,location,value:0} if *location==Point{x:5,y:5})));
    assert!(out.iter().any(|o|matches!(o,ZoneOutbound::ToMany{session_ids,..} if session_ids.contains(&SessionId::new("first")) && session_ids.contains(&SessionId::new("second")))));
}

#[test]
fn mining_replayed_swing_and_tampered_checkpoint_are_rejected() {
    let mut zone = fixture();
    let swing = zone
        .prepare_mining_swing(
            &SessionId::new("first"),
            MirDirection::Up,
            &tool(),
            1_000_000,
        )
        .unwrap();
    zone.commit_mining_swing(&swing).unwrap();
    let before = zone.checkpoint_bytes().unwrap();
    assert!(zone.commit_mining_swing(&swing).is_none());
    assert_eq!(zone.checkpoint_bytes().unwrap(), before);
    for key in ["sequence", "spots"] {
        let mut value: serde_json::Value = serde_json::from_slice(&before).unwrap();
        if key == "sequence" {
            value["mining"][key] = serde_json::json!(999);
        } else {
            value["mining"][key]["5,4"]["stones_left"] = serde_json::json!(80);
        }
        assert!(ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}
