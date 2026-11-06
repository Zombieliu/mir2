//! Production spectator transport state.
//!
//! Spectators never receive a [`GatewaySession`](crate::GatewaySession). Active player
//! sessions publish sanitized world frames into this hub; a separate read-only WebSocket
//! consumes those frames with a server-enforced delay. This keeps observation outside the
//! authoritative player command boundary and makes "read only" structural rather than a UI
//! convention.

use mir2_simulation::WorldSnapshot;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, VecDeque};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SPECTATOR_SCHEMA: &str = "obelisk.mir2.spectator.v1";
const DEFAULT_CAPTURE_INTERVAL_MS: u64 = 250;
const DEFAULT_PUBLIC_DELAY_MS: u64 = 30_000;
const DEFAULT_MAX_DELAY_MS: u64 = 120_000;
const DEFAULT_RING_FRAMES: usize = 2_400;
const DEFAULT_MAX_ENTITIES: usize = 2_048;
const DEFAULT_REPLAY_LIMIT: usize = 10_000;
const DEFAULT_RETENTION_HOURS: u64 = 168;
const DEFAULT_ENTITY_STALE_MS: u64 = 15_000;
const MAX_FRAME_BYTES: usize = 2 * 1024 * 1024;
const MAX_EVENTS_PER_FRAME: usize = 512;

#[derive(Debug, Clone)]
pub struct SpectatorConfig {
    pub enabled: bool,
    pub recording_enabled: bool,
    pub public_enabled: bool,
    pub public_maps: Vec<String>,
    pub director_token: Option<String>,
    pub capture_interval_ms: u64,
    pub public_delay_ms: u64,
    pub max_delay_ms: u64,
    pub ring_frames: usize,
    pub max_entities: usize,
    pub replay_limit: usize,
    pub retention_hours: u64,
    pub entity_stale_ms: u64,
    pub data_dir: PathBuf,
}

impl SpectatorConfig {
    pub fn from_env() -> Self {
        let public_maps = env::var("MIR2_SPECTATOR_PUBLIC_MAPS")
            .unwrap_or_else(|_| "0".to_string())
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect();
        Self {
            enabled: bool_env("MIR2_SPECTATOR_ENABLED", true),
            recording_enabled: bool_env("MIR2_SPECTATOR_RECORDING_ENABLED", true),
            public_enabled: bool_env("MIR2_SPECTATOR_PUBLIC", true),
            public_maps,
            director_token: env::var("MIR2_SPECTATOR_DIRECTOR_TOKEN")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            capture_interval_ms: bounded_u64_env(
                "MIR2_SPECTATOR_CAPTURE_INTERVAL_MS",
                DEFAULT_CAPTURE_INTERVAL_MS,
                100,
                5_000,
            ),
            public_delay_ms: bounded_u64_env(
                "MIR2_SPECTATOR_PUBLIC_DELAY_MS",
                DEFAULT_PUBLIC_DELAY_MS,
                0,
                DEFAULT_MAX_DELAY_MS,
            ),
            max_delay_ms: bounded_u64_env(
                "MIR2_SPECTATOR_MAX_DELAY_MS",
                DEFAULT_MAX_DELAY_MS,
                DEFAULT_PUBLIC_DELAY_MS,
                15 * 60_000,
            ),
            ring_frames: bounded_usize_env(
                "MIR2_SPECTATOR_RING_FRAMES",
                DEFAULT_RING_FRAMES,
                40,
                100_000,
            ),
            max_entities: bounded_usize_env(
                "MIR2_SPECTATOR_MAX_ENTITIES",
                DEFAULT_MAX_ENTITIES,
                16,
                20_000,
            ),
            replay_limit: bounded_usize_env(
                "MIR2_SPECTATOR_REPLAY_LIMIT",
                DEFAULT_REPLAY_LIMIT,
                100,
                100_000,
            ),
            retention_hours: bounded_u64_env(
                "MIR2_SPECTATOR_RETENTION_HOURS",
                DEFAULT_RETENTION_HOURS,
                1,
                24 * 365,
            ),
            entity_stale_ms: bounded_u64_env(
                "MIR2_SPECTATOR_ENTITY_STALE_MS",
                DEFAULT_ENTITY_STALE_MS,
                1_000,
                300_000,
            ),
            data_dir: env::var("MIR2_SPECTATOR_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(".mir2-data/spectator")),
        }
    }

    pub fn is_public_map(&self, map: &str) -> bool {
        self.public_maps
            .iter()
            .any(|candidate| candidate == "*" || candidate.eq_ignore_ascii_case(map))
    }

    pub fn authorize(
        &self,
        map: Option<&str>,
        requested_delay_ms: Option<u64>,
        token: Option<&str>,
    ) -> Result<SpectatorAuthorization, String> {
        if !self.enabled {
            return Err("spectator service is disabled".to_string());
        }
        let director =
            self.director_token
                .as_deref()
                .zip(token)
                .is_some_and(|(expected, actual)| {
                    constant_time_eq(expected.as_bytes(), actual.as_bytes())
                });
        if !director {
            if !self.public_enabled {
                return Err("public spectator access is disabled".to_string());
            }
            if let Some(map) = map {
                if !self.is_public_map(map) {
                    return Err(format!("map {map} is not public for spectators"));
                }
            }
        }
        let requested_delay_ms = requested_delay_ms.unwrap_or(self.public_delay_ms);
        let delay_ms = if director {
            requested_delay_ms.min(self.max_delay_ms)
        } else {
            requested_delay_ms
                .max(self.public_delay_ms)
                .min(self.max_delay_ms)
        };
        Ok(SpectatorAuthorization { director, delay_ms })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpectatorAuthorization {
    pub director: bool,
    pub delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpectatorFrame {
    pub schema: String,
    pub recording_id: String,
    pub sequence: u64,
    pub captured_at_ms: u64,
    pub map_file_name: String,
    pub map_title: String,
    pub digest: String,
    #[serde(default)]
    pub events: Vec<SpectatorEvent>,
    pub world: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpectatorEvent {
    pub kind: String,
    pub at_ms: u64,
    pub object_id: Option<u64>,
    pub name: Option<String>,
    pub payload: Value,
}

impl SpectatorFrame {
    pub fn world_for_view(
        &self,
        requested_target: Option<&str>,
        director: bool,
        camera: Option<(i32, i32)>,
    ) -> Value {
        let mut world = self.world.clone();
        let Some(object) = world.as_object_mut() else {
            return world;
        };
        let entities = object
            .get_mut("entities")
            .and_then(Value::as_array_mut)
            .expect("sanitized spectator world always has entities");

        let selected_index = requested_target
            .filter(|target| !target.trim().is_empty())
            .and_then(|target| {
                entities.iter().position(|entity| {
                    entity
                        .get("name")
                        .and_then(Value::as_str)
                        .is_some_and(|name| name.eq_ignore_ascii_case(target))
                })
            })
            .or_else(|| director.then(|| director_target_index(entities)).flatten())
            .or_else(|| {
                entities.iter().position(|entity| {
                    matches!(
                        entity.get("kind").and_then(Value::as_str),
                        Some("selfPlayer" | "player")
                    )
                })
            });

        for entity in entities.iter_mut() {
            if entity.get("kind").and_then(Value::as_str) == Some("selfPlayer") {
                entity["kind"] = Value::String("player".to_string());
            }
        }

        if let Some((x, y)) = camera {
            let camera_id = u32::MAX;
            entities.push(json!({
                "objectId": camera_id,
                "kind": "selfPlayer",
                "name": "Director Camera",
                "x": x,
                "y": y,
                "direction": "Down",
                "hp": 1,
                "maxHp": 1,
                "light": 0,
                "nameColourArgb": 0,
                "dead": false,
                "disposition": "neutral",
                "sprite": null,
                "questIds": []
            }));
            object.insert("playerObjectId".to_string(), json!(camera_id));
            object.insert("playerHp".to_string(), json!(1));
            object.insert("playerMaxHp".to_string(), json!(1));
        } else if let Some(index) = selected_index {
            let entity = &mut entities[index];
            entity["kind"] = Value::String("selfPlayer".to_string());
            let object_id = entity.get("objectId").cloned().unwrap_or(Value::Null);
            let hp = entity.get("hp").cloned().unwrap_or(Value::Null);
            let max_hp = entity.get("maxHp").cloned().unwrap_or(Value::Null);
            object.insert("playerObjectId".to_string(), object_id);
            object.insert("playerHp".to_string(), hp);
            object.insert("playerMaxHp".to_string(), max_hp);
        }
        world
    }

    pub fn targets(&self) -> Vec<SpectatorTarget> {
        self.world
            .get("entities")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|entity| {
                matches!(
                    entity.get("kind").and_then(Value::as_str),
                    Some("selfPlayer" | "player")
                )
            })
            .filter_map(|entity| {
                Some(SpectatorTarget {
                    object_id: entity.get("objectId")?.as_u64()? as u32,
                    name: entity.get("name")?.as_str()?.to_string(),
                    hp: entity
                        .get("hp")
                        .and_then(Value::as_i64)
                        .map(|value| value as i32),
                    max_hp: entity
                        .get("maxHp")
                        .and_then(Value::as_i64)
                        .map(|value| value as i32),
                    x: entity.get("x")?.as_i64()? as i32,
                    y: entity.get("y")?.as_i64()? as i32,
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpectatorTarget {
    pub object_id: u32,
    pub name: String,
    pub hp: Option<i32>,
    pub max_hp: Option<i32>,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpectatorMatch {
    pub map_file_name: String,
    pub map_title: String,
    pub recording_id: String,
    pub latest_sequence: u64,
    pub latest_captured_at_ms: u64,
    pub player_count: usize,
    pub entity_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpectatorRecording {
    pub recording_id: String,
    pub map_file_name: String,
    pub bytes: u64,
    pub modified_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpectatorMetrics {
    pub active_viewers: usize,
    pub active_maps: usize,
    pub recording_enabled: bool,
    pub buffered_frames: usize,
    pub published_frames_total: u64,
    pub persisted_frames_total: u64,
    pub dropped_frames_total: u64,
    pub recording_errors_total: u64,
    pub data_dir: String,
}

#[derive(Debug, Default)]
struct MapStream {
    frames: VecDeque<SpectatorFrame>,
    latest_world: Option<Value>,
    latest_map_title: String,
    entity_last_seen_ms: BTreeMap<u64, u64>,
    drop_last_seen_ms: BTreeMap<u64, u64>,
    pending_events: Vec<SpectatorEvent>,
    last_emit_ms: u64,
    sequence: u64,
}

#[derive(Debug, Default)]
struct SpectatorState {
    maps: BTreeMap<String, MapStream>,
    published_frames_total: u64,
    persisted_frames_total: u64,
    dropped_frames_total: u64,
    recording_errors_total: u64,
}

#[derive(Debug, Clone)]
pub struct SpectatorHub {
    config: Arc<SpectatorConfig>,
    state: Arc<Mutex<SpectatorState>>,
    active_viewers: Arc<AtomicUsize>,
}

pub struct SpectatorViewerGuard {
    active_viewers: Arc<AtomicUsize>,
}

impl Drop for SpectatorViewerGuard {
    fn drop(&mut self) {
        self.active_viewers.fetch_sub(1, Ordering::Relaxed);
    }
}

impl SpectatorHub {
    pub fn from_env() -> Self {
        Self::new(SpectatorConfig::from_env())
    }

    pub fn new(config: SpectatorConfig) -> Self {
        if config.enabled && config.recording_enabled {
            if let Err(error) = fs::create_dir_all(&config.data_dir) {
                eprintln!(
                    "spectator recording directory {} unavailable: {error}",
                    config.data_dir.display()
                );
            }
            prune_recordings(&config);
        }
        Self {
            config: Arc::new(config),
            state: Arc::new(Mutex::new(SpectatorState::default())),
            active_viewers: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn config(&self) -> &SpectatorConfig {
        &self.config
    }

    pub fn viewer_connected(&self) -> SpectatorViewerGuard {
        self.active_viewers.fetch_add(1, Ordering::Relaxed);
        SpectatorViewerGuard {
            active_viewers: Arc::clone(&self.active_viewers),
        }
    }

    pub fn publish(&self, snapshot: &WorldSnapshot) -> Result<Option<SpectatorFrame>, String> {
        if !self.config.enabled {
            return Ok(None);
        }
        let map_file_name = snapshot
            .map_file_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "world snapshot has no map file name".to_string())?
            .to_string();
        let now_ms = now_ms();
        let sanitized = sanitize_world(snapshot, self.config.max_entities)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| "spectator hub mutex poisoned".to_string())?;
        let stream = state.maps.entry(map_file_name.clone()).or_default();
        record_seen_ids(
            &sanitized,
            "entities",
            now_ms,
            &mut stream.entity_last_seen_ms,
        );
        record_seen_ids(
            &sanitized,
            "groundDrops",
            now_ms,
            &mut stream.drop_last_seen_ms,
        );
        let previous_world = stream.latest_world.clone();
        let mut merged_world = merge_worlds(stream.latest_world.take(), sanitized);
        prune_stale_objects(
            &mut merged_world,
            "entities",
            now_ms,
            self.config.entity_stale_ms,
            &mut stream.entity_last_seen_ms,
        );
        prune_stale_objects(
            &mut merged_world,
            "groundDrops",
            now_ms,
            self.config.entity_stale_ms,
            &mut stream.drop_last_seen_ms,
        );
        stream
            .pending_events
            .extend(world_events(previous_world.as_ref(), &merged_world, now_ms));
        if stream.pending_events.len() > MAX_EVENTS_PER_FRAME * 4 {
            let remove = stream.pending_events.len() - MAX_EVENTS_PER_FRAME * 4;
            stream.pending_events.drain(..remove);
        }
        stream.latest_world = Some(merged_world);
        stream.latest_map_title = snapshot
            .map_title
            .clone()
            .unwrap_or_else(|| map_file_name.clone());
        if now_ms.saturating_sub(stream.last_emit_ms) < self.config.capture_interval_ms {
            return Ok(None);
        }
        stream.last_emit_ms = now_ms;
        stream.sequence = stream.sequence.saturating_add(1);
        let recording_id = recording_id(&map_file_name, now_ms);
        let world = stream
            .latest_world
            .clone()
            .ok_or_else(|| "spectator merged world disappeared".to_string())?;
        let events = if stream.pending_events.len() > MAX_EVENTS_PER_FRAME {
            stream
                .pending_events
                .split_off(stream.pending_events.len() - MAX_EVENTS_PER_FRAME)
        } else {
            std::mem::take(&mut stream.pending_events)
        };
        let encoded = serde_json::to_vec(&json!({
            "world": &world,
            "events": &events
        }))
        .map_err(|error| format!("encode spectator frame payload failed: {error}"))?;
        if encoded.len() > MAX_FRAME_BYTES {
            state.dropped_frames_total = state.dropped_frames_total.saturating_add(1);
            return Err(format!(
                "spectator frame is {} bytes; maximum is {MAX_FRAME_BYTES}",
                encoded.len()
            ));
        }
        let digest = hex_digest(Sha256::digest(&encoded).as_slice());
        let frame = SpectatorFrame {
            schema: SPECTATOR_SCHEMA.to_string(),
            recording_id,
            sequence: stream.sequence,
            captured_at_ms: now_ms,
            map_file_name,
            map_title: stream.latest_map_title.clone(),
            digest,
            events,
            world,
        };
        stream.frames.push_back(frame.clone());
        while stream.frames.len() > self.config.ring_frames {
            stream.frames.pop_front();
        }
        state.published_frames_total = state.published_frames_total.saturating_add(1);
        if self.config.recording_enabled {
            match append_frame(&self.config.data_dir, &frame) {
                Ok(()) => {
                    state.persisted_frames_total = state.persisted_frames_total.saturating_add(1)
                }
                Err(error) => {
                    state.recording_errors_total = state.recording_errors_total.saturating_add(1);
                    eprintln!("spectator recording append failed: {error}");
                }
            }
        }
        Ok(Some(frame))
    }

    pub fn matches(&self, director: bool) -> Vec<SpectatorMatch> {
        let Ok(state) = self.state.lock() else {
            return Vec::new();
        };
        state
            .maps
            .iter()
            .filter(|(map, _)| director || self.config.is_public_map(map))
            .filter_map(|(map, stream)| {
                let latest = stream.frames.back()?;
                Some(SpectatorMatch {
                    map_file_name: map.clone(),
                    map_title: latest.map_title.clone(),
                    recording_id: latest.recording_id.clone(),
                    latest_sequence: latest.sequence,
                    latest_captured_at_ms: latest.captured_at_ms,
                    player_count: latest.targets().len(),
                    entity_count: latest
                        .world
                        .get("entities")
                        .and_then(Value::as_array)
                        .map_or(0, Vec::len),
                })
            })
            .collect()
    }

    pub fn frame_at(
        &self,
        map: &str,
        visible_at_ms: u64,
        after_sequence: u64,
    ) -> Option<SpectatorFrame> {
        let state = self.state.lock().ok()?;
        state
            .maps
            .get(map)?
            .frames
            .iter()
            .rev()
            .find(|frame| frame.captured_at_ms <= visible_at_ms && frame.sequence > after_sequence)
            .cloned()
    }

    pub fn latest_map(&self, director: bool) -> Option<String> {
        self.matches(director)
            .into_iter()
            .max_by_key(|entry| entry.latest_captured_at_ms)
            .map(|entry| entry.map_file_name)
    }

    pub fn recordings(&self, director: bool) -> Vec<SpectatorRecording> {
        if !self.config.recording_enabled {
            return Vec::new();
        }
        let Ok(entries) = fs::read_dir(&self.config.data_dir) else {
            return Vec::new();
        };
        let mut recordings = entries
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                let recording_id = path.file_stem()?.to_str()?.to_string();
                if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
                    return None;
                }
                let map_file_name = map_from_recording_id(&recording_id)?;
                if !director && !self.config.is_public_map(&map_file_name) {
                    return None;
                }
                let metadata = entry.metadata().ok()?;
                let modified_at_ms = metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |duration| duration.as_millis() as u64);
                Some(SpectatorRecording {
                    recording_id,
                    map_file_name,
                    bytes: metadata.len(),
                    modified_at_ms,
                })
            })
            .collect::<Vec<_>>();
        recordings.sort_by_key(|entry| std::cmp::Reverse(entry.modified_at_ms));
        recordings
    }

    pub fn load_replay(
        &self,
        recording_id: &str,
        director: bool,
        public_visible_at_ms: u64,
    ) -> Result<Vec<SpectatorFrame>, String> {
        if !self.config.recording_enabled {
            return Err("spectator recordings are disabled".to_string());
        }
        validate_recording_id(recording_id)?;
        let map = map_from_recording_id(recording_id)
            .ok_or_else(|| "invalid spectator recording id".to_string())?;
        if !director && !self.config.is_public_map(&map) {
            return Err("recording is not public".to_string());
        }
        let path = self.config.data_dir.join(format!("{recording_id}.jsonl"));
        let file = File::open(&path).map_err(|error| {
            format!(
                "open spectator recording {} failed: {error}",
                path.display()
            )
        })?;
        let mut frames = Vec::new();
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|error| format!("read spectator recording failed: {error}"))?;
            if line.trim().is_empty() {
                continue;
            }
            let frame: SpectatorFrame = serde_json::from_str(&line)
                .map_err(|error| format!("decode spectator recording failed: {error}"))?;
            if director || frame.captured_at_ms <= public_visible_at_ms {
                frames.push(frame);
            }
            if frames.len() >= self.config.replay_limit {
                break;
            }
        }
        Ok(frames)
    }

    pub fn metrics(&self) -> SpectatorMetrics {
        let state = self.state.lock().expect("spectator hub mutex poisoned");
        SpectatorMetrics {
            active_viewers: self.active_viewers.load(Ordering::Relaxed),
            active_maps: state.maps.len(),
            recording_enabled: self.config.recording_enabled,
            buffered_frames: state.maps.values().map(|stream| stream.frames.len()).sum(),
            published_frames_total: state.published_frames_total,
            persisted_frames_total: state.persisted_frames_total,
            dropped_frames_total: state.dropped_frames_total,
            recording_errors_total: state.recording_errors_total,
            data_dir: self.config.data_dir.display().to_string(),
        }
    }
}

fn sanitize_world(snapshot: &WorldSnapshot, max_entities: usize) -> Result<Value, String> {
    let value = serde_json::to_value(snapshot.client_view())
        .map_err(|error| format!("encode world snapshot for spectator failed: {error}"))?;
    let source = value
        .as_object()
        .ok_or_else(|| "world snapshot did not encode to an object".to_string())?;
    let entities = source
        .get("entities")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .take(max_entities)
        .map(sanitize_entity)
        .collect::<Vec<_>>();
    let drops = source
        .get("groundDrops")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    Ok(json!({
        "tick": source.get("tick").cloned().unwrap_or(json!(0)),
        "mapTitle": source.get("mapTitle").cloned().unwrap_or(Value::Null),
        "mapFileName": source.get("mapFileName").cloned().unwrap_or(Value::Null),
        "inSafeZone": source.get("inSafeZone").cloned().unwrap_or(json!(false)),
        "lightSetting": source.get("lightSetting").cloned().unwrap_or(json!(0)),
        "playerObjectId": Value::Null,
        "playerHp": Value::Null,
        "playerMaxHp": Value::Null,
        "playerMp": Value::Null,
        "playerMaxMp": Value::Null,
        "playerExperience": 0,
        "playerMaxExperience": 1,
        "gold": 0,
        "credit": 0,
        "cityCurrencies": {},
        "currentWeight": 0,
        "maxWeight": 0,
        "freeBagSlots": 0,
        "maxBagSlots": 0,
        "storageSize": 0,
        "hasExpandedStorage": false,
        "hasStoragePassword": false,
        "requireStoragePassword": false,
        "sceneView": source.get("sceneView").cloned().unwrap_or(Value::Null),
        "terrainPatches": source.get("terrainPatches").cloned().unwrap_or(json!([])),
        "decorObjects": source.get("decorObjects").cloned().unwrap_or(json!([])),
        "entities": entities,
        "groundDrops": drops,
        "beltItems": [],
        "inventoryItems": [],
        "storageItems": [],
        "equipmentItems": [],
        "questLog": [],
        "activeNpcDialog": Value::Null,
        "npcScriptDiagnostics": [],
        "knownSkills": [],
        "activeBuffs": [],
        "stage5Systems": {},
        "mapTransfers": source.get("mapTransfers").cloned().unwrap_or(json!([])),
        "interactionHints": []
    }))
}

fn sanitize_entity(mut entity: Value) -> Value {
    if let Some(object) = entity.as_object_mut() {
        object.remove("questIds");
        object.insert("questIds".to_string(), json!([]));
    }
    entity
}

fn merge_worlds(previous: Option<Value>, next: Value) -> Value {
    let Some(mut previous) = previous else {
        return next;
    };
    let Some(previous_object) = previous.as_object_mut() else {
        return next;
    };
    let Some(next_object) = next.as_object() else {
        return previous;
    };
    for (key, value) in next_object {
        if key != "entities" && key != "groundDrops" {
            previous_object.insert(key.clone(), value.clone());
        }
    }
    merge_object_array(previous_object, next_object, "entities", "objectId");
    merge_object_array(previous_object, next_object, "groundDrops", "objectId");
    previous
}

fn merge_object_array(
    previous: &mut Map<String, Value>,
    next: &Map<String, Value>,
    field: &str,
    key: &str,
) {
    let mut values = BTreeMap::<u64, Value>::new();
    for item in previous
        .get(field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .chain(
            next.get(field)
                .and_then(Value::as_array)
                .into_iter()
                .flatten(),
        )
    {
        if let Some(id) = item.get(key).and_then(Value::as_u64) {
            values.insert(id, item.clone());
        }
    }
    previous.insert(
        field.to_string(),
        Value::Array(values.into_values().collect()),
    );
}

fn record_seen_ids(world: &Value, field: &str, now_ms: u64, last_seen: &mut BTreeMap<u64, u64>) {
    for object in world
        .get(field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(object_id) = object.get("objectId").and_then(Value::as_u64) {
            last_seen.insert(object_id, now_ms);
        }
    }
}

fn prune_stale_objects(
    world: &mut Value,
    field: &str,
    now_ms: u64,
    stale_ms: u64,
    last_seen: &mut BTreeMap<u64, u64>,
) {
    let Some(objects) = world.get_mut(field).and_then(Value::as_array_mut) else {
        return;
    };
    objects.retain(|object| {
        let Some(object_id) = object.get("objectId").and_then(Value::as_u64) else {
            return false;
        };
        last_seen
            .get(&object_id)
            .is_some_and(|seen_at| now_ms.saturating_sub(*seen_at) <= stale_ms)
    });
    last_seen.retain(|_, seen_at| now_ms.saturating_sub(*seen_at) <= stale_ms);
}

fn indexed_objects<'a>(world: Option<&'a Value>, field: &str) -> BTreeMap<u64, &'a Value> {
    world
        .and_then(|world| world.get(field))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|object| {
            object
                .get("objectId")
                .and_then(Value::as_u64)
                .map(|object_id| (object_id, object))
        })
        .collect()
}

fn world_events(previous: Option<&Value>, current: &Value, at_ms: u64) -> Vec<SpectatorEvent> {
    let previous_entities = indexed_objects(previous, "entities");
    let current_entities = indexed_objects(Some(current), "entities");
    let mut events = Vec::new();

    for (object_id, entity) in &current_entities {
        let name = entity
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string);
        let Some(previous) = previous_entities.get(object_id) else {
            events.push(SpectatorEvent {
                kind: "spawn".to_string(),
                at_ms,
                object_id: Some(*object_id),
                name,
                payload: json!({
                    "x": entity.get("x"),
                    "y": entity.get("y"),
                    "entityKind": entity.get("kind")
                }),
            });
            continue;
        };
        let previous_position = (
            previous.get("x").and_then(Value::as_i64),
            previous.get("y").and_then(Value::as_i64),
        );
        let current_position = (
            entity.get("x").and_then(Value::as_i64),
            entity.get("y").and_then(Value::as_i64),
        );
        if previous_position != current_position {
            events.push(SpectatorEvent {
                kind: "move".to_string(),
                at_ms,
                object_id: Some(*object_id),
                name: name.clone(),
                payload: json!({
                    "from": {"x": previous_position.0, "y": previous_position.1},
                    "to": {"x": current_position.0, "y": current_position.1}
                }),
            });
        }
        let previous_hp = previous.get("hp").and_then(Value::as_i64);
        let current_hp = entity.get("hp").and_then(Value::as_i64);
        if previous_hp != current_hp {
            events.push(SpectatorEvent {
                kind: "health".to_string(),
                at_ms,
                object_id: Some(*object_id),
                name: name.clone(),
                payload: json!({"from": previous_hp, "to": current_hp}),
            });
        }
        let previous_dead = previous
            .get("dead")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let current_dead = entity.get("dead").and_then(Value::as_bool).unwrap_or(false);
        if previous_dead != current_dead {
            events.push(SpectatorEvent {
                kind: if current_dead { "death" } else { "revive" }.to_string(),
                at_ms,
                object_id: Some(*object_id),
                name,
                payload: json!({"dead": current_dead}),
            });
        }
    }
    for (object_id, entity) in &previous_entities {
        if !current_entities.contains_key(object_id) {
            events.push(SpectatorEvent {
                kind: "despawn".to_string(),
                at_ms,
                object_id: Some(*object_id),
                name: entity
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                payload: json!({}),
            });
        }
    }

    let previous_drops = indexed_objects(previous, "groundDrops");
    let current_drops = indexed_objects(Some(current), "groundDrops");
    for (object_id, drop) in &current_drops {
        if !previous_drops.contains_key(object_id) {
            events.push(SpectatorEvent {
                kind: "dropSpawn".to_string(),
                at_ms,
                object_id: Some(*object_id),
                name: drop
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                payload: json!({"x": drop.get("x"), "y": drop.get("y"), "quantity": drop.get("quantity")}),
            });
        }
    }
    for (object_id, drop) in &previous_drops {
        if !current_drops.contains_key(object_id) {
            events.push(SpectatorEvent {
                kind: "dropRemove".to_string(),
                at_ms,
                object_id: Some(*object_id),
                name: drop.get("name").and_then(Value::as_str).map(str::to_string),
                payload: json!({}),
            });
        }
    }
    events
}

fn director_target_index(entities: &[Value]) -> Option<usize> {
    entities
        .iter()
        .enumerate()
        .filter(|(_, entity)| {
            matches!(
                entity.get("kind").and_then(Value::as_str),
                Some("selfPlayer" | "player")
            )
        })
        .max_by_key(|(_, entity)| {
            let hp = entity.get("hp").and_then(Value::as_i64).unwrap_or(1);
            let max_hp = entity
                .get("maxHp")
                .and_then(Value::as_i64)
                .unwrap_or(hp.max(1));
            let danger = ((max_hp - hp).max(0) * 100 / max_hp.max(1)) as u64;
            let x = entity.get("x").and_then(Value::as_i64).unwrap_or(0);
            let y = entity.get("y").and_then(Value::as_i64).unwrap_or(0);
            let nearby = entities
                .iter()
                .filter(|candidate| {
                    candidate.get("kind").and_then(Value::as_str) == Some("monster")
                        && (candidate
                            .get("x")
                            .and_then(Value::as_i64)
                            .unwrap_or(i64::MAX)
                            - x)
                            .abs()
                            <= 8
                        && (candidate
                            .get("y")
                            .and_then(Value::as_i64)
                            .unwrap_or(i64::MAX)
                            - y)
                            .abs()
                            <= 8
                })
                .count() as u64;
            danger.saturating_add(nearby.saturating_mul(20))
        })
        .map(|(index, _)| index)
}

fn append_frame(data_dir: &Path, frame: &SpectatorFrame) -> Result<(), String> {
    fs::create_dir_all(data_dir)
        .map_err(|error| format!("create spectator data dir failed: {error}"))?;
    let path = data_dir.join(format!("{}.jsonl", frame.recording_id));
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("open {} failed: {error}", path.display()))?;
    serde_json::to_writer(&mut file, frame)
        .map_err(|error| format!("encode {} failed: {error}", path.display()))?;
    file.write_all(b"\n")
        .and_then(|()| file.flush())
        .map_err(|error| format!("flush {} failed: {error}", path.display()))
}

fn prune_recordings(config: &SpectatorConfig) {
    let Ok(entries) = fs::read_dir(&config.data_dir) else {
        return;
    };
    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(
            config.retention_hours.saturating_mul(60 * 60),
        ))
        .unwrap_or(UNIX_EPOCH);
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
            continue;
        }
        let expired = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .is_some_and(|modified| modified < cutoff);
        if expired {
            if let Err(error) = fs::remove_file(&path) {
                eprintln!(
                    "remove expired spectator recording {} failed: {error}",
                    path.display()
                );
            }
        }
    }
}

fn recording_id(map: &str, now_ms: u64) -> String {
    let hour = now_ms / 3_600_000;
    format!("{}-{hour}", safe_component(map))
}

fn map_from_recording_id(recording_id: &str) -> Option<String> {
    let (map, hour) = recording_id.rsplit_once('-')?;
    hour.parse::<u64>().ok()?;
    (!map.is_empty()).then(|| map.to_string())
}

fn validate_recording_id(recording_id: &str) -> Result<(), String> {
    if recording_id.len() > 128
        || recording_id.is_empty()
        || !recording_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("invalid spectator recording id".to_string());
    }
    Ok(())
}

fn safe_component(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn bool_env(name: &str, default: bool) -> bool {
    env::var(name)
        .ok()
        .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        })
        .unwrap_or(default)
}

fn bounded_u64_env(name: &str, default: u64, min: u64, max: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(default)
        .clamp(min, max)
}

fn bounded_usize_env(name: &str, default: usize, min: usize, max: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(default)
        .clamp(min, max)
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0u8, |difference, (left, right)| difference | (left ^ right))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GatewayConfig;
    use mir2_protocol::{ClientPacket, UserItem};
    use mir2_simulation::{
        GroundDropItemPayload, GroundDropLootSnapshot, GroundDropSnapshot, SimulationSession,
    };

    fn exact_ground_drop() -> GroundDropSnapshot {
        GroundDropSnapshot {
            object_id: 9_101,
            name: "Dagger".to_string(),
            name_colour_argb: -1,
            icon: 1,
            x: 10,
            y: 20,
            quantity: 1,
            source_monster: "spectator-test".to_string(),
            owner_object_id: None,
            ownership_remaining_ticks: None,
            loot: GroundDropLootSnapshot::InventoryItem {
                key: "crystal-item-222".to_string(),
                name: "Dagger".to_string(),
                description: String::new(),
                weight: 5,
                durability_current: Some(1_000),
                durability_max: Some(2_000),
                added_attack: 0,
                added_defence: 0,
                added_stats: Vec::new(),
                cursed: false,
                socket_slots: 0,
                show_group_pickup: false,
                exact_item: Some(GroundDropItemPayload {
                    uid_assigned: true,
                    item: UserItem {
                        unique_id: 88_101,
                        item_index: 222,
                        current_dura: 1_000,
                        max_dura: 2_000,
                        count: 1,
                        soul_bound_id: -1,
                        identified: true,
                        cursed: false,
                        slots: Vec::new(),
                        gem_count: 0,
                        added_stats: Vec::new(),
                        awake_type: 3,
                        awake_values: vec![9],
                        refined_value: 0,
                        refine_added: 0,
                        refine_success_chance: 0,
                        wedding_ring: -1,
                        expire_info: None,
                        rental_information: None,
                        is_shop_item: false,
                        sealed_info: None,
                        gm_made: false,
                    },
                }),
            },
        }
    }

    fn test_config(data_dir: PathBuf) -> SpectatorConfig {
        SpectatorConfig {
            enabled: true,
            recording_enabled: true,
            public_enabled: true,
            public_maps: vec!["0".to_string()],
            director_token: Some("director-secret".to_string()),
            capture_interval_ms: 100,
            public_delay_ms: 30_000,
            max_delay_ms: 120_000,
            ring_frames: 40,
            max_entities: 128,
            replay_limit: 100,
            retention_hours: 1,
            entity_stale_ms: 15_000,
            data_dir,
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let path = env::temp_dir().join(format!("mir2-spectator-{name}-{}", now_ms()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn started_demo_session() -> SimulationSession {
        let mut session = SimulationSession::new(GatewayConfig::default());
        let login = session.handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        });
        assert!(login
            .iter()
            .any(|packet| matches!(packet, mir2_protocol::ServerPacket::LoginSuccess { .. })));
        let start = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        assert!(start.iter().any(|packet| matches!(
            packet,
            mir2_protocol::ServerPacket::StartGame { result: 4, .. }
        )));
        session
    }

    #[test]
    fn public_delay_is_enforced_but_director_can_be_live() {
        let config = test_config(temp_dir("authorization"));
        assert_eq!(
            config.authorize(Some("0"), Some(0), None).unwrap(),
            SpectatorAuthorization {
                director: false,
                delay_ms: 30_000
            }
        );
        assert_eq!(
            config
                .authorize(Some("0"), Some(0), Some("director-secret"))
                .unwrap(),
            SpectatorAuthorization {
                director: true,
                delay_ms: 0
            }
        );
        assert!(config.authorize(Some("secret-map"), None, None).is_err());
    }

    #[test]
    fn sanitized_recording_excludes_private_inventory_and_supports_replay() {
        let data_dir = temp_dir("recording");
        let hub = SpectatorHub::new(test_config(data_dir.clone()));
        let session = started_demo_session();
        let snapshot = session.world_snapshot();
        let frame = hub.publish(&snapshot).unwrap().unwrap();
        assert_eq!(frame.world["inventoryItems"], json!([]));
        assert_eq!(frame.world["storageItems"], json!([]));
        assert_eq!(frame.world["questLog"], json!([]));
        assert!(!frame.targets().is_empty());
        assert!(frame.events.iter().any(|event| event.kind == "spawn"));
        let replay = hub
            .load_replay(&frame.recording_id, true, u64::MAX)
            .unwrap();
        assert_eq!(replay.len(), 1);
        assert_eq!(replay[0].digest, frame.digest);
        fs::remove_dir_all(data_dir).ok();
    }

    #[test]
    fn spectator_live_and_replay_redact_exact_ground_item_identity() {
        let data_dir = temp_dir("exact-ground-drop-redaction");
        let hub = SpectatorHub::new(test_config(data_dir.clone()));
        let mut snapshot = started_demo_session().world_snapshot();
        snapshot.ground_drops.push(exact_ground_drop());
        let frame = hub.publish(&snapshot).unwrap().unwrap();
        assert!(frame.world["groundDrops"][0]["loot"]
            .get("exactItem")
            .is_none());
        let replay = hub
            .load_replay(&frame.recording_id, true, u64::MAX)
            .unwrap();
        assert!(replay[0].world["groundDrops"][0]["loot"]
            .get("exactItem")
            .is_none());
        fs::remove_dir_all(data_dir).ok();
    }

    #[test]
    fn recording_disabled_keeps_live_ring_but_does_not_touch_disk() {
        let root = temp_dir("recording-disabled");
        let data_dir = root.join("recordings");
        let mut config = test_config(data_dir.clone());
        config.recording_enabled = false;
        let hub = SpectatorHub::new(config);
        let session = started_demo_session();

        let frame = hub.publish(&session.world_snapshot()).unwrap().unwrap();
        let metrics = hub.metrics();
        assert_eq!(metrics.recording_enabled, false);
        assert_eq!(
            serde_json::to_value(&metrics).unwrap()["recordingEnabled"],
            false
        );
        assert_eq!(metrics.published_frames_total, 1);
        assert_eq!(metrics.persisted_frames_total, 0);
        assert_eq!(metrics.buffered_frames, 1);
        assert!(!data_dir.exists());
        assert!(hub.recordings(true).is_empty());
        let error = hub
            .load_replay(&frame.recording_id, true, u64::MAX)
            .unwrap_err();
        assert_eq!(error, "spectator recordings are disabled");
        assert!(!root
            .read_dir()
            .unwrap()
            .any(|entry| entry.unwrap().path().extension().is_some()));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn camera_or_target_becomes_the_only_self_player() {
        let data_dir = temp_dir("view");
        let hub = SpectatorHub::new(test_config(data_dir.clone()));
        let session = started_demo_session();
        let frame = hub.publish(&session.world_snapshot()).unwrap().unwrap();
        let target = frame.targets().first().unwrap().name.clone();
        let world = frame.world_for_view(Some(&target), false, None);
        let self_players = world["entities"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|entity| entity["kind"] == "selfPlayer")
            .count();
        assert_eq!(self_players, 1);
        let camera = frame.world_for_view(None, false, Some((12, 34)));
        assert_eq!(camera["playerObjectId"], json!(u32::MAX));
        fs::remove_dir_all(data_dir).ok();
    }

    #[test]
    fn event_timeline_tracks_movement_health_death_and_drops() {
        let previous = json!({
            "entities": [{"objectId": 7, "name": "Scout", "kind": "player", "x": 10, "y": 20, "hp": 60, "dead": false}],
            "groundDrops": []
        });
        let current = json!({
            "entities": [{"objectId": 7, "name": "Scout", "kind": "player", "x": 11, "y": 20, "hp": 0, "dead": true}],
            "groundDrops": [{"objectId": 90, "name": "Sword", "x": 11, "y": 20, "quantity": 1}]
        });
        let events = world_events(Some(&previous), &current, 1234);
        for expected in ["move", "health", "death", "dropSpawn"] {
            assert!(
                events.iter().any(|event| event.kind == expected),
                "missing {expected}: {events:?}"
            );
        }
    }

    #[test]
    fn stale_entities_are_pruned_from_the_merged_world() {
        let mut world = json!({
            "entities": [
                {"objectId": 1, "name": "fresh"},
                {"objectId": 2, "name": "stale"}
            ]
        });
        let mut last_seen = BTreeMap::from([(1, 9_500), (2, 1_000)]);
        prune_stale_objects(&mut world, "entities", 10_000, 1_000, &mut last_seen);
        assert_eq!(world["entities"].as_array().unwrap().len(), 1);
        assert_eq!(world["entities"][0]["name"], "fresh");
        assert_eq!(last_seen.keys().copied().collect::<Vec<_>>(), vec![1]);
    }
}
