//! Same-map combat/movement load harness for the shared `ZoneRuntime`.
//!
//! This is the capacity *measurement ruler* called for in
//! `docs/SCALABILITY-AND-CAPACITY.md`: it drives a real `ZoneRuntime` with N
//! players crowded into one map, all moving every tick (the realistic
//! same-screen broadcast load), and reports the per-tick wall-clock cost as the
//! population steps up. The "knee" — where mean tick time crosses a budget
//! (default 100 ms) — is the single-core capacity of one zone, which is what
//! you divide a map's peak on-screen load by to size cores per map.
//!
//! It deliberately exercises the path L1 optimized (`diff_visibility_for` /
//! `diff_zone_object_visibility_for`): players packed within AOI range of each
//! other maximize visibility fan-out, so this both measures capacity and guards
//! against AOI regressions.
//!
//! Run:
//!   cargo run --release --example zone_load
//!   MIR2_LOAD_STEPS=50,100,200,400 MIR2_LOAD_TICKS=200 \
//!     MIR2_LOAD_BUDGET_MS=100 cargo run --release --example zone_load
//!
//! Honesty notes:
//! - Single-process, in-memory; no network/serialization cost is included, so
//!   real deployments will be *slower* than these numbers — treat the knee as
//!   an optimistic upper bound.
//! - Debug builds are several times slower than `--release`; always measure
//!   with `--release` for capacity numbers.

use std::time::Instant;

use mir2_protocol::{MirClass, MirDirection, MirGender, Point};
use mir2_simulation::{
    SessionId, ZoneChatProfile, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey, ZoneManager,
    ZonePlayerCombatStats, ZoneRuntime,
};

const DIRECTIONS: [MirDirection; 4] = [
    MirDirection::Right,
    MirDirection::Down,
    MirDirection::Left,
    MirDirection::Up,
];

fn env_usize_list(key: &str, default: &[usize]) -> Vec<usize> {
    match std::env::var(key) {
        Ok(raw) => raw
            .split(',')
            .filter_map(|part| part.trim().parse::<usize>().ok())
            .filter(|n| *n > 0)
            .collect(),
        Err(_) => default.to_vec(),
    }
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Pack `count` players into a roughly square block so most are within AOI
/// range of many neighbors — the dense same-screen case that stresses
/// visibility fan-out the hardest.
fn dense_block_positions(count: usize) -> Vec<Point> {
    let side = (count as f64).sqrt().ceil() as i32;
    let origin = 330;
    let mut out = Vec::with_capacity(count);
    for i in 0..count as i32 {
        // 2-tile spacing keeps neighbors inside the 18x14 AOI rectangle while
        // leaving room to step without colliding on the same tile.
        out.push(Point {
            x: origin + (i % side) * 2,
            y: origin + (i / side) * 2,
        });
    }
    out
}

fn join_at(index: usize, position: Point) -> ZoneJoin {
    ZoneJoin {
        session_id: SessionId::new(format!("load-{index}")),
        account_id: format!("load-{index}-acct"),
        character_index: index as i32,
        object_id: 1 + index as u32,
        name: format!("Bot{index}"),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 7,
        hp: 60,
        max_hp: 60,
        mp: 100,
        map_file_name: "0".to_string(),
        position,
        direction: MirDirection::Down,
        chat_profile: ZoneChatProfile::default(),
        combat_stats: ZonePlayerCombatStats::default(),
    }
}

struct StepResult {
    players: usize,
    mean_ms: f64,
    p95_ms: f64,
    max_ms: f64,
    ticks_per_player_ms: f64,
}

fn run_step(players: usize, ticks: u64) -> StepResult {
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());

    let positions = dense_block_positions(players);
    let sessions: Vec<SessionId> = (0..players)
        .map(|i| SessionId::new(format!("load-{i}")))
        .collect();
    for (i, pos) in positions.iter().enumerate() {
        zone.handle(ZoneCommand::Join(join_at(i, pos.clone())));
    }

    // Warm-up tick so first-touch allocations don't skew the first sample.
    let _ = zone.tick(0);

    let mut samples = Vec::with_capacity(ticks as usize);
    let mut now_ms: u64 = 1;
    for t in 0..ticks {
        // Every player issues a walk this tick, cycling direction so they keep
        // crossing AOI/grid-cell boundaries (max visibility churn). 700ms/tick
        // step clears ZONE_WALK_DELAY so each walk commits.
        let dir = DIRECTIONS[(t as usize) % DIRECTIONS.len()];
        for (i, session) in sessions.iter().enumerate() {
            zone.handle(ZoneCommand::Walk {
                session_id: session.clone(),
                direction: dir,
                seq: t + 1,
                now_ms,
            });
            // Nudge seq uniqueness per player without extra cost.
            let _ = i;
        }

        let start = Instant::now();
        let _ = zone.tick(now_ms);
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        now_ms += 700;
    }

    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean_ms = samples.iter().sum::<f64>() / samples.len() as f64;
    let p95_ms = samples[((samples.len() as f64 * 0.95) as usize).min(samples.len() - 1)];
    let max_ms = *samples.last().unwrap();
    StepResult {
        players,
        mean_ms,
        p95_ms,
        max_ms,
        ticks_per_player_ms: mean_ms / players as f64,
    }
}

fn main() {
    let steps = env_usize_list("MIR2_LOAD_STEPS", &[25, 50, 100, 200, 400]);
    let ticks = env_u64("MIR2_LOAD_TICKS", 120);
    let budget_ms = env_u64("MIR2_LOAD_BUDGET_MS", 100) as f64;

    if cfg!(debug_assertions) {
        eprintln!(
            "WARNING: debug build — numbers are several times slower than release. \
             Use `cargo run --release --example zone_load` for capacity figures.\n"
        );
    }

    println!(
        "Same-map zone load: {} ticks/step, budget {:.0} ms/tick, all players walking each tick\n",
        ticks, budget_ms
    );
    println!(
        "{:>8}  {:>10}  {:>10}  {:>10}  {:>14}",
        "players", "mean ms", "p95 ms", "max ms", "ms/player"
    );

    let mut knee: Option<usize> = None;
    for players in steps {
        let r = run_step(players, ticks);
        let flag = if r.mean_ms > budget_ms {
            "  <-- over budget"
        } else {
            ""
        };
        println!(
            "{:>8}  {:>10.3}  {:>10.3}  {:>10.3}  {:>14.4}{}",
            r.players, r.mean_ms, r.p95_ms, r.max_ms, r.ticks_per_player_ms, flag
        );
        if knee.is_none() && r.mean_ms > budget_ms {
            knee = Some(r.players);
        }
    }

    println!();
    match knee {
        Some(n) => println!(
            "Knee: mean tick time first exceeds {:.0} ms at ~{} players in one zone (single core).",
            budget_ms, n
        ),
        None => println!(
            "No knee reached within tested steps: all stayed under {:.0} ms/tick. \
             Raise MIR2_LOAD_STEPS to find the ceiling.",
            budget_ms
        ),
    }
    println!(
        "Sizing: cores_per_map ~= peak_concurrent_players_on_map / knee. \
         Add network/serialization headroom (this harness is in-process only)."
    );

    run_multizone_scaling();
}

/// Stage-A multi-core proof: build `zones` independent zones (each = one map
/// with a crowd), and tick them all via `ZoneManager::tick_all` (parallel) vs a
/// per-zone sequential baseline. Because zones share no state, throughput should
/// scale with cores — this is the win that map=zone routing unlocks.
fn run_multizone_scaling() {
    let zones = env_usize_list("MIR2_LOAD_ZONES", &[1, 2, 4, 8, 16]);
    let players_per_zone = env_u64("MIR2_LOAD_ZONE_PLAYERS", 150) as usize;
    let ticks = env_u64("MIR2_LOAD_ZONE_TICKS", 40);
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    println!(
        "\nMulti-zone scaling: {players_per_zone} players/zone, {ticks} ticks, \
         {cores} cores available (parallel tick_all vs sequential baseline)\n"
    );
    println!(
        "{:>6}  {:>13}  {:>13}  {:>8}",
        "zones", "parallel ms", "sequential ms", "speedup"
    );

    for z in zones {
        let par = time_multizone(z, players_per_zone, ticks, true);
        let seq = time_multizone(z, players_per_zone, ticks, false);
        let speedup = if par > 0.0 { seq / par } else { 0.0 };
        println!("{z:>6}  {par:>13.2}  {seq:>13.2}  {speedup:>7.2}x");
    }
    println!(
        "\nLinear speedup => zones are truly independent (no shared lock/state). \
         Production needs map=zone routing in the gateway to USE this; today the \
         gateway runs a single 'primary' zone. See docs/L2-ECS-ZONE-DESIGN.md."
    );
}

fn time_multizone(zones: usize, players_per_zone: usize, ticks: u64, parallel: bool) -> f64 {
    let mut mgr = ZoneManager::new();
    for z in 0..zones {
        let map = format!("loadmap{z}");
        let block = dense_block_positions(players_per_zone);
        for (i, pos) in block.iter().enumerate() {
            let mut j = join_at(z * players_per_zone + i, pos.clone());
            j.map_file_name = map.clone();
            mgr.join(j);
        }
    }
    let _ = mgr.tick_all(0); // warm-up

    let start = Instant::now();
    let mut now_ms = 1;
    for t in 0..ticks {
        // Drive a walk in every zone so each has real per-tick work.
        let dir = DIRECTIONS[(t as usize) % DIRECTIONS.len()];
        for z in 0..zones {
            mgr.handle_for_key(
                ZoneKey::for_map(format!("loadmap{z}")),
                ZoneCommand::Walk {
                    session_id: SessionId::new(format!("load-{}", z * players_per_zone)),
                    direction: dir,
                    seq: t + 1,
                    now_ms,
                },
            );
        }
        if parallel {
            let _ = mgr.tick_all(now_ms);
        } else {
            let _ = mgr.tick_all_sequential(now_ms);
        }
        now_ms += 700;
    }
    start.elapsed().as_secs_f64() * 1000.0
}
