//! Gate16.1 measurement ruler for the legacy v4 full-checkpoint path.
//!
//! This executable intentionally exercises the current end-to-end TCP Zone RPC
//! path. Every history entry is an acknowledged command, then the active host
//! exports its full checkpoint and a fresh standby installs/replays it.
//!
//! Run a quick local baseline:
//!   MIR2_GATE16_HISTORY_STEPS=700 cargo run --release \
//!     --bin gate16_checkpoint_load
//!
//! Run the certification history matrix:
//!   MIR2_GATE16_HISTORY_STEPS=700,10000,100000 cargo run --release \
//!     --bin gate16_checkpoint_load

use std::env;
use std::fs;
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mir2_gateway::{
    GatewayConfig, InMemoryZoneOwnerLeaseAuthority, SharedZoneOwnerLeaseAuthority,
    TcpZoneOwnerRpcTransport, ZoneHostServer, ZoneHostTelemetrySnapshot, ZoneId,
    ZoneOwnerCommandRequest, ZoneOwnerLeaseAuthority, ZoneOwnerRpcTransport, ZoneRpcLimits,
    ZONE_HOST_CHECKPOINT_VERSION, ZONE_RPC_PROTOCOL_VERSION,
};
use mir2_protocol::ClientPacket;
use mir2_simulation::WorldCommand;
use serde::Serialize;

const DEFAULT_OUTPUT_PATH: &str = "docs/generated/gate16/v4-checkpoint-baseline.json";
const DEFAULT_MAX_FRAME_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Gate16V4BaselineReport {
    schema_version: u32,
    generated_at_unix_ms: u128,
    workload: &'static str,
    checkpoint_version: u32,
    zone_rpc_protocol_version: u16,
    build: &'static str,
    environment: BenchmarkEnvironment,
    history_steps: Vec<usize>,
    results: Vec<HistoryResult>,
    caveats: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BenchmarkEnvironment {
    profile_label: Option<String>,
    requested_cpu_cores: Option<String>,
    requested_memory_bytes: Option<String>,
    cgroup_cpu_max: Option<String>,
    cgroup_memory_max: Option<String>,
    available_parallelism: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HistoryResult {
    requested_commands: usize,
    completed_commands: usize,
    success: bool,
    error: Option<String>,
    command_wall_ms: f64,
    command_latency_ms: NumericSummary,
    checkpoint_entries: Option<usize>,
    checkpoint_bytes: Option<usize>,
    modeled_wire_mbps_at_100ms: Option<f64>,
    modeled_wire_mbps_at_5s: Option<f64>,
    export_wall_ms: Option<f64>,
    install_wall_ms: Option<f64>,
    process_rss_before_bytes: Option<u64>,
    process_rss_after_history_bytes: Option<u64>,
    process_rss_after_install_bytes: Option<u64>,
    active_telemetry: ZoneHostTelemetrySnapshot,
    standby_telemetry: ZoneHostTelemetrySnapshot,
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
    server: Arc<ZoneHostServer>,
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
        let server = Arc::new(ZoneHostServer::with_options(
            GatewayConfig::default(),
            authority,
            None,
            limits,
        ));
        let stop = Arc::new(AtomicBool::new(false));
        let running_server = Arc::clone(&server);
        let server_stop = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            if let Err(error) = running_server.serve_until(listener, server_stop) {
                eprintln!("Gate16 baseline Zone Host stopped with error: {error}");
            }
        });
        Ok(Self {
            address,
            server,
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
    let history_steps = usize_list_env("MIR2_GATE16_HISTORY_STEPS", &[700, 10_000, 100_000]);
    let output_path = PathBuf::from(
        env::var("MIR2_GATE16_BASELINE_OUT").unwrap_or_else(|_| DEFAULT_OUTPUT_PATH.to_string()),
    );
    let max_frame_bytes = usize_env("MIR2_GATE16_MAX_FRAME_BYTES", DEFAULT_MAX_FRAME_BYTES);
    let mut results = Vec::with_capacity(history_steps.len());

    for &history_entries in &history_steps {
        eprintln!("Gate16 v4 baseline: {history_entries} acknowledged commands");
        results.push(run_history_step(history_entries, max_frame_bytes));
    }

    let report = Gate16V4BaselineReport {
        schema_version: 1,
        generated_at_unix_ms: unix_now_ms(),
        workload: "zone-rpc-v4-full-checkpoint-history",
        checkpoint_version: ZONE_HOST_CHECKPOINT_VERSION,
        zone_rpc_protocol_version: ZONE_RPC_PROTOCOL_VERSION,
        build: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        environment: benchmark_environment(),
        history_steps,
        results,
        caveats: vec![
            "The workload uses one sequential KeepAlive command stream to isolate history-size cost.",
            "Command latency includes a fresh localhost TCP Zone RPC connection and acknowledgement.",
            "This is a single-process localhost baseline; container CPU/network certification is separate.",
            "The v4 max frame limit is raised for measurement and is not a production recommendation.",
            "Modeled wire rates use checkpoint payload bytes and omit TCP/IP framing and retransmits.",
        ],
    };

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &output_path,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    println!("Wrote {}", output_path.display());
    if report.results.iter().any(|result| !result.success) {
        std::process::exit(1);
    }
    Ok(())
}

fn run_history_step(requested_commands: usize, max_frame_bytes: usize) -> HistoryResult {
    match try_run_history_step(requested_commands, max_frame_bytes) {
        Ok(result) => result,
        Err(failure) => failure,
    }
}

fn try_run_history_step(
    requested_commands: usize,
    max_frame_bytes: usize,
) -> Result<HistoryResult, HistoryResult> {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let shared_authority: SharedZoneOwnerLeaseAuthority = authority.clone();
    let limits = ZoneRpcLimits {
        max_frame_bytes,
        max_connections: 64,
        max_sessions: 8,
        max_sessions_per_zone: 8,
        io_timeout: Duration::from_secs(120),
        ..ZoneRpcLimits::default()
    };
    let active = RunningHost::start(Arc::clone(&shared_authority), limits.clone())
        .map_err(|error| empty_failure(requested_commands, error))?;
    let standby = RunningHost::start(shared_authority, limits.clone())
        .map_err(|error| empty_failure(requested_commands, error))?;
    let zone_id = ZoneId::new("map:0");
    let active_transport = transport(
        active.address,
        zone_id.clone(),
        "gate16-history",
        limits.clone(),
    );
    let standby_transport = transport(standby.address, zone_id.clone(), "gate16-history", limits);
    let owner_lease = authority.owner_lease(&zone_id);
    let rss_before = process_rss_bytes();

    active_transport
        .on_connect()
        .map_err(|error| failure_from_hosts(requested_commands, 0, error, &active, &standby))?;

    let mut command_samples = Vec::with_capacity(requested_commands);
    let command_started = Instant::now();
    for sequence in 0..requested_commands {
        let started = Instant::now();
        active_transport
            .execute(ZoneOwnerCommandRequest::direct(
                owner_lease.clone(),
                WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                    time: i64::try_from(sequence).unwrap_or(i64::MAX),
                }),
            ))
            .map_err(|error| {
                failure_from_hosts(
                    requested_commands,
                    command_samples.len(),
                    error,
                    &active,
                    &standby,
                )
            })?;
        command_samples.push(started.elapsed().as_secs_f64() * 1_000.0);
    }
    let command_wall_ms = command_started.elapsed().as_secs_f64() * 1_000.0;
    let rss_after_history = process_rss_bytes();

    let export_started = Instant::now();
    let checkpoint = active_transport.export_host_checkpoint().map_err(|error| {
        failure_from_hosts(
            requested_commands,
            command_samples.len(),
            error,
            &active,
            &standby,
        )
    })?;
    let export_wall_ms = export_started.elapsed().as_secs_f64() * 1_000.0;

    let install_started = Instant::now();
    standby_transport
        .install_host_checkpoint(&checkpoint)
        .map_err(|error| {
            failure_from_hosts(
                requested_commands,
                command_samples.len(),
                error,
                &active,
                &standby,
            )
        })?;
    let install_wall_ms = install_started.elapsed().as_secs_f64() * 1_000.0;

    Ok(HistoryResult {
        requested_commands,
        completed_commands: command_samples.len(),
        success: true,
        error: None,
        command_wall_ms,
        command_latency_ms: summarize(&mut command_samples),
        checkpoint_entries: Some(checkpoint.entry_count),
        checkpoint_bytes: Some(checkpoint.as_bytes().len()),
        modeled_wire_mbps_at_100ms: Some(modeled_wire_mbps(checkpoint.as_bytes().len(), 100)),
        modeled_wire_mbps_at_5s: Some(modeled_wire_mbps(checkpoint.as_bytes().len(), 5_000)),
        export_wall_ms: Some(export_wall_ms),
        install_wall_ms: Some(install_wall_ms),
        process_rss_before_bytes: rss_before,
        process_rss_after_history_bytes: rss_after_history,
        process_rss_after_install_bytes: process_rss_bytes(),
        active_telemetry: active.server.telemetry_snapshot(),
        standby_telemetry: standby.server.telemetry_snapshot(),
    })
}

fn failure_from_hosts(
    requested_commands: usize,
    completed_commands: usize,
    error: impl std::fmt::Display,
    active: &RunningHost,
    standby: &RunningHost,
) -> HistoryResult {
    HistoryResult {
        requested_commands,
        completed_commands,
        success: false,
        error: Some(error.to_string()),
        command_wall_ms: 0.0,
        command_latency_ms: summarize(&mut []),
        checkpoint_entries: None,
        checkpoint_bytes: None,
        modeled_wire_mbps_at_100ms: None,
        modeled_wire_mbps_at_5s: None,
        export_wall_ms: None,
        install_wall_ms: None,
        process_rss_before_bytes: None,
        process_rss_after_history_bytes: process_rss_bytes(),
        process_rss_after_install_bytes: None,
        active_telemetry: active.server.telemetry_snapshot(),
        standby_telemetry: standby.server.telemetry_snapshot(),
    }
}

fn empty_failure(requested_commands: usize, error: impl std::fmt::Display) -> HistoryResult {
    let placeholder_authority: SharedZoneOwnerLeaseAuthority =
        Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let placeholder = ZoneHostServer::with_options(
        GatewayConfig::default(),
        placeholder_authority,
        None,
        ZoneRpcLimits::default(),
    );
    let telemetry = placeholder.telemetry_snapshot();
    HistoryResult {
        requested_commands,
        completed_commands: 0,
        success: false,
        error: Some(error.to_string()),
        command_wall_ms: 0.0,
        command_latency_ms: summarize(&mut []),
        checkpoint_entries: None,
        checkpoint_bytes: None,
        modeled_wire_mbps_at_100ms: None,
        modeled_wire_mbps_at_5s: None,
        export_wall_ms: None,
        install_wall_ms: None,
        process_rss_before_bytes: None,
        process_rss_after_history_bytes: None,
        process_rss_after_install_bytes: None,
        active_telemetry: telemetry.clone(),
        standby_telemetry: telemetry,
    }
}

fn transport(
    address: SocketAddr,
    zone_id: ZoneId,
    session_id: &str,
    limits: ZoneRpcLimits,
) -> TcpZoneOwnerRpcTransport {
    TcpZoneOwnerRpcTransport::with_options(address.to_string(), zone_id, session_id, None, limits)
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

fn process_rss_bytes() -> Option<u64> {
    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        if let Some(kibibytes) = status.lines().find_map(|line| {
            line.strip_prefix("VmRSS:")
                .and_then(|value| value.split_whitespace().next())
                .and_then(|value| value.parse::<u64>().ok())
        }) {
            return kibibytes.checked_mul(1_024);
        }
    }
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let kibibytes = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u64>()
        .ok()?;
    kibibytes.checked_mul(1_024)
}

fn benchmark_environment() -> BenchmarkEnvironment {
    BenchmarkEnvironment {
        profile_label: env::var("GATE16_PROFILE_LABEL").ok(),
        requested_cpu_cores: env::var("GATE16_PROFILE_CPU_CORES").ok(),
        requested_memory_bytes: env::var("GATE16_PROFILE_MEMORY_BYTES").ok(),
        cgroup_cpu_max: read_trimmed("/sys/fs/cgroup/cpu.max"),
        cgroup_memory_max: read_trimmed("/sys/fs/cgroup/memory.max"),
        available_parallelism: thread::available_parallelism()
            .map(|parallelism| parallelism.get())
            .unwrap_or(1),
    }
}

fn read_trimmed(path: &str) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn modeled_wire_mbps(bytes: usize, interval_ms: u64) -> f64 {
    bytes as f64 * 8.0 / (interval_ms as f64 / 1_000.0) / 1_000_000.0
}

fn usize_list_env(name: &str, fallback: &[usize]) -> Vec<usize> {
    env::var(name)
        .ok()
        .map(|raw| {
            raw.split(',')
                .filter_map(|part| part.trim().parse::<usize>().ok())
                .filter(|value| *value > 0)
                .collect::<Vec<_>>()
        })
        .filter(|values| !values.is_empty())
        .unwrap_or_else(|| fallback.to_vec())
}

fn usize_env(name: &str, fallback: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

fn unix_now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}
