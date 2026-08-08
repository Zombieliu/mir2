use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mir2_protocol::{
    decode_frame, decode_server_packet, encode_client_packet, server_packet_display_name,
    ClientPacket, MirDirection,
};
use serde::Serialize;

const DEFAULT_ADDR: &str = "127.0.0.1:7000";
const DEFAULT_OUTPUT_PATH: &str = "docs/generated/load/latest-tcp.json";
const READ_TIMEOUT_MS: u64 = 250;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TcpLoadReport {
    r#type: &'static str,
    addr: String,
    run_id: String,
    generated_at_unix_ms: u128,
    clients: usize,
    actions_per_client: usize,
    duration_ms: u128,
    ready: usize,
    failed: usize,
    commands_sent: usize,
    packets_received: usize,
    decode_errors: usize,
    client_results: Vec<ClientResult>,
    rss_samples: Vec<RssSample>,
    rss: NumericSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientResult {
    index: usize,
    ready: bool,
    commands_sent: usize,
    packets_received: usize,
    decode_errors: usize,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RssSample {
    at_unix_ms: u128,
    pid: u32,
    working_set_bytes: u64,
    handle_count: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NumericSummary {
    count: usize,
    min: Option<u64>,
    max: Option<u64>,
    avg: Option<u64>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = env::var("MIR2_GATEWAY_TCP_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.into());
    let clients = usize_env("MIR2_TCP_LOAD_CLIENTS", 64);
    let actions = usize_env("MIR2_TCP_LOAD_ACTIONS", 20);
    let output_path =
        PathBuf::from(env::var("MIR2_TCP_LOAD_OUT").unwrap_or_else(|_| DEFAULT_OUTPUT_PATH.into()));
    let run_id = format!("{}-{}", now_unix_ms(), std::process::id());

    let keep_sampling = Arc::new(AtomicBool::new(true));
    let rss_samples = Arc::new(Mutex::new(Vec::new()));
    let sampler = spawn_rss_sampler(Arc::clone(&keep_sampling), Arc::clone(&rss_samples));

    let started = Instant::now();
    let mut handles = Vec::with_capacity(clients);
    for index in 0..clients {
        let addr = addr.clone();
        let run_id = run_id.clone();
        handles.push(thread::spawn(move || {
            run_client(index, &run_id, &addr, actions)
        }));
    }

    let mut client_results = Vec::with_capacity(clients);
    for handle in handles {
        match handle.join() {
            Ok(result) => client_results.push(result),
            Err(_) => client_results.push(ClientResult {
                index: client_results.len(),
                ready: false,
                commands_sent: 0,
                packets_received: 0,
                decode_errors: 0,
                error: Some("client thread panicked".to_string()),
            }),
        }
    }

    keep_sampling.store(false, Ordering::Relaxed);
    let _ = sampler.join();
    let rss_samples = rss_samples
        .lock()
        .map(|samples| samples.clone())
        .unwrap_or_default();
    let report = TcpLoadReport {
        r#type: "tcp",
        addr,
        run_id,
        generated_at_unix_ms: now_unix_ms(),
        clients,
        actions_per_client: actions,
        duration_ms: started.elapsed().as_millis(),
        ready: client_results.iter().filter(|result| result.ready).count(),
        failed: client_results.iter().filter(|result| !result.ready).count(),
        commands_sent: client_results
            .iter()
            .map(|result| result.commands_sent)
            .sum(),
        packets_received: client_results
            .iter()
            .map(|result| result.packets_received)
            .sum(),
        decode_errors: client_results
            .iter()
            .map(|result| result.decode_errors)
            .sum(),
        client_results,
        rss: summarize_rss(&rss_samples),
        rss_samples,
    };

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &output_path,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;

    println!(
        "TCP load completed: ready={}/{} failed={} packets={} decode_errors={}",
        report.ready, report.clients, report.failed, report.packets_received, report.decode_errors
    );
    println!("Wrote {}", output_path.display());

    if report.failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}

fn run_client(index: usize, run_id: &str, addr: &str, actions: usize) -> ClientResult {
    let mut result = ClientResult {
        index,
        ready: false,
        commands_sent: 0,
        packets_received: 0,
        decode_errors: 0,
        error: None,
    };

    let account_id = format!("load-tcp-{run_id}-{index}");
    let mut stream = match connect(addr) {
        Ok(stream) => stream,
        Err(error) => return result.with_error(error),
    };
    if let Err(error) = stream.set_read_timeout(Some(Duration::from_millis(READ_TIMEOUT_MS))) {
        return result.with_error(error);
    }

    read_available(&mut stream, &mut result);

    for packet in [
        ClientPacket::ClientVersion {
            version_hash: Vec::new(),
        },
        ClientPacket::NewAccount {
            account_id: account_id.clone(),
            password: "load-pass".to_string(),
            birth_date_binary: 0,
            user_name: account_id.clone(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        },
        ClientPacket::Login {
            account_id,
            password: "load-pass".to_string(),
        },
        ClientPacket::StartGame { character_index: 0 },
    ] {
        if let Err(error) = send_packet(&mut stream, &packet) {
            return result.with_error(error);
        }
        result.commands_sent += 1;
        read_available(&mut stream, &mut result);
    }
    result.ready = true;

    let directions = [
        MirDirection::Right,
        MirDirection::Down,
        MirDirection::Left,
        MirDirection::Up,
    ];
    for action in 0..actions {
        let keep_alive = ClientPacket::KeepAlive {
            time: (now_unix_ms() as i64)
                .saturating_mul(1000)
                .saturating_add(index as i64)
                .saturating_add(action as i64),
        };
        if let Err(error) = send_packet(&mut stream, &keep_alive) {
            return result.with_error(error);
        }
        result.commands_sent += 1;

        let walk = ClientPacket::Walk {
            direction: directions[action % directions.len()],
        };
        if let Err(error) = send_packet(&mut stream, &walk) {
            return result.with_error(error);
        }
        result.commands_sent += 1;

        if action % 10 == 0 {
            let chat = ClientPacket::Chat {
                message: format!("tcp load {index}:{action}"),
                linked_items: Vec::new(),
            };
            if let Err(error) = send_packet(&mut stream, &chat) {
                return result.with_error(error);
            }
            result.commands_sent += 1;
        }

        read_available(&mut stream, &mut result);
        thread::sleep(Duration::from_millis(10));
    }

    result
}

impl ClientResult {
    fn with_error(mut self, error: impl std::fmt::Display) -> Self {
        self.error = Some(error.to_string());
        self
    }
}

fn connect(addr: &str) -> io::Result<TcpStream> {
    let mut addrs = addr.to_socket_addrs()?;
    let Some(addr) = addrs.next() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "address resolved to no socket addresses",
        ));
    };
    TcpStream::connect_timeout(&addr, Duration::from_secs(2))
}

fn send_packet(stream: &mut TcpStream, packet: &ClientPacket) -> Result<(), String> {
    let bytes = encode_client_packet(packet).map_err(|error| error.to_string())?;
    stream
        .write_all(&bytes)
        .map_err(|error| error.to_string())?;
    stream.flush().map_err(|error| error.to_string())
}

fn read_available(stream: &mut TcpStream, result: &mut ClientResult) {
    loop {
        match read_frame(stream) {
            Ok(bytes) => {
                result.packets_received += 1;
                if let Ok(frame) = decode_frame(&bytes) {
                    if decode_server_packet(&bytes)
                        .map(|packet| server_packet_display_name(&packet))
                        .is_err()
                    {
                        eprintln!("tcp load decode error for packet {}", frame.packet_id);
                        result.decode_errors += 1;
                    }
                } else {
                    result.decode_errors += 1;
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::UnexpectedEof
                ) =>
            {
                break;
            }
            Err(_) => {
                result.decode_errors += 1;
                break;
            }
        }
    }
}

fn read_frame(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut header = [0_u8; 2];
    stream.read_exact(&mut header)?;
    let len = u16::from_le_bytes(header) as usize;
    if len < 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid frame length {len}"),
        ));
    }
    let mut frame = vec![0_u8; len];
    frame[..2].copy_from_slice(&header);
    stream.read_exact(&mut frame[2..])?;
    Ok(frame)
}

fn spawn_rss_sampler(
    keep_sampling: Arc<AtomicBool>,
    samples: Arc<Mutex<Vec<RssSample>>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        while keep_sampling.load(Ordering::Relaxed) {
            if let Some(sample) = gateway_rss_sample() {
                if let Ok(mut samples) = samples.lock() {
                    samples.push(sample);
                }
            }
            thread::sleep(Duration::from_millis(500));
        }
        if let Some(sample) = gateway_rss_sample() {
            if let Ok(mut samples) = samples.lock() {
                samples.push(sample);
            }
        }
    })
}

fn gateway_rss_sample() -> Option<RssSample> {
    if !cfg!(windows) {
        return None;
    }
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "$p = Get-Process -Name mir2-gateway -ErrorAction SilentlyContinue | Sort-Object StartTime -Descending | Select-Object -First 1; if ($p) { [Console]::WriteLine(($p.Id.ToString() + ',' + $p.WorkingSet64.ToString() + ',' + $p.HandleCount.ToString())) }",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut parts = stdout.trim().split(',');
    let pid = parts.next()?.parse::<u32>().ok()?;
    let working_set_bytes = parts.next()?.parse::<u64>().ok()?;
    let handle_count = parts.next()?.parse::<u32>().ok()?;
    Some(RssSample {
        at_unix_ms: now_unix_ms(),
        pid,
        working_set_bytes,
        handle_count,
    })
}

fn summarize_rss(samples: &[RssSample]) -> NumericSummary {
    let mut values = samples
        .iter()
        .map(|sample| sample.working_set_bytes)
        .collect::<Vec<_>>();
    values.sort_unstable();
    if values.is_empty() {
        return NumericSummary {
            count: 0,
            min: None,
            max: None,
            avg: None,
        };
    }
    let sum = values.iter().copied().sum::<u64>();
    NumericSummary {
        count: values.len(),
        min: values.first().copied(),
        max: values.last().copied(),
        avg: Some(sum / values.len() as u64),
    }
}

fn usize_env(name: &str, fallback: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}
