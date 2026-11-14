use super::*;
use crate::runtime::zone::types::{ZoneMovementAction, ZoneMovementActionKind};
use crate::runtime::zone::{
    ZoneChatProfile, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey, ZonePlayerCombatStats,
    ZoneRuntime,
};
use crate::{GroundDropLootSnapshot, GroundDropSnapshot};
use mir2_protocol::{MirClass, MirDirection, MirGender};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

#[derive(Debug)]
struct Guard {
    enabled: AtomicBool,
    forbidden_x: Option<i32>,
    paths: Mutex<Vec<Vec<Point>>>,
}
impl Guard {
    fn new(enabled: bool) -> Arc<Self> {
        Arc::new(Self {
            enabled: AtomicBool::new(enabled),
            forbidden_x: None,
            paths: Mutex::new(Vec::new()),
        })
    }
}
impl ZoneActionBoundary for Guard {
    fn allows(&self, r: &ZoneActionBoundaryRequest<'_>) -> bool {
        assert_eq!(r.account_id, "actor-fixture");
        assert_eq!(r.character_index, 2);
        self.paths.lock().unwrap().push(r.path.to_vec());
        self.enabled.load(Ordering::SeqCst) && !r.path.iter().any(|p| Some(p.x) == self.forbidden_x)
    }
}
fn join(session: &str, id: u32, x: i32) -> ZoneJoin {
    ZoneJoin {
        session_id: session.into(),
        account_id: "actor-fixture".into(),
        character_index: 2,
        object_id: id,
        name: session.into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 10,
        hp: 100,
        max_hp: 100,
        mp: 100,
        map_file_name: "boundary-fixture".into(),
        position: Point { x, y: 10 },
        direction: MirDirection::Right,
        chat_profile: ZoneChatProfile {
            attack_mode: 5,
            ..Default::default()
        },
        combat_stats: ZonePlayerCombatStats::default(),
    }
}
fn zone() -> ZoneRuntime {
    let mut zone = ZoneRuntime::new_with_collision(
        ZoneKey::for_map("boundary-fixture"),
        ZoneCollision::unbounded(),
    );
    zone.handle(ZoneCommand::Join(join("actor", 101, 10)));
    zone
}
fn sid(value: &str) -> crate::runtime::zone::SessionId {
    value.into()
}
fn step(kind: ZoneMovementActionKind) -> ZoneMovementAction {
    ZoneMovementAction {
        kind,
        direction: MirDirection::Right,
        seq: Some(1),
        received_at_ms: 1000,
    }
}

#[test]
fn zone_action_boundary_checks_every_actual_run_tile_before_any_position_change() {
    let mut zone = zone();
    zone.players
        .get_mut(&sid("actor"))
        .unwrap()
        .run_step_until_ms = 10_000;
    let guard = Arc::new(Guard {
        enabled: AtomicBool::new(true),
        forbidden_x: Some(11),
        paths: Mutex::new(Vec::new()),
    });
    zone.set_action_boundary(guard.clone());
    zone.consume_step_action(&"actor".into(), step(ZoneMovementActionKind::Run), 1000);
    assert_eq!(zone.players[&sid("actor")].position, Point { x: 10, y: 10 });
    assert_eq!(
        guard.paths.lock().unwrap().as_slice(),
        &[vec![Point { x: 11, y: 10 }, Point { x: 12, y: 10 }]]
    );
}

#[test]
fn zone_action_boundary_rechecks_buffered_movement_after_authority_changes() {
    let mut zone = zone();
    let guard = Guard::new(true);
    zone.set_action_boundary(guard.clone());
    zone.players
        .get_mut(&sid("actor"))
        .unwrap()
        .movement_actions
        .push_back(step(ZoneMovementActionKind::Walk));
    guard.enabled.store(false, Ordering::SeqCst);
    zone.consume_movement_action(&"actor".into(), 2000);
    assert_eq!(zone.players[&sid("actor")].position, Point { x: 10, y: 10 });
    assert_eq!(guard.paths.lock().unwrap().len(), 1);
    assert!(zone.players[&sid("actor")].movement_actions.is_empty());
}

#[test]
fn zone_action_boundary_recognizes_direct_movement_transfer_before_stepping_onto_it() {
    #[derive(Debug)]
    struct ExitGuard(AtomicBool);
    impl ZoneActionBoundary for ExitGuard {
        fn allows(&self, request: &ZoneActionBoundaryRequest<'_>) -> bool {
            self.0.store(request.crosses_map_exit, Ordering::SeqCst);
            !request.crosses_map_exit
        }
    }
    let collision =
        ZoneCollision::unbounded().with_transfer_source_cells_for_test([Point { x: 11, y: 10 }]);
    let mut zone = ZoneRuntime::new_with_collision(ZoneKey::for_map("boundary-fixture"), collision);
    zone.handle(ZoneCommand::Join(join("actor", 101, 10)));
    let guard = Arc::new(ExitGuard(AtomicBool::new(false)));
    zone.set_action_boundary(guard.clone());
    zone.consume_step_action(&"actor".into(), step(ZoneMovementActionKind::Walk), 1000);
    assert!(guard.0.load(Ordering::SeqCst));
    assert_eq!(zone.players[&sid("actor")].position, Point { x: 10, y: 10 });
}

#[test]
fn zone_action_boundary_is_not_serialized_and_transaction_forks_keep_live_host_guard() {
    let mut zone = zone();
    let bytes = zone.checkpoint_bytes().unwrap();
    let guard = Guard::new(false);
    zone.set_action_boundary(guard.clone());
    assert_eq!(zone.checkpoint_bytes().unwrap(), bytes);
    let mut fork = zone.transaction_fork();
    fork.consume_step_action(&"actor".into(), step(ZoneMovementActionKind::Walk), 1000);
    assert_eq!(fork.players[&sid("actor")].position, Point { x: 10, y: 10 });
    // Cold restore reselects signed collision by map, rather than accepting a
    // fixture's injected unbounded collision. Use that real map constructor;
    // runtime policy still must come only from the trusted live host.
    let cold = ZoneRuntime::new(ZoneKey::for_map("0"));
    let mut restored = ZoneRuntime::restore_checkpoint(&cold.checkpoint_bytes().unwrap()).unwrap();
    assert!(restored.action_boundary.is_none());
    restored.set_action_boundary(guard);
    assert!(restored.action_boundary.is_some());
}

#[test]
fn zone_action_boundary_receives_exact_static_collision_across_transaction_forks() {
    #[derive(Debug, Default)]
    struct CollisionGuard(Mutex<Vec<(Arc<()>, bool)>>);
    impl ZoneActionBoundary for CollisionGuard {
        fn allows(&self, request: &ZoneActionBoundaryRequest<'_>) -> bool {
            let allowed = request.path.iter().all(|point| request.collision.static_path_cell_walkable(point));
            self.0.lock().unwrap().push((request.collision.static_path_cache_token(), allowed));
            allowed
        }
    }
    let collision = ZoneCollision::unbounded().with_blocked_cells([Point { x: 11, y: 10 }]);
    let token = collision.static_path_cache_token();
    let mut zone = ZoneRuntime::new_with_collision(ZoneKey::for_map("boundary-fixture"), collision);
    zone.handle(ZoneCommand::Join(join("actor", 101, 10)));
    let guard = Arc::new(CollisionGuard::default());
    zone.set_action_boundary(guard.clone());
    let mut fork = zone.transaction_fork();
    zone.consume_step_action(&"actor".into(), step(ZoneMovementActionKind::Walk), 1000);
    fork.consume_step_action(&"actor".into(), step(ZoneMovementActionKind::Walk), 1000);
    assert_eq!(zone.players[&sid("actor")].position, Point { x: 10, y: 10 });
    assert_eq!(fork.players[&sid("actor")].position, Point { x: 10, y: 10 });
    let seen = guard.0.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert!(seen.iter().all(|(actual, allowed)| Arc::ptr_eq(actual, &token) && !allowed));
}

#[test]
fn zone_action_boundary_live_occupancy_uses_retained_players_npcs_and_alive_monsters_excluding_self() {
    use crate::runtime::zone::types::ZoneNativeMonster;
    use crate::runtime::zone::{ZoneMonsterDefense, ZoneMonsterSpawn};
    let collision = ZoneCollision::unbounded().with_door(7, vec![(18, 10)]);
    let mut zone = ZoneRuntime::new_with_collision(ZoneKey::for_map("boundary-fixture"), collision);
    zone.handle(ZoneCommand::Join(join("actor", 101, 10)));
    let actor = sid("actor");
    let mut other = join("other", 102, 12);
    other.account_id = "other-fixture".into();
    other.character_index = 3;
    zone.handle(ZoneCommand::Join(other));
    zone.handle(ZoneCommand::SyncSharedObjects {
        session_id: actor.clone(), include_owner: false, now_ms: 1000,
        packets: vec![mir2_protocol::ServerPacket::ObjectNpc { info: mir2_protocol::NpcInfo {
            object_id: 301, name: "LiveNpc".into(), name_colour_argb: -1, image: 1,
            colour_argb: -1, location: Point { x: 14, y: 10 }, direction: MirDirection::Down,
            quest_ids: Vec::new(),
        }}],
    });
    let spawn = ZoneMonsterSpawn {
        crystal_drop_seed: None, object_id: 401, name: "Deer".into(), name_colour_argb: -1,
        image: 1, ai: 0, disposition: None, level: 1, max_hp: 10, hp: 10, experience: 0,
        move_speed_ms: 1500, attack_speed_ms: 1800, friendly_guild: None,
        position: Point { x: 16, y: 10 }, direction: MirDirection::Down,
        defense: ZoneMonsterDefense::default(), respawn: None, drops: Vec::new(),
    };
    zone.native_monsters.insert(401, ZoneNativeMonster::from_spawn(&spawn, 401));
    {
        let view = ZoneMovementOccupancy::for_zone(&zone, &actor);
        assert!(view.can_occupy(&Point { x: 10, y: 10 }), "exclude this retained actor");
        for x in [12, 14, 16, 18] { assert!(!view.can_occupy(&Point { x, y: 10 })); }
        for x in 8..=20 {
            assert_eq!(view.can_occupy(&Point { x, y: 10 }), zone.can_player_movement_occupy(&Point { x, y: 10 }, Some(&actor)));
        }
        let dynamic = view.dynamic_blocking_cells(crate::runtime::zone::ZoneBounds::new(8, 20, 8, 12));
        for y in 8..=12 {
            for x in 8..=20 {
                let point = Point { x, y };
                if zone.collision.static_path_cell_walkable(&point) {
                    assert_eq!(dynamic.contains(&point), !view.can_occupy(&point));
                }
            }
        }
    }
    zone.handle(ZoneCommand::OpenDoor { session_id: actor.clone(), door_index: 7, now_ms: 1100 });
    assert!(ZoneMovementOccupancy::for_zone(&zone, &actor).can_occupy(&Point { x: 18, y: 10 }));
    zone.native_monsters.get_mut(&401).unwrap().position.x = 17;
    assert!(ZoneMovementOccupancy::for_zone(&zone, &actor).can_occupy(&Point { x: 16, y: 10 }));
    assert!(!ZoneMovementOccupancy::for_zone(&zone, &actor).can_occupy(&Point { x: 17, y: 10 }));
    {
        let monster = zone.native_monsters.get_mut(&401).unwrap();
        monster.dead = true; monster.hp = 0;
    }
    assert!(ZoneMovementOccupancy::for_zone(&zone, &actor).can_occupy(&Point { x: 17, y: 10 }));
    {
        let monster = zone.native_monsters.get_mut(&401).unwrap();
        monster.dead = false; monster.hp = 10;
    }
    assert!(!ZoneMovementOccupancy::for_zone(&zone, &actor).can_occupy(&Point { x: 17, y: 10 }), "a revived monster blocks even if a client retains stale death metadata");
    zone.objects.remove(&301);
    zone.handle(ZoneCommand::Leave { session_id: sid("other") });
    let mut gate = spawn.clone();
    gate.object_id = 402; gate.name = "SabukGate".into(); gate.ai = 81;
    gate.position = Point { x: 22, y: 10 };
    zone.native_monsters.insert(402, ZoneNativeMonster::from_spawn(&gate, 402));
    let fork = zone.transaction_fork();
    for current in [&zone, &fork] {
        let view = ZoneMovementOccupancy::for_zone(current, &actor);
        assert!(view.can_occupy(&Point { x: 12, y: 10 }));
        assert!(view.can_occupy(&Point { x: 14, y: 10 }));
        assert!(!view.can_occupy(&Point { x: 17, y: 10 }));
        let bounds = crate::runtime::zone::ZoneBounds::new(8, 20, 8, 12);
        let dynamic = view.dynamic_blocking_cells(bounds);
        assert!(dynamic.contains(&Point { x: 20, y: 10 }), "a closed gate's footprint can enter the rectangle while its centre is outside");
        assert!(!dynamic.contains(&Point { x: 18, y: 10 }), "the opened static door cannot become a phantom dynamic blocker");
        for y in bounds.min_y..=bounds.max_y {
            for x in bounds.min_x..=bounds.max_x {
                let point = Point { x, y };
                if current.collision.static_path_cell_walkable(&point) {
                    assert_eq!(dynamic.contains(&point), !view.can_occupy(&point), "exact dynamic mask at {point:?}");
                }
            }
        }
    }
}

#[test]
fn zone_action_boundary_blocks_attack_before_damage_and_action_clock_changes() {
    let mut zone = zone();
    let mut target = join("target", 102, 11);
    target.account_id = "target-fixture".into();
    target.character_index = 3;
    zone.handle(ZoneCommand::Join(target));
    let guard = Guard::new(false);
    zone.set_action_boundary(guard.clone());
    zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: "actor".into(),
        object_id: 102,
        direction: MirDirection::Right,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 10,
        now_ms: 1000,
    });
    assert_eq!(zone.players[&sid("target")].hp, 100);
    assert_eq!(zone.players[&sid("actor")].next_attack_ready_at_ms, 0);
    assert_eq!(
        guard.paths.lock().unwrap().last().unwrap(),
        &[Point { x: 11, y: 10 }]
    );
}

#[test]
fn zone_action_boundary_blocks_real_drop_claim_before_removal_or_reservation() {
    let mut zone = zone();
    let session = "actor".into();
    let drop = GroundDropSnapshot {
        object_id: 501,
        name: "Gold".into(),
        name_colour_argb: 0,
        icon: 0,
        x: 10,
        y: 10,
        quantity: 1,
        source_monster: "fixture".into(),
        owner_object_id: None,
        ownership_remaining_ticks: None,
        loot: GroundDropLootSnapshot::Gold { amount: 1 },
    };
    let stored = zone.new_zone_ground_drop(drop, None).unwrap();
    zone.ground_drops.insert(501, stored);
    let guard = Guard::new(false);
    zone.set_action_boundary(guard.clone());
    let before = zone.next_ground_drop_claim_id;
    zone.claim_ground_drop(&session, Some(501), &Point { x: 10, y: 10 }, &[], 1000);
    assert!(zone.ground_drops.contains_key(&501));
    assert!(zone.claimed_ground_drops.is_empty());
    assert_eq!(zone.next_ground_drop_claim_id, before);
    guard.enabled.store(true, Ordering::SeqCst);
    zone.claim_ground_drop(&session, Some(501), &Point { x: 10, y: 10 }, &[], 1001);
    assert!(!zone.ground_drops.contains_key(&501));
    assert!(!zone.claimed_ground_drops.is_empty());
}
