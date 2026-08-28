//! Gateway-layer evidence for the ordinary Bichon vertical slice.
//!
//! The richer combat, quest, drop, pickup, and reward sequence is exercised
//! without privileged commands by `mir2-simulation`'s
//! `ordinary_candidate_loop` integration test.  This companion test owns the
//! missing boundary: a fresh account and character must traverse the Gateway
//! command route, save through logout, then reload through a newly constructed
//! Gateway session.  It deliberately uses only normal Crystal client packets.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mir2_gateway::{GatewayConfig, GatewaySession};
use mir2_protocol::{
    ClientPacket, MirClass, MirDirection, MirGender, MirGridType, Point, ServerPacket, Spell,
};
use mir2_simulation::{
    GroundDropLootSnapshot, ItemContainer, QuestStage, WorldEntityKind, WorldEntitySnapshot,
};

const TEST_RECOVERY_MAC_KEY: [u8; 32] = [
    0x10, 0x21, 0x32, 0x43, 0x54, 0x65, 0x76, 0x87, 0x98, 0xa9, 0xba, 0xcb, 0xdc, 0xed, 0xfe, 0x0f,
    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xf1, 0x02,
];
static GATEWAY_WORLD_MODE_LOCK: Mutex<()> = Mutex::new(());

struct SaveFileGuard(PathBuf);

impl Drop for SaveFileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn unique_save_path() -> SaveFileGuard {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    SaveFileGuard(std::env::temp_dir().join(format!(
        "mir2-gateway-vertical-slice-{}-{nanos}.json",
        std::process::id()
    )))
}

fn file_backed_gateway_config(path: PathBuf) -> GatewayConfig {
    GatewayConfig::default()
        .with_account_store_path(path)
        .with_save_recovery_mac_key(TEST_RECOVERY_MAC_KEY)
        .expect("test-only file store must have a valid recovery MAC key")
}

fn file_backed_crystal_gateway_config(path: PathBuf) -> GatewayConfig {
    file_backed_gateway_config(path).with_crystal_world_runtime()
}

fn lock_gateway_world_mode(full_crystal_world: bool) -> MutexGuard<'static, ()> {
    let guard = GATEWAY_WORLD_MODE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    mir2_simulation::set_crystal_full_world_zone_collision(full_crystal_world);
    guard
}

fn login(session: &mut GatewaySession, account_id: &str, password: &str) {
    let packets = session
        .try_handle_packet(ClientPacket::Login {
            account_id: account_id.to_string(),
            password: password.to_string(),
        })
        .expect("Gateway Login should execute");
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })),
        "Gateway Login should succeed: {packets:?}"
    );
}

fn player(session: &GatewaySession) -> mir2_simulation::WorldEntitySnapshot {
    session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .expect("Gateway StartGame should expose the self player")
}

fn nearby_npc(session: &GatewaySession, name: &str) -> WorldEntitySnapshot {
    let snapshot = session.world_snapshot();
    snapshot
        .entities
        .iter()
        .cloned()
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::Npc && entity.name == name)
        .unwrap_or_else(|| {
            let player = snapshot
                .entities
                .iter()
                .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
                .map(|entity| (entity.x, entity.y));
            let visible_npcs = snapshot
                .entities
                .iter()
                .filter(|entity| entity.kind == WorldEntityKind::Npc)
                .map(|entity| (entity.name.as_str(), entity.x, entity.y))
                .collect::<Vec<_>>();
            panic!(
                "Gateway should expose NPC {name}: map={:?}, player={player:?}, npcs={visible_npcs:?}",
                snapshot.map_file_name
            )
        })
}

fn direction_toward(from: &Point, to: &Point) -> MirDirection {
    match ((to.x - from.x).signum(), (to.y - from.y).signum()) {
        (0, -1) => MirDirection::Up,
        (1, -1) => MirDirection::UpRight,
        (1, 0) => MirDirection::Right,
        (1, 1) => MirDirection::DownRight,
        (0, 1) => MirDirection::Down,
        (-1, 1) => MirDirection::DownLeft,
        (-1, 0) => MirDirection::Left,
        (-1, -1) => MirDirection::UpLeft,
        _ => MirDirection::Down,
    }
}

fn tile_distance(left: &Point, right: &Point) -> i32 {
    (left.x - right.x).abs().max((left.y - right.y).abs())
}

#[derive(Clone, Copy)]
struct HuntBounds {
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
}

impl HuntBounds {
    fn contains(&self, point: &Point) -> bool {
        point.x >= self.min_x
            && point.x <= self.max_x
            && point.y >= self.min_y
            && point.y <= self.max_y
    }
}

const WESTERN_DEER_HARVEST_BOUNDS: HuntBounds = HuntBounds {
    min_x: 140,
    max_x: 260,
    min_y: 270,
    max_y: 330,
};

// The final Gateway snapshot retains a 16-tile Crystal data range, while the
// walk helper only guarantees stopping within 8 tiles of a search point. A
// 16-tile grid therefore gives every covered spawn an observation point within
// the remaining 8-tile radius; the old 36-tile grid left blind strips.
const FIELD_SEARCH_OFFSETS: [i32; 7] = [-48, -32, -16, 0, 16, 32, 48];

struct CrystalMapWalkability {
    width: i32,
    height: i32,
    walkable: Vec<bool>,
}

impl CrystalMapWalkability {
    fn is_walkable(&self, x: i32, y: i32) -> bool {
        if x < 0 || x >= self.width || y < 0 || y >= self.height {
            return false;
        }
        self.walkable[(x * self.height + y) as usize]
    }
}

fn bichon_map_walkability() -> &'static CrystalMapWalkability {
    static WALKABILITY: OnceLock<CrystalMapWalkability> = OnceLock::new();
    WALKABILITY.get_or_init(|| {
        let map_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../web/lib/generated/crystal-map-pack/0.map.gz");
        let compressed = fs::read(&map_path)
            .unwrap_or_else(|error| panic!("read bundled Bichon map {map_path:?}: {error}"));
        let mut decoder = flate2::read::GzDecoder::new(compressed.as_slice());
        let mut bytes = Vec::new();
        decoder
            .read_to_end(&mut bytes)
            .unwrap_or_else(|error| panic!("decompress bundled Bichon map {map_path:?}: {error}"));
        assert_eq!(&bytes[0..4], &[1, 0, 0x43, 0x23]);

        let width = i32::from(i16::from_le_bytes([bytes[4], bytes[5]]));
        let height = i32::from(i16::from_le_bytes([bytes[6], bytes[7]]));
        assert!(width > 0 && height > 0);
        let mut walkable = vec![false; (width * height) as usize];
        let mut offset = 8_usize;
        for x in 0..width {
            for y in 0..height {
                offset += 2;
                let high_wall = (i32::from_le_bytes(
                    bytes[offset..offset + 4]
                        .try_into()
                        .expect("Bichon high-wall bytes"),
                ) & 0x2000_0000)
                    != 0;
                offset += 10;
                let low_wall = (i16::from_le_bytes(
                    bytes[offset..offset + 2]
                        .try_into()
                        .expect("Bichon low-wall bytes"),
                ) & i16::MIN)
                    != 0;
                offset += 2;
                let closed_door = bytes[offset] > 0;
                offset += 12;
                walkable[(x * height + y) as usize] = !high_wall && !low_wall && !closed_door;
            }
        }
        assert_eq!(offset, bytes.len());

        CrystalMapWalkability {
            width,
            height,
            walkable,
        }
    })
}

fn tick_after_client_action(session: &mut GatewaySession) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    for _ in 0..8 {
        packets.extend(session.tick());
    }
    packets
}

fn direction_step(direction: MirDirection) -> (i32, i32) {
    match direction {
        MirDirection::Up => (0, -1),
        MirDirection::UpRight => (1, -1),
        MirDirection::Right => (1, 0),
        MirDirection::DownRight => (1, 1),
        MirDirection::Down => (0, 1),
        MirDirection::DownLeft => (-1, 1),
        MirDirection::Left => (-1, 0),
        MirDirection::UpLeft => (-1, -1),
    }
}

const WALK_DIRECTIONS: [MirDirection; 8] = [
    MirDirection::Up,
    MirDirection::UpRight,
    MirDirection::Right,
    MirDirection::DownRight,
    MirDirection::Down,
    MirDirection::DownLeft,
    MirDirection::Left,
    MirDirection::UpLeft,
];

fn planned_walk_directions(
    start: &Point,
    target: &Point,
    maximum_distance: i32,
    blocked: &BTreeSet<(i32, i32)>,
) -> Option<Vec<MirDirection>> {
    if tile_distance(start, target) <= maximum_distance {
        return Some(Vec::new());
    }
    let map = bichon_map_walkability();
    let minimum_x = start.x.min(target.x).saturating_sub(128).max(0);
    let maximum_x = start.x.max(target.x).saturating_add(128).min(map.width - 1);
    let minimum_y = start.y.min(target.y).saturating_sub(128).max(0);
    let maximum_y = start
        .y
        .max(target.y)
        .saturating_add(128)
        .min(map.height - 1);
    let start_key = (start.x, start.y);
    let mut queue = VecDeque::from([start_key]);
    let mut visited = BTreeSet::from([start_key]);
    let mut previous = BTreeMap::<(i32, i32), ((i32, i32), MirDirection)>::new();
    let mut goal = None;

    while let Some(current) = queue.pop_front() {
        for direction in WALK_DIRECTIONS {
            let (dx, dy) = direction_step(direction);
            let next = (current.0 + dx, current.1 + dy);
            if next.0 < minimum_x
                || next.0 > maximum_x
                || next.1 < minimum_y
                || next.1 > maximum_y
                || !map.is_walkable(next.0, next.1)
                || blocked.contains(&next)
                || !visited.insert(next)
            {
                continue;
            }
            previous.insert(next, (current, direction));
            if tile_distance(
                &Point {
                    x: next.0,
                    y: next.1,
                },
                target,
            ) <= maximum_distance
            {
                goal = Some(next);
                break;
            }
            queue.push_back(next);
        }
        if goal.is_some() {
            break;
        }
    }

    let mut cursor = goal?;
    let mut reversed = Vec::new();
    while cursor != start_key {
        let (parent, direction) = previous.get(&cursor).copied()?;
        reversed.push(direction);
        cursor = parent;
    }
    reversed.reverse();
    Some(reversed)
}

fn try_walk_direction(session: &mut GatewaySession, direction: MirDirection) -> bool {
    let before = player(session);
    let mut packets = session.handle_packet(ClientPacket::Walk { direction });
    for _ in 0..128 {
        let after = player(session);
        if (after.x, after.y) != (before.x, before.y) {
            return true;
        }
        if packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::UserLocation { .. }))
        {
            return false;
        }
        packets = session.tick();
    }
    false
}

fn try_walk_within_with_step_budget(
    session: &mut GatewaySession,
    target: &Point,
    maximum_distance: i32,
    maximum_walk_steps: usize,
) -> bool {
    let mut blocked = BTreeSet::<(i32, i32)>::new();
    let mut dynamic_block_resets = 0_u8;
    let mut attempted_walk_steps = 0_usize;
    for _ in 0..1_024 {
        let current = player(session);
        let current_point = Point {
            x: current.x,
            y: current.y,
        };
        if tile_distance(&current_point, target) <= maximum_distance {
            return true;
        }

        let Some(path) =
            planned_walk_directions(&current_point, target, maximum_distance, &blocked)
        else {
            // Reaching this tile proves that at least one neighboring edge was
            // traversable. If all eight neighbors have since rejected movement,
            // shared monsters/players temporarily occupied the exits; they are
            // not permanent map collision. Let the Zone advance and relearn.
            dynamic_block_resets = dynamic_block_resets.saturating_add(1);
            if blocked.is_empty() || dynamic_block_resets > 16 {
                return false;
            }
            blocked.clear();
            for _ in 0..8 {
                tick_after_client_action(session);
            }
            continue;
        };
        let mut should_replan = false;
        for direction in path {
            if attempted_walk_steps >= maximum_walk_steps {
                return false;
            }
            attempted_walk_steps = attempted_walk_steps.saturating_add(1);
            let before = player(session);
            let (dx, dy) = direction_step(direction);
            let attempted = (before.x + dx, before.y + dy);
            let mut moved = false;
            for _ in 0..3 {
                if try_walk_direction(session, direction) {
                    moved = true;
                    break;
                }
                tick_after_client_action(session);
            }
            if !moved {
                blocked.insert(attempted);
                should_replan = true;
                break;
            }
            let after = player(session);
            if tile_distance(
                &Point {
                    x: after.x,
                    y: after.y,
                },
                target,
            ) <= maximum_distance
            {
                return true;
            }
        }
        if !should_replan {
            break;
        }
    }

    let current = player(session);
    let current_point = Point {
        x: current.x,
        y: current.y,
    };
    tile_distance(&current_point, target) <= maximum_distance
}

fn try_walk_within(session: &mut GatewaySession, target: &Point, maximum_distance: i32) -> bool {
    try_walk_within_with_step_budget(session, target, maximum_distance, 1_024)
}

fn walk_within(session: &mut GatewaySession, target: &Point, maximum_distance: i32) {
    let mut reached = false;
    // A live shared field can temporarily occupy every edge chosen by one
    // planner budget. Preserve the same collision-aware ordinary Walk path,
    // but allow a few bounded replans after advancing the Zone instead of
    // treating one dynamic blockage as a permanently unreachable map tile.
    for _ in 0..4 {
        if try_walk_within(session, target, maximum_distance) {
            reached = true;
            break;
        }
        tick_after_client_action(session);
    }
    let current = player(session);
    let current_point = Point {
        x: current.x,
        y: current.y,
    };
    assert!(
        reached,
        "Gateway Walk path could not reach required distance: current={current_point:?}, target={target:?}, maximum_distance={maximum_distance}"
    );
}

fn field_centers_nearest_first(session: &GatewaySession, centers: &[Point]) -> Vec<Point> {
    let current = player(session);
    let current = Point {
        x: current.x,
        y: current.y,
    };
    let mut ordered = centers.to_vec();
    ordered.sort_by_key(|center| tile_distance(&current, center));
    ordered
}

fn field_search_points_nearest_first(
    session: &GatewaySession,
    centers: &[Point],
    hunt_bounds: Option<HuntBounds>,
) -> Vec<Point> {
    let current = player(session);
    let current = Point {
        x: current.x,
        y: current.y,
    };
    let mut unique = BTreeSet::new();
    let mut points = Vec::new();
    for center in centers {
        for dx in FIELD_SEARCH_OFFSETS {
            for dy in FIELD_SEARCH_OFFSETS {
                let point = Point {
                    x: center.x + dx,
                    y: center.y + dy,
                };
                if hunt_bounds.is_some_and(|bounds| !bounds.contains(&point)) {
                    continue;
                }
                if unique.insert((point.x, point.y)) {
                    points.push(point);
                }
            }
        }
    }
    points.sort_by_key(|point| tile_distance(&current, point));
    points
}

fn walk_toward(session: &mut GatewaySession, target: &Point) {
    walk_within(session, target, 1);
}

fn walk_onto(session: &mut GatewaySession, target: &Point) {
    walk_within(session, target, 0);
}

#[test]
fn bundled_bichon_walk_planner_routes_to_isolated_deer_field() {
    let start = Point { x: 292, y: 603 };
    let target = Point { x: 205, y: 325 };
    let directions = planned_walk_directions(&start, &target, 8, &BTreeSet::new())
        .expect("bundled Crystal collision must expose a route to the western Deer field");
    assert!(!directions.is_empty());

    let map = bichon_map_walkability();
    let mut current = start;
    for direction in directions {
        let (dx, dy) = direction_step(direction);
        current.x += dx;
        current.y += dy;
        assert!(
            map.is_walkable(current.x, current.y),
            "planned ordinary Walk step must remain on Crystal-walkable 0.map cells: {current:?}"
        );
    }
    assert!(tile_distance(&current, &target) <= 8);

    let deer_group = mir2_game_data::crystal_map_respawns_by_file_name("0")
        .expect("Bichon respawn manifest")
        .respawns
        .into_iter()
        .find(|respawn| respawn.monster_name == "Deer" && respawn.location == target)
        .expect("western Deer respawn group");
    let safe_slots = mir2_simulation::crystal_world_respawn_spawns("0", &deer_group)
        .into_iter()
        .filter(|(_, point, _)| WESTERN_DEER_HARVEST_BOUNDS.contains(point))
        .map(|(_, point, _)| point)
        .collect::<Vec<_>>();
    assert!(
        safe_slots.len() >= 10,
        "western Deer field must retain enough static northern slots for the probabilistic q4 harvest: {}",
        safe_slots.len()
    );
    let search_points = FIELD_SEARCH_OFFSETS
        .into_iter()
        .flat_map(|dx| {
            FIELD_SEARCH_OFFSETS.into_iter().filter_map(move |dy| {
                let point = Point {
                    x: target.x + dx,
                    y: target.y + dy,
                };
                WESTERN_DEER_HARVEST_BOUNDS
                    .contains(&point)
                    .then_some(point)
            })
        })
        .collect::<Vec<_>>();
    for slot in safe_slots {
        assert!(
            search_points
                .iter()
                .any(|search_point| tile_distance(search_point, &slot) <= 8),
            "Gateway search grid must expose western Deer slot {slot:?} inside the retained AOI"
        );
    }
}

fn quest_stage(session: &GatewaySession, quest_id: i32) -> Option<QuestStage> {
    session
        .world_snapshot()
        .quest_log
        .into_iter()
        .find(|quest| quest.quest_id == quest_id)
        .map(|quest| quest.stage)
}

fn newcomer_cumulative_experience(session: &GatewaySession) -> i64 {
    const THRESHOLDS: [i64; 6] = [100, 200, 300, 400, 600, 900];
    let snapshot = session.world_snapshot();
    let level = player(session).level.unwrap_or(1).max(1);
    THRESHOLDS
        .iter()
        .take(usize::from(level.saturating_sub(1)))
        .sum::<i64>()
        .saturating_add(snapshot.player_experience)
}

fn open_quest_npc(
    session: &mut GatewaySession,
    npc_name: &str,
    expected_link: &str,
) -> WorldEntitySnapshot {
    if !session
        .world_snapshot()
        .entities
        .iter()
        .any(|entity| entity.kind == WorldEntityKind::Npc && entity.name == npc_name)
    {
        let manifest_npc = mir2_game_data::crystal_npc_info_manifest()
            .npcs
            .into_iter()
            .find(|npc| npc.name == npc_name)
            .unwrap_or_else(|| panic!("missing Crystal manifest coordinate for NPC {npc_name}"));
        let manifest_position = Point {
            x: manifest_npc.location.x,
            y: manifest_npc.location.y,
        };
        walk_toward(session, &manifest_position);
        tick_after_client_action(session);
    }
    for _ in 0..40 {
        if session
            .world_snapshot()
            .entities
            .iter()
            .any(|entity| entity.kind == WorldEntityKind::Npc && entity.name == npc_name)
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
        tick_after_client_action(session);
    }
    let npc = nearby_npc(session, npc_name);
    walk_toward(session, &Point { x: npc.x, y: npc.y });
    let packets = session.handle_packet(ClientPacket::CallNpc {
        object_id: npc.object_id,
        key: "@Main".to_string(),
    });
    assert!(
        session
            .world_snapshot()
            .active_npc_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.links.iter().any(|link| link.target == expected_link)),
        "ordinary Gateway CallNpc must expose {expected_link} at {npc_name}: {packets:?}"
    );
    npc
}

fn use_hp_drug_if_needed(session: &mut GatewaySession) -> Vec<ServerPacket> {
    let snapshot = session.world_snapshot();
    let (Some(hp), Some(max_hp)) = (snapshot.player_hp, snapshot.player_max_hp) else {
        return Vec::new();
    };
    if hp <= 0 || hp.saturating_mul(2) > max_hp {
        return Vec::new();
    }
    let potion = snapshot
        .inventory_items
        .iter()
        .find(|item| item.name.starts_with("(HP)Drug"))
        .map(|item| (item.unique_id, MirGridType::Inventory))
        .or_else(|| {
            snapshot
                .belt_items
                .iter()
                .find(|item| item.name.starts_with("(HP)Drug"))
                .map(|item| (item.unique_id, MirGridType::Belt))
        });
    let Some((unique_id, grid)) = potion else {
        return Vec::new();
    };

    let mut packets = session.handle_packet(ClientPacket::UseItem { unique_id, grid });
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::UseItem { success: true, .. })),
        "ordinary Gateway HP recovery must use the carried potion: {packets:?}"
    );
    packets.extend(session.tick());
    packets
}

fn equip_inventory_item(session: &mut GatewaySession, name: &str, to: i32) {
    let item = session
        .world_snapshot()
        .inventory_items
        .into_iter()
        .find(|item| item.name == name)
        .unwrap_or_else(|| panic!("ordinary Gateway starter item {name} should be in inventory"));
    let packets = session.handle_packet(ClientPacket::EquipItem {
        grid: MirGridType::Inventory,
        unique_id: item.unique_id,
        to,
    });
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::EquipItem { success: true, .. })),
        "ordinary Gateway EquipItem must equip {name}: {packets:?}"
    );
}

fn nearest_alive_monster(
    session: &GatewaySession,
    name: &str,
    excluded_object_ids: &BTreeSet<u32>,
    hunt_bounds: Option<HuntBounds>,
) -> Option<WorldEntitySnapshot> {
    let current = player(session);
    session
        .world_snapshot()
        .entities
        .into_iter()
        .filter(|entity| {
            entity.kind == WorldEntityKind::Monster
                && entity.name == name
                && !entity.dead
                && entity.hp.unwrap_or(1) > 0
                && !excluded_object_ids.contains(&entity.object_id)
                && hunt_bounds.is_none_or(|bounds| {
                    bounds.contains(&Point {
                        x: entity.x,
                        y: entity.y,
                    })
                })
        })
        .min_by_key(|entity| {
            tile_distance(
                &Point {
                    x: current.x,
                    y: current.y,
                },
                &Point {
                    x: entity.x,
                    y: entity.y,
                },
            )
        })
}

fn attack_monster_until_dead(
    session: &mut GatewaySession,
    monster: &WorldEntitySnapshot,
    hunt_bounds: Option<HuntBounds>,
) -> (Vec<ServerPacket>, Option<Point>) {
    let mut packets = Vec::new();
    let mut last_position = Point {
        x: monster.x,
        y: monster.y,
    };
    let mut missing_ticks = 0_u8;
    let mut last_hp = monster.hp.unwrap_or(i32::MAX);
    let mut no_damage_rounds = 0_u8;

    for _ in 0..80 {
        if session.world_snapshot().player_hp.is_some_and(|hp| hp <= 0) {
            let revived = session.handle_packet(ClientPacket::TownRevive);
            assert!(
                revived
                    .iter()
                    .any(|packet| matches!(packet, ServerPacket::Revived)),
                "ordinary Gateway TownRevive must recover the defeated player: {revived:?}"
            );
            packets.extend(revived);
            packets.extend(tick_after_client_action(session));
            return (packets, None);
        }
        packets.extend(use_hp_drug_if_needed(session));
        let snapshot = session.world_snapshot();
        assert!(
            snapshot.player_hp.is_none_or(|hp| hp > 0),
            "ordinary Gateway player must remain alive while hunting {}",
            monster.name
        );
        let current_player_position = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .map(|entity| Point {
                x: entity.x,
                y: entity.y,
            })
            .expect("Gateway snapshot should retain SelfPlayer while hunting");
        if tile_distance(&current_player_position, &last_position) > 64 {
            return (packets, None);
        }
        let current_monster = snapshot
            .entities
            .into_iter()
            .find(|entity| entity.object_id == monster.object_id);
        let Some(current_monster) = current_monster else {
            missing_ticks = missing_ticks.saturating_add(1);
            if missing_ticks >= 8 {
                return if hunt_bounds.is_some() {
                    (packets, None)
                } else {
                    (packets, Some(last_position))
                };
            }
            if !try_walk_within_with_step_budget(session, &last_position, 2, 64) {
                return (packets, None);
            }
            packets.extend(session.tick());
            continue;
        };
        missing_ticks = 0;
        last_position = Point {
            x: current_monster.x,
            y: current_monster.y,
        };
        if hunt_bounds.is_some_and(|bounds| !bounds.contains(&last_position)) {
            return (packets, None);
        }
        let current_hp = current_monster.hp.unwrap_or(1);
        if current_monster.dead || current_hp <= 0 {
            return (packets, Some(last_position));
        }
        if current_hp < last_hp {
            last_hp = current_hp;
            no_damage_rounds = 0;
        } else {
            no_damage_rounds = no_damage_rounds.saturating_add(1);
            if no_damage_rounds >= 12 {
                return (packets, None);
            }
        }

        if !try_walk_within_with_step_budget(session, &last_position, 1, 64) {
            return (packets, None);
        }
        let current = player(session);
        let direction = direction_toward(
            &Point {
                x: current.x,
                y: current.y,
            },
            &last_position,
        );
        packets.extend(session.handle_packet(ClientPacket::Turn { direction }));
        let attack = session.handle_packet(ClientPacket::Attack {
            direction,
            spell: Spell::None,
        });
        let defeated_position = attack.iter().find_map(|packet| match packet {
            ServerPacket::ObjectDied { info } if info.object_id == monster.object_id => {
                Some(info.location.clone())
            }
            _ => None,
        });
        packets.extend(attack);
        let tick_packets = session.tick();
        let defeated_position = defeated_position.or_else(|| {
            tick_packets.iter().find_map(|packet| match packet {
                ServerPacket::ObjectDied { info } if info.object_id == monster.object_id => {
                    Some(info.location.clone())
                }
                _ => None,
            })
        });
        packets.extend(tick_packets);
        if let Some(defeated_position) = defeated_position {
            return (packets, Some(defeated_position));
        }
    }

    (packets, None)
}

fn harvest_corpse(session: &mut GatewaySession, corpse_position: Point) -> Vec<ServerPacket> {
    assert!(
        try_walk_within_with_step_budget(session, &corpse_position, 1, 64),
        "ordinary Gateway player must reach the authoritative death coordinate before Harvest: player={:?}, corpse={corpse_position:?}",
        player(session)
    );
    let current = player(session);
    let direction = direction_toward(
        &Point {
            x: current.x,
            y: current.y,
        },
        &corpse_position,
    );
    let mut packets = session.handle_packet(ClientPacket::Turn { direction });
    // Crystal Harvest selects the first eligible corpse in the 3x3 scan
    // around the facing tile; a busy shared field can retain as many as nine
    // corpses in that scan. Deer needs five skinning passes plus one transfer
    // pass, so bound the ordinary-packet retry by the complete scan capacity
    // instead of assuming the just-killed corpse is always selected first.
    const MAX_HARVEST_SCAN_PASSES: usize = 9 * 6;
    for _ in 0..MAX_HARVEST_SCAN_PASSES {
        let harvest = session.handle_packet(ClientPacket::Harvest { direction });
        let completed = harvest
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectHarvested { .. }));
        packets.extend(harvest);
        if completed {
            break;
        }
        packets.extend(session.tick());
    }
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectHarvested { .. })),
        "ordinary Gateway Harvest must finish the shared corpse lifecycle: player={:?}, corpse={corpse_position:?}, harvest-passes={}, packets={packets:?}",
        player(session),
        packets
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::ObjectHarvest { .. }))
            .count()
    );
    packets
}

fn progress_original_item_quest(
    session: &mut GatewaySession,
    quest_id: i32,
    monster_name: &str,
    field_centers: &[Point],
    maximum_kills: usize,
    harvest: bool,
    weapon_upgrade: Option<(&str, u16)>,
    hunt_bounds: Option<HuntBounds>,
) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    let mut confirmed_player_kills = 0_usize;
    let mut attempts = 0_usize;
    let mut attempts_since_player_kill = 0_usize;
    let mut excluded_object_ids = BTreeSet::<u32>::new();
    let mut active_field_center = field_centers_nearest_first(session, field_centers)
        .first()
        .cloned();
    if let Some(field_center) = active_field_center.as_ref() {
        walk_within(session, field_center, 8);
        packets.extend(session.tick());
        let current = player(session);
        eprintln!(
            "gateway original q{quest_id}: reached field near ({}, {}) at ({}, {})",
            field_center.x, field_center.y, current.x, current.y
        );
    }
    while confirmed_player_kills < maximum_kills
        && attempts < maximum_kills.saturating_mul(4).max(1)
    {
        if quest_stage(session, quest_id)
            .is_some_and(|stage| matches!(stage, QuestStage::ReadyToTurnIn | QuestStage::Completed))
        {
            break;
        }
        if let Some((weapon_name, required_level)) = weapon_upgrade {
            let snapshot = session.world_snapshot();
            let can_equip = player(session)
                .level
                .is_some_and(|level| level >= required_level)
                && snapshot
                    .inventory_items
                    .iter()
                    .any(|item| item.name == weapon_name)
                && !snapshot
                    .equipment_items
                    .iter()
                    .any(|item| item.name == weapon_name);
            if can_equip {
                equip_inventory_item(session, weapon_name, 0);
            }
        }
        attempts = attempts.saturating_add(1);
        attempts_since_player_kill = attempts_since_player_kill.saturating_add(1);

        let mut monster =
            nearest_alive_monster(session, monster_name, &excluded_object_ids, hunt_bounds);
        if monster.is_none() && !excluded_object_ids.is_empty() {
            excluded_object_ids.clear();
            monster =
                nearest_alive_monster(session, monster_name, &excluded_object_ids, hunt_bounds);
        }
        if monster.is_none() {
            // Crystal respawn positions are scattered across each rule's
            // spread, while Gateway AOI is smaller than a 50-tile field. Scan
            // a coverage grid through ordinary Walk packets instead of
            // assuming a live monster must sit near the rule's center.
            for search_point in
                field_search_points_nearest_first(session, field_centers, hunt_bounds)
            {
                if !try_walk_within(session, &search_point, 8) {
                    continue;
                }
                for _ in 0..8 {
                    packets.extend(session.tick());
                    monster = nearest_alive_monster(
                        session,
                        monster_name,
                        &excluded_object_ids,
                        hunt_bounds,
                    );
                    if monster.is_some() {
                        active_field_center = Some(search_point);
                        break;
                    }
                }
                if monster.is_some() {
                    break;
                }
            }
        }
        let monster = monster.unwrap_or_else(|| {
            panic!("ordinary Gateway should expose {monster_name} near {field_centers:?}")
        });
        let experience_before = newcomer_cumulative_experience(session);
        let (combat_packets, corpse_position) =
            attack_monster_until_dead(session, &monster, hunt_bounds);
        packets.extend(combat_packets);
        let player_owned_kill = newcomer_cumulative_experience(session) > experience_before;
        if player_owned_kill {
            confirmed_player_kills = confirmed_player_kills.saturating_add(1);
            attempts_since_player_kill = 0;
            if harvest && corpse_position.is_some() {
                let corpse_position = corpse_position.expect("checked above");
                packets.extend(harvest_corpse(session, corpse_position));
            }
            eprintln!(
                "gateway original q{quest_id}: confirmed player kill {confirmed_player_kills}/{maximum_kills}, stage={:?}, quest-items={:?}",
                quest_stage(session, quest_id),
                session
                    .world_snapshot()
                    .inventory_items
                    .iter()
                    .filter(|item| item.container == ItemContainer::Quest)
                    .map(|item| (item.name.as_str(), item.quantity))
                    .collect::<Vec<_>>()
            );
            if quest_stage(session, quest_id) == Some(QuestStage::InProgress) {
                if let Some(field_center) = active_field_center.as_ref() {
                    // Returning to the patrol anchor is only a search
                    // optimization. A retained corpse or live monster may
                    // temporarily occupy the last edge; the next coverage
                    // scan can safely choose another ordinary Walk route.
                    if try_walk_within(session, field_center, 8) {
                        packets.extend(session.tick());
                    }
                }
            }
        } else {
            if corpse_position.is_none() {
                excluded_object_ids.insert(monster.object_id);
            }
        }
        if !player_owned_kill && attempts_since_player_kill >= 4 {
            let current = player(session);
            let visible_targets = session
                .world_snapshot()
                .entities
                .into_iter()
                .filter(|entity| {
                    entity.kind == WorldEntityKind::Monster
                        && entity.name == monster_name
                        && !entity.dead
                })
                .count();
            eprintln!(
                "gateway original q{quest_id}: {attempts} attempts, {confirmed_player_kills} kills, player=({}, {}), visible {monster_name}={visible_targets}",
                current.x, current.y
            );
            if field_centers.len() > 1 {
                let current = Point {
                    x: current.x,
                    y: current.y,
                };
                if let Some(next_center) = field_centers_nearest_first(session, field_centers)
                    .into_iter()
                    .find(|center| tile_distance(&current, center) > 8)
                {
                    if try_walk_within(session, &next_center, 8) {
                        packets.extend(session.tick());
                    }
                }
            }
            attempts_since_player_kill = 0;
        }
    }
    packets
}

#[test]
fn gateway_fresh_account_bichon_logout_and_new_session_reload_are_authoritative() {
    let _world_mode = lock_gateway_world_mode(false);
    let save_guard = unique_save_path();
    let save_path = save_guard.0.clone();
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let account_id = format!("gateway_slice_{}_{}", std::process::id(), suffix);
    let password = "GatewaySlice42!";

    let mut first = GatewaySession::new(file_backed_gateway_config(save_path.clone()));
    let created = first
        .try_handle_packet(ClientPacket::NewAccount {
            account_id: account_id.clone(),
            password: password.to_string(),
            birth_date_binary: 0,
            user_name: String::new(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        })
        .expect("Gateway NewAccount should execute");
    assert!(
        created
            .iter()
            .any(|packet| matches!(packet, ServerPacket::NewAccount { result: 8 })),
        "Gateway NewAccount should create an ordinary account: {created:?}"
    );
    login(&mut first, &account_id, password);

    let created_character = first.handle_packet(ClientPacket::NewCharacter {
        name: format!("GateSlice{}", std::process::id()),
        gender: MirGender::Male,
        class: MirClass::Warrior,
    });
    let character_index = created_character
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .unwrap_or_else(|| panic!("Gateway NewCharacter should succeed: {created_character:?}"));

    let started = first.handle_packet(ClientPacket::StartGame { character_index });
    assert!(
        started.iter().any(|packet| matches!(
            packet,
            ServerPacket::StartGame {
                result: 4,
                resolution
            } if *resolution > 0
        )),
        "Gateway StartGame should enter Bichon: {started:?}"
    );
    let initial = first.world_snapshot();
    assert_eq!(initial.map_file_name.as_deref(), Some("0"));
    assert!(initial
        .map_title
        .as_deref()
        .is_some_and(|title| title.to_ascii_lowercase().contains("bichon")));

    let player_before = player(&first);
    let turned = first.handle_packet(ClientPacket::Turn {
        direction: MirDirection::Left,
    });
    assert!(
        turned
            .iter()
            .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })),
        "Gateway Turn must reach the authoritative Zone: {turned:?}"
    );
    let player_after = player(&first);
    assert_eq!(player_after.direction, MirDirection::Left);
    assert_eq!(
        (player_after.x, player_after.y),
        (player_before.x, player_before.y),
        "a turn must not manufacture movement"
    );

    let logout = first.handle_packet(ClientPacket::LogOut);
    assert!(
        logout
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })),
        "Gateway LogOut should save then acknowledge: {logout:?}"
    );
    drop(first);

    let mut second = GatewaySession::new(file_backed_gateway_config(save_path.clone()));
    login(&mut second, &account_id, password);
    let restarted = second.handle_packet(ClientPacket::StartGame { character_index });
    assert!(
        restarted
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })),
        "new Gateway session should restore its selected character: {restarted:?}"
    );
    let reloaded = player(&second);
    assert_eq!(
        (reloaded.x, reloaded.y, reloaded.direction),
        (player_after.x, player_after.y, player_after.direction),
        "a new Gateway session must reload the saved authoritative transform"
    );

    drop(second);
    drop(save_guard);
    assert!(
        !save_path.exists(),
        "Gateway evidence save file should be cleaned up"
    );
}

#[test]
fn gateway_ordinary_bichon_starter_loop_uses_client_packets_and_reloads() {
    let _world_mode = lock_gateway_world_mode(false);
    let save_guard = unique_save_path();
    let save_path = save_guard.0.clone();
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let account_id = format!("gateway_ordinary_{}_{}", std::process::id(), suffix);
    let password = "GatewayOrdinary42!";

    let mut first = GatewaySession::new(file_backed_gateway_config(save_path.clone()));
    let created = first
        .try_handle_packet(ClientPacket::NewAccount {
            account_id: account_id.clone(),
            password: password.to_string(),
            birth_date_binary: 0,
            user_name: String::new(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        })
        .expect("Gateway NewAccount should execute");
    assert!(
        created
            .iter()
            .any(|packet| matches!(packet, ServerPacket::NewAccount { result: 8 })),
        "Gateway NewAccount should create an ordinary account: {created:?}"
    );
    login(&mut first, &account_id, password);

    let created_character = first.handle_packet(ClientPacket::NewCharacter {
        name: format!("GateOrdinary{}", std::process::id()),
        gender: MirGender::Male,
        class: MirClass::Warrior,
    });
    let character_index = created_character
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .unwrap_or_else(|| panic!("Gateway NewCharacter should succeed: {created_character:?}"));
    let started = first.handle_packet(ClientPacket::StartGame { character_index });
    assert!(
        started
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })),
        "Gateway StartGame should enter Bichon: {started:?}"
    );

    let remote_accept = first.handle_packet(ClientPacket::AcceptQuest {
        npc_index: 4001,
        quest_index: 1001,
    });
    assert!(
        !remote_accept.iter().any(|packet| matches!(
            packet,
            ServerPacket::ChangeQuest {
                quest_id: 1001,
                taken: true,
                ..
            }
        )),
        "Gateway must reject AcceptQuest without a nearby active dialog: {remote_accept:?}"
    );

    let initial_player = player(&first);
    let guide = nearby_npc(&first, "Village Guide");
    walk_toward(
        &mut first,
        &Point {
            x: guide.x,
            y: guide.y,
        },
    );
    let after_walk = player(&first);
    assert_ne!(
        (after_walk.x, after_walk.y),
        (initial_player.x, initial_player.y),
        "ordinary Gateway Walk packets must change the authoritative transform"
    );

    let opened = first.handle_packet(ClientPacket::CallNpc {
        object_id: guide.object_id,
        key: "@Main".to_string(),
    });
    assert!(
        opened.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectChat { object_id, .. } if *object_id == guide.object_id
        )) && first
            .world_snapshot()
            .active_npc_dialog
            .as_ref()
            .is_some_and(|dialog| dialog
                .links
                .iter()
                .any(|link| link.target == "@AcceptQuest:1001")),
        "ordinary Gateway CallNpc must expose the starter quest link: {opened:?}"
    );

    let accepted = first.handle_packet(ClientPacket::AcceptQuest {
        npc_index: 4001,
        quest_index: 1001,
    });
    assert!(
        accepted.iter().any(|packet| matches!(
            packet,
            ServerPacket::ChangeQuest {
                quest_id: 1001,
                taken: true,
                ..
            }
        )),
        "ordinary Gateway AcceptQuest packet must start the offered quest: {accepted:?}"
    );
    assert!(first
        .world_snapshot()
        .quest_log
        .iter()
        .any(|quest| { quest.quest_id == 1001 && quest.stage == QuestStage::InProgress }));

    let mut combat_packets = Vec::new();
    for _ in 0..600 {
        let Some(wasp) = first.world_snapshot().entities.into_iter().find(|entity| {
            entity.kind == WorldEntityKind::Monster
                && entity.name == "Field Wasp"
                && !entity.dead
                && entity.hp.unwrap_or(1) > 0
        }) else {
            std::thread::sleep(Duration::from_millis(25));
            combat_packets.extend(tick_after_client_action(&mut first));
            continue;
        };
        walk_toward(
            &mut first,
            &Point {
                x: wasp.x,
                y: wasp.y,
            },
        );
        let Some(wasp) = first.world_snapshot().entities.into_iter().find(|entity| {
            entity.object_id == wasp.object_id && !entity.dead && entity.hp.unwrap_or(1) > 0
        }) else {
            combat_packets.extend(tick_after_client_action(&mut first));
            continue;
        };
        let current = player(&first);
        let direction = direction_toward(
            &Point {
                x: current.x,
                y: current.y,
            },
            &Point {
                x: wasp.x,
                y: wasp.y,
            },
        );
        combat_packets.extend(first.handle_packet(ClientPacket::Turn { direction }));
        combat_packets.extend(first.handle_packet(ClientPacket::Attack {
            direction,
            spell: Spell::None,
        }));
        combat_packets.extend(tick_after_client_action(&mut first));
        if first
            .world_snapshot()
            .quest_log
            .iter()
            .any(|quest| quest.quest_id == 1001 && quest.stage == QuestStage::ReadyToTurnIn)
        {
            break;
        }
    }

    let after_combat = first.world_snapshot();
    assert!(
        after_combat.quest_log.iter().any(|quest| {
            quest.quest_id == 1001 && quest.stage == QuestStage::ReadyToTurnIn
        }),
        "ordinary Gateway Attack packets must make the starter quest ready; packets={combat_packets:?}"
    );
    assert!(
        after_combat.inventory_items.iter().any(|item| {
            item.container == ItemContainer::Quest && item.key == "crystal-item-876"
        }),
        "the player-owned Field Wasp death must grant the quest proof"
    );

    let wasp_gold = after_combat
        .ground_drops
        .iter()
        .find_map(|drop| match drop.loot {
            GroundDropLootSnapshot::Gold { amount } if drop.source_monster == "Field Wasp" => {
                Some((
                    drop.object_id,
                    Point {
                        x: drop.x,
                        y: drop.y,
                    },
                    amount,
                ))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("Field Wasp must create a visible gold drop: {after_combat:?}"));
    let gold_before_pickup = after_combat.gold;
    walk_onto(&mut first, &wasp_gold.1);
    let pickup = first.handle_packet(ClientPacket::PickUp);
    let after_pickup = first.world_snapshot();
    assert!(pickup.iter().any(|packet| matches!(
        packet,
        ServerPacket::GainedGold { gold } if *gold == wasp_gold.2
    )));
    assert_eq!(after_pickup.gold, gold_before_pickup + wasp_gold.2);
    assert!(!after_pickup
        .ground_drops
        .iter()
        .any(|drop| drop.object_id == wasp_gold.0));

    let remote_finish = first.handle_packet(ClientPacket::FinishQuest {
        quest_index: 1001,
        selected_item_index: -1,
    });
    assert!(
        !remote_finish.iter().any(|packet| matches!(
            packet,
            ServerPacket::CompleteQuest { completed_quests }
                if completed_quests.contains(&1001)
        )),
        "Gateway must reject FinishQuest away from an active finish dialog"
    );

    walk_toward(
        &mut first,
        &Point {
            x: guide.x,
            y: guide.y,
        },
    );
    let opened_finish = first.handle_packet(ClientPacket::CallNpc {
        object_id: guide.object_id,
        key: "@Main".to_string(),
    });
    assert!(
        first
            .world_snapshot()
            .active_npc_dialog
            .as_ref()
            .is_some_and(|dialog| dialog
                .links
                .iter()
                .any(|link| link.target == "@FinishQuest:1001")),
        "ordinary Gateway CallNpc must expose the finish link: {opened_finish:?}"
    );

    let gold_before_finish = first.world_snapshot().gold;
    let finished = first.handle_packet(ClientPacket::FinishQuest {
        quest_index: 1001,
        selected_item_index: -1,
    });
    let after_finish = first.world_snapshot();
    assert!(finished.iter().any(|packet| matches!(
        packet,
        ServerPacket::CompleteQuest { completed_quests }
            if completed_quests.contains(&1001)
    )));
    assert_eq!(after_finish.gold, gold_before_finish + 300);
    assert!(after_finish
        .quest_log
        .iter()
        .any(|quest| quest.quest_id == 1001 && quest.stage == QuestStage::Completed));
    assert!(!after_finish
        .inventory_items
        .iter()
        .any(|item| item.key == "crystal-item-876"));
    assert!(after_finish.inventory_items.iter().any(|item| {
        matches!(item.container, ItemContainer::Bag1 | ItemContainer::Bag2)
            && item.key == "crystal-item-1135"
            && item.quantity == 2
    }));
    assert!(
        after_finish
            .equipment_items
            .iter()
            .any(|item| item.name == "CopperRing")
            || after_finish
                .inventory_items
                .iter()
                .any(|item| item.name == "CopperRing")
    );

    let before_logout = first.world_snapshot();
    let before_player = player(&first);
    let logout = first.handle_packet(ClientPacket::LogOut);
    assert!(logout
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
    drop(first);

    let mut second = GatewaySession::new(file_backed_gateway_config(save_path.clone()));
    login(&mut second, &account_id, password);
    let restarted = second.handle_packet(ClientPacket::StartGame { character_index });
    assert!(restarted
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    let after_reload = second.world_snapshot();
    let reloaded_player = player(&second);
    assert_eq!(after_reload.gold, before_logout.gold);
    assert_eq!(after_reload.inventory_items, before_logout.inventory_items);
    assert_eq!(after_reload.equipment_items, before_logout.equipment_items);
    assert_eq!(after_reload.quest_log, before_logout.quest_log);
    assert_eq!(
        (
            reloaded_player.x,
            reloaded_player.y,
            reloaded_player.direction,
        ),
        (before_player.x, before_player.y, before_player.direction,)
    );
    assert!(after_reload
        .quest_log
        .iter()
        .any(|quest| quest.quest_id == 1001 && quest.stage == QuestStage::Completed));

    drop(second);
    drop(save_guard);
    assert!(!save_path.exists(), "Gateway save file should be removed");
}

#[test]
fn gateway_assistant_jane_survives_ordinary_walk_aoi_round_trip() {
    let _world_mode = lock_gateway_world_mode(true);
    let save_guard = unique_save_path();
    let save_path = save_guard.0.clone();
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let account_id = format!("gateway_jane_aoi_{}_{}", std::process::id(), suffix);
    let password = "GatewayJaneAoi42!";

    let mut session = GatewaySession::new(file_backed_crystal_gateway_config(save_path.clone()));
    let created = session
        .try_handle_packet(ClientPacket::NewAccount {
            account_id: account_id.clone(),
            password: password.to_string(),
            birth_date_binary: 0,
            user_name: String::new(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        })
        .expect("Gateway NewAccount should execute");
    assert!(created
        .iter()
        .any(|packet| matches!(packet, ServerPacket::NewAccount { result: 8 })));
    login(&mut session, &account_id, password);

    let created_character = session.handle_packet(ClientPacket::NewCharacter {
        name: format!("GateJaneAoi{}", std::process::id()),
        gender: MirGender::Male,
        class: MirClass::Warrior,
    });
    let character_index = created_character
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .unwrap_or_else(|| panic!("Gateway NewCharacter should succeed: {created_character:?}"));
    let started = session.handle_packet(ClientPacket::StartGame { character_index });
    assert!(started
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));

    let jane = open_quest_npc(&mut session, "Assistant_Jane", "@quest:accept:1");
    let accepted = session.handle_packet(ClientPacket::AcceptQuest {
        npc_index: 3,
        quest_index: 1,
    });
    assert!(accepted.iter().any(|packet| matches!(
        packet,
        ServerPacket::ChangeQuest {
            quest_id: 1,
            taken: true,
            completed: true,
            ..
        }
    )));
    open_quest_npc(&mut session, "CraftsLady_Jude", "@quest:finish:1");
    let finished = session.handle_packet(ClientPacket::FinishQuest {
        quest_index: 1,
        selected_item_index: -1,
    });
    assert!(finished.iter().any(|packet| matches!(
        packet,
        ServerPacket::CompleteQuest { completed_quests } if completed_quests.contains(&1)
    )));
    walk_toward(&mut session, &Point { x: 290, y: 625 });
    for _ in 0..32 {
        tick_after_client_action(&mut session);
    }
    walk_toward(
        &mut session,
        &Point {
            x: jane.x,
            y: jane.y,
        },
    );
    let returned = nearby_npc(&session, "Assistant_Jane");
    assert_eq!(returned.object_id, jane.object_id);

    drop(session);
    drop(save_guard);
    assert!(!save_path.exists(), "Gateway save file should be removed");
}

#[test]
fn gateway_original_bichon_q1_to_q4_uses_client_packets_and_reloads() {
    let _world_mode = lock_gateway_world_mode(true);
    let save_guard = unique_save_path();
    let save_path = save_guard.0.clone();
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let account_id = format!("gateway_original_{}_{}", std::process::id(), suffix);
    let password = "GatewayOriginal42!";

    let mut first = GatewaySession::new(file_backed_crystal_gateway_config(save_path.clone()));
    let created = first
        .try_handle_packet(ClientPacket::NewAccount {
            account_id: account_id.clone(),
            password: password.to_string(),
            birth_date_binary: 0,
            user_name: String::new(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        })
        .expect("Gateway NewAccount should execute");
    assert!(created
        .iter()
        .any(|packet| matches!(packet, ServerPacket::NewAccount { result: 8 })));
    login(&mut first, &account_id, password);

    let created_character = first.handle_packet(ClientPacket::NewCharacter {
        name: format!("GateOriginal{}", std::process::id()),
        gender: MirGender::Male,
        class: MirClass::Warrior,
    });
    let character_index = created_character
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .unwrap_or_else(|| panic!("Gateway NewCharacter should succeed: {created_character:?}"));
    let started = first.handle_packet(ClientPacket::StartGame { character_index });
    assert!(started
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    assert_eq!(first.world_snapshot().map_file_name.as_deref(), Some("0"));
    assert_eq!(quest_stage(&first, 1), Some(QuestStage::Available));
    equip_inventory_item(&mut first, "WoodenSword", 0);
    equip_inventory_item(&mut first, "BaseDress(M)", 1);

    open_quest_npc(&mut first, "Assistant_Jane", "@quest:accept:1");
    let accept_q1 = first.handle_packet(ClientPacket::AcceptQuest {
        npc_index: 3,
        quest_index: 1,
    });
    assert!(accept_q1.iter().any(|packet| matches!(
        packet,
        ServerPacket::ChangeQuest {
            quest_id: 1,
            taken: true,
            completed: true,
            ..
        }
    )));
    assert_eq!(quest_stage(&first, 1), Some(QuestStage::ReadyToTurnIn));
    assert!(first.world_snapshot().inventory_items.iter().any(|item| {
        item.container == ItemContainer::Quest
            && item.name == "CannibalLeaves"
            && item.quantity == 5
    }));

    open_quest_npc(&mut first, "CraftsLady_Jude", "@quest:finish:1");
    let q1_exp_before = newcomer_cumulative_experience(&first);
    let q1_gold_before = first.world_snapshot().gold;
    let q1_potions_before = first
        .world_snapshot()
        .belt_items
        .into_iter()
        .chain(first.world_snapshot().inventory_items)
        .filter(|item| item.name == "(HP)DrugSmall")
        .map(|item| item.quantity)
        .sum::<u32>();
    let finish_q1 = first.handle_packet(ClientPacket::FinishQuest {
        quest_index: 1,
        selected_item_index: -1,
    });
    assert!(finish_q1.iter().any(|packet| matches!(
        packet,
        ServerPacket::CompleteQuest { completed_quests } if completed_quests.contains(&1)
    )));
    assert_eq!(newcomer_cumulative_experience(&first) - q1_exp_before, 10);
    assert_eq!(first.world_snapshot().gold, q1_gold_before);
    assert_eq!(quest_stage(&first, 1), Some(QuestStage::Completed));
    let after_q1 = first.world_snapshot();
    assert!(!after_q1
        .inventory_items
        .iter()
        .any(|item| item.name == "CannibalLeaves"));
    assert_eq!(
        after_q1
            .belt_items
            .iter()
            .chain(&after_q1.inventory_items)
            .filter(|item| item.name == "(HP)DrugSmall")
            .map(|item| item.quantity)
            .sum::<u32>(),
        q1_potions_before + 1
    );

    open_quest_npc(&mut first, "CraftsLady_Jude", "@quest:accept:2");
    let accept_q2 = first.handle_packet(ClientPacket::AcceptQuest {
        npc_index: 4,
        quest_index: 2,
    });
    assert!(accept_q2.iter().any(|packet| matches!(
        packet,
        ServerPacket::ChangeQuest {
            quest_id: 2,
            taken: true,
            completed: false,
            ..
        }
    )));
    let q2_hunt_packets = progress_original_item_quest(
        &mut first,
        2,
        "Scarecrow",
        &[
            Point { x: 270, y: 625 },
            Point { x: 290, y: 615 },
            Point { x: 300, y: 610 },
            Point { x: 110, y: 60 },
            Point { x: 220, y: 60 },
            Point { x: 200, y: 400 },
            Point { x: 500, y: 400 },
            Point { x: 540, y: 530 },
            Point { x: 330, y: 530 },
        ],
        30,
        false,
        None,
        None,
    );
    assert_eq!(
        quest_stage(&first, 2),
        Some(QuestStage::ReadyToTurnIn),
        "ordinary Gateway Scarecrow kills must collect GingerTea: observed death packets={}, revives={}, gained-items={:?}",
        q2_hunt_packets
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::ObjectDied { .. }))
            .count(),
        q2_hunt_packets
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::Revived { .. }))
            .count(),
        q2_hunt_packets
            .iter()
            .filter_map(|packet| match packet {
                ServerPacket::GainedItem { item } => Some(item.item_index),
                _ => None,
            })
            .collect::<Vec<_>>()
    );
    assert!(first.world_snapshot().inventory_items.iter().any(|item| {
        item.container == ItemContainer::Quest && item.name == "GingerTea" && item.quantity >= 1
    }));

    open_quest_npc(&mut first, "Assistant_Jane", "@quest:finish:2");
    let q2_exp_before = newcomer_cumulative_experience(&first);
    let q2_gold_before = first.world_snapshot().gold;
    let finish_q2 = first.handle_packet(ClientPacket::FinishQuest {
        quest_index: 2,
        selected_item_index: -1,
    });
    assert!(finish_q2.iter().any(|packet| matches!(
        packet,
        ServerPacket::CompleteQuest { completed_quests } if completed_quests.contains(&2)
    )));
    assert_eq!(newcomer_cumulative_experience(&first) - q2_exp_before, 30);
    assert_eq!(first.world_snapshot().gold, q2_gold_before + 200);
    assert_eq!(quest_stage(&first, 2), Some(QuestStage::Completed));
    let after_q2 = first.world_snapshot();
    assert!(!after_q2
        .inventory_items
        .iter()
        .any(|item| item.name == "GingerTea"));
    for reward_name in ["GoldenPendant", "CopperRing"] {
        assert!(
            after_q2
                .inventory_items
                .iter()
                .any(|item| item.name == reward_name)
                || after_q2
                    .equipment_items
                    .iter()
                    .any(|item| item.name == reward_name),
            "q2 must retain original reward {reward_name}"
        );
    }

    open_quest_npc(&mut first, "Assistant_Jane", "@quest:accept:3");
    let accept_q3 = first.handle_packet(ClientPacket::AcceptQuest {
        npc_index: 3,
        quest_index: 3,
    });
    assert!(accept_q3.iter().any(|packet| matches!(
        packet,
        ServerPacket::ChangeQuest {
            quest_id: 3,
            taken: true,
            completed: true,
            ..
        }
    )));
    assert_eq!(quest_stage(&first, 3), Some(QuestStage::ReadyToTurnIn));

    let merchant = open_quest_npc(&mut first, "Merchant_John", "@quest:finish:3");
    let choose_q3 = first.handle_packet(ClientPacket::CallNpc {
        object_id: merchant.object_id,
        key: "@quest:finish:3".to_string(),
    });
    assert!(!choose_q3
        .iter()
        .any(|packet| matches!(packet, ServerPacket::CompleteQuest { .. })));
    assert_eq!(quest_stage(&first, 3), Some(QuestStage::ReadyToTurnIn));
    let reward_dialog = first
        .world_snapshot()
        .active_npc_dialog
        .expect("ordinary Gateway q3 must expose its reward selection dialog");
    for (reward_index, reward_name) in ["SharpDagger", "ToughHoaSword", "StiffWoodenBow"]
        .into_iter()
        .enumerate()
    {
        assert!(
            reward_dialog.links.iter().any(|link| {
                link.target.ends_with(&format!(":3:{reward_index}"))
                    && link.text.contains(reward_name)
            }),
            "q3 reward option {reward_index} must be {reward_name}: {:?}",
            reward_dialog.links
        );
    }
    let q3_exp_before = newcomer_cumulative_experience(&first);
    let q3_gold_before = first.world_snapshot().gold;
    let finish_q3 = first.handle_packet(ClientPacket::FinishQuest {
        quest_index: 3,
        selected_item_index: 0,
    });
    assert!(finish_q3.iter().any(|packet| matches!(
        packet,
        ServerPacket::CompleteQuest { completed_quests } if completed_quests.contains(&3)
    )));
    assert_eq!(newcomer_cumulative_experience(&first) - q3_exp_before, 10);
    assert_eq!(first.world_snapshot().gold, q3_gold_before);
    assert_eq!(quest_stage(&first, 3), Some(QuestStage::Completed));
    assert!(first
        .world_snapshot()
        .inventory_items
        .iter()
        .any(|item| item.name == "SharpDagger"));
    if player(&first).level.is_some_and(|level| level >= 2) {
        equip_inventory_item(&mut first, "SharpDagger", 0);
    }

    open_quest_npc(&mut first, "Merchant_John", "@quest:accept:4");
    let accept_q4 = first.handle_packet(ClientPacket::AcceptQuest {
        npc_index: 6,
        quest_index: 4,
    });
    assert!(accept_q4.iter().any(|packet| matches!(
        packet,
        ServerPacket::ChangeQuest {
            quest_id: 4,
            taken: true,
            completed: false,
            ..
        }
    )));
    let q4_hunt_packets = progress_original_item_quest(
        &mut first,
        4,
        "Deer",
        // Use the western Deer field, then constrain targets to its northern
        // static slots so the route does not enter the adjacent Scarecrow/Yob
        // activation edge while a multi-pass Harvest is in progress.
        &[Point { x: 205, y: 325 }],
        // Crystal's DeerMeat quest entry is a real 1/2 Q drop. Match the
        // deterministic Simulation slice budget instead of assuming five
        // successful harvest rolls in only eighteen player-owned kills.
        30,
        true,
        Some(("SharpDagger", 2)),
        Some(WESTERN_DEER_HARVEST_BOUNDS),
    );
    assert_eq!(
        quest_stage(&first, 4),
        Some(QuestStage::ReadyToTurnIn),
        "ordinary Gateway Attack and Harvest packets must collect DeerMeat: deaths={}, harvests={}, gained-items={:?}",
        q4_hunt_packets
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::ObjectDied { .. }))
            .count(),
        q4_hunt_packets
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::ObjectHarvested { .. }))
            .count(),
        q4_hunt_packets
            .iter()
            .filter_map(|packet| match packet {
                ServerPacket::GainedItem { item } => Some(item.item_index),
                _ => None,
            })
            .collect::<Vec<_>>()
    );
    assert!(
        first
            .world_snapshot()
            .inventory_items
            .iter()
            .filter(|item| item.container == ItemContainer::Quest && item.name == "DeerMeat")
            .map(|item| item.quantity)
            .sum::<u32>()
            >= 5
    );

    // The original Crystal server checks reward capacity before removing
    // quest-bag proof items. Keep this full client-packet path honest by
    // freeing the q3 weapon slot when the newcomer is still too low-level to
    // equip it; otherwise the legitimate bag-full rejection masks q4 turn-in.
    if let Some(dagger) = first
        .world_snapshot()
        .inventory_items
        .into_iter()
        .find(|item| item.container == ItemContainer::Bag1 && item.name == "SharpDagger")
    {
        let dropped = first.handle_packet(ClientPacket::DropItem {
            unique_id: dagger.unique_id,
            count: 1,
            hero_inventory: false,
        });
        assert!(dropped.iter().any(|packet| matches!(
            packet,
            ServerPacket::DropItem {
                unique_id,
                success: true,
                ..
            } if *unique_id == dagger.unique_id
        )));
    }

    open_quest_npc(&mut first, "Merchant_John", "@quest:finish:4");
    let q4_exp_before = newcomer_cumulative_experience(&first);
    let q4_gold_before = first.world_snapshot().gold;
    let finish_q4 = first.handle_packet(ClientPacket::FinishQuest {
        quest_index: 4,
        selected_item_index: -1,
    });
    assert!(
        finish_q4.iter().any(|packet| matches!(
            packet,
            ServerPacket::CompleteQuest { completed_quests } if completed_quests.contains(&4)
        )),
        "q4 turn-in must complete after freeing a Crystal reward slot: {finish_q4:?}"
    );
    assert_eq!(newcomer_cumulative_experience(&first) - q4_exp_before, 80);
    assert_eq!(first.world_snapshot().gold, q4_gold_before + 20);
    assert_eq!(quest_stage(&first, 4), Some(QuestStage::Completed));
    let after_q4 = first.world_snapshot();
    assert!(!after_q4
        .inventory_items
        .iter()
        .any(|item| item.name == "DeerMeat"));
    assert!(
        after_q4
            .inventory_items
            .iter()
            .any(|item| item.name == "OldCopperRing")
            || after_q4
                .equipment_items
                .iter()
                .any(|item| item.name == "OldCopperRing")
    );

    let before_logout = first.world_snapshot();
    let before_player = player(&first);
    let logout = first.handle_packet(ClientPacket::LogOut);
    assert!(logout
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
    drop(first);

    let mut second = GatewaySession::new(file_backed_crystal_gateway_config(save_path.clone()));
    login(&mut second, &account_id, password);
    let restarted = second.handle_packet(ClientPacket::StartGame { character_index });
    assert!(restarted
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    let after_reload = second.world_snapshot();
    let reloaded_player = player(&second);
    assert_eq!(after_reload.gold, before_logout.gold);
    assert_eq!(
        after_reload.player_experience,
        before_logout.player_experience
    );
    assert_eq!(
        after_reload.player_max_experience,
        before_logout.player_max_experience
    );
    assert_eq!(after_reload.inventory_items, before_logout.inventory_items);
    assert_eq!(after_reload.belt_items, before_logout.belt_items);
    assert_eq!(after_reload.equipment_items, before_logout.equipment_items);
    assert_eq!(after_reload.quest_log, before_logout.quest_log);
    assert_eq!(
        (
            reloaded_player.x,
            reloaded_player.y,
            reloaded_player.direction,
            reloaded_player.level,
            reloaded_player.class,
        ),
        (
            before_player.x,
            before_player.y,
            before_player.direction,
            before_player.level,
            before_player.class,
        )
    );
    for quest_id in 1..=4 {
        assert_eq!(quest_stage(&second, quest_id), Some(QuestStage::Completed));
    }

    drop(second);
    drop(save_guard);
    assert!(!save_path.exists(), "Gateway save file should be removed");
}
