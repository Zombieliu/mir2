use super::*;
use crate::companion_supply::LifeScopeStatus;

fn scope() -> LifeScope {
    LifeScope {
        life_permit_id: "life:homeward-fixture".into(), map_file_name: "0".into(),
        bounds: SupplyBounds { min_x: 270, min_y: 595, max_x: 310, max_y: 645 },
        home: SupplyPoint { x: 288, y: 616 }, expires_at_ms: 10_000, status: LifeScopeStatus::Revoked,
    }
}
fn p(x: i32, y: i32) -> Point { Point { x, y } }

#[test]
fn companion_homeward_real_bichon_return_first_moves_down_away_from_geometric_home() {
    let scope = scope();
    // The exact ordinary map constructor used by a live Zone, without editing
    // data, replacing terrain, spawning actors or a debug teleport.
    let collision = ZoneCollision::for_full_world_test("0");
    let map = HomewardMap::build(&scope, &collision, collision.static_path_cache_token()).expect("actual Bichon home must be statically reachable");
    let start = p(290, 631);
    let down = p(290, 632);
    let cheb = |point: &Point| (point.x - scope.home.x).abs().max((point.y - scope.home.y).abs());
    assert_eq!((cheb(&start), cheb(&down)), (15, 16));
    let distance = map.distance(&start).expect("real failed return position belongs to a reachable corridor");
    assert_eq!(map.distance(&down), Some(distance - 1));
    assert!(map.allows_path(&start, &[down.clone()]));
    // Complete the static corridor and verify every step with the same policy.
    let mut current = down;
    let mut count = 1;
    while map.distance(&current).unwrap() > 0 {
        let distance = map.distance(&current).unwrap();
        let next = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
            .map(|(dx, dy)| p(current.x + dx, current.y + dy))
            .find(|next| map.step_walkable(&current, next) && map.distance(next) == Some(distance - 1))
            .expect("each finite BFS distance has a physical preceding tile");
        assert!(map.allows_path(&current, std::slice::from_ref(&next)));
        current = next;
        count += 1;
        assert!(count <= MAX_CELLS);
    }
    assert!(cheb(&current) <= 1);
    assert_eq!(count, usize::from(map.distance(&start).unwrap()));
}

#[test]
fn companion_homeward_blocks_static_home_unknown_terrain_and_disconnected_return() {
    let cache = HomewardCache::default();
    let scope = scope();
    let blocked_home = ZoneCollision::unbounded().with_blocked_cells([p(288, 616)]);
    assert!(!cache.allows(&scope, &blocked_home, &p(288, 618), &[p(288, 617)]));
    let missing = ZoneCollision::for_map("missing-homeward-collision-fixture");
    assert!(!cache.allows(&scope, &missing, &p(288, 618), &[p(288, 617)]));
    let wall = ZoneCollision::unbounded().with_blocked_cells((270..=310).map(|x| p(x, 620)));
    assert!(!cache.allows(&scope, &wall, &p(288, 625), &[p(288, 624)]));
}

#[test]
fn companion_homeward_stops_at_home_or_its_neighbour_and_rejects_lateral_circles() {
    let cache = HomewardCache::default();
    let scope = scope();
    let collision = ZoneCollision::unbounded();
    assert!(cache.allows(&scope, &collision, &p(288, 616), &[p(288, 616)]));
    assert!(!cache.allows(&scope, &collision, &p(288, 616), &[p(289, 616)]));
    assert!(!cache.allows(&scope, &collision, &p(289, 617), &[p(288, 617)]));
    assert!(!cache.allows(&scope, &collision, &p(291, 616), &[p(291, 615)]), "equal distance cannot authorize a circuit");
    assert!(!cache.allows(&scope, &collision, &p(291, 616), &[p(292, 616)]));
}

#[test]
fn companion_homeward_checks_all_run_tiles_and_never_cuts_static_corners() {
    let cache = HomewardCache::default();
    let scope = scope();
    let collision = ZoneCollision::unbounded();
    assert!(cache.allows(&scope, &collision, &p(292, 616), &[p(291, 616), p(290, 616)]));
    assert!(!cache.allows(&scope, &collision, &p(291, 616), &[p(291, 615), p(290, 615)]), "final progress cannot hide a non-homeward intermediate tile");
    assert!(!cache.allows(&scope, &collision, &p(291, 616), &[p(289, 616)]), "non-adjacent paths cannot bypass a wall");
    let corner = collision.with_blocked_cells([p(290, 616)]);
    assert!(!cache.allows(&scope, &corner, &p(291, 616), &[p(290, 615)]));
}

#[test]
fn companion_homeward_cache_shares_identical_geometry_across_permits_and_invalidates_terrain_changes() {
    let cache = HomewardCache::default();
    let mut scope = scope();
    let collision = ZoneCollision::unbounded();
    for _ in 0..10 { assert!(cache.allows(&scope, &collision.clone(), &p(291, 616), &[p(290, 616)])); }
    assert_eq!(cache.maps.lock().unwrap().len(), 1, "normal steps and live transaction forks reuse one BFS");
    let changed = collision.clone().with_blocked_cells([p(290, 616)]);
    assert!(!cache.allows(&scope, &changed, &p(291, 616), &[p(290, 616)]));
    assert_eq!(cache.maps.lock().unwrap().len(), 2);
    scope.life_permit_id = "life:new-authority".into();
    assert!(cache.allows(&scope, &collision, &p(291, 616), &[p(290, 616)]));
    assert_eq!(cache.maps.lock().unwrap().len(), 2, "a different actor/permit cannot evict identical geometry");
    for index in 0..MAX_MAPS + 4 {
        scope.life_permit_id = format!("life:bounded-history:{index}");
        scope.home.y = 600 + index as i32;
        let _ = cache.allows(&scope, &collision, &p(291, 616), &[p(290, 616)]);
    }
    assert_eq!(cache.maps.lock().unwrap().len(), MAX_MAPS);
}

#[test]
fn companion_homeward_malformed_scope_and_cache_poison_fail_closed() {
    let cache = HomewardCache::default();
    let mut scope = scope();
    let collision = ZoneCollision::unbounded();
    scope.bounds.min_x = i32::MIN;
    assert!(!cache.allows(&scope, &collision, &p(291, 616), &[p(290, 616)]));
    assert_eq!(cache.maps.lock().unwrap().len(), 0);
    scope.bounds.min_x = 270;
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _locked = cache.maps.lock().unwrap();
        panic!("controlled local cache poison");
    }));
    assert!(!cache.allows(&scope, &collision, &p(291, 616), &[p(290, 616)]));
}

#[test]
fn companion_homeward_public_client_route_from_288637_matches_authoritative_terrain() {
    // Independent route produced by the existing public client BFS over its
    // distributed 0.map.gz, with the exported fixed village NPC positions.
    // No running game, private state, server grant or edited map supplies it.
    let route = [
        (288,637),(288,638),(288,639),(288,640),(288,641),(288,642),(288,643),
        (289,644),(290,644),(291,644),(292,643),(293,642),(294,641),(295,640),
        (296,639),(297,638),(298,637),(298,636),(298,635),(298,634),(298,633),
        (298,632),(298,631),(298,630),(297,629),(296,628),(295,627),(294,626),
        (293,625),(292,624),(291,623),(291,622),(291,621),(291,620),(291,619),
        (290,618),(289,617),
    ];
    let scope = scope();
    let collision = ZoneCollision::for_full_world_test("0");
    let map = HomewardMap::build(&scope, &collision, collision.static_path_cache_token()).unwrap();
    for (index, pair) in route.windows(2).enumerate() {
        let from = p(pair[0].0, pair[0].1);
        let to = p(pair[1].0, pair[1].1);
        assert!(map.allows_path(&from, std::slice::from_ref(&to)),
            "public client step {index}: {from:?} distance {:?} -> {to:?} distance {:?}",
            map.distance(&from), map.distance(&to));
    }
}

#[test]
fn companion_homeward_recorded_live_blocker_at_288638_allows_public_client_detour_to_home() {
    let scope = scope();
    let collision = ZoneCollision::for_full_world_test("0");
    let cache = HomewardCache::default();
    let mut blocked = mir2_game_data::crystal_npc_info_manifest().npcs.iter()
        .filter(|npc| npc.map_file_name.as_deref() == Some("0") && npc.location.x >= 270 && npc.location.x <= 310 && npc.location.y >= 595 && npc.location.y <= 645)
        .map(|npc| npc.location.clone()).collect::<Vec<_>>();
    // Public lf-44cc9d5021 proof: id206626 walked here at1793731962408,
    // and remained here to attack at1793731964014. Its type was not recorded.
    blocked.push(p(288, 638));
    let live = ZoneMovementOccupancy::for_test(&blocked);
    let origin = p(288, 637);
    assert!(!cache.allows(&scope, &collision, &origin, &[p(289, 637)]), "old static distance rejects the real detour's equal-distance first step");
    assert!(!cache.allows_live(&scope, &collision, &live, &origin, &[p(288, 638)]), "cannot step through the observed blocker");
    // Independently generated by the existing public client BFS with that
    // observed blocking cell and the unchanged distributed map/NPC catalog.
    let route = [
        (288,637),(289,637),(289,638),(289,639),(289,640),(289,641),(289,642),
        (289,643),(289,644),(290,644),(291,644),(292,643),(293,642),(294,641),
        (295,640),(296,639),(297,638),(298,637),(298,636),(298,635),(298,634),
        (298,633),(298,632),(298,631),(298,630),(297,629),(296,628),(295,627),
        (294,626),(293,625),(292,624),(291,623),(291,622),(291,621),(291,620),
        (291,619),(290,618),(289,617),
    ];
    for (index, pair) in route.windows(2).enumerate() {
        let from = p(pair[0].0, pair[0].1);
        let to = p(pair[1].0, pair[1].1);
        assert!(cache.allows_live(&scope, &collision, &live, &from, std::slice::from_ref(&to)), "independent live detour step {index}: {from:?} -> {to:?}");
    }
    assert_eq!(cache.maps.lock().unwrap().len(), 1);
    assert_eq!(cache.live_maps.lock().unwrap().len(), 1, "all 37 steps share a distance graph while actual blockers stay unchanged");
}

#[test]
fn companion_homeward_live_masks_invalidate_when_an_occupant_moves_or_disappears() {
    let scope = scope();
    let collision = ZoneCollision::unbounded();
    let cache = HomewardCache::default();
    let blocker = [p(290, 616)];
    let live = ZoneMovementOccupancy::for_test(&blocker);
    assert!(!cache.allows(&scope, &collision, &p(291, 616), &[p(291, 615)]));
    assert!(cache.allows_live(&scope, &collision, &live, &p(291, 616), &[p(291, 615)]));
    assert!(!cache.allows_live(&scope, &collision, &live, &p(291, 616), &[p(290, 616)]));
    assert!(!cache.allows_live(&scope, &collision, &live, &p(291, 616), &[p(292, 616)]), "a dynamic obstacle never authorizes a lateral circuit");
    assert!(!cache.allows_live(&scope, &collision, &live, &p(291, 616), &[p(291, 615), p(291, 614)]), "every actual Run cell must progress in the same live graph");
    let gone = ZoneMovementOccupancy::for_test(&[]);
    assert!(cache.allows_live(&scope, &collision, &gone, &p(291, 616), &[p(290, 616)]));
    assert!(!cache.allows_live(&scope, &collision, &gone, &p(291, 616), &[p(291, 615)]), "stale dynamic detours cannot survive removal");
    let moved = [p(291, 615)];
    let moved = ZoneMovementOccupancy::for_test(&moved);
    assert!(!cache.allows_live(&scope, &collision, &moved, &p(291, 616), &[p(291, 615)]));
    assert!(cache.allows_live(&scope, &collision, &moved, &p(291, 616), &[p(290, 616)]));
    assert_eq!(cache.live_maps.lock().unwrap().len(), 2);
}

#[test]
fn companion_homeward_live_blockers_may_close_every_home_goal_and_poison_stays_fail_closed() {
    let scope = scope();
    let collision = ZoneCollision::unbounded();
    let cache = HomewardCache::default();
    let occupied = (615..=617).flat_map(|y| (287..=289).map(move |x| p(x, y))).collect::<Vec<_>>();
    assert!(!cache.allows_live(&scope, &collision, &ZoneMovementOccupancy::for_test(&occupied), &p(291, 616), &[p(290, 616)]));
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _locked = cache.live_maps.lock().unwrap();
        panic!("controlled live occupancy cache poison");
    }));
    assert!(!cache.allows_live(&scope, &collision, &ZoneMovementOccupancy::for_test(&[]), &p(291, 616), &[p(290, 616)]));
}
