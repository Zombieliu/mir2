//! Gate 16.6 capacity and steady-state replication certification.
//!
//! The history comparison deliberately measures a production-shaped cycle:
//! install a periodic base at N-64, then compare one 64-mutation v5 delta
//! against a v4 full-history checkpoint at N. Cold base installation is
//! reported separately and is not hidden inside the steady-state result.

use std::env;
use std::fs;
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mir2_gateway::{
    GatewayConfig, InMemoryZoneOwnerLeaseAuthority, SharedInProcessZoneRuntimeFactory,
    SharedZoneOwnerLeaseAuthority, TcpZoneOwnerRpcTransport, ZoneHostServer, ZoneId,
    ZoneOwnerCommandRequest, ZoneOwnerLease, ZoneOwnerLeaseAuthority, ZoneOwnerRpcTransport,
    ZoneRpcLimits, DEFAULT_ZONE_REPLICATION_MAX_BATCH_BYTES,
    DEFAULT_ZONE_REPLICATION_MAX_BATCH_ENTRIES, ZONE_RPC_PROTOCOL_VERSION,
};
use mir2_protocol::ClientPacket;
use mir2_simulation::WorldCommand;
use serde::Serialize;

const DEFAULT_OUTPUT: &str = "docs/generated/gate16/v5-certification.json";
const DELTA_ENTRIES: usize = 64;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CertificationReport {
    schema_version: u32,
    generated_at_unix_ms: u128,
    build: &'static str,
    zone_rpc_protocol_version: u16,
    environment: Environment,
    player_results: Vec<PlayerResult>,
    history_results: Vec<HistoryResult>,
    assertions: Assertions,
    success: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Environment {
    profile_label: Option<String>,
    requested_cpu_cores: Option<String>,
    requested_memory_bytes: Option<String>,
    cgroup_cpu_max: Option<String>,
    cgroup_memory_max: Option<String>,
    available_parallelism: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlayerResult {
    requested_players: usize,
    connected_players: usize,
    commands_per_player: usize,
    completed_commands: usize,
    wall_ms: f64,
    throughput_commands_per_second: f64,
    latency_ms: NumericSummary,
    success: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HistoryResult {
    history_entries: usize,
    base_sequence: u64,
    delta_entries: usize,
    base_snapshot_wire_bytes: usize,
    base_install_wall_ms: f64,
    v5_delta_wire_bytes: usize,
    v5_apply_wall_ms: f64,
    v5_apply_cpu_ticks: Option<u64>,
    v4_checkpoint_wire_bytes: usize,
    v4_install_wall_ms: f64,
    v4_install_cpu_ticks: Option<u64>,
    network_reduction_percent: f64,
    wall_reduction_percent: f64,
    cpu_reduction_percent: Option<f64>,
    heads_match: bool,
    success: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Assertions {
    player_profiles_pass: bool,
    history_profiles_pass: bool,
    network_reduction_at_least_80_percent: bool,
    cpu_reduction_at_least_80_percent: bool,
    wall_reduction_at_least_80_percent: bool,
    cgroup_profile_matches: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NumericSummary {
    count: usize,
    min: Option<f64>,
    mean: Option<f64>,
    p50: Option<f64>,
    p95: Option<f64>,
    p99: Option<f64>,
    max: Option<f64>,
}

struct RunningHost {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl RunningHost {
    fn start(
        authority: SharedZoneOwnerLeaseAuthority,
        limits: ZoneRpcLimits,
    ) -> Result<Self, String> {
        let listener =
            TcpListener::bind("127.0.0.1:0").map_err(|error| format!("bind Zone Host: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("read Zone Host address: {error}"))?;
        let server = Arc::new(ZoneHostServer::with_options_and_factory(
            GatewayConfig::default(),
            authority,
            None,
            limits,
            Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
                Duration::from_secs(24 * 60 * 60),
                Default::default(),
            )),
        ));
        let stop = Arc::new(AtomicBool::new(false));
        let running = Arc::clone(&server);
        let server_stop = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            if let Err(error) = running.serve_until(listener, server_stop) {
                eprintln!("Gate 16.6 Zone Host stopped: {error}");
            }
        });
        Ok(Self {
            address,
            stop,
            handle: Some(handle),
        })
    }
}

impl Drop for RunningHost {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = std::net::TcpStream::connect(self.address);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from(
        env::var("MIR2_GATE16_CERTIFICATION_OUT").unwrap_or_else(|_| DEFAULT_OUTPUT.to_string()),
    );
    let player_profiles = usize_list_env("MIR2_GATE16_PLAYER_PROFILES", &[50, 125]);
    let history_profiles = usize_list_env("MIR2_GATE16_HISTORY_STEPS", &[700, 10_000, 100_000]);
    let mut player_results = Vec::new();
    for players in player_profiles {
        eprintln!("Gate 16.6: {players} concurrent player Sessions");
        player_results.push(run_player_profile(players)?);
    }
    let history_results = run_history_profiles(&history_profiles)?;
    let environment = environment();
    let assertions = Assertions {
        player_profiles_pass: player_results.iter().all(|result| result.success),
        history_profiles_pass: history_results.iter().all(|result| result.success),
        network_reduction_at_least_80_percent: history_results
            .iter()
            .all(|result| result.network_reduction_percent >= 80.0),
        cpu_reduction_at_least_80_percent: history_results.iter().all(|result| {
            result
                .cpu_reduction_percent
                .is_some_and(|reduction| reduction >= 80.0)
        }),
        wall_reduction_at_least_80_percent: history_results
            .iter()
            .all(|result| result.wall_reduction_percent >= 80.0),
        cgroup_profile_matches: cgroup_profile_matches(&environment),
    };
    let success = assertions.player_profiles_pass
        && assertions.history_profiles_pass
        && assertions.network_reduction_at_least_80_percent
        && assertions.cpu_reduction_at_least_80_percent
        && assertions.wall_reduction_at_least_80_percent
        && assertions.cgroup_profile_matches;
    let report = CertificationReport {
        schema_version: 1,
        generated_at_unix_ms: unix_now_ms(),
        build: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        zone_rpc_protocol_version: ZONE_RPC_PROTOCOL_VERSION,
        environment,
        player_results,
        history_results,
        assertions,
        success,
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &output,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    println!("Wrote {}", output.display());
    if !report.success {
        std::process::exit(1);
    }
    Ok(())
}

fn run_player_profile(players: usize) -> Result<PlayerResult, String> {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let limits = benchmark_limits(players.max(256));
    let shared: SharedZoneOwnerLeaseAuthority = authority.clone();
    let host = RunningHost::start(shared, limits.clone())?;
    let zone_id = ZoneId::new(format!("capacity:{players}"));
    let lease = authority.owner_lease(&zone_id);
    let commands_per_player = 8;
    for player in 0..players {
        transport(
            host.address,
            zone_id.clone(),
            &format!("capacity-{players}-{player}"),
            limits.clone(),
        )
        .on_connect()?;
    }

    let barrier = Arc::new(Barrier::new(players.max(1)));
    let latencies = Arc::new(Mutex::new(Vec::with_capacity(
        players.saturating_mul(commands_per_player),
    )));
    let completed = Arc::new(AtomicUsize::new(0));
    let failures = Arc::new(Mutex::new(Vec::<String>::new()));
    let started = Instant::now();
    thread::scope(|scope| {
        for player in 0..players {
            let barrier = Arc::clone(&barrier);
            let latencies = Arc::clone(&latencies);
            let completed = Arc::clone(&completed);
            let failures = Arc::clone(&failures);
            let zone_id = zone_id.clone();
            let lease = lease.clone();
            let limits = limits.clone();
            scope.spawn(move || {
                let client = transport(
                    host.address,
                    zone_id,
                    &format!("capacity-{players}-{player}"),
                    limits,
                );
                barrier.wait();
                for command in 0..commands_per_player {
                    let command_started = Instant::now();
                    match client.execute(ZoneOwnerCommandRequest::direct(
                        lease.clone(),
                        WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                            time: i64::try_from(
                                player.saturating_mul(commands_per_player) + command,
                            )
                            .unwrap_or(i64::MAX),
                        }),
                    )) {
                        Ok(_) => {
                            latencies
                                .lock()
                                .unwrap()
                                .push(command_started.elapsed().as_secs_f64() * 1_000.0);
                            completed.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(error) => failures.lock().unwrap().push(error),
                    }
                }
            });
        }
    });
    let wall_ms = started.elapsed().as_secs_f64() * 1_000.0;
    let completed_commands = completed.load(Ordering::Acquire);
    let mut latency_values = Arc::try_unwrap(latencies)
        .map_err(|_| "player latency references remain".to_string())?
        .into_inner()
        .map_err(|_| "player latency mutex poisoned".to_string())?;
    let failure_count = failures.lock().map(|items| items.len()).unwrap_or(1);
    Ok(PlayerResult {
        requested_players: players,
        connected_players: players,
        commands_per_player,
        completed_commands,
        wall_ms,
        throughput_commands_per_second: if wall_ms > 0.0 {
            completed_commands as f64 / (wall_ms / 1_000.0)
        } else {
            0.0
        },
        latency_ms: summarize(&mut latency_values),
        success: failure_count == 0
            && completed_commands == players.saturating_mul(commands_per_player),
    })
}

fn run_history_profiles(profiles: &[usize]) -> Result<Vec<HistoryResult>, String> {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let limits = benchmark_limits(256);
    let shared: SharedZoneOwnerLeaseAuthority = authority.clone();
    let active = RunningHost::start(shared, limits.clone())?;
    let zone_id = ZoneId::new("history:gate16");
    let lease = authority.owner_lease(&zone_id);
    let worker_count = 32;
    for worker in 0..worker_count {
        transport(
            active.address,
            zone_id.clone(),
            &format!("history-worker-{worker}"),
            limits.clone(),
        )
        .on_connect()?;
    }

    let mut completed = 0usize;
    let mut results = Vec::new();
    for &history_entries in profiles {
        if history_entries <= DELTA_ENTRIES || history_entries <= completed {
            return Err(format!(
                "history profile {history_entries} must be increasing and greater than {DELTA_ENTRIES}"
            ));
        }
        let base_at = history_entries - DELTA_ENTRIES;
        run_history_commands(
            active.address,
            &zone_id,
            &lease,
            &limits,
            completed,
            base_at - completed,
            worker_count,
        )?;
        completed = base_at;
        let active_client = transport(
            active.address,
            zone_id.clone(),
            "history-worker-0",
            limits.clone(),
        );
        let base = active_client.export_base_snapshot()?;
        if base.base_sequence != base_at as u64 {
            return Err(format!(
                "base cursor {} does not match requested history {base_at}",
                base.base_sequence
            ));
        }
        let standby_v5 = RunningHost::start(
            Arc::new(InMemoryZoneOwnerLeaseAuthority::new()),
            limits.clone(),
        )?;
        let standby_v5_client = transport(
            standby_v5.address,
            zone_id.clone(),
            "history-worker-0",
            limits.clone(),
        );
        let base_started = Instant::now();
        standby_v5_client
            .install_base_snapshot(&base)
            .map_err(|error| {
                format!("history {history_entries} v5 base installation failed: {error}")
            })?;
        let base_install_wall_ms = base_started.elapsed().as_secs_f64() * 1_000.0;

        run_history_commands(
            active.address,
            &zone_id,
            &lease,
            &limits,
            completed,
            DELTA_ENTRIES,
            worker_count,
        )?;
        completed = history_entries;
        let batch = active_client.export_mutation_batch(
            base.base_sequence,
            DEFAULT_ZONE_REPLICATION_MAX_BATCH_ENTRIES,
            DEFAULT_ZONE_REPLICATION_MAX_BATCH_BYTES,
        )?;
        if batch.entries.len() != DELTA_ENTRIES || batch.has_more {
            return Err(format!(
                "expected one {DELTA_ENTRIES}-entry delta at {history_entries}, got {} (has_more={})",
                batch.entries.len(),
                batch.has_more
            ));
        }
        let v5_cpu_before = process_cpu_ticks();
        let v5_started = Instant::now();
        standby_v5_client
            .apply_mutation_batch(&batch)
            .map_err(|error| {
                format!("history {history_entries} v5 delta application failed: {error}")
            })?;
        let v5_apply_wall_ms = v5_started.elapsed().as_secs_f64() * 1_000.0;
        let v5_apply_cpu_ticks = cpu_tick_delta(v5_cpu_before, process_cpu_ticks());

        let checkpoint = active_client.export_host_checkpoint()?;
        let standby_v4 = RunningHost::start(
            Arc::new(InMemoryZoneOwnerLeaseAuthority::new()),
            limits.clone(),
        )?;
        let standby_v4_client = transport(
            standby_v4.address,
            zone_id.clone(),
            "history-worker-0",
            limits.clone(),
        );
        let v4_cpu_before = process_cpu_ticks();
        let v4_started = Instant::now();
        standby_v4_client
            .install_host_checkpoint(&checkpoint)
            .map_err(|error| {
                format!("history {history_entries} v4 checkpoint installation failed: {error}")
            })?;
        let v4_install_wall_ms = v4_started.elapsed().as_secs_f64() * 1_000.0;
        let v4_install_cpu_ticks = cpu_tick_delta(v4_cpu_before, process_cpu_ticks());
        let active_head = active_client.replication_head()?;
        let standby_head = standby_v5_client.replication_head()?;
        let heads_match = active_head.next_sequence == standby_head.next_sequence
            && active_head.latest_digest == standby_head.latest_digest;
        let v5_delta_wire_bytes = serde_json::to_vec(&batch)
            .map_err(|error| format!("encode v5 delta: {error}"))?
            .len();
        let v4_checkpoint_wire_bytes = checkpoint.as_bytes().len();
        let network_reduction_percent =
            reduction_percent(v5_delta_wire_bytes as f64, v4_checkpoint_wire_bytes as f64);
        let wall_reduction_percent = reduction_percent(v5_apply_wall_ms, v4_install_wall_ms);
        let cpu_reduction_percent = match (v5_apply_cpu_ticks, v4_install_cpu_ticks) {
            (Some(v5), Some(v4)) if v4 > 0 => Some(reduction_percent(v5 as f64, v4 as f64)),
            _ => None,
        };
        let success = heads_match
            && network_reduction_percent >= 80.0
            && wall_reduction_percent >= 80.0
            && cpu_reduction_percent.is_some_and(|value| value >= 80.0);
        eprintln!(
            "Gate 16.6 history={history_entries}: network={network_reduction_percent:.2}% wall={wall_reduction_percent:.2}% cpu={cpu_reduction_percent:?}"
        );
        results.push(HistoryResult {
            history_entries,
            base_sequence: base.base_sequence,
            delta_entries: batch.entries.len(),
            base_snapshot_wire_bytes: serde_json::to_vec(&base)
                .map_err(|error| format!("encode base snapshot: {error}"))?
                .len(),
            base_install_wall_ms,
            v5_delta_wire_bytes,
            v5_apply_wall_ms,
            v5_apply_cpu_ticks,
            v4_checkpoint_wire_bytes,
            v4_install_wall_ms,
            v4_install_cpu_ticks,
            network_reduction_percent,
            wall_reduction_percent,
            cpu_reduction_percent,
            heads_match,
            success,
        });
    }
    Ok(results)
}

fn run_history_commands(
    address: SocketAddr,
    zone_id: &ZoneId,
    lease: &ZoneOwnerLease,
    limits: &ZoneRpcLimits,
    sequence_start: usize,
    count: usize,
    worker_count: usize,
) -> Result<(), String> {
    let next = Arc::new(AtomicUsize::new(0));
    let failure = Arc::new(Mutex::new(None::<String>));
    thread::scope(|scope| {
        for worker in 0..worker_count {
            let next = Arc::clone(&next);
            let failure = Arc::clone(&failure);
            let lease = lease.clone();
            let zone_id = zone_id.clone();
            let limits = limits.clone();
            scope.spawn(move || {
                let client = transport(
                    address,
                    zone_id,
                    &format!("history-worker-{worker}"),
                    limits,
                );
                loop {
                    let offset = next.fetch_add(1, Ordering::Relaxed);
                    if offset >= count || failure.lock().unwrap().is_some() {
                        break;
                    }
                    if let Err(error) = client.execute(ZoneOwnerCommandRequest::direct(
                        lease.clone(),
                        WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                            time: i64::try_from(sequence_start.saturating_add(offset))
                                .unwrap_or(i64::MAX),
                        }),
                    )) {
                        *failure.lock().unwrap() = Some(error);
                        break;
                    }
                }
            });
        }
    });
    if let Some(error) = failure
        .lock()
        .map_err(|_| "history failure mutex poisoned")?
        .take()
    {
        return Err(error);
    }
    Ok(())
}

fn transport(
    address: SocketAddr,
    zone_id: ZoneId,
    session_id: &str,
    limits: ZoneRpcLimits,
) -> TcpZoneOwnerRpcTransport {
    TcpZoneOwnerRpcTransport::with_options(address.to_string(), zone_id, session_id, None, limits)
        .with_connection_reuse()
}

fn benchmark_limits(capacity: usize) -> ZoneRpcLimits {
    ZoneRpcLimits {
        max_frame_bytes: 256 * 1024 * 1024,
        max_connections: capacity.max(256),
        max_sessions: capacity.max(256),
        max_sessions_per_zone: capacity.max(256),
        // The deliberately slow 100k v4 full-history replay is the baseline
        // Gate 16 must beat. It can legitimately exceed ten minutes inside a
        // constrained 2C2G container, so the certification deadline must not
        // truncate the very cost being measured.
        io_timeout: Duration::from_secs(30 * 60),
        ..ZoneRpcLimits::default()
    }
}

fn reduction_percent(smaller: f64, baseline: f64) -> f64 {
    if baseline <= 0.0 {
        return 0.0;
    }
    ((1.0 - smaller / baseline) * 100.0).clamp(-10_000.0, 100.0)
}

fn process_cpu_ticks() -> Option<u64> {
    let stat = fs::read_to_string("/proc/self/stat").ok()?;
    let fields = stat
        .rsplit_once(") ")?
        .1
        .split_whitespace()
        .collect::<Vec<_>>();
    let user = fields.get(11)?.parse::<u64>().ok()?;
    let system = fields.get(12)?.parse::<u64>().ok()?;
    user.checked_add(system)
}

fn cpu_tick_delta(before: Option<u64>, after: Option<u64>) -> Option<u64> {
    Some(after?.saturating_sub(before?))
}

fn summarize(samples: &mut [f64]) -> NumericSummary {
    samples.sort_by(|left, right| left.total_cmp(right));
    if samples.is_empty() {
        return NumericSummary {
            count: 0,
            min: None,
            mean: None,
            p50: None,
            p95: None,
            p99: None,
            max: None,
        };
    }
    NumericSummary {
        count: samples.len(),
        min: samples.first().copied(),
        mean: Some(samples.iter().sum::<f64>() / samples.len() as f64),
        p50: percentile(samples, 50),
        p95: percentile(samples, 95),
        p99: percentile(samples, 99),
        max: samples.last().copied(),
    }
}

fn percentile(sorted: &[f64], percentile: usize) -> Option<f64> {
    let index = sorted
        .len()
        .saturating_mul(percentile)
        .div_ceil(100)
        .saturating_sub(1)
        .min(sorted.len().saturating_sub(1));
    sorted.get(index).copied()
}

fn usize_list_env(name: &str, defaults: &[usize]) -> Vec<usize> {
    env::var(name)
        .ok()
        .map(|value| {
            value
                .split(',')
                .filter_map(|part| part.trim().parse::<usize>().ok())
                .filter(|value| *value > 0)
                .collect::<Vec<_>>()
        })
        .filter(|values| !values.is_empty())
        .unwrap_or_else(|| defaults.to_vec())
}

fn environment() -> Environment {
    Environment {
        profile_label: env::var("GATE16_PROFILE_LABEL").ok(),
        requested_cpu_cores: env::var("GATE16_PROFILE_CPU_CORES").ok(),
        requested_memory_bytes: env::var("GATE16_PROFILE_MEMORY_BYTES").ok(),
        cgroup_cpu_max: fs::read_to_string("/sys/fs/cgroup/cpu.max")
            .ok()
            .map(|value| value.trim().to_string()),
        cgroup_memory_max: fs::read_to_string("/sys/fs/cgroup/memory.max")
            .ok()
            .map(|value| value.trim().to_string()),
        available_parallelism: thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1),
    }
}

fn cgroup_profile_matches(environment: &Environment) -> bool {
    let memory_matches = environment
        .requested_memory_bytes
        .as_ref()
        .zip(environment.cgroup_memory_max.as_ref())
        .is_some_and(|(requested, actual)| requested == actual);
    let cpu_matches = environment
        .requested_cpu_cores
        .as_ref()
        .zip(environment.cgroup_cpu_max.as_ref())
        .and_then(|(requested, actual)| {
            let requested = requested.parse::<f64>().ok()?;
            let mut fields = actual.split_whitespace();
            let quota = fields.next()?.parse::<f64>().ok()?;
            let period = fields.next()?.parse::<f64>().ok()?;
            Some((quota / period - requested).abs() < 0.01)
        })
        .unwrap_or(false);
    memory_matches && cpu_matches
}

fn unix_now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
