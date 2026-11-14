//! Bounded, ignored component profile; this file is not a production module.
//!
//! The caller must mount this module *inside* `spectator` with a cfg(test) path
//! hook. It calls the real merge/prune/event helpers. Only the publish control
//! sequence (pending caps, virtual capture gate, encoding and ring insertion)
//! is mirrored here; it does not call SpectatorHub::publish or measure its lock.
//! No environment, clock, service, network, database or recording is changed.
//!
//! A/B is test-only: A clones the entire previous Value; B projects only the
//! previous fields read by world_events. Current/merged/emitted worlds stay full.
//! Comparisons, fixture construction and fixture clones are outside leaf timers.
//! This is not a concurrent benchmark, a Dog map profile or a capacity result.

use super::{
    hex_digest, merge_worlds, prune_stale_objects, record_seen_ids, recording_id, world_events,
    MapStream, SpectatorEvent, SpectatorFrame, DEFAULT_CAPTURE_INTERVAL_MS, DEFAULT_ENTITY_STALE_MS,
    DEFAULT_RING_FRAMES, MAX_EVENTS_PER_FRAME, MAX_FRAME_BYTES, SPECTATOR_SCHEMA,
};
use mir2_game_data::{SceneView, TerrainKind, TerrainPatchTemplate};
use mir2_protocol::{MirClass, MirDirection, MirGender, Point};
use mir2_simulation::{WorldEntityDisposition, WorldEntityKind, WorldEntitySnapshot};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::time::Instant;

const CALLS_PER_OWNER: usize = 32;
const TRIALS: usize = 3;
const VIEWPORT_ENTITIES: usize = 8;
const VIRTUAL_START_MS: u64 = 25_000;
const PROFILE_MAP: &str = "SyntheticSpectatorProfile";

#[derive(Clone, Copy)]
enum PreviousCapture {
    Full,
    Projected,
}

impl PreviousCapture {
    fn name(self) -> &'static str {
        match self {
            Self::Full => "full-previous-value-clone",
            Self::Projected => "previous-event-fields-value-projection",
        }
    }
}

#[derive(Default)]
struct Samples(Vec<u64>);

impl Samples {
    fn elapsed(&mut self, started: Instant) -> u64 {
        let ns = u64::try_from(started.elapsed().as_nanos()).expect("bounded profile duration");
        self.0.push(ns);
        ns
    }

    fn summary(&self) -> Value {
        if self.0.is_empty() {
            return json!({"count": 0, "totalNs": 0, "p95Ns": null, "maxNs": null});
        }
        let mut sorted = self.0.clone();
        sorted.sort_unstable();
        let rank = (sorted.len() * 95).div_ceil(100) - 1;
        json!({
            "count": sorted.len(),
            "totalNs": sorted.iter().copied().sum::<u64>(),
            "p95Ns": sorted[rank],
            "maxNs": sorted.last().copied()
        })
    }
}

#[derive(Default)]
struct Profile {
    record_seen: Samples,
    previous_capture: Samples,
    merge_prune: Samples,
    events: Samples,
    pending_caps: Samples,
    emit_clone: Samples,
    encode: Samples,
    digest: Samples,
    ring_clone: Samples,
    previous_drop: Samples,
    capture_events_drop: Samples,
    updates: usize,
    previous_full_union: usize,
    emits: usize,
    non_emits: usize,
    generated_events: usize,
    event_kinds: BTreeMap<String, usize>,
    max_union: usize,
    max_pending_before_cap: usize,
    pending_drained: usize,
    emitted_events: usize,
    max_emit_retained_prefix: usize,
    max_frame_bytes: usize,
    max_ring_frames: usize,
}

impl Profile {
    fn summary(&self, mode: PreviousCapture) -> Value {
        json!({
            "mode": mode.name(),
            "stages": {
                "recordSeenIds": self.record_seen.summary(),
                "previousCapture": self.previous_capture.summary(),
                "mergeAndPrune": self.merge_prune.summary(),
                "worldEvents": self.events.summary(),
                "pendingExtendAndCap": self.pending_caps.summary(),
                "emitWorldClone": self.emit_clone.summary(),
                "encodeWorldEvents": self.encode.summary(),
                "digest": self.digest.summary(),
                "ringFrameClone": self.ring_clone.summary(),
                "previousDrop": self.previous_drop.summary(),
                "capturePlusEventsPlusDrop": self.capture_events_drop.summary()
            },
            "counts": {
                "updates": self.updates, "previousAtFullUnion": self.previous_full_union,
                "emits": self.emits, "nonEmits": self.non_emits,
                "generatedEvents": self.generated_events, "eventKinds": &self.event_kinds,
                "maxEntityUnion": self.max_union, "groundDropUnion": 0,
                "maxPendingBeforeCap": self.max_pending_before_cap,
                "pendingCapDrained": self.pending_drained,
                "emittedEvents": self.emitted_events,
                "maxOldPendingPrefixRetainedOnEmit": self.max_emit_retained_prefix,
                "maxEncodedPayloadBytes": self.max_frame_bytes,
                "maxRingFrames": self.max_ring_frames
            }
        })
    }
}

// Preserve array order, duplicate IDs, absent keys and all Value types. This is
// deliberately a Value prototype, not a second implementation of world_events.
fn projected_previous(previous: &Value) -> Value {
    let Value::Object(object) = previous else {
        return previous.clone();
    };
    let mut projected = Map::new();
    for (field, keys) in [
        ("entities", &["objectId", "name", "x", "y", "hp", "dead"][..]),
        ("groundDrops", &["objectId", "name"][..]),
    ] {
        let Some(value) = object.get(field) else {
            continue;
        };
        let projected_field = match value {
            Value::Array(rows) => Value::Array(
                rows.iter()
                    .map(|row| match row {
                        Value::Object(row) => Value::Object(
                            keys.iter()
                                .filter_map(|key| {
                                    row.get(*key).map(|value| ((*key).to_string(), value.clone()))
                                })
                                .collect(),
                        ),
                        other => other.clone(),
                    })
                    .collect(),
            ),
            other => other.clone(),
        };
        projected.insert(field.to_string(), projected_field);
    }
    Value::Object(projected)
}

fn assert_events_equal(left: &[SpectatorEvent], right: &[SpectatorEvent]) {
    assert_eq!(left.len(), right.len(), "event sequence length");
    for (left, right) in left.iter().zip(right) {
        assert_eq!(left.kind, right.kind);
        assert_eq!(left.at_ms, right.at_ms);
        assert_eq!(left.object_id, right.object_id);
        assert_eq!(left.name, right.name);
        assert_eq!(left.payload, right.payload);
    }
}

fn check_projection_edge_cases() {
    let previous = [
        Value::Null,
        json!([1, {"entities": []}]),
        json!({"entities": "not-an-array", "groundDrops": null}),
        json!({}),
        json!({
            "entities": [
                {"objectId": 7, "name": "first", "x": 1, "y": 2, "hp": 9,
                 "dead": false, "unread": [1, 2]},
                {"objectId": 7, "name": "last", "x": "1", "y": null,
                 "hp": 9.5, "dead": "false"},
                {"objectId": 8, "name": null}, {"objectId": -1}, 5, null
            ],
            "groundDrops": [{"objectId": 50, "name": "first", "quantity": 4},
                            {"objectId": 50, "name": 7}, {"name": "missing-id"}]
        }),
    ];
    let current = [
        json!({}),
        json!({"entities": null, "groundDrops": 3}),
        json!({
            "entities": [{"objectId": 7, "name": "now", "kind": "player",
                          "x": 1, "y": 2, "hp": 8, "dead": true},
                         {"objectId": 9, "x": 4, "y": 5}],
            "groundDrops": [{"objectId": 51, "name": "new", "quantity": 2}]
        }),
    ];
    for previous in &previous {
        let projected = projected_previous(previous);
        for current in &current {
            assert_events_equal(
                &world_events(Some(previous), current, 42),
                &world_events(Some(&projected), current, 42),
            );
        }
    }
    let projected = projected_previous(&previous[4]);
    assert_eq!(projected["entities"].as_array().unwrap().len(), 6);
    assert_eq!(projected["entities"][0]["objectId"], 7);
    assert_eq!(projected["entities"][1]["objectId"], 7);
    assert_eq!(projected["entities"][1]["x"], "1");
    assert!(projected["entities"][2].get("x").is_none());
    assert!(projected["entities"][0].get("unread").is_none());
    assert_eq!(projected["groundDrops"][1]["name"], 7);
}

struct Prepared {
    previous: Option<Value>,
    merged: Value,
    events: Vec<SpectatorEvent>,
    capture_ns: u64,
    events_ns: u64,
}

fn prepare(
    stream: &mut MapStream,
    incoming: Value,
    now: u64,
    owners: usize,
    mode: PreviousCapture,
    profile: &mut Profile,
) -> Prepared {
    profile.updates += 1;
    if stream.latest_world.as_ref().is_some_and(|world| {
        world.get("entities").and_then(Value::as_array).map(Vec::len) == Some(owners)
    }) {
        profile.previous_full_union += 1;
    }
    let started = Instant::now();
    record_seen_ids(&incoming, "entities", now, &mut stream.entity_last_seen_ms);
    record_seen_ids(&incoming, "groundDrops", now, &mut stream.drop_last_seen_ms);
    profile.record_seen.elapsed(started);

    let started = Instant::now();
    let previous = match mode {
        PreviousCapture::Full => stream.latest_world.clone(),
        PreviousCapture::Projected => stream.latest_world.as_ref().map(projected_previous),
    };
    let capture_ns = profile.previous_capture.elapsed(started);
    let started = Instant::now();
    let mut merged = merge_worlds(stream.latest_world.take(), incoming);
    prune_stale_objects(
        &mut merged, "entities", now, DEFAULT_ENTITY_STALE_MS,
        &mut stream.entity_last_seen_ms,
    );
    prune_stale_objects(
        &mut merged, "groundDrops", now, DEFAULT_ENTITY_STALE_MS,
        &mut stream.drop_last_seen_ms,
    );
    profile.merge_prune.elapsed(started);
    let started = Instant::now();
    let events = world_events(previous.as_ref(), &merged, now);
    let events_ns = profile.events.elapsed(started);
    profile.generated_events += events.len();
    for event in &events {
        *profile.event_kinds.entry(event.kind.clone()).or_default() += 1;
    }
    profile.max_union = profile.max_union.max(merged["entities"].as_array().unwrap().len());
    Prepared { previous, merged, events, capture_ns, events_ns }
}

struct Emission {
    encoded: Vec<u8>,
    frame: SpectatorFrame,
}

fn finish(
    stream: &mut MapStream,
    prepared: Prepared,
    now: u64,
    profile: &mut Profile,
) -> Option<Emission> {
    let Prepared { previous, merged, events, capture_ns, events_ns } = prepared;
    let started = Instant::now();
    stream.pending_events.extend(events);
    profile.max_pending_before_cap = profile.max_pending_before_cap.max(stream.pending_events.len());
    if stream.pending_events.len() > MAX_EVENTS_PER_FRAME * 4 {
        let remove = stream.pending_events.len() - MAX_EVENTS_PER_FRAME * 4;
        profile.pending_drained += remove;
        stream.pending_events.drain(..remove);
    }
    profile.pending_caps.elapsed(started);
    stream.latest_world = Some(merged);
    stream.latest_map_title = "Public synthetic component fixture".to_string();

    // Component-only mirror of production gate/caps. No sleep or wall-clock gate.
    let emission = if now.saturating_sub(stream.last_emit_ms) < DEFAULT_CAPTURE_INTERVAL_MS {
        profile.non_emits += 1;
        None
    } else {
        profile.emits += 1;
        stream.last_emit_ms = now;
        stream.sequence = stream.sequence.saturating_add(1);
        let recording_id = recording_id(PROFILE_MAP, now);
        let started = Instant::now();
        let world = stream.latest_world.clone().expect("merged world");
        profile.emit_clone.elapsed(started);
        let events = if stream.pending_events.len() > MAX_EVENTS_PER_FRAME {
            // Preserve production split_off semantics: the old prefix remains.
            stream.pending_events.split_off(stream.pending_events.len() - MAX_EVENTS_PER_FRAME)
        } else {
            std::mem::take(&mut stream.pending_events)
        };
        profile.max_emit_retained_prefix = profile.max_emit_retained_prefix.max(stream.pending_events.len());
        profile.emitted_events += events.len();
        let started = Instant::now();
        let encoded = serde_json::to_vec(&json!({"world": &world, "events": &events}))
            .expect("synthetic public frame encoding");
        profile.encode.elapsed(started);
        assert!(encoded.len() <= MAX_FRAME_BYTES, "retain production payload bound");
        profile.max_frame_bytes = profile.max_frame_bytes.max(encoded.len());
        let started = Instant::now();
        let digest = hex_digest(Sha256::digest(&encoded).as_slice());
        profile.digest.elapsed(started);
        let frame = SpectatorFrame {
            schema: SPECTATOR_SCHEMA.to_string(), recording_id, sequence: stream.sequence,
            captured_at_ms: now, map_file_name: PROFILE_MAP.to_string(),
            map_title: stream.latest_map_title.clone(), digest, events, world,
        };
        let started = Instant::now();
        stream.frames.push_back(frame.clone());
        profile.ring_clone.elapsed(started);
        while stream.frames.len() > DEFAULT_RING_FRAMES {
            stream.frames.pop_front();
        }
        profile.max_ring_frames = profile.max_ring_frames.max(stream.frames.len());
        Some(Emission { encoded, frame })
    };
    // Production keeps previous_world alive through both emit/non-emit paths.
    let started = Instant::now();
    drop(previous);
    let drop_ns = profile.previous_drop.elapsed(started);
    profile.capture_events_drop.0.push(capture_ns + events_ns + drop_ns);
    emission
}

fn public_entity(index: usize) -> Value {
    let entity = WorldEntitySnapshot {
        object_id: 10_000 + index as u32, kind: WorldEntityKind::Player,
        name: format!("SyntheticPlayer{index:03}"), owner_name: None, ai: None,
        x: (index % 16) as i32, y: (index / 16) as i32, direction: MirDirection::Down,
        class: Some(MirClass::Warrior), gender: Some(MirGender::Male), level: Some(40),
        hp: Some(100), max_hp: Some(100), light: 0, wing_effect: Some(0),
        name_colour_argb: -1, dead: false, riding_mount: Some(false), can_mount_attack: None,
        has_class_weapon: None, dazed: None, fishing: Some(false), transform_type: Some(0),
        disposition: WorldEntityDisposition::Friendly, sprite: None, quest_ids: Vec::new(),
        quest_icon: None,
    };
    serde_json::to_value(entity).expect("actual public entity DTO")
}

fn public_world(tiles: usize) -> Value {
    assert!(matches!(tiles, 0 | 1024));
    let scene = (tiles != 0).then_some(SceneView {
        center: Point { x: 16, y: 16 }, width: 32, height: 32,
    });
    let terrain: Vec<_> = (0..tiles).map(|tile| TerrainPatchTemplate {
        x: (tile % 32) as i32, y: (tile / 32) as i32, width: 1, height: 1,
        kind: match tile % 5 {
            0 => TerrainKind::Grass, 1 => TerrainKind::Dirt, 2 => TerrainKind::Road,
            3 => TerrainKind::Water, _ => TerrainKind::Stone,
        },
    }).collect();
    let world = json!({
        "tick": 0, "mapTitle": "Public synthetic component fixture", "mapFileName": PROFILE_MAP,
        "inSafeZone": true, "lightSetting": 0,
        "playerObjectId": null, "playerHp": null, "playerMaxHp": null,
        "playerMp": null, "playerMaxMp": null, "playerExperience": 0, "playerMaxExperience": 1,
        "gold": 0, "credit": 0, "cityCurrencies": {}, "currentWeight": 0, "maxWeight": 0,
        "freeBagSlots": 0, "maxBagSlots": 0, "storageSize": 0, "hasExpandedStorage": false,
        "hasStoragePassword": false, "requireStoragePassword": false,
        "sceneView": scene, "terrainPatches": terrain, "decorObjects": [],
        "entities": [], "groundDrops": [], "beltItems": [], "inventoryItems": [],
        "storageItems": [], "equipmentItems": [], "questLog": [], "activeNpcDialog": null,
        "npcScriptDiagnostics": [], "knownSkills": [], "activeBuffs": [],
        "stage5Systems": {}, "mapTransfers": [], "interactionHints": []
    });
    assert_eq!(world["terrainPatches"].as_array().unwrap().len(), tiles);
    if tiles != 0 {
        assert_eq!(world["terrainPatches"][1023]["x"], 31);
        assert_eq!(world["terrainPatches"][1023]["y"], 31);
        assert_eq!(world["terrainPatches"][0]["width"], 1);
    }
    world
}

#[test]
#[ignore = "bounded component-only A/B profile; run Release explicitly without concurrent game/build"]
fn spectator_previous_capture_component_profile() {
    check_projection_edge_cases();
    for owners in [25usize, 50, 100] {
        for tiles in [0usize, 1024] {
            let template = public_world(tiles);
            let entity_templates: Vec<_> = (0..owners).map(public_entity).collect();
            let geometry_bytes = serde_json::to_vec(&template["terrainPatches"]).unwrap().len();
            let mut baseline = Profile::default();
            let mut candidate = Profile::default();
            let mut parity_updates = 0usize;
            let mut parity_emits = 0usize;
            let mut trial_counts = Vec::new();
            let component_started = Instant::now();
            for trial in 0..TRIALS {
                let mut old = MapStream::default();
                let mut projected = MapStream::default();
                let mut versions = vec![false; owners];
                let mut trial_emits = 0usize;
                for update in 0..owners * CALLS_PER_OWNER {
                    let owner = update % owners;
                    versions[owner] = !versions[owner];
                    // Both owned inputs are constructed before any leaf timer.
                    let mut incoming = template.clone();
                    incoming["tick"] = json!(update);
                    incoming["entities"] = Value::Array((0..VIEWPORT_ENTITIES).map(|offset| {
                        let id = (owner + offset) % owners;
                        let mut entity = entity_templates[id].clone();
                        entity["x"] = json!((id % 16) as i32 + i32::from(versions[id]));
                        entity
                    }).collect());
                    let second_input = incoming.clone();
                    let now = VIRTUAL_START_MS
                        + (update as u64 * DEFAULT_CAPTURE_INTERVAL_MS / owners as u64);
                    // Alternate A/B execution order, not just their output labels.
                    let (a, b) = if (trial + update) % 2 == 0 {
                        let a = prepare(&mut old, incoming, now, owners, PreviousCapture::Full, &mut baseline);
                        let b = prepare(&mut projected, second_input, now, owners, PreviousCapture::Projected, &mut candidate);
                        (a, b)
                    } else {
                        let b = prepare(&mut projected, incoming, now, owners, PreviousCapture::Projected, &mut candidate);
                        let a = prepare(&mut old, second_input, now, owners, PreviousCapture::Full, &mut baseline);
                        (a, b)
                    };
                    assert_events_equal(&a.events, &b.events);
                    assert_eq!(a.merged, b.merged, "same full current world");
                    parity_updates += 1;
                    let (a, b) = if (trial + update) % 2 == 0 {
                        let a = finish(&mut old, a, now, &mut baseline);
                        let b = finish(&mut projected, b, now, &mut candidate);
                        (a, b)
                    } else {
                        let b = finish(&mut projected, b, now, &mut candidate);
                        let a = finish(&mut old, a, now, &mut baseline);
                        (a, b)
                    };
                    match (a, b) {
                        (Some(a), Some(b)) => {
                            assert_eq!(a.encoded, b.encoded, "exact emitted payload bytes");
                            assert_eq!(a.frame.digest, b.frame.digest, "exact emitted digest");
                            assert_eq!(a.frame.sequence, b.frame.sequence);
                            assert_eq!(a.frame.captured_at_ms, b.frame.captured_at_ms);
                            parity_emits += 1;
                            trial_emits += 1;
                        }
                        (None, None) => {}
                        _ => panic!("A/B emission gate differs"),
                    }
                }
                assert_eq!(old.entity_last_seen_ms.len(), owners);
                assert_eq!(projected.entity_last_seen_ms.len(), owners);
                assert_eq!(trial_emits, CALLS_PER_OWNER);
                assert_eq!(old.frames.len(), CALLS_PER_OWNER);
                assert!(old.frames.len() <= DEFAULT_RING_FRAMES);
                assert_events_equal(&old.pending_events, &projected.pending_events);
                trial_counts.push(json!({
                    "trial": trial, "updatesPerVariant": owners * CALLS_PER_OWNER,
                    "emitsPerVariant": trial_emits, "finalEntityUnion": owners,
                    "pendingAtEnd": old.pending_events.len()
                }));
            }
            assert_eq!(parity_updates, owners * CALLS_PER_OWNER * TRIALS);
            assert_eq!(parity_emits, CALLS_PER_OWNER * TRIALS);
            println!("{}", json!({
                "schema": "mir2.spectator.previous-capture-component-profile.v1",
                "componentOnly": true, "capacityClaimAllowed": false,
                "owners": owners, "viewportEntities": VIEWPORT_ENTITIES,
                "geometry": {"terrainPatchCount": tiles, "serializedPatchesBytes": geometry_bytes,
                             "shape": "actual SceneView and TerrainPatchTemplate DTO; synthetic 32x32 1x1 cells"},
                "callsPerOwnerPerTrial": CALLS_PER_OWNER, "trials": TRIALS,
                "virtualCaptureIntervalMs": DEFAULT_CAPTURE_INTERVAL_MS,
                "virtualTimeRule": "25000 + floor(updateIndex * 250 / owners)",
                "maxVirtualSpanMs": CALLS_PER_OWNER as u64 * DEFAULT_CAPTURE_INTERVAL_MS,
                "limits": {"pending": MAX_EVENTS_PER_FRAME * 4, "eventsPerFrame": MAX_EVENTS_PER_FRAME,
                           "frameBytes": MAX_FRAME_BYTES, "ringFrames": DEFAULT_RING_FRAMES,
                           "entityStaleMs": DEFAULT_ENTITY_STALE_MS},
                "parity": {"allUpdateEventSequencesEqual": true, "updatesChecked": parity_updates,
                           "allEmissionBytesAndDigestsEqual": true, "emissionsChecked": parity_emits},
                "fixture": {"onlyOwnerXToggles": true, "globalIdsStableAcrossViewports": true,
                            "groundDropsEmpty": true, "spriteNone": true,
                            "initialUnionGrowthIncluded": true, "ttlNotReached": true},
                "timing": {"unit": "ns", "p95": "nearest-rank over all leaf calls across three trials",
                           "clock": "std::time::Instant", "timerOverheadSubtracted": false,
                           "abOrder": "alternates per update and trial",
                           "inputConstructionAndAssertionsExcludedFromLeafTimers": true,
                           "componentWallNsIncludingInputsAndAssertions":
                                u64::try_from(component_started.elapsed().as_nanos()).unwrap()},
                "excluded": ["Hub mutex wait/held", "snapshot/sanitize", "Tokio scheduling",
                             "recording/file IO", "network/DB", "real map/account data",
                             "process CPU/RSS/CCU", "ring eviction and nonempty drop workload"],
                "controlSequence": "component-only mirror of production gate/caps/ring; real merge/prune/events",
                "baseline": baseline.summary(PreviousCapture::Full),
                "candidatePrototype": candidate.summary(PreviousCapture::Projected),
                "trialCounts": trial_counts
            }));
        }
    }
}
