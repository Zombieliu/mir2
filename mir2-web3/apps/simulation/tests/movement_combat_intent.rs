use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey,
    ZoneMonsterSpawn, ZoneOutbound, ZoneRuntime,
};

fn owner() -> SessionId {
    SessionId::new("movement-combat-owner")
}

fn join(id: SessionId, object_id: u32, class: MirClass, position: Point) -> ZoneJoin {
    ZoneJoin {
        session_id: id,
        account_id: format!("isolated-{object_id}"),
        character_index: 0,
        object_id,
        name: format!("Player{object_id}"),
        class,
        gender: MirGender::Male,
        level: 30,
        hp: 500,
        max_hp: 500,
        mp: 100,
        map_file_name: "movement-combat".into(),
        position,
        direction: MirDirection::Right,
        chat_profile: mir2_simulation::ZoneChatProfile {
            attack_mode: 5,
            ..Default::default()
        },
        combat_stats: Default::default(),
    }
}

fn fixture(class: MirClass) -> ZoneRuntime {
    let mut zone = ZoneRuntime::new_with_collision(
        ZoneKey::for_map("movement-combat"),
        ZoneCollision::unbounded(),
    );
    zone.handle(ZoneCommand::Join(join(
        owner(),
        101,
        class,
        Point { x: 10, y: 10 },
    )));
    zone.handle(ZoneCommand::sync_player_combat_state(
        owner(),
        class,
        true,
        false,
        true,
        false,
        false,
        false,
    ));
    zone
}

fn monster(position: Point) -> ZoneMonsterSpawn {
    ZoneMonsterSpawn {
        crystal_drop_seed: None,
        object_id: 2001,
        name: "Scarecrow".into(),
        name_colour_argb: -1,
        image: 5,
        ai: 0,
        disposition: Some(WorldEntityDisposition::Hostile),
        level: 1,
        max_hp: 500,
        hp: 500,
        experience: 1,
        move_speed_ms: 600,
        attack_speed_ms: 1200,
        friendly_guild: None,
        position,
        direction: MirDirection::Left,
        defense: Default::default(),
        respawn: None,
        drops: Vec::new(),
    }
}

fn spawn(zone: &mut ZoneRuntime, position: Point) {
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: owner(),
        monster: monster(position),
        now_ms: 0,
    });
}

fn buffer_run(zone: &mut ZoneRuntime) -> u64 {
    zone.handle(ZoneCommand::Walk {
        session_id: owner(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 100,
    });
    assert_eq!(zone.player_position(&owner()), Some(Point { x: 11, y: 10 }));
    zone.handle(ZoneCommand::Run {
        session_id: owner(),
        direction: MirDirection::Up,
        seq: 2,
        now_ms: 101,
    });
    zone.next_pending_movement_deadline_ms()
        .expect("Run is buffered by the walk's ActionTime")
}

fn melee(now_ms: u64, materialized: bool, position: Point) -> ZoneCommand {
    if materialized {
        ZoneCommand::PlayerAttackMaterializedObject {
            session_id: owner(),
            object_id: 2001,
            monster: Some(monster(position)),
            direction: MirDirection::Right,
            spell: Spell::None as u8,
            level: 0,
            attack_type: 0,
            damage: 10,
            now_ms,
        }
    } else {
        ZoneCommand::PlayerAttackObject {
            session_id: owner(),
            object_id: 2001,
            direction: MirDirection::Right,
            spell: Spell::None as u8,
            level: 0,
            attack_type: 0,
            damage: 10,
            now_ms,
        }
    }
}

fn ranged(now_ms: u64, materialized: bool, position: Point) -> ZoneCommand {
    if materialized {
        ZoneCommand::PlayerRangeAttackMaterializedObject {
            session_id: owner(),
            object_id: 2001,
            monster: Some(monster(position.clone())),
            direction: MirDirection::Right,
            target: position,
            spell: Spell::None,
            level: 0,
            attack_type: 0,
            damage: 10,
            now_ms,
        }
    } else {
        ZoneCommand::PlayerRangeAttackObject {
            session_id: owner(),
            object_id: 2001,
            direction: MirDirection::Right,
            target: position,
            spell: Spell::None,
            level: 0,
            attack_type: 0,
            damage: 10,
            now_ms,
        }
    }
}

fn magic(now_ms: u64, cast: bool, with_item: bool) -> ZoneCommand {
    if with_item {
        ZoneCommand::PlayerCastMagicWithItem {
            session_id: owner(),
            object_id: 2001,
            spell: Spell::FireBall,
            direction: MirDirection::Right,
            target: Point { x: 12, y: 10 },
            cast,
            level: 0,
            damage: 10,
            mp_cost: 10,
            cooldown_ms: 5000,
            item_param: 0,
            now_ms,
        }
    } else {
        ZoneCommand::PlayerCastMagic {
            session_id: owner(),
            object_id: 2001,
            spell: Spell::FireBall,
            direction: MirDirection::Right,
            target: Point { x: 12, y: 10 },
            cast,
            level: 0,
            damage: 10,
            mp_cost: 10,
            cooldown_ms: 5000,
            now_ms,
        }
    }
}

fn packets(out: &[ZoneOutbound]) -> Vec<&ServerPacket> {
    out.iter()
        .flat_map(|out| match out {
            ZoneOutbound::ToSession { packets, .. }
            | ZoneOutbound::ToMany { packets, .. }
            | ZoneOutbound::ToAll { packets } => packets.iter().collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect()
}

fn has_action(out: &[ZoneOutbound]) -> bool {
    packets(out).iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::ObjectAttack { .. }
                | ServerPacket::ObjectRangeAttack { .. }
                | ServerPacket::ObjectMagic { .. }
        )
    })
}

fn assert_owner_correction(out: &[ZoneOutbound], position: Point) {
    let mut corrections = 0;
    for outbound in out {
        for packet in packets(std::slice::from_ref(outbound)) {
            if let ServerPacket::UserLocation { location } = packet {
                assert!(
                    matches!(outbound, ZoneOutbound::ToSession { session_id, .. }
                    if *session_id == owner()),
                    "correction must go only to this owner"
                );
                assert_eq!(location.position, position);
                corrections += 1;
            }
        }
    }
    assert_eq!(
        corrections, 1,
        "combat intent reconciles pending movement once"
    );
}

#[test]
fn movement_locked_melee_and_range_intents_cancel_prior_run_without_admitting_combat() {
    for ranged_attack in [false, true] {
        for materialized in [false, true] {
            let class = if ranged_attack {
                MirClass::Archer
            } else {
                MirClass::Warrior
            };
            let mut zone = fixture(class);
            let target = Point { x: 12, y: 10 };
            if !materialized {
                spawn(&mut zone, target.clone());
            }
            let before_vitals = zone.player_vitals(&owner());
            let deadline = buffer_run(&mut zone);
            let remaining = zone.player_magic_cooldown_remaining_ms(&owner(), Spell::FireBall, 120);
            let out = zone.handle(if ranged_attack {
                ranged(120, materialized, target)
            } else {
                melee(120, materialized, target)
            });
            assert!(
                !has_action(&out),
                "the original movement lock still rejects combat"
            );
            assert_owner_correction(&out, Point { x: 11, y: 10 });
            assert_eq!(zone.player_vitals(&owner()), before_vitals);
            assert_eq!(
                zone.player_magic_cooldown_remaining_ms(&owner(), Spell::FireBall, 120),
                remaining
            );
            assert_eq!(
                zone.native_monster_count(),
                usize::from(!materialized),
                "rejection must not materialize a target"
            );
            assert_eq!(zone.next_pending_movement_deadline_ms(), None);
            assert!(zone.tick_pending_movement(deadline + 5000).is_empty());
            assert_eq!(zone.player_position(&owner()), Some(Point { x: 11, y: 10 }));
        }
    }
}

#[test]
fn fresh_chase_after_rejected_attack_retains_original_movement_deadline_and_sequence_watermark() {
    let mut zone = fixture(MirClass::Warrior);
    spawn(&mut zone, Point { x: 12, y: 10 });
    let deadline = buffer_run(&mut zone);
    zone.handle(melee(120, false, Point { x: 12, y: 10 }));
    assert_eq!(zone.player_last_seen_move_seq(&owner()), Some(2));
    zone.handle(ZoneCommand::Run {
        session_id: owner(),
        direction: MirDirection::Up,
        seq: 2,
        now_ms: 121,
    });
    assert_eq!(
        zone.next_pending_movement_deadline_ms(),
        None,
        "old sequence cannot resurrect the run"
    );
    zone.handle(ZoneCommand::Walk {
        session_id: owner(),
        direction: MirDirection::Down,
        seq: 3,
        now_ms: 122,
    });
    assert_eq!(zone.next_pending_movement_deadline_ms(), Some(deadline));
    assert!(zone.tick_pending_movement(deadline - 1).is_empty());
    assert_eq!(zone.player_position(&owner()), Some(Point { x: 11, y: 10 }));
    zone.tick_pending_movement(deadline);
    assert_eq!(zone.player_position(&owner()), Some(Point { x: 11, y: 11 }));
}

#[test]
fn attack_cooldown_rejection_cancels_prior_run_without_resetting_swing_deadline() {
    for materialized in [false, true] {
        let mut zone = fixture(MirClass::Warrior);
        spawn(&mut zone, Point { x: 11, y: 10 });
        assert!(has_action(&zone.handle(melee(
            100,
            materialized,
            Point { x: 11, y: 10 }
        ))));
        zone.handle(ZoneCommand::Run {
            session_id: owner(),
            direction: MirDirection::Up,
            seq: 1,
            now_ms: 101,
        });
        assert_eq!(zone.next_pending_movement_deadline_ms(), Some(650));
        let out = zone.handle(melee(650, materialized, Point { x: 11, y: 10 }));
        assert!(
            !has_action(&out),
            "movement ActionTime has elapsed but the 600ms swing deadline has not"
        );
        assert_owner_correction(&out, Point { x: 10, y: 10 });
        assert_eq!(zone.next_pending_movement_deadline_ms(), None);
        assert!(zone.tick_pending_movement(650).is_empty());
        assert!(!has_action(&zone.handle(melee(
            699,
            materialized,
            Point { x: 11, y: 10 }
        ))));
        assert!(
            has_action(&zone.handle(melee(700, materialized, Point { x: 11, y: 10 }))),
            "rejection must neither shorten nor restart the original attack cooldown"
        );
    }
}

#[test]
fn physical_cast_cancels_prior_run_and_preserves_movement_and_spell_admission() {
    for with_item in [false, true] {
        let mut zone = fixture(MirClass::Wizard);
        spawn(&mut zone, Point { x: 12, y: 10 });
        let deadline = buffer_run(&mut zone);
        let before = zone.player_vitals(&owner());
        let out = zone.handle(magic(120, true, with_item));
        assert!(!has_action(&out));
        assert_owner_correction(&out, Point { x: 11, y: 10 });
        assert_eq!(
            zone.player_vitals(&owner()),
            before,
            "rejected cast cannot spend mana"
        );
        assert_eq!(
            zone.player_magic_cooldown_remaining_ms(&owner(), Spell::FireBall, 120),
            Some(deadline - 120)
        );
        assert_eq!(zone.next_pending_movement_deadline_ms(), None);
        assert!(zone.tick_pending_movement(deadline - 1).is_empty());
        assert!(!has_action(&zone.handle(magic(
            deadline - 1,
            true,
            with_item
        ))));
        assert!(has_action(&zone.handle(magic(deadline, true, with_item))));
        assert_eq!(
            zone.player_vitals(&owner()).unwrap().2,
            before.unwrap().2 - 10
        );
        let spell_remaining =
            zone.player_magic_cooldown_remaining_ms(&owner(), Spell::FireBall, deadline + 1);
        zone.handle(ZoneCommand::Run {
            session_id: owner(),
            direction: MirDirection::Up,
            seq: 3,
            now_ms: deadline + 1,
        });
        let rejected = zone.handle(magic(deadline + 600, true, with_item));
        assert!(
            !has_action(&rejected),
            "spell-specific cooldown still rejects after ActionTime elapses"
        );
        assert_eq!(
            zone.player_magic_cooldown_remaining_ms(&owner(), Spell::FireBall, deadline + 1),
            spell_remaining
        );
        assert_eq!(zone.next_pending_movement_deadline_ms(), None);
        assert!(zone.tick_pending_movement(deadline + 600).is_empty());
    }
}

#[test]
fn spell_toggle_preparation_and_cast_false_preserve_buffered_run() {
    for preparation in [false, true] {
        let mut zone = fixture(MirClass::Warrior);
        spawn(&mut zone, Point { x: 12, y: 10 });
        let deadline = buffer_run(&mut zone);
        zone.handle(if preparation {
            ZoneCommand::PreparePlayerFlamingSword {
                session_id: owner(),
                level: 1,
                now_ms: 120,
            }
        } else {
            magic(120, false, false)
        });
        assert_eq!(zone.next_pending_movement_deadline_ms(), Some(deadline));
        zone.tick_pending_movement(deadline);
        assert_ne!(zone.player_position(&owner()), Some(Point { x: 11, y: 10 }));
    }
}

#[test]
fn accepted_attack_preserves_new_chase_but_keeps_its_action_lock() {
    let mut zone = fixture(MirClass::Warrior);
    spawn(&mut zone, Point { x: 12, y: 10 });
    let deadline = buffer_run(&mut zone);
    let out = zone.handle(melee(deadline, false, Point { x: 12, y: 10 }));
    assert!(has_action(&out));
    assert_owner_correction(&out, Point { x: 11, y: 10 });
    assert!(zone.tick_pending_movement(deadline).is_empty());
    zone.handle(ZoneCommand::Walk {
        session_id: owner(),
        direction: MirDirection::Down,
        seq: 3,
        now_ms: deadline + 1,
    });
    assert_eq!(
        zone.next_pending_movement_deadline_ms(),
        Some(deadline + 550)
    );
    assert!(zone.tick_pending_movement(deadline + 549).is_empty());
    zone.tick_pending_movement(deadline + 550);
    assert_eq!(zone.player_position(&owner()), Some(Point { x: 11, y: 11 }));
}

#[test]
fn combat_correction_is_owner_only_even_when_no_server_movement_remains() {
    let mut zone = fixture(MirClass::Warrior);
    zone.handle(ZoneCommand::Join(join(
        SessionId::new("observer"),
        102,
        MirClass::Warrior,
        Point { x: 10, y: 11 },
    )));
    spawn(&mut zone, Point { x: 10, y: 9 });
    let mut command = melee(100, false, Point { x: 10, y: 9 });
    if let ZoneCommand::PlayerAttackObject { direction, .. } = &mut command {
        *direction = MirDirection::Up;
    }
    let out = zone.handle(command);
    assert!(has_action(&out));
    assert_owner_correction(&out, Point { x: 10, y: 10 });
    assert!(
        packets(&out).iter().any(|packet| matches!(packet,
        ServerPacket::UserLocation { location } if location.direction == MirDirection::Up)),
        "the correction uses the actual facing after the admitted action"
    );
    assert_eq!(
        zone.player_position(&SessionId::new("observer")),
        Some(Point { x: 10, y: 11 })
    );
    let absent = zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("absent"),
        object_id: 2001,
        direction: MirDirection::Right,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 10,
        now_ms: 101,
    });
    assert!(
        absent.is_empty(),
        "a missing transform cannot produce a fabricated correction"
    );
}

#[test]
fn cancelling_owner_movement_does_not_prevent_another_actors_incoming_damage() {
    let mut zone = fixture(MirClass::Warrior);
    let opponent = SessionId::new("opponent");
    zone.handle(ZoneCommand::Join(join(
        opponent.clone(),
        102,
        MirClass::Warrior,
        Point { x: 11, y: 11 },
    )));
    zone.handle(ZoneCommand::sync_player_combat_state(
        opponent.clone(),
        MirClass::Warrior,
        true,
        false,
        true,
        false,
        false,
        false,
    ));
    spawn(&mut zone, Point { x: 12, y: 10 });
    let deadline = buffer_run(&mut zone);
    zone.handle(melee(120, false, Point { x: 12, y: 10 }));
    let before = zone.player_vitals(&owner()).unwrap().0;
    let incoming = zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: opponent,
        object_id: 101,
        direction: MirDirection::Up,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 10,
        now_ms: 121,
    });
    assert!(has_action(&incoming));
    assert!(
        zone.player_vitals(&owner()).unwrap().0 < before,
        "rejected own combat cannot grant immunity to another actor's accepted hit"
    );
    assert!(zone.tick_pending_movement(deadline).is_empty());
}
