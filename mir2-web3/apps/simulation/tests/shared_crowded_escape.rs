//! Dense shared-world combat must not turn ordinary hit presentation into a
//! movement lock. Ring fixtures use genuine shared SpawnMonster/AI ticks. The
//! feature-gated finite-status fixture below only isolates Crystal control
//! semantics; it does not prove an ordinary boss-poison player journey.
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey,
    ZoneMonsterSpawn, ZoneOutbound, ZoneRuntime,
};

const OWNER: &str = "crowded-escape-owner";
const OBSERVER: &str = "crowded-escape-observer";
const OWNER_ID: u32 = 101;
const OBSERVER_ID: u32 = 102;
const HP: i32 = 100_000;
const FIRST_MONSTER: u32 = 9_000;
const RING: [(i32, i32); 7] = [
    (9, 9),
    (10, 9),
    (11, 9),
    (9, 10),
    (9, 11),
    (10, 11),
    (11, 11),
];

fn session(name: &str) -> SessionId {
    SessionId::new(name)
}
fn point(x: i32, y: i32) -> Point {
    Point { x, y }
}

fn fixture(class: MirClass) -> ZoneRuntime {
    let mut zone = ZoneRuntime::new_with_collision(
        ZoneKey::for_map("crowded-escape-fixture"),
        ZoneCollision::unbounded(),
    );
    for (name, object_id, position) in [
        (OWNER, OWNER_ID, point(10, 10)),
        (OBSERVER, OBSERVER_ID, point(22, 10)),
    ] {
        zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: session(name),
            account_id: format!("{name}-account"),
            character_index: 0,
            object_id,
            name: name.into(),
            class,
            gender: MirGender::Male,
            level: 40,
            hp: HP,
            max_hp: HP,
            mp: 100,
            map_file_name: "crowded-escape-fixture".into(),
            position,
            direction: MirDirection::Right,
            chat_profile: Default::default(),
            combat_stats: Default::default(),
        }));
        zone.handle(ZoneCommand::sync_player_combat_state(
            session(name),
            class,
            true,
            false,
            false,
            false,
            false,
            false,
        ));
    }
    zone
}

fn spawn(zone: &mut ZoneRuntime, index: usize, position: Point, now_ms: u64) {
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: session(OWNER),
        now_ms,
        monster: ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: FIRST_MONSTER + index as u32,
            name: "Skeleton".into(),
            name_colour_argb: -1,
            image: 3,
            ai: 0,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 10,
            max_hp: 10_000,
            hp: 10_000,
            experience: 0,
            move_speed_ms: 60_000,
            attack_speed_ms: 60_000,
            friendly_guild: None,
            position,
            direction: MirDirection::Right,
            defense: Default::default(),
            respawn: None,
            drops: Vec::new(),
        },
    });
}

fn packets_for<'a>(out: &'a [ZoneOutbound], name: &str) -> Vec<&'a ServerPacket> {
    let id = session(name);
    out.iter()
        .flat_map(|outbound| match outbound {
            ZoneOutbound::ToSession {
                session_id,
                packets,
            } if *session_id == id => packets.as_slice(),
            ZoneOutbound::ToMany {
                session_ids,
                packets,
            } if session_ids.contains(&id) => packets.as_slice(),
            ZoneOutbound::ToAll { packets } => packets.as_slice(),
            _ => &[],
        })
        .collect()
}

fn struck_count(out: &[ZoneOutbound], name: &str) -> usize {
    packets_for(out, name)
        .iter()
        .filter(|packet| {
            matches!(packet,
        ServerPacket::ObjectStruck { info } if info.object_id == OWNER_ID)
        })
        .count()
}

fn damaged(out: &[ZoneOutbound]) -> (usize, i32) {
    out.iter()
        .filter_map(|outbound| match outbound {
            ZoneOutbound::PlayerDamaged {
                session_id, damage, ..
            } if *session_id == session(OWNER) => Some(*damage),
            _ => None,
        })
        .fold((0, 0), |(count, sum), damage| (count + 1, sum + damage))
}

fn hp(zone: &ZoneRuntime) -> i32 {
    zone.player_vitals(&session(OWNER)).unwrap().0
}

fn staggered_ring() -> ZoneRuntime {
    let mut zone = fixture(MirClass::Warrior);
    for (first, last, now_ms) in [(0, 2, 0), (2, 4, 100), (4, 6, 200), (6, 7, 501)] {
        for index in first..last {
            let (x, y) = RING[index];
            spawn(&mut zone, index, point(x, y), now_ms);
        }
        zone.tick(now_ms);
    }
    zone
}

fn melee(
    zone: &mut ZoneRuntime,
    index: usize,
    direction: MirDirection,
    now_ms: u64,
    damage: i32,
) -> Vec<ZoneOutbound> {
    zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: session(OWNER),
        object_id: FIRST_MONSTER + index as u32,
        direction,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage,
        now_ms,
    })
}

fn move_at(zone: &mut ZoneRuntime, run: bool, seq: u64, now_ms: u64) -> Vec<ZoneOutbound> {
    let command = if run {
        ZoneCommand::Run {
            session_id: session(OWNER),
            direction: MirDirection::Right,
            seq,
            now_ms,
        }
    } else {
        ZoneCommand::Walk {
            session_id: session(OWNER),
            direction: MirDirection::Right,
            seq,
            now_ms,
        }
    };
    let mut out = zone.handle(command);
    out.extend(zone.tick_pending_movement(now_ms));
    out
}

#[test]
fn seven_same_tick_monster_hits_damage_each_time_but_present_one_shared_flinch() {
    let mut zone = fixture(MirClass::Warrior);
    for (index, (x, y)) in RING.into_iter().enumerate() {
        spawn(&mut zone, index, point(x, y), 0);
    }
    let attacks = zone.tick(0);
    assert_eq!(
        packets_for(&attacks, OWNER)
            .iter()
            .filter(|packet| matches!(packet,
        ServerPacket::ObjectAttack { info } if info.object_id >= FIRST_MONSTER))
            .count(),
        7
    );
    let out = zone.tick(600);
    let (count, damage) = damaged(&out);
    assert_eq!(
        count, 7,
        "all seven real hits must retain their independent settlement"
    );
    assert!(damage > 0);
    assert_eq!(hp(&zone), HP - damage);
    assert_eq!(
        packets_for(&out, OWNER)
            .iter()
            .filter(|packet| matches!(
                packet,
                ServerPacket::DamageIndicator {
                    object_id: OWNER_ID,
                    ..
                }
            ))
            .count(),
        7
    );
    for name in [OWNER, OBSERVER] {
        assert_eq!(struck_count(&out, name), 1,
            "Crystal monster attacks share the player's 500 ms StruckTime, not one window per attacker");
    }
}

#[test]
fn staggered_dense_hits_share_five_hundred_ms_window_without_dropping_damage() {
    let mut zone = staggered_ring();
    let mut total_damage = 0;
    for (now_ms, hit_count, flinches) in [(600, 2, 1), (700, 2, 0), (800, 2, 0), (1101, 1, 1)] {
        let out = zone.tick(now_ms);
        let (count, amount) = damaged(&out);
        assert_eq!(
            count, hit_count,
            "normal delayed monster hit count at {now_ms}"
        );
        total_damage += amount;
        assert_eq!(hp(&zone), HP - total_damage);
        for name in [OWNER, OBSERVER] {
            assert_eq!(
                struck_count(&out, name),
                flinches,
                "presentation window at {now_ms}"
            );
        }
    }
}

#[test]
fn ordinary_struck_admission_is_strictly_after_the_shared_five_hundred_ms_deadline() {
    let mut zone = fixture(MirClass::Warrior);
    for (index, now_ms) in [(0, 0), (1, 500), (2, 501)] {
        let (x, y) = RING[index];
        spawn(&mut zone, index, point(x, y), now_ms);
        zone.tick(now_ms);
    }
    for (now_ms, flinches) in [(600, 1), (1100, 0), (1101, 1)] {
        let out = zone.tick(now_ms);
        assert_eq!(damaged(&out).0, 1);
        for name in [OWNER, OBSERVER] {
            assert_eq!(
                struck_count(&out, name),
                flinches,
                "strict StruckTime comparison at {now_ms}"
            );
        }
    }
}

#[test]
fn pending_escape_keeps_finite_player_action_deadline_despite_dense_incoming_hits() {
    let mut zone = staggered_ring();
    zone.tick(600);
    let attack = melee(&mut zone, 0, MirDirection::UpLeft, 610, 1);
    assert!(packets_for(&attack, OWNER)
        .iter()
        .any(|packet| matches!(packet,
        ServerPacket::ObjectAttack { info } if info.object_id == OWNER_ID)));
    move_at(&mut zone, true, 1, 620);
    assert_eq!(zone.next_pending_movement_deadline_ms(), Some(1160));
    for now_ms in [700, 800, 1101] {
        assert!(damaged(&zone.tick(now_ms)).0 > 0);
        assert_eq!(
            zone.next_pending_movement_deadline_ms(),
            Some(1160),
            "ordinary hits must never push escape farther into the future"
        );
    }
    let escaped = zone.tick_pending_movement(1160);
    assert_eq!(
        zone.player_position(&session(OWNER)),
        Some(point(11, 10)),
        "Run from standstill should make its first lawful walk through the open corridor"
    );
    assert!(packets_for(&escaped, OWNER)
        .iter()
        .any(|packet| matches!(packet,
        ServerPacket::UserLocation { location } if location.position == point(11, 10))));
    assert_eq!(zone.next_pending_movement_deadline_ms(), None);
}

#[test]
fn completely_surrounded_player_cannot_cross_live_monster_but_can_escape_after_kill() {
    let mut zone = fixture(MirClass::Warrior);
    for (index, (x, y)) in RING.into_iter().enumerate() {
        spawn(&mut zone, index, point(x, y), 0);
    }
    spawn(&mut zone, 7, point(11, 10), 0);
    move_at(&mut zone, true, 1, 10);
    assert_eq!(zone.player_position(&session(OWNER)), Some(point(10, 10)));
    let accepted = melee(&mut zone, 7, MirDirection::Right, 100, 10_000);
    assert!(packets_for(&accepted, OWNER)
        .iter()
        .any(|packet| matches!(packet,
        ServerPacket::ObjectAttack { info } if info.object_id == OWNER_ID)));
    zone.tick(700);
    assert_eq!(zone.native_monster_is_alive(FIRST_MONSTER + 7), Some(false));
    move_at(&mut zone, true, 2, 701);
    assert_eq!(
        zone.player_position(&session(OWNER)),
        Some(point(11, 10)),
        "an ordinary corpse must release both the authoritative and retained collision state"
    );
}

#[cfg(feature = "test-support")]
fn status(zone: &mut ZoneRuntime, mask: u16, duration_ms: u64, now_ms: u64) {
    let out = zone.apply_finite_player_status_for_test(&session(OWNER), mask, duration_ms, now_ms);
    assert!(packets_for(&out, OWNER)
        .iter()
        .any(|packet| matches!(packet,
        ServerPacket::ObjectPoisoned { object_id: OWNER_ID, poison } if poison & mask != 0)));
}

#[cfg(feature = "test-support")]
#[test]
fn crystal_stun_allows_manual_walk_and_run_through_empty_corridor() {
    let mut zone = fixture(MirClass::Warrior);
    status(&mut zone, 16, 6000, 100);
    move_at(&mut zone, false, 1, 200);
    assert_eq!(
        zone.player_position(&session(OWNER)),
        Some(point(11, 10)),
        "Crystal STUN is not paralysis and must not block walking"
    );
    move_at(&mut zone, true, 2, 800);
    assert_eq!(
        zone.player_position(&session(OWNER)),
        Some(point(13, 10)),
        "still-active STUN must not block a lawful two-cell run"
    );
}

#[cfg(feature = "test-support")]
#[test]
fn crystal_stun_allows_physical_attack_but_never_bypasses_melee_action_cadence() {
    let mut zone = fixture(MirClass::Warrior);
    spawn(&mut zone, 0, point(9, 10), 0);
    status(&mut zone, 16, 6000, 100);
    let accepted = melee(&mut zone, 0, MirDirection::Left, 200, 8);
    assert!(
        packets_for(&accepted, OWNER)
            .iter()
            .any(|packet| matches!(packet,
        ServerPacket::ObjectAttack { info } if info.object_id == OWNER_ID)),
        "Crystal CanAttack permits STUN while CanCast rejects it"
    );
    let rejected = melee(&mut zone, 0, MirDirection::Left, 201, 8);
    assert!(!packets_for(&rejected, OWNER)
        .iter()
        .any(|packet| matches!(packet,
        ServerPacket::ObjectAttack { info } if info.object_id == OWNER_ID)));
}

fn cast(zone: &mut ZoneRuntime, now_ms: u64) -> Vec<ZoneOutbound> {
    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: session(OWNER),
        object_id: FIRST_MONSTER,
        spell: Spell::FireBall,
        direction: MirDirection::Left,
        target: point(9, 10),
        cast: true,
        level: 0,
        damage: 8,
        mp_cost: 3,
        cooldown_ms: 1800,
        now_ms,
    })
}

#[cfg(feature = "test-support")]
#[test]
fn crystal_stun_rejects_actual_cast_without_mana_cooldown_or_target_damage() {
    let mut baseline = fixture(MirClass::Wizard);
    spawn(&mut baseline, 0, point(9, 10), 0);
    assert!(
        packets_for(&cast(&mut baseline, 200), OWNER)
            .iter()
            .any(|packet| matches!(
                packet,
                ServerPacket::Magic {
                    spell: Spell::FireBall,
                    cast: true,
                    ..
                }
            )),
        "the fixture requires an otherwise legal, accepted FireBall"
    );
    let mut zone = fixture(MirClass::Wizard);
    spawn(&mut zone, 0, point(9, 10), 0);
    status(&mut zone, 16, 6000, 100);
    assert!(
        !zone.can_player_cast_magic(
            &session(OWNER),
            FIRST_MONSTER,
            Spell::FireBall,
            MirDirection::Left,
            &point(9, 10),
            true,
            8,
            3,
            1800,
            200
        ),
        "shared authoritative preflight must enforce Crystal STUN cast rejection"
    );
    let before = zone.player_vitals(&session(OWNER));
    let out = cast(&mut zone, 200);
    assert!(!packets_for(&out, OWNER).iter().any(|packet| matches!(
        packet,
        ServerPacket::Magic { cast: true, .. } | ServerPacket::ObjectProjectile { .. }
    )));
    assert_eq!(zone.player_vitals(&session(OWNER)), before);
    assert_eq!(
        zone.player_magic_cooldown_remaining_ms(&session(OWNER), Spell::FireBall, 200),
        Some(0)
    );
    assert_eq!(zone.native_monster_snapshots()[0].hp, 10_000);
}

#[cfg(feature = "test-support")]
#[test]
fn crystal_dazed_allows_escape_but_rejects_physical_attack_and_actual_magic() {
    let mut movement = fixture(MirClass::Warrior);
    status(&mut movement, 1024, 6000, 100);
    move_at(&mut movement, false, 1, 200);
    assert_eq!(
        movement.player_position(&session(OWNER)),
        Some(point(11, 10))
    );
    move_at(&mut movement, true, 2, 800);
    assert_eq!(
        movement.player_position(&session(OWNER)),
        Some(point(13, 10)),
        "Crystal DAZED cannot prevent a lawful escape"
    );

    let mut physical = fixture(MirClass::Warrior);
    spawn(&mut physical, 0, point(9, 10), 0);
    status(&mut physical, 1024, 6000, 100);
    let rejected = melee(&mut physical, 0, MirDirection::Left, 200, 8);
    assert!(
        !packets_for(&rejected, OWNER)
            .iter()
            .any(|packet| matches!(packet,
        ServerPacket::ObjectAttack { info } if info.object_id == OWNER_ID)),
        "Crystal CanAttack rejects DAZED independently of CanMove"
    );

    let mut magic = fixture(MirClass::Wizard);
    spawn(&mut magic, 0, point(9, 10), 0);
    status(&mut magic, 1024, 6000, 100);
    assert!(!magic.can_player_cast_magic(
        &session(OWNER),
        FIRST_MONSTER,
        Spell::FireBall,
        MirDirection::Left,
        &point(9, 10),
        true,
        8,
        3,
        1800,
        200
    ));
    let before = magic.player_vitals(&session(OWNER));
    let rejected = cast(&mut magic, 200);
    assert!(!packets_for(&rejected, OWNER).iter().any(|packet| matches!(
        packet,
        ServerPacket::Magic { cast: true, .. } | ServerPacket::ObjectProjectile { .. }
    )));
    assert_eq!(magic.player_vitals(&session(OWNER)), before);
    assert_eq!(
        magic.player_magic_cooldown_remaining_ms(&session(OWNER), Spell::FireBall, 200),
        Some(0)
    );
}

#[cfg(feature = "test-support")]
#[test]
fn crystal_cast_status_mask_keeps_lr_paralysis_out_and_preparation_neutral() {
    let mut lr = fixture(MirClass::Wizard);
    spawn(&mut lr, 0, point(9, 10), 0);
    status(&mut lr, 256, 6000, 100);
    assert!(
        lr.can_player_cast_magic(
            &session(OWNER),
            FIRST_MONSTER,
            Spell::FireBall,
            MirDirection::Left,
            &point(9, 10),
            true,
            8,
            3,
            1800,
            200
        ),
        "Crystal server CanCast omits LRParalysis even though CanMove rejects it"
    );
    assert!(packets_for(&cast(&mut lr, 200), OWNER)
        .iter()
        .any(|packet| matches!(
            packet,
            ServerPacket::Magic {
                spell: Spell::FireBall,
                cast: true,
                ..
            }
        )));

    for mask in [8, 16, 32, 1024] {
        let mut preparation = fixture(MirClass::Wizard);
        spawn(&mut preparation, 0, point(9, 10), 0);
        status(&mut preparation, mask, 6000, 100);
        assert!(preparation.can_player_cast_magic(
            &session(OWNER),
            FIRST_MONSTER,
            Spell::FireBall,
            MirDirection::Left,
            &point(9, 10),
            false,
            8,
            3,
            1800,
            200
        ));
        let before = preparation.player_vitals(&session(OWNER));
        let out = preparation.handle(ZoneCommand::PlayerCastMagic {
            session_id: session(OWNER),
            object_id: FIRST_MONSTER,
            spell: Spell::FireBall,
            direction: MirDirection::Left,
            target: point(9, 10),
            cast: false,
            level: 0,
            damage: 8,
            mp_cost: 3,
            cooldown_ms: 1800,
            now_ms: 200,
        });
        assert!(packets_for(&out, OWNER).iter().any(|packet| matches!(
            packet,
            ServerPacket::Magic {
                spell: Spell::FireBall,
                cast: false,
                ..
            }
        )));
        assert_eq!(preparation.player_vitals(&session(OWNER)), before);
        assert_eq!(
            preparation.player_magic_cooldown_remaining_ms(&session(OWNER), Spell::FireBall, 200),
            Some(0)
        );
    }
}

#[cfg(feature = "test-support")]
#[test]
fn paralysis_frozen_and_lr_paralysis_keep_real_block_until_deadline_then_new_held_move_works() {
    for mask in [8, 32, 256] {
        let mut zone = fixture(MirClass::Warrior);
        status(&mut zone, mask, 500, 100);
        move_at(&mut zone, true, 1, 599);
        assert_eq!(
            zone.player_position(&session(OWNER)),
            Some(point(10, 10)),
            "real control bit {mask} cannot be bypassed by escape input"
        );
        let cleared = zone.tick(600);
        assert!(packets_for(&cleared, OWNER).iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectPoisoned {
                object_id: OWNER_ID,
                poison: 0
            }
        )));
        move_at(&mut zone, true, 2, 600);
        assert_eq!(
            zone.player_position(&session(OWNER)),
            Some(point(11, 10)),
            "held input after control expiry must become a new finite action for bit {mask}"
        );
    }
}

#[cfg(feature = "test-support")]
#[test]
fn frozen_expiry_releases_movement_even_when_independent_stun_remains_active() {
    let mut zone = fixture(MirClass::Warrior);
    status(&mut zone, 16, 6000, 100);
    status(&mut zone, 8, 500, 100);
    move_at(&mut zone, true, 1, 599);
    assert_eq!(zone.player_position(&session(OWNER)), Some(point(10, 10)));
    let cleared = zone.tick(600);
    assert!(packets_for(&cleared, OWNER).iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectPoisoned {
            object_id: OWNER_ID,
            poison: 16
        }
    )));
    move_at(&mut zone, true, 2, 600);
    assert_eq!(zone.player_position(&session(OWNER)), Some(point(11, 10)));
}

#[test]
fn dense_struck_window_survives_exact_checkpoint_without_restarting_hit_flinch() {
    let mut original = staggered_ring();
    original.tick(600);
    let bytes = original.checkpoint_bytes().unwrap();
    let mut restored = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
    let first = original.tick(700);
    let second = restored.tick(700);
    assert_eq!(
        first, second,
        "an exact checkpoint keeps ordinary pending impacts and presentation admission"
    );
    assert_eq!(damaged(&second).0, 2);
    assert_eq!(
        struck_count(&second, OWNER),
        0,
        "same living player's window survives recovery"
    );
}

#[test]
fn living_struck_clock_is_optional_in_checkpoints_and_cleared_on_life_transition() {
    let mut zone = staggered_ring();
    assert!(
        !String::from_utf8(zone.checkpoint_bytes().unwrap())
            .unwrap()
            .contains("native_struck_ready_at_ms"),
        "absent clock keeps older checkpoint state encoding unchanged"
    );
    zone.tick(600);
    assert!(String::from_utf8(zone.checkpoint_bytes().unwrap())
        .unwrap()
        .contains("native_struck_ready_at_ms"));
    zone.handle(ZoneCommand::SyncPlayerVitals {
        session_id: session(OWNER),
        hp: 0,
        max_hp: HP,
        mp: 100,
    });
    assert!(
        !String::from_utf8(zone.checkpoint_bytes().unwrap())
            .unwrap()
            .contains("native_struck_ready_at_ms"),
        "death clears the old life's presentation window"
    );
    zone.handle(ZoneCommand::SyncPlayerVitals {
        session_id: session(OWNER),
        hp: HP,
        max_hp: HP,
        mp: 100,
    });
    let checkpoint = zone.checkpoint_bytes().unwrap();
    assert!(!String::from_utf8(checkpoint.clone())
        .unwrap()
        .contains("native_struck_ready_at_ms"));
    let restored = ZoneRuntime::restore_checkpoint(&checkpoint).unwrap();
    let mut before: serde_json::Value = serde_json::from_slice(&checkpoint).unwrap();
    let after: serde_json::Value =
        serde_json::from_slice(&restored.checkpoint_bytes().unwrap()).unwrap();
    // The original root is verified before public cold recovery clears online
    // authority. That intentional P3 change is independent of the optional
    // presentation clock; it must not erase or default any other saved field.
    assert_eq!(before["online_presence"].as_object().unwrap().len(), 2);
    assert_eq!(after["online_presence"], serde_json::json!({}));
    assert_eq!(
        after["state_root"].as_str().unwrap(),
        restored.canonical_state_root().unwrap()
    );
    let mut tampered = before.clone();
    tampered["players"][OWNER]["hp"] = serde_json::json!(HP - 1);
    assert!(
        ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&tampered).unwrap())
            .unwrap_err()
            .contains("state root mismatch"),
        "cold authority clearing must not bypass the original root verification"
    );
    before["online_presence"] = serde_json::json!({});
    before["state_root"] = after["state_root"].clone();
    assert_eq!(
        after, before,
        "only cold online authority/root change; no presentation clock or unrelated state is injected"
    );
}
