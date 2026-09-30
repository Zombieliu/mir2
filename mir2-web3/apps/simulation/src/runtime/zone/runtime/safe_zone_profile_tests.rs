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
