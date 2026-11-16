use super::*;
use crate::runtime::zone::collision::ZoneBounds;
use crate::runtime::zone::types::{ZoneChatProfile, ZonePlayerCombatStats};
use mir2_protocol::{MirClass, MirDirection, MirGender};

fn join_player(
    zone: &mut ZoneRuntime,
    id: &str,
    object_id: u32,
    position: Point,
    profile: ZoneChatProfile,
    stats: ZonePlayerCombatStats,
) -> SessionId {
    let session_id = SessionId::new(id);
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: session_id.clone(),
        account_id: format!("acct-{id}"),
        character_index: object_id as i32,
        object_id,
        name: id.to_owned(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 30,
        hp: 100,
        max_hp: 100,
        mp: 100,
        map_file_name: zone.key().map_file_name.clone(),
        position,
        direction: MirDirection::Up,
        chat_profile: profile,
        combat_stats: stats,
    }));
    zone.handle(ZoneCommand::sync_player_combat_state(
        session_id.clone(),
        MirClass::Warrior,
        false,
        false,
        true,
        false,
        false,
        false,
    ));
    session_id
}

fn step(
    zone: &mut ZoneRuntime,
    session_id: &SessionId,
    kind: ZoneMovementActionKind,
    direction: MirDirection,
    now_ms: u64,
) {
    match kind {
        ZoneMovementActionKind::Walk => {
            zone.handle(ZoneCommand::Walk {
                session_id: session_id.clone(),
                direction,
                seq: now_ms,
                now_ms,
            });
        }
        ZoneMovementActionKind::Run => {
            zone.handle(ZoneCommand::Run {
                session_id: session_id.clone(),
                direction,
                seq: now_ms,
                now_ms,
            });
        }
        ZoneMovementActionKind::Turn => {
            zone.handle(ZoneCommand::Turn {
                session_id: session_id.clone(),
                direction,
                now_ms,
            });
        }
    };
    zone.handle(ZoneCommand::TickPlayerMovement {
        session_id: session_id.clone(),
        now_ms,
    });
}

#[test]
fn real_map_zero_accepted_walk_enters_and_run_leaves_manifest_safe_area() {
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
    let owner = join_player(
        &mut zone,
        "owner",
        101,
        Point { x: 324, y: 275 },
        ZoneChatProfile::default(),
        Default::default(),
    );
    step(
        &mut zone,
        &owner,
        ZoneMovementActionKind::Walk,
        MirDirection::Up,
        1,
    );
    assert_eq!(zone.players[&owner].position, Point { x: 324, y: 274 });
    assert!(zone.players[&owner].chat_profile.in_safe_zone);
    step(
        &mut zone,
        &owner,
        ZoneMovementActionKind::Run,
        MirDirection::Down,
        1_000,
    );
    assert!(!zone.players[&owner].chat_profile.in_safe_zone);
}

#[test]
fn rejected_walk_and_turn_preserve_existing_safe_profile() {
    let collision = ZoneCollision::unbounded().with_bounds(ZoneBounds::new(324, 324, 274, 274));
    let mut zone = ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), collision);
    let owner = join_player(
        &mut zone,
        "owner",
        101,
        Point { x: 324, y: 274 },
        ZoneChatProfile {
            in_safe_zone: true,
            ..Default::default()
        },
        Default::default(),
    );
    step(
        &mut zone,
        &owner,
        ZoneMovementActionKind::Walk,
        MirDirection::Down,
        1,
    );
    assert_eq!(zone.players[&owner].position, Point { x: 324, y: 274 });
    assert!(zone.players[&owner].chat_profile.in_safe_zone);
    zone.players
        .get_mut(&owner)
        .unwrap()
        .chat_profile
        .in_safe_zone = false;
    step(
        &mut zone,
        &owner,
        ZoneMovementActionKind::Turn,
        MirDirection::Left,
        1_000,
    );
    assert!(!zone.players[&owner].chat_profile.in_safe_zone);
}

#[test]
fn unknown_map_keeps_trusted_safe_profile() {
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("custom"), ZoneCollision::unbounded());
    let owner = join_player(
        &mut zone,
        "owner",
        101,
        Point { x: 10, y: 10 },
        ZoneChatProfile {
            in_safe_zone: true,
            ..Default::default()
        },
        Default::default(),
    );
    step(
        &mut zone,
        &owner,
        ZoneMovementActionKind::Walk,
        MirDirection::Right,
        1,
    );
    assert!(zone.players[&owner].chat_profile.in_safe_zone);
}

#[test]
fn real_damage_admission_blocks_after_entering_safe_area_then_allows_after_leaving() {
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
    let attacker = join_player(
        &mut zone,
        "attacker",
        101,
        Point { x: 324, y: 275 },
        ZoneChatProfile {
            attack_mode: 5,
            ..Default::default()
        },
        ZonePlayerCombatStats {
            min_dc: 50,
            max_dc: 50,
            accuracy: 100,
            ..Default::default()
        },
    );
    let target = join_player(
        &mut zone,
        "target",
        102,
        Point { x: 323, y: 275 },
        ZoneChatProfile::default(),
        Default::default(),
    );
    step(
        &mut zone,
        &attacker,
        ZoneMovementActionKind::Walk,
        MirDirection::Up,
        1,
    );
    let blocked =
        zone.player_attack_native_object(&attacker, 102, MirDirection::UpLeft, 0, 0, 0, 50, 1_000);
    assert_eq!(zone.players[&target].hp, 100);
    assert!(!blocked.iter().any(|outbound| matches!(outbound, ZoneOutbound::PlayerDamaged { session_id, .. } if session_id == &target)));
    step(
        &mut zone,
        &attacker,
        ZoneMovementActionKind::Walk,
        MirDirection::Down,
        2_000,
    );
    let admitted =
        zone.player_attack_native_object(&attacker, 102, MirDirection::Left, 0, 0, 0, 50, 3_000);
    assert!(zone.players[&target].hp < 100);
    assert!(admitted.iter().any(|outbound| matches!(outbound, ZoneOutbound::PlayerDamaged { session_id, damage, .. } if session_id == &target && *damage > 0)));
}

#[test]
fn manifest_safety_join_uses_closed_geometry_instead_of_supplied_flag() {
    for (point, expected) in [
        (Point { x: 278, y: 606 }, true),
        (Point { x: 298, y: 626 }, true),
        (Point { x: 299, y: 626 }, false),
        (Point { x: 298, y: 627 }, false),
        (Point { x: 292, y: 629 }, false),
    ] {
        let mut zone =
            ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
        let owner = join_player(
            &mut zone,
            "owner",
            101,
            point.clone(),
            ZoneChatProfile {
                in_safe_zone: !expected,
                ..Default::default()
            },
            Default::default(),
        );
        assert_eq!(zone.players[&owner].position, point);
        assert_eq!(zone.players[&owner].chat_profile.in_safe_zone, expected);
    }
}

#[test]
fn manifest_safety_join_uses_actual_occupancy_adjusted_position() {
    let collision = ZoneCollision::unbounded().with_bounds(ZoneBounds::new(298, 299, 626, 626));
    let mut zone = ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), collision);
    join_player(
        &mut zone,
        "blocker",
        101,
        Point { x: 298, y: 626 },
        Default::default(),
        Default::default(),
    );
    let owner = join_player(
        &mut zone,
        "owner",
        102,
        Point { x: 298, y: 626 },
        ZoneChatProfile {
            in_safe_zone: true,
            ..Default::default()
        },
        Default::default(),
    );
    assert_eq!(zone.players[&owner].position, Point { x: 299, y: 626 });
    assert!(!zone.players[&owner].chat_profile.in_safe_zone);
}

#[test]
fn manifest_safety_rejects_both_stale_profile_directions_and_retains_other_fields() {
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
    let owner = join_player(
        &mut zone,
        "owner",
        101,
        Point { x: 292, y: 629 },
        Default::default(),
        Default::default(),
    );
    let output = zone.handle(ZoneCommand::UpdateChatProfile {
        session_id: owner.clone(),
        profile: ZoneChatProfile {
            in_safe_zone: true,
            free_server_shout: true,
            group_members: vec!["Friend".into()],
            pet_mode: 1,
            ..Default::default()
        },
    });
    assert!(
        output.is_empty(),
        "metadata repair must not introduce a world action or ACK"
    );
    assert!(!zone.players[&owner].chat_profile.in_safe_zone);
    assert!(zone.players[&owner].chat_profile.free_server_shout);
    assert_eq!(zone.players[&owner].chat_profile.group_members, ["Friend"]);
    assert_eq!(zone.players[&owner].chat_profile.pet_mode, 1);
    zone.handle(ZoneCommand::SyncPlayerTransform {
        session_id: owner.clone(),
        position: Point { x: 288, y: 626 },
        direction: MirDirection::Up,
    });
    zone.handle(ZoneCommand::UpdateChatProfile {
        session_id: owner.clone(),
        profile: ZoneChatProfile {
            in_safe_zone: false,
            ..Default::default()
        },
    });
    assert!(zone.players[&owner].chat_profile.in_safe_zone);
    assert!(!zone.players[&owner].chat_profile.free_server_shout);
}

#[test]
fn manifest_safety_sync_transform_repairs_changed_and_identical_positions() {
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
    let owner = join_player(
        &mut zone,
        "owner",
        101,
        Point { x: 288, y: 626 },
        Default::default(),
        Default::default(),
    );
    zone.handle(ZoneCommand::SyncPlayerTransform {
        session_id: owner.clone(),
        position: Point { x: 292, y: 629 },
        direction: MirDirection::Down,
    });
    assert!(!zone.players[&owner].chat_profile.in_safe_zone);
    // A same-position reconciliation must heal old metadata without resetting movement clocks.
    zone.players
        .get_mut(&owner)
        .unwrap()
        .chat_profile
        .in_safe_zone = true;
    zone.players.get_mut(&owner).unwrap().movement_ready_at_ms = 1234;
    let output = zone.handle(ZoneCommand::SyncPlayerTransform {
        session_id: owner.clone(),
        position: Point { x: 292, y: 629 },
        direction: MirDirection::Down,
    });
    assert!(output.is_empty());
    assert!(!zone.players[&owner].chat_profile.in_safe_zone);
    assert_eq!(zone.players[&owner].movement_ready_at_ms, 1234);
}

#[test]
fn manifest_safety_queued_walk_cannot_be_reverted_by_delayed_profile() {
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
    let owner = join_player(
        &mut zone,
        "owner",
        101,
        Point { x: 288, y: 625 },
        Default::default(),
        Default::default(),
    );
    step(
        &mut zone,
        &owner,
        ZoneMovementActionKind::Walk,
        MirDirection::Down,
        1,
    );
    assert_eq!(zone.players[&owner].position, Point { x: 288, y: 626 });
    step(
        &mut zone,
        &owner,
        ZoneMovementActionKind::Walk,
        MirDirection::Down,
        2,
    );
    assert_eq!(zone.players[&owner].position, Point { x: 288, y: 626 });
    zone.handle(ZoneCommand::TickPlayerMovement {
        session_id: owner.clone(),
        now_ms: 1000,
    });
    assert_eq!(zone.players[&owner].position, Point { x: 288, y: 627 });
    zone.handle(ZoneCommand::UpdateChatProfile {
        session_id: owner.clone(),
        profile: ZoneChatProfile {
            in_safe_zone: true,
            ..Default::default()
        },
    });
    assert!(!zone.players[&owner].chat_profile.in_safe_zone);
}

#[test]
fn manifest_safety_custom_map_profile_and_sync_keep_trusted_semantics() {
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("custom"), ZoneCollision::unbounded());
    let owner = join_player(
        &mut zone,
        "owner",
        101,
        Point { x: 10, y: 10 },
        ZoneChatProfile {
            in_safe_zone: true,
            ..Default::default()
        },
        Default::default(),
    );
    zone.handle(ZoneCommand::SyncPlayerTransform {
        session_id: owner.clone(),
        position: Point { x: 20, y: 20 },
        direction: MirDirection::Down,
    });
    assert!(zone.players[&owner].chat_profile.in_safe_zone);
    zone.handle(ZoneCommand::UpdateChatProfile {
        session_id: owner.clone(),
        profile: ZoneChatProfile::default(),
    });
    assert!(!zone.players[&owner].chat_profile.in_safe_zone);
    zone.handle(ZoneCommand::SyncPlayerTransform {
        session_id: owner.clone(),
        position: Point { x: 20, y: 20 },
        direction: MirDirection::Down,
    });
    assert!(!zone.players[&owner].chat_profile.in_safe_zone);
}
