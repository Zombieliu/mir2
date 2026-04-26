use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mir2_protocol::{
    client_packet_name, decode_frame, decode_server_packet, encode_client_packet, ClientPacket,
    MirClass, MirDirection, MirGender, MirGridType, PacketTraceDirection, Point, ServerPacketId,
    Spell,
};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{timeout, Instant};

const DEFAULT_GATEWAY_TCP_ADDR: &str = "127.0.0.1:7000";
const DEFAULT_TRACE_OUT: &str = "docs/generated/packet-traces/latest.json";
const DEFAULT_MATRIX_OUT_DIR: &str = "docs/generated/packet-traces/matrix";
const PARITY_MATRIX_PATH: &str = "docs/parity-matrix.json";
const READ_DRAIN_MS: u64 = 80;
const READ_STEP_TIMEOUT_MS: u64 = 750;
const CONNECT_TIMEOUT_MS: u64 = 1_500;

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
}

#[derive(Debug, Serialize)]
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
    accepted_live_comparison_count: usize,
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
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MatrixSkip {
    matrix_id: String,
    reason: String,
}

#[derive(Debug, Serialize)]
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
    decoded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    decode_error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TraceDiff {
    clean: bool,
    compared_entries: usize,
    mismatch_reasons: Vec<String>,
    mismatches: Vec<TraceMismatch>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TraceMismatch {
    sequence: usize,
    reason: String,
    local: Option<TraceEntrySummary>,
    crystal: Option<TraceEntrySummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TraceEntrySummary {
    direction: PacketTraceDirection,
    packet_id: i16,
    packet: String,
    payload_len: usize,
    payload_hash: String,
    decoded: bool,
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
    if args.iter().any(|arg| arg == "--matrix") {
        let report = capture_matrix(&fixture).await?;
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
        )?;
        return Ok(());
    }

    let flow_name =
        env::var("MIR2_PACKET_TRACE_FLOW").unwrap_or_else(|_| "core_bootstrap".to_string());
    let artifact = capture_flow(&flow_name, &fixture).await?;
    let out = env::var("MIR2_PACKET_TRACE_OUT").unwrap_or_else(|_| DEFAULT_TRACE_OUT.to_string());
    write_json(Path::new(&out), &artifact)?;
    println!(
        "wrote {out} (flow={}, local_ok={}, crystal_ok={:?}, diff_clean={:?})",
        artifact.flow,
        artifact.local.ok,
        artifact.crystal.as_ref().map(|trace| trace.ok),
        artifact.diff.as_ref().map(|diff| diff.clean)
    );
    enforce_requirements(
        artifact.local.ok,
        artifact.crystal.as_ref().is_some_and(|trace| trace.ok),
        artifact.diff.as_ref().is_some_and(|diff| diff.clean),
    )?;

    Ok(())
}

async fn capture_matrix(fixture: &Fixture) -> Result<MatrixArtifact, Box<dyn std::error::Error>> {
    let out_dir = env::var("MIR2_PACKET_TRACE_MATRIX_OUT_DIR")
        .unwrap_or_else(|_| DEFAULT_MATRIX_OUT_DIR.into());
    fs::create_dir_all(&out_dir)?;

    let matrix_text = fs::read_to_string(parity_matrix_path())?;
    let matrix: ParityMatrix = serde_json::from_str(&matrix_text)?;
    let flow_names = trace_flow_names();
    let mut artifacts = Vec::new();
    let mut skipped = Vec::new();

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

        let artifact = capture_flow(&trace_flow, fixture).await?;
        let file_name = format!("{}.json", sanitize_file_stem(&matrix_flow.id));
        let path = Path::new(&out_dir).join(file_name);
        write_json(&path, &artifact)?;
        artifacts.push(MatrixEntry {
            matrix_id: matrix_flow.id,
            trace_flow,
            path: path.to_string_lossy().into_owned(),
            local_ok: artifact.local.ok,
            crystal_ok: artifact.crystal.as_ref().map(|trace| trace.ok),
            diff_clean: artifact.diff.as_ref().map(|diff| diff.clean),
        });
    }

    let report = MatrixArtifact {
        schema_version: 1,
        generated_at_unix_ms: now_unix_ms(),
        fixture: fixture.clone(),
        summary: matrix_summary(&artifacts, &skipped),
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

fn matrix_summary(artifacts: &[MatrixEntry], skipped: &[MatrixSkip]) -> MatrixSummary {
    MatrixSummary {
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
        accepted_live_comparison_count: artifacts
            .iter()
            .filter(|entry| {
                entry.local_ok && entry.crystal_ok == Some(true) && entry.diff_clean == Some(true)
            })
            .count(),
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
    let local = capture_endpoint(&local_addr, &(flow.packets)(fixture)).await;
    let crystal = match env::var("MIR2_CRYSTAL_TCP_ADDR") {
        Ok(addr) if !addr.trim().is_empty() => {
            Some(capture_endpoint(addr.trim(), &(flow.packets)(fixture)).await)
        }
        _ => None,
    };
    let diff = crystal.as_ref().map(|crystal| diff_traces(&local, crystal));

    Ok(TraceArtifact {
        schema_version: 1,
        generated_at_unix_ms: now_unix_ms(),
        flow: flow.name.to_string(),
        fixture: fixture.clone(),
        local,
        crystal,
        diff,
    })
}

async fn capture_endpoint(addr: &str, packets: &[ClientPacket]) -> EndpointTrace {
    let started = Instant::now();
    let mut entries = Vec::new();
    let result = async {
        let mut stream = timeout(
            Duration::from_millis(CONNECT_TIMEOUT_MS),
            TcpStream::connect(addr),
        )
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "connect timeout"))??;

        drain_server_packets(&mut stream, started, &mut entries).await?;
        for packet in packets {
            let bytes = encode_client_packet(packet)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
            entries.push(trace_client_entry(
                entries.len(),
                started.elapsed().as_millis(),
                packet,
                &bytes,
            ));
            stream.write_all(&bytes).await?;
            stream.flush().await?;
            drain_server_packets(&mut stream, started, &mut entries).await?;
        }
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
) -> io::Result<()> {
    loop {
        match timeout(Duration::from_millis(READ_DRAIN_MS), read_frame(stream)).await {
            Ok(Ok(frame)) => {
                entries.push(trace_server_entry(
                    entries.len(),
                    started.elapsed().as_millis(),
                    &frame,
                ));
            }
            Ok(Err(error)) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            Ok(Err(error)) => return Err(error),
            Err(_) => return Ok(()),
        }
    }
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
        decoded: true,
        decode_error: None,
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
            decoded: true,
            decode_error: None,
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
            decoded: false,
            decode_error: Some(error.to_string()),
        },
    }
}

fn payload_hash(frame: &[u8]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in frame.get(4..).unwrap_or_default() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
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
            description: "NewAccount, Login, NewCharacter, DeleteCharacter, ChangePassword, LogOut",
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
    vec![
        ClientPacket::ClientVersion {
            version_hash: vec![1, 0, 0, 0],
        },
        login_packet(&fixture.account),
        ClientPacket::StartGame { character_index: 0 },
    ]
}

fn account_lifecycle_packets(fixture: &Fixture) -> Vec<ClientPacket> {
    vec![
        ClientPacket::NewAccount {
            account_id: fixture.lifecycle_account.clone(),
            password: lifecycle_password(),
            birth_date_binary: 0,
            user_name: "Trace Fixture".to_string(),
            secret_question: "q".to_string(),
            secret_answer: "a".to_string(),
            email_address: "trace@example.invalid".to_string(),
        },
        ClientPacket::Login {
            account_id: fixture.lifecycle_account.clone(),
            password: lifecycle_password(),
        },
        ClientPacket::NewCharacter {
            name: fixture.lifecycle_character.clone(),
            gender: MirGender::Male,
            class: MirClass::Warrior,
        },
        ClientPacket::DeleteCharacter { character_index: 0 },
        ClientPacket::ChangePassword {
            account_id: fixture.lifecycle_account.clone(),
            current_password: lifecycle_password(),
            new_password: lifecycle_new_password(),
        },
        ClientPacket::Login {
            account_id: fixture.lifecycle_account.clone(),
            password: lifecycle_new_password(),
        },
        ClientPacket::LogOut,
    ]
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
            direction: MirDirection::Down,
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
        login_packet(&fixture.account),
        ClientPacket::StartGame { character_index: 0 },
    ]
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
            "trace-fixture".to_string()
        } else {
            format!("trace-{stamp}")
        }
    });
    let lifecycle_character = env::var("MIR2_PACKET_TRACE_CHARACTER").unwrap_or_else(|_| {
        if mode == "stable" {
            "TraceOne".to_string()
        } else {
            format!("Trace{}", stamp % 100_000)
        }
    });

    Fixture {
        mode,
        account,
        lifecycle_account,
        lifecycle_character: lifecycle_character.chars().take(12).collect::<String>(),
        character: env::var("MIR2_PACKET_TRACE_CHARACTER").unwrap_or_else(|_| "Scout".into()),
    }
}

fn lifecycle_password() -> String {
    env::var("MIR2_PACKET_TRACE_LIFECYCLE_PASSWORD").unwrap_or_else(|_| "trace-pass".into())
}

fn lifecycle_new_password() -> String {
    env::var("MIR2_PACKET_TRACE_LIFECYCLE_NEW_PASSWORD").unwrap_or_else(|_| "trace-new-pass".into())
}

fn enforce_requirements(
    local_ok: bool,
    crystal_ok: bool,
    diff_clean: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let require_local = env_flag("MIR2_PACKET_TRACE_REQUIRE_LOCAL");
    let require_crystal = env_flag("MIR2_PACKET_TRACE_REQUIRE_CRYSTAL");
    let require_diff_clean = require_crystal || env_flag("MIR2_PACKET_TRACE_REQUIRE_DIFF_CLEAN");

    if require_local && !local_ok {
        return Err("MIR2_PACKET_TRACE_REQUIRE_LOCAL is set and no local trace succeeded".into());
    }
    if require_crystal && !crystal_ok {
        return Err(
            "MIR2_PACKET_TRACE_REQUIRE_CRYSTAL is set and no Crystal trace succeeded".into(),
        );
    }
    if require_diff_clean && !diff_clean {
        return Err(
            "MIR2_PACKET_TRACE_REQUIRE_DIFF_CLEAN is set and packet diff is not clean".into(),
        );
    }
    Ok(())
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
    fn trace_requirements_fail_when_required_crystal_capture_is_missing() {
        env::set_var("MIR2_PACKET_TRACE_REQUIRE_CRYSTAL", "1");
        let err = enforce_requirements(true, false, true).unwrap_err();
        env::remove_var("MIR2_PACKET_TRACE_REQUIRE_CRYSTAL");
        assert!(err
            .to_string()
            .contains("MIR2_PACKET_TRACE_REQUIRE_CRYSTAL"));
    }

    #[test]
    fn trace_requirements_fail_when_required_clean_diff_is_missing() {
        env::set_var("MIR2_PACKET_TRACE_REQUIRE_DIFF_CLEAN", "1");
        let err = enforce_requirements(true, true, false).unwrap_err();
        env::remove_var("MIR2_PACKET_TRACE_REQUIRE_DIFF_CLEAN");
        assert!(err
            .to_string()
            .contains("MIR2_PACKET_TRACE_REQUIRE_DIFF_CLEAN"));
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
            },
            MatrixEntry {
                matrix_id: "dirty".to_string(),
                trace_flow: "core_bootstrap".to_string(),
                path: "dirty.json".to_string(),
                local_ok: true,
                crystal_ok: Some(true),
                diff_clean: Some(false),
            },
            MatrixEntry {
                matrix_id: "local-failed".to_string(),
                trace_flow: "core_bootstrap".to_string(),
                path: "local-failed.json".to_string(),
                local_ok: false,
                crystal_ok: None,
                diff_clean: None,
            },
        ];
        let skipped = vec![MatrixSkip {
            matrix_id: "ui-only".to_string(),
            reason: "matrix entry does not declare traceFlow".to_string(),
        }];

        let summary = matrix_summary(&artifacts, &skipped);

        assert_eq!(summary.artifact_count, 3);
        assert_eq!(summary.skipped_count, 1);
        assert_eq!(summary.local_ok_count, 2);
        assert_eq!(summary.local_failed_count, 1);
        assert_eq!(summary.crystal_ok_count, 2);
        assert_eq!(summary.crystal_missing_count, 1);
        assert_eq!(summary.diff_clean_count, 1);
        assert_eq!(summary.diff_dirty_count, 1);
        assert_eq!(summary.diff_missing_count, 1);
        assert_eq!(summary.accepted_live_comparison_count, 1);
    }
}
