//! Same-map movement/AOI load harness for the shared `ZoneRuntime`.
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
//! - Single-process, in-memory. Server-packet payload bytes are encoded and
//!   reported, but TCP/RPC framing, retransmits, TLS, Gateway work, persistence,
//!   monsters, and combat are not included. Treat the result as a benchmark
//!   profile, never as a production capacity certificate.
//! - Debug builds are several times slower than `--release`; always measure
//!   with `--release` for capacity numbers.

use std::fs;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use mir2_protocol::{encode_server_packet, MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    SessionId, ZoneChatProfile, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey, ZoneManager,
    ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};
use serde::Serialize;

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

fn env_string(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StepResult {
    players: usize,
    ticks: u64,
    mean_ms: f64,
    p50_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
    max_ms: f64,
    ms_per_player: f64,
    logical_packet_deliveries: u64,
    encoded_payload_bytes: u64,
    packet_encode_errors: u64,
    modeled_egress_mbps: f64,
    rss_after_bytes: Option<u64>,
}

fn run_step(players: usize, ticks: u64, command_interval_ms: u64) -> StepResult {
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
    let mut logical_packet_deliveries = 0_u64;
    let mut encoded_payload_bytes = 0_u64;
    let mut packet_encode_errors = 0_u64;
    let mut now_ms: u64 = 1;
    for t in 0..ticks {
        // Every player issues a walk this tick, cycling direction so they keep
        // crossing AOI/grid-cell boundaries (max visibility churn). The
        // configured command interval must clear ZONE_WALK_DELAY so each walk
        // commits; the container profile uses 700 ms.
        let dir = DIRECTIONS[(t as usize) % DIRECTIONS.len()];
        let start = Instant::now();
        let mut outbounds = Vec::new();
        for session in &sessions {
            outbounds.extend(zone.handle(ZoneCommand::Walk {
                session_id: session.clone(),
                direction: dir,
                seq: t + 1,
                now_ms,
            }));
        }

        outbounds.extend(zone.tick(now_ms));
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        let wire = outbound_wire_totals(&outbounds, players);
        logical_packet_deliveries += wire.logical_packet_deliveries;
        encoded_payload_bytes += wire.encoded_payload_bytes;
        packet_encode_errors += wire.packet_encode_errors;
        now_ms += command_interval_ms;
    }

    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean_ms = samples.iter().sum::<f64>() / samples.len() as f64;
    let simulated_seconds = ticks as f64 * command_interval_ms as f64 / 1_000.0;
    let max_ms = *samples.last().unwrap();
    StepResult {
        players,
        ticks,
        mean_ms,
        p50_ms: percentile(&samples, 50),
        p95_ms: percentile(&samples, 95),
        p99_ms: percentile(&samples, 99),
        max_ms,
        ms_per_player: mean_ms / players as f64,
        logical_packet_deliveries,
        encoded_payload_bytes,
        packet_encode_errors,
        modeled_egress_mbps: encoded_payload_bytes as f64 * 8.0 / simulated_seconds / 1_000_000.0,
        rss_after_bytes: process_rss_bytes(),
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct WireTotals {
    logical_packet_deliveries: u64,
    encoded_payload_bytes: u64,
    packet_encode_errors: u64,
}

fn outbound_wire_totals(outbounds: &[ZoneOutbound], audience_size: usize) -> WireTotals {
    let mut totals = WireTotals::default();
    for outbound in outbounds {
        let (recipients, packets): (usize, &[ServerPacket]) = match outbound {
            ZoneOutbound::ToSession { packets, .. } => (1, packets),
            ZoneOutbound::ToMany {
                session_ids,
                packets,
            } => (session_ids.len(), packets),
            ZoneOutbound::ToAll { packets } => (audience_size, packets),
            _ => continue,
        };
        for packet in packets {
            match encode_server_packet(packet) {
                Ok(encoded) => {
                    totals.logical_packet_deliveries += recipients as u64;
                    totals.encoded_payload_bytes += encoded.len() as u64 * recipients as u64;
                }
                Err(_) => totals.packet_encode_errors += recipients as u64,
            }
        }
    }
    totals
}

fn percentile(sorted: &[f64], percentile: usize) -> f64 {
    let index = sorted
        .len()
        .saturating_mul(percentile)
        .div_ceil(100)
        .saturating_sub(1)
        .min(sorted.len().saturating_sub(1));
    sorted.get(index).copied().unwrap_or_default()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MultiZoneResult {
    zones: usize,
    players_per_zone: usize,
    total_players: usize,
    ticks: u64,
    parallel_mean_ms: f64,
    parallel_p95_ms: f64,
    sequential_mean_ms: f64,
    sequential_p95_ms: f64,
    speedup: f64,
    logical_packet_deliveries: u64,
    encoded_payload_bytes: u64,
    packet_encode_errors: u64,
    modeled_egress_mbps: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HardwareProfile {
    label: String,
    requested_cpu_cores: String,
    requested_memory_bytes: u64,
    requested_network_egress_mbps: f64,
    requested_disk_bytes: u64,
    cgroup_cpu_max: Option<String>,
    cgroup_memory_max: Option<String>,
    available_parallelism: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Recommendation {
    safety_bps: u64,
    safe_network_budget_mbps: f64,
    max_tested_compute_players: Option<usize>,
    max_tested_network_players: Option<usize>,
    max_tested_combined_players: Option<usize>,
    max_tested_combined_zones: Option<usize>,
    max_tested_combined_total_players: Option<usize>,
    max_tested_combined_players_per_zone: Option<usize>,
    status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CapacityProfileReport {
    schema_version: u32,
    generated_at_unix_ms: u128,
    workload: &'static str,
    build: &'static str,
    tick_budget_ms: f64,
    command_interval_ms: u64,
    hardware: HardwareProfile,
    recommendation: Recommendation,
    single_zone: Vec<StepResult>,
    multi_zone: Vec<MultiZoneResult>,
    caveats: Vec<&'static str>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let steps = env_usize_list("MIR2_LOAD_STEPS", &[25, 50, 100, 200, 400]);
    let ticks = env_u64("MIR2_LOAD_TICKS", 120);
    let budget_ms = env_u64("MIR2_LOAD_BUDGET_MS", 100) as f64;
    let command_interval_ms = env_u64("MIR2_LOAD_COMMAND_INTERVAL_MS", 700);
    let network_egress_mbps = env_u64("DUBHE_PROFILE_NETWORK_EGRESS_MBPS", 0) as f64;
    let safety_bps = env_u64("DUBHE_PROFILE_SAFETY_BPS", 7_000).min(10_000);

    if cfg!(debug_assertions) {
        eprintln!(
            "WARNING: debug build — numbers are several times slower than release. \
             Use `cargo run --release --example zone_load` for capacity figures.\n"
        );
    }

    println!(
        "Same-map zone load: {} ticks/step, budget {:.0} ms/tick, command interval {} ms, all players walking each tick\n",
        ticks, budget_ms, command_interval_ms
    );
    println!(
        "{:>8}  {:>10}  {:>10}  {:>10}  {:>14}  {:>12}",
        "players", "mean ms", "p95 ms", "max ms", "ms/player", "egress Mbps"
    );

    let mut knee: Option<usize> = None;
    let mut single_zone = Vec::new();
    for players in steps {
        let r = run_step(players, ticks, command_interval_ms);
        let flag = if r.p95_ms > budget_ms {
            "  <-- over budget"
        } else {
            ""
        };
        println!(
            "{:>8}  {:>10.3}  {:>10.3}  {:>10.3}  {:>14.4}  {:>12.3}{}",
            r.players, r.mean_ms, r.p95_ms, r.max_ms, r.ms_per_player, r.modeled_egress_mbps, flag
        );
        if knee.is_none() && r.p95_ms > budget_ms {
            knee = Some(r.players);
        }
        single_zone.push(r);
    }

    println!();
    match knee {
        Some(n) => println!(
            "Knee: p95 tick time first exceeds {:.0} ms at ~{} players in one dense zone.",
            budget_ms, n
        ),
        None => println!(
            "No knee reached within tested steps: all stayed under {:.0} ms/tick. \
             Raise MIR2_LOAD_STEPS to find the ceiling.",
            budget_ms
        ),
    }
    println!(
        "The knee is the single-Zone compute boundary only. Use the combined \
         recommendation for a profiled network link."
    );

    let multi_zone = run_multizone_scaling();
    let safe_network_budget_mbps = network_egress_mbps * safety_bps as f64 / 10_000.0;
    let max_tested_compute_players = single_zone
        .iter()
        .filter(|result| result.p95_ms <= budget_ms)
        .map(|result| result.players)
        .max();
    let max_tested_network_players = (network_egress_mbps > 0.0)
        .then(|| {
            single_zone
                .iter()
                .filter(|result| result.modeled_egress_mbps <= safe_network_budget_mbps)
                .map(|result| result.players)
                .max()
        })
        .flatten();
    let max_tested_combined_players = single_zone
        .iter()
        .filter(|result| {
            result.p95_ms <= budget_ms
                && (network_egress_mbps <= 0.0
                    || result.modeled_egress_mbps <= safe_network_budget_mbps)
        })
        .map(|result| result.players)
        .max();
    let max_tested_multi_zone = multi_zone
        .iter()
        .filter(|result| {
            result.parallel_p95_ms <= budget_ms
                && (network_egress_mbps <= 0.0
                    || result.modeled_egress_mbps <= safe_network_budget_mbps)
        })
        .max_by_key(|result| (result.total_players, result.zones));
    let report = CapacityProfileReport {
        schema_version: 1,
        generated_at_unix_ms: SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
        workload: "dense-same-map-all-players-walk-plus-multi-zone-tick",
        build: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        tick_budget_ms: budget_ms,
        command_interval_ms,
        hardware: HardwareProfile {
            label: env_string("DUBHE_PROFILE_LABEL", "unprofiled"),
            requested_cpu_cores: env_string("DUBHE_PROFILE_CPU_CORES", "unlimited"),
            requested_memory_bytes: env_u64("DUBHE_PROFILE_MEMORY_BYTES", 0),
            requested_network_egress_mbps: network_egress_mbps,
            requested_disk_bytes: env_u64("DUBHE_PROFILE_DISK_BYTES", 0),
            cgroup_cpu_max: read_trimmed("/sys/fs/cgroup/cpu.max"),
            cgroup_memory_max: read_trimmed("/sys/fs/cgroup/memory.max"),
            available_parallelism: std::thread::available_parallelism()
                .map(|value| value.get())
                .unwrap_or(1),
        },
        recommendation: Recommendation {
            safety_bps,
            safe_network_budget_mbps,
            max_tested_compute_players,
            max_tested_network_players,
            max_tested_combined_players,
            max_tested_combined_zones: max_tested_multi_zone.map(|result| result.zones),
            max_tested_combined_total_players: max_tested_multi_zone
                .map(|result| result.total_players),
            max_tested_combined_players_per_zone: max_tested_multi_zone
                .map(|result| result.players_per_zone),
            status: "benchmark-only-not-production-certified",
        },
        single_zone,
        multi_zone,
        caveats: vec![
            "Encoded payload bytes exclude TCP/IP, TLS, and Zone RPC framing overhead.",
            "The workload models dense movement and AOI fan-out; monsters, combat, persistence, Gateway CPU, and failover are excluded.",
            "Disk capacity is recorded as profile metadata because Zone Runtime is in-memory and does not benchmark persistent storage.",
            "The combined recommendation is the largest passing tested step, not an interpolated or extrapolated ceiling.",
        ],
    };
    if let Ok(output) = std::env::var("MIR2_LOAD_OUT") {
        let output = PathBuf::from(output);
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            &output,
            format!("{}\n", serde_json::to_string_pretty(&report)?),
        )?;
        println!("Wrote {}", output.display());
    }
    Ok(())
}

/// Stage-A multi-core proof: build `zones` independent zones (each = one map
/// with a crowd), and tick them all via `ZoneManager::tick_all` (parallel) vs a
/// per-zone sequential baseline. Because zones share no state, throughput should
/// scale with cores — this is the win that map=zone routing unlocks.
fn run_multizone_scaling() -> Vec<MultiZoneResult> {
    let zones = env_usize_list("MIR2_LOAD_ZONES", &[1, 2, 4, 8, 16]);
    let player_steps = env_usize_list("MIR2_LOAD_ZONE_PLAYER_STEPS", &[150]);
    let ticks = env_u64("MIR2_LOAD_ZONE_TICKS", 40);
    let command_interval_ms = env_u64("MIR2_LOAD_COMMAND_INTERVAL_MS", 700);
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    let mut results = Vec::new();
    for players_per_zone in player_steps {
        println!(
            "\nMulti-zone scaling: {players_per_zone} players/zone, {ticks} ticks, \
             {cores} cores available (parallel tick_all vs sequential baseline)\n"
        );
        println!(
            "{:>6}  {:>13}  {:>13}  {:>8}  {:>12}",
            "zones", "parallel ms", "sequential ms", "speedup", "egress Mbps"
        );
        for z in &zones {
            let par = time_multizone(*z, players_per_zone, ticks, command_interval_ms, true);
            let seq = time_multizone(*z, players_per_zone, ticks, command_interval_ms, false);
            let parallel_mean_ms = par.samples.iter().sum::<f64>() / par.samples.len() as f64;
            let sequential_mean_ms = seq.samples.iter().sum::<f64>() / seq.samples.len() as f64;
            let speedup = if parallel_mean_ms > 0.0 {
                sequential_mean_ms / parallel_mean_ms
            } else {
                0.0
            };
            let simulated_seconds = ticks as f64 * command_interval_ms as f64 / 1_000.0;
            let modeled_egress_mbps =
                par.wire.encoded_payload_bytes as f64 * 8.0 / simulated_seconds / 1_000_000.0;
            println!(
                "{:>6}  {:>13.2}  {:>13.2}  {speedup:>7.2}x  {:>12.3}",
                z, parallel_mean_ms, sequential_mean_ms, modeled_egress_mbps
            );
            let mut par_sorted = par.samples;
            let mut seq_sorted = seq.samples;
            par_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            seq_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            results.push(MultiZoneResult {
                zones: *z,
                players_per_zone,
                total_players: *z * players_per_zone,
                ticks,
                parallel_mean_ms,
                parallel_p95_ms: percentile(&par_sorted, 95),
                sequential_mean_ms,
                sequential_p95_ms: percentile(&seq_sorted, 95),
                speedup,
                logical_packet_deliveries: par.wire.logical_packet_deliveries,
                encoded_payload_bytes: par.wire.encoded_payload_bytes,
                packet_encode_errors: par.wire.packet_encode_errors,
                modeled_egress_mbps,
            });
        }
    }
    println!(
        "\nThe multi-Zone curve includes serial command ingestion plus parallel Zone ticks. \
         Speedup below 1x means tick parallelism does not offset scheduling overhead for that \
         workload; do not infer node capacity from core count alone."
    );
    results
}

struct TimedMultiZone {
    samples: Vec<f64>,
    wire: WireTotals,
}

fn time_multizone(
    zones: usize,
    players_per_zone: usize,
    ticks: u64,
    command_interval_ms: u64,
    parallel: bool,
) -> TimedMultiZone {
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

    let mut samples = Vec::with_capacity(ticks as usize);
    let mut wire = WireTotals::default();
    let mut now_ms = 1;
    for t in 0..ticks {
        // Drive every player in every zone so the parallel and sequential paths
        // see the same dense AOI fan-out as the single-zone curve.
        let dir = DIRECTIONS[(t as usize) % DIRECTIONS.len()];
        let started = Instant::now();
        let mut outbounds = Vec::new();
        for z in 0..zones {
            for i in 0..players_per_zone {
                outbounds.extend(mgr.handle_for_key(
                    ZoneKey::for_map(format!("loadmap{z}")),
                    ZoneCommand::Walk {
                        session_id: SessionId::new(format!("load-{}", z * players_per_zone + i)),
                        direction: dir,
                        seq: t + 1,
                        now_ms,
                    },
                ));
            }
        }
        if parallel {
            outbounds.extend(mgr.tick_all(now_ms));
        } else {
            outbounds.extend(mgr.tick_all_sequential(now_ms));
        }
        samples.push(started.elapsed().as_secs_f64() * 1_000.0);
        let tick_wire = outbound_wire_totals(&outbounds, players_per_zone);
        wire.logical_packet_deliveries += tick_wire.logical_packet_deliveries;
        wire.encoded_payload_bytes += tick_wire.encoded_payload_bytes;
        wire.packet_encode_errors += tick_wire.packet_encode_errors;
        now_ms += command_interval_ms;
    }
    TimedMultiZone { samples, wire }
}

fn read_trimmed(path: &str) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
}

fn process_rss_bytes() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let kib = status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))?
        .split_whitespace()
        .next()?
        .parse::<u64>()
        .ok()?;
    Some(kib.saturating_mul(1_024))
}
