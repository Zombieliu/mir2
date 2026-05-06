use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mir2_protocol::{
    client_packet_name, decode_frame, decode_server_packet, encode_client_packet, ClientPacket,
    MirClass, MirDirection, MirGender, MirGridType, PacketTraceDirection, Point, ServerPacket,
    ServerPacketId, Spell,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout, Instant};

const DEFAULT_GATEWAY_TCP_ADDR: &str = "127.0.0.1:7000";
const DEFAULT_TRACE_OUT: &str = "docs/generated/packet-traces/latest.json";
const DEFAULT_MATRIX_OUT_DIR: &str = "docs/generated/packet-traces/matrix";
const DEFAULT_MATRIX_FLOW_DELAY_MS: u64 = 750;
const PARITY_MATRIX_PATH: &str = "docs/parity-matrix.json";
const DEFAULT_READ_DRAIN_MS: u64 = 500;
const READ_STEP_TIMEOUT_MS: u64 = 750;
const CONNECT_TIMEOUT_MS: u64 = 1_500;
const DYNAMIC_NEW_CHARACTER_INDEX: i32 = i32::MIN;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiffAcceptanceMode {
    Exact,
    Stable,
}

impl DiffAcceptanceMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Stable => "stable",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct TraceFlow {
    name: &'static str,
    description: &'static str,
    packets: fn(&Fixture) -> Vec<ClientPacket>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    mode: String,
    account: String,
    lifecycle_account: String,
    lifecycle_character: String,
    character: String,
    character_index: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TraceArtifact {
    schema_version: u8,
    generated_at_unix_ms: u128,
    flow: String,
    fixture: Fixture,
    local: EndpointTrace,
    #[serde(skip_serializing_if = "Option::is_none")]
    crystal: Option<EndpointTrace>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diff: Option<TraceDiff>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stable_diff: Option<TraceDiff>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MatrixArtifact {
    schema_version: u8,
    generated_at_unix_ms: u128,
    fixture: Fixture,
    summary: MatrixSummary,
    artifacts: Vec<MatrixEntry>,
    skipped: Vec<MatrixSkip>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MatrixSummary {
    acceptance_mode: String,
    artifact_count: usize,
    skipped_count: usize,
    local_ok_count: usize,
    local_failed_count: usize,
    crystal_ok_count: usize,
    crystal_failed_count: usize,
    crystal_missing_count: usize,
    diff_clean_count: usize,
    diff_dirty_count: usize,
    diff_missing_count: usize,
    stable_diff_clean_count: usize,
    stable_diff_dirty_count: usize,
    stable_diff_missing_count: usize,
    accepted_live_comparison_count: usize,
    accepted_stable_live_comparison_count: usize,
    accepted_packet_parity_count: usize,
    packet_parity_accepted: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MatrixEntry {
    matrix_id: String,
    trace_flow: String,
    path: String,
    local_ok: bool,
    crystal_ok: Option<bool>,
    diff_clean: Option<bool>,
    stable_diff_clean: Option<bool>,
    accepted_packet_parity: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MatrixSkip {
    matrix_id: String,
    reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EndpointTrace {
    endpoint: String,
    ok: bool,
    elapsed_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    entries: Vec<TraceEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TraceEntry {
    sequence: usize,
    elapsed_ms: u128,
    direction: PacketTraceDirection,
    packet_id: i16,
    packet: String,
    payload_len: usize,
    payload_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload_hex: Option<String>,
    decoded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    decode_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TraceDiff {
    clean: bool,
    compared_entries: usize,
    mismatch_reasons: Vec<String>,
    mismatches: Vec<TraceMismatch>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TraceMismatch {
    sequence: usize,
    reason: String,
    local: Option<TraceEntrySummary>,
    crystal: Option<TraceEntrySummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TraceEntrySummary {
    direction: PacketTraceDirection,
    packet_id: i16,
    packet: String,
    payload_len: usize,
    payload_hash: String,
    decoded: bool,
}

#[derive(Debug, Default)]
struct EndpointCaptureState {
    last_new_character_index: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct ParityMatrix {
    flows: Vec<ParityFlow>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ParityFlow {
    id: String,
    trace_flow: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.iter().any(|arg| arg == "--list-flows") {
        for flow in trace_flows() {
            println!("{}\t{}", flow.name, flow.description);
        }
        return Ok(());
    }

    let fixture = fixture_from_env();
    let acceptance_mode = diff_acceptance_mode_from_env();
    if args.iter().any(|arg| arg == "--matrix") {
        let report = capture_matrix(&fixture, acceptance_mode).await?;
        enforce_requirements(
            report.artifacts.iter().any(|entry| entry.local_ok),
            report
                .artifacts
                .iter()
                .filter_map(|entry| entry.crystal_ok)
                .any(|ok| ok),
            !report.artifacts.is_empty()
                && report
                    .artifacts
                    .iter()
                    .all(|entry| entry.diff_clean == Some(true)),
            report.summary.packet_parity_accepted,
            acceptance_mode,
        )?;
        return Ok(());
    }

    let flow_name =
        env::var("MIR2_PACKET_TRACE_FLOW").unwrap_or_else(|_| "core_bootstrap".to_string());
    let artifact = capture_flow(&flow_name, &fixture).await?;
    let out = env::var("MIR2_PACKET_TRACE_OUT").unwrap_or_else(|_| DEFAULT_TRACE_OUT.to_string());
    write_json(Path::new(&out), &artifact)?;
    println!(
        "wrote {out} (flow={}, local_ok={}, crystal_ok={:?}, diff_clean={:?}, stable_diff_clean={:?}, acceptance_mode={}, accepted_packet_parity={})",
        artifact.flow,
        artifact.local.ok,
        artifact.crystal.as_ref().map(|trace| trace.ok),
        artifact.diff.as_ref().map(|diff| diff.clean),
        artifact.stable_diff.as_ref().map(|diff| diff.clean),
        acceptance_mode.as_str(),
        accepted_artifact_packet_parity(&artifact, acceptance_mode)
    );
    enforce_requirements(
        artifact.local.ok,
        artifact.crystal.as_ref().is_some_and(|trace| trace.ok),
        artifact.diff.as_ref().is_some_and(|diff| diff.clean),
        accepted_artifact_packet_parity(&artifact, acceptance_mode),
        acceptance_mode,
    )?;

    Ok(())
}

async fn capture_matrix(
    fixture: &Fixture,
    acceptance_mode: DiffAcceptanceMode,
) -> Result<MatrixArtifact, Box<dyn std::error::Error>> {
    let out_dir = env::var("MIR2_PACKET_TRACE_MATRIX_OUT_DIR")
        .unwrap_or_else(|_| DEFAULT_MATRIX_OUT_DIR.into());
    fs::create_dir_all(&out_dir)?;
    let flow_delay_ms = env::var("MIR2_PACKET_TRACE_MATRIX_FLOW_DELAY_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_MATRIX_FLOW_DELAY_MS);

    let matrix_text = fs::read_to_string(parity_matrix_path())?;
    let matrix: ParityMatrix = serde_json::from_str(&matrix_text)?;
    let flow_names = trace_flow_names();
    let mut artifacts = Vec::new();
    let mut skipped = Vec::new();
    let mut captured_flows = BTreeMap::<String, TraceArtifact>::new();
    let mut account_lifecycle_ordinal = 0usize;
    let account_lifecycle_run_seed = now_unix_ms();

    for trace_flow in matrix_capture_order(&matrix.flows, &flow_names) {
        let capture_fixture = if trace_flow == "account_lifecycle" {
            account_lifecycle_ordinal += 1;
            fixture_for_matrix_flow_with_suffix(
                fixture,
                &trace_flow,
                &matrix_lifecycle_suffix(account_lifecycle_run_seed, account_lifecycle_ordinal),
            )
        } else {
            fixture.clone()
        };
        let artifact = capture_flow(&trace_flow, &capture_fixture).await?;
        captured_flows.insert(trace_flow, artifact);
        if flow_delay_ms > 0 {
            sleep(Duration::from_millis(flow_delay_ms)).await;
        }
    }

    for matrix_flow in matrix.flows {
        let Some(trace_flow) = matrix_flow.trace_flow else {
            skipped.push(MatrixSkip {
                matrix_id: matrix_flow.id,
                reason: "matrix entry does not declare traceFlow".to_string(),
            });
            continue;
        };

        if !flow_names.contains(trace_flow.as_str()) {
            skipped.push(MatrixSkip {
                matrix_id: matrix_flow.id,
                reason: format!("unknown traceFlow {trace_flow}"),
            });
            continue;
        }

        let artifact = captured_flows
            .get(&trace_flow)
            .expect("valid matrix trace flow should have been captured")
            .clone();
        let file_name = format!("{}.json", sanitize_file_stem(&matrix_flow.id));
        let path = Path::new(&out_dir).join(file_name);
        write_json(&path, &artifact)?;
        let mut entry = MatrixEntry {
            matrix_id: matrix_flow.id,
            trace_flow,
            path: path.to_string_lossy().into_owned(),
            local_ok: artifact.local.ok,
            crystal_ok: artifact.crystal.as_ref().map(|trace| trace.ok),
            diff_clean: artifact.diff.as_ref().map(|diff| diff.clean),
            stable_diff_clean: artifact.stable_diff.as_ref().map(|diff| diff.clean),
            accepted_packet_parity: false,
        };
        entry.accepted_packet_parity = matrix_entry_accepted_packet_parity(&entry, acceptance_mode);
        artifacts.push(entry);
    }

    let report = MatrixArtifact {
        schema_version: 1,
        generated_at_unix_ms: now_unix_ms(),
        fixture: fixture.clone(),
        summary: matrix_summary(&artifacts, &skipped, acceptance_mode),
        artifacts,
        skipped,
    };
    let latest_path = Path::new(&out_dir).join("latest-matrix.json");
    write_json(&latest_path, &report)?;
    println!(
        "wrote {} (artifacts={}, skipped={})",
        latest_path.to_string_lossy(),
        report.artifacts.len(),
        report.skipped.len()
    );
    Ok(report)
}

fn matrix_capture_order(flows: &[ParityFlow], flow_names: &BTreeSet<&'static str>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut order = Vec::new();
    for flow in flows {
        let Some(trace_flow) = flow.trace_flow.as_deref() else {
            continue;
        };
        if !flow_names.contains(trace_flow) || !seen.insert(trace_flow.to_string()) {
            continue;
        }
        order.push(trace_flow.to_string());
    }
    order.sort_by_key(|trace_flow| matrix_capture_priority(trace_flow));
    order
}

fn matrix_capture_priority(trace_flow: &str) -> usize {
    match trace_flow {
        "core_bootstrap" => 0,
        "combat_basic" => 1,
        "magic_basic" => 2,
        "inventory_storage" => 3,
        "storage_password" => 4,
        "movement_chat_keepalive" => 5,
        "account_lifecycle" => 6,
        _ => 10,
    }
}

#[cfg(test)]
fn fixture_for_matrix_flow(fixture: &Fixture, trace_flow: &str, ordinal: usize) -> Fixture {
    fixture_for_matrix_flow_with_suffix(fixture, trace_flow, &format!("{ordinal:02}"))
}

fn fixture_for_matrix_flow_with_suffix(
    fixture: &Fixture,
    trace_flow: &str,
    suffix: &str,
) -> Fixture {
    let mut fixture = fixture.clone();
    if trace_flow == "account_lifecycle" {
        fixture.lifecycle_account = with_suffix(&fixture.lifecycle_account, suffix, 13);
        fixture.lifecycle_character = with_suffix(&fixture.lifecycle_character, suffix, 12);
    }
    fixture
}

fn matrix_lifecycle_suffix(seed: u128, ordinal: usize) -> String {
    let mut value = (seed as usize).wrapping_add(ordinal);
    let mut suffix = String::new();
    for _ in 0..4 {
        suffix.push(char::from(b'a' + (value % 26) as u8));
        value /= 26;
    }
    suffix.push(char::from(b'a' + (ordinal % 26) as u8));
    suffix
}

fn with_suffix(value: &str, suffix: &str, max_len: usize) -> String {
    let stem_len = max_len.saturating_sub(suffix.len());
    let stem = value.chars().take(stem_len).collect::<String>();
    format!("{stem}{suffix}")
}

fn matrix_summary(
    artifacts: &[MatrixEntry],
    skipped: &[MatrixSkip],
    acceptance_mode: DiffAcceptanceMode,
) -> MatrixSummary {
    let accepted_packet_parity_count = artifacts
        .iter()
        .filter(|entry| entry.accepted_packet_parity)
        .count();
    MatrixSummary {
        acceptance_mode: acceptance_mode.as_str().to_string(),
        artifact_count: artifacts.len(),
        skipped_count: skipped.len(),
        local_ok_count: artifacts.iter().filter(|entry| entry.local_ok).count(),
        local_failed_count: artifacts.iter().filter(|entry| !entry.local_ok).count(),
        crystal_ok_count: artifacts
            .iter()
            .filter(|entry| entry.crystal_ok == Some(true))
            .count(),
        crystal_failed_count: artifacts
            .iter()
            .filter(|entry| entry.crystal_ok == Some(false))
            .count(),
        crystal_missing_count: artifacts
            .iter()
            .filter(|entry| entry.crystal_ok.is_none())
            .count(),
        diff_clean_count: artifacts
            .iter()
            .filter(|entry| entry.diff_clean == Some(true))
            .count(),
        diff_dirty_count: artifacts
            .iter()
            .filter(|entry| entry.diff_clean == Some(false))
            .count(),
        diff_missing_count: artifacts
            .iter()
            .filter(|entry| entry.diff_clean.is_none())
            .count(),
        stable_diff_clean_count: artifacts
            .iter()
            .filter(|entry| entry.stable_diff_clean == Some(true))
            .count(),
        stable_diff_dirty_count: artifacts
            .iter()
            .filter(|entry| entry.stable_diff_clean == Some(false))
            .count(),
        stable_diff_missing_count: artifacts
            .iter()
            .filter(|entry| entry.stable_diff_clean.is_none())
            .count(),
        accepted_live_comparison_count: artifacts
            .iter()
            .filter(|entry| {
                entry.local_ok && entry.crystal_ok == Some(true) && entry.diff_clean == Some(true)
            })
            .count(),
        accepted_stable_live_comparison_count: artifacts
            .iter()
            .filter(|entry| {
                entry.local_ok
                    && entry.crystal_ok == Some(true)
                    && entry.stable_diff_clean == Some(true)
            })
            .count(),
        accepted_packet_parity_count,
        packet_parity_accepted: !artifacts.is_empty()
            && accepted_packet_parity_count == artifacts.len(),
    }
}

fn matrix_entry_accepted_packet_parity(
    entry: &MatrixEntry,
    acceptance_mode: DiffAcceptanceMode,
) -> bool {
    entry.local_ok
        && entry.crystal_ok == Some(true)
        && match acceptance_mode {
            DiffAcceptanceMode::Exact => entry.diff_clean == Some(true),
            DiffAcceptanceMode::Stable => entry.stable_diff_clean == Some(true),
        }
}

fn accepted_artifact_packet_parity(
    artifact: &TraceArtifact,
    acceptance_mode: DiffAcceptanceMode,
) -> bool {
    artifact.local.ok
        && artifact.crystal.as_ref().is_some_and(|trace| trace.ok)
        && match acceptance_mode {
            DiffAcceptanceMode::Exact => artifact.diff.as_ref().is_some_and(|diff| diff.clean),
            DiffAcceptanceMode::Stable => {
                artifact.stable_diff.as_ref().is_some_and(|diff| diff.clean)
            }
        }
}

async fn capture_flow(
    flow_name: &str,
    fixture: &Fixture,
) -> Result<TraceArtifact, Box<dyn std::error::Error>> {
    let flow = trace_flows()
        .into_iter()
        .find(|flow| flow.name == flow_name)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown packet trace flow {flow_name}"),
            )
        })?;

    let local_addr =
        env::var("MIR2_GATEWAY_TCP_ADDR").unwrap_or_else(|_| DEFAULT_GATEWAY_TCP_ADDR.into());
    let packets = (flow.packets)(fixture);
    let local = capture_endpoint(&local_addr, &packets).await;
    let crystal = match env::var("MIR2_CRYSTAL_TCP_ADDR") {
        Ok(addr) if !addr.trim().is_empty() => Some(capture_endpoint(addr.trim(), &packets).await),
        _ => None,
    };
    let diff = crystal.as_ref().map(|crystal| diff_traces(&local, crystal));
    let stable_diff = crystal
        .as_ref()
        .map(|crystal| stable_diff_traces(&local, crystal));

    Ok(TraceArtifact {
        schema_version: 1,
        generated_at_unix_ms: now_unix_ms(),
        flow: flow.name.to_string(),
        fixture: fixture.clone(),
        local,
        crystal,
        diff,
        stable_diff,
    })
}

async fn capture_endpoint(addr: &str, packets: &[ClientPacket]) -> EndpointTrace {
    let started = Instant::now();
    let mut entries = Vec::new();
    let mut state = EndpointCaptureState::default();
    let result = async {
        let mut stream = timeout(
            Duration::from_millis(CONNECT_TIMEOUT_MS),
            TcpStream::connect(addr),
        )
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "connect timeout"))??;

        drain_initial_server_packets(&mut stream, started, &mut entries, &mut state).await?;
        for packet in packets {
            let packet = resolve_trace_packet(packet, &state);
            let bytes = encode_client_packet(&packet)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
            entries.push(trace_client_entry(
                entries.len(),
                started.elapsed().as_millis(),
                &packet,
                &bytes,
            ));
            stream.write_all(&bytes).await?;
            stream.flush().await?;
            drain_server_packets(&mut stream, started, &mut entries, &mut state).await?;
        }
        if flow_needs_logout_cleanup(packets) {
            let _ = logout_endpoint(&mut stream).await;
        }
        let _ = disconnect_endpoint(&mut stream).await;
        Ok::<(), io::Error>(())
    }
    .await;

    EndpointTrace {
        endpoint: addr.to_string(),
        ok: result.is_ok(),
        elapsed_ms: started.elapsed().as_millis(),
        error: result.err().map(|error| error.to_string()),
        entries,
    }
}

async fn drain_server_packets(
    stream: &mut TcpStream,
    started: Instant,
    entries: &mut Vec<TraceEntry>,
    state: &mut EndpointCaptureState,
) -> io::Result<()> {
    drain_server_packets_with_idle(stream, started, entries, state, read_drain_timeout_ms()).await
}

async fn drain_initial_server_packets(
    stream: &mut TcpStream,
    started: Instant,
    entries: &mut Vec<TraceEntry>,
    state: &mut EndpointCaptureState,
) -> io::Result<()> {
    drain_server_packets_with_idle(stream, started, entries, state, READ_STEP_TIMEOUT_MS).await
}

async fn drain_server_packets_with_idle(
    stream: &mut TcpStream,
    started: Instant,
    entries: &mut Vec<TraceEntry>,
    state: &mut EndpointCaptureState,
    idle_timeout_ms: u64,
) -> io::Result<()> {
    loop {
        match timeout(Duration::from_millis(idle_timeout_ms), read_frame(stream)).await {
            Ok(Ok(frame)) => {
                update_capture_state(state, &frame);
                entries.push(trace_server_entry(
                    entries.len(),
                    started.elapsed().as_millis(),
                    &frame,
                ));
            }
            Ok(Err(error)) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            Ok(Err(error)) if error.kind() == io::ErrorKind::TimedOut => return Ok(()),
            Ok(Err(error)) => return Err(error),
            Err(_) => return Ok(()),
        }
    }
}

fn update_capture_state(state: &mut EndpointCaptureState, frame: &[u8]) {
    if let Ok(ServerPacket::NewCharacterSuccess { char_info }) = decode_server_packet(frame) {
        state.last_new_character_index = Some(char_info.index);
    }
}

fn resolve_trace_packet(packet: &ClientPacket, state: &EndpointCaptureState) -> ClientPacket {
    match packet {
        ClientPacket::DeleteCharacter { character_index }
            if *character_index == DYNAMIC_NEW_CHARACTER_INDEX =>
        {
            ClientPacket::DeleteCharacter {
                character_index: state.last_new_character_index.unwrap_or(0),
            }
        }
        _ => packet.clone(),
    }
}

fn flow_needs_logout_cleanup(packets: &[ClientPacket]) -> bool {
    packets
        .iter()
        .any(|packet| matches!(packet, ClientPacket::Login { .. }))
}

async fn disconnect_endpoint(stream: &mut TcpStream) -> io::Result<()> {
    let bytes = encode_client_packet(&ClientPacket::Disconnect)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    stream.write_all(&bytes).await?;
    stream.flush().await?;
    stream.shutdown().await
}

async fn logout_endpoint(stream: &mut TcpStream) -> io::Result<()> {
    let bytes = encode_client_packet(&ClientPacket::LogOut)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    stream.write_all(&bytes).await?;
    stream.flush().await?;
    drain_unrecorded_server_packets(stream).await
}

async fn drain_unrecorded_server_packets(stream: &mut TcpStream) -> io::Result<()> {
    loop {
        match timeout(
            Duration::from_millis(read_drain_timeout_ms()),
            read_frame(stream),
        )
        .await
        {
            Ok(Ok(_)) => {}
            Ok(Err(error)) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            Ok(Err(error)) if error.kind() == io::ErrorKind::TimedOut => return Ok(()),
            Ok(Err(error)) => return Err(error),
            Err(_) => return Ok(()),
        }
    }
}

fn read_drain_timeout_ms() -> u64 {
    env::var("MIR2_PACKET_TRACE_READ_DRAIN_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_READ_DRAIN_MS)
}

async fn read_frame(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut header = [0_u8; 2];
    timeout(
        Duration::from_millis(READ_STEP_TIMEOUT_MS),
        stream.read_exact(&mut header),
    )
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "read frame header timeout"))??;
    let len = u16::from_le_bytes(header) as usize;
    if len < 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid frame length {len}"),
        ));
    }
    let mut frame = vec![0_u8; len];
    frame[..2].copy_from_slice(&header);
    timeout(
        Duration::from_millis(READ_STEP_TIMEOUT_MS),
        stream.read_exact(&mut frame[2..]),
    )
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "read frame body timeout"))??;
    Ok(frame)
}

fn trace_client_entry(
    sequence: usize,
    elapsed_ms: u128,
    packet: &ClientPacket,
    frame: &[u8],
) -> TraceEntry {
    TraceEntry {
        sequence,
        elapsed_ms,
        direction: PacketTraceDirection::Client,
        packet_id: packet.packet_id() as i16,
        packet: client_packet_name(packet).to_string(),
        payload_len: frame.len().saturating_sub(4),
        payload_hash: payload_hash(frame),
        payload_hex: trace_payload_hex(frame),
        decoded: true,
        decode_error: None,
        detail: client_packet_detail(packet),
    }
}

fn trace_server_entry(sequence: usize, elapsed_ms: u128, frame: &[u8]) -> TraceEntry {
    let packet_id = decode_frame(frame)
        .map(|frame| frame.packet_id)
        .unwrap_or_default();
    match decode_server_packet(frame) {
        Ok(packet) => TraceEntry {
            sequence,
            elapsed_ms,
            direction: PacketTraceDirection::Server,
            packet_id,
            packet: mir2_protocol::server_packet_name(&packet).to_string(),
            payload_len: frame.len().saturating_sub(4),
            payload_hash: payload_hash(frame),
            payload_hex: trace_payload_hex(frame),
            decoded: true,
            decode_error: None,
            detail: server_packet_detail(&packet),
        },
        Err(error) => TraceEntry {
            sequence,
            elapsed_ms,
            direction: PacketTraceDirection::Server,
            packet_id,
            packet: ServerPacketId::try_from(packet_id)
                .map(|id| format!("{id:?}"))
                .unwrap_or_else(|_| "Unknown".to_string()),
            payload_len: frame.len().saturating_sub(4),
            payload_hash: payload_hash(frame),
            payload_hex: trace_payload_hex(frame),
            decoded: false,
            decode_error: Some(error.to_string()),
            detail: None,
        },
    }
}

fn client_packet_detail(packet: &ClientPacket) -> Option<Value> {
    match packet {
        ClientPacket::ClientVersion { version_hash } => Some(json!({
            "versionHashLength": version_hash.len()
        })),
        ClientPacket::NewAccount { account_id, .. } => Some(json!({
            "accountId": account_id
        })),
        ClientPacket::ChangePassword { account_id, .. } => Some(json!({
            "accountId": account_id
        })),
        ClientPacket::Login { account_id, .. } => Some(json!({
            "accountId": account_id
        })),
        ClientPacket::NewCharacter {
            name,
            gender,
            class,
        } => Some(json!({
            "name": name,
            "gender": format!("{gender:?}"),
            "class": format!("{class:?}")
        })),
        ClientPacket::DeleteCharacter { character_index }
        | ClientPacket::StartGame { character_index } => Some(json!({
            "characterIndex": character_index
        })),
        ClientPacket::DepositTradeItem { from, to }
        | ClientPacket::RetrieveTradeItem { from, to }
        | ClientPacket::TakeBackHeroItem { from, to }
        | ClientPacket::TransferHeroItem { from, to } => Some(json!({
            "from": from,
            "to": to
        })),
        ClientPacket::NewHero {
            name,
            gender,
            class,
        } => Some(json!({
            "name": name,
            "gender": format!("{gender:?}"),
            "class": format!("{class:?}")
        })),
        ClientPacket::SetHeroBehaviour { behaviour } => Some(json!({
            "behaviour": behaviour
        })),
        ClientPacket::ChangeHero { list_index } => Some(json!({
            "listIndex": list_index
        })),
        ClientPacket::ConsignItem {
            unique_id,
            price,
            market_type,
        } => Some(json!({
            "uniqueId": unique_id,
            "price": price,
            "marketType": market_type
        })),
        ClientPacket::MarketSearch {
            match_text,
            item_type,
            user_mode,
            min_shape,
            max_shape,
            market_type,
        } => Some(json!({
            "matchText": match_text,
            "itemType": item_type,
            "userMode": user_mode,
            "minShape": min_shape,
            "maxShape": max_shape,
            "marketType": market_type
        })),
        ClientPacket::MarketPage { page } => Some(json!({
            "page": page
        })),
        ClientPacket::MarketBuy {
            auction_id,
            bid_price,
        } => Some(json!({
            "auctionId": auction_id,
            "bidPrice": bid_price
        })),
        ClientPacket::MarketGetBack { mode, auction_id } => Some(json!({
            "mode": mode,
            "auctionId": auction_id
        })),
        ClientPacket::MarketSellNow { auction_id } => Some(json!({
            "auctionId": auction_id
        })),
        ClientPacket::MarriageReply { accept_invite }
        | ClientPacket::DivorceReply { accept_invite }
        | ClientPacket::MentorReply { accept_invite } => Some(json!({
            "acceptInvite": accept_invite
        })),
        ClientPacket::AddMentor { name } => Some(json!({
            "name": name
        })),
        ClientPacket::TradeReply { accept_invite } => Some(json!({
            "acceptInvite": accept_invite
        })),
        ClientPacket::TradeGold { amount } => Some(json!({
            "amount": amount
        })),
        ClientPacket::TradeConfirm { locked } => Some(json!({
            "locked": locked
        })),
        ClientPacket::FishingCast { cast_out } => Some(json!({
            "castOut": cast_out
        })),
        ClientPacket::FishingChangeAutocast { auto_cast } => Some(json!({
            "autoCast": auto_cast
        })),
        ClientPacket::SendMail {
            name,
            gold,
            items_idx,
            stamped,
            ..
        } => Some(json!({
            "name": name,
            "gold": gold,
            "itemsIdx": items_idx,
            "stamped": stamped
        })),
        ClientPacket::ReadMail { mail_id }
        | ClientPacket::CollectParcel { mail_id }
        | ClientPacket::DeleteMail { mail_id } => Some(json!({
            "mailId": mail_id
        })),
        ClientPacket::LockMail { mail_id, lock } => Some(json!({
            "mailId": mail_id,
            "lock": lock
        })),
        ClientPacket::MailLockedItem { unique_id, locked } => Some(json!({
            "uniqueId": unique_id,
            "locked": locked
        })),
        ClientPacket::MailCost {
            gold,
            items_idx,
            stamped,
        } => Some(json!({
            "gold": gold,
            "itemsIdx": items_idx,
            "stamped": stamped
        })),
        ClientPacket::UpdateIntelligentCreature {
            creature,
            summon_me,
            unsummon_me,
            release_me,
        } => Some(json!({
            "customName": creature.custom_name,
            "slotIndex": creature.slot_index,
            "summonMe": summon_me,
            "unsummonMe": unsummon_me,
            "releaseMe": release_me
        })),
        ClientPacket::IntelligentCreaturePickup {
            mouse_mode,
            location,
        } => Some(json!({
            "mouseMode": mouse_mode,
            "location": {
                "x": location.x,
                "y": location.y
            }
        })),
        ClientPacket::RequestIntelligentCreatureUpdates { update } => Some(json!({
            "update": update
        })),
        ClientPacket::AddFriend { name, blocked } => Some(json!({
            "name": name,
            "blocked": blocked
        })),
        ClientPacket::RemoveFriend { character_index } => Some(json!({
            "characterIndex": character_index
        })),
        ClientPacket::AddMemo {
            character_index,
            memo,
        } => Some(json!({
            "characterIndex": character_index,
            "memo": memo
        })),
        _ => None,
    }
}

fn server_packet_detail(packet: &ServerPacket) -> Option<Value> {
    match packet {
        ServerPacket::Raw { payload, .. } => Some(json!({
            "rawPayloadLength": payload.len()
        })),
        ServerPacket::ClientVersion { result }
        | ServerPacket::NewAccount { result }
        | ServerPacket::ChangePassword { result }
        | ServerPacket::Login { result }
        | ServerPacket::NewCharacter { result }
        | ServerPacket::DeleteCharacter { result } => Some(json!({
            "result": result
        })),
        ServerPacket::LoginSuccess { characters } => Some(json!({
            "characterCount": characters.len(),
            "characters": characters
                .iter()
                .map(|character| json!({
                    "index": character.index,
                    "name": character.name,
                    "level": character.level,
                    "class": format!("{:?}", character.class),
                    "gender": format!("{:?}", character.gender),
                    "lastAccessBinaryDatetime": character.last_access_binary_datetime
                }))
                .collect::<Vec<_>>()
        })),
        ServerPacket::NewCharacterSuccess { char_info } => Some(json!({
            "index": char_info.index,
            "name": char_info.name,
            "level": char_info.level,
            "class": format!("{:?}", char_info.class),
            "gender": format!("{:?}", char_info.gender),
            "lastAccessBinaryDatetime": char_info.last_access_binary_datetime
        })),
        ServerPacket::DeleteCharacterSuccess { character_index } => Some(json!({
            "characterIndex": character_index
        })),
        ServerPacket::StartGame { result, resolution } => Some(json!({
            "result": result,
            "resolution": resolution
        })),
        ServerPacket::MapInformation { info } => Some(json!({
            "mapIndex": info.map_index,
            "fileName": info.file_name,
            "title": info.title,
            "miniMap": info.mini_map,
            "bigMap": info.big_map,
            "lights": info.lights,
            "flags": info.flags,
            "mapDarkLight": info.map_dark_light,
            "music": info.music,
            "weatherParticles": info.weather_particles
        })),
        ServerPacket::UserLocation { location } => Some(json!({
            "location": {
                "x": location.position.x,
                "y": location.position.y
            },
            "direction": format!("{:?}", location.direction)
        })),
        ServerPacket::UserInformation { info } => Some(json!({
            "objectId": info.object_id,
            "realId": info.real_id,
            "name": info.name,
            "guildName": info.guild_name,
            "guildRank": info.guild_rank,
            "nameColourArgb": info.name_colour_argb,
            "class": format!("{:?}", info.class),
            "gender": format!("{:?}", info.gender),
            "level": info.level,
            "location": {
                "x": info.location.x,
                "y": info.location.y
            },
            "direction": format!("{:?}", info.direction),
            "hair": info.hair,
            "hp": info.hp,
            "mp": info.mp,
            "experience": info.experience,
            "maxExperience": info.max_experience,
            "levelEffects": info.level_effects,
            "hasHero": info.has_hero,
            "heroBehaviour": info.hero_behaviour,
            "gold": info.gold,
            "credit": info.credit,
            "hasExpandedStorage": info.has_expanded_storage,
            "hasStoragePassword": info.has_storage_password,
            "requireStoragePassword": info.require_storage_password,
            "storagePasswordLastSetBinaryDatetime": info.storage_password_last_set_binary_datetime,
            "expandedStorageExpiryTimeBinaryDatetime": info.expanded_storage_expiry_time_binary_datetime,
            "magicCount": info.magic_count,
            "intelligentCreatureCount": info.intelligent_creature_count,
            "summonedCreatureType": info.summoned_creature_type,
            "creatureSummoned": info.creature_summoned,
            "allowObserve": info.allow_observe,
            "observer": info.observer,
            "inventory": user_item_slots_detail(info.inventory.as_deref()),
            "equipment": user_item_slots_detail(info.equipment.as_deref()),
            "questInventory": user_item_slots_detail(info.quest_inventory.as_deref())
        })),
        ServerPacket::ObjectTurn { movement }
        | ServerPacket::ObjectWalk { movement }
        | ServerPacket::ObjectRun { movement } => Some(json!({
            "objectId": movement.object_id,
            "location": {
                "x": movement.position.x,
                "y": movement.position.y
            },
            "direction": format!("{:?}", movement.direction)
        })),
        ServerPacket::ObjectChat {
            object_id,
            text,
            chat_type,
        } => Some(json!({
            "objectId": object_id,
            "text": text,
            "chatType": format!("{:?}", chat_type)
        })),
        ServerPacket::NewItemInfo { info } => Some(json!({
            "index": info.index,
            "name": info.name,
            "type": info.item_type,
            "image": info.image,
            "durability": info.durability,
            "stackSize": info.stack_size
        })),
        ServerPacket::ObjectMonster { info } => Some(json!({
            "objectId": info.object_id,
            "name": info.name,
            "nameColourArgb": info.name_colour_argb,
            "location": {
                "x": info.location.x,
                "y": info.location.y
            },
            "image": info.image,
            "direction": format!("{:?}", info.direction),
            "effect": info.effect,
            "ai": info.ai,
            "light": info.light,
            "dead": info.dead,
            "skeleton": info.skeleton,
            "poison": info.poison,
            "hidden": info.hidden,
            "shockTime": info.shock_time,
            "bindingShotCenter": info.binding_shot_center,
            "extra": info.extra,
            "extraByte": info.extra_byte,
            "masterObjectId": info.master_object_id,
            "rarity": info.rarity,
            "buffs": info.buffs
        })),
        ServerPacket::ObjectNpc { info } => Some(json!({
            "objectId": info.object_id,
            "name": info.name,
            "nameColourArgb": info.name_colour_argb,
            "image": info.image,
            "colourArgb": info.colour_argb,
            "location": {
                "x": info.location.x,
                "y": info.location.y
            },
            "direction": format!("{:?}", info.direction),
            "questIds": info.quest_ids
        })),
        ServerPacket::ObjectSpell { info } => Some(json!({
            "objectId": info.object_id,
            "location": {
                "x": info.location.x,
                "y": info.location.y
            },
            "spell": info.spell as u8,
            "direction": format!("{:?}", info.direction),
            "param": info.param
        })),
        ServerPacket::TradeRequest { name } | ServerPacket::TradeAccept { name } => Some(json!({
            "name": name
        })),
        ServerPacket::TradeGold { amount } => Some(json!({
            "amount": amount
        })),
        ServerPacket::TradeItem { trade_items } => Some(json!({
            "tradeItemCount": trade_items.len(),
            "tradeItems": trade_items
        })),
        ServerPacket::TradeCancel { unlock } => Some(json!({
            "unlock": unlock
        })),
        ServerPacket::NewHeroInfo {
            info,
            storage_index,
        } => Some(json!({
            "index": info.index,
            "name": info.name,
            "level": info.level,
            "class": format!("{:?}", info.class),
            "gender": format!("{:?}", info.gender),
            "storageIndex": storage_index
        })),
        ServerPacket::TakeBackHeroItem { from, to, success }
        | ServerPacket::TransferHeroItem { from, to, success } => Some(json!({
            "from": from,
            "to": to,
            "success": success
        })),
        ServerPacket::HeroHealthChanged { hp, mp } => Some(json!({
            "hp": hp,
            "mp": mp
        })),
        ServerPacket::GainHeroExperience { amount } => Some(json!({
            "amount": amount
        })),
        ServerPacket::HeroLevelChanged {
            level,
            experience,
            max_experience,
        } => Some(json!({
            "level": level,
            "experience": experience,
            "maxExperience": max_experience
        })),
        ServerPacket::HeroCreateRequest { can_create_class } => Some(json!({
            "canCreateClass": can_create_class
        })),
        ServerPacket::NewHero { result } => Some(json!({
            "result": result
        })),
        ServerPacket::UpdateHeroSpawnState { state } => Some(json!({
            "state": state
        })),
        ServerPacket::SetHeroBehaviour { behaviour } => Some(json!({
            "behaviour": behaviour
        })),
        ServerPacket::ManageHeroes {
            maximum_count,
            current_hero,
            heroes,
        } => Some(json!({
            "maximumCount": maximum_count,
            "currentHero": current_hero,
            "heroSlots": heroes.as_ref().map(|heroes| heroes.len()).unwrap_or_default()
        })),
        ServerPacket::ChangeHero { from_index } => Some(json!({
            "fromIndex": from_index
        })),
        ServerPacket::ConsignItem { unique_id, success } => Some(json!({
            "uniqueId": unique_id,
            "success": success
        })),
        ServerPacket::MarketFail { reason } => Some(json!({
            "reason": reason
        })),
        ServerPacket::MarketSuccess { message } => Some(json!({
            "message": message
        })),
        ServerPacket::MarriageRequest { name } | ServerPacket::DivorceRequest { name } => {
            Some(json!({
                "name": name
            }))
        }
        ServerPacket::MentorRequest { name, level } => Some(json!({
            "name": name,
            "level": level
        })),
        ServerPacket::MountUpdate {
            object_id,
            mount_type,
            riding_mount,
        } => Some(json!({
            "objectId": object_id,
            "mountType": mount_type,
            "ridingMount": riding_mount
        })),
        ServerPacket::FishingUpdate {
            object_id,
            fishing,
            progress_percent,
            chance_percent,
            fishing_point,
            found_fish,
        } => Some(json!({
            "objectId": object_id,
            "fishing": fishing,
            "progressPercent": progress_percent,
            "chancePercent": chance_percent,
            "fishingPoint": {
                "x": fishing_point.x,
                "y": fishing_point.y
            },
            "foundFish": found_fish
        })),
        ServerPacket::ReceiveMail { mail } => Some(json!({
            "mailCount": mail.len()
        })),
        ServerPacket::MailLockedItem { unique_id, locked } => Some(json!({
            "uniqueId": unique_id,
            "locked": locked
        })),
        ServerPacket::MailSent { result } | ServerPacket::ParcelCollected { result } => {
            Some(json!({
                "result": result
            }))
        }
        ServerPacket::MailCost { cost } => Some(json!({
            "cost": cost
        })),
        ServerPacket::FriendUpdate { friends } => Some(json!({
            "friendCount": friends.len()
        })),
        ServerPacket::NewIntelligentCreature { creature } => Some(json!({
            "customName": creature.custom_name,
            "slotIndex": creature.slot_index
        })),
        ServerPacket::UpdateIntelligentCreatureList {
            creature_list,
            creature_summoned,
            summoned_creature_type,
            pearl_count,
        } => Some(json!({
            "creatureCount": creature_list.len(),
            "creatureSummoned": creature_summoned,
            "summonedCreatureType": summoned_creature_type,
            "pearlCount": pearl_count
        })),
        ServerPacket::IntelligentCreaturePickup { object_id } => Some(json!({
            "objectId": object_id
        })),
        ServerPacket::ObjectMana { info } => Some(json!({
            "objectId": info.object_id,
            "percent": info.percent
        })),
        ServerPacket::AddBuff { buff } => Some(json!({
            "buffType": buff.buff_type,
            "visible": buff.visible,
            "objectId": buff.object_id,
            "expireTime": buff.expire_time,
            "infinite": buff.infinite,
            "paused": buff.paused,
            "stats": buff.stats,
            "values": buff.values
        })),
        ServerPacket::RemoveBuff {
            buff_type,
            object_id,
        } => Some(json!({
            "buffType": buff_type,
            "objectId": object_id
        })),
        ServerPacket::NewQuestInfo { payload } => Some(json!({
            "rawPayloadLength": payload.len()
        })),
        ServerPacket::NewRecipeInfo { payload } => Some(json!({
            "rawPayloadLength": payload.len()
        })),
        _ => None,
    }
}

fn user_item_slots_detail(items: Option<&[Option<mir2_protocol::UserItem>]>) -> Value {
    let Some(items) = items else {
        return json!({
            "present": false,
            "len": 0,
            "occupied": []
        });
    };
    json!({
        "present": true,
        "len": items.len(),
        "occupied": items
            .iter()
            .enumerate()
            .filter_map(|(slot, item)| {
                item.as_ref().map(|item| json!({
                    "slot": slot,
                    "uniqueId": item.unique_id,
                    "itemIndex": item.item_index,
                    "currentDura": item.current_dura,
                    "maxDura": item.max_dura,
                    "count": item.count,
                    "identified": item.identified,
                    "cursed": item.cursed,
                    "socketSlots": item.slots.len(),
                    "gemCount": item.gem_count,
                    "addedStats": item.added_stats,
                    "soulBoundId": item.soul_bound_id
                }))
            })
            .collect::<Vec<_>>()
    })
}

fn payload_hash(frame: &[u8]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in frame.get(4..).unwrap_or_default() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn trace_payload_hex(frame: &[u8]) -> Option<String> {
    if !env_flag("MIR2_PACKET_TRACE_CAPTURE_PAYLOAD_HEX") {
        return None;
    }
    Some(hex_lower(frame.get(4..).unwrap_or_default()))
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn diff_traces(local: &EndpointTrace, crystal: &EndpointTrace) -> TraceDiff {
    let mut reasons = BTreeSet::new();
    let mut mismatches = Vec::new();
    if !local.ok {
        reasons.insert("endpoint_error".to_string());
    }
    if !crystal.ok {
        reasons.insert("endpoint_error".to_string());
    }

    let count = local.entries.len().max(crystal.entries.len());
    for sequence in 0..count {
        match (local.entries.get(sequence), crystal.entries.get(sequence)) {
            (Some(local), Some(crystal)) => {
                compare_entry(sequence, local, crystal, &mut reasons, &mut mismatches);
            }
            (Some(local), None) => push_mismatch(
                sequence,
                "missing_crystal_packet",
                Some(local),
                None,
                &mut reasons,
                &mut mismatches,
            ),
            (None, Some(crystal)) => push_mismatch(
                sequence,
                "missing_local_packet",
                None,
                Some(crystal),
                &mut reasons,
                &mut mismatches,
            ),
            (None, None) => {}
        }
    }

    let mismatch_reasons: Vec<String> = reasons.into_iter().collect();
    TraceDiff {
        clean: mismatch_reasons.is_empty(),
        compared_entries: count,
        mismatch_reasons,
        mismatches,
    }
}

fn stable_diff_traces(local: &EndpointTrace, crystal: &EndpointTrace) -> TraceDiff {
    let mut reasons = BTreeSet::new();
    let mut mismatches = Vec::new();
    if !local.ok {
        reasons.insert("endpoint_error".to_string());
    }
    if !crystal.ok {
        reasons.insert("endpoint_error".to_string());
    }

    let local_entries: Vec<&TraceEntry> = local
        .entries
        .iter()
        .filter(|entry| stable_ordered_packet(entry))
        .collect();
    let crystal_entries: Vec<&TraceEntry> = crystal
        .entries
        .iter()
        .filter(|entry| stable_ordered_packet(entry))
        .collect();

    let count = local_entries.len().max(crystal_entries.len());
    for sequence in 0..count {
        match (local_entries.get(sequence), crystal_entries.get(sequence)) {
            (Some(local), Some(crystal)) => {
                compare_stable_entry(sequence, local, crystal, &mut reasons, &mut mismatches)
            }
            (Some(local), None) => push_mismatch(
                sequence,
                "missing_crystal_packet",
                Some(local),
                None,
                &mut reasons,
                &mut mismatches,
            ),
            (None, Some(crystal)) => push_mismatch(
                sequence,
                "missing_local_packet",
                None,
                Some(crystal),
                &mut reasons,
                &mut mismatches,
            ),
            (None, None) => {}
        }
    }

    let mismatch_reasons: Vec<String> = reasons.into_iter().collect();
    TraceDiff {
        clean: mismatch_reasons.is_empty(),
        compared_entries: count,
        mismatch_reasons,
        mismatches,
    }
}

fn stable_ordered_packet(entry: &TraceEntry) -> bool {
    if entry.direction == PacketTraceDirection::Server
        && entry.packet == "Unknown"
        && matches!(entry.packet_id, 75 | 77)
    {
        return false;
    }
    !matches!(
        entry.packet.as_str(),
        "ObjectMonster"
            | "ObjectRemove"
            | "ObjectTurn"
            | "ObjectWalk"
            | "ObjectRun"
            | "ObjectNpc"
            | "ObjectAttack"
            | "ObjectRangeAttack"
            | "ObjectSpell"
            | "ObjectStruck"
            | "ObjectHealth"
            | "ObjectMana"
            | "AddBuff"
            | "RemoveBuff"
            | "Struck"
            | "ObjectDied"
            | "NPCUpdate"
            | "NPCResponse"
    )
}

fn compare_stable_entry(
    sequence: usize,
    local: &TraceEntry,
    crystal: &TraceEntry,
    reasons: &mut BTreeSet<String>,
    mismatches: &mut Vec<TraceMismatch>,
) {
    let reason = if local.direction != crystal.direction {
        Some("direction_mismatch")
    } else if local.packet_id != crystal.packet_id {
        Some("packet_id_mismatch")
    } else if local.packet != crystal.packet {
        Some("packet_name_mismatch")
    } else if local.decoded != crystal.decoded {
        Some("decode_status_mismatch")
    } else if local.payload_len != crystal.payload_len {
        Some("payload_length_mismatch")
    } else if local.payload_hash != crystal.payload_hash && !stable_payload_hash_volatile(local) {
        Some("payload_hash_mismatch")
    } else {
        None
    };

    if let Some(reason) = reason {
        push_mismatch(
            sequence,
            reason,
            Some(local),
            Some(crystal),
            reasons,
            mismatches,
        );
    }
}

fn stable_payload_hash_volatile(entry: &TraceEntry) -> bool {
    matches!(
        entry.packet.as_str(),
        "LoginSuccess"
            | "UserInformation"
            | "ObjectNpc"
            | "ObjectChat"
            | "TimeOfDay"
            | "DefaultNPC"
            | "NPCUpdate"
            | "NewCharacterSuccess"
            | "DeleteCharacter"
            | "DeleteCharacterSuccess"
    )
}

fn compare_entry(
    sequence: usize,
    local: &TraceEntry,
    crystal: &TraceEntry,
    reasons: &mut BTreeSet<String>,
    mismatches: &mut Vec<TraceMismatch>,
) {
    let reason = if local.direction != crystal.direction {
        Some("direction_mismatch")
    } else if local.packet_id != crystal.packet_id {
        Some("packet_id_mismatch")
    } else if local.packet != crystal.packet {
        Some("packet_name_mismatch")
    } else if local.decoded != crystal.decoded {
        Some("decode_status_mismatch")
    } else if local.payload_len != crystal.payload_len {
        Some("payload_length_mismatch")
    } else if local.payload_hash != crystal.payload_hash {
        Some("payload_hash_mismatch")
    } else {
        None
    };

    if let Some(reason) = reason {
        push_mismatch(
            sequence,
            reason,
            Some(local),
            Some(crystal),
            reasons,
            mismatches,
        );
    }
}

fn push_mismatch(
    sequence: usize,
    reason: &str,
    local: Option<&TraceEntry>,
    crystal: Option<&TraceEntry>,
    reasons: &mut BTreeSet<String>,
    mismatches: &mut Vec<TraceMismatch>,
) {
    reasons.insert(reason.to_string());
    mismatches.push(TraceMismatch {
        sequence,
        reason: reason.to_string(),
        local: local.map(trace_summary),
        crystal: crystal.map(trace_summary),
    });
}

fn trace_summary(entry: &TraceEntry) -> TraceEntrySummary {
    TraceEntrySummary {
        direction: entry.direction,
        packet_id: entry.packet_id,
        packet: entry.packet.clone(),
        payload_len: entry.payload_len,
        payload_hash: entry.payload_hash.clone(),
        decoded: entry.decoded,
    }
}

fn trace_flows() -> Vec<TraceFlow> {
    vec![
        TraceFlow {
            name: "core_bootstrap",
            description: "ClientVersion, Login, StartGame bootstrap",
            packets: core_bootstrap_packets,
        },
        TraceFlow {
            name: "account_lifecycle",
            description:
                "ClientVersion, NewAccount, ChangePassword, Login, NewCharacter, optional DeleteCharacter",
            packets: account_lifecycle_packets,
        },
        TraceFlow {
            name: "movement_chat_keepalive",
            description: "Login, StartGame, Turn, Walk, Run, Chat, KeepAlive",
            packets: movement_chat_keepalive_packets,
        },
        TraceFlow {
            name: "inventory_storage",
            description:
                "Login, StartGame, MoveItem, SplitItem, DropGold, PickUp, StoreItem, TakeBackItem",
            packets: inventory_storage_packets,
        },
        TraceFlow {
            name: "combat_basic",
            description: "Login, StartGame, Attack, RangeAttack, Harvest",
            packets: combat_basic_packets,
        },
        TraceFlow {
            name: "magic_basic",
            description: "Login, StartGame, MagicKey, Magic, SpellToggle",
            packets: magic_basic_packets,
        },
        TraceFlow {
            name: "storage_password",
            description:
                "Login, StartGame, SetStoragePassword, UnlockStorage, RemoveStoragePassword",
            packets: storage_password_packets,
        },
    ]
}

fn trace_flow_names() -> BTreeSet<&'static str> {
    trace_flows().into_iter().map(|flow| flow.name).collect()
}

fn core_bootstrap_packets(fixture: &Fixture) -> Vec<ClientPacket> {
    logged_in_packets(fixture)
}

fn account_lifecycle_packets(fixture: &Fixture) -> Vec<ClientPacket> {
    let mut packets = vec![
        client_version_packet(),
        ClientPacket::NewAccount {
            account_id: fixture.lifecycle_account.clone(),
            password: lifecycle_password(),
            birth_date_binary: 0,
            user_name: "Trace Fixture".to_string(),
            secret_question: "q".to_string(),
            secret_answer: "a".to_string(),
            email_address: "trace@example.invalid".to_string(),
        },
        ClientPacket::ChangePassword {
            account_id: fixture.lifecycle_account.clone(),
            current_password: lifecycle_password(),
            new_password: lifecycle_new_password(),
        },
        ClientPacket::Login {
            account_id: fixture.lifecycle_account.clone(),
            password: lifecycle_new_password(),
        },
        ClientPacket::NewCharacter {
            name: fixture.lifecycle_character.clone(),
            gender: MirGender::Male,
            class: MirClass::Warrior,
        },
    ];

    if !env_flag("MIR2_PACKET_TRACE_KEEP_LIFECYCLE_CHARACTER") {
        packets.push(ClientPacket::DeleteCharacter {
            character_index: DYNAMIC_NEW_CHARACTER_INDEX,
        });
    }

    packets
}

fn movement_chat_keepalive_packets(fixture: &Fixture) -> Vec<ClientPacket> {
    let mut packets = logged_in_packets(fixture);
    packets.extend([
        ClientPacket::Turn {
            direction: MirDirection::Right,
        },
        ClientPacket::Walk {
            direction: MirDirection::Right,
        },
        ClientPacket::Run {
            direction: MirDirection::Right,
        },
        ClientPacket::Chat {
            message: "trace hello".to_string(),
        },
        ClientPacket::KeepAlive {
            time: now_unix_ms() as i64,
        },
    ]);
    packets
}

fn inventory_storage_packets(fixture: &Fixture) -> Vec<ClientPacket> {
    let mut packets = logged_in_packets(fixture);
    packets.extend([
        ClientPacket::MoveItem {
            grid: MirGridType::Inventory,
            from: 0,
            to: 10,
        },
        ClientPacket::SplitItem {
            grid: MirGridType::Inventory,
            unique_id: 1,
            count: 1,
        },
        ClientPacket::DropGold { amount: 1 },
        ClientPacket::PickUp,
        ClientPacket::StoreItem { from: 1, to: 0 },
        ClientPacket::TakeBackItem { from: 0, to: 1 },
    ]);
    packets
}

fn combat_basic_packets(fixture: &Fixture) -> Vec<ClientPacket> {
    let mut packets = logged_in_packets(fixture);
    packets.extend([
        ClientPacket::Attack {
            direction: MirDirection::Right,
            spell: Spell::None,
        },
        ClientPacket::RangeAttack {
            direction: MirDirection::Right,
            location: Point { x: 331, y: 270 },
            target_id: 0,
            target_location: Point { x: 332, y: 270 },
        },
        ClientPacket::Harvest {
            direction: MirDirection::Right,
        },
    ]);
    packets
}

fn magic_basic_packets(fixture: &Fixture) -> Vec<ClientPacket> {
    let mut packets = logged_in_packets(fixture);
    packets.extend([
        ClientPacket::MagicKey {
            spell: Spell::Fury,
            key: 7,
            old_key: 0,
        },
        ClientPacket::Magic {
            object_id: 0,
            spell: Spell::Fury,
            direction: MirDirection::Right,
            target_id: 0,
            location: Point { x: 334, y: 267 },
            spell_target_lock: false,
        },
        ClientPacket::SpellToggle {
            spell: Spell::Slaying,
            toggle_state: 1,
        },
    ]);
    packets
}

fn storage_password_packets(fixture: &Fixture) -> Vec<ClientPacket> {
    let mut packets = logged_in_packets(fixture);
    packets.extend([
        ClientPacket::SetStoragePassword {
            current_password: String::new(),
            new_password: "Safe123".to_string(),
        },
        ClientPacket::UnlockStorage {
            password: "Safe123".to_string(),
        },
        ClientPacket::RemoveStoragePassword {
            current_password: "Safe123".to_string(),
        },
    ]);
    packets
}

fn logged_in_packets(fixture: &Fixture) -> Vec<ClientPacket> {
    vec![
        client_version_packet(),
        login_packet(&fixture.account),
        ClientPacket::StartGame {
            character_index: fixture.character_index,
        },
    ]
}

fn client_version_packet() -> ClientPacket {
    ClientPacket::ClientVersion {
        version_hash: vec![1, 0, 0, 0],
    }
}

fn login_packet(account: &str) -> ClientPacket {
    ClientPacket::Login {
        account_id: account.to_string(),
        password: env::var("MIR2_PACKET_TRACE_PASSWORD").unwrap_or_else(|_| "demo".to_string()),
    }
}

fn fixture_from_env() -> Fixture {
    let mode = env::var("MIR2_PACKET_TRACE_FIXTURE_MODE").unwrap_or_else(|_| "ephemeral".into());
    let stamp = now_unix_ms();
    let account = env::var("MIR2_PACKET_TRACE_ACCOUNT").unwrap_or_else(|_| "demo".into());
    let lifecycle_account = env::var("MIR2_PACKET_TRACE_LIFECYCLE_ACCOUNT").unwrap_or_else(|_| {
        if mode == "stable" {
            "TraceFixture".to_string()
        } else {
            format!("T{:012}", stamp % 1_000_000_000_000)
        }
    });
    let lifecycle_character =
        env::var("MIR2_PACKET_TRACE_LIFECYCLE_CHARACTER").unwrap_or_else(|_| {
            if mode == "stable" {
                "TraceOne".to_string()
            } else {
                format!("Trace{:07}", stamp % 10_000_000)
            }
        });

    Fixture {
        mode,
        account,
        lifecycle_account,
        lifecycle_character: lifecycle_character.chars().take(12).collect::<String>(),
        character: env::var("MIR2_PACKET_TRACE_CHARACTER").unwrap_or_else(|_| "Scout".into()),
        character_index: env::var("MIR2_PACKET_TRACE_CHARACTER_INDEX")
            .ok()
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(0),
    }
}

fn lifecycle_password() -> String {
    env::var("MIR2_PACKET_TRACE_LIFECYCLE_PASSWORD").unwrap_or_else(|_| "Tracepass1".into())
}

fn lifecycle_new_password() -> String {
    env::var("MIR2_PACKET_TRACE_LIFECYCLE_NEW_PASSWORD").unwrap_or_else(|_| "Tracenew1".into())
}

fn enforce_requirements(
    local_ok: bool,
    crystal_ok: bool,
    exact_diff_clean: bool,
    accepted_diff_clean: bool,
    acceptance_mode: DiffAcceptanceMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let require_local = env_flag("MIR2_PACKET_TRACE_REQUIRE_LOCAL");
    let require_crystal = env_flag("MIR2_PACKET_TRACE_REQUIRE_CRYSTAL");
    let require_exact_diff_clean = env_flag("MIR2_PACKET_TRACE_REQUIRE_DIFF_CLEAN");
    let require_accepted_diff_clean =
        require_crystal || env_flag("MIR2_PACKET_TRACE_REQUIRE_ACCEPTED_DIFF_CLEAN");

    if require_local && !local_ok {
        return Err("MIR2_PACKET_TRACE_REQUIRE_LOCAL is set and no local trace succeeded".into());
    }
    if require_crystal && !crystal_ok {
        return Err(
            "MIR2_PACKET_TRACE_REQUIRE_CRYSTAL is set and no Crystal trace succeeded".into(),
        );
    }
    if require_exact_diff_clean && !exact_diff_clean {
        return Err(
            "MIR2_PACKET_TRACE_REQUIRE_DIFF_CLEAN is set and packet diff is not clean".into(),
        );
    }
    if require_accepted_diff_clean && !accepted_diff_clean {
        return Err(format!(
            "packet diff is not accepted by {} mode; set MIR2_PACKET_TRACE_REQUIRE_DIFF_CLEAN for strict exact diagnostics or MIR2_PACKET_TRACE_ACCEPT_STABLE_DIFF=1 for the accepted stable comparator",
            acceptance_mode.as_str()
        )
        .into());
    }
    Ok(())
}

fn diff_acceptance_mode_from_env() -> DiffAcceptanceMode {
    let explicit = env::var("MIR2_PACKET_TRACE_DIFF_ACCEPTANCE")
        .or_else(|_| env::var("MIR2_PACKET_TRACE_ACCEPTANCE_MODE"))
        .unwrap_or_default();
    match explicit.trim().to_ascii_lowercase().as_str() {
        "stable" | "stable-diff" | "stable_diff" => DiffAcceptanceMode::Stable,
        "exact" | "strict" | "strict-exact" | "strict_exact" => DiffAcceptanceMode::Exact,
        _ if env_flag("MIR2_PACKET_TRACE_ACCEPT_STABLE_DIFF") => DiffAcceptanceMode::Stable,
        _ => DiffAcceptanceMode::Exact,
    }
}

fn env_flag(name: &str) -> bool {
    matches!(
        env::var(name).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    )
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes)?;
    Ok(())
}

fn parity_matrix_path() -> PathBuf {
    let cwd_path = PathBuf::from(PARITY_MATRIX_PATH);
    if cwd_path.exists() {
        return cwd_path;
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(PARITY_MATRIX_PATH)
}

fn sanitize_file_stem(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_flow_names_are_stable_for_matrix_references() {
        let names = trace_flow_names();
        for expected in [
            "core_bootstrap",
            "account_lifecycle",
            "movement_chat_keepalive",
            "inventory_storage",
            "combat_basic",
            "magic_basic",
            "storage_password",
        ] {
            assert!(names.contains(expected), "missing flow {expected}");
        }
    }

    #[test]
    fn parity_matrix_defines_required_categories_and_trace_flows() {
        let matrix_text = fs::read_to_string(parity_matrix_path())
            .expect("parity matrix should be readable from repo root");
        let matrix: ParityMatrix =
            serde_json::from_str(&matrix_text).expect("parity matrix should decode");
        let trace_flows: Vec<&ParityFlow> = matrix
            .flows
            .iter()
            .filter(|flow| flow.trace_flow.is_some())
            .collect();
        assert!(
            trace_flows.len() >= 6,
            "expected representative TCP trace matrix coverage"
        );

        let names = trace_flow_names();
        for flow in trace_flows {
            let trace_flow = flow.trace_flow.as_deref().unwrap();
            assert!(
                names.contains(trace_flow),
                "matrix flow {} references unknown traceFlow {}",
                flow.id,
                trace_flow
            );
        }
    }

    #[test]
    fn matrix_capture_order_runs_stateful_movement_last() {
        let flows = vec![
            ParityFlow {
                id: "movement".to_string(),
                trace_flow: Some("movement_chat_keepalive".to_string()),
            },
            ParityFlow {
                id: "inventory".to_string(),
                trace_flow: Some("inventory_storage".to_string()),
            },
            ParityFlow {
                id: "storage".to_string(),
                trace_flow: Some("storage_password".to_string()),
            },
        ];

        let order = matrix_capture_order(&flows, &trace_flow_names());

        assert_eq!(
            order,
            vec![
                "inventory_storage".to_string(),
                "storage_password".to_string(),
                "movement_chat_keepalive".to_string()
            ]
        );
    }

    #[test]
    fn logged_in_flows_start_with_version_and_use_fixture_character_index() {
        let fixture = Fixture {
            mode: "stable".to_string(),
            account: "demo".to_string(),
            lifecycle_account: "TraceFixture".to_string(),
            lifecycle_character: "TraceOne".to_string(),
            character: "Scout".to_string(),
            character_index: 6,
        };

        let packets = logged_in_packets(&fixture);

        assert!(matches!(packets[0], ClientPacket::ClientVersion { .. }));
        assert!(matches!(
            packets[2],
            ClientPacket::StartGame { character_index: 6 }
        ));
    }

    #[test]
    fn account_lifecycle_flow_uses_crystal_login_stage_order() {
        let fixture = Fixture {
            mode: "stable".to_string(),
            account: "demo".to_string(),
            lifecycle_account: "TraceFixture".to_string(),
            lifecycle_character: "TraceOne".to_string(),
            character: "Scout".to_string(),
            character_index: 0,
        };

        let packets = account_lifecycle_packets(&fixture);

        assert!(matches!(packets[0], ClientPacket::ClientVersion { .. }));
        assert!(matches!(packets[1], ClientPacket::NewAccount { .. }));
        assert!(matches!(packets[2], ClientPacket::ChangePassword { .. }));
        assert!(matches!(packets[3], ClientPacket::Login { .. }));
        assert!(matches!(packets[4], ClientPacket::NewCharacter { .. }));
        assert!(matches!(
            packets[5],
            ClientPacket::DeleteCharacter {
                character_index: DYNAMIC_NEW_CHARACTER_INDEX
            }
        ));
    }

    #[test]
    fn account_lifecycle_can_keep_created_character_for_client_qa() {
        let key = "MIR2_PACKET_TRACE_KEEP_LIFECYCLE_CHARACTER";
        let previous = env::var_os(key);
        env::set_var(key, "1");

        let fixture = Fixture {
            mode: "stable".to_string(),
            account: "demo".to_string(),
            lifecycle_account: "TraceFixture".to_string(),
            lifecycle_character: "TraceOne".to_string(),
            character: "Scout".to_string(),
            character_index: 0,
        };

        let packets = account_lifecycle_packets(&fixture);

        if let Some(value) = previous {
            env::set_var(key, value);
        } else {
            env::remove_var(key);
        }

        assert!(matches!(packets[4], ClientPacket::NewCharacter { .. }));
        assert!(
            !packets
                .iter()
                .any(|packet| matches!(packet, ClientPacket::DeleteCharacter { .. })),
            "keep-character mode should not delete the QA fixture character"
        );
    }

    #[test]
    fn matrix_account_lifecycle_derives_unique_bounded_fixture_names() {
        let fixture = Fixture {
            mode: "stable".to_string(),
            account: "demo".to_string(),
            lifecycle_account: "TraceFixture".to_string(),
            lifecycle_character: "TraceOneName".to_string(),
            character: "Scout".to_string(),
            character_index: 0,
        };

        let first = fixture_for_matrix_flow(&fixture, "account_lifecycle", 1);
        let second = fixture_for_matrix_flow(&fixture, "account_lifecycle", 2);
        let unchanged = fixture_for_matrix_flow(&fixture, "core_bootstrap", 1);

        assert_eq!(first.lifecycle_account, "TraceFixtur01");
        assert_eq!(second.lifecycle_account, "TraceFixtur02");
        assert_eq!(first.lifecycle_character, "TraceOneNa01");
        assert_eq!(second.lifecycle_character, "TraceOneNa02");
        assert_eq!(unchanged.lifecycle_account, "TraceFixture");
        assert_eq!(unchanged.lifecycle_character, "TraceOneName");
    }

    #[test]
    fn login_flows_request_unrecorded_logout_cleanup() {
        let fixture = Fixture {
            mode: "stable".to_string(),
            account: "demo".to_string(),
            lifecycle_account: "TraceFixture".to_string(),
            lifecycle_character: "TraceOne".to_string(),
            character: "Scout".to_string(),
            character_index: 0,
        };

        assert!(flow_needs_logout_cleanup(&core_bootstrap_packets(&fixture)));
        assert!(flow_needs_logout_cleanup(&account_lifecycle_packets(
            &fixture
        )));
        assert!(!flow_needs_logout_cleanup(&[client_version_packet()]));
    }

    #[test]
    fn dynamic_delete_character_uses_last_created_character_index() {
        let packet = resolve_trace_packet(
            &ClientPacket::DeleteCharacter {
                character_index: DYNAMIC_NEW_CHARACTER_INDEX,
            },
            &EndpointCaptureState {
                last_new_character_index: Some(42),
            },
        );

        assert!(matches!(
            packet,
            ClientPacket::DeleteCharacter {
                character_index: 42
            }
        ));
    }

    #[test]
    fn trace_requirements_fail_when_required_crystal_capture_is_missing() {
        env::set_var("MIR2_PACKET_TRACE_REQUIRE_CRYSTAL", "1");
        let err =
            enforce_requirements(true, false, true, true, DiffAcceptanceMode::Exact).unwrap_err();
        env::remove_var("MIR2_PACKET_TRACE_REQUIRE_CRYSTAL");
        assert!(err
            .to_string()
            .contains("MIR2_PACKET_TRACE_REQUIRE_CRYSTAL"));
    }

    #[test]
    fn trace_requirements_fail_when_required_clean_diff_is_missing() {
        env::set_var("MIR2_PACKET_TRACE_REQUIRE_DIFF_CLEAN", "1");
        let err =
            enforce_requirements(true, true, false, true, DiffAcceptanceMode::Stable).unwrap_err();
        env::remove_var("MIR2_PACKET_TRACE_REQUIRE_DIFF_CLEAN");
        assert!(err
            .to_string()
            .contains("MIR2_PACKET_TRACE_REQUIRE_DIFF_CLEAN"));
    }

    #[test]
    fn trace_requirements_accept_stable_diff_when_mode_is_explicit() {
        env::set_var("MIR2_PACKET_TRACE_REQUIRE_CRYSTAL", "1");
        let result = enforce_requirements(true, true, false, true, DiffAcceptanceMode::Stable);
        env::remove_var("MIR2_PACKET_TRACE_REQUIRE_CRYSTAL");
        assert!(result.is_ok());
    }

    #[test]
    fn payload_hash_is_stable_fnv1a64() {
        assert_eq!(payload_hash(&[4, 0, 1, 0]), "fnv1a64:cbf29ce484222325");
        assert_eq!(
            payload_hash(&[5, 0, 1, 0, 0xaa]),
            "fnv1a64:af64274c86026bfd"
        );
    }

    #[test]
    fn stable_diff_ignores_known_dynamic_hashes_and_dynamic_objects() {
        let mut local = test_trace(vec![
            test_entry(
                0,
                PacketTraceDirection::Server,
                4,
                "LoginSuccess",
                8,
                "local-login",
            ),
            test_entry(
                1,
                PacketTraceDirection::Server,
                5,
                "ObjectMonster",
                40,
                "local-monster",
            ),
            test_entry(
                2,
                PacketTraceDirection::Server,
                6,
                "ObjectNpc",
                32,
                "local-npc",
            ),
            test_entry(
                3,
                PacketTraceDirection::Server,
                26,
                "ObjectRemove",
                4,
                "local-remove",
            ),
            test_entry(4, PacketTraceDirection::Server, 61, "TimeOfDay", 1, "time"),
            test_entry(
                5,
                PacketTraceDirection::Server,
                22,
                "NewCharacterSuccess",
                28,
                "local-character-index",
            ),
        ]);
        let mut crystal = test_trace(vec![
            test_entry(
                0,
                PacketTraceDirection::Server,
                4,
                "LoginSuccess",
                8,
                "crystal-login",
            ),
            test_entry(
                1,
                PacketTraceDirection::Server,
                5,
                "ObjectMonster",
                40,
                "crystal-monster",
            ),
            test_entry(
                2,
                PacketTraceDirection::Server,
                6,
                "ObjectNpc",
                32,
                "crystal-npc",
            ),
            test_entry(
                3,
                PacketTraceDirection::Server,
                26,
                "ObjectRemove",
                4,
                "crystal-remove",
            ),
            test_entry(4, PacketTraceDirection::Server, 61, "TimeOfDay", 1, "time"),
            test_entry(
                5,
                PacketTraceDirection::Server,
                22,
                "NewCharacterSuccess",
                28,
                "crystal-character-index",
            ),
        ]);

        assert!(stable_diff_traces(&local, &crystal).clean);

        crystal.entries.push(test_entry(
            6,
            PacketTraceDirection::Server,
            26,
            "ObjectRemove",
            4,
            "extra-remove",
        ));
        crystal.entries.push(test_entry(
            7,
            PacketTraceDirection::Server,
            5,
            "ObjectMonster",
            40,
            "extra-monster",
        ));
        crystal.entries.push(test_entry(
            8,
            PacketTraceDirection::Server,
            72,
            "ObjectAttack",
            16,
            "extra-attack",
        ));
        crystal.entries.push(test_entry(
            9,
            PacketTraceDirection::Server,
            149,
            "ObjectSpell",
            15,
            "extra-spell",
        ));
        crystal.entries.push(test_entry(
            10,
            PacketTraceDirection::Server,
            73,
            "Struck",
            4,
            "extra-struck",
        ));
        crystal.entries.push(test_entry(
            11,
            PacketTraceDirection::Server,
            75,
            "Unknown",
            9,
            "extra-dynamic-unknown",
        ));
        crystal.entries.push(test_entry(
            12,
            PacketTraceDirection::Server,
            77,
            "Unknown",
            8,
            "extra-dynamic-health",
        ));
        crystal.entries.push(test_entry(
            13,
            PacketTraceDirection::Server,
            187,
            "NPCUpdate",
            4,
            "extra-npc-update",
        ));
        crystal.entries.push(test_entry(
            14,
            PacketTraceDirection::Server,
            93,
            "NPCResponse",
            4,
            "extra-npc-response",
        ));
        assert!(stable_diff_traces(&local, &crystal).clean);

        local.entries[4].payload_hash = "different-time".to_string();
        assert!(stable_diff_traces(&local, &test_trace(crystal.entries[..6].to_vec())).clean);

        local.entries.push(test_entry(
            6,
            PacketTraceDirection::Server,
            1,
            "ClientVersion",
            1,
            "local-version",
        ));
        let mut crystal_with_static_payload = test_trace(crystal.entries[..6].to_vec());
        crystal_with_static_payload.entries.push(test_entry(
            6,
            PacketTraceDirection::Server,
            1,
            "ClientVersion",
            1,
            "crystal-version",
        ));
        let diff = stable_diff_traces(&local, &crystal_with_static_payload);
        assert!(!diff.clean);
        assert!(diff
            .mismatch_reasons
            .contains(&"payload_hash_mismatch".to_string()));
    }

    #[test]
    fn sanitize_file_stem_keeps_matrix_paths_portable() {
        assert_eq!(
            sanitize_file_stem("account.version_login_start"),
            "account-version_login_start"
        );
    }

    #[test]
    fn matrix_summary_counts_local_crystal_and_diff_statuses() {
        let artifacts = vec![
            MatrixEntry {
                matrix_id: "ok".to_string(),
                trace_flow: "core_bootstrap".to_string(),
                path: "ok.json".to_string(),
                local_ok: true,
                crystal_ok: Some(true),
                diff_clean: Some(true),
                stable_diff_clean: Some(true),
                accepted_packet_parity: true,
            },
            MatrixEntry {
                matrix_id: "dirty".to_string(),
                trace_flow: "core_bootstrap".to_string(),
                path: "dirty.json".to_string(),
                local_ok: true,
                crystal_ok: Some(true),
                diff_clean: Some(false),
                stable_diff_clean: Some(true),
                accepted_packet_parity: true,
            },
            MatrixEntry {
                matrix_id: "local-failed".to_string(),
                trace_flow: "core_bootstrap".to_string(),
                path: "local-failed.json".to_string(),
                local_ok: false,
                crystal_ok: None,
                diff_clean: None,
                stable_diff_clean: None,
                accepted_packet_parity: false,
            },
        ];
        let skipped = vec![MatrixSkip {
            matrix_id: "ui-only".to_string(),
            reason: "matrix entry does not declare traceFlow".to_string(),
        }];

        let summary = matrix_summary(&artifacts, &skipped, DiffAcceptanceMode::Stable);

        assert_eq!(summary.acceptance_mode, "stable");
        assert_eq!(summary.artifact_count, 3);
        assert_eq!(summary.skipped_count, 1);
        assert_eq!(summary.local_ok_count, 2);
        assert_eq!(summary.local_failed_count, 1);
        assert_eq!(summary.crystal_ok_count, 2);
        assert_eq!(summary.crystal_missing_count, 1);
        assert_eq!(summary.diff_clean_count, 1);
        assert_eq!(summary.diff_dirty_count, 1);
        assert_eq!(summary.diff_missing_count, 1);
        assert_eq!(summary.stable_diff_clean_count, 2);
        assert_eq!(summary.stable_diff_dirty_count, 0);
        assert_eq!(summary.stable_diff_missing_count, 1);
        assert_eq!(summary.accepted_live_comparison_count, 1);
        assert_eq!(summary.accepted_stable_live_comparison_count, 2);
        assert_eq!(summary.accepted_packet_parity_count, 2);
        assert!(!summary.packet_parity_accepted);
    }

    fn test_trace(entries: Vec<TraceEntry>) -> EndpointTrace {
        EndpointTrace {
            endpoint: "test".to_string(),
            ok: true,
            elapsed_ms: 0,
            error: None,
            entries,
        }
    }

    fn test_entry(
        sequence: usize,
        direction: PacketTraceDirection,
        packet_id: i16,
        packet: &str,
        payload_len: usize,
        payload_hash: &str,
    ) -> TraceEntry {
        TraceEntry {
            sequence,
            elapsed_ms: 0,
            direction,
            packet_id,
            packet: packet.to_string(),
            payload_len,
            payload_hash: payload_hash.to_string(),
            payload_hex: None,
            decoded: true,
            decode_error: None,
            detail: None,
        }
    }
}
