//! Ignored, bounded full-Hub profile, including its real mutex and recording path.
//!
//! Public shape reference: one 29,007-byte JSONL record (including newline),
//! SHA256 b91a89d851c83db0d61d2948b5d26b4bc89c9f54f272a55067db4026ab0faf53.
//! It had 26 sprite-rich players, zero terrain/decor/drops, and 19 transfers.
//! This generated fixture uses anonymous, stable global IDs and synthetic views;
//! it is neither that recording's identities nor a gateway/AOI/load simulation.
//! Inputs and validation are outside publish timers. Recording remains enabled.
//! Before each round, ONLY this test resets last_emit_ms while workers are idle.
//! The real 250ms gate/clock still run, and every natural additional emit counts.
//! Thus this measures forced capture opportunities, NOT four-Hz throughput/CCU.
//! Files are exclusively created on C: and retained. Run in an isolated bounded
//! Release job; a Rust wall assertion cannot preempt a blocked OS file operation.

use super::{
    hex_digest, now_ms, SpectatorConfig, SpectatorEvent, SpectatorFrame, SpectatorHub,
    DEFAULT_CAPTURE_INTERVAL_MS, DEFAULT_ENTITY_STALE_MS, DEFAULT_MAX_DELAY_MS,
    DEFAULT_MAX_ENTITIES, DEFAULT_PUBLIC_DELAY_MS, DEFAULT_REPLAY_LIMIT, DEFAULT_RETENTION_HOURS,
    DEFAULT_RING_FRAMES, MAX_FRAME_BYTES,
};
use mir2_simulation::{WorldEntitySnapshot, WorldSnapshot};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Serialize, Serializer};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const PROFILE_MAP: &str = "SyntheticFullPublish";
const SOURCE_RECORD_SHA256: &str =
    "b91a89d851c83db0d61d2948b5d26b4bc89c9f54f272a55067db4026ab0faf53";
const ROUNDS: usize = 32;
const TRIALS: usize = 3;
const MAX_EMITS_PER_TRIAL: usize = 512;
const MAX_RECORD_BYTES_PER_TRIAL: u64 = 128 * 1024 * 1024;
const MAX_RECORD_BYTES_TOTAL: u64 = 512 * 1024 * 1024;
const PROFILE_WALL: Duration = Duration::from_secs(600);
const CHANNEL_WAIT: Duration = Duration::from_secs(30);

#[derive(Default)]
struct Durations(Vec<u64>);

impl Durations {
    fn add(&mut self, value: u64) {
        self.0.push(value);
    }

    fn summary(&self) -> Value {
        let mut sorted = self.0.clone();
        sorted.sort_unstable();
        let p95 = (!sorted.is_empty()).then(|| sorted[(sorted.len() * 95).div_ceil(100) - 1]);
        json!({"count":sorted.len(), "totalNs":sorted.iter().map(|v| u128::from(*v)).sum::<u128>(),
            "p95Ns":p95, "maxNs":sorted.last().copied()})
    }
}

fn nanos(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).expect("profile interval fits u64")
}

fn new_output_root() -> PathBuf {
    assert!(
        cfg!(windows),
        "this ignored recording profile requires its explicit C: output root"
    );
    let parent = PathBuf::from("C:/Users/Administrator/AppData/Local/Temp");
    assert!(parent.is_dir());
    let root = parent.join(format!(
        "mir2-spectator-full-publish-{}-{}",
        std::process::id(),
        now_ms()
    ));
    // No create_dir_all on an existing evidence tree, no cleanup, no overwrite.
    fs::create_dir(&root).expect("exclusively create fresh profile root");
    assert!(!fs::symlink_metadata(&root)
        .unwrap()
        .file_type()
        .is_symlink());
    root
}

fn config(data_dir: PathBuf) -> SpectatorConfig {
    SpectatorConfig {
        enabled: true,
        recording_enabled: true,
        public_enabled: true,
        public_maps: vec![PROFILE_MAP.to_string()],
        director_token: None,
        capture_interval_ms: DEFAULT_CAPTURE_INTERVAL_MS,
        public_delay_ms: DEFAULT_PUBLIC_DELAY_MS,
        max_delay_ms: DEFAULT_MAX_DELAY_MS,
        ring_frames: DEFAULT_RING_FRAMES,
        max_entities: DEFAULT_MAX_ENTITIES,
        replay_limit: DEFAULT_REPLAY_LIMIT,
        retention_hours: DEFAULT_RETENTION_HOURS,
        entity_stale_ms: DEFAULT_ENTITY_STALE_MS,
        data_dir,
    }
}

fn entity_template() -> WorldEntitySnapshot {
    // Parse static public DTO JSON instead of expanding a large json! literal
    // through the crate's unchanged default macro recursion limit.
    serde_json::from_str(
        r#"{
        "objectId":10000,"kind":"player","name":"SyntheticPlayer000","ownerName":null,
        "ai":null,"x":40,"y":50,"direction":"Down","class":"Warrior","gender":"Male",
        "level":40,"hp":674,"maxHp":674,"light":3,"wingEffect":0,"nameColourArgb":-1,
        "dead":false,"ridingMount":false,"canMountAttack":true,"hasClassWeapon":false,
        "dazed":false,"fishing":false,"transformType":-1,"disposition":"friendly","questIds":[],
        "sprite":{"altBodyLibrary":null,"altFrameBaseOffset":null,"altHairLibrary":null,
            "altWeaponFrameOffset":null,"altWeaponLibrary":null,"altWeaponLibrarySecondary":null,
            "bodyLibrary":"CArmour/01","directionStride":4,"frameBaseOffset":0,"frameCount":4,
            "hairLibrary":"CHair/00","mountFrameOffset":null,"mountLibrary":null,
            "weaponFrameOffset":0,"weaponLibrary":"CWeapon/00","weaponLibrarySecondary":null}
    }"#,
    )
    .expect("public sprite shape decodes using the real DTO")
}

fn snapshot_template() -> WorldSnapshot {
    // Preserve the reference's transfer DTO/count, but use anonymous destinations.
    let transfers: Vec<Value> = (0..19).map(|i| json!({
        "key":format!("synthetic-transfer-{i}"),"mapFileName":PROFILE_MAP,
        "bounds":{"minX":400+i,"maxX":400+i,"minY":300,"maxY":300},
        "toMapFileName":format!("SyntheticDestination{i}"),"toMapTitle":"Synthetic destination",
        "toPosition":{"x":10,"y":18},"toDirection":"Down"
    })).collect();
    let mut value: Value = serde_json::from_str(
        r#"{
        "tick":0,"mapTitle":"Synthetic public scene","mapFileName":"SyntheticFullPublish",
        "inSafeZone":false,"lightSetting":2,"playerObjectId":null,"playerHp":null,
        "playerMaxHp":null,"playerMp":null,"playerMaxMp":null,"playerExperience":0,
        "playerMaxExperience":1,"gold":0,"credit":0,"currentWeight":0,"maxWeight":0,
        "freeBagSlots":0,"maxBagSlots":0,"storageSize":0,"hasExpandedStorage":false,
        "hasStoragePassword":false,"requireStoragePassword":false,
        "storagePasswordLastSetBinaryDatetime":0,"expandedStorageExpiryTimeBinaryDatetime":0,
        "sceneView":{"center":{"x":47,"y":60},"width":24,"height":18},
        "terrainPatches":[],"decorObjects":[],"entities":[],"groundDrops":[],
        "beltItems":[],"inventoryItems":[],"equipmentItems":[],"questLog":[],
        "activeNpcDialog":null,"npcScriptDiagnostics":[],"knownSkills":[],"activeBuffs":[],
        "stage5Systems":{},"mapTransfers":[],"interactionHints":[]
    }"#,
    )
    .expect("static public snapshot JSON");
    value["mapFileName"] = Value::String(PROFILE_MAP.to_string());
    value["mapTransfers"] = Value::Array(transfers);
    serde_json::from_value(value)
        .expect("synthetic public shape decodes using the real WorldSnapshot DTO")
}

fn round_inputs(owners: usize, round: usize, producers: usize) -> Vec<Vec<WorldSnapshot>> {
    let columns = (owners as f64).sqrt().ceil() as usize;
    let template = entity_template();
    let entities: Vec<_> = (0..owners)
        .map(|id| {
            let mut entity = template.clone();
            entity.object_id = 10000 + id as u32;
            entity.name = format!("SyntheticPlayer{id:03}");
            entity.x = 40 + (id % columns) as i32 * 2 + (round % 2) as i32;
            entity.y = 50 + (id / columns) as i32 * 2;
            entity.hp = Some(674 - (round % 2) as i32);
            entity
        })
        .collect();
    let base = snapshot_template();
    let mut batches: Vec<Vec<WorldSnapshot>> = (0..producers).map(|_| Vec::new()).collect();
    for owner in 0..owners {
        let mut snapshot = base.clone();
        snapshot.tick = (round * owners + owner) as u64;
        let center = &entities[owner];
        snapshot.scene_view.as_mut().unwrap().center.x = center.x;
        snapshot.scene_view.as_mut().unwrap().center.y = center.y;
        // Synthetic overlapping rectangular views; NOT the gameplay AOI algorithm.
        snapshot.entities = entities
            .iter()
            .filter(|e| (e.x - center.x).abs() <= 12 && (e.y - center.y).abs() <= 9)
            .cloned()
            .collect();
        assert!(snapshot
            .entities
            .iter()
            .any(|e| e.object_id == center.object_id));
        batches[owner % producers].push(snapshot);
    }
    batches
}

type StartGate = Arc<(Mutex<bool>, Condvar)>;

struct Batch {
    inputs: Vec<WorldSnapshot>,
    start: StartGate,
}

#[derive(Default)]
struct BatchResult {
    all: Vec<u64>,
    emitted: Vec<u64>,
    gated: Vec<u64>,
    frames: Vec<SpectatorFrame>,
}

enum WorkerEvent {
    Ready(usize),
    Done(usize, Result<BatchResult, String>),
}

fn run_batch(hub: &SpectatorHub, batch: Batch) -> Result<BatchResult, String> {
    let (lock, wake) = &*batch.start;
    let ready = lock.lock().map_err(|_| "start gate poisoned")?;
    let (ready, timeout) = wake
        .wait_timeout_while(ready, CHANNEL_WAIT, |go| !*go)
        .map_err(|_| "start gate poisoned")?;
    if timeout.timed_out() || !*ready {
        return Err("profile start gate timeout".into());
    }
    drop(ready);
    let mut result = BatchResult::default();
    for input in batch.inputs {
        let start = Instant::now();
        let frame = hub.publish(&input);
        let ns = nanos(start);
        // Result processing, returned-frame destruction and digest checks are not
        // timed. Workers retain emitted frames until every producer is idle.
        let frame = frame?;
        result.all.push(ns);
        if let Some(frame) = frame {
            result.emitted.push(ns);
            result.frames.push(frame);
        } else {
            result.gated.push(ns);
        }
    }
    Ok(result)
}

// Same serialization contract as json!({world:&world,events:&events}), without
// manufacturing an owned Value. Key order is taken from that real json! object
// outside timers, to remain exact under either serde_json map-order feature.
struct BorrowedPayload<'a> {
    frame: &'a SpectatorFrame,
    world_first: bool,
    event_keys: &'a [String],
}

impl Serialize for BorrowedPayload<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(2))?;
        let events = BorrowedEvents {
            events: &self.frame.events,
            keys: self.event_keys,
        };
        if self.world_first {
            map.serialize_entry("world", &self.frame.world)?;
            map.serialize_entry("events", &events)?;
        } else {
            map.serialize_entry("events", &events)?;
            map.serialize_entry("world", &self.frame.world)?;
        }
        map.end()
    }
}

struct BorrowedEvents<'a> {
    events: &'a [SpectatorEvent],
    keys: &'a [String],
}

impl Serialize for BorrowedEvents<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.events.len()))?;
        for event in self.events {
            seq.serialize_element(&BorrowedEvent {
                event,
                keys: self.keys,
            })?;
        }
        seq.end()
    }
}

struct BorrowedEvent<'a> {
    event: &'a SpectatorEvent,
    keys: &'a [String],
}

impl Serialize for BorrowedEvent<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.keys.len()))?;
        for key in self.keys {
            match key.as_str() {
                "kind" => map.serialize_entry(key, &self.event.kind)?,
                "atMs" => map.serialize_entry(key, &self.event.at_ms)?,
                "objectId" => map.serialize_entry(key, &self.event.object_id)?,
                "name" => map.serialize_entry(key, &self.event.name)?,
                "payload" => map.serialize_entry(key, &self.event.payload)?,
                _ => {
                    return Err(serde::ser::Error::custom(
                        "unexpected spectator event field",
                    ))
                }
            }
        }
        map.end()
    }
}

#[derive(Default)]
struct EncodeProfile {
    owned: Durations,
    borrowed: Durations,
}

fn verify_payload(frame: &SpectatorFrame, index: usize, profile: &mut EncodeProfile) {
    let order = json!({"world":null,"events":null});
    let world_first = order.as_object().unwrap().keys().next().unwrap() == "world";
    // json! converts struct events into Value objects too: preserve THAT key
    // order, rather than silently comparing differently ordered struct JSON.
    let event_keys: Vec<String> = frame
        .events
        .first()
        .map(|event| {
            serde_json::to_value(event)
                .unwrap()
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    assert!(frame.events.is_empty() || event_keys.len() == 5);
    let owned = || {
        let start = Instant::now();
        let bytes =
            serde_json::to_vec(&json!({"world":&frame.world,"events":&frame.events})).unwrap();
        (bytes, nanos(start))
    };
    let borrowed = || {
        let start = Instant::now();
        let bytes = serde_json::to_vec(&BorrowedPayload {
            frame,
            world_first,
            event_keys: &event_keys,
        })
        .unwrap();
        (bytes, nanos(start))
    };
    let ((old, old_ns), (new, new_ns)) = if index % 2 == 0 {
        let a = owned();
        let b = borrowed();
        (a, b)
    } else {
        let b = borrowed();
        let a = owned();
        (a, b)
    };
    assert_eq!(
        old, new,
        "borrowed serialization must preserve exact payload bytes"
    );
    assert!(old.len() <= MAX_FRAME_BYTES);
    assert_eq!(hex_digest(Sha256::digest(&old).as_slice()), frame.digest);
    assert_eq!(hex_digest(Sha256::digest(&new).as_slice()), frame.digest);
    profile.owned.add(old_ns);
    profile.borrowed.add(new_ns);
}

fn verify_frames(
    hub: &SpectatorHub,
    frames: &mut [SpectatorFrame],
    owners: usize,
    encode: &mut EncodeProfile,
) -> Value {
    frames.sort_by_key(|frame| frame.sequence);
    assert!((ROUNDS..=MAX_EMITS_PER_TRIAL).contains(&frames.len()));
    let mut by_recording: BTreeMap<String, Vec<&SpectatorFrame>> = BTreeMap::new();
    let mut observed = Vec::with_capacity(frames.len());
    let mut event_counts = BTreeMap::<String, usize>::new();
    let mut serialized_frames_hash = Sha256::new();
    let mut max_union = 0;
    for (i, frame) in frames.iter().enumerate() {
        assert_eq!(frame.sequence, i as u64 + 1);
        verify_payload(frame, i, encode);
        let entity_count = frame.world["entities"].as_array().unwrap().len();
        max_union = max_union.max(entity_count);
        assert!(entity_count <= owners);
        assert_eq!(frame.world["terrainPatches"].as_array().unwrap().len(), 0);
        assert_eq!(frame.world["mapTransfers"].as_array().unwrap().len(), 19);
        for field in [
            "inventoryItems",
            "beltItems",
            "equipmentItems",
            "storageItems",
            "questLog",
        ] {
            assert!(frame.world[field].as_array().unwrap().is_empty());
        }
        for event in &frame.events {
            *event_counts.entry(event.kind.clone()).or_default() += 1;
        }
        let bytes = serde_json::to_vec(frame).unwrap();
        serialized_frames_hash.update((bytes.len() as u64).to_le_bytes());
        serialized_frames_hash.update(&bytes);
        observed.push(
            json!({"sequence":frame.sequence,"capturedAtMs":frame.captured_at_ms,
            "recordingId":frame.recording_id,"digest":frame.digest,
            "entities":entity_count,"events":frame.events.len(),"frameBytes":bytes.len()}),
        );
        by_recording
            .entry(frame.recording_id.clone())
            .or_default()
            .push(frame);
    }
    let mut replay_count = 0;
    let mut recorded_bytes = 0;
    for (recording, expected) in by_recording {
        let replay = hub
            .load_replay(&recording, true, u64::MAX)
            .expect("real disk replay");
        assert_eq!(replay.len(), expected.len());
        for (actual, expected) in replay.iter().zip(expected) {
            assert_eq!(
                serde_json::to_vec(actual).unwrap(),
                serde_json::to_vec(expected).unwrap()
            );
        }
        replay_count += replay.len();
        recorded_bytes += fs::metadata(hub.config().data_dir.join(format!("{recording}.jsonl")))
            .unwrap()
            .len();
    }
    assert_eq!(replay_count, frames.len());
    assert!(recorded_bytes <= MAX_RECORD_BYTES_PER_TRIAL);
    let latest = hub
        .frame_at(PROFILE_MAP, u64::MAX, 0)
        .expect("real ring read");
    assert_eq!(
        serde_json::to_vec(&latest).unwrap(),
        serde_json::to_vec(frames.last().unwrap()).unwrap()
    );
    let metrics = hub.metrics();
    assert_eq!(metrics.active_maps, 1);
    assert_eq!(metrics.published_frames_total, frames.len() as u64);
    assert_eq!(metrics.persisted_frames_total, frames.len() as u64);
    assert_eq!(metrics.buffered_frames, frames.len());
    assert_eq!(metrics.recording_errors_total, 0);
    assert_eq!(metrics.dropped_frames_total, 0);
    let state = hub.state.lock().unwrap();
    let stream = state.maps.get(PROFILE_MAP).unwrap();
    let final_union = stream.latest_world.as_ref().unwrap()["entities"]
        .as_array()
        .unwrap()
        .len();
    assert_eq!(final_union, owners);
    assert_eq!(stream.entity_last_seen_ms.len(), owners);
    json!({"dataDirectory":hub.config().data_dir,"emits":frames.len(),"replayFrames":replay_count,
        "recordedBytes":recorded_bytes,"maxEmittedUnion":max_union,"finalUnion":final_union,
        "pendingEventsAfterLastCall":stream.pending_events.len(),"eventCounts":event_counts,
        "serializedFramesLengthPrefixedSha256":hex_digest(serialized_frames_hash.finalize().as_slice()),
        "observedEmits":observed,"metrics":metrics})
}

fn trial(root: &Path, owners: usize, producers: usize, number: usize, wall: Instant) -> Value {
    let directory = root.join(format!(
        "owners-{owners}-producers-{producers}-trial-{number}"
    ));
    fs::create_dir(&directory).expect("exclusively create trial directory");
    let hub = SpectatorHub::new(config(directory));
    let (events_tx, events_rx) = mpsc::channel();
    let mut commands = Vec::new();
    let mut workers = Vec::new();
    for id in 0..producers {
        let (tx, rx) = mpsc::sync_channel::<Batch>(1);
        commands.push(tx);
        let output = events_tx.clone();
        let shared = hub.clone();
        workers.push(thread::spawn(move || {
            while let Ok(batch) = rx.recv() {
                if output.send(WorkerEvent::Ready(id)).is_err() {
                    break;
                }
                let result = run_batch(&shared, batch);
                if output.send(WorkerEvent::Done(id, result)).is_err() {
                    break;
                }
            }
        }));
    }
    drop(events_tx);
    let mut all = Durations::default();
    let mut emitted = Durations::default();
    let mut gated = Durations::default();
    let mut round_wall = Durations::default();
    let mut frames = Vec::new();
    let mut min_view_entities = usize::MAX;
    let mut max_view_entities = 0;
    for round in 0..ROUNDS {
        assert!(
            wall.elapsed() < PROFILE_WALL,
            "bounded profile wall exceeded; files retained"
        );
        let batches = round_inputs(owners, round, producers);
        for input in batches.iter().flatten() {
            min_view_entities = min_view_entities.min(input.entities.len());
            max_view_entities = max_view_entities.max(input.entities.len());
        }
        // Explicit profile isolation: all previous calls have finished. Never
        // change production clock, config, seen ages, event queues or map data.
        if let Some(stream) = hub.state.lock().unwrap().maps.get_mut(PROFILE_MAP) {
            stream.last_emit_ms = 0;
        }
        let start: StartGate = Arc::new((Mutex::new(false), Condvar::new()));
        for (tx, inputs) in commands.iter().zip(batches) {
            tx.send(Batch {
                inputs,
                start: start.clone(),
            })
            .unwrap();
        }
        let mut ready = vec![false; producers];
        for _ in 0..producers {
            match events_rx
                .recv_timeout(CHANNEL_WAIT)
                .expect("bounded producer readiness")
            {
                WorkerEvent::Ready(id) => {
                    assert!(!ready[id]);
                    ready[id] = true;
                }
                WorkerEvent::Done(_, _) => panic!("unexpected done before start"),
            }
        }
        let round_start = Instant::now();
        *start.0.lock().unwrap() = true;
        start.1.notify_all();
        let mut done = vec![false; producers];
        for _ in 0..producers {
            match events_rx
                .recv_timeout(CHANNEL_WAIT)
                .expect("bounded producer completion")
            {
                WorkerEvent::Done(id, result) => {
                    assert!(!done[id]);
                    done[id] = true;
                    let result = result.expect("real publish must succeed");
                    all.0.extend(result.all);
                    emitted.0.extend(result.emitted);
                    gated.0.extend(result.gated);
                    frames.extend(result.frames);
                }
                WorkerEvent::Ready(_) => panic!("unexpected extra readiness"),
            }
        }
        round_wall.add(nanos(round_start));
        assert!(
            frames.len() <= MAX_EMITS_PER_TRIAL,
            "emit bound reached; no truncation"
        );
    }
    drop(commands);
    for worker in workers {
        worker.join().expect("profile producer did not panic");
    }
    assert_eq!(all.0.len(), owners * ROUNDS);
    assert_eq!(all.0.len(), emitted.0.len() + gated.0.len());
    let mut encode = EncodeProfile::default();
    let evidence = verify_frames(&hub, &mut frames, owners, &mut encode);
    json!({"trial":number,"owners":owners,"producers":producers,"calls":all.0.len(),
        "inputViewportEntities":{"min":min_view_entities,"max":max_view_entities},
        "publishOuterCall":all.summary(),"emittingPublishOuterCall":emitted.summary(),
        "gatedPublishOuterCall":gated.summary(),"roundWallIncludingWakeAndCollection":round_wall.summary(),
        "encodeComponentOnly":{"oldOwnedJson":encode.owned.summary(),"borrowedSerialize":encode.borrowed.summary(),
            "exactPayloadBytesAndBothDigestsMatched":true,"includesPublishMutexOrRecording":false},
        "evidence":evidence})
}

#[test]
#[ignore = "isolated Release full-publish profile; real recording to fresh C: dirs, not a capacity test"]
fn spectator_full_publish_recording_profile() {
    let root = new_output_root();
    let wall = Instant::now();
    let mut total_recorded_bytes = 0;
    for owners in [25, 50, 100] {
        for producers in [1, 4] {
            for number in 0..TRIALS {
                let result = trial(&root, owners, producers, number, wall);
                total_recorded_bytes += result["evidence"]["recordedBytes"].as_u64().unwrap();
                assert!(
                    total_recorded_bytes <= MAX_RECORD_BYTES_TOTAL,
                    "total recording bound reached; files retained"
                );
                println!(
                    "{}",
                    json!({"schema":"mir2.spectator.full-publish-profile.v1",
                    "capacityClaimAllowed":false,"componentOnly":true,"sourceRecordSha256":SOURCE_RECORD_SHA256,
                    "sourceShape":{"recordBytesWithNewline":29007,"entities":26,"terrain":0,"decor":0,"drops":0,"transfers":19},
                    "fixtureIdentity":"generated-anonymous-stable-global-ids","captureIntervalMs":250,
                    "recordingEnabled":true,"testOnlyGateResetBeforeEachRound":true,
                    "realClockAndAdditionalNaturalEmitsRetained":true,"roundsPerOwner":ROUNDS,
                    "noGameplayOrCCUClaim":true,"outputRoot":root,"elapsedMs":wall.elapsed().as_millis(),
                    "totalRecordedBytesSoFar":total_recorded_bytes,"result":result})
                );
                assert!(
                    wall.elapsed() < PROFILE_WALL,
                    "bounded profile wall exceeded; files retained"
                );
            }
        }
    }
}
