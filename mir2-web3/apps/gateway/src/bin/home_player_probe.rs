use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mir2_protocol::{decode_server_packet, encode_client_packet, ClientPacket, ServerPacket};
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const DEFAULT_GATEWAY_ADDR: &str = "127.0.0.1:7000";
const DEFAULT_OUTPUT: &str = "docs/generated/home-node/live-player-probe.json";
// A debug Zone Host can spend more than ten seconds building the first Mir2
// world/session. This is a correctness probe, not a latency SLO check; the JSON
// still records each delay so performance regressions remain visible.
const DEFAULT_TIMEOUT_MS: u64 = 30_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeStep {
    name: &'static str,
    latency_ms: u128,
    observed_packets: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeReport {
    schema_version: u32,
    generated_at_unix_ms: u64,
    gateway_addr: String,
    account_id: String,
    character_index: i32,
    keep_alive_value: i64,
    timeout_ms: u64,
    steps: Vec<ProbeStep>,
    total_latency_ms: u128,
    success: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gateway_addr =
        env::var("MIR2_HOME_PLAYER_GATEWAY_ADDR").unwrap_or_else(|_| DEFAULT_GATEWAY_ADDR.into());
    let output = PathBuf::from(
        env::var("MIR2_HOME_PLAYER_PROBE_OUT").unwrap_or_else(|_| DEFAULT_OUTPUT.into()),
    );
    let account_id = env::var("MIR2_HOME_PLAYER_ACCOUNT").unwrap_or_else(|_| "demo".into());
    let password = env::var("MIR2_HOME_PLAYER_PASSWORD").unwrap_or_else(|_| "demo".into());
    let character_index = env::var("MIR2_HOME_PLAYER_CHARACTER_INDEX")
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(0);
    let keep_alive_value = env::var("MIR2_HOME_PLAYER_KEEP_ALIVE")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(4242);
    let timeout_ms = env::var("MIR2_HOME_PLAYER_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_TIMEOUT_MS);
    let timeout = Duration::from_millis(timeout_ms);
    let total_started = Instant::now();
    let mut steps = Vec::new();

    let started = Instant::now();
    let mut player = tokio::time::timeout(timeout, TcpStream::connect(&gateway_addr))
        .await
        .map_err(|_| format!("timed out connecting to official Gateway {gateway_addr}"))??;
    let connected = read_player_until(&mut player, timeout, |packet| {
        matches!(packet, ServerPacket::Connected { .. })
    })
    .await?;
    steps.push(ProbeStep {
        name: "connect",
        latency_ms: started.elapsed().as_millis(),
        observed_packets: packet_names(&connected),
    });

    let started = Instant::now();
    send_player_packet(
        &mut player,
        &ClientPacket::Login {
            account_id: account_id.clone(),
            password,
        },
    )
    .await?;
    let login = read_player_until(&mut player, timeout, |packet| {
        matches!(packet, ServerPacket::LoginSuccess { .. })
    })
    .await?;
    steps.push(ProbeStep {
        name: "login",
        latency_ms: started.elapsed().as_millis(),
        observed_packets: packet_names(&login),
    });

    let started = Instant::now();
    send_player_packet(&mut player, &ClientPacket::StartGame { character_index }).await?;
    let game = read_player_until(&mut player, timeout, |packet| {
        matches!(packet, ServerPacket::StartGame { .. })
    })
    .await?;
    steps.push(ProbeStep {
        name: "startGame",
        latency_ms: started.elapsed().as_millis(),
        observed_packets: packet_names(&game),
    });

    let started = Instant::now();
    send_player_packet(
        &mut player,
        &ClientPacket::KeepAlive {
            time: keep_alive_value,
        },
    )
    .await?;
    let keep_alive = read_player_until(&mut player, timeout, |packet| {
        matches!(
            packet,
            ServerPacket::KeepAlive { time } if *time == keep_alive_value
        )
    })
    .await?;
    steps.push(ProbeStep {
        name: "keepAlive",
        latency_ms: started.elapsed().as_millis(),
        observed_packets: packet_names(&keep_alive),
    });

    let report = ProbeReport {
        schema_version: 1,
        generated_at_unix_ms: now_ms(),
        gateway_addr,
        account_id,
        character_index,
        keep_alive_value,
        timeout_ms,
        steps,
        total_latency_ms: total_started.elapsed().as_millis(),
        success: true,
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &output,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    println!(
        "HOME_PLAYER_PROBE_PASS output={} total_latency_ms={}",
        output.display(),
        report.total_latency_ms
    );
    Ok(())
}

async fn send_player_packet(stream: &mut TcpStream, packet: &ClientPacket) -> io::Result<()> {
    let frame = encode_client_packet(packet)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    stream.write_all(&frame).await
}

async fn read_player_packet(stream: &mut TcpStream, timeout: Duration) -> io::Result<ServerPacket> {
    let mut header = [0_u8; 2];
    tokio::time::timeout(timeout, stream.read_exact(&mut header))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "player response header timeout"))??;
    let length = u16::from_le_bytes(header) as usize;
    if length < 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid player frame length {length}"),
        ));
    }
    let mut frame = vec![0_u8; length];
    frame[..2].copy_from_slice(&header);
    tokio::time::timeout(timeout, stream.read_exact(&mut frame[2..]))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "player response body timeout"))??;
    decode_server_packet(&frame).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

async fn read_player_until(
    stream: &mut TcpStream,
    timeout: Duration,
    expected: impl Fn(&ServerPacket) -> bool,
) -> io::Result<Vec<ServerPacket>> {
    let deadline = Instant::now() + timeout;
    let mut packets = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "expected Mir2 player packet did not arrive",
            ));
        }
        let packet = read_player_packet(stream, remaining).await?;
        let done = expected(&packet);
        packets.push(packet);
        if done {
            return Ok(packets);
        }
    }
}

fn packet_names(packets: &[ServerPacket]) -> Vec<String> {
    packets
        .iter()
        .map(|packet| {
            let debug = format!("{packet:?}");
            debug
                .split_once([' ', '{', '('])
                .map(|(name, _)| name)
                .unwrap_or(debug.as_str())
                .to_string()
        })
        .collect()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
