use super::*;
fn controller_snapshot() -> CombatSnapshot {
    let player = actor("3", EntityKind::SelfPlayer, 10, 10);
    let target = actor("4", EntityKind::Monster, 11, 10);
    CombatSnapshot {
        identity: CombatIdentity {
            controller_run: 100,
            connection_generation: 1,
            session_generation: 2,
            owner_revision: 0,
            player_object_id: 3,
        },
        revision: 1,
        model_revision: 1,
        map: "D001".into(),
        enabled: true,
        now_ms: 10,
        player: player.clone(),
        hp: 10,
        max_hp: 10,
        mp: 10,
        level: 1,
        attack_speed: 0,
        learned: vec![
            serde_json::json!({"key":"fireball","spell":"FireBall","hotkey":1,"castKind":"target","mpCost":5}),
        ],
        actors: vec![player, target],
        timing: vec![],
        click_targets: vec![CrystalWorldClickTarget {
            kind: EntityKind::Monster,
            object_id: 4,
            x: 11,
            y: 10,
            dead: Some(false),
            ai: Some(0),
            harvestable: Some(false),
        }],
        selected_object_id: Some(4),
        hovered_object_id: Some("4".into()),
        cursor: Some((12, 10)),
        class: Some("Warrior".into()),
        has_class_weapon: Some(true),
        riding_mount: Some(false),
        fishing: Some(false),
        dazed: Some(false),
        motion_remaining_ms: 0,
        movement_ready: true,
        planning_position: None,
        neighbours: vec![ReachableNeighbour {
            x: 11,
            y: 9,
            cost: 1,
        }],
        spell_lock_key: "None".into(),
        pointer_spell_lock: false,
        bindings: None,
    }
}
#[test]
fn controller_delay_metadata_before_cast_and_repeated_delay_do_not_start_clock() {
    let mut s = controller_snapshot();
    s.learned[0]["delayMs"] = serde_json::json!(200);
    s.timing = vec![CombatTiming {
        spell: "FireBall".into(),
        sequence: 0,
        observed_at_ms: 10,
        delay_ms: Some(2200),
    }];
    let mut c = CombatController::default();
    assert!(c.ingest(s.clone()));
    assert!(matches!(
        only_proof(
            c.edge(
                controller_edge(
                    &s,
                    1,
                    CombatAction::Slot {
                        slot: 1,
                        cursor: None
                    }
                ),
                10
            )
            .1
        )
        .command,
        CombatCommand::Magic { .. }
    ));
    s.revision = 2;
    s.now_ms = 100;
    s.timing[0].sequence = 1;
    s.timing[0].observed_at_ms = 100;
    assert!(c.ingest(s.clone()));
    assert!(c
        .edge(
            controller_edge(
                &s,
                2,
                CombatAction::Slot {
                    slot: 1,
                    cursor: None
                }
            ),
            300
        )
        .1
        .is_empty());
    s.revision = 3;
    s.now_ms = 500;
    assert!(c.ingest(s.clone()));
    assert!(c
        .edge(
            controller_edge(
                &s,
                3,
                CombatAction::Slot {
                    slot: 1,
                    cursor: None
                }
            ),
            2299
        )
        .1
        .is_empty());
    assert!(matches!(
        only_proof(
            c.edge(
                controller_edge(
                    &s,
                    4,
                    CombatAction::Slot {
                        slot: 1,
                        cursor: None
                    }
                ),
                2300
            )
            .1
        )
        .command,
        CombatCommand::Magic { .. }
    ));
}
fn controller_edge(s: &CombatSnapshot, sequence: u64, action: CombatAction) -> CombatEdge {
    CombatEdge {
        identity: s.identity,
        model_revision: s.model_revision,
        map: s.map.clone(),
        sequence,
        action,
    }
}
fn only_proof(outputs: Vec<CombatOutput>) -> CombatProof {
    assert_eq!(outputs.len(), 1);
    match outputs.into_iter().next().unwrap() {
        CombatOutput::Wire { proof } => proof,
        _ => panic!("expected actual wire"),
    }
}
#[test]
fn controller_same_select_preserves_cadence_and_move_request_gate() {
    let s = controller_snapshot();
    let mut c = CombatController::default();
    assert!(c.ingest(s.clone()));
    let first = only_proof(
        c.edge(
            controller_edge(&s, 1, CombatAction::Select { object_id: 4 }),
            10,
        )
        .1,
    );
    assert!(c
        .edge(
            controller_edge(&s, 2, CombatAction::Select { object_id: 4 }),
            11
        )
        .1
        .is_empty());
    let deadline = 10 + attack_request_interval_ms(1, 0);
    assert!(c.tick(deadline - 1, 0).is_empty());
    let second = only_proof(c.tick(deadline, 0));
    assert!(second.sequence > first.sequence);
    let mut s = s;
    s.revision += 1;
    s.model_revision += 1;
    s.now_ms = deadline;
    s.motion_remaining_ms = 20;
    s.movement_ready = false;
    assert!(c.ingest(s));
    assert!(c
        .tick(deadline + attack_request_interval_ms(1, 0) - 1, 0)
        .is_empty());
    // An in-range movement ACK flag alone is not a request cadence gate.
    let mut s = controller_snapshot();
    s.identity.controller_run = 101;
    s.movement_ready = false;
    assert!(c.ingest(s.clone()));
    assert!(matches!(
        only_proof(
            c.edge(
                controller_edge(&s, 1, CombatAction::Select { object_id: 4 }),
                deadline + attack_request_interval_ms(1, 0)
            )
            .1
        )
        .command,
        CombatCommand::Attack { .. }
    ));
}
#[test]
fn controller_actual_repeat_edge_is_consumed_without_cast_and_bad_binding_cannot_replay() {
    let mut s = controller_snapshot();
    let mut c = CombatController::default();
    assert!(c.ingest(s.clone()));
    let action = CombatAction::Key {
        key: "F1".into(),
        modifiers: CombatModifiers::default(),
        repeat: true,
    };
    let (handled, out) = c.edge(controller_edge(&s, 1, action), 10);
    assert!(handled);
    assert!(out.is_empty());
    let proof = only_proof(
        c.edge(
            controller_edge(
                &s,
                2,
                CombatAction::Key {
                    key: "F1".into(),
                    modifiers: CombatModifiers::default(),
                    repeat: false,
                },
            ),
            11,
        )
        .1,
    );
    assert_eq!(proof.skill_id, Some(0));
    s.revision = 2;
    s.model_revision = 2;
    s.now_ms = 12;
    s.learned.insert(
        0,
        serde_json::json!({"spell":"Healing","hotkey":2,"castKind":"target"}),
    );
    assert!(c.ingest(s.clone()));
    assert!(c
        .edge(
            CombatEdge {
                model_revision: 1,
                ..controller_edge(
                    &s,
                    3,
                    CombatAction::Slot {
                        slot: 1,
                        cursor: None
                    }
                )
            },
            12
        )
        .1
        .is_empty());
    s.revision = 3;
    s.model_revision = 3;
    s.learned.clear();
    assert!(c.ingest(s.clone()));
    assert!(c
        .edge(
            controller_edge(
                &s,
                4,
                CombatAction::Slot {
                    slot: 1,
                    cursor: None
                }
            ),
            13
        )
        .1
        .is_empty());
}
#[test]
fn controller_reachable_neighbour_repath_removal_manual_cancel_are_actual_outputs() {
    let mut s = controller_snapshot();
    s.actors[1].x = 15;
    s.click_targets[0].x = 15;
    s.neighbours = vec![
        ReachableNeighbour {
            x: 15,
            y: 9,
            cost: 4,
        },
        ReachableNeighbour {
            x: 14,
            y: 10,
            cost: 3,
        },
        ReachableNeighbour {
            x: 15,
            y: 10,
            cost: 0,
        },
    ];
    let mut c = CombatController::default();
    assert!(c.ingest(s.clone()));
    let first = c
        .edge(
            controller_edge(&s, 1, CombatAction::Select { object_id: 4 }),
            10,
        )
        .1;
    assert!(matches!(
        first.as_slice(),
        [CombatOutput::Approach { x: 14, y: 10, .. }]
    ));
    assert!(c.tick(100, 0).is_empty());
    s.revision = 2;
    s.model_revision = 2;
    s.now_ms = 100;
    s.actors[1].y = 11;
    s.click_targets[0].y = 11;
    s.neighbours = vec![ReachableNeighbour {
        x: 14,
        y: 11,
        cost: 4,
    }];
    assert!(c.ingest(s.clone()));
    assert!(matches!(
        c.tick(100, 0).as_slice(),
        [CombatOutput::Approach { x: 14, y: 11, .. }]
    ));
    assert!(matches!(
        c.edge(controller_edge(&s, 2, CombatAction::Cancel), 101)
            .1
            .as_slice(),
        [CombatOutput::Clear]
    ));
    assert!(c.tick(2000, 0).is_empty());
    let mut c = CombatController::default();
    assert!(c.ingest(s.clone()));
    c.edge(
        controller_edge(&s, 1, CombatAction::Select { object_id: 4 }),
        100,
    );
    s.revision = 3;
    s.now_ms = 101;
    s.actors.pop();
    assert!(c.ingest(s));
    assert!(matches!(c.tick(101, 0).as_slice(), [CombatOutput::Clear]));
}
#[test]
fn controller_archer_required_flags_range_and_native_attack_fallback() {
    for (mount, fishing, dazed) in [
        (Some(true), Some(false), Some(false)),
        (Some(false), Some(true), Some(false)),
        (None, Some(false), Some(false)),
        (Some(false), None, Some(false)),
        (Some(false), Some(false), None),
        (Some(false), Some(false), Some(true)),
    ] {
        let mut s = controller_snapshot();
        s.class = Some("Archer".into());
        s.riding_mount = mount;
        s.fishing = fishing;
        s.dazed = dazed;
        let mut c = CombatController::default();
        assert!(c.ingest(s.clone()));
        let command = only_proof(
            c.edge(
                controller_edge(&s, 1, CombatAction::Select { object_id: 4 }),
                10,
            )
            .1,
        )
        .command;
        if mount == Some(false) && fishing == Some(false) {
            assert!(matches!(
                command,
                CombatCommand::RangeAttack { target_id: 4, .. }
            ));
        } else {
            assert!(matches!(command, CombatCommand::Attack { object_id: 4 }));
        }
        s.actors[1].x = 12;
        s.click_targets[0].x = 12;
        s.revision = 2;
        s.model_revision = 2;
        let mut c = CombatController::default();
        assert!(c.ingest(s.clone()));
        let outputs = c
            .edge(
                controller_edge(&s, 1, CombatAction::Select { object_id: 4 }),
                10,
            )
            .1;
        if mount == Some(false) && fishing == Some(false) {
            assert!(matches!(
                only_proof(outputs).command,
                CombatCommand::RangeAttack { target_id: 4, .. }
            ));
        } else {
            assert!(outputs.is_empty());
        }
    }
    let mut s = controller_snapshot();
    s.class = Some("Archer".into());
    s.actors[1].x = 19;
    s.click_targets[0].x = 19;
    let mut c = CombatController::default();
    assert!(c.ingest(s.clone()));
    assert!(matches!(
        only_proof(
            c.edge(
                controller_edge(&s, 1, CombatAction::Select { object_id: 4 }),
                10
            )
            .1
        )
        .command,
        CombatCommand::RangeAttack { target_id: 4, .. }
    ));
    s.actors[1].x = 20;
    s.click_targets[0].x = 20;
    let mut c = CombatController::default();
    assert!(c.ingest(s.clone()));
    assert!(c
        .edge(
            controller_edge(&s, 1, CombatAction::Select { object_id: 4 }),
            10
        )
        .1
        .is_empty());
}
#[test]
fn controller_clear_failed_edges_and_retired_run_never_reuse_request_ids() {
    let mut s = controller_snapshot();
    let mut c = CombatController::default();
    assert!(c.ingest(s.clone()));
    let first = only_proof(
        c.edge(
            controller_edge(
                &s,
                1,
                CombatAction::Slot {
                    slot: 1,
                    cursor: None,
                },
            ),
            10,
        )
        .1,
    );
    c.clear();
    s.revision = 2;
    s.model_revision = 2;
    assert!(c.ingest(s.clone()));
    assert!(c
        .edge(
            controller_edge(
                &s,
                1,
                CombatAction::Slot {
                    slot: 1,
                    cursor: None
                }
            ),
            11
        )
        .1
        .is_empty());
    assert!(c
        .edge(
            controller_edge(
                &s,
                2,
                CombatAction::Slot {
                    slot: 2,
                    cursor: None
                }
            ),
            11
        )
        .1
        .is_empty());
    let next = only_proof(
        c.edge(
            controller_edge(
                &s,
                3,
                CombatAction::Slot {
                    slot: 1,
                    cursor: None,
                },
            ),
            12,
        )
        .1,
    );
    assert!(next.sequence > first.sequence + 1);
    let old = s.clone();
    s.identity.controller_run += 1;
    s.revision = 1;
    assert!(c.ingest(s));
    assert!(!c.ingest(old));
}
#[test]
fn controller_bad_current_snapshot_stale_owner_and_monotonic_clock() {
    let mut s = controller_snapshot();
    let mut c = CombatController::default();
    assert!(c.ingest(s.clone()));
    let mut stale = s.clone();
    stale.identity.controller_run -= 1;
    stale.learned = vec![serde_json::Value::Null];
    assert!(!c.ingest(stale));
    assert!(c.snapshot.is_some());
    s.revision = 2;
    s.learned = vec![serde_json::Value::Null];
    assert!(!c.ingest(s.clone()));
    assert!(c.snapshot.is_none());
    assert!(c
        .edge(
            controller_edge(
                &s,
                1,
                CombatAction::Slot {
                    slot: 1,
                    cursor: None
                }
            ),
            10
        )
        .1
        .is_empty());
    s.learned = controller_snapshot().learned;
    s.revision = 3;
    assert!(c.ingest(s));
    assert!(c.tick(9, 0).is_empty());
    assert!(c.snapshot.is_none());
}
#[test]
fn controller_packet_cooldown_key16_toggle_reorder_and_unreachable_cancel() {
    let mut s = controller_snapshot();
    s.learned.push(serde_json::json!({"id":90,"spell":"HalfMoon","hotkey":16,"castKind":"toggle","canUse":true}));
    let mut c = CombatController::default();
    assert!(c.ingest(s.clone()));
    assert!(matches!(
        only_proof(
            c.edge(
                controller_edge(
                    &s,
                    1,
                    CombatAction::Slot {
                        slot: 16,
                        cursor: None
                    }
                ),
                10
            )
            .1
        )
        .command,
        CombatCommand::SpellToggle {
            toggle_state: 0,
            ..
        }
    ));
    s.timing = vec![CombatTiming {
        spell: "FireBall".into(),
        sequence: 1,
        observed_at_ms: 10,
        delay_ms: Some(1000),
    }];
    s.revision = 2;
    assert!(c.ingest(s.clone()));
    assert!(c
        .edge(
            controller_edge(
                &s,
                2,
                CombatAction::Slot {
                    slot: 1,
                    cursor: None
                }
            ),
            100
        )
        .1
        .is_empty());
    s.revision = 3;
    s.now_ms = 200;
    s.learned.reverse();
    assert!(c.ingest(s.clone()));
    assert!(c
        .edge(
            controller_edge(
                &s,
                3,
                CombatAction::Slot {
                    slot: 1,
                    cursor: None
                }
            ),
            500
        )
        .1
        .is_empty());
    assert!(matches!(
        only_proof(
            c.edge(
                controller_edge(
                    &s,
                    4,
                    CombatAction::Slot {
                        slot: 1,
                        cursor: None
                    }
                ),
                1010
            )
            .1
        )
        .command,
        CombatCommand::Magic { .. }
    ));
    let mut s = controller_snapshot();
    s.actors[1].x = 15;
    s.click_targets[0].x = 15;
    s.neighbours.clear();
    let mut c = CombatController::default();
    assert!(c.ingest(s.clone()));
    assert!(matches!(
        c.edge(
            controller_edge(&s, 1, CombatAction::Select { object_id: 4 }),
            10
        )
        .1
        .as_slice(),
        [CombatOutput::Clear]
    ));
    assert!(c.tick(2000, 0).is_empty());
}
fn actor(id: &str, kind: EntityKind, x: i32, y: i32) -> CombatActor {
    CombatActor {
        object_id: id.into(),
        kind,
        x,
        y,
        direction: Some("up".into()),
        dead: false,
        master_object_id: 0,
        ai: 0,
    }
}
fn skills() -> SkillModel {
    serde_json::from_value(serde_json::json!({"skills":[{"id":10,"spell":"FireBall","name":"火球","hotkey":1,"castKind":"target","mpCost":5},{"id":20,"spell":"Healing","hotkey":16,"castKind":"target","mpCost":0}]})).unwrap()
}
#[test]
fn explicit_two_bank_bindings_and_unknown_metadata_match_native() {
    let s = skills();
    assert_eq!(
        skill_ready(&s, 1, 10, 5, |_| 0).unwrap().spell.as_deref(),
        Some("FireBall")
    );
    assert_eq!(skill_ready(&s, 16, 10, 0, |_| 0).unwrap().skill_id, 20);
    assert!(skill_ready(&s, 2, 10, 99, |_| 0).is_none());
    assert!(skill_ready(&s, 1, 0, 99, |_| 0).is_none());
    assert!(skill_ready(&s, 1, 10, 4, |_| 0).is_none());
    assert!(skill_ready(&s, 1, 10, 99, |_| 1).is_none());
    let name_only: SkillModel = serde_json::from_value(
        serde_json::json!({"skills":[{"id":1,"name":"FireBall","hotkey":1,"castKind":"target"}]}),
    )
    .unwrap();
    assert!(skill_ready(&name_only, 1, 10, 99, |_| 0).is_none());
    let mut s = s;
    s.bindings[0].cast_kind = Some("future".into());
    assert!(skill_ready(&s, 1, 10, 99, |_| 0).is_none());
    s.bindings[0].cast_kind = Some("passive".into());
    assert!(skill_ready(&s, 1, 10, 99, |_| 0).is_none());
    s.bindings[0].cast_kind = Some("target".into());
    s.bindings[0].cooldown_remaining_ticks = 1;
    assert!(skill_ready(&s, 1, 10, 99, |_| 0).is_none());
    assert_eq!(next_toggle_state(Some(true)), 0);
    assert_eq!(next_toggle_state(Some(false)), 1);
    assert_eq!(next_toggle_state(None), 1);
}
#[test]
fn default_keys_and_custom_modifier_rules_are_common_data() {
    let b = default_skill_bindings();
    assert_eq!(b.len(), 16);
    assert_eq!(
        matching_skill_slots(&b, "F1", CombatModifiers::default()),
        vec![1]
    );
    assert_eq!(
        matching_skill_slots(
            &b,
            "F8",
            CombatModifiers {
                ctrl: true,
                ..Default::default()
            }
        ),
        vec![16]
    );
    assert!(matching_skill_slots(
        &b,
        "F1",
        CombatModifiers {
            shift: true,
            ..Default::default()
        }
    )
    .is_empty());
    let mut custom = b[0].clone();
    custom.key = "K".into();
    custom.ctrl = 2;
    assert_eq!(
        matching_skill_slots(
            &[custom],
            "K",
            CombatModifiers {
                ctrl: true,
                ..Default::default()
            }
        ),
        vec![1]
    );
}

#[test]
fn custom_skill_insert_preserves_native_ctrl_insert_exclusion() {
    let mut b = default_skill_bindings()[0].clone();
    b.key = "Insert".into();
    b.ctrl = 1;
    b.alt = 2;
    b.shift = 2;
    assert!(matching_skill_slots(
        &[b.clone()],
        "Insert",
        CombatModifiers {
            ctrl: true,
            ..Default::default()
        }
    )
    .is_empty());
    assert_eq!(
        matching_skill_slots(
            &[b.clone()],
            "Insert",
            CombatModifiers {
                ctrl: true,
                alt: true,
                ..Default::default()
            }
        ),
        vec![1]
    );
    assert_eq!(
        matching_skill_slots(
            &[b.clone()],
            "Insert",
            CombatModifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            }
        ),
        vec![1]
    );
    b.ctrl = 0;
    assert_eq!(
        matching_skill_slots(&[b], "Insert", CombatModifiers::default()),
        vec![1]
    );
}
#[test]
fn hover_memory_heal_pet_reincarnation_ground_and_flashdash() {
    let player = actor("1", EntityKind::SelfPlayer, 10, 10);
    let mut monster = actor("2", EntityKind::Monster, 12, 11);
    let mut memory = SpellTargetMemory::default();
    let cursor = Some((20, 22));
    assert_eq!(
        memory
            .aim(
                "FireBall",
                &player,
                &[player.clone(), monster.clone()],
                None,
                Some("2"),
                cursor
            )
            .unwrap()
            .target_id,
        2
    );
    assert_eq!(
        memory
            .aim("FireBall", &player, &[monster.clone()], None, None, cursor)
            .unwrap()
            .target_id,
        2
    );
    assert_eq!(
        memory
            .aim(
                "Healing",
                &player,
                &[monster.clone()],
                None,
                Some("2"),
                cursor
            )
            .unwrap()
            .target_id,
        1
    );
    monster.master_object_id = 1;
    assert_eq!(
        memory
            .aim(
                "Healing",
                &player,
                &[monster.clone()],
                None,
                Some("2"),
                cursor
            )
            .unwrap()
            .target_id,
        2
    );
    monster.kind = EntityKind::Player;
    monster.dead = true;
    assert_eq!(
        memory
            .aim(
                "Reincarnation",
                &player,
                &[monster.clone()],
                None,
                Some("2"),
                cursor
            )
            .unwrap()
            .target_id,
        2
    );
    assert_eq!(
        memory
            .aim(
                "FireWall",
                &player,
                &[monster.clone()],
                None,
                Some("2"),
                cursor
            )
            .unwrap()
            .location,
        (20, 22)
    );
    assert_eq!(
        memory
            .aim("FlashDash", &player, &[], None, None, cursor)
            .unwrap()
            .direction,
        "up"
    );
    memory.session(2);
    assert!(memory.magic.is_none());
    let changed = actor("3", EntityKind::SelfPlayer, 10, 10);
    assert_eq!(
        memory
            .aim("FireBall", &changed, &[], None, None, cursor)
            .unwrap()
            .target_id,
        0
    );
}
#[test]
fn selected_immune_monster_cannot_become_magic_memory() {
    let p = actor("1", EntityKind::SelfPlayer, 10, 10);
    for ai in [6, 64, 70] {
        let mut m = actor("2", EntityKind::Monster, 11, 10);
        m.ai = ai;
        let mut state = SpellTargetMemory::default();
        assert_eq!(
            state
                .aim("FireBall", &p, &[m], Some(2), None, Some((11, 10)))
                .unwrap()
                .target_id,
            0
        );
    }
}
#[test]
fn chase_pacing_preserves_delayed_ack_archer_and_neighbours() {
    assert_eq!(attack_request_interval_ms(1, 0), 1386);
    assert_eq!(attack_request_interval_ms(40, 99), 550);
    assert_eq!(
        chase_decision(true, 1, false, false, true, false, true),
        ChaseDecision::Attack
    );
    assert_eq!(
        chase_decision(true, 3, false, false, false, true, false),
        ChaseDecision::Approach
    );
    assert_eq!(
        chase_decision(true, 10, true, true, true, true, false),
        ChaseDecision::Wait
    );
    assert_eq!(
        chase_decision(true, 9, true, true, true, false, false),
        ChaseDecision::Attack
    );
    assert_eq!(
        chase_decision(false, 1, false, false, true, true, false),
        ChaseDecision::Clear
    );
    let neighbours = attack_neighbours((20, 20));
    assert_eq!(neighbours.len(), 8);
    assert!(!neighbours.contains(&(20, 20)));
    assert!(attack_neighbours((i32::MAX, i32::MAX))
        .iter()
        .all(|&(x, y)| x <= i32::MAX && y <= i32::MAX));
}
#[test]
fn world_click_requires_native_archer_flags_and_alt_precedes_shift() {
    let mut c = CrystalWorldClickContext {
        in_game: true,
        world_actions_blocked: false,
        player_hp: Some(10),
        player_max_hp: Some(10),
        player_x: 10,
        player_y: 10,
        target: Some(CrystalWorldClickTarget {
            kind: EntityKind::Monster,
            object_id: 2,
            x: 12,
            y: 10,
            dead: Some(false),
            ai: Some(0),
            harvestable: Some(false),
        }),
        alt: false,
        shift: false,
        class: Some("Archer".into()),
        has_class_weapon: Some(true),
        riding_mount: Some(false),
        dazed: Some(false),
        fishing: Some(false),
        target_in_range: Some(true),
    };
    assert!(matches!(
        resolve_world_click(&c),
        Some(CombatCommand::RangeAttack { target_id: 2, .. })
    ));
    for dazed in [None, Some(true)] {
        c.dazed = dazed;
        assert!(matches!(
            resolve_world_click(&c),
            Some(CombatCommand::RangeAttack { target_id: 2, .. })
        ));
        c.shift = true;
        assert!(resolve_world_click(&c).is_none());
        c.shift = false;
    }
    c.dazed = Some(false);
    c.fishing = None;
    assert!(resolve_world_click(&c).is_none());
    c.shift = true;
    assert!(matches!(
        resolve_world_click(&c),
        Some(CombatCommand::RangeAttack { .. })
    ));
    c.alt = true;
    assert!(matches!(
        resolve_world_click(&c),
        Some(CombatCommand::Harvest { .. })
    ));
    c.riding_mount = None;
    assert!(resolve_world_click(&c).is_none());
}

#[test]
fn controller_skill_key_requires_unique_exact_some_key_not_name_or_ordinal() {
    let s = controller_snapshot();
    let mut c = CombatController::default();
    assert!(c.ingest(s.clone()));
    let proof = only_proof(
        c.edge(
            controller_edge(
                &s,
                1,
                CombatAction::SkillKey {
                    skill_key: "fireball".into(),
                },
            ),
            10,
        )
        .1,
    );
    assert_eq!(proof.spell.as_deref(), Some("FireBall"));
    assert_eq!(proof.slot, Some(1));
    for (rows, requested) in [
        (s.learned.clone(), "FireBall"),
        (s.learned.clone(), "0"),
        (
            vec![
                serde_json::json!({"key":null,"magicName":"fireball","spell":"FireBall","hotkey":1,"castKind":"target","mpCost":5}),
            ],
            "fireball",
        ),
        (
            vec![serde_json::json!({"spell":"FireBall","hotkey":1,"castKind":"target","mpCost":5})],
            "fireball",
        ),
        (
            vec![
                s.learned[0].clone(),
                serde_json::json!({"key":"fireball","spell":"Healing","hotkey":2,"castKind":"target","mpCost":5}),
            ],
            "fireball",
        ),
    ] {
        let mut s = controller_snapshot();
        s.learned = rows;
        let mut c = CombatController::default();
        assert!(c.ingest(s.clone()));
        assert!(
            c.edge(
                controller_edge(
                    &s,
                    1,
                    CombatAction::SkillKey {
                        skill_key: requested.into()
                    }
                ),
                10
            )
            .1
            .is_empty(),
            "unexpected key match: {requested}"
        );
    }
}
