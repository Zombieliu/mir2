//! Consumption-time Crystal CanMove, Fishing and GetDelayTime(TurnDelay).
//! Fixtures use a private explicit arena, real Zone commands and finite poison
//! application; no QA movement-delay override or replay identity is installed.
use super::*;
use mir2_protocol::{MirClass, MirGender};

const OWNER_ID: u32 = 10_101;
const OBSERVER_ID: u32 = 10_102;
const ARENA: &str = "source-can-move-arena";

fn fixture_at(map: &str, position: Point) -> (ZoneRuntime, SessionId, SessionId) {
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map(map), ZoneCollision::unbounded());
    let owner = SessionId::new("source-can-move-owner");
    let observer = SessionId::new("source-can-move-observer");
    for (session_id, object_id, point) in [
        (owner.clone(), OWNER_ID, position.clone()),
        (
            observer.clone(),
            OBSERVER_ID,
            Point {
                x: position.x + 3,
                y: position.y,
            },
        ),
    ] {
        zone.handle(ZoneCommand::Join(ZoneJoin {
            account_id: format!("{}-account", session_id.as_str()),
            session_id,
            character_index: object_id as i32,
            object_id,
            name: format!("SourceCanMove{object_id}"),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 7,
            hp: 100,
            max_hp: 100,
            mp: 100,
            map_file_name: map.into(),
            position: point,
            direction: MirDirection::Right,
            chat_profile: Default::default(),
            combat_stats: Default::default(),
        }));
    }
    (zone, owner, observer)
}

fn fixture() -> (ZoneRuntime, SessionId, SessionId) {
    fixture_at(ARENA, Point { x: 10, y: 10 })
}

fn action(
    zone: &mut ZoneRuntime,
    owner: &SessionId,
    kind: ZoneMovementActionKind,
    direction: MirDirection,
    seq: u64,
    now_ms: u64,
) -> Vec<ZoneOutbound> {
    zone.handle(match kind {
        ZoneMovementActionKind::Turn => ZoneCommand::Turn {
            session_id: owner.clone(),
            direction,
            now_ms,
        },
        ZoneMovementActionKind::Walk => ZoneCommand::Walk {
            session_id: owner.clone(),
            direction,
            seq,
            now_ms,
        },
        ZoneMovementActionKind::Run => ZoneCommand::Run {
            session_id: owner.clone(),
            direction,
            seq,
            now_ms,
        },
    })
}

fn movement_tick(
    zone: &mut ZoneRuntime,
    owner: &SessionId,
    now_ms: u64,
) -> Vec<ZoneOutbound> {
    zone.handle(ZoneCommand::TickPlayerMovement {
        session_id: owner.clone(),
        now_ms,
    })
}

fn has_packet(
    out: &[ZoneOutbound],
    recipient: &SessionId,
    predicate: impl Fn(&ServerPacket) -> bool,
) -> bool {
    out.iter().any(|item| match item {
        ZoneOutbound::ToSession {
            session_id,
            packets,
        } if session_id == recipient => packets.iter().any(|packet| predicate(packet)),
        ZoneOutbound::ToMany {
            session_ids,
            packets,
        } if session_ids.contains(recipient) => packets.iter().any(|packet| predicate(packet)),
        ZoneOutbound::ToAll { packets } => packets.iter().any(|packet| predicate(packet)),
        _ => false,
    })
}

fn has_save(out: &[ZoneOutbound], owner: &SessionId) -> bool {
    out.iter().any(|item| {
        matches!(item, ZoneOutbound::SaveTransform { session_id, .. } if session_id == owner)
    })
}

fn assert_correction_only(out: &[ZoneOutbound], owner: &SessionId, before: &ZonePlayer) {
    assert_eq!(out.len(), 1, "rejection must only correct its owner: {out:?}");
    let ZoneOutbound::ToSession {
        session_id,
        packets,
    } = &out[0]
    else {
        panic!("rejection must not save or broadcast a transform: {out:?}");
    };
    assert_eq!(session_id, owner);
    assert_eq!(packets.len(), 1);
    assert!(matches!(&packets[0], ServerPacket::UserLocation { location }
        if location.position == before.position && location.direction == before.direction));
}

fn prime_walk(zone: &mut ZoneRuntime, owner: &SessionId) -> u64 {
    assert!(action(
        zone,
        owner,
        ZoneMovementActionKind::Walk,
        MirDirection::Right,
        1,
        0,
    )
    .is_empty());
    assert!(has_save(&movement_tick(zone, owner, 0), owner));
    assert_eq!(zone.players[owner].position, Point { x: 11, y: 10 });
    assert_eq!(zone.players[owner].run_step_until_ms, ZONE_RUN_GRACE_MS);
    let ready = zone.players[owner].movement_ready_at_ms;
    assert!(ready > 0);
    ready
}

fn assert_retired_rejection(
    zone: &mut ZoneRuntime,
    owner: &SessionId,
    kind: ZoneMovementActionKind,
    now_ms: u64,
) {
    let before = zone.players[owner].clone();
    let occupancy = zone.occupancy.clone();
    let out = movement_tick(zone, owner, now_ms);
    assert_correction_only(&out, owner, &before);
    let after = &zone.players[owner];
    assert!(after.movement_actions.is_empty(), "ready rejected intent must retire");
    assert_eq!(after.position, before.position);
    assert_eq!(after.direction, before.direction);
    assert_eq!(after.movement_ready_at_ms, before.movement_ready_at_ms);
    assert_eq!(after.next_attack_ready_at_ms, before.next_attack_ready_at_ms);
    assert_eq!(after.next_spell_ready_at_ms, before.next_spell_ready_at_ms);
    assert_eq!(after.magic_ready_at_ms, before.magic_ready_at_ms);
    assert_eq!(
        after.run_step_until_ms,
        if kind == ZoneMovementActionKind::Turn {
            0
        } else {
            before.run_step_until_ms
        },
        "Crystal clears the step counter on consumed Turn, not rejected Walk/Run",
    );
    assert_eq!(zone.occupancy, occupancy);
    assert!(zone.ecs_mirror_matches_players());
}

fn assert_no_replay_then_fresh_turn(zone: &mut ZoneRuntime, owner: &SessionId, now_ms: u64) {
    let before = zone.players[owner].clone();
    assert!(movement_tick(zone, owner, now_ms).is_empty());
    assert!(movement_tick(zone, owner, now_ms + 1_000).is_empty());
    assert_eq!(zone.players[owner].position, before.position);
    assert_eq!(zone.players[owner].direction, before.direction);
    assert_eq!(
        zone.players[owner].movement_ready_at_ms,
        before.movement_ready_at_ms,
    );
    let fresh = action(
        zone,
        owner,
        ZoneMovementActionKind::Turn,
        MirDirection::Down,
        3,
        now_ms + 1_001,
    );
    assert!(has_save(&fresh, owner), "a fresh post-status intent must work");
    assert_eq!(zone.players[owner].direction, MirDirection::Down);
}

#[test]
fn queued_turn_walk_and_run_revalidate_new_control_poisons_then_retire() {
    for poison in [
        CRYSTAL_POISON_PARALYSIS,
        CRYSTAL_POISON_LR_PARALYSIS,
        CRYSTAL_POISON_FROZEN,
    ] {
        for kind in [
            ZoneMovementActionKind::Turn,
            ZoneMovementActionKind::Walk,
            ZoneMovementActionKind::Run,
        ] {
            for direction in [MirDirection::Right, MirDirection::Left] {
                let (mut zone, owner, _) = fixture();
                let ready = prime_walk(&mut zone, &owner);
                assert!(action(&mut zone, &owner, kind, direction, 2, ready / 4).is_empty());
                let applied_at = ready / 2;
                zone.apply_native_player_status_poison(&owner, poison, 2_000, applied_at);
                let queued = zone.players[&owner].movement_actions.clone();
                let grace = zone.players[&owner].run_step_until_ms;
                assert!(movement_tick(&mut zone, &owner, ready - 1).is_empty());
                assert_eq!(zone.players[&owner].movement_actions, queued);
                assert_eq!(zone.players[&owner].run_step_until_ms, grace);
                assert_retired_rejection(&mut zone, &owner, kind, ready);
                let expired_at = applied_at + 2_000;
                zone.expire_zone_player_status_poisons(expired_at);
                assert_no_replay_then_fresh_turn(&mut zone, &owner, expired_at);
            }
        }
    }
}

fn set_fishing(zone: &mut ZoneRuntime, owner: &SessionId, fishing: bool, packet: bool, now_ms: u64) {
    if packet {
        zone.handle(ZoneCommand::BroadcastPackets {
            session_id: owner.clone(),
            owner_local_object_id: OWNER_ID,
            packets: vec![ServerPacket::FishingUpdate {
                object_id: OWNER_ID,
                fishing,
                progress_percent: 0,
                chance_percent: 0,
                fishing_point: Point { x: 14, y: 10 },
                found_fish: false,
            }],
            now_ms,
        });
    } else {
        let mut appearance = zone.players[owner].appearance();
        appearance.fishing = fishing;
        zone.handle(ZoneCommand::SyncPlayerAppearance {
            session_id: owner.clone(),
            appearance,
        });
    }
    assert_eq!(zone.players[owner].fishing, fishing);
}

#[test]
fn queued_actions_revalidate_fishing_from_both_trusted_projections_without_replay() {
    for packet in [false, true] {
        for kind in [
            ZoneMovementActionKind::Turn,
            ZoneMovementActionKind::Walk,
            ZoneMovementActionKind::Run,
        ] {
            let (mut zone, owner, _) = fixture();
            let ready = prime_walk(&mut zone, &owner);
            assert!(action(
                &mut zone,
                &owner,
                kind,
                MirDirection::Left,
                2,
                ready / 4,
            )
            .is_empty());
            set_fishing(&mut zone, &owner, true, packet, ready / 2);
            let queued = zone.players[&owner].movement_actions.clone();
            let grace = zone.players[&owner].run_step_until_ms;
            assert!(movement_tick(&mut zone, &owner, ready - 1).is_empty());
            assert_eq!(zone.players[&owner].movement_actions, queued);
            assert_eq!(zone.players[&owner].run_step_until_ms, grace);
            assert_retired_rejection(&mut zone, &owner, kind, ready);
            set_fishing(&mut zone, &owner, false, packet, ready + 100);
            assert_no_replay_then_fresh_turn(&mut zone, &owner, ready + 100);
        }
    }
}

#[test]
fn not_ready_status_does_not_consume_or_change_the_latest_pending_step() {
    let (mut zone, owner, _) = fixture();
    let ready = prime_walk(&mut zone, &owner);
    assert!(action(
        &mut zone,
        &owner,
        ZoneMovementActionKind::Walk,
        MirDirection::Right,
        2,
        ready / 4,
    )
    .is_empty());
    assert!(action(
        &mut zone,
        &owner,
        ZoneMovementActionKind::Run,
        MirDirection::Left,
        3,
        ready / 4,
    )
    .is_empty());
    assert_eq!(zone.players[&owner].movement_actions.len(), 1);
    let latest = zone.players[&owner].movement_actions.clone();
    assert_eq!(latest[0].kind, ZoneMovementActionKind::Run);
    assert_eq!(latest[0].direction, MirDirection::Left);
    assert_eq!(latest[0].seq, Some(3));
    zone.apply_native_player_status_poison(&owner, CRYSTAL_POISON_PARALYSIS, 2_000, ready / 2);
    assert!(movement_tick(&mut zone, &owner, ready - 1).is_empty());
    assert_eq!(zone.players[&owner].movement_actions, latest);
    assert_eq!(zone.players[&owner].movement_ready_at_ms, ready);
    assert_eq!(zone.players[&owner].run_step_until_ms, ZONE_RUN_GRACE_MS);
    assert_retired_rejection(&mut zone, &owner, ZoneMovementActionKind::Run, ready);
    zone.expire_zone_player_status_poisons(ready / 2 + 2_000);
    assert_no_replay_then_fresh_turn(&mut zone, &owner, ready / 2 + 2_000);
}

#[test]
fn movement_poison_expiring_exactly_at_consume_time_uses_the_base_turn_delay() {
    for poison in [
        CRYSTAL_POISON_PARALYSIS,
        CRYSTAL_POISON_LR_PARALYSIS,
        CRYSTAL_POISON_FROZEN,
        CRYSTAL_POISON_SLOW,
    ] {
        let (mut zone, owner, observer) = fixture();
        let ready = prime_walk(&mut zone, &owner);
        assert!(action(
            &mut zone,
            &owner,
            ZoneMovementActionKind::Turn,
            MirDirection::Left,
            2,
            ready / 4,
        )
        .is_empty());
        zone.apply_native_player_status_poison(&owner, poison, ready - ready / 2, ready / 2);
        assert!(movement_tick(&mut zone, &owner, ready - 1).is_empty());
        let accepted = movement_tick(&mut zone, &owner, ready);
        assert!(has_save(&accepted, &owner));
        assert!(has_packet(&accepted, &observer, |packet| matches!(packet,
            ServerPacket::ObjectTurn { movement }
                if movement.object_id == OWNER_ID && movement.direction == MirDirection::Left)));
        assert_eq!(zone.players[&owner].movement_ready_at_ms, ready + 350);
        assert!(zone.players[&owner].movement_actions.is_empty());
    }
}

#[test]
fn queued_same_direction_turn_uses_slow_added_after_arrival() {
    let (mut zone, owner, observer) = fixture();
    let ready = prime_walk(&mut zone, &owner);
    assert!(action(
        &mut zone,
        &owner,
        ZoneMovementActionKind::Turn,
        MirDirection::Right,
        2,
        ready / 4,
    )
    .is_empty());
    zone.apply_native_player_status_poison(&owner, CRYSTAL_POISON_SLOW, 10_000, ready / 2);
    assert!(movement_tick(&mut zone, &owner, ready - 1).is_empty());
    assert_eq!(zone.players[&owner].run_step_until_ms, ZONE_RUN_GRACE_MS);
    let accepted = movement_tick(&mut zone, &owner, ready);
    assert!(has_save(&accepted, &owner), "Source admits a same-direction Turn");
    assert!(has_packet(&accepted, &observer, |packet| matches!(packet,
        ServerPacket::ObjectTurn { movement }
            if movement.object_id == OWNER_ID && movement.direction == MirDirection::Right)));
    assert_eq!(zone.players[&owner].movement_ready_at_ms, ready + 700);
    assert_eq!(zone.players[&owner].run_step_until_ms, 0);
}

#[test]
fn slow_expiring_before_consume_uses_350_without_rewriting_an_accepted_700_deadline() {
    let (mut zone, owner, _) = fixture();
    zone.apply_native_player_status_poison(&owner, CRYSTAL_POISON_SLOW, 150, 50);
    let accepted = action(
        &mut zone,
        &owner,
        ZoneMovementActionKind::Turn,
        MirDirection::Right,
        1,
        100,
    );
    assert!(has_save(&accepted, &owner));
    assert_eq!(zone.players[&owner].movement_ready_at_ms, 800);
    assert!(action(
        &mut zone,
        &owner,
        ZoneMovementActionKind::Turn,
        MirDirection::Left,
        2,
        150,
    )
    .is_empty());
    zone.expire_zone_player_status_poisons(200);
    assert_eq!(zone.players[&owner].movement_ready_at_ms, 800);
    assert!(movement_tick(&mut zone, &owner, 799).is_empty());
    let accepted = movement_tick(&mut zone, &owner, 800);
    assert!(has_save(&accepted, &owner));
    assert_eq!(zone.players[&owner].movement_ready_at_ms, 1_150);
}

#[test]
fn non_movement_poisons_and_missing_combat_admission_do_not_block_source_can_move() {
    // Literal masks also guard against importing CanAttack/CanCast's broader
    // Dazed/STUN/weapon/mount admission conditions into Source CanMove.
    for poison in [1, 2, 16, 1_024] {
        for kind in [
            ZoneMovementActionKind::Turn,
            ZoneMovementActionKind::Walk,
            ZoneMovementActionKind::Run,
        ] {
            let (mut zone, owner, observer) = fixture();
            assert!(zone.players[&owner].combat_state.is_none());
            zone.apply_native_player_status_poison(&owner, poison, 1_000, 0);
            let accepted = action(&mut zone, &owner, kind, MirDirection::Down, 1, 100);
            assert!(has_save(&accepted, &owner), "{poison}/{kind:?}: {accepted:?}");
            assert!(has_packet(&accepted, &observer, |packet| matches!(packet,
                ServerPacket::ObjectTurn { .. }
                    | ServerPacket::ObjectWalk { .. }
                    | ServerPacket::ObjectRun { .. })));
        }
    }
}

#[test]
fn dead_players_only_receive_correction_for_turn_walk_and_run_without_clock_changes() {
    for kind in [
        ZoneMovementActionKind::Turn,
        ZoneMovementActionKind::Walk,
        ZoneMovementActionKind::Run,
    ] {
        let (mut zone, owner, _) = fixture();
        zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
            session_id: owner.clone(),
            hp: 0,
            max_hp: 100,
            mp: 100,
            dead: true,
        });
        let before = zone.players[&owner].clone();
        let out = action(&mut zone, &owner, kind, MirDirection::Left, 1, 100);
        assert_correction_only(&out, &owner, &before);
        assert_eq!(zone.players[&owner].movement_ready_at_ms, before.movement_ready_at_ms);
        assert_eq!(zone.players[&owner].direction, before.direction);
        assert_eq!(zone.players[&owner].position, before.position);
        assert!(zone.players[&owner].movement_actions.is_empty());
        assert!(movement_tick(&mut zone, &owner, 1_000).is_empty());
        assert!(zone.ecs_mirror_matches_players());
    }
}

#[test]
fn accepted_same_direction_turn_still_checks_the_current_coordinate() {
    let (map, point, expected_message, expected_chat_type) =
        mir2_game_data::crystal_map_events::crystal_map_event_manifest_ref()
            .typed_map_coordinate_bindings
            .iter()
            .find_map(|binding| {
                let point = Point { x: binding.x, y: binding.y };
                match crystal_map_coordinate_decision(
                    &binding.map_id,
                    &point,
                    7,
                    0,
                    MirDirection::Right,
                ) {
                    Some(CrystalMapCoordinateDecision::Denied { message, chat_type }) => {
                        Some((binding.map_id.clone(), point, message, chat_type))
                    }
                    _ => None,
                }
            })
            .expect("retained original map-coordinate fixture must have an entry-level Hint");
    let (mut zone, owner, observer) = fixture_at(&map, point);
    let accepted = action(
        &mut zone,
        &owner,
        ZoneMovementActionKind::Turn,
        MirDirection::Right,
        1,
        100,
    );
    assert!(has_save(&accepted, &owner));
    assert!(has_packet(&accepted, &observer, |packet| matches!(packet,
        ServerPacket::ObjectTurn { movement }
            if movement.object_id == OWNER_ID && movement.direction == MirDirection::Right)));
    assert!(has_packet(&accepted, &owner, |packet| matches!(packet,
        ServerPacket::Chat { message, chat_type }
            if message == &expected_message && *chat_type == expected_chat_type)));
    assert_eq!(zone.players[&owner].movement_ready_at_ms, 450);
    let mut appearance = zone.players[&owner].appearance();
    appearance.fishing = true;
    zone.handle(ZoneCommand::SyncPlayerAppearance { session_id: owner.clone(), appearance });
    let before = zone.players[&owner].clone();
    let rejected = action(
        &mut zone,
        &owner,
        ZoneMovementActionKind::Turn,
        MirDirection::Right,
        2,
        451,
    );
    assert_correction_only(&rejected, &owner, &before);
    assert_eq!(zone.players[&owner].movement_ready_at_ms, 450);
}
