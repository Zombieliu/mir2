//! Loopback-only transparent relay for evidence-bound Crystal screenshots.
//!
//! The relay deliberately treats the client-to-server stream as opaque bytes:
//! login identifiers, passwords, chat text, and other client payloads are never
//! decoded or persisted.  Only selected server-to-client world-state packets
//! are decoded, while the exact frame bytes are forwarded unchanged.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mir2_protocol::{decode_frame, decode_server_packet, server_packet_name, ServerPacket};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{interval, MissedTickBehavior};

const EVIDENCE_SCHEMA: &str = "mir2-crystal-original-state-evidence-v1";
const EVIDENCE_PRODUCER: &str = "crystal-original-state-relay";
const DEFAULT_BIND: &str = "127.0.0.1:7010";
const DEFAULT_HEARTBEAT_MS: u64 = 500;
const MIN_HEARTBEAT_MS: u64 = 100;
const MAX_HEARTBEAT_MS: u64 = 5_000;
const MAX_FRAME_BYTES: usize = u16::MAX as usize;
const RUN_ID_MAX_LEN: usize = 96;

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
struct RelayConfig {
    bind: SocketAddr,
    upstream: SocketAddr,
    run_id: String,
    state_out: PathBuf,
    heartbeat: Duration,
    relay_executable_sha256: String,
}

impl RelayConfig {
    fn from_env() -> Result<Self, String> {
        let bind = env::var("MIR2_ORIGINAL_CAPTURE_RELAY_BIND")
            .unwrap_or_else(|_| DEFAULT_BIND.to_owned())
            .parse::<SocketAddr>()
            .map_err(|error| format!("invalid MIR2_ORIGINAL_CAPTURE_RELAY_BIND: {error}"))?;
        let upstream = required_env("MIR2_ORIGINAL_CAPTURE_RELAY_UPSTREAM")?
            .parse::<SocketAddr>()
            .map_err(|error| format!("invalid MIR2_ORIGINAL_CAPTURE_RELAY_UPSTREAM: {error}"))?;
        if !bind.ip().is_loopback() {
            return Err("capture relay bind must be loopback".to_owned());
        }
        if !upstream.ip().is_loopback() {
            return Err("capture relay upstream must be loopback".to_owned());
        }
        if bind == upstream {
            return Err("capture relay bind and upstream must differ".to_owned());
        }

        let run_id = required_env("MIR2_ORIGINAL_CAPTURE_RUN_ID")?;
        validate_run_id(&run_id)?;
        let state_out = PathBuf::from(required_env("MIR2_ORIGINAL_CAPTURE_STATE_OUT")?);
        if state_out.as_os_str().is_empty() {
            return Err("MIR2_ORIGINAL_CAPTURE_STATE_OUT must not be empty".to_owned());
        }
        let heartbeat_ms = env::var("MIR2_ORIGINAL_CAPTURE_HEARTBEAT_MS")
            .ok()
            .map(|value| {
                value
                    .parse::<u64>()
                    .map_err(|error| format!("invalid MIR2_ORIGINAL_CAPTURE_HEARTBEAT_MS: {error}"))
            })
            .transpose()?
            .unwrap_or(DEFAULT_HEARTBEAT_MS)
            .clamp(MIN_HEARTBEAT_MS, MAX_HEARTBEAT_MS);
        let relay_executable_sha256 = hash_file(
            &env::current_exe().map_err(|error| format!("resolve relay executable: {error}"))?,
        )
        .map_err(|error| format!("hash relay executable: {error}"))?;

        Ok(Self {
            bind,
            upstream,
            run_id,
            state_out,
            heartbeat: Duration::from_millis(heartbeat_ms),
            relay_executable_sha256,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PacketObservation {
    connection_id: u64,
    sequence: u64,
    observed_at_unix_ms: u64,
    packet: String,
    packet_id: i16,
    frame_sha256: String,
}

impl PacketObservation {
    fn from_frame(connection_id: u64, sequence: u64, packet: &ServerPacket, frame: &[u8]) -> Self {
        Self {
            connection_id,
            sequence,
            observed_at_unix_ms: now_unix_ms(),
            packet: server_packet_name(packet).to_owned(),
            packet_id: decode_frame(frame)
                .map(|decoded| decoded.packet_id)
                .unwrap_or_default(),
            frame_sha256: hash_bytes(frame),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RelayDescriptor {
    bind: String,
    upstream: String,
    connection_id: u64,
    connection_active: bool,
    connected_at_unix_ms: u64,
    last_server_sequence: u64,
    decode_error_count: u64,
    relay_executable_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorldEvidence {
    map: String,
    map_index: i32,
    x: i32,
    y: i32,
    direction: u8,
    light: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PacketEvidenceSet {
    start_game: Option<PacketObservation>,
    map: Option<PacketObservation>,
    position: Option<PacketObservation>,
    light: Option<PacketObservation>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AcceptanceState {
    eligible: bool,
    blockers: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RelayEvidence {
    schema_version: &'static str,
    producer: &'static str,
    run_id: String,
    generated_at_unix_ms: u64,
    relay: RelayDescriptor,
    world: Option<WorldEvidence>,
    packets: PacketEvidenceSet,
    acceptance: AcceptanceState,
}

#[derive(Debug, Clone)]
struct RelayState {
    connection_id: u64,
    connection_active: bool,
    connected_at_unix_ms: u64,
    last_server_sequence: u64,
    decode_error_count: u64,
    in_game: bool,
    start_game: Option<PacketObservation>,
    map_file_name: Option<String>,
    map_index: Option<i32>,
    map_light_setting: Option<u8>,
    map_dark_light: Option<u8>,
    map_observation: Option<PacketObservation>,
    position: Option<(i32, i32)>,
    direction: Option<u8>,
    position_observation: Option<PacketObservation>,
    time_of_day_light_setting: Option<u8>,
    time_of_day_observation: Option<PacketObservation>,
}

impl RelayState {
    fn new(connection_id: u64) -> Self {
        Self {
            connection_id,
            connection_active: true,
            connected_at_unix_ms: now_unix_ms(),
            last_server_sequence: 0,
            decode_error_count: 0,
            in_game: false,
            start_game: None,
            map_file_name: None,
            map_index: None,
            map_light_setting: None,
            map_dark_light: None,
            map_observation: None,
            position: None,
            direction: None,
            position_observation: None,
            time_of_day_light_setting: None,
            time_of_day_observation: None,
        }
    }

    fn observe_server_frame(&mut self, frame: &[u8]) {
        self.last_server_sequence = self.last_server_sequence.saturating_add(1);
        let Ok(packet) = decode_server_packet(frame) else {
            self.decode_error_count = self.decode_error_count.saturating_add(1);
            return;
        };
        let observation = PacketObservation::from_frame(
            self.connection_id,
            self.last_server_sequence,
            &packet,
            frame,
        );

        match packet {
            ServerPacket::StartGame { result: 4, .. } => {
                self.clear_world(false);
                self.in_game = true;
                self.start_game = Some(observation);
            }
            ServerPacket::StartGame { .. } => {
                self.clear_world(true);
            }
            ServerPacket::MapInformation { info } => {
                self.observe_map(
                    info.map_index,
                    info.file_name,
                    info.lights,
                    info.map_dark_light,
                    observation,
                );
            }
            ServerPacket::MapChanged {
                map_index,
                file_name,
                lights,
                location,
                direction,
                map_dark_light,
                ..
            } => {
                self.clear_map_specific();
                self.map_index = Some(map_index);
                self.map_file_name = non_empty_map_name(file_name);
                self.map_light_setting = valid_light_setting(lights);
                self.map_dark_light = valid_map_dark_light(map_dark_light);
                self.map_observation = Some(observation.clone());
                self.position = Some((location.x, location.y));
                self.direction = Some(direction as u8);
                self.position_observation = Some(observation);
            }
            ServerPacket::UserInformation { info } => {
                self.position = Some((info.location.x, info.location.y));
                self.direction = Some(info.direction as u8);
                self.position_observation = Some(observation);
            }
            ServerPacket::UserLocation { location } => {
                self.position = Some((location.position.x, location.position.y));
                self.direction = Some(location.direction as u8);
                self.position_observation = Some(observation);
            }
            ServerPacket::TimeOfDay { lights } => {
                self.time_of_day_light_setting = valid_light_setting(lights);
                self.time_of_day_observation = self
                    .time_of_day_light_setting
                    .is_some()
                    .then_some(observation);
            }
            ServerPacket::LogOutSuccess { .. } => self.clear_world(true),
            _ => {}
        }
    }

    fn observe_map(
        &mut self,
        map_index: i32,
        file_name: String,
        lights: u8,
        map_dark_light: u8,
        observation: PacketObservation,
    ) {
        let next_name = non_empty_map_name(file_name);
        let changed = self
            .map_file_name
            .as_deref()
            .zip(next_name.as_deref())
            .is_some_and(|(current, next)| normalize_map_name(current) != normalize_map_name(next));
        if changed {
            self.clear_map_specific();
        }
        self.map_index = Some(map_index);
        self.map_file_name = next_name;
        self.map_light_setting = valid_light_setting(lights);
        self.map_dark_light = valid_map_dark_light(map_dark_light);
        self.map_observation = Some(observation);
    }

    fn clear_map_specific(&mut self) {
        self.map_file_name = None;
        self.map_index = None;
        self.map_light_setting = None;
        self.map_dark_light = None;
        self.map_observation = None;
        self.position = None;
        self.direction = None;
        self.position_observation = None;
    }

    fn clear_world(&mut self, clear_connection_light: bool) {
        self.in_game = false;
        self.start_game = None;
        self.clear_map_specific();
        if clear_connection_light {
            self.time_of_day_light_setting = None;
            self.time_of_day_observation = None;
        }
    }

    fn snapshot(&self, config: &RelayConfig) -> RelayEvidence {
        let map = self.map_file_name.as_ref();
        let position = self.position;
        let direction = self.direction;
        let effective_light = self.map_light_setting.or(self.time_of_day_light_setting);
        let light_observation = if self.map_light_setting.is_some() {
            self.map_observation.clone()
        } else {
            self.time_of_day_observation.clone()
        };

        let mut blockers = Vec::new();
        if !self.connection_active {
            blockers.push("connection-inactive");
        }
        if !self.in_game || self.start_game.is_none() {
            blockers.push("successful-start-game-not-observed");
        }
        if map.is_none() || self.map_index.is_none() || self.map_observation.is_none() {
            blockers.push("authoritative-map-not-observed");
        }
        if position.is_none() || direction.is_none() || self.position_observation.is_none() {
            blockers.push("authoritative-position-not-observed");
        }
        if effective_light.is_none() || light_observation.is_none() || self.map_dark_light.is_none()
        {
            blockers.push("authoritative-light-not-observed");
        }
        let eligible = blockers.is_empty();
        let world = map
            .zip(self.map_index)
            .zip(position)
            .zip(direction)
            .zip(effective_light)
            .zip(self.map_dark_light)
            .map(
                |(((((map, map_index), (x, y)), direction), setting), map_dark_light)| {
                    WorldEvidence {
                        map: map.clone(),
                        map_index,
                        x,
                        y,
                        direction,
                        light: format!("setting={setting};mapDarkLight={map_dark_light}"),
                    }
                },
            );

        RelayEvidence {
            schema_version: EVIDENCE_SCHEMA,
            producer: EVIDENCE_PRODUCER,
            run_id: config.run_id.clone(),
            generated_at_unix_ms: now_unix_ms(),
            relay: RelayDescriptor {
                bind: config.bind.to_string(),
                upstream: config.upstream.to_string(),
                connection_id: self.connection_id,
                connection_active: self.connection_active,
                connected_at_unix_ms: self.connected_at_unix_ms,
                last_server_sequence: self.last_server_sequence,
                decode_error_count: self.decode_error_count,
                relay_executable_sha256: config.relay_executable_sha256.clone(),
            },
            world,
            packets: PacketEvidenceSet {
                start_game: self.start_game.clone(),
                map: self.map_observation.clone(),
                position: self.position_observation.clone(),
                light: light_observation,
            },
            acceptance: AcceptanceState { eligible, blockers },
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Arc::new(RelayConfig::from_env().map_err(io::Error::other)?);
    let listener = TcpListener::bind(config.bind).await?;
    println!(
        "CRYSTAL_ORIGINAL_CAPTURE_RELAY_READY bind={} upstream={} state={} runId={}",
        config.bind,
        config.upstream,
        config.state_out.display(),
        config.run_id
    );

    let mut connection_id = 0_u64;
    loop {
        let (client, peer) = listener.accept().await?;
        if !peer.ip().is_loopback() {
            eprintln!("rejected non-loopback capture client {peer}");
            continue;
        }
        connection_id = connection_id.saturating_add(1);
        let upstream = match TcpStream::connect(config.upstream).await {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("capture relay upstream connect failed: {error}");
                continue;
            }
        };
        let state = Arc::new(Mutex::new(RelayState::new(connection_id)));
        write_current_evidence(&config, &state)?;
        eprintln!("capture relay connection {connection_id} accepted from {peer}");
        let result =
            relay_connection(client, upstream, Arc::clone(&state), Arc::clone(&config)).await;
        {
            let mut current = lock_state(&state);
            current.connection_active = false;
        }
        write_current_evidence(&config, &state)?;
        if let Err(error) = result {
            eprintln!("capture relay connection {connection_id} ended: {error}");
        }
    }
}

async fn relay_connection(
    client: TcpStream,
    upstream: TcpStream,
    state: Arc<Mutex<RelayState>>,
    config: Arc<RelayConfig>,
) -> io::Result<()> {
    let (mut client_reader, mut client_writer) = client.into_split();
    let (mut upstream_reader, mut upstream_writer) = upstream.into_split();

    let client_to_server = async {
        tokio::io::copy(&mut client_reader, &mut upstream_writer).await?;
        upstream_writer.shutdown().await
    };
    let server_to_client = async {
        loop {
            let frame = read_frame(&mut upstream_reader).await?;
            client_writer.write_all(&frame).await?;
            client_writer.flush().await?;
            lock_state(&state).observe_server_frame(&frame);
        }
        #[allow(unreachable_code)]
        Ok::<(), io::Error>(())
    };
    let heartbeat = evidence_heartbeat(Arc::clone(&state), Arc::clone(&config));

    tokio::select! {
        result = client_to_server => result,
        result = server_to_client => result,
        result = heartbeat => result,
    }
}

async fn evidence_heartbeat(
    state: Arc<Mutex<RelayState>>,
    config: Arc<RelayConfig>,
) -> io::Result<()> {
    let mut ticker = interval(config.heartbeat);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        write_current_evidence(&config, &state)?;
    }
}

async fn read_frame(reader: &mut (impl AsyncRead + Unpin)) -> io::Result<Vec<u8>> {
    let mut header = [0_u8; 2];
    reader.read_exact(&mut header).await?;
    let length = u16::from_le_bytes(header) as usize;
    if !(4..=MAX_FRAME_BYTES).contains(&length) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid Crystal frame length {length}"),
        ));
    }
    let mut frame = vec![0_u8; length];
    frame[..2].copy_from_slice(&header);
    reader.read_exact(&mut frame[2..]).await?;
    Ok(frame)
}

fn write_current_evidence(config: &RelayConfig, state: &Arc<Mutex<RelayState>>) -> io::Result<()> {
    let evidence = lock_state(state).snapshot(config);
    write_json_atomic(&config.state_out, &evidence)
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        fs::create_dir_all(parent)?;
    }
    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("original-state.json");
    let temporary = path.with_file_name(format!(
        ".{file_name}.tmp-{}-{sequence}",
        std::process::id()
    ));
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn lock_state(state: &Arc<Mutex<RelayState>>) -> std::sync::MutexGuard<'_, RelayState> {
    state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn valid_light_setting(value: u8) -> Option<u8> {
    (1..=4).contains(&value).then_some(value)
}

fn valid_map_dark_light(value: u8) -> Option<u8> {
    (0..=4).contains(&value).then_some(value)
}

fn non_empty_map_name(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn normalize_map_name(value: &str) -> String {
    let normalized = value.trim().replace('\\', "/");
    let file_name = normalized.rsplit('/').next().unwrap_or_default();
    let lower = file_name.to_ascii_lowercase();
    lower.strip_suffix(".map").unwrap_or(&lower).to_owned()
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{name} is required"))
}

fn validate_run_id(value: &str) -> Result<(), String> {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return Err("MIR2_ORIGINAL_CAPTURE_RUN_ID must not be empty".to_owned());
    };
    if value.len() > RUN_ID_MAX_LEN
        || !first.is_ascii_alphanumeric()
        || !chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        return Err(
            "MIR2_ORIGINAL_CAPTURE_RUN_ID must match [A-Za-z0-9][A-Za-z0-9._-]{0,95}".to_owned(),
        );
    }
    Ok(())
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn hash_file(path: &Path) -> io::Result<String> {
    fs::read(path).map(|bytes| hash_bytes(&bytes))
}

fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_protocol::{encode_server_packet, MapInformation, MirDirection, Point, UserLocation};
    use tokio::time::{sleep, timeout};

    fn test_config(path: PathBuf) -> RelayConfig {
        RelayConfig {
            bind: "127.0.0.1:7010".parse().unwrap(),
            upstream: "127.0.0.1:7000".parse().unwrap(),
            run_id: "pair-test-001".to_owned(),
            state_out: path,
            heartbeat: Duration::from_millis(100),
            relay_executable_sha256: "a".repeat(64),
        }
    }

    fn observe(state: &mut RelayState, packet: ServerPacket) {
        state.observe_server_frame(&encode_server_packet(&packet).unwrap());
    }

    #[test]
    fn complete_same_connection_world_state_becomes_eligible() {
        let mut state = RelayState::new(7);
        observe(&mut state, ServerPacket::TimeOfDay { lights: 3 });
        observe(
            &mut state,
            ServerPacket::StartGame {
                result: 4,
                resolution: 1024,
            },
        );
        observe(
            &mut state,
            ServerPacket::MapInformation {
                info: MapInformation {
                    map_index: 0,
                    file_name: "Maps/0.map".to_owned(),
                    title: "Bichon Province".to_owned(),
                    mini_map: 1,
                    big_map: 1,
                    lights: 0,
                    flags: 0,
                    map_dark_light: 2,
                    music: 1,
                    weather_particles: 0,
                },
            },
        );
        observe(
            &mut state,
            ServerPacket::UserLocation {
                location: UserLocation {
                    position: Point { x: 287, y: 618 },
                    direction: MirDirection::Up,
                },
            },
        );

        let evidence = state.snapshot(&test_config(PathBuf::from("unused.json")));
        assert!(evidence.acceptance.eligible);
        assert!(evidence.acceptance.blockers.is_empty());
        let world = evidence.world.unwrap();
        assert_eq!(world.map, "Maps/0.map");
        assert_eq!((world.x, world.y), (287, 618));
        assert_eq!(world.light, "setting=3;mapDarkLight=2");
        assert_eq!(evidence.packets.start_game.unwrap().connection_id, 7);
        assert_eq!(evidence.packets.map.unwrap().connection_id, 7);
        assert_eq!(evidence.packets.position.unwrap().connection_id, 7);
        assert_eq!(evidence.packets.light.unwrap().connection_id, 7);
    }

    #[test]
    fn map_changed_replaces_map_and_position_as_one_authoritative_frame() {
        let mut state = RelayState::new(3);
        observe(
            &mut state,
            ServerPacket::StartGame {
                result: 4,
                resolution: 1024,
            },
        );
        observe(
            &mut state,
            ServerPacket::MapChanged {
                map_index: 1,
                file_name: "BichonProvince".to_owned(),
                title: "Bichon".to_owned(),
                mini_map: 1,
                big_map: 2,
                lights: 4,
                location: Point { x: 300, y: 400 },
                direction: MirDirection::Down,
                map_dark_light: 1,
                music: 5,
                weather: 0,
            },
        );

        let evidence = state.snapshot(&test_config(PathBuf::from("unused.json")));
        assert!(evidence.acceptance.eligible);
        let map = evidence.packets.map.unwrap();
        let position = evidence.packets.position.unwrap();
        let light = evidence.packets.light.unwrap();
        assert_eq!(map.sequence, position.sequence);
        assert_eq!(map.frame_sha256, position.frame_sha256);
        assert_eq!(map.frame_sha256, light.frame_sha256);
        assert_eq!(evidence.world.unwrap().light, "setting=4;mapDarkLight=1");
    }

    #[test]
    fn logout_and_disconnect_fail_closed() {
        let mut state = RelayState::new(9);
        observe(
            &mut state,
            ServerPacket::StartGame {
                result: 4,
                resolution: 1024,
            },
        );
        observe(
            &mut state,
            ServerPacket::MapChanged {
                map_index: 0,
                file_name: "0".to_owned(),
                title: "Bichon".to_owned(),
                mini_map: 1,
                big_map: 1,
                lights: 3,
                location: Point { x: 1, y: 2 },
                direction: MirDirection::Up,
                map_dark_light: 0,
                music: 1,
                weather: 0,
            },
        );
        observe(
            &mut state,
            ServerPacket::LogOutSuccess { characters: vec![] },
        );
        let config = test_config(PathBuf::from("unused.json"));
        let logged_out = state.snapshot(&config);
        assert!(!logged_out.acceptance.eligible);
        assert!(logged_out.world.is_none());
        assert!(logged_out
            .acceptance
            .blockers
            .contains(&"successful-start-game-not-observed"));

        state.connection_active = false;
        let disconnected = state.snapshot(&config);
        assert!(disconnected
            .acceptance
            .blockers
            .contains(&"connection-inactive"));
    }

    #[test]
    fn atomic_evidence_contains_no_client_credentials() {
        let root = env::temp_dir().join(format!(
            "mir2-original-relay-test-{}-{}",
            std::process::id(),
            TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let path = root.join("state.json");
        let config = test_config(path.clone());
        let state = Arc::new(Mutex::new(RelayState::new(1)));
        write_current_evidence(&config, &state).unwrap();
        {
            let mut current = lock_state(&state);
            current.connection_active = false;
        }
        // The real relay overwrites the heartbeat file for the lifetime of a
        // connection.  Exercise replacement explicitly because Windows file
        // replacement semantics differ from Unix rename semantics.
        write_current_evidence(&config, &state).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains(EVIDENCE_SCHEMA));
        assert!(text.contains("\"connectionActive\": false"));
        assert!(!text.contains("password"));
        assert!(!text.contains("accountId"));
        assert!(!text.contains("chat"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn run_id_validation_matches_pair_gate_contract() {
        assert!(validate_run_id("pair-20260824_01.test").is_ok());
        assert!(validate_run_id("").is_err());
        assert!(validate_run_id("-bad").is_err());
        assert!(validate_run_id("bad space").is_err());
        assert!(validate_run_id(&"a".repeat(97)).is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn relay_forwards_exact_bytes_and_only_records_server_world_state() {
        async fn tcp_pair() -> (TcpStream, TcpStream) {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let connect = TcpStream::connect(address);
            let accept = listener.accept();
            let (connected, accepted) = tokio::join!(connect, accept);
            (connected.unwrap(), accepted.unwrap().0)
        }

        let root = env::temp_dir().join(format!(
            "mir2-original-relay-network-test-{}-{}",
            std::process::id(),
            TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let config = Arc::new(test_config(root.join("state.json")));
        let state = Arc::new(Mutex::new(RelayState::new(11)));
        write_current_evidence(&config, &state).unwrap();
        let (mut original_client, relay_client) = tcp_pair().await;
        let (relay_upstream, mut original_server) = tcp_pair().await;
        let relay = tokio::spawn(relay_connection(
            relay_client,
            relay_upstream,
            Arc::clone(&state),
            Arc::clone(&config),
        ));

        let opaque_client_bytes = b"opaque-client-credential-bytes-never-decoded";
        original_client
            .write_all(opaque_client_bytes)
            .await
            .unwrap();
        original_client.flush().await.unwrap();
        let mut forwarded_client_bytes = vec![0; opaque_client_bytes.len()];
        timeout(
            Duration::from_secs(1),
            original_server.read_exact(&mut forwarded_client_bytes),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(forwarded_client_bytes, opaque_client_bytes);

        let packets = [
            ServerPacket::TimeOfDay { lights: 3 },
            ServerPacket::StartGame {
                result: 4,
                resolution: 1024,
            },
            ServerPacket::MapChanged {
                map_index: 0,
                file_name: "0".to_owned(),
                title: "Bichon".to_owned(),
                mini_map: 1,
                big_map: 1,
                lights: 0,
                location: Point { x: 287, y: 618 },
                direction: MirDirection::Up,
                map_dark_light: 0,
                music: 1,
                weather: 0,
            },
        ];
        let server_bytes = packets
            .iter()
            .flat_map(|packet| encode_server_packet(packet).unwrap())
            .collect::<Vec<_>>();
        original_server.write_all(&server_bytes).await.unwrap();
        original_server.flush().await.unwrap();
        let mut forwarded_server_bytes = vec![0; server_bytes.len()];
        timeout(
            Duration::from_secs(1),
            original_client.read_exact(&mut forwarded_server_bytes),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(forwarded_server_bytes, server_bytes);

        timeout(Duration::from_secs(1), async {
            loop {
                if lock_state(&state).snapshot(&config).acceptance.eligible {
                    break;
                }
                sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        let json = serde_json::to_string(&lock_state(&state).snapshot(&config)).unwrap();
        assert!(!json.contains("opaque-client-credential"));
        assert!(json.contains("\"map\":\"0\""));

        original_server.shutdown().await.unwrap();
        let relay_result = timeout(Duration::from_secs(1), relay)
            .await
            .unwrap()
            .unwrap();
        assert!(relay_result
            .as_ref()
            .is_err_and(|error| error.kind() == io::ErrorKind::UnexpectedEof));
        fs::remove_dir_all(root).unwrap();
    }
}
