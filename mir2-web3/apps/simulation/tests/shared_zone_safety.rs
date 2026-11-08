use mir2_game_data::MapBounds;
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    SessionId, ZoneChatProfile, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey, ZoneOutbound,
    ZoneRuntime,
};

// Checkpoint restore rebuilds collision from the map key, so use a fixture map
// with the same unbounded collision instead of overriding a real map's data.
const TEST_MAP: &str = "safety-fixture";

fn player() -> SessionId {
    SessionId::new("safety-player")
}

fn bounds(min_x: i32, max_x: i32, min_y: i32, max_y: i32) -> MapBounds {
    MapBounds {
        min_x,
        max_x,
        min_y,
        max_y,
    }
}

fn zone_at(position: Point, initially_safe: bool, collision: ZoneCollision) -> ZoneRuntime {
    let mut zone = ZoneRuntime::new_with_collision(ZoneKey::for_map(TEST_MAP), collision);
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: player(),
        account_id: "safety-account".to_string(),
        character_index: 1,
        object_id: 1,
        name: "SafetyPlayer".to_string(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 1,
        hp: 18,
        max_hp: 18,
        mp: 0,
        map_file_name: TEST_MAP.to_string(),
        position,
        direction: MirDirection::Down,
        chat_profile: ZoneChatProfile {
            in_safe_zone: initially_safe,
            ..Default::default()
        },
        combat_stats: Default::default(),
    }));
    zone
}

fn configure(zone: &mut ZoneRuntime, bounds: Vec<MapBounds>) {
    assert!(
        zone.handle(ZoneCommand::ConfigurePlayerSafetyZones {
            session_id: player(),
            bounds,
        })
        .is_empty(),
        "trusted safety configuration must not manufacture movement packets"
    );
}

fn safe(zone: &ZoneRuntime) -> bool {
    zone.player_chat_profile(&player())
        .expect("joined player's profile")
        .in_safe_zone
}

fn walk(
    zone: &mut ZoneRuntime,
    direction: MirDirection,
    seq: u64,
    now_ms: u64,
) -> Vec<ZoneOutbound> {
    zone.handle(ZoneCommand::Walk {
        session_id: player(),
        direction,
        seq,
        now_ms,
    })
}

fn sync(zone: &mut ZoneRuntime, x: i32, y: i32) {
    zone.handle(ZoneCommand::SyncPlayerTransform {
        session_id: player(),
        position: Point { x, y },
        direction: MirDirection::Down,
    });
}

#[test]
fn configured_safety_follows_closed_boundaries_for_walk_run_exit_and_reentry() {
    let mut zone = zone_at(Point { x: 10, y: 10 }, false, ZoneCollision::unbounded());
    configure(&mut zone, vec![bounds(10, 11, 10, 11)]);
    assert!(
        safe(&zone),
        "configuration immediately classifies the minimum boundary"
    );
    walk(&mut zone, MirDirection::Right, 1, 1);
    assert_eq!(
        zone.player_position(&player()),
        Some(Point { x: 11, y: 10 })
    );
    assert!(safe(&zone), "the maximum x boundary is included");
    walk(&mut zone, MirDirection::Down, 2, 1_000);
    assert_eq!(
        zone.player_position(&player()),
        Some(Point { x: 11, y: 11 })
    );
    assert!(safe(&zone), "the maximum y boundary is included");
    zone.handle(ZoneCommand::Run {
        session_id: player(),
        direction: MirDirection::Right,
        seq: 3,
        now_ms: 1_800,
    });
    assert_eq!(
        zone.player_position(&player()),
        Some(Point { x: 13, y: 11 })
    );
    assert!(
        !safe(&zone),
        "a real run beyond the safe bounds loses protection"
    );
    walk(&mut zone, MirDirection::Left, 4, 2_500);
    assert!(!safe(&zone));
    walk(&mut zone, MirDirection::Left, 5, 3_200);
    assert_eq!(
        zone.player_position(&player()),
        Some(Point { x: 11, y: 11 })
    );
    assert!(
        safe(&zone),
        "ordinary reentry restores the spatial classification"
    );
}

#[test]
fn queued_steps_refresh_on_cadence_consumption_and_blocked_steps_keep_the_origin_flag() {
    let collision = ZoneCollision::unbounded().with_blocked_cells([Point { x: 12, y: 10 }]);
    let mut zone = zone_at(Point { x: 10, y: 10 }, false, collision);
    configure(&mut zone, vec![bounds(10, 11, 10, 10)]);
    walk(&mut zone, MirDirection::Right, 1, 1);
    walk(&mut zone, MirDirection::Down, 2, 2);
    assert_eq!(
        zone.player_position(&player()),
        Some(Point { x: 11, y: 10 })
    );
    assert!(
        safe(&zone),
        "unconsumed movement intent does not move the safe flag"
    );
    zone.tick(601);
    assert_eq!(
        zone.player_position(&player()),
        Some(Point { x: 11, y: 11 })
    );
    assert!(!safe(&zone), "the cadence-owned queued step updates safety");

    sync(&mut zone, 11, 10);
    assert!(safe(&zone));
    let correction = walk(&mut zone, MirDirection::Right, 3, 1_100);
    assert_eq!(
        zone.player_position(&player()),
        Some(Point { x: 11, y: 10 })
    );
    assert!(
        safe(&zone),
        "a static collision preserves the origin's safe status"
    );
    assert!(correction.iter().any(|outbound| matches!(outbound,
        ZoneOutbound::ToSession { session_id, packets }
            if session_id == &player() && packets.iter().any(|packet| matches!(packet, ServerPacket::UserLocation { .. }))
    )));
}

#[test]
fn configured_additional_regions_and_internal_sync_override_stale_personal_profiles() {
    let mut zone = zone_at(Point { x: 20, y: 20 }, false, ZoneCollision::unbounded());
    configure(
        &mut zone,
        vec![bounds(10, 11, 10, 11), bounds(20, 20, 20, 20)],
    );
    assert!(
        safe(&zone),
        "a same-map additional safe region is authoritative too"
    );
    let mut stale = zone.player_chat_profile(&player()).unwrap();
    stale.in_safe_zone = false;
    stale.guild_name = Some("UnrelatedGuild".to_string());
    zone.handle(ZoneCommand::UpdateChatProfile {
        session_id: player(),
        profile: stale,
    });
    assert!(
        safe(&zone),
        "a lagging personal profile cannot clear a real safe tile"
    );
    assert_eq!(
        zone.player_chat_profile(&player())
            .unwrap()
            .guild_name
            .as_deref(),
        Some("UnrelatedGuild")
    );

    sync(&mut zone, 21, 20);
    assert!(
        !safe(&zone),
        "an internal authoritative displacement refreshes safety"
    );
    let mut stale = zone.player_chat_profile(&player()).unwrap();
    stale.in_safe_zone = true;
    zone.handle(ZoneCommand::UpdateChatProfile {
        session_id: player(),
        profile: stale,
    });
    assert!(
        !safe(&zone),
        "a lagging personal profile cannot extend town protection"
    );
    sync(&mut zone, 20, 20);
    assert!(safe(&zone));
    configure(&mut zone, Vec::new());
    assert!(
        !safe(&zone),
        "configured empty bounds explicitly mean no safe tiles"
    );
}

#[test]
fn missing_safety_configuration_preserves_legacy_trusted_profiles_and_checkpoint_shape() {
    let mut zone = zone_at(Point { x: 10, y: 10 }, true, ZoneCollision::unbounded());
    walk(&mut zone, MirDirection::Right, 1, 1);
    assert!(
        safe(&zone),
        "unconfigured fixtures keep their existing trusted flag"
    );
    let checkpoint = zone
        .checkpoint_bytes()
        .expect("capture unconfigured checkpoint");
    let old_shape: serde_json::Value = serde_json::from_slice(&checkpoint).unwrap();
    assert!(
        old_shape["players"][player().as_str()]
            .get("safety_zone_bounds")
            .is_none(),
        "None is omitted, preserving the old canonical checkpoint field set"
    );
    let mut restored = ZoneRuntime::restore_checkpoint(&checkpoint)
        .expect("a checkpoint without safety bounds remains valid");
    assert!(safe(&restored));
    let mut trusted = restored.player_chat_profile(&player()).unwrap();
    trusted.in_safe_zone = false;
    restored.handle(ZoneCommand::UpdateChatProfile {
        session_id: player(),
        profile: trusted,
    });
    sync(&mut restored, 10, 10);
    assert!(
        !safe(&restored),
        "legacy profiles remain directly trusted until configured"
    );
}

#[test]
fn configured_bounds_and_queued_exit_survive_checkpoint_restore() {
    let mut zone = zone_at(Point { x: 10, y: 10 }, false, ZoneCollision::unbounded());
    configure(&mut zone, vec![bounds(10, 11, 10, 10)]);
    walk(&mut zone, MirDirection::Right, 1, 1);
    walk(&mut zone, MirDirection::Down, 2, 2);
    let checkpoint = zone
        .checkpoint_bytes()
        .expect("capture configured checkpoint");
    let encoded: serde_json::Value = serde_json::from_slice(&checkpoint).unwrap();
    assert_eq!(
        encoded["players"][player().as_str()]["safety_zone_bounds"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let mut restored = ZoneRuntime::restore_checkpoint(&checkpoint)
        .expect("configured bounds participate in a restorable canonical checkpoint");
    assert!(safe(&restored));
    restored.tick(601);
    assert_eq!(
        restored.player_position(&player()),
        Some(Point { x: 11, y: 11 })
    );
    assert!(
        !safe(&restored),
        "restored pending movement uses restored safety bounds"
    );
    sync(&mut restored, 10, 10);
    assert!(safe(&restored));
}
