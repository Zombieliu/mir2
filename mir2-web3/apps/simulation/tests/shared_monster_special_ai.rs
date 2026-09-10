//! Public shared-Zone contracts for Crystal WoomaTaurus.ProcessAI.
//! Reference: Crystal/Server/MirObjects/Monsters/WoomaTaurus.cs:18-77.
//! These tests inspect genuine checkpoint output; they never rewrite a root.

use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneBounds, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey,
    ZoneMonsterSpawn, ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};
use serde_json::Value;

const MONSTER_ID: u32 = 9_111;
const SOURCE_MAP: &str = "shared-monster-special-ai-fixture";

fn point(x: i32, y: i32) -> Point {
    Point { x, y }
}

fn player(session: &str, object_id: u32, position: Point) -> ZoneJoin {
    ZoneJoin {
        session_id: SessionId::new(session),
        account_id: format!("{session}-account"),
        character_index: object_id as i32,
        object_id,
        name: session.to_string(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 40,
        hp: 100_000,
        max_hp: 100_000,
        mp: 100,
        map_file_name: SOURCE_MAP.to_string(),
        position,
        direction: MirDirection::Down,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats {
            min_dc: 10,
            max_dc: 10,
            accuracy: 100,
            min_ac: 100_000,
            max_ac: 100_000,
            ..Default::default()
        },
    }
}

fn monster(ai: u8, position: Point) -> ZoneMonsterSpawn {
    ZoneMonsterSpawn {
        crystal_drop_seed: None,
        object_id: MONSTER_ID,
        name: if ai == 11 { "WoomaTaurus" } else { "Scarecrow" }.to_string(),
        name_colour_argb: -1,
        image: if ai == 11 { 11 } else { 5 },
        ai,
        disposition: Some(WorldEntityDisposition::Hostile),
        level: 30,
        max_hp: 70,
        hp: 70,
        experience: 0,
        move_speed_ms: 3_000,
        attack_speed_ms: 5_000,
        friendly_guild: None,
        position,
        direction: MirDirection::Down,
        defense: Default::default(),
        respawn: None,
        drops: Vec::new(),
    }
}

fn spawn(zone: &mut ZoneRuntime, owner: &str, ai: u8, position: Point) -> Vec<ZoneOutbound> {
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new(owner),
        monster: monster(ai, position),
        now_ms: 0,
    })
}

fn packets_for<'a>(outbounds: &'a [ZoneOutbound], recipient: &str) -> Vec<&'a ServerPacket> {
    let recipient = SessionId::new(recipient);
    let mut packets = Vec::new();
    for outbound in outbounds {
        match outbound {
            ZoneOutbound::ToSession {
                session_id,
                packets: batch,
            } if session_id == &recipient => {
                packets.extend(batch);
            }
            ZoneOutbound::ToMany {
                session_ids,
                packets: batch,
            } if session_ids.contains(&recipient) => {
                packets.extend(batch);
            }
            ZoneOutbound::ToAll { packets: batch } => packets.extend(batch),
            _ => {}
        }
    }
    packets
}

fn checkpoint_monster(zone: &ZoneRuntime) -> Value {
    let bytes = zone.checkpoint_bytes().expect("real Zone checkpoint");
    let checkpoint: Value = serde_json::from_slice(&bytes).expect("checkpoint JSON");
    checkpoint["native_monsters"][MONSTER_ID.to_string()].clone()
}

fn teleport_surface(packets: &[&ServerPacket]) -> Vec<&'static str> {
    packets
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::ObjectTeleportOut { object_id, .. } if *object_id == MONSTER_ID => {
                Some("out")
            }
            ServerPacket::ObjectRemove { object_id } if *object_id == MONSTER_ID => Some("remove"),
            ServerPacket::ObjectMonster { info } if info.object_id == MONSTER_ID => Some("spawn"),
            ServerPacket::ObjectTeleportIn { object_id, .. } if *object_id == MONSTER_ID => {
                Some("in")
            }
            ServerPacket::ObjectHealth { info } if info.object_id == MONSTER_ID => Some("health"),
            ServerPacket::ObjectWalk { movement } if movement.object_id == MONSTER_ID => {
                Some("walk")
            }
            _ => None,
        })
        .collect()
}

#[test]
fn wooma_surrounded_teleport_reconciles_two_observers_without_fake_walk_or_duplicate_tick() {
    // The single free unoccupied destination is deliberately beyond both AOIs.
    // This locks the all-map destination contract without depending on RNG.
    let old = point(4, 4);
    let new = point(60, 60);
    let old_observer = point(2, 2);
    let new_observer = point(62, 62);
    let free = [
        old.clone(),
        new.clone(),
        old_observer.clone(),
        new_observer.clone(),
    ];
    let blocked = (0..=64)
        .flat_map(|y| (0..=64).map(move |x| point(x, y)))
        .filter(|cell| !free.contains(cell))
        .collect::<Vec<_>>();
    let collision = ZoneCollision::unbounded()
        .with_bounds(ZoneBounds::new(0, 64, 0, 64))
        .with_blocked_cells(blocked);
    let mut zone = ZoneRuntime::new_with_collision(ZoneKey::for_map(SOURCE_MAP), collision);
    zone.handle(ZoneCommand::Join(player("old-observer", 101, old_observer)));
    zone.handle(ZoneCommand::Join(player("new-observer", 102, new_observer)));
    let spawned = spawn(&mut zone, "old-observer", 11, old.clone());
    assert!(teleport_surface(&packets_for(&spawned, "old-observer")).contains(&"spawn"));
    assert!(!teleport_surface(&packets_for(&spawned, "new-observer")).contains(&"spawn"));

    let teleported = zone.tick(1);
    assert_eq!(zone.native_monster_snapshots()[0].position, new);
    assert_eq!(
        teleport_surface(&packets_for(&teleported, "old-observer")),
        ["out", "remove"]
    );
    assert_eq!(
        teleport_surface(&packets_for(&teleported, "new-observer")),
        ["spawn", "in", "health"]
    );
    let state = checkpoint_monster(&zone);
    assert_eq!(state["special_ai"]["wooma"]["next_teleport_ms"], 10_001);

    // The destination is also surrounded. Only the cooldown prevents returning
    // to the old free tile. Equality does not satisfy Crystal's strict > check.
    for now_ms in [1, 2, 10_001] {
        let repeated = zone.tick(now_ms);
        for recipient in ["old-observer", "new-observer"] {
            let surface = teleport_surface(&packets_for(&repeated, recipient));
            assert!(
                !surface
                    .iter()
                    .any(|event| matches!(*event, "out" | "in" | "walk")),
                "duplicate/premature teleport at {now_ms}: {surface:?}"
            );
        }
        assert_eq!(zone.native_monster_snapshots()[0].position, new);
    }
}

fn damaged_wooma() -> ZoneRuntime {
    // An unknown map key resolves to the same unbounded collision on restore.
    let mut zone = ZoneRuntime::new(ZoneKey::for_map(SOURCE_MAP));
    zone.handle(ZoneCommand::Join(player("attacker", 101, point(10, 11))));
    zone.handle(ZoneCommand::Join(player("observer", 102, point(15, 15))));
    spawn(&mut zone, "attacker", 11, point(10, 10));
    zone.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("attacker"),
        MirClass::Warrior,
        false,
        false,
        true,
        false,
        false,
        false,
    ));
    let attacked = zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("attacker"),
        object_id: MONSTER_ID,
        direction: MirDirection::Up,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 99_999, // Trusted authoritative stats must win over this legacy scalar.
        now_ms: 1,
    });
    assert!(packets_for(&attacked, "attacker").iter().any(
        |packet| matches!(packet, ServerPacket::ObjectAttack { info } if info.object_id == 101)
    ));
    let hit = zone.tick(1);
    for recipient in ["attacker", "observer"] {
        assert!(packets_for(&hit, recipient).iter().any(|packet|
            matches!(packet, ServerPacket::ObjectHealth { info } if info.object_id == MONSTER_ID)),
            "both observers must see the damage: {hit:?}");
    }
    assert_eq!(zone.native_monster_snapshots()[0].hp, 60);
    zone
}

#[test]
fn wooma_hp_stage_uses_crystal_rage_speeds_and_expires_strictly_after_eight_seconds() {
    let mut zone = damaged_wooma();
    let state = checkpoint_monster(&zone);
    let rage = &state["special_ai"]["wooma"];
    assert_eq!(rage["stage"], 6);
    assert_eq!(rage["mad_until_ms"], 8_001);
    assert_eq!(state["move_speed_ms"], 400);
    assert_eq!(state["attack_speed_ms"], 500);
    let base_move = rage["base_move_speed_ms"]
        .as_u64()
        .expect("base movement cadence");
    let base_attack = rage["base_attack_speed_ms"]
        .as_u64()
        .expect("base attack cadence");
    assert!(base_move > 400 && base_attack > 500);

    zone.tick(8_001);
    let at_deadline = checkpoint_monster(&zone);
    assert_eq!(at_deadline["special_ai"]["wooma"]["mad_until_ms"], 8_001);
    assert_eq!(at_deadline["move_speed_ms"], 400);
    assert_eq!(at_deadline["attack_speed_ms"], 500);

    // Give the next ordinary AI iteration time to run, rather than assuming
    // that an unrelated per-frame poll bypasses the monster's think cadence.
    zone.tick(8_401);
    let expired = checkpoint_monster(&zone);
    assert_eq!(expired["special_ai"]["wooma"]["mad_until_ms"], 0);
    assert_eq!(expired["move_speed_ms"], base_move);
    assert_eq!(expired["attack_speed_ms"], base_attack);
    assert_eq!(expired["special_ai"]["wooma"]["stage"], 6);
}

#[test]
fn wooma_checkpoint_preserves_rage_phase_and_does_not_retrigger_for_unchanged_hp() {
    let mut zone = damaged_wooma();
    let before = checkpoint_monster(&zone);
    let bytes = zone.checkpoint_bytes().expect("unmodified checkpoint");
    let mut restored = ZoneRuntime::restore_checkpoint(&bytes).expect("authenticated restore");
    assert_eq!(
        zone.canonical_state_root().unwrap(),
        restored.canonical_state_root().unwrap()
    );
    assert_eq!(checkpoint_monster(&restored), before);

    for now_ms in [1, 401, 901] {
        let expected = zone.tick(now_ms);
        let actual = restored.tick(now_ms);
        assert_eq!(
            actual, expected,
            "same checkpoint must replay the same outbounds"
        );
        assert_eq!(
            zone.canonical_state_root().unwrap(),
            restored.canonical_state_root().unwrap()
        );
        let state = checkpoint_monster(&restored);
        assert_eq!(state["special_ai"]["wooma"]["stage"], 6);
        assert_eq!(
            state["special_ai"]["wooma"]["mad_until_ms"], 8_001,
            "unchanged HP must not renew the eight-second rage"
        );
    }
}

#[test]
fn ordinary_monster_does_not_acquire_wooma_state_or_rage_speeds() {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map(SOURCE_MAP));
    zone.handle(ZoneCommand::Join(player("observer", 101, point(10, 11))));
    spawn(&mut zone, "observer", 0, point(10, 10));
    let before = checkpoint_monster(&zone);
    zone.tick(1);
    zone.tick(10_001);
    let after = checkpoint_monster(&zone);
    assert!(after.get("special_ai").is_none_or(Value::is_null));
    assert_eq!(after["move_speed_ms"], before["move_speed_ms"]);
    assert_eq!(after["attack_speed_ms"], before["attack_speed_ms"]);
    let bytes = zone
        .checkpoint_bytes()
        .expect("ordinary monster checkpoint");
    let restored = ZoneRuntime::restore_checkpoint(&bytes).expect("ordinary monster restore");
    assert_eq!(
        restored.canonical_state_root().unwrap(),
        zone.canonical_state_root().unwrap()
    );
}
