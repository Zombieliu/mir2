//! Packet-first Android entity projection over the last authoritative snapshot.
//!
//! This cache does not predict movement or invent actors. It applies only
//! server packets to the renderer-neutral entity model and, when available,
//! to the exact native entity-render state built from the same snapshot.

use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{LazyLock, Mutex},
    time::Instant,
};

const MAX_PACKET_BYTES: usize = 16 * 1024;
const MAX_RENDER_BYTES: usize = 64 * 1024 * 1024;
const MAX_ENTITIES: usize = 8192;
// Nineteen bounded action families (including derived Pushed, DashL and DashR)
// across the eight Crystal directions.
const MAX_ACTION_POSES: usize = 19 * 8;
const MAX_DAMAGE_EVENTS: usize = 48;
const MAX_GROUND_ITEM_FRAMES: usize = 6_000;
const CELL_WIDTH: f64 = 48.0;
const CELL_HEIGHT: f64 = 32.0;
const POISON_SLOW: u16 = 4;

#[derive(Default)]
struct LiveEntityCache {
    request_id: u64,
    models: Option<Value>,
    render_request_id: u64,
    render: Option<Value>,
    hidden: HashMap<u32, HiddenEntity>,
    tombstones: HashSet<u32>,
    actions: HashMap<u32, ActiveAction>,
    health_generation: u64,
    health_revision: u64,
    damage_sequence: u64,
    damage_events: VecDeque<LiveDamageEvent>,
}

#[derive(Default)]
struct HiddenEntity {
    model: Option<Value>,
    render: Option<Value>,
}

#[derive(Debug)]
struct ActiveAction {
    action: String,
    started_ms: u64,
    interval_ms: u64,
    slowed: bool,
    frame_count: usize,
    last_frame: usize,
    direction: String,
    completion: ActionCompletion,
}

#[derive(Clone, Copy, Debug)]
enum ActionCompletion {
    Standing,
    Pose(&'static str),
    Hold,
}

static LIVE_ENTITIES: LazyLock<Mutex<LiveEntityCache>> =
    LazyLock::new(|| Mutex::new(LiveEntityCache::default()));
static LIVE_ENTITY_CLOCK: LazyLock<Instant> = LazyLock::new(Instant::now);
#[cfg(test)]
static LIVE_ENTITY_TEST_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LiveEntityPacketOutcome {
    Applied {
        models: String,
        render: Option<String>,
        presentation_event: Option<String>,
    },
    Ignored,
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LiveDamageEvent {
    pub(crate) sequence: u64,
    pub(crate) object_id: u32,
    pub(crate) damage: i32,
    pub(crate) damage_type: u8,
}

pub(crate) fn clear() {
    *LIVE_ENTITIES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = LiveEntityCache::default();
}

#[cfg(target_os = "android")]
pub(crate) fn clear_with_presentation_reset() {
    clear();
    crate::shared_shell::enqueue_presentation_event(
        json!({"type": "clear", "atMs": live_now_ms()}).to_string(),
    );
}

pub(crate) fn drain_damage_events() -> Vec<LiveDamageEvent> {
    LIVE_ENTITIES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .damage_events
        .drain(..)
        .collect()
}

pub(crate) fn install_models(json_text: &str, request_id: u64) -> bool {
    if request_id == 0 {
        return false;
    }
    let Ok(mut value) = serde_json::from_str::<Value>(json_text) else {
        return false;
    };
    if !valid_models(&value) {
        return false;
    }
    let mut cache = LIVE_ENTITIES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if cache.request_id != request_id {
        cache.actions.clear();
        cache.health_generation = cache.health_generation.wrapping_add(1).max(1);
        cache.health_revision = 0;
    }
    let LiveEntityCache {
        hidden, tombstones, ..
    } = &mut *cache;
    retain_visible_models(&mut value, hidden, tombstones);
    cache.request_id = request_id;
    cache.models = Some(value);
    if cache.render_request_id != request_id {
        cache.render_request_id = 0;
        cache.render = None;
    }
    true
}

pub(crate) fn install_render(json_text: &str, directions_text: &str) -> bool {
    if json_text.is_empty()
        || directions_text.is_empty()
        || json_text
            .len()
            .checked_add(directions_text.len())
            .is_none_or(|bytes| bytes > MAX_RENDER_BYTES)
    {
        return false;
    }
    let Ok(mut value) = serde_json::from_str::<Value>(json_text) else {
        return false;
    };
    let Some(request_id) = value
        .get("_nativeWorldRequest")
        .and_then(Value::as_u64)
        .filter(|request_id| *request_id != 0)
    else {
        return false;
    };
    let Ok(directions) = serde_json::from_str::<Value>(directions_text) else {
        return false;
    };
    if directions
        .get("_nativeWorldRequest")
        .and_then(Value::as_u64)
        != Some(request_id)
        || !merge_direction_layers(&mut value, &directions)
    {
        return false;
    }
    if !valid_render(&value) {
        return false;
    }
    let mut cache = LIVE_ENTITIES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let LiveEntityCache {
        hidden, tombstones, ..
    } = &mut *cache;
    retain_visible_render(&mut value, hidden, tombstones);
    if cache.request_id == request_id {
        let Some(models) = cache.models.as_ref() else {
            return false;
        };
        if !align_render_to_models(&mut value, models) {
            return false;
        }
    }
    cache.render_request_id = request_id;
    cache.render = Some(value);
    true
}

fn retain_visible_models(
    value: &mut Value,
    hidden: &mut HashMap<u32, HiddenEntity>,
    tombstones: &HashSet<u32>,
) {
    let Some(entities) = value.get_mut("entities").and_then(Value::as_array_mut) else {
        return;
    };
    entities.retain(|entity| {
        let Some(object_id) = entity_id(entity) else {
            return true;
        };
        if let Some(entry) = hidden.get_mut(&object_id) {
            entry.model = Some(entity.clone());
            return false;
        }
        !tombstones.contains(&object_id)
    });
    if let Some(ground_drops) = value.get_mut("groundDrops").and_then(Value::as_array_mut) {
        ground_drops.retain(|drop| {
            entity_id(drop).is_none_or(|object_id| !tombstones.contains(&object_id))
        });
    }
}

fn retain_visible_render(
    value: &mut Value,
    hidden: &mut HashMap<u32, HiddenEntity>,
    tombstones: &HashSet<u32>,
) {
    let Some(entities) = value.get_mut("entities").and_then(Value::as_array_mut) else {
        return;
    };
    entities.retain(|entity| {
        let Some(object_id) = entity_id(entity) else {
            return true;
        };
        if let Some(entry) = hidden.get_mut(&object_id) {
            entry.render = Some(entity.clone());
            return false;
        }
        !tombstones.contains(&object_id)
    });
}

fn merge_direction_layers(render: &mut Value, sidecar: &Value) -> bool {
    let Some(render_entities) = render.get_mut("entities").and_then(Value::as_array_mut) else {
        return false;
    };
    let Some(entries) = sidecar.get("entities").and_then(Value::as_array) else {
        return false;
    };
    if entries.len() > render_entities.len() || entries.len() > MAX_ENTITIES {
        return false;
    }
    let ground_item_frames = match sidecar.get("groundItemFrames") {
        None => json!({}),
        Some(value) if valid_ground_item_frames(value) => value.clone(),
        Some(_) => return false,
    };
    let mut ids = HashSet::new();
    for entry in entries {
        let Some(object_id) = entity_id(entry) else {
            return false;
        };
        let Some(directions) = entry
            .get("directionLayers")
            .filter(|value| value.is_object())
        else {
            return false;
        };
        let Some(prototype) = entry.get("prototype").filter(|value| {
            value.is_object()
                && serde_json::to_string(value).is_ok_and(|encoded| encoded.len() <= 8 * 1024)
        }) else {
            return false;
        };
        let actions = entry.get("actionLayers");
        if actions.is_some_and(|value| !valid_action_layers(value)) {
            return false;
        }
        let Some(target) = render_entities
            .iter_mut()
            .find(|entity| entity_id(entity) == Some(object_id))
        else {
            return false;
        };
        if !ids.insert(object_id) || target.get("directionLayers").is_some() {
            return false;
        }
        target["directionLayers"] = directions.clone();
        target["prototype"] = prototype.clone();
        if let Some(actions) = actions {
            target["actionLayers"] = actions.clone();
        }
    }
    render["_nativeGroundItemFrames"] = ground_item_frames;
    true
}

fn valid_ground_item_frames(value: &Value) -> bool {
    value.as_object().is_some_and(|frames| {
        frames.len() <= MAX_GROUND_ITEM_FRAMES
            && frames.iter().all(|(key, frame)| {
                key.parse::<u16>().is_ok()
                    && frame
                        .get("width")
                        .and_then(Value::as_u64)
                        .is_some_and(|width| (1..=256).contains(&width))
                    && frame
                        .get("height")
                        .and_then(Value::as_u64)
                        .is_some_and(|height| (1..=256).contains(&height))
            })
    })
}

fn valid_action_layers(value: &Value) -> bool {
    value.as_object().is_some_and(|actions| {
        actions.len() <= MAX_ACTION_POSES
            && actions.iter().all(|(key, action)| {
                !key.is_empty()
                    && key.len() <= 48
                    && action
                        .get("intervalMs")
                        .and_then(Value::as_u64)
                        .is_some_and(|interval| (1..=5_000).contains(&interval))
                    && action
                        .get("frames")
                        .and_then(Value::as_array)
                        .is_some_and(|frames| {
                            !frames.is_empty()
                                && frames.len() <= 64
                                && frames.iter().all(|frame| {
                                    frame.as_array().is_some_and(|layers| valid_layers(layers))
                                })
                        })
            })
    })
}

pub(crate) fn apply_packet(json_text: &str) -> LiveEntityPacketOutcome {
    apply_packet_at(json_text, live_now_ms())
}

fn live_now_ms() -> u64 {
    u64::try_from(LIVE_ENTITY_CLOCK.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn apply_packet_at(json_text: &str, now_ms: u64) -> LiveEntityPacketOutcome {
    if json_text.is_empty() || json_text.len() > MAX_PACKET_BYTES {
        return LiveEntityPacketOutcome::Rejected;
    }
    let Ok(envelope) = serde_json::from_str::<Value>(json_text) else {
        return LiveEntityPacketOutcome::Rejected;
    };
    if envelope.get("type").and_then(Value::as_str) != Some("packet") {
        return LiveEntityPacketOutcome::Rejected;
    }
    let Some(packet) = envelope.get("packet").and_then(Value::as_str) else {
        return LiveEntityPacketOutcome::Rejected;
    };
    let Some(payload) = envelope.get("payload").and_then(Value::as_object) else {
        return LiveEntityPacketOutcome::Rejected;
    };
    let body = payload
        .get("info")
        .and_then(Value::as_object)
        .unwrap_or(payload);
    let mut cache = LIVE_ENTITIES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let request_id = cache.request_id;
    let Some(mut models) = cache.models.take() else {
        return LiveEntityPacketOutcome::Ignored;
    };
    // Crystal ignores ObjectPushed for the local player; its own Pushed packet
    // has separate camera/input semantics and must not be inferred here.
    if matches!(packet, "ObjectPushed" | "ObjectDash" | "ObjectDashFail")
        && object_id(body).is_some_and(|object_id| self_object_id(&models) == Some(object_id))
    {
        cache.models = Some(models);
        return LiveEntityPacketOutcome::Ignored;
    }
    let mutation = match packet {
        "UserLocation" => location(body)
            .map(|position| EntityMutation::MoveSelf(position, direction(body).map(str::to_owned))),
        "ObjectBackStep" | "ObjectTurn" => {
            object_id(body)
                .zip(location(body))
                .map(|(object_id, position)| {
                    EntityMutation::Move(object_id, position, direction(body).map(str::to_owned))
                })
        }
        "ObjectWalk" | "ObjectRun" | "ObjectHarvest" | "ObjectHarvested" | "ObjectAttack"
        | "ObjectRangeAttack" | "ObjectStruck" | "ObjectDashAttack" | "ObjectMagic" => {
            object_id(body)
                .zip(location(body))
                .map(|(object_id, position)| EntityMutation::Action {
                    object_id,
                    position,
                    direction: action_direction(body, &models, object_id),
                    action: packet_action(packet, body, &models, object_id),
                    started_ms: now_ms,
                    life_state: None,
                })
        }
        "ObjectPushed" => object_id(body)
            .zip(location(body))
            .zip(mir_direction(body))
            .map(
                |((object_id, position), direction)| EntityMutation::Action {
                    object_id,
                    position,
                    direction: Some(direction.to_owned()),
                    action: "pushed".to_owned(),
                    started_ms: now_ms,
                    life_state: None,
                },
            ),
        "ObjectDash" => object_id(body)
            .zip(location(body))
            .zip(mir_direction(body))
            .map(
                |((object_id, position), direction)| EntityMutation::Action {
                    object_id,
                    position,
                    direction: Some(direction.to_owned()),
                    action: remote_dash_action(&cache.actions, object_id).to_owned(),
                    started_ms: now_ms,
                    life_state: None,
                },
            ),
        "ObjectDashFail" => object_id(body)
            .zip(location(body))
            .zip(mir_direction(body))
            .map(|((object_id, position), direction)| {
                EntityMutation::Move(object_id, position, Some(direction.to_owned()))
            }),
        "DamageIndicator" => object_id(body)
            .zip(damage(body))
            .zip(damage_type(body))
            .map(|((object_id, damage), damage_type)| {
                cache.damage_sequence = cache.damage_sequence.saturating_add(1);
                EntityMutation::Damage(LiveDamageEvent {
                    sequence: cache.damage_sequence,
                    object_id,
                    damage,
                    damage_type,
                })
            }),
        "ObjectHealth" => object_id(body).zip(percent(body)).zip(expire(body)).map(
            |((object_id, percent), expire)| {
                cache.health_revision = cache.health_revision.wrapping_add(1).max(1);
                EntityMutation::Health {
                    object_id,
                    percent,
                    expire,
                    generation: cache.health_generation,
                    revision: cache.health_revision,
                }
            },
        ),
        "ObjectName" => {
            object_id(body)
                .zip(bounded_text(body, "name", false))
                .map(|(object_id, name)| EntityMutation::Metadata {
                    object_id,
                    update: EntityMetadata::Name(name.expect("non-empty name")),
                })
        }
        "ObjectColourChanged" => object_id(body).zip(signed_i32(body, "nameColourArgb")).map(
            |(object_id, name_colour_argb)| EntityMutation::Metadata {
                object_id,
                update: EntityMetadata::NameColour(name_colour_argb),
            },
        ),
        "ObjectGuildNameChanged" => object_id(body)
            .zip(bounded_text(body, "guildName", true))
            .map(|(object_id, guild_name)| EntityMutation::Metadata {
                object_id,
                update: EntityMetadata::GuildName(guild_name),
            }),
        "ObjectPoisoned" => {
            object_id(body)
                .zip(unsigned_u16(body, "poison"))
                .map(|(object_id, poison)| EntityMutation::Poison {
                    object_id,
                    poison,
                    at_ms: now_ms,
                })
        }
        "ObjectLevelEffects" => object_id(body).zip(unsigned_u16(body, "levelEffects")).map(
            |(object_id, level_effects)| EntityMutation::LevelEffects {
                object_id,
                level_effects,
            },
        ),
        "ObjectHidden" => object_id(body)
            .zip(boolean(body, "hidden"))
            .map(|(object_id, hidden)| EntityMutation::Hidden { object_id, hidden }),
        "Death" => self_object_id(&models)
            .zip(location(body))
            .map(|(object_id, position)| EntityMutation::Action {
                object_id,
                position,
                direction: action_direction(body, &models, object_id),
                action: "die".to_owned(),
                started_ms: now_ms,
                life_state: Some(true),
            }),
        "ObjectDied" => object_id(body)
            .zip(location(body))
            .map(|(object_id, position)| EntityMutation::Action {
                object_id,
                position,
                direction: action_direction(body, &models, object_id),
                action: "die".to_owned(),
                started_ms: now_ms,
                life_state: Some(true),
            }),
        "Revived" => self_object_id(&models).and_then(|object_id| {
            model_pose(&models, object_id).map(|(position, direction)| EntityMutation::Action {
                object_id,
                position,
                direction,
                action: "revive".to_owned(),
                started_ms: now_ms,
                life_state: Some(false),
            })
        }),
        "ObjectRevived" => object_id(body).and_then(|object_id| {
            model_pose(&models, object_id).map(|(position, direction)| EntityMutation::Action {
                object_id,
                position,
                direction,
                action: "revive".to_owned(),
                started_ms: now_ms,
                life_state: Some(false),
            })
        }),
        "ObjectHide" | "ObjectTeleportOut" => object_id(body).map(EntityMutation::Hide),
        "ObjectShow" | "ObjectTeleportIn" => object_id(body).map(EntityMutation::Show),
        "ObjectRemove" => object_id(body).map(EntityMutation::Remove),
        "ObjectItem" => ground_drop(body, false).map(EntityMutation::GroundDrop),
        "ObjectGold" => ground_drop(body, true).map(EntityMutation::GroundDrop),
        "ObjectPlayer" | "ObjectHero" => spawn(body, "player").map(EntityMutation::Spawn),
        "ObjectMonster" | "NewMonsterInfo" => spawn(body, "monster").map(EntityMutation::Spawn),
        "ObjectNpc" | "NewNpcInfo" => spawn(body, "npc").map(EntityMutation::Spawn),
        _ => {
            cache.models = Some(models);
            return LiveEntityPacketOutcome::Ignored;
        }
    };
    let Some(mutation) = mutation else {
        cache.models = Some(models);
        return LiveEntityPacketOutcome::Rejected;
    };
    let presentation_event = presentation_event(packet, &models, &mutation, now_ms);
    if matches!(
        mutation,
        EntityMutation::Hide(_) | EntityMutation::Remove(_)
    ) && mutation_object_id(&mutation).is_some_and(|object_id| {
        !cache.tombstones.contains(&object_id) && cache.tombstones.len() >= MAX_ENTITIES
    }) {
        cache.models = Some(models);
        return LiveEntityPacketOutcome::Rejected;
    }
    let mut render_value = (cache.render_request_id == request_id)
        .then(|| cache.render.take())
        .flatten();
    let (changed, render_changed) = {
        let LiveEntityCache {
            hidden,
            tombstones,
            actions,
            ..
        } = &mut *cache;
        apply_cache_mutation(
            &mut models,
            render_value.as_mut(),
            &mutation,
            hidden,
            tombstones,
            actions,
        )
    };
    if !changed {
        if cache.render_request_id == request_id {
            cache.render = render_value;
        }
        cache.models = Some(models);
        return LiveEntityPacketOutcome::Ignored;
    }
    if let EntityMutation::Damage(event) = mutation {
        while cache.damage_events.len() >= MAX_DAMAGE_EVENTS {
            cache.damage_events.pop_front();
        }
        cache.damage_events.push_back(event);
    }

    let render = render_changed
        .then(|| render_value.as_mut().and_then(runtime_render_json))
        .flatten();
    if cache.render_request_id == request_id {
        cache.render = render_value;
    }
    let encoded_models = models.to_string();
    cache.models = Some(models);
    LiveEntityPacketOutcome::Applied {
        models: encoded_models,
        render,
        presentation_event,
    }
}

/// Mirror authoritative remote-object movement into the shared Bevy
/// presentation clock. The event changes only the render offset: the packed
/// model has already moved to the server-provided endpoint and remains the
/// sole gameplay state used by Android.
fn presentation_event(
    packet: &str,
    models: &Value,
    mutation: &EntityMutation,
    now_ms: u64,
) -> Option<String> {
    let remote_motion = |object_id: u32,
                         to: (i32, i32),
                         packet_direction: Option<&str>,
                         mode: &str,
                         phase_count: Option<u8>| {
        if self_object_id(models) == Some(object_id) {
            return None;
        }
        let (from, previous_direction) = model_pose(models, object_id)?;
        let mut event = json!({
            "type": "remoteMotion",
            "atMs": now_ms,
            "packet": packet,
            "objectId": object_id.to_string(),
            "fromX": from.0,
            "fromY": from.1,
            "toX": to.0,
            "toY": to.1,
            "direction": packet_direction
                .map(str::to_owned)
                .or(previous_direction)
                .unwrap_or_else(|| "Down".to_owned()),
            "mode": mode,
        });
        if let Some(phase_count) = phase_count {
            event["phaseCount"] = json!(phase_count);
        }
        Some(event.to_string())
    };

    match (packet, mutation) {
        (
            "ObjectWalk" | "ObjectRun",
            EntityMutation::Action {
                object_id,
                position,
                direction,
                ..
            },
        ) => remote_motion(
            *object_id,
            *position,
            direction.as_deref(),
            if packet == "ObjectRun" { "run" } else { "walk" },
            None,
        ),
        ("ObjectBackStep" | "ObjectTurn", EntityMutation::Move(object_id, position, direction)) => {
            remote_motion(
                *object_id,
                *position,
                direction.as_deref(),
                if packet == "ObjectTurn" {
                    "turn"
                } else {
                    "backstep"
                },
                None,
            )
        }
        (
            "ObjectPushed",
            EntityMutation::Action {
                object_id,
                position,
                direction,
                ..
            },
        ) => remote_motion(
            *object_id,
            *position,
            direction.as_deref(),
            "pushed",
            Some(3),
        ),
        (
            "ObjectDash",
            EntityMutation::Action {
                object_id,
                position,
                direction,
                action,
                ..
            },
        ) => remote_motion(*object_id, *position, direction.as_deref(), action, Some(3)),
        ("ObjectDashFail", EntityMutation::Move(object_id, position, direction)) => remote_motion(
            *object_id,
            *position,
            direction.as_deref(),
            "dashFail",
            Some(1),
        ),
        (
            "ObjectHide" | "ObjectTeleportOut" | "ObjectRemove",
            EntityMutation::Hide(object_id) | EntityMutation::Remove(object_id),
        ) => (self_object_id(models) != Some(*object_id)).then(|| {
            json!({
                "type": "remoteRemove",
                "atMs": now_ms,
                "objectId": object_id.to_string(),
            })
            .to_string()
        }),
        _ => None,
    }
}

fn packet_action(
    packet: &str,
    payload: &serde_json::Map<String, Value>,
    models: &Value,
    object_id: u32,
) -> String {
    match packet {
        "ObjectWalk" => "walking",
        "ObjectRun" => "running",
        "ObjectHarvest" => "harvest",
        "ObjectHarvested" => "skeleton",
        "ObjectRangeAttack" => "attackRange1",
        "ObjectMagic" => "spell",
        "ObjectStruck" => "struck",
        "ObjectDashAttack" => "dashAttack",
        "ObjectAttack" => {
            let is_player = models
                .get("entities")
                .and_then(Value::as_array)
                .and_then(|entities| {
                    entities
                        .iter()
                        .find(|entity| entity_id(entity) == Some(object_id))
                })
                .and_then(|entity| entity.get("kind"))
                .and_then(Value::as_str)
                .is_some_and(|kind| matches!(kind, "selfPlayer" | "player"));
            if is_player {
                "attack1"
            } else {
                match payload
                    .get("attackType")
                    .or_else(|| payload.get("type"))
                    .and_then(Value::as_u64)
                    .unwrap_or_default()
                {
                    1 => "attack2",
                    2 => "attack3",
                    3 => "attack4",
                    _ => "attack1",
                }
            }
        }
        _ => "standing",
    }
    .to_owned()
}

fn action_direction(
    payload: &serde_json::Map<String, Value>,
    models: &Value,
    object_id: u32,
) -> Option<String> {
    direction(payload)
        .or_else(|| {
            models
                .get("entities")
                .and_then(Value::as_array)
                .and_then(|entities| {
                    entities
                        .iter()
                        .find(|entity| entity_id(entity) == Some(object_id))
                })
                .and_then(|entity| entity.get("direction"))
                .and_then(Value::as_str)
        })
        .map(str::to_owned)
}

fn remote_dash_action(actions: &HashMap<u32, ActiveAction>, object_id: u32) -> &'static str {
    if actions
        .get(&object_id)
        .is_some_and(|action| action.action == "dashL")
    {
        "dashR"
    } else {
        "dashL"
    }
}

fn self_object_id(models: &Value) -> Option<u32> {
    models
        .get("entities")
        .and_then(Value::as_array)
        .and_then(|entities| {
            entities
                .iter()
                .find(|entity| entity.get("kind").and_then(Value::as_str) == Some("selfPlayer"))
        })
        .and_then(entity_id)
}

fn model_pose(models: &Value, object_id: u32) -> Option<((i32, i32), Option<String>)> {
    let entity = models
        .get("entities")
        .and_then(Value::as_array)?
        .iter()
        .find(|entity| entity_id(entity) == Some(object_id))?;
    let position = (coordinate(entity.get("x"))?, coordinate(entity.get("y"))?);
    let direction = entity
        .get("direction")
        .and_then(Value::as_str)
        .map(str::to_owned);
    Some((position, direction))
}

fn mutation_object_id(mutation: &EntityMutation) -> Option<u32> {
    match mutation {
        EntityMutation::Hide(object_id)
        | EntityMutation::Show(object_id)
        | EntityMutation::Remove(object_id)
        | EntityMutation::Health { object_id, .. }
        | EntityMutation::Metadata { object_id, .. }
        | EntityMutation::Poison { object_id, .. }
        | EntityMutation::LevelEffects { object_id, .. }
        | EntityMutation::Hidden { object_id, .. }
        | EntityMutation::Action { object_id, .. } => Some(*object_id),
        EntityMutation::Damage(event) => Some(event.object_id),
        EntityMutation::Move(object_id, ..) => Some(*object_id),
        EntityMutation::Spawn(spawn) => entity_id(&spawn.model),
        EntityMutation::GroundDrop(drop) => entity_id(&drop.model),
        EntityMutation::MoveSelf(..) => None,
    }
}

fn apply_cache_mutation(
    models: &mut Value,
    render: Option<&mut Value>,
    mutation: &EntityMutation,
    hidden: &mut HashMap<u32, HiddenEntity>,
    tombstones: &mut HashSet<u32>,
    actions: &mut HashMap<u32, ActiveAction>,
) -> (bool, bool) {
    let action_cancelled = match mutation {
        EntityMutation::Move(object_id, ..) => actions.remove(object_id).is_some(),
        _ => false,
    };
    match mutation {
        EntityMutation::Hide(object_id) => {
            actions.remove(object_id);
            let model = take_entity(models, *object_id);
            let rendered = render.and_then(|value| take_entity(value, *object_id));
            let visible_changed = model.is_some() || rendered.is_some();
            if visible_changed {
                let entry = hidden.entry(*object_id).or_default();
                if model.is_some() {
                    entry.model = model;
                }
                if rendered.is_some() {
                    entry.render = rendered;
                }
            }
            let changed = tombstones.insert(*object_id) || visible_changed;
            (changed, visible_changed)
        }
        EntityMutation::Show(object_id) => {
            let Some(mut entry) = hidden.remove(object_id) else {
                return (false, false);
            };
            let model_changed = entry
                .model
                .take()
                .is_some_and(|entity| push_entity(models, entity));
            let render_changed = entry
                .render
                .take()
                .zip(render)
                .is_some_and(|(entity, value)| push_entity(value, entity));
            if model_changed || render_changed {
                tombstones.remove(object_id);
                (true, render_changed)
            } else {
                hidden.insert(*object_id, entry);
                (false, false)
            }
        }
        EntityMutation::Remove(object_id) => {
            actions.remove(object_id);
            let model_changed = take_entity(models, *object_id).is_some()
                | take_ground_drop(models, *object_id).is_some();
            let render_changed = render
                .and_then(|value| take_entity(value, *object_id))
                .is_some();
            let hidden_changed = hidden.remove(object_id).is_some();
            let changed =
                tombstones.insert(*object_id) || model_changed || render_changed || hidden_changed;
            (changed, render_changed)
        }
        EntityMutation::Spawn(spawn) => {
            let object_id = entity_id(&spawn.model).expect("spawn was validated");
            actions.remove(&object_id);
            tombstones.remove(&object_id);
            hidden.remove(&object_id);
            let model_changed = apply_models(models, mutation);
            let render_changed = render.is_some_and(|value| apply_render(value, mutation));
            (model_changed || render_changed, render_changed)
        }
        EntityMutation::GroundDrop(drop) => {
            let object_id = entity_id(&drop.model).expect("ground drop was validated");
            actions.remove(&object_id);
            tombstones.remove(&object_id);
            hidden.remove(&object_id);
            let model_changed = apply_models(models, mutation);
            let render_changed = render.is_some_and(|value| apply_render(value, mutation));
            (model_changed || render_changed, render_changed)
        }
        EntityMutation::Metadata { object_id, update } => {
            let model_changed = apply_models(models, mutation);
            let hidden_changed = hidden
                .get_mut(object_id)
                .and_then(|entry| entry.model.as_mut())
                .is_some_and(|entity| patch_model_metadata(entity, update));
            (model_changed || hidden_changed, false)
        }
        EntityMutation::Poison {
            object_id,
            poison,
            at_ms,
        } => {
            let model_changed = apply_models(models, mutation);
            let visible_actor = models
                .get("entities")
                .and_then(Value::as_array)
                .is_some_and(|entities| {
                    entities
                        .iter()
                        .any(|entity| entity_id(entity) == Some(*object_id))
                });
            let render_changed =
                visible_actor && render.is_some_and(|value| apply_render(value, mutation));
            let hidden_changed = hidden.get_mut(object_id).is_some_and(|entry| {
                let model_changed = entry
                    .model
                    .as_mut()
                    .is_some_and(|entity| patch_model_poison(entity, *poison));
                let render_changed = entry
                    .render
                    .as_mut()
                    .is_some_and(|entity| patch_render_poison(entity, *poison));
                model_changed || render_changed
            });
            if let Some(action) = actions.get_mut(object_id) {
                update_action_slow(action, poison & POISON_SLOW != 0, *at_ms);
            }
            (
                model_changed || render_changed || hidden_changed,
                render_changed,
            )
        }
        EntityMutation::LevelEffects {
            object_id,
            level_effects,
        } => {
            let model_changed = apply_models(models, mutation);
            let hidden_changed = hidden
                .get_mut(object_id)
                .and_then(|entry| entry.model.as_mut())
                .is_some_and(|entity| patch_model_level_effects(entity, *level_effects));
            (model_changed || hidden_changed, false)
        }
        EntityMutation::Hidden {
            object_id,
            hidden: actor_hidden,
        } => {
            let visible_actor = models
                .get("entities")
                .and_then(Value::as_array)
                .is_some_and(|entities| {
                    entities
                        .iter()
                        .any(|entity| entity_id(entity) == Some(*object_id))
                });
            let model_changed = apply_models(models, mutation);
            let render_changed =
                visible_actor && render.is_some_and(|value| apply_render(value, mutation));
            let hidden_changed = hidden.get_mut(object_id).is_some_and(|entry| {
                let model_changed = entry
                    .model
                    .as_mut()
                    .is_some_and(|entity| patch_model_hidden(entity, *actor_hidden));
                let render_changed = entry
                    .render
                    .as_mut()
                    .is_some_and(|entity| patch_render_hidden(entity, *actor_hidden));
                model_changed || render_changed
            });
            (
                model_changed || render_changed || hidden_changed,
                render_changed,
            )
        }
        EntityMutation::Action {
            object_id,
            action,
            started_ms,
            direction,
            ..
        } => {
            let model_changed = apply_models(models, mutation);
            let render_changed = render.is_some_and(|value| {
                let changed = apply_render(value, mutation);
                start_render_action(
                    value,
                    *object_id,
                    action,
                    direction.as_deref(),
                    *started_ms,
                    actions,
                );
                changed
            });
            (model_changed || render_changed, render_changed)
        }
        _ => {
            let model_changed = apply_models(models, mutation);
            let render_changed = render.is_some_and(|value| apply_render(value, mutation));
            (
                model_changed || render_changed || action_cancelled,
                render_changed,
            )
        }
    }
}

fn take_entity(value: &mut Value, object_id: u32) -> Option<Value> {
    let entities = value.get_mut("entities")?.as_array_mut()?;
    let index = entities
        .iter()
        .position(|entity| entity_id(entity) == Some(object_id))?;
    Some(entities.remove(index))
}

fn take_ground_drop(value: &mut Value, object_id: u32) -> Option<Value> {
    let drops = value.get_mut("groundDrops")?.as_array_mut()?;
    let index = drops
        .iter()
        .position(|drop| entity_id(drop) == Some(object_id))?;
    Some(drops.remove(index))
}

fn push_entity(value: &mut Value, entity: Value) -> bool {
    let Some(entities) = value.get_mut("entities").and_then(Value::as_array_mut) else {
        return false;
    };
    if entities.len() >= MAX_ENTITIES
        || entity_id(&entity).is_none()
        || entities
            .iter()
            .any(|existing| entity_id(existing) == entity_id(&entity))
    {
        return false;
    }
    entities.push(entity);
    true
}

#[derive(Debug)]
enum EntityMutation {
    MoveSelf((i32, i32), Option<String>),
    Move(u32, (i32, i32), Option<String>),
    Action {
        object_id: u32,
        position: (i32, i32),
        direction: Option<String>,
        action: String,
        started_ms: u64,
        life_state: Option<bool>,
    },
    Health {
        object_id: u32,
        percent: u8,
        expire: u8,
        generation: u64,
        revision: u64,
    },
    Metadata {
        object_id: u32,
        update: EntityMetadata,
    },
    Poison {
        object_id: u32,
        poison: u16,
        at_ms: u64,
    },
    LevelEffects {
        object_id: u32,
        level_effects: u16,
    },
    Hidden {
        object_id: u32,
        hidden: bool,
    },
    Damage(LiveDamageEvent),
    Hide(u32),
    Show(u32),
    Remove(u32),
    Spawn(EntitySpawn),
    GroundDrop(GroundDropSpawn),
}

#[derive(Debug)]
enum EntityMetadata {
    Name(String),
    NameColour(i32),
    GuildName(Option<String>),
}

#[derive(Debug)]
struct EntitySpawn {
    model: Value,
    prototype: Option<Value>,
    position: (i32, i32),
    direction: Option<String>,
}

#[derive(Debug)]
struct GroundDropSpawn {
    model: Value,
    position: (i32, i32),
    image: u16,
}

fn coordinate(value: Option<&Value>) -> Option<i32> {
    i32::try_from(value?.as_i64()?)
        .ok()
        .filter(|value| *value >= 0)
}

fn object_id(payload: &serde_json::Map<String, Value>) -> Option<u32> {
    u32::try_from(payload.get("objectId")?.as_u64()?)
        .ok()
        .filter(|value| *value != 0)
}

fn location(payload: &serde_json::Map<String, Value>) -> Option<(i32, i32)> {
    let source = payload
        .get("location")
        .and_then(Value::as_object)
        .unwrap_or(payload);
    Some((coordinate(source.get("x"))?, coordinate(source.get("y"))?))
}

fn direction(payload: &serde_json::Map<String, Value>) -> Option<&str> {
    payload
        .get("direction")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 16)
}

fn mir_direction(payload: &serde_json::Map<String, Value>) -> Option<&str> {
    direction(payload).filter(|direction| {
        matches!(
            *direction,
            "Up" | "UpRight" | "Right" | "DownRight" | "Down" | "DownLeft" | "Left" | "UpLeft"
        )
    })
}

fn percent(payload: &serde_json::Map<String, Value>) -> Option<u8> {
    u8::try_from(payload.get("percent")?.as_u64()?)
        .ok()
        .filter(|value| *value <= 100)
}

fn expire(payload: &serde_json::Map<String, Value>) -> Option<u8> {
    u8::try_from(payload.get("expire")?.as_u64()?).ok()
}

fn damage(payload: &serde_json::Map<String, Value>) -> Option<i32> {
    i32::try_from(payload.get("damage")?.as_i64()?).ok()
}

fn damage_type(payload: &serde_json::Map<String, Value>) -> Option<u8> {
    u8::try_from(payload.get("damageType")?.as_u64()?).ok()
}

fn signed_i32(payload: &serde_json::Map<String, Value>, key: &str) -> Option<i32> {
    i32::try_from(payload.get(key)?.as_i64()?).ok()
}

fn unsigned_u16(payload: &serde_json::Map<String, Value>, key: &str) -> Option<u16> {
    u16::try_from(payload.get(key)?.as_u64()?).ok()
}

fn boolean(payload: &serde_json::Map<String, Value>, key: &str) -> Option<bool> {
    payload.get(key)?.as_bool()
}

fn bounded_text(
    payload: &serde_json::Map<String, Value>,
    key: &str,
    empty_is_none: bool,
) -> Option<Option<String>> {
    let value = payload.get(key)?.as_str()?.trim();
    if value.chars().count() > 128 || (!empty_is_none && value.is_empty()) {
        return None;
    }
    Some((!value.is_empty()).then(|| value.to_owned()))
}

fn ground_drop(payload: &serde_json::Map<String, Value>, gold: bool) -> Option<GroundDropSpawn> {
    let object_id = object_id(payload)?;
    let position = location(payload)?;
    let quantity = if gold {
        u32::try_from(payload.get("gold")?.as_u64()?).ok()?
    } else {
        payload
            .get("quantity")
            .map(|value| u32::try_from(value.as_u64()?).ok())
            .unwrap_or(Some(1))?
    };
    if quantity == 0 {
        return None;
    }
    let image = if gold {
        gold_ground_frame(quantity)
    } else {
        u16::try_from(payload.get("image")?.as_u64()?).ok()?
    };
    let name = if gold {
        "Gold".to_owned()
    } else {
        let name = payload.get("name")?.as_str()?.trim();
        if name.is_empty() || name.chars().count() > 128 {
            return None;
        }
        name.to_owned()
    };
    let name_colour_argb = match payload.get("nameColourArgb") {
        None if gold => -1,
        Some(value) => i32::try_from(value.as_i64()?).ok()?,
        None => return None,
    };
    let grade = match payload.get("grade") {
        None if gold => 0,
        Some(value) => u8::try_from(value.as_u64()?).ok()?,
        None => return None,
    };
    let model = json!({
        "objectId": object_id.to_string(),
        "name": name,
        "nameColourArgb": name_colour_argb,
        "x": position.0,
        "y": position.1,
        "quantity": quantity,
        "image": image,
        "grade": grade,
        "dropKind": if gold { "gold" } else { "item" },
    });
    Some(GroundDropSpawn {
        model,
        position,
        image,
    })
}

fn gold_ground_frame(quantity: u32) -> u16 {
    match quantity {
        0..=99 => 112,
        100..=199 => 113,
        200..=499 => 114,
        500..=999 => 115,
        _ => 116,
    }
}

fn spawn(payload: &serde_json::Map<String, Value>, kind: &str) -> Option<EntitySpawn> {
    let object_id = object_id(payload)?;
    let name = payload
        .get("name")?
        .as_str()?
        .trim()
        .chars()
        .take(129)
        .collect::<String>();
    if name.is_empty() || name.chars().count() > 128 {
        return None;
    }
    let (x, y) = location(payload)?;
    let level = payload
        .get("level")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok());
    let direction = direction(payload).map(str::to_owned);
    let name_colour_argb = match payload.get("nameColourArgb") {
        None | Some(Value::Null) => None,
        Some(value) => Some(i32::try_from(value.as_i64()?).ok()?),
    };
    let guild_name = match payload.get("guildName") {
        None | Some(Value::Null) => None,
        Some(value) => {
            let value = value.as_str()?.trim();
            if value.chars().count() > 128 {
                return None;
            }
            (!value.is_empty()).then(|| value.to_owned())
        }
    };
    let image = match payload.get("image") {
        None | Some(Value::Null) => None,
        Some(value) => Some(u16::try_from(value.as_u64()?).ok()?),
    };
    let dead = match payload.get("dead") {
        None | Some(Value::Null) => false,
        Some(value) => value.as_bool()?,
    };
    let hidden = match payload.get("hidden") {
        None | Some(Value::Null) => false,
        Some(value) => value.as_bool()?,
    };
    let poison = match payload.get("poison") {
        None | Some(Value::Null) => 0,
        Some(value) => u16::try_from(value.as_u64()?).ok()?,
    };
    let level_effects = match payload.get("levelEffects") {
        None | Some(Value::Null) => 0,
        Some(value) => u16::try_from(value.as_u64()?).ok()?,
    };
    let model = json!({
        "objectId": object_id.to_string(),
        "kind": kind,
        "name": name,
        "x": x,
        "y": y,
        "level": level,
        "direction": direction,
        "nameColourArgb": name_colour_argb,
        "guildName": guild_name,
        "image": image,
        "dead": dead,
        "hidden": hidden,
        "poison": poison,
        "levelEffects": level_effects,
    });
    Some(EntitySpawn {
        model,
        prototype: prototype_from_payload(payload, kind),
        position: (x, y),
        direction,
    })
}

fn prototype_from_payload(payload: &serde_json::Map<String, Value>, kind: &str) -> Option<Value> {
    let sprite = payload.get("sprite")?.as_object()?;
    let body_library = required_string(sprite, "bodyLibrary")?;
    let class_key = payload
        .get("classKey")
        .or_else(|| payload.get("class"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if class_key.len() > 32 {
        return None;
    }
    Some(json!({
        "kind": kind,
        "classKey": class_key,
        "dead": payload.get("dead").and_then(Value::as_bool).unwrap_or(false),
        "sprite": {
            "bodyLibrary": body_library,
            "hairLibrary": optional_string(sprite, "hairLibrary")?,
            "weaponLibrary": optional_string(sprite, "weaponLibrary")?,
            "weaponLibrarySecondary": optional_string(sprite, "weaponLibrarySecondary")?,
            "altBodyLibrary": optional_string(sprite, "altBodyLibrary")?,
            "altHairLibrary": optional_string(sprite, "altHairLibrary")?,
            "altWeaponLibrary": optional_string(sprite, "altWeaponLibrary")?,
            "altWeaponLibrarySecondary": optional_string(sprite, "altWeaponLibrarySecondary")?,
            "mountLibrary": optional_string(sprite, "mountLibrary")?,
            "frameBaseOffset": integer_or_default(sprite, "frameBaseOffset", 0)?,
            "weaponFrameOffset": optional_integer(sprite, "weaponFrameOffset")?,
            "altFrameBaseOffset": optional_integer(sprite, "altFrameBaseOffset")?,
            "altWeaponFrameOffset": optional_integer(sprite, "altWeaponFrameOffset")?,
            "frameCount": integer_or_default(sprite, "frameCount", 4)?,
            "directionStride": integer_or_default(sprite, "directionStride", 4)?,
            "mountFrameOffset": optional_integer(sprite, "mountFrameOffset")?,
        },
    }))
}

fn required_string(map: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    map.get(key)?
        .as_str()
        .filter(|value| !value.is_empty() && value.len() <= 192)
        .map(str::to_owned)
}

fn optional_string(map: &serde_json::Map<String, Value>, key: &str) -> Option<Value> {
    match map.get(key) {
        None | Some(Value::Null) => Some(Value::Null),
        Some(Value::String(value)) if !value.is_empty() && value.len() <= 192 => {
            Some(Value::String(value.clone()))
        }
        _ => None,
    }
}

fn optional_integer(map: &serde_json::Map<String, Value>, key: &str) -> Option<Value> {
    match map.get(key) {
        None | Some(Value::Null) => Some(Value::Null),
        Some(value) => i32::try_from(value.as_i64()?)
            .ok()
            .map(|value| json!(value)),
    }
}

fn integer_or_default(
    map: &serde_json::Map<String, Value>,
    key: &str,
    default: i32,
) -> Option<i32> {
    match map.get(key) {
        None => Some(default),
        Some(value) => i32::try_from(value.as_i64()?).ok(),
    }
}

fn valid_models(value: &Value) -> bool {
    let Some(entities) = value.get("entities").and_then(Value::as_array) else {
        return false;
    };
    let ground_drops = match value.get("groundDrops") {
        None => &[][..],
        Some(value) => match value.as_array() {
            Some(ground_drops) => ground_drops.as_slice(),
            None => return false,
        },
    };
    if entities.is_empty()
        || entities
            .len()
            .checked_add(ground_drops.len())
            .is_none_or(|count| count > MAX_ENTITIES)
    {
        return false;
    }
    let mut ids = HashSet::new();
    entities.iter().all(|entity| {
        let Some(object_id) = entity
            .get("objectId")
            .and_then(Value::as_str)
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|value| *value != 0)
        else {
            return false;
        };
        ids.insert(object_id)
            && coordinate(entity.get("x")).is_some()
            && coordinate(entity.get("y")).is_some()
            && matches!(
                entity.get("kind").and_then(Value::as_str),
                Some("selfPlayer" | "player" | "monster" | "npc")
            )
            && entity.get("levelEffects").is_none_or(|value| {
                value
                    .as_u64()
                    .is_some_and(|value| u16::try_from(value).is_ok())
            })
            && entity.get("hidden").is_none_or(Value::is_boolean)
    }) && ground_drops.iter().all(|drop| {
        let Some(object_id) = entity_id(drop) else {
            return false;
        };
        ids.insert(object_id)
            && coordinate(drop.get("x")).is_some()
            && coordinate(drop.get("y")).is_some()
            && drop
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| !name.is_empty() && name.chars().count() <= 128)
            && drop
                .get("quantity")
                .and_then(Value::as_u64)
                .is_some_and(|quantity| (1..=u64::from(u32::MAX)).contains(&quantity))
            && drop
                .get("image")
                .and_then(Value::as_u64)
                .is_some_and(|image| u16::try_from(image).is_ok())
            && matches!(
                drop.get("dropKind").and_then(Value::as_str),
                Some("item" | "gold")
            )
    })
}

fn valid_render(value: &Value) -> bool {
    let Some(entities) = value.get("entities").and_then(Value::as_array) else {
        return false;
    };
    if entities.len() > MAX_ENTITIES {
        return false;
    }
    let mut ids = HashSet::new();
    entities.iter().all(|entity| {
        let Some(object_id) = entity
            .get("objectId")
            .and_then(Value::as_str)
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|value| *value != 0)
        else {
            return false;
        };
        ids.insert(object_id)
            && coordinate(entity.get("gridX")).is_some()
            && coordinate(entity.get("gridY")).is_some()
            && entity
                .get("layers")
                .and_then(Value::as_array)
                .is_some_and(|layers| valid_layers(layers))
            && entity.get("directionLayers").is_none_or(|directions| {
                directions.as_object().is_some_and(|directions| {
                    directions.len() <= 8
                        && directions.iter().all(|(direction, layers)| {
                            matches!(
                                direction.as_str(),
                                "Up" | "UpRight"
                                    | "Right"
                                    | "DownRight"
                                    | "Down"
                                    | "DownLeft"
                                    | "Left"
                                    | "UpLeft"
                            ) && layers.as_array().is_some_and(|layers| valid_layers(layers))
                        })
                })
            })
            && entity.get("prototype").is_none_or(|prototype| {
                prototype.is_object()
                    && serde_json::to_string(prototype)
                        .is_ok_and(|encoded| encoded.len() <= 8 * 1024)
            })
            && entity.get("actionLayers").is_none_or(valid_action_layers)
    })
}

fn valid_layers(layers: &[Value]) -> bool {
    layers.len() <= 8
        && layers.iter().all(|layer| {
            layer
                .get("left")
                .and_then(Value::as_f64)
                .is_some_and(f64::is_finite)
                && layer
                    .get("top")
                    .and_then(Value::as_f64)
                    .is_some_and(f64::is_finite)
                && layer
                    .get("z")
                    .is_none_or(|z| z.as_f64().is_some_and(f64::is_finite))
        })
}

fn runtime_render_json(value: &mut Value) -> Option<String> {
    let ground_item_frames = value.as_object_mut()?.remove("_nativeGroundItemFrames");
    let removed = {
        let entities = value.get_mut("entities")?.as_array_mut()?;
        entities
            .iter_mut()
            .map(|entity| {
                entity
                    .as_object_mut()
                    .expect("validated render entity")
                    .remove("directionLayers")
            })
            .collect::<Vec<_>>()
    };
    let prototypes = {
        let entities = value.get_mut("entities")?.as_array_mut()?;
        entities
            .iter_mut()
            .map(|entity| {
                entity
                    .as_object_mut()
                    .expect("validated render entity")
                    .remove("prototype")
            })
            .collect::<Vec<_>>()
    };
    let actions = {
        let entities = value.get_mut("entities")?.as_array_mut()?;
        entities
            .iter_mut()
            .map(|entity| {
                entity
                    .as_object_mut()
                    .expect("validated render entity")
                    .remove("actionLayers")
            })
            .collect::<Vec<_>>()
    };
    let encoded = value.to_string();
    for (entity, directions) in value
        .get_mut("entities")?
        .as_array_mut()?
        .iter_mut()
        .zip(removed.into_iter().zip(prototypes).zip(actions))
    {
        let ((directions, prototype), actions) = directions;
        if let Some(directions) = directions {
            entity
                .as_object_mut()
                .expect("validated render entity")
                .insert("directionLayers".into(), directions);
        }
        if let Some(prototype) = prototype {
            entity
                .as_object_mut()
                .expect("validated render entity")
                .insert("prototype".into(), prototype);
        }
        if let Some(actions) = actions {
            entity
                .as_object_mut()
                .expect("validated render entity")
                .insert("actionLayers".into(), actions);
        }
    }
    if let Some(ground_item_frames) = ground_item_frames {
        value
            .as_object_mut()?
            .insert("_nativeGroundItemFrames".into(), ground_item_frames);
    }
    (encoded.len() <= MAX_RENDER_BYTES).then_some(encoded)
}

fn entity_id(entity: &Value) -> Option<u32> {
    entity
        .get("objectId")?
        .as_str()?
        .parse::<u32>()
        .ok()
        .filter(|value| *value != 0)
}

fn apply_models(models: &mut Value, mutation: &EntityMutation) -> bool {
    if let EntityMutation::GroundDrop(drop) = mutation {
        return upsert_ground_drop_model(models, drop);
    }
    if let EntityMutation::Spawn(spawn) = mutation {
        if let Some(object_id) = entity_id(&spawn.model) {
            take_ground_drop(models, object_id);
        }
    }
    let Some(entities) = models.get_mut("entities").and_then(Value::as_array_mut) else {
        return false;
    };
    match mutation {
        EntityMutation::MoveSelf(position, direction) => entities
            .iter_mut()
            .find(|entity| entity.get("kind").and_then(Value::as_str) == Some("selfPlayer"))
            .is_some_and(|entity| patch_model_transform(entity, *position, direction.as_deref())),
        EntityMutation::Move(object_id, position, direction) => entities
            .iter_mut()
            .find(|entity| entity_id(entity) == Some(*object_id))
            .is_some_and(|entity| patch_model_transform(entity, *position, direction.as_deref())),
        EntityMutation::Action {
            object_id,
            position,
            direction,
            action,
            started_ms,
            life_state,
        } => entities
            .iter_mut()
            .find(|entity| entity_id(entity) == Some(*object_id))
            .is_some_and(|entity| {
                let mut changed = patch_model_transform(entity, *position, direction.as_deref());
                changed |=
                    entity.get("_nativeAnimationAction").and_then(Value::as_str) != Some(action);
                changed |= entity
                    .get("_nativeAnimationStartedMs")
                    .and_then(Value::as_u64)
                    != Some(*started_ms);
                entity["_nativeAnimationAction"] = json!(action);
                entity["_nativeAnimationStartedMs"] = json!(started_ms);
                if let Some(dead) = life_state {
                    changed |= patch_model_dead(entity, *dead);
                }
                changed
            }),
        EntityMutation::Health {
            object_id,
            percent,
            expire,
            generation,
            revision,
        } => entities
            .iter_mut()
            .find(|entity| entity_id(entity) == Some(*object_id))
            .is_some_and(|entity| {
                patch_model_health(entity, *percent, *expire, *generation, *revision)
            }),
        EntityMutation::Metadata { object_id, update } => entities
            .iter_mut()
            .find(|entity| entity_id(entity) == Some(*object_id))
            .is_some_and(|entity| patch_model_metadata(entity, update)),
        EntityMutation::Poison {
            object_id, poison, ..
        } => entities
            .iter_mut()
            .find(|entity| entity_id(entity) == Some(*object_id))
            .is_some_and(|entity| patch_model_poison(entity, *poison)),
        EntityMutation::LevelEffects {
            object_id,
            level_effects,
        } => entities
            .iter_mut()
            .find(|entity| entity_id(entity) == Some(*object_id))
            .is_some_and(|entity| patch_model_level_effects(entity, *level_effects)),
        EntityMutation::Hidden { object_id, hidden } => entities
            .iter_mut()
            .find(|entity| entity_id(entity) == Some(*object_id))
            .is_some_and(|entity| patch_model_hidden(entity, *hidden)),
        EntityMutation::Damage(event) => entities
            .iter_mut()
            .find(|entity| entity_id(entity) == Some(event.object_id))
            .is_some_and(|entity| {
                if event.damage > 0 && matches!(event.damage_type, 0 | 2) {
                    entity["_healthHitRevision"] = json!(event.sequence);
                }
                true
            }),
        EntityMutation::Spawn(spawn) => {
            let object_id = entity_id(&spawn.model).expect("spawn was validated");
            if let Some(existing) = entities
                .iter_mut()
                .find(|existing| entity_id(existing) == Some(object_id))
            {
                let preserve_self =
                    existing.get("kind").and_then(Value::as_str) == Some("selfPlayer");
                *existing = spawn.model.clone();
                if preserve_self {
                    existing["kind"] = json!("selfPlayer");
                }
            } else if entities.len() < MAX_ENTITIES {
                entities.push(spawn.model.clone());
            } else {
                return false;
            }
            true
        }
        EntityMutation::Hide(_)
        | EntityMutation::Show(_)
        | EntityMutation::Remove(_)
        | EntityMutation::GroundDrop(_) => false,
    }
}

fn patch_model_metadata(entity: &mut Value, update: &EntityMetadata) -> bool {
    match update {
        EntityMetadata::Name(name) => {
            let changed = entity.get("name").and_then(Value::as_str) != Some(name);
            entity["name"] = json!(name);
            changed
        }
        EntityMetadata::NameColour(name_colour_argb) => {
            let changed = entity.get("nameColourArgb").and_then(Value::as_i64)
                != Some(i64::from(*name_colour_argb));
            entity["nameColourArgb"] = json!(name_colour_argb);
            changed
        }
        EntityMetadata::GuildName(guild_name) => {
            let next = guild_name.as_deref();
            let changed = entity.get("guildName").and_then(Value::as_str) != next
                || (next.is_none()
                    && entity
                        .get("guildName")
                        .is_some_and(|value| !value.is_null()));
            entity["guildName"] = guild_name
                .as_ref()
                .map_or(Value::Null, |value| json!(value));
            changed
        }
    }
}

fn patch_model_poison(entity: &mut Value, poison: u16) -> bool {
    let changed = entity.get("poison").and_then(Value::as_u64) != Some(u64::from(poison));
    entity["poison"] = json!(poison);
    changed
}

fn patch_model_level_effects(entity: &mut Value, level_effects: u16) -> bool {
    let changed =
        entity.get("levelEffects").and_then(Value::as_u64) != Some(u64::from(level_effects));
    entity["levelEffects"] = json!(level_effects);
    changed
}

fn patch_model_hidden(entity: &mut Value, hidden: bool) -> bool {
    let changed = entity.get("hidden").and_then(Value::as_bool) != Some(hidden);
    entity["hidden"] = json!(hidden);
    changed
}

fn upsert_ground_drop_model(models: &mut Value, drop: &GroundDropSpawn) -> bool {
    let object_id = entity_id(&drop.model).expect("ground drop was validated");
    let actor_removed = take_entity(models, object_id).is_some();
    if models.get("groundDrops").is_none() {
        models["groundDrops"] = json!([]);
    }
    let entity_count = models
        .get("entities")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let Some(drops) = models.get_mut("groundDrops").and_then(Value::as_array_mut) else {
        return false;
    };
    if let Some(existing) = drops
        .iter_mut()
        .find(|existing| entity_id(existing) == Some(object_id))
    {
        if *existing == drop.model {
            return actor_removed;
        }
        *existing = drop.model.clone();
        return true;
    }
    if entity_count.saturating_add(drops.len()) >= MAX_ENTITIES {
        return actor_removed;
    }
    drops.push(drop.model.clone());
    true
}

fn patch_model_health(
    entity: &mut Value,
    percent: u8,
    expire: u8,
    generation: u64,
    revision: u64,
) -> bool {
    entity["hpPercent"] = json!(percent);
    entity["_healthPercent"] = json!(percent);
    entity["_healthExpireSeconds"] = json!(expire);
    entity["_healthGeneration"] = json!(generation);
    entity["_healthRevision"] = json!(revision);
    patch_model_dead(entity, percent == 0);
    // Every valid packet advances the server-driven visibility revision even
    // when its percentage repeats, so the presentation deadline is renewed.
    true
}

fn patch_model_dead(entity: &mut Value, dead: bool) -> bool {
    let changed = entity.get("dead").and_then(Value::as_bool) != Some(dead);
    entity["dead"] = json!(dead);
    changed
}

fn set_model_position(entity: &mut Value, (x, y): (i32, i32)) -> bool {
    let changed = entity.get("x").and_then(Value::as_i64) != Some(i64::from(x))
        || entity.get("y").and_then(Value::as_i64) != Some(i64::from(y));
    entity["x"] = json!(x);
    entity["y"] = json!(y);
    changed
}

fn patch_model_transform(
    entity: &mut Value,
    position: (i32, i32),
    direction: Option<&str>,
) -> bool {
    let mut changed = set_model_position(entity, position);
    if let Some(direction) = direction {
        changed |= entity.get("direction").and_then(Value::as_str) != Some(direction);
        entity["direction"] = json!(direction);
    }
    changed
}

fn apply_render(render: &mut Value, mutation: &EntityMutation) -> bool {
    if let EntityMutation::GroundDrop(drop) = mutation {
        return upsert_ground_drop_render(render, drop);
    }
    let center = render
        .get("centerX")
        .and_then(|x| coordinate(Some(x)))
        .zip(render.get("centerY").and_then(|y| coordinate(Some(y))));
    let Some(entities) = render.get_mut("entities").and_then(Value::as_array_mut) else {
        return false;
    };
    match mutation {
        EntityMutation::MoveSelf(position, direction) => entities
            .iter_mut()
            .find(|entity| entity.get("isSelf").and_then(Value::as_bool) == Some(true))
            .is_some_and(|entity| patch_render_transform(entity, *position, direction.as_deref())),
        EntityMutation::Action {
            object_id,
            position,
            direction,
            action,
            life_state,
            ..
        } => entities
            .iter_mut()
            .find(|entity| entity_id(entity) == Some(*object_id))
            .is_some_and(|entity| {
                let transformed = patch_render_transform(entity, *position, direction.as_deref());
                let mut appearance = life_state.is_some() && patch_render_dead(entity, false);
                let posed = select_action_frame(entity, action, direction.as_deref(), 0);
                if !posed {
                    if *life_state == Some(true) {
                        appearance |= patch_render_dead(entity, true);
                    } else if action == "revive" {
                        appearance |= select_standing_frame(entity, direction.as_deref());
                    }
                }
                transformed | appearance | posed
            }),
        EntityMutation::Move(object_id, position, direction) => entities
            .iter_mut()
            .find(|entity| entity_id(entity) == Some(*object_id))
            .is_some_and(|entity| patch_render_transform(entity, *position, direction.as_deref())),
        EntityMutation::Health {
            object_id, percent, ..
        } => entities
            .iter_mut()
            .find(|entity| entity_id(entity) == Some(*object_id))
            .is_some_and(|entity| patch_render_dead(entity, *percent == 0)),
        EntityMutation::Metadata { .. } => false,
        EntityMutation::Poison {
            object_id, poison, ..
        } => entities
            .iter_mut()
            .find(|entity| entity_id(entity) == Some(*object_id))
            .is_some_and(|entity| patch_render_poison(entity, *poison)),
        EntityMutation::LevelEffects { .. } => false,
        EntityMutation::Hidden { object_id, hidden } => entities
            .iter_mut()
            .find(|entity| entity_id(entity) == Some(*object_id))
            .is_some_and(|entity| patch_render_hidden(entity, *hidden)),
        EntityMutation::Damage(_) => false,
        EntityMutation::Spawn(spawn) => {
            if let Some(existing) = entities
                .iter_mut()
                .find(|entity| entity_id(entity) == entity_id(&spawn.model))
            {
                let ground_item = existing
                    .get("layers")
                    .and_then(Value::as_array)
                    .is_some_and(|layers| {
                        layers.iter().any(|layer| {
                            layer
                                .get("key")
                                .and_then(Value::as_str)
                                .is_some_and(|key| key.ends_with(":ground-item"))
                        })
                    });
                if !ground_item {
                    let mut changed = patch_render_transform(
                        existing,
                        spawn.position,
                        spawn.direction.as_deref(),
                    );
                    changed |= patch_render_dead(
                        existing,
                        spawn
                            .model
                            .get("dead")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    );
                    changed |= patch_render_hidden(
                        existing,
                        spawn
                            .model
                            .get("hidden")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    );
                    changed |= patch_render_poison(
                        existing,
                        spawn
                            .model
                            .get("poison")
                            .and_then(Value::as_u64)
                            .unwrap_or_default() as u16,
                    );
                    return changed;
                }
                let object_id = entity_id(&spawn.model).expect("spawn was validated");
                entities.retain(|entity| entity_id(entity) != Some(object_id));
            }
            center.is_some_and(|center| spawn_render_from_prototype(entities, spawn, center))
        }
        EntityMutation::Hide(_)
        | EntityMutation::Show(_)
        | EntityMutation::Remove(_)
        | EntityMutation::GroundDrop(_) => false,
    }
}

fn upsert_ground_drop_render(render: &mut Value, drop: &GroundDropSpawn) -> bool {
    let object_id = entity_id(&drop.model).expect("ground drop was validated");
    let center = render
        .get("centerX")
        .and_then(|x| coordinate(Some(x)))
        .zip(render.get("centerY").and_then(|y| coordinate(Some(y))));
    let dimensions = render
        .get("_nativeGroundItemFrames")
        .and_then(|frames| frames.get(drop.image.to_string()))
        .and_then(|frame| {
            Some((
                u32::try_from(frame.get("width")?.as_u64()?).ok()?,
                u32::try_from(frame.get("height")?.as_u64()?).ok()?,
            ))
        });
    let Some(entities) = render.get_mut("entities").and_then(Value::as_array_mut) else {
        return false;
    };
    let previous_len = entities.len();
    entities.retain(|entity| entity_id(entity) != Some(object_id));
    let removed = entities.len() != previous_len;
    let Some(((center_x, center_y), (width, height))) = center.zip(dimensions) else {
        return removed;
    };
    let dx = drop.position.0.saturating_sub(center_x);
    let dy = drop.position.1.saturating_sub(center_y);
    if dx.abs() > 24 || dy.abs() > 32 || entities.len() >= MAX_ENTITIES {
        return removed;
    }
    let root_left = 480.0 + f64::from(dx) * CELL_WIDTH;
    let root_top = 352.0 + f64::from(dy) * CELL_HEIGHT;
    let depth = 4096 + dy * 128 + dx * 2 + 64;
    entities.push(json!({
        "objectId": object_id.to_string(),
        "isSelf": false,
        "gridX": drop.position.0,
        "gridY": drop.position.1,
        "layers": [{
            "key": format!("{object_id}:ground-item"),
            "path": format!("/original-ui/DNItems/{}.png", drop.image),
            "left": root_left + (CELL_WIDTH - f64::from(width)) / 2.0,
            "top": root_top + (CELL_HEIGHT - f64::from(height)) / 2.0,
            "width": width,
            "height": height,
            "z": f64::from(depth) * 10.0,
            "opacity": 1.0,
        }]
    }));
    true
}

fn select_action_frame(
    entity: &mut Value,
    action: &str,
    direction: Option<&str>,
    frame_index: usize,
) -> bool {
    let direction = direction.unwrap_or("Down");
    let pose_key = format!("{action}:{direction}");
    let Some(layers) = entity
        .get("actionLayers")
        .and_then(|actions| actions.get(&pose_key))
        .and_then(|descriptor| descriptor.get("frames"))
        .and_then(Value::as_array)
        .and_then(|frames| frames.get(frame_index))
        .and_then(Value::as_array)
        .cloned()
    else {
        return false;
    };
    if entity.get("layers").and_then(Value::as_array) == Some(&layers) {
        return false;
    }
    entity["layers"] = Value::Array(layers);
    true
}

fn select_standing_frame(entity: &mut Value, direction: Option<&str>) -> bool {
    let direction = direction.unwrap_or("Down");
    let Some(layers) = entity
        .get("directionLayers")
        .and_then(|directions| directions.get(direction))
        .and_then(Value::as_array)
        .cloned()
    else {
        return false;
    };
    if entity.get("layers").and_then(Value::as_array) == Some(&layers) {
        return false;
    }
    entity["layers"] = Value::Array(layers);
    true
}

fn start_render_action(
    render: &Value,
    object_id: u32,
    action: &str,
    direction: Option<&str>,
    started_ms: u64,
    actions: &mut HashMap<u32, ActiveAction>,
) {
    let direction = direction.unwrap_or("Down");
    let pose_key = format!("{action}:{direction}");
    let Some(entity) = render
        .get("entities")
        .and_then(Value::as_array)
        .and_then(|entities| {
            entities
                .iter()
                .find(|entity| entity_id(entity) == Some(object_id))
        })
    else {
        actions.remove(&object_id);
        return;
    };
    let Some(descriptor) = entity
        .get("actionLayers")
        .and_then(|action_layers| action_layers.get(&pose_key))
    else {
        actions.remove(&object_id);
        return;
    };
    let Some(interval_ms) = descriptor.get("intervalMs").and_then(Value::as_u64) else {
        actions.remove(&object_id);
        return;
    };
    let Some(frame_count) = descriptor
        .get("frames")
        .and_then(Value::as_array)
        .map(Vec::len)
        .filter(|count| *count != 0)
    else {
        actions.remove(&object_id);
        return;
    };
    actions.insert(
        object_id,
        ActiveAction {
            action: action.to_owned(),
            started_ms,
            interval_ms,
            slowed: movement_action_uses_slow(action)
                && entity
                    .get("poison")
                    .and_then(Value::as_u64)
                    .is_some_and(|poison| poison & u64::from(POISON_SLOW) != 0),
            frame_count,
            last_frame: 0,
            direction: direction.to_owned(),
            completion: match action {
                "die" => ActionCompletion::Pose("dead"),
                "dead" | "skeleton" => ActionCompletion::Hold,
                _ => ActionCompletion::Standing,
            },
        },
    );
}

fn movement_action_uses_slow(action: &str) -> bool {
    matches!(action, "walking" | "running" | "dashAttack")
}

fn action_frame_interval_ms(action: &ActiveAction) -> u64 {
    action
        .interval_ms
        .saturating_mul(if action.slowed { 2 } else { 1 })
        .max(1)
}

fn update_action_slow(action: &mut ActiveAction, slowed: bool, now_ms: u64) {
    if !movement_action_uses_slow(&action.action) || action.slowed == slowed {
        return;
    }
    let previous_interval = action_frame_interval_ms(action);
    let elapsed = now_ms.saturating_sub(action.started_ms);
    let phase = elapsed / previous_interval;
    let remainder = elapsed % previous_interval;
    action.slowed = slowed;
    let next_interval = action_frame_interval_ms(action);
    let rebased_elapsed = phase
        .saturating_mul(next_interval)
        .saturating_add(remainder.saturating_mul(next_interval) / previous_interval);
    action.started_ms = now_ms.saturating_sub(rebased_elapsed);
}

pub(crate) fn poll_action_frame() -> Option<String> {
    poll_action_frame_at(live_now_ms())
}

fn poll_action_frame_at(now_ms: u64) -> Option<String> {
    let mut cache = LIVE_ENTITIES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if cache.request_id == 0 || cache.render_request_id != cache.request_id {
        return None;
    }
    let mut render = cache.render.take()?;
    let mut changed = false;
    let mut completed = Vec::new();
    for (object_id, action) in &mut cache.actions {
        let elapsed = now_ms.saturating_sub(action.started_ms);
        let phase =
            usize::try_from(elapsed / action_frame_interval_ms(action)).unwrap_or(usize::MAX);
        let Some(entity) = render
            .get_mut("entities")
            .and_then(Value::as_array_mut)
            .and_then(|entities| {
                entities
                    .iter_mut()
                    .find(|entity| entity_id(entity) == Some(*object_id))
            })
        else {
            completed.push(*object_id);
            continue;
        };
        if phase >= action.frame_count {
            match action.completion {
                ActionCompletion::Standing => {
                    changed |= select_standing_frame(entity, Some(&action.direction));
                }
                ActionCompletion::Pose(pose) => {
                    changed |= select_action_frame(entity, pose, Some(&action.direction), 0);
                }
                ActionCompletion::Hold => {}
            }
            completed.push(*object_id);
            continue;
        }
        if phase != action.last_frame {
            changed |= select_action_frame(entity, &action.action, Some(&action.direction), phase);
            action.last_frame = phase;
        }
    }
    for object_id in completed {
        cache.actions.remove(&object_id);
    }
    let encoded = changed.then(|| runtime_render_json(&mut render)).flatten();
    cache.render = Some(render);
    encoded
}

fn patch_render_dead(entity: &mut Value, dead: bool) -> bool {
    let changed = entity.get("dead").and_then(Value::as_bool) != Some(dead);
    entity["dead"] = json!(dead);
    changed | patch_render_actor_opacity(entity)
}

fn patch_render_hidden(entity: &mut Value, hidden: bool) -> bool {
    let changed = entity.get("hidden").and_then(Value::as_bool) != Some(hidden);
    entity["hidden"] = json!(hidden);
    changed | patch_render_actor_opacity(entity)
}

fn patch_render_actor_opacity(entity: &mut Value) -> bool {
    let opacity = if entity.get("hidden").and_then(Value::as_bool) == Some(true) {
        0.5
    } else if entity.get("dead").and_then(Value::as_bool) == Some(true) {
        0.45
    } else {
        1.0
    };
    let mut changed = false;
    if let Some(layers) = entity.get_mut("layers").and_then(Value::as_array_mut) {
        changed |= patch_layers_opacity(layers, opacity);
    }
    if let Some(directions) = entity
        .get_mut("directionLayers")
        .and_then(Value::as_object_mut)
    {
        for layers in directions.values_mut().filter_map(Value::as_array_mut) {
            changed |= patch_layers_opacity(layers, opacity);
        }
    }
    if let Some(actions) = entity
        .get_mut("actionLayers")
        .and_then(Value::as_object_mut)
    {
        for descriptor in actions.values_mut() {
            if let Some(frames) = descriptor.get_mut("frames").and_then(Value::as_array_mut) {
                for layers in frames.iter_mut().filter_map(Value::as_array_mut) {
                    changed |= patch_layers_opacity(layers, opacity);
                }
            }
        }
    }
    changed
}

/// Crystal computes one actor-wide DrawColour from the PoisonType flags. Keep
/// its precedence exact and omit white so older/native producers remain byte
/// compatible when no status tint is active.
pub(crate) fn poison_tint_argb(poison: u16) -> Option<i32> {
    const GREEN: u16 = 1;
    const RED: u16 = 2;
    const FROZEN: u16 = 8;
    const STUN: u16 = 16;
    const PARALYSIS: u16 = 32;
    const DELAYED_EXPLOSION: u16 = 64;
    const BLEEDING: u16 = 128;
    const LR_PARALYSIS: u16 = 256;
    const BLINDNESS: u16 = 512;
    const DAZED: u16 = 1024;

    let argb = if poison & DELAYED_EXPLOSION != 0 {
        0xFFFF_A500_u32
    } else if poison & (PARALYSIS | LR_PARALYSIS) != 0 {
        0xFF80_8080_u32
    } else if poison & FROZEN != 0 {
        0xFF00_00FF_u32
    } else if poison & BLINDNESS != 0 {
        0xFFC7_1585_u32
    } else if poison & (STUN | DAZED) != 0 {
        0xFFFF_FF00_u32
    } else if poison & POISON_SLOW != 0 {
        0xFF80_0080_u32
    } else if poison & BLEEDING != 0 {
        0xFF8B_0000_u32
    } else if poison & RED != 0 {
        0xFFFF_0000_u32
    } else if poison & GREEN != 0 {
        0xFF00_8000_u32
    } else {
        return None;
    };
    Some(argb as i32)
}

fn patch_render_poison(entity: &mut Value, poison: u16) -> bool {
    let tint = poison_tint_argb(poison);
    let mut changed = entity.get("poison").and_then(Value::as_u64) != Some(u64::from(poison));
    entity["poison"] = json!(poison);
    if let Some(layers) = entity.get_mut("layers").and_then(Value::as_array_mut) {
        changed |= patch_layers_tint(layers, tint);
    }
    if let Some(directions) = entity
        .get_mut("directionLayers")
        .and_then(Value::as_object_mut)
    {
        for layers in directions.values_mut().filter_map(Value::as_array_mut) {
            changed |= patch_layers_tint(layers, tint);
        }
    }
    if let Some(actions) = entity
        .get_mut("actionLayers")
        .and_then(Value::as_object_mut)
    {
        for descriptor in actions.values_mut() {
            if let Some(frames) = descriptor.get_mut("frames").and_then(Value::as_array_mut) {
                for layers in frames.iter_mut().filter_map(Value::as_array_mut) {
                    changed |= patch_layers_tint(layers, tint);
                }
            }
        }
    }
    changed
}

fn patch_layers_tint(layers: &mut [Value], tint: Option<i32>) -> bool {
    let mut changed = false;
    for layer in layers {
        let previous = layer.get("tintArgb").and_then(Value::as_i64);
        changed |= previous != tint.map(i64::from);
        if let Some(tint) = tint {
            layer["tintArgb"] = json!(tint);
        } else if let Some(layer) = layer.as_object_mut() {
            layer.remove("tintArgb");
        }
    }
    changed
}

fn patch_layers_opacity(layers: &mut [Value], opacity: f64) -> bool {
    let mut changed = false;
    for layer in layers {
        changed |= layer.get("opacity").and_then(Value::as_f64) != Some(opacity);
        layer["opacity"] = json!(opacity);
    }
    changed
}

fn spawn_render_from_prototype(
    entities: &mut Vec<Value>,
    spawn: &EntitySpawn,
    center: (i32, i32),
) -> bool {
    let Some(prototype) = spawn.prototype.as_ref() else {
        return false;
    };
    if spawn.position.0.abs_diff(center.0) > 24 || spawn.position.1.abs_diff(center.1) > 32 {
        return false;
    }
    let Some(object_id) = entity_id(&spawn.model) else {
        return false;
    };
    let Some(mut rendered) = entities
        .iter()
        .find(|entity| entity.get("prototype") == Some(prototype))
        .cloned()
    else {
        return false;
    };
    rendered["objectId"] = json!(object_id.to_string());
    rendered["isSelf"] = json!(false);
    if !rewrite_layer_keys(&mut rendered, object_id)
        || !patch_render_transform(&mut rendered, spawn.position, spawn.direction.as_deref())
        || entities.len() >= MAX_ENTITIES
    {
        return false;
    }
    patch_render_poison(
        &mut rendered,
        spawn
            .model
            .get("poison")
            .and_then(Value::as_u64)
            .unwrap_or_default() as u16,
    );
    patch_render_dead(
        &mut rendered,
        spawn
            .model
            .get("dead")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    );
    patch_render_hidden(
        &mut rendered,
        spawn
            .model
            .get("hidden")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    );
    entities.push(rendered);
    true
}

fn rewrite_layer_keys(entity: &mut Value, object_id: u32) -> bool {
    let Some(layers) = entity.get_mut("layers").and_then(Value::as_array_mut) else {
        return false;
    };
    if !rewrite_keys(layers, object_id) {
        return false;
    }
    if let Some(directions) = entity
        .get_mut("directionLayers")
        .and_then(Value::as_object_mut)
    {
        for layers in directions.values_mut() {
            let Some(layers) = layers.as_array_mut() else {
                return false;
            };
            if !rewrite_keys(layers, object_id) {
                return false;
            }
        }
    }
    if let Some(actions) = entity
        .get_mut("actionLayers")
        .and_then(Value::as_object_mut)
    {
        for descriptor in actions.values_mut() {
            let Some(frames) = descriptor.get_mut("frames").and_then(Value::as_array_mut) else {
                return false;
            };
            for frame in frames {
                let Some(layers) = frame.as_array_mut() else {
                    return false;
                };
                if !rewrite_keys(layers, object_id) {
                    return false;
                }
            }
        }
    }
    true
}

fn rewrite_keys(layers: &mut [Value], object_id: u32) -> bool {
    for layer in layers {
        let Some(suffix) = layer
            .get("key")
            .and_then(Value::as_str)
            .and_then(|key| key.split_once(':').map(|(_, suffix)| suffix.to_owned()))
        else {
            return false;
        };
        layer["key"] = json!(format!("{object_id}:{suffix}"));
    }
    true
}

fn set_render_position(entity: &mut Value, (x, y): (i32, i32)) -> bool {
    let Some(old_x) = coordinate(entity.get("gridX")) else {
        return false;
    };
    let Some(old_y) = coordinate(entity.get("gridY")) else {
        return false;
    };
    let dx = f64::from(x - old_x) * CELL_WIDTH;
    let dy = f64::from(y - old_y) * CELL_HEIGHT;
    let dz = (f64::from(y - old_y) * 128.0 + f64::from(x - old_x) * 2.0) * 10.0;
    entity["gridX"] = json!(x);
    entity["gridY"] = json!(y);
    let Some(layers) = entity.get_mut("layers").and_then(Value::as_array_mut) else {
        return false;
    };
    if !shift_layers(layers, dx, dy, dz) {
        return false;
    }
    if let Some(directions) = entity
        .get_mut("directionLayers")
        .and_then(Value::as_object_mut)
    {
        for layers in directions.values_mut() {
            let Some(layers) = layers.as_array_mut() else {
                return false;
            };
            if !shift_layers(layers, dx, dy, dz) {
                return false;
            }
        }
    }
    if let Some(actions) = entity
        .get_mut("actionLayers")
        .and_then(Value::as_object_mut)
    {
        for descriptor in actions.values_mut() {
            let Some(frames) = descriptor.get_mut("frames").and_then(Value::as_array_mut) else {
                return false;
            };
            for frame in frames {
                let Some(layers) = frame.as_array_mut() else {
                    return false;
                };
                if !shift_layers(layers, dx, dy, dz) {
                    return false;
                }
            }
        }
    }
    true
}

fn shift_layers(layers: &mut [Value], dx: f64, dy: f64, dz: f64) -> bool {
    for layer in layers {
        let Some(left) = layer.get("left").and_then(Value::as_f64) else {
            return false;
        };
        let Some(top) = layer.get("top").and_then(Value::as_f64) else {
            return false;
        };
        layer["left"] = json!(left + dx);
        layer["top"] = json!(top + dy);
        if let Some(z) = layer.get("z").and_then(Value::as_f64) {
            layer["z"] = json!(z + dz);
        }
    }
    true
}

fn patch_render_transform(
    entity: &mut Value,
    position: (i32, i32),
    direction: Option<&str>,
) -> bool {
    if !set_render_position(entity, position) {
        return false;
    }
    if let Some(layers) = direction.and_then(|direction| {
        entity
            .get("directionLayers")
            .and_then(|directions| directions.get(direction))
            .and_then(Value::as_array)
            .cloned()
    }) {
        entity["layers"] = Value::Array(layers);
    }
    true
}

fn align_render_to_models(render: &mut Value, models: &Value) -> bool {
    let Some(model_entities) = models.get("entities").and_then(Value::as_array) else {
        return false;
    };
    let positions = model_entities
        .iter()
        .chain(
            models
                .get("groundDrops")
                .and_then(Value::as_array)
                .into_iter()
                .flatten(),
        )
        .filter_map(|entity| {
            Some((
                entity_id(entity)?,
                (coordinate(entity.get("x"))?, coordinate(entity.get("y"))?),
            ))
        })
        .collect::<std::collections::HashMap<_, _>>();
    let Some(render_entities) = render.get_mut("entities").and_then(Value::as_array_mut) else {
        return false;
    };
    render_entities
        .retain(|entity| entity_id(entity).is_some_and(|id| positions.contains_key(&id)));
    render_entities.iter_mut().all(|entity| {
        entity_id(entity)
            .and_then(|id| positions.get(&id).copied())
            .is_some_and(|position| set_render_position(entity, position))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authoritative_packets_patch_exact_models_and_render_without_prediction() {
        let _guard = LIVE_ENTITY_TEST_LOCK.lock().unwrap();
        clear();
        assert!(install_models(
            r#"{"entities":[{"objectId":"42","kind":"selfPlayer","name":"Self","x":300,"y":630,"level":7,"direction":"Down"},{"objectId":"43","kind":"monster","name":"Deer","x":301,"y":630,"level":null,"direction":"Left"}]}"#,
            9,
        ));
        assert!(install_render(
            r#"{"_nativeWorldRequest":9,"enabled":true,"centerX":300,"centerY":630,"entities":[{"objectId":"42","isSelf":true,"gridX":300,"gridY":630,"layers":[{"key":"42:body:0","left":480.0,"top":352.0}]},{"objectId":"43","isSelf":false,"gridX":301,"gridY":630,"layers":[{"key":"43:body:0","left":528.0,"top":352.0,"z":1000.0,"atlasRectKey":"left"}]}]}"#,
            r#"{"_nativeWorldRequest":9,"entities":[{"objectId":"43","prototype":{"kind":"monster","classKey":"","dead":false,"sprite":{"bodyLibrary":"Mon/01","hairLibrary":null,"weaponLibrary":null,"weaponLibrarySecondary":null,"altBodyLibrary":null,"altHairLibrary":null,"altWeaponLibrary":null,"altWeaponLibrarySecondary":null,"mountLibrary":null,"frameBaseOffset":0,"weaponFrameOffset":null,"altFrameBaseOffset":null,"altWeaponFrameOffset":null,"frameCount":4,"directionStride":4,"mountFrameOffset":null}},"directionLayers":{"Left":[{"key":"43:body:0","left":528.0,"top":352.0,"z":1000.0,"atlasRectKey":"left"}],"DownRight":[{"key":"43:body:0","left":528.0,"top":352.0,"z":1000.0,"atlasRectKey":"down-right"}]}}]}"#,
        ));
        let LiveEntityPacketOutcome::Applied {
            models,
            render,
            presentation_event,
        } = apply_packet(
            r#"{"type":"packet","packet":"ObjectWalk","payload":{"objectId":43,"x":302,"y":631,"direction":"DownRight"}}"#,
        )
        else {
            panic!("movement should apply");
        };
        let presentation: Value =
            serde_json::from_str(presentation_event.as_deref().expect("remote motion event"))
                .unwrap();
        assert_eq!(presentation["type"], "remoteMotion");
        assert_eq!(presentation["packet"], "ObjectWalk");
        assert_eq!(presentation["objectId"], "43");
        assert_eq!(presentation["fromX"], 301);
        assert_eq!(presentation["fromY"], 630);
        assert_eq!(presentation["toX"], 302);
        assert_eq!(presentation["toY"], 631);
        assert_eq!(presentation["direction"], "DownRight");
        assert_eq!(presentation["mode"], "walk");
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["entities"][1]["x"], 302);
        assert_eq!(models["entities"][1]["y"], 631);
        assert_eq!(models["entities"][1]["direction"], "DownRight");
        let render: Value = serde_json::from_str(render.as_deref().unwrap()).unwrap();
        assert_eq!(render["entities"][1]["gridX"], 302);
        assert_eq!(render["entities"][1]["layers"][0]["left"], 576.0);
        assert_eq!(render["entities"][1]["layers"][0]["top"], 384.0);
        assert_eq!(render["entities"][1]["layers"][0]["z"], 2300.0);
        assert_eq!(
            render["entities"][1]["layers"][0]["atlasRectKey"],
            "down-right"
        );
        assert!(render["entities"][1].get("directionLayers").is_none());
        assert!(render["entities"][1].get("prototype").is_none());

        let LiveEntityPacketOutcome::Applied { models, render, .. } = apply_packet(
            r#"{"type":"packet","packet":"NewMonsterInfo","payload":{"info":{"objectId":78,"name":"Deer Two","location":{"x":299,"y":629},"direction":"Left","sprite":{"bodyLibrary":"Mon/01"}}}}"#,
        ) else {
            panic!("matching packet actor should apply");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert!(models["entities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entity| entity["objectId"] == "78"));
        let render: Value = serde_json::from_str(render.as_deref().unwrap()).unwrap();
        let actor = render["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entity| entity["objectId"] == "78")
            .expect("matching packet actor should be rendered");
        assert_eq!(actor["isSelf"], false);
        assert_eq!(actor["gridX"], 299);
        assert_eq!(actor["gridY"], 629);
        assert_eq!(actor["layers"][0]["key"], "78:body:0");
        assert_eq!(actor["layers"][0]["atlasRectKey"], "left");
        assert!(actor.get("directionLayers").is_none());
        assert!(actor.get("prototype").is_none());
        assert!(matches!(
            apply_packet(r#"{"type":"packet","packet":"ObjectRemove","payload":{"objectId":78}}"#),
            LiveEntityPacketOutcome::Applied { .. }
        ));

        assert!(matches!(
            apply_packet(r#"{"type":"packet","packet":"ObjectRemove","payload":{"objectId":43}}"#),
            LiveEntityPacketOutcome::Applied { .. }
        ));
        assert_eq!(
            apply_packet(
                r#"{"type":"packet","packet":"ObjectWalk","payload":{"objectId":43,"x":1,"y":1}}"#
            ),
            LiveEntityPacketOutcome::Ignored
        );
        let LiveEntityPacketOutcome::Applied { models, .. } = apply_packet(
            r#"{"type":"packet","packet":"NewMonsterInfo","payload":{"info":{"objectId":77,"name":"Hen","location":{"x":299,"y":629},"direction":"Left"}}}"#,
        ) else {
            panic!("spawn alias should apply");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["entities"][1]["objectId"], "77");
        assert_eq!(models["entities"][1]["kind"], "monster");

        let LiveEntityPacketOutcome::Applied { models, .. } = apply_packet(
            r#"{"type":"packet","packet":"ObjectTurn","payload":{"objectId":77,"x":299,"y":629,"direction":"Up"}}"#,
        ) else {
            panic!("turn should apply");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["entities"][1]["direction"], "Up");

        let LiveEntityPacketOutcome::Applied { models, .. } = apply_packet(
            r#"{"type":"packet","packet":"ObjectPlayer","payload":{"objectId":42,"name":"Self Refresh","location":{"x":301,"y":630},"direction":"Right"}}"#,
        ) else {
            panic!("self refresh should apply");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["entities"][0]["kind"], "selfPlayer");
        assert_eq!(models["entities"][0]["direction"], "Right");
        assert_eq!(apply_packet("not-json"), LiveEntityPacketOutcome::Rejected);
        clear();
    }

    #[test]
    fn remote_run_backstep_turn_and_remove_emit_authoritative_presentation_events() {
        let _guard = LIVE_ENTITY_TEST_LOCK.lock().unwrap();
        clear();
        assert!(install_models(
            r#"{"entities":[{"objectId":"42","kind":"selfPlayer","name":"Self","x":300,"y":630,"direction":"Down"},{"objectId":"43","kind":"monster","name":"Deer","x":301,"y":630,"direction":"Right"}]}"#,
            10,
        ));

        let LiveEntityPacketOutcome::Applied {
            presentation_event: Some(run),
            ..
        } = apply_packet_at(
            r#"{"type":"packet","packet":"ObjectRun","payload":{"objectId":43,"x":303,"y":630,"direction":"Right"}}"#,
            100,
        )
        else {
            panic!("run should apply");
        };
        let run: Value = serde_json::from_str(&run).unwrap();
        assert_eq!(run["atMs"], 100);
        assert_eq!(run["fromX"], 301);
        assert_eq!(run["toX"], 303);
        assert_eq!(run["mode"], "run");

        let LiveEntityPacketOutcome::Applied {
            presentation_event: Some(backstep),
            ..
        } = apply_packet_at(
            r#"{"type":"packet","packet":"ObjectBackStep","payload":{"objectId":43,"x":302,"y":630,"direction":"Right","distance":1}}"#,
            200,
        )
        else {
            panic!("backstep should apply");
        };
        let backstep: Value = serde_json::from_str(&backstep).unwrap();
        assert_eq!(backstep["packet"], "ObjectBackStep");
        assert_eq!(backstep["fromX"], 303);
        assert_eq!(backstep["toX"], 302);
        assert_eq!(backstep["mode"], "backstep");

        let LiveEntityPacketOutcome::Applied {
            presentation_event: Some(turn),
            ..
        } = apply_packet_at(
            r#"{"type":"packet","packet":"ObjectTurn","payload":{"objectId":43,"x":302,"y":630,"direction":"Up"}}"#,
            250,
        )
        else {
            panic!("turn should apply");
        };
        let turn: Value = serde_json::from_str(&turn).unwrap();
        assert_eq!(turn["fromX"], 302);
        assert_eq!(turn["toX"], 302);
        assert_eq!(turn["direction"], "Up");
        assert_eq!(turn["mode"], "turn");

        let LiveEntityPacketOutcome::Applied {
            presentation_event: Some(remove),
            ..
        } = apply_packet_at(
            r#"{"type":"packet","packet":"ObjectRemove","payload":{"objectId":43}}"#,
            300,
        )
        else {
            panic!("remove should apply");
        };
        let remove: Value = serde_json::from_str(&remove).unwrap();
        assert_eq!(remove["type"], "remoteRemove");
        assert_eq!(remove["objectId"], "43");

        let LiveEntityPacketOutcome::Applied {
            presentation_event, ..
        } = apply_packet_at(
            r#"{"type":"packet","packet":"ObjectWalk","payload":{"objectId":42,"x":301,"y":630,"direction":"Right"}}"#,
            400,
        )
        else {
            panic!("self movement model update should apply");
        };
        assert!(presentation_event.is_none());
        clear();
    }

    #[test]
    fn authoritative_identity_packets_update_visible_and_hidden_actor_metadata() {
        let _guard = LIVE_ENTITY_TEST_LOCK.lock().unwrap();
        clear();
        assert!(install_models(
            r#"{"entities":[{"objectId":"42","kind":"selfPlayer","name":"Self","x":300,"y":630},{"objectId":"43","kind":"player","name":"Old name","guildName":"Old guild","nameColourArgb":-1,"x":301,"y":630}]}"#,
            12,
        ));

        let LiveEntityPacketOutcome::Applied { models, render, .. } = apply_packet(
            r#"{"type":"packet","packet":"ObjectName","payload":{"objectId":43,"name":"Renamed by server"}}"#,
        ) else {
            panic!("name update should apply");
        };
        assert!(render.is_none());
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["entities"][1]["name"], "Renamed by server");

        let LiveEntityPacketOutcome::Applied { models, .. } = apply_packet(
            r#"{"type":"packet","packet":"ObjectColourChanged","payload":{"objectId":43,"nameColourArgb":-65281}}"#,
        ) else {
            panic!("name-colour update should apply");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["entities"][1]["nameColourArgb"], -65281);

        assert!(matches!(
            apply_packet(r#"{"type":"packet","packet":"ObjectHide","payload":{"objectId":43}}"#),
            LiveEntityPacketOutcome::Applied { .. }
        ));
        assert!(matches!(
            apply_packet(
                r#"{"type":"packet","packet":"ObjectGuildNameChanged","payload":{"objectId":43,"guildName":"Hidden update"}}"#
            ),
            LiveEntityPacketOutcome::Applied { .. }
        ));
        let LiveEntityPacketOutcome::Applied { models, .. } =
            apply_packet(r#"{"type":"packet","packet":"ObjectShow","payload":{"objectId":43}}"#)
        else {
            panic!("hidden actor should restore");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["entities"][1]["guildName"], "Hidden update");

        let LiveEntityPacketOutcome::Applied { models, .. } = apply_packet(
            r#"{"type":"packet","packet":"ObjectGuildNameChanged","payload":{"objectId":43,"guildName":""}}"#,
        ) else {
            panic!("empty authoritative guild should clear the guild line");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert!(models["entities"][1]["guildName"].is_null());
        assert_eq!(
            apply_packet(
                r#"{"type":"packet","packet":"ObjectGuildNameChanged","payload":{"objectId":43,"guildName":""}}"#
            ),
            LiveEntityPacketOutcome::Ignored
        );

        for invalid in [
            r#"{"type":"packet","packet":"ObjectName","payload":{"objectId":43,"name":""}}"#,
            r#"{"type":"packet","packet":"ObjectColourChanged","payload":{"objectId":43,"nameColourArgb":2147483648}}"#,
            r#"{"type":"packet","packet":"ObjectGuildNameChanged","payload":{"objectId":43}}"#,
        ] {
            assert_eq!(apply_packet(invalid), LiveEntityPacketOutcome::Rejected);
        }
        assert_eq!(
            apply_packet(
                r#"{"type":"packet","packet":"ObjectName","payload":{"objectId":99,"name":"Unknown"}}"#
            ),
            LiveEntityPacketOutcome::Ignored
        );
        clear();
    }

    #[test]
    fn authoritative_poison_updates_visible_hidden_and_future_actor_layers() {
        let _guard = LIVE_ENTITY_TEST_LOCK.lock().unwrap();
        clear();
        assert!(install_models(
            r#"{"entities":[{"objectId":"42","kind":"selfPlayer","name":"Self","x":300,"y":630},{"objectId":"43","kind":"player","name":"Remote","x":301,"y":630,"poison":0}],"groundDrops":[{"objectId":"50","name":"Potion","x":302,"y":630,"quantity":1,"image":0,"dropKind":"item"}]}"#,
            13,
        ));
        assert!(install_render(
            r#"{"_nativeWorldRequest":13,"enabled":true,"centerX":300,"centerY":630,"entities":[{"objectId":"42","isSelf":true,"gridX":300,"gridY":630,"layers":[]},{"objectId":"43","isSelf":false,"gridX":301,"gridY":630,"layers":[{"key":"43:body:0","left":528.0,"top":352.0,"opacity":1.0}]},{"objectId":"50","isSelf":false,"gridX":302,"gridY":630,"layers":[{"key":"50:ground-item","left":576.0,"top":352.0,"opacity":1.0}]}]}"#,
            r#"{"_nativeWorldRequest":13,"entities":[{"objectId":"43","prototype":{"kind":"player","classKey":"warrior","dead":false,"sprite":{"bodyLibrary":"CArmour/00"}},"directionLayers":{"Down":[{"key":"43:body:0","left":528.0,"top":352.0,"opacity":1.0}]},"actionLayers":{"attack1:Down":{"intervalMs":100,"frames":[[{"key":"43:body:0","left":528.0,"top":352.0,"opacity":1.0}]]}}}]}"#,
        ));

        let LiveEntityPacketOutcome::Applied { models, render, .. } = apply_packet(
            r#"{"type":"packet","packet":"ObjectPoisoned","payload":{"objectId":43,"poison":8}}"#,
        ) else {
            panic!("poison should apply");
        };
        assert_eq!(
            serde_json::from_str::<Value>(&models).unwrap()["entities"][1]["poison"],
            8
        );
        let render: Value = serde_json::from_str(render.as_deref().unwrap()).unwrap();
        assert_eq!(
            render["entities"][1]["layers"][0]["tintArgb"],
            0xFF00_00FF_u32 as i32
        );

        let LiveEntityPacketOutcome::Applied { render, .. } = apply_packet_at(
            r#"{"type":"packet","packet":"ObjectAttack","payload":{"objectId":43,"x":301,"y":630,"direction":"Down"}}"#,
            100,
        ) else {
            panic!("future action pose should apply");
        };
        let render: Value = serde_json::from_str(render.as_deref().unwrap()).unwrap();
        assert_eq!(
            render["entities"][1]["layers"][0]["tintArgb"],
            0xFF00_00FF_u32 as i32
        );

        assert!(matches!(
            apply_packet(r#"{"type":"packet","packet":"ObjectHide","payload":{"objectId":43}}"#),
            LiveEntityPacketOutcome::Applied { .. }
        ));
        assert!(matches!(
            apply_packet(
                r#"{"type":"packet","packet":"ObjectPoisoned","payload":{"objectId":43,"poison":64}}"#
            ),
            LiveEntityPacketOutcome::Applied { render: None, .. }
        ));
        let LiveEntityPacketOutcome::Applied { models, render, .. } =
            apply_packet(r#"{"type":"packet","packet":"ObjectShow","payload":{"objectId":43}}"#)
        else {
            panic!("hidden poisoned actor should restore");
        };
        assert_eq!(
            serde_json::from_str::<Value>(&models).unwrap()["entities"][1]["poison"],
            64
        );
        let render: Value = serde_json::from_str(render.as_deref().unwrap()).unwrap();
        assert_eq!(
            render["entities"][2]["layers"][0]["tintArgb"],
            0xFFFF_A500_u32 as i32
        );

        let LiveEntityPacketOutcome::Applied { models, render, .. } = apply_packet(
            r#"{"type":"packet","packet":"ObjectPoisoned","payload":{"objectId":43,"poison":0}}"#,
        ) else {
            panic!("poison clear should apply");
        };
        assert_eq!(
            serde_json::from_str::<Value>(&models).unwrap()["entities"][1]["poison"],
            0
        );
        let render: Value = serde_json::from_str(render.as_deref().unwrap()).unwrap();
        assert!(render["entities"][2]["layers"][0].get("tintArgb").is_none());
        assert_eq!(
            apply_packet(
                r#"{"type":"packet","packet":"ObjectPoisoned","payload":{"objectId":43,"poison":0}}"#
            ),
            LiveEntityPacketOutcome::Ignored
        );
        for invalid in [
            r#"{"type":"packet","packet":"ObjectPoisoned","payload":{"objectId":43,"poison":-1}}"#,
            r#"{"type":"packet","packet":"ObjectPoisoned","payload":{"objectId":43,"poison":65536}}"#,
        ] {
            assert_eq!(apply_packet(invalid), LiveEntityPacketOutcome::Rejected);
        }
        assert_eq!(
            apply_packet(
                r#"{"type":"packet","packet":"ObjectPoisoned","payload":{"objectId":99,"poison":8}}"#
            ),
            LiveEntityPacketOutcome::Ignored
        );
        assert_eq!(
            apply_packet(
                r#"{"type":"packet","packet":"ObjectPoisoned","payload":{"objectId":50,"poison":8}}"#
            ),
            LiveEntityPacketOutcome::Ignored,
            "actor status packets must not mutate a same-id ground-render entry"
        );
        clear();
    }

    #[test]
    fn crystal_poison_draw_colour_precedence_is_exact() {
        for (poison, expected) in [
            (0, None),
            (1, Some(0xFF00_8000_u32 as i32)),
            (2, Some(0xFFFF_0000_u32 as i32)),
            (128, Some(0xFF8B_0000_u32 as i32)),
            (4, Some(0xFF80_0080_u32 as i32)),
            (16 | 1024, Some(0xFFFF_FF00_u32 as i32)),
            (512 | 16, Some(0xFFC7_1585_u32 as i32)),
            (8 | 512, Some(0xFF00_00FF_u32 as i32)),
            (32 | 8, Some(0xFF80_8080_u32 as i32)),
            (256 | 32, Some(0xFF80_8080_u32 as i32)),
            (64 | 32, Some(0xFFFF_A500_u32 as i32)),
        ] {
            assert_eq!(poison_tint_argb(poison), expected, "poison={poison}");
        }
    }

    #[test]
    fn authoritative_level_effect_flags_follow_existing_actor_identity_without_placeholders() {
        let _guard = LIVE_ENTITY_TEST_LOCK.lock().unwrap();
        clear();
        assert!(install_models(
            r#"{"entities":[{"objectId":"42","kind":"selfPlayer","name":"Self","x":300,"y":630},{"objectId":"43","kind":"player","name":"Remote","x":301,"y":630,"levelEffects":0}],"groundDrops":[{"objectId":"50","name":"Potion","x":302,"y":630,"quantity":1,"image":0,"dropKind":"item"}]}"#,
            14,
        ));

        let LiveEntityPacketOutcome::Applied { models, render, .. } = apply_packet(
            r#"{"type":"packet","packet":"ObjectLevelEffects","payload":{"objectId":43,"levelEffects":4}}"#,
        ) else {
            panic!("level effect flags should update an existing actor");
        };
        assert!(
            render.is_none(),
            "missing licensed frames must not draw a placeholder"
        );
        assert_eq!(
            serde_json::from_str::<Value>(&models).unwrap()["entities"][1]["levelEffects"],
            4
        );
        assert!(matches!(
            apply_packet(r#"{"type":"packet","packet":"ObjectHide","payload":{"objectId":43}}"#),
            LiveEntityPacketOutcome::Applied { .. }
        ));
        assert!(matches!(
            apply_packet(
                r#"{"type":"packet","packet":"ObjectLevelEffects","payload":{"objectId":43,"levelEffects":256}}"#
            ),
            LiveEntityPacketOutcome::Applied { render: None, .. }
        ));
        let LiveEntityPacketOutcome::Applied { models, .. } =
            apply_packet(r#"{"type":"packet","packet":"ObjectShow","payload":{"objectId":43}}"#)
        else {
            panic!("hidden actor should restore with packet-fresh flags");
        };
        assert_eq!(
            serde_json::from_str::<Value>(&models).unwrap()["entities"][1]["levelEffects"],
            256
        );
        assert_eq!(
            apply_packet(
                r#"{"type":"packet","packet":"ObjectLevelEffects","payload":{"objectId":43,"levelEffects":256}}"#
            ),
            LiveEntityPacketOutcome::Ignored
        );
        for invalid in [
            r#"{"type":"packet","packet":"ObjectLevelEffects","payload":{"objectId":43,"levelEffects":-1}}"#,
            r#"{"type":"packet","packet":"ObjectLevelEffects","payload":{"objectId":43,"levelEffects":65536}}"#,
        ] {
            assert_eq!(apply_packet(invalid), LiveEntityPacketOutcome::Rejected);
        }
        assert_eq!(
            apply_packet(
                r#"{"type":"packet","packet":"ObjectLevelEffects","payload":{"objectId":99,"levelEffects":1}}"#
            ),
            LiveEntityPacketOutcome::Ignored
        );
        assert_eq!(
            apply_packet(
                r#"{"type":"packet","packet":"ObjectLevelEffects","payload":{"objectId":50,"levelEffects":1}}"#
            ),
            LiveEntityPacketOutcome::Ignored
        );

        let spawn_outcome = apply_packet(
            r#"{"type":"packet","packet":"ObjectPlayer","payload":{"objectId":44,"name":"Spawned","location":{"x":302,"y":630},"levelEffects":8}}"#,
        );
        let LiveEntityPacketOutcome::Applied { models, .. } = spawn_outcome else {
            panic!("spawn should retain the authoritative initial flags: {spawn_outcome:?}");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert!(models["entities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entity| entity["objectId"] == "44" && entity["levelEffects"] == 8));
        clear();
    }

    #[test]
    fn object_hidden_keeps_actor_present_and_applies_crystal_half_opacity() {
        let _guard = LIVE_ENTITY_TEST_LOCK.lock().unwrap();
        clear();
        assert!(install_models(
            r#"{"entities":[{"objectId":"42","kind":"selfPlayer","name":"Self","x":300,"y":630},{"objectId":"43","kind":"player","name":"Remote","x":301,"y":630,"dead":false,"hidden":false}],"groundDrops":[{"objectId":"50","name":"Potion","x":302,"y":630,"quantity":1,"image":0,"dropKind":"item"}]}"#,
            18,
        ));
        assert!(install_render(
            r#"{"_nativeWorldRequest":18,"enabled":true,"centerX":300,"centerY":630,"entities":[{"objectId":"42","isSelf":true,"gridX":300,"gridY":630,"layers":[]},{"objectId":"43","isSelf":false,"dead":false,"hidden":false,"gridX":301,"gridY":630,"layers":[{"key":"43:body:0","left":528.0,"top":352.0,"opacity":1.0}]},{"objectId":"50","isSelf":false,"gridX":302,"gridY":630,"layers":[{"key":"50:ground-item","left":576.0,"top":352.0,"opacity":1.0}]}]}"#,
            r#"{"_nativeWorldRequest":18,"entities":[{"objectId":"43","prototype":{"kind":"player","classKey":"warrior","dead":false,"sprite":{"bodyLibrary":"CArmour/00"}},"directionLayers":{"Down":[{"key":"43:body:0","left":528.0,"top":352.0,"opacity":1.0}]},"actionLayers":{"attack1:Down":{"intervalMs":100,"frames":[[{"key":"43:body:0","left":528.0,"top":352.0,"opacity":1.0}]]}}}]}"#,
        ));

        let LiveEntityPacketOutcome::Applied { models, render, .. } = apply_packet(
            r#"{"type":"packet","packet":"ObjectHidden","payload":{"objectId":43,"hidden":true}}"#,
        ) else {
            panic!("ObjectHidden should apply to the retained actor");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["entities"][1]["hidden"], true);
        let render: Value = serde_json::from_str(render.as_deref().unwrap()).unwrap();
        assert_eq!(render["entities"][1]["hidden"], true);
        assert_eq!(render["entities"][1]["layers"][0]["opacity"], 0.5);

        let LiveEntityPacketOutcome::Applied { render, .. } = apply_packet_at(
            r#"{"type":"packet","packet":"ObjectAttack","payload":{"objectId":43,"x":301,"y":630,"direction":"Down","attackType":1}}"#,
            100,
        ) else {
            panic!("ObjectAttack should keep ObjectHidden opacity on action layers");
        };
        let render: Value = serde_json::from_str(render.as_deref().unwrap()).unwrap();
        assert_eq!(render["entities"][1]["layers"][0]["opacity"], 0.5);

        let LiveEntityPacketOutcome::Applied { render, .. } = apply_packet(
            r#"{"type":"packet","packet":"ObjectHealth","payload":{"objectId":43,"percent":0,"expire":0}}"#,
        ) else {
            panic!("dead state should compose while the actor remains hidden");
        };
        let render: Value = serde_json::from_str(render.as_deref().unwrap()).unwrap();
        assert_eq!(render["entities"][1]["layers"][0]["opacity"], 0.5);

        let LiveEntityPacketOutcome::Applied { render, .. } = apply_packet(
            r#"{"type":"packet","packet":"ObjectHidden","payload":{"objectId":43,"hidden":false}}"#,
        ) else {
            panic!("clearing ObjectHidden should restore the existing death fallback");
        };
        let render: Value = serde_json::from_str(render.as_deref().unwrap()).unwrap();
        assert_eq!(render["entities"][1]["hidden"], false);
        assert_eq!(render["entities"][1]["layers"][0]["opacity"], 0.45);
        assert_eq!(
            apply_packet(
                r#"{"type":"packet","packet":"ObjectHidden","payload":{"objectId":43,"hidden":false}}"#
            ),
            LiveEntityPacketOutcome::Ignored
        );

        assert!(matches!(
            apply_packet(r#"{"type":"packet","packet":"ObjectHide","payload":{"objectId":43}}"#),
            LiveEntityPacketOutcome::Applied { .. }
        ));
        assert!(matches!(
            apply_packet(
                r#"{"type":"packet","packet":"ObjectHidden","payload":{"objectId":43,"hidden":true}}"#
            ),
            LiveEntityPacketOutcome::Applied { render: None, .. }
        ));
        let LiveEntityPacketOutcome::Applied { models, render, .. } =
            apply_packet(r#"{"type":"packet","packet":"ObjectShow","payload":{"objectId":43}}"#)
        else {
            panic!("ObjectShow should restore the lifecycle-hidden actor");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["entities"][1]["hidden"], true);
        let render: Value = serde_json::from_str(render.as_deref().unwrap()).unwrap();
        let restored = render["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entity| entity["objectId"] == "43")
            .unwrap();
        assert_eq!(restored["layers"][0]["opacity"], 0.5);

        for ignored in [
            r#"{"type":"packet","packet":"ObjectHidden","payload":{"objectId":99,"hidden":true}}"#,
            r#"{"type":"packet","packet":"ObjectHidden","payload":{"objectId":50,"hidden":true}}"#,
        ] {
            assert_eq!(apply_packet(ignored), LiveEntityPacketOutcome::Ignored);
        }
        for invalid in [
            r#"{"type":"packet","packet":"ObjectHidden","payload":{"objectId":43}}"#,
            r#"{"type":"packet","packet":"ObjectHidden","payload":{"objectId":43,"hidden":1}}"#,
        ] {
            assert_eq!(apply_packet(invalid), LiveEntityPacketOutcome::Rejected);
        }

        let LiveEntityPacketOutcome::Applied { models, .. } = apply_packet(
            r#"{"type":"packet","packet":"ObjectPlayer","payload":{"objectId":44,"name":"Hidden spawn","location":{"x":302,"y":630},"hidden":true}}"#,
        ) else {
            panic!("spawn should retain its initial ObjectHidden state");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert!(models["entities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entity| entity["objectId"] == "44" && entity["hidden"] == true));
        clear();
    }

    #[test]
    fn lifecycle_packets_keep_hidden_and_removed_objects_authoritative() {
        let _guard = LIVE_ENTITY_TEST_LOCK.lock().unwrap();
        clear();
        assert!(install_models(
            r#"{"entities":[{"objectId":"42","kind":"selfPlayer","name":"Self","x":300,"y":630,"level":7,"direction":"Down"},{"objectId":"43","kind":"monster","name":"Deer","x":301,"y":630,"level":null,"direction":"Left"}]}"#,
            19,
        ));
        assert!(install_render(
            r#"{"_nativeWorldRequest":19,"enabled":true,"centerX":300,"centerY":630,"entities":[{"objectId":"42","isSelf":true,"gridX":300,"gridY":630,"layers":[{"key":"42:body:0","left":480.0,"top":352.0,"opacity":1.0}]},{"objectId":"43","isSelf":false,"gridX":301,"gridY":630,"layers":[{"key":"43:body:0","left":528.0,"top":352.0,"opacity":1.0}]}]}"#,
            r#"{"_nativeWorldRequest":19,"entities":[{"objectId":"43","prototype":{"kind":"monster","classKey":"","dead":false,"sprite":{"bodyLibrary":"Mon/01","hairLibrary":null,"weaponLibrary":null,"weaponLibrarySecondary":null,"altBodyLibrary":null,"altHairLibrary":null,"altWeaponLibrary":null,"altWeaponLibrarySecondary":null,"mountLibrary":null,"frameBaseOffset":0,"weaponFrameOffset":null,"altFrameBaseOffset":null,"altWeaponFrameOffset":null,"frameCount":4,"directionStride":4,"mountFrameOffset":null}},"directionLayers":{"Left":[{"key":"43:body:0","left":528.0,"top":352.0,"opacity":1.0}]}}]}"#,
        ));

        let LiveEntityPacketOutcome::Applied { models, render, .. } = apply_packet(
            r#"{"type":"packet","packet":"ObjectHealth","payload":{"objectId":43,"percent":0,"expire":0}}"#,
        ) else {
            panic!("health should apply");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["entities"][1]["hpPercent"], 0);
        assert_eq!(models["entities"][1]["_healthPercent"], 0);
        assert_eq!(models["entities"][1]["_healthExpireSeconds"], 0);
        assert_eq!(models["entities"][1]["_healthGeneration"], 1);
        assert_eq!(models["entities"][1]["_healthRevision"], 1);
        assert_eq!(models["entities"][1]["dead"], true);
        let render: Value = serde_json::from_str(render.as_deref().unwrap()).unwrap();
        assert_eq!(render["entities"][1]["layers"][0]["opacity"], 0.45);

        let LiveEntityPacketOutcome::Applied { models, render, .. } =
            apply_packet(r#"{"type":"packet","packet":"ObjectHide","payload":{"objectId":43}}"#)
        else {
            panic!("hide should apply");
        };
        assert_eq!(
            serde_json::from_str::<Value>(&models).unwrap()["entities"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            serde_json::from_str::<Value>(render.as_deref().unwrap()).unwrap()["entities"]
                .as_array()
                .unwrap()
                .len(),
            1
        );

        // A later periodic snapshot cannot resurrect a hidden packet object.
        assert!(install_models(
            r#"{"entities":[{"objectId":"42","kind":"selfPlayer","name":"Self","x":300,"y":630,"level":7,"direction":"Down"},{"objectId":"43","kind":"monster","name":"Deer refreshed","x":302,"y":631,"level":null,"direction":"Right"}]}"#,
            20,
        ));
        assert!(matches!(
            apply_packet(r#"{"type":"packet","packet":"ObjectShow","payload":{"objectId":43}}"#),
            LiveEntityPacketOutcome::Applied { .. }
        ));
        let LiveEntityPacketOutcome::Applied { models, .. } = apply_packet(
            r#"{"type":"packet","packet":"ObjectDied","payload":{"objectId":43,"location":{"x":303,"y":632},"direction":"Up"}}"#,
        ) else {
            panic!("death should apply");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["entities"][1]["name"], "Deer refreshed");
        assert_eq!(models["entities"][1]["x"], 303);
        assert_eq!(models["entities"][1]["dead"], true);
        let LiveEntityPacketOutcome::Applied { models, .. } = apply_packet(
            r#"{"type":"packet","packet":"ObjectRevived","payload":{"objectId":43,"effect":true}}"#,
        ) else {
            panic!("revive should apply");
        };
        assert_eq!(
            serde_json::from_str::<Value>(&models).unwrap()["entities"][1]["dead"],
            false
        );

        assert!(matches!(
            apply_packet(
                r#"{"type":"packet","packet":"ObjectTeleportOut","payload":{"objectId":43,"effectType":1}}"#
            ),
            LiveEntityPacketOutcome::Applied { .. }
        ));
        assert!(matches!(
            apply_packet(
                r#"{"type":"packet","packet":"ObjectTeleportIn","payload":{"objectId":43,"effectType":1}}"#
            ),
            LiveEntityPacketOutcome::Applied { .. }
        ));
        assert!(matches!(
            apply_packet(r#"{"type":"packet","packet":"ObjectRemove","payload":{"objectId":43}}"#),
            LiveEntityPacketOutcome::Applied { .. }
        ));
        assert_eq!(
            apply_packet(r#"{"type":"packet","packet":"ObjectShow","payload":{"objectId":43}}"#),
            LiveEntityPacketOutcome::Ignored
        );
        assert!(install_models(
            r#"{"entities":[{"objectId":"42","kind":"selfPlayer","name":"Self","x":300,"y":630,"level":7,"direction":"Down"},{"objectId":"43","kind":"monster","name":"Stale Deer","x":304,"y":633,"level":null,"direction":"Right"}]}"#,
            21,
        ));
        assert!(matches!(
            apply_packet(
                r#"{"type":"packet","packet":"NewMonsterInfo","payload":{"objectId":43,"name":"Fresh Deer","location":{"x":305,"y":634},"direction":"Down"}}"#
            ),
            LiveEntityPacketOutcome::Applied { .. }
        ));

        let LiveEntityPacketOutcome::Applied { models, .. } = apply_packet(
            r#"{"type":"packet","packet":"Death","payload":{"location":{"x":299,"y":629},"direction":"Left"}}"#,
        ) else {
            panic!("self death should apply");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["entities"][0]["dead"], true);
        assert_eq!(models["entities"][0]["x"], 299);
        assert!(matches!(
            apply_packet(r#"{"type":"packet","packet":"Revived","payload":{}}"#),
            LiveEntityPacketOutcome::Applied { .. }
        ));
        assert_eq!(
            apply_packet(
                r#"{"type":"packet","packet":"ObjectHealth","payload":{"objectId":43,"percent":101}}"#
            ),
            LiveEntityPacketOutcome::Rejected
        );
        clear();
    }

    #[test]
    fn damage_indicator_retains_exact_event_and_only_real_hits_extend_health() {
        let _guard = LIVE_ENTITY_TEST_LOCK.lock().unwrap();
        clear();
        assert!(install_models(
            r#"{"entities":[{"objectId":"42","kind":"selfPlayer","name":"Self","x":300,"y":630},{"objectId":"43","kind":"monster","name":"Deer","x":301,"y":630}]}"#,
            23,
        ));
        assert!(matches!(
            apply_packet(
                r#"{"type":"packet","packet":"ObjectHealth","payload":{"objectId":43,"percent":73,"expire":0}}"#
            ),
            LiveEntityPacketOutcome::Applied { .. }
        ));

        let LiveEntityPacketOutcome::Applied { models, render, .. } = apply_packet(
            r#"{"type":"packet","packet":"DamageIndicator","payload":{"damage":12,"damageType":2,"objectId":43,"typed":true}}"#,
        ) else {
            panic!("critical damage should apply");
        };
        assert!(render.is_none());
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["entities"][1]["_healthHitRevision"], 1);
        assert_eq!(
            drain_damage_events(),
            vec![LiveDamageEvent {
                sequence: 1,
                object_id: 43,
                damage: 12,
                damage_type: 2,
            }]
        );

        let LiveEntityPacketOutcome::Applied { models, .. } = apply_packet(
            r#"{"type":"packet","packet":"DamageIndicator","payload":{"damage":0,"damageType":1,"objectId":43}}"#,
        ) else {
            panic!("miss should remain visible feedback");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["entities"][1]["_healthHitRevision"], 1);
        assert_eq!(drain_damage_events()[0].damage_type, 1);

        assert_eq!(
            apply_packet(
                r#"{"type":"packet","packet":"DamageIndicator","payload":{"damage":9,"objectId":43}}"#
            ),
            LiveEntityPacketOutcome::Rejected
        );
        assert!(drain_damage_events().is_empty());
        clear();
    }

    #[test]
    fn ground_item_and_gold_packets_update_models_and_exact_dnitems_render() {
        let _guard = LIVE_ENTITY_TEST_LOCK.lock().unwrap();
        clear();
        assert!(install_models(
            r#"{"entities":[{"objectId":"42","kind":"selfPlayer","name":"Self","x":300,"y":630,"level":7,"direction":"Down"}],"groundDrops":[]}"#,
            25,
        ));
        assert!(install_render(
            r#"{"_nativeWorldRequest":25,"enabled":true,"centerX":300,"centerY":630,"entities":[{"objectId":"42","isSelf":true,"gridX":300,"gridY":630,"layers":[{"key":"42:body:0","left":480.0,"top":352.0,"z":40960.0}]}]}"#,
            r#"{"_nativeWorldRequest":25,"entities":[],"groundItemFrames":{"0":{"width":20,"height":13},"114":{"width":16,"height":10}}}"#,
        ));

        let LiveEntityPacketOutcome::Applied { models, render, .. } = apply_packet(
            r#"{"type":"packet","packet":"ObjectItem","payload":{"objectId":50,"name":"Potion","nameColourArgb":-1,"location":{"x":301,"y":630},"image":0,"grade":0}}"#,
        ) else {
            panic!("item spawn should apply");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["groundDrops"][0]["objectId"], "50");
        assert_eq!(models["groundDrops"][0]["dropKind"], "item");
        let render: Value = serde_json::from_str(render.as_deref().unwrap()).unwrap();
        assert_eq!(render["entities"][1]["objectId"], "50");
        assert_eq!(
            render["entities"][1]["layers"][0]["path"],
            "/original-ui/DNItems/0.png"
        );
        assert_eq!(render["entities"][1]["layers"][0]["left"], 542.0);
        assert!(render.get("_nativeGroundItemFrames").is_none());

        let LiveEntityPacketOutcome::Applied { models, render, .. } = apply_packet(
            r#"{"type":"packet","packet":"ObjectGold","payload":{"objectId":51,"gold":250,"location":{"x":300,"y":631}}}"#,
        ) else {
            panic!("gold spawn should apply");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["groundDrops"][1]["quantity"], 250);
        assert_eq!(models["groundDrops"][1]["image"], 114);
        assert_eq!(models["groundDrops"][1]["dropKind"], "gold");
        let render: Value = serde_json::from_str(render.as_deref().unwrap()).unwrap();
        assert_eq!(
            render["entities"][2]["layers"][0]["path"],
            "/original-ui/DNItems/114.png"
        );

        let LiveEntityPacketOutcome::Applied { models, render, .. } =
            apply_packet(r#"{"type":"packet","packet":"ObjectRemove","payload":{"objectId":50}}"#)
        else {
            panic!("authoritative removal should clear the item");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["groundDrops"].as_array().unwrap().len(), 1);
        let render: Value = serde_json::from_str(render.as_deref().unwrap()).unwrap();
        assert!(render["entities"]
            .as_array()
            .unwrap()
            .iter()
            .all(|entity| entity["objectId"] != "50"));
        assert!(install_models(
            r#"{"entities":[{"objectId":"42","kind":"selfPlayer","name":"Self","x":300,"y":630,"level":7,"direction":"Down"}],"groundDrops":[{"objectId":"50","name":"Stale potion","nameColourArgb":-1,"x":301,"y":630,"quantity":1,"image":0,"dropKind":"item"},{"objectId":"51","name":"Gold","nameColourArgb":-1,"x":300,"y":631,"quantity":250,"image":114,"dropKind":"gold"}]}"#,
            26,
        ));
        let cache = LIVE_ENTITIES
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert!(cache
            .models
            .as_ref()
            .is_some_and(|models| models["groundDrops"]
                .as_array()
                .is_some_and(|drops| drops.len() == 1 && drops[0]["objectId"] == "51")));
        drop(cache);
        clear();
    }

    #[test]
    fn packet_action_frames_advance_and_settle_without_another_snapshot() {
        let _guard = LIVE_ENTITY_TEST_LOCK.lock().unwrap();
        clear();
        assert!(install_models(
            r#"{"entities":[{"objectId":"42","kind":"selfPlayer","name":"Self","x":300,"y":630,"level":7,"direction":"Down"},{"objectId":"43","kind":"monster","name":"Deer","x":301,"y":630,"level":null,"direction":"Down"}]}"#,
            29,
        ));
        assert!(install_render(
            r#"{"_nativeWorldRequest":29,"enabled":true,"centerX":300,"centerY":630,"entities":[{"objectId":"42","isSelf":true,"gridX":300,"gridY":630,"layers":[{"key":"42:body:0","left":480.0,"top":352.0,"atlasRectKey":"self-standing"}]},{"objectId":"43","isSelf":false,"gridX":301,"gridY":630,"layers":[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"standing"}]}]}"#,
            r#"{"_nativeWorldRequest":29,"entities":[{"objectId":"43","prototype":{},"directionLayers":{"Down":[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"standing"}]},"actionLayers":{"attack2:Down":{"intervalMs":100,"frames":[[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"attack-0"}],[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"attack-1"}]]},"walking:Down":{"intervalMs":100,"frames":[[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"walk-0"}],[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"walk-1"}]]}}}]}"#,
        ));

        let LiveEntityPacketOutcome::Applied { models, render, .. } = apply_packet_at(
            r#"{"type":"packet","packet":"ObjectAttack","payload":{"objectId":43,"x":301,"y":630,"attackType":1}}"#,
            1_000,
        ) else {
            panic!("attack should enter its first authoritative pose");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["entities"][1]["_nativeAnimationAction"], "attack2");
        let render: Value = serde_json::from_str(render.as_deref().unwrap()).unwrap();
        assert_eq!(
            render["entities"][1]["layers"][0]["atlasRectKey"],
            "attack-0"
        );
        assert!(render["entities"][1].get("actionLayers").is_none());
        assert!(poll_action_frame_at(1_099).is_none());

        let frame: Value =
            serde_json::from_str(&poll_action_frame_at(1_100).expect("second action frame"))
                .unwrap();
        assert_eq!(
            frame["entities"][1]["layers"][0]["atlasRectKey"],
            "attack-1"
        );
        let settled: Value =
            serde_json::from_str(&poll_action_frame_at(1_200).expect("standing settle")).unwrap();
        assert_eq!(
            settled["entities"][1]["layers"][0]["atlasRectKey"],
            "standing"
        );

        let LiveEntityPacketOutcome::Applied { models, render, .. } = apply_packet_at(
            r#"{"type":"packet","packet":"ObjectWalk","payload":{"objectId":43,"x":302,"y":631}}"#,
            1_300,
        ) else {
            panic!("walk should use the last authoritative direction");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["entities"][1]["_nativeAnimationAction"], "walking");
        assert_eq!(models["entities"][1]["x"], 302);
        assert_eq!(models["entities"][1]["y"], 631);
        let render: Value = serde_json::from_str(render.as_deref().unwrap()).unwrap();
        assert_eq!(render["entities"][1]["layers"][0]["atlasRectKey"], "walk-0");
        let frame: Value =
            serde_json::from_str(&poll_action_frame_at(1_400).expect("second walk frame")).unwrap();
        assert_eq!(frame["entities"][1]["layers"][0]["atlasRectKey"], "walk-1");
        let settled: Value =
            serde_json::from_str(&poll_action_frame_at(1_500).expect("walk settle")).unwrap();
        assert_eq!(
            settled["entities"][1]["layers"][0]["atlasRectKey"],
            "standing"
        );
        assert!(poll_action_frame_at(1_600).is_none());
        clear();
    }

    #[test]
    fn remote_object_pushed_uses_authoritative_endpoint_and_crystal_reverse_walk_pose() {
        let _guard = LIVE_ENTITY_TEST_LOCK.lock().unwrap();
        clear();
        assert!(install_models(
            r#"{"entities":[{"objectId":"42","kind":"selfPlayer","name":"Self","x":300,"y":630,"direction":"Down"},{"objectId":"43","kind":"player","name":"Remote","x":301,"y":630,"direction":"Right"}],"groundDrops":[{"objectId":"50","name":"Potion","x":302,"y":630,"image":0,"quantity":1,"dropKind":"item"}]}"#,
            31,
        ));
        assert!(install_render(
            r#"{"_nativeWorldRequest":31,"enabled":true,"centerX":300,"centerY":630,"entities":[{"objectId":"42","isSelf":true,"gridX":300,"gridY":630,"layers":[{"key":"42:body:0","left":480.0,"top":352.0,"atlasRectKey":"self-standing"}]},{"objectId":"43","isSelf":false,"gridX":301,"gridY":630,"layers":[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"standing-right"}]}]}"#,
            r#"{"_nativeWorldRequest":31,"entities":[{"objectId":"43","prototype":{},"directionLayers":{"Right":[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"standing-right"}]},"actionLayers":{"pushed:Right":{"intervalMs":100,"frames":[[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"push-5"}],[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"push-3"}],[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"push-1"}]]}}}],"groundItemFrames":{"0":{"width":20,"height":13}}}"#,
        ));

        let LiveEntityPacketOutcome::Applied {
            models,
            render: Some(render),
            presentation_event: Some(presentation),
        } = apply_packet_at(
            r#"{"type":"packet","packet":"ObjectPushed","payload":{"objectId":43,"location":{"x":300,"y":630},"direction":"Right"}}"#,
            1_000,
        )
        else {
            panic!("remote push should apply");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["entities"][1]["x"], 300);
        assert_eq!(models["entities"][1]["y"], 630);
        assert_eq!(models["entities"][1]["direction"], "Right");
        assert_eq!(models["entities"][1]["_nativeAnimationAction"], "pushed");
        let render: Value = serde_json::from_str(&render).unwrap();
        assert_eq!(render["entities"][1]["gridX"], 300);
        assert_eq!(render["entities"][1]["layers"][0]["atlasRectKey"], "push-5");
        let presentation: Value = serde_json::from_str(&presentation).unwrap();
        assert_eq!(presentation["packet"], "ObjectPushed");
        assert_eq!(presentation["fromX"], 301);
        assert_eq!(presentation["toX"], 300);
        assert_eq!(presentation["mode"], "pushed");
        assert_eq!(presentation["phaseCount"], 3);

        let second: Value =
            serde_json::from_str(&poll_action_frame_at(1_100).expect("second pushed frame"))
                .unwrap();
        assert_eq!(second["entities"][1]["layers"][0]["atlasRectKey"], "push-3");
        let third: Value =
            serde_json::from_str(&poll_action_frame_at(1_200).expect("third pushed frame"))
                .unwrap();
        assert_eq!(third["entities"][1]["layers"][0]["atlasRectKey"], "push-1");
        let settled: Value =
            serde_json::from_str(&poll_action_frame_at(1_300).expect("pushed settle")).unwrap();
        assert_eq!(
            settled["entities"][1]["layers"][0]["atlasRectKey"],
            "standing-right"
        );

        assert_eq!(
            apply_packet_at(
                r#"{"type":"packet","packet":"ObjectPushed","payload":{"objectId":42,"location":{"x":299,"y":630},"direction":"Left"}}"#,
                1_400,
            ),
            LiveEntityPacketOutcome::Ignored
        );
        {
            let cache = LIVE_ENTITIES
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let models = cache.models.as_ref().unwrap();
            assert_eq!(models["entities"][0]["x"], 300);
            assert_eq!(models["entities"][0]["direction"], "Down");
        }
        assert!(matches!(
            apply_packet_at(
                r#"{"type":"packet","packet":"ObjectRemove","payload":{"objectId":43}}"#,
                1_450,
            ),
            LiveEntityPacketOutcome::Applied { .. }
        ));
        for packet in [
            r#"{"type":"packet","packet":"ObjectPushed","payload":{"objectId":43,"location":{"x":299,"y":630},"direction":"Left"}}"#,
            r#"{"type":"packet","packet":"ObjectPushed","payload":{"objectId":99,"location":{"x":299,"y":630},"direction":"Left"}}"#,
            r#"{"type":"packet","packet":"ObjectPushed","payload":{"objectId":50,"location":{"x":299,"y":630},"direction":"Left"}}"#,
        ] {
            assert_eq!(
                apply_packet_at(packet, 1_500),
                LiveEntityPacketOutcome::Ignored
            );
        }
        for packet in [
            r#"{"type":"packet","packet":"ObjectPushed","payload":{"objectId":43,"direction":"Left"}}"#,
            r#"{"type":"packet","packet":"ObjectPushed","payload":{"objectId":43,"location":{"x":299,"y":630},"direction":"North"}}"#,
        ] {
            assert_eq!(
                apply_packet_at(packet, 1_600),
                LiveEntityPacketOutcome::Rejected
            );
        }
        clear();
    }

    #[test]
    fn remote_object_dash_alternates_running_halves_and_fail_cancels_motion() {
        let _guard = LIVE_ENTITY_TEST_LOCK.lock().unwrap();
        clear();
        assert!(install_models(
            r#"{"entities":[{"objectId":"42","kind":"selfPlayer","name":"Self","x":300,"y":630,"direction":"Down"},{"objectId":"43","kind":"player","name":"Remote","x":301,"y":630,"direction":"Right"}]}"#,
            32,
        ));
        assert!(install_render(
            r#"{"_nativeWorldRequest":32,"enabled":true,"centerX":300,"centerY":630,"entities":[{"objectId":"42","isSelf":true,"gridX":300,"gridY":630,"layers":[{"key":"42:body:0","left":480.0,"top":352.0,"atlasRectKey":"self-standing"}]},{"objectId":"43","isSelf":false,"gridX":301,"gridY":630,"layers":[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"standing-right"}]}]}"#,
            r#"{"_nativeWorldRequest":32,"entities":[{"objectId":"43","prototype":{},"directionLayers":{"Right":[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"standing-right"}]},"actionLayers":{"dashL:Right":{"intervalMs":100,"frames":[[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"run-0"}],[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"run-1"}],[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"run-2"}]]},"dashR:Right":{"intervalMs":100,"frames":[[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"run-3"}],[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"run-4"}],[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"run-5"}]]}}}]}"#,
        ));

        let LiveEntityPacketOutcome::Applied {
            render: Some(render),
            presentation_event: Some(first),
            ..
        } = apply_packet_at(
            r#"{"type":"packet","packet":"ObjectDash","payload":{"objectId":43,"location":{"x":302,"y":630},"direction":"Right"}}"#,
            1_000,
        )
        else {
            panic!("first remote dash should apply");
        };
        let render: Value = serde_json::from_str(&render).unwrap();
        assert_eq!(render["entities"][1]["layers"][0]["atlasRectKey"], "run-0");
        let first: Value = serde_json::from_str(&first).unwrap();
        assert_eq!(first["fromX"], 301);
        assert_eq!(first["toX"], 302);
        assert_eq!(first["mode"], "dashL");
        assert_eq!(first["phaseCount"], 3);

        let LiveEntityPacketOutcome::Applied {
            models,
            render: Some(render),
            presentation_event: Some(second),
        } = apply_packet_at(
            r#"{"type":"packet","packet":"ObjectDash","payload":{"objectId":43,"location":{"x":303,"y":630},"direction":"Right"}}"#,
            1_050,
        )
        else {
            panic!("queued remote dash should alternate");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["entities"][1]["x"], 303);
        assert_eq!(models["entities"][1]["_nativeAnimationAction"], "dashR");
        let render: Value = serde_json::from_str(&render).unwrap();
        assert_eq!(render["entities"][1]["layers"][0]["atlasRectKey"], "run-3");
        let second: Value = serde_json::from_str(&second).unwrap();
        assert_eq!(second["fromX"], 302);
        assert_eq!(second["toX"], 303);
        assert_eq!(second["mode"], "dashR");

        let frame: Value =
            serde_json::from_str(&poll_action_frame_at(1_150).expect("dashR middle frame"))
                .unwrap();
        assert_eq!(frame["entities"][1]["layers"][0]["atlasRectKey"], "run-4");
        let frame: Value =
            serde_json::from_str(&poll_action_frame_at(1_250).expect("dashR final frame")).unwrap();
        assert_eq!(frame["entities"][1]["layers"][0]["atlasRectKey"], "run-5");
        let settled: Value =
            serde_json::from_str(&poll_action_frame_at(1_350).expect("dash standing settle"))
                .unwrap();
        assert_eq!(
            settled["entities"][1]["layers"][0]["atlasRectKey"],
            "standing-right"
        );

        let LiveEntityPacketOutcome::Applied {
            presentation_event: Some(third),
            ..
        } = apply_packet_at(
            r#"{"type":"packet","packet":"ObjectDash","payload":{"objectId":43,"location":{"x":304,"y":630},"direction":"Right"}}"#,
            1_400,
        )
        else {
            panic!("dash after settle should restart with DashL");
        };
        assert_eq!(
            serde_json::from_str::<Value>(&third).unwrap()["mode"],
            "dashL"
        );
        let LiveEntityPacketOutcome::Applied {
            render,
            presentation_event: Some(failed),
            ..
        } = apply_packet_at(
            r#"{"type":"packet","packet":"ObjectDashFail","payload":{"objectId":43,"location":{"x":304,"y":630},"direction":"Right"}}"#,
            1_450,
        )
        else {
            panic!("dash failure should cancel an active dash even at the same endpoint");
        };
        let render: Value = serde_json::from_str(&render.expect("dash fail standing render"))
            .expect("valid render json");
        let remote = render["entities"]
            .as_array()
            .and_then(|entities| entities.iter().find(|entity| entity_id(entity) == Some(43)))
            .expect("remote player render");
        assert_eq!(remote["layers"][0]["atlasRectKey"], "standing-right");
        let failed: Value = serde_json::from_str(&failed).unwrap();
        assert_eq!(failed["mode"], "dashFail");
        assert_eq!(failed["phaseCount"], 1);
        assert!(poll_action_frame_at(1_500).is_none());

        for packet in [
            r#"{"type":"packet","packet":"ObjectDash","payload":{"objectId":42,"location":{"x":301,"y":630},"direction":"Right"}}"#,
            r#"{"type":"packet","packet":"ObjectDashFail","payload":{"objectId":42,"location":{"x":300,"y":630},"direction":"Down"}}"#,
        ] {
            assert_eq!(
                apply_packet_at(packet, 1_600),
                LiveEntityPacketOutcome::Ignored
            );
        }
        for packet in [
            r#"{"type":"packet","packet":"ObjectDash","payload":{"objectId":43,"direction":"Right"}}"#,
            r#"{"type":"packet","packet":"ObjectDashFail","payload":{"objectId":43,"location":{"x":304,"y":630},"direction":"North"}}"#,
        ] {
            assert_eq!(
                apply_packet_at(packet, 1_700),
                LiveEntityPacketOutcome::Rejected
            );
        }
        clear();
    }

    #[test]
    fn crystal_slow_halves_only_movement_action_cadence_and_rebases_mid_frame() {
        let _guard = LIVE_ENTITY_TEST_LOCK.lock().unwrap();
        clear();
        assert!(install_models(
            r#"{"entities":[{"objectId":"42","kind":"selfPlayer","name":"Self","x":300,"y":630},{"objectId":"43","kind":"player","name":"Remote","x":301,"y":630,"direction":"Down","poison":0}]}"#,
            30,
        ));
        assert!(install_render(
            r#"{"_nativeWorldRequest":30,"enabled":true,"centerX":300,"centerY":630,"entities":[{"objectId":"43","isSelf":false,"gridX":301,"gridY":630,"layers":[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"standing"}]}]}"#,
            r#"{"_nativeWorldRequest":30,"entities":[{"objectId":"43","prototype":{},"directionLayers":{"Down":[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"standing"}]},"actionLayers":{"walking:Down":{"intervalMs":100,"frames":[[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"walk-0"}],[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"walk-1"}],[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"walk-2"}]]},"attack1:Down":{"intervalMs":100,"frames":[[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"attack-0"}],[{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"attack-1"}]]}}}]}"#,
        ));

        assert!(matches!(
            apply_packet_at(
                r#"{"type":"packet","packet":"ObjectWalk","payload":{"objectId":43,"x":302,"y":630,"direction":"Down"}}"#,
                1_000,
            ),
            LiveEntityPacketOutcome::Applied { .. }
        ));
        assert!(poll_action_frame_at(1_049).is_none());
        assert!(matches!(
            apply_packet_at(
                r#"{"type":"packet","packet":"ObjectPoisoned","payload":{"objectId":43,"poison":4}}"#,
                1_050,
            ),
            LiveEntityPacketOutcome::Applied { .. }
        ));
        assert!(poll_action_frame_at(1_149).is_none());
        let slowed: Value =
            serde_json::from_str(&poll_action_frame_at(1_150).expect("slowed second frame"))
                .unwrap();
        assert_eq!(slowed["entities"][0]["layers"][0]["atlasRectKey"], "walk-1");

        assert!(matches!(
            apply_packet_at(
                r#"{"type":"packet","packet":"ObjectPoisoned","payload":{"objectId":43,"poison":0}}"#,
                1_200,
            ),
            LiveEntityPacketOutcome::Applied { .. }
        ));
        assert!(poll_action_frame_at(1_274).is_none());
        let restored: Value =
            serde_json::from_str(&poll_action_frame_at(1_275).expect("restored third frame"))
                .unwrap();
        assert_eq!(
            restored["entities"][0]["layers"][0]["atlasRectKey"],
            "walk-2"
        );

        assert!(matches!(
            apply_packet_at(
                r#"{"type":"packet","packet":"ObjectPoisoned","payload":{"objectId":43,"poison":4}}"#,
                1_400,
            ),
            LiveEntityPacketOutcome::Applied { .. }
        ));
        assert!(matches!(
            apply_packet_at(
                r#"{"type":"packet","packet":"ObjectAttack","payload":{"objectId":43,"x":302,"y":630,"direction":"Down"}}"#,
                1_500,
            ),
            LiveEntityPacketOutcome::Applied { .. }
        ));
        let attack: Value =
            serde_json::from_str(&poll_action_frame_at(1_600).expect("attack stays full speed"))
                .unwrap();
        assert_eq!(
            attack["entities"][0]["layers"][0]["atlasRectKey"],
            "attack-1"
        );
        assert!(matches!(
            apply_packet_at(
                r#"{"type":"packet","packet":"ObjectWalk","payload":{"objectId":43,"x":303,"y":630,"direction":"Down"}}"#,
                1_800,
            ),
            LiveEntityPacketOutcome::Applied { .. }
        ));
        assert!(poll_action_frame_at(1_999).is_none());
        let slow_from_start: Value = serde_json::from_str(
            &poll_action_frame_at(2_000).expect("existing slow status halves new movement"),
        )
        .unwrap();
        assert_eq!(
            slow_from_start["entities"][0]["layers"][0]["atlasRectKey"],
            "walk-1"
        );
        clear();
    }

    #[test]
    fn death_skeleton_and_revive_use_terminal_packet_poses() {
        let _guard = LIVE_ENTITY_TEST_LOCK.lock().unwrap();
        clear();
        assert!(install_models(
            r#"{"entities":[{"objectId":"42","kind":"selfPlayer","name":"Self","x":300,"y":630,"level":7,"direction":"Down"},{"objectId":"43","kind":"monster","name":"Deer","x":301,"y":630,"level":null,"direction":"Down"}]}"#,
            31,
        ));
        let render = json!({
            "_nativeWorldRequest": 31,
            "enabled": true,
            "centerX": 300,
            "centerY": 630,
            "entities": [{
                "objectId": "43",
                "isSelf": false,
                "gridX": 301,
                "gridY": 630,
                "layers": [{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"standing","opacity":1.0}]
            }]
        });
        let sidecar = json!({
            "_nativeWorldRequest": 31,
            "entities": [{
                "objectId": "43",
                "prototype": {},
                "directionLayers": {
                    "Down": [{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"standing","opacity":1.0}]
                },
                "actionLayers": {
                    "die:Down": {"intervalMs":100,"frames":[
                        [{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"die-0","opacity":1.0}],
                        [{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"die-1","opacity":1.0}]
                    ]},
                    "dead:Down": {"intervalMs":1000,"frames":[
                        [{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"dead","opacity":1.0}]
                    ]},
                    "skeleton:Down": {"intervalMs":1000,"frames":[
                        [{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"skeleton","opacity":1.0}]
                    ]},
                    "revive:Down": {"intervalMs":100,"frames":[
                        [{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"revive-0","opacity":0.45}],
                        [{"key":"43:body:0","left":528.0,"top":352.0,"atlasRectKey":"revive-1","opacity":0.45}]
                    ]}
                }
            }]
        });
        assert!(install_render(&render.to_string(), &sidecar.to_string()));

        let LiveEntityPacketOutcome::Applied { models, render, .. } = apply_packet_at(
            r#"{"type":"packet","packet":"ObjectDied","payload":{"objectId":43,"location":{"x":302,"y":631}}}"#,
            1_000,
        ) else {
            panic!("death should start the exact die sequence");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["entities"][1]["dead"], true);
        let render: Value = serde_json::from_str(render.as_deref().unwrap()).unwrap();
        assert_eq!(render["entities"][0]["layers"][0]["atlasRectKey"], "die-0");
        let frame: Value =
            serde_json::from_str(&poll_action_frame_at(1_100).expect("second die frame")).unwrap();
        assert_eq!(frame["entities"][0]["layers"][0]["atlasRectKey"], "die-1");
        let corpse: Value =
            serde_json::from_str(&poll_action_frame_at(1_200).expect("dead terminal pose"))
                .unwrap();
        assert_eq!(corpse["entities"][0]["layers"][0]["atlasRectKey"], "dead");
        assert!(poll_action_frame_at(1_300).is_none());

        let LiveEntityPacketOutcome::Applied { render, .. } = apply_packet_at(
            r#"{"type":"packet","packet":"ObjectHarvested","payload":{"objectId":43,"location":{"x":302,"y":631}}}"#,
            1_400,
        ) else {
            panic!("harvested corpse should enter its skeleton pose");
        };
        let skeleton: Value = serde_json::from_str(render.as_deref().unwrap()).unwrap();
        assert_eq!(
            skeleton["entities"][0]["layers"][0]["atlasRectKey"],
            "skeleton"
        );
        assert!(poll_action_frame_at(2_400).is_none());

        let LiveEntityPacketOutcome::Applied { models, render, .. } = apply_packet_at(
            r#"{"type":"packet","packet":"ObjectRevived","payload":{"objectId":43}}"#,
            2_500,
        ) else {
            panic!("revive should start from the retained authoritative pose");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["entities"][1]["dead"], false);
        let render: Value = serde_json::from_str(render.as_deref().unwrap()).unwrap();
        assert_eq!(
            render["entities"][0]["layers"][0]["atlasRectKey"],
            "revive-0"
        );
        assert_eq!(render["entities"][0]["layers"][0]["opacity"], 1.0);
        let frame: Value =
            serde_json::from_str(&poll_action_frame_at(2_600).expect("second revive frame"))
                .unwrap();
        assert_eq!(
            frame["entities"][0]["layers"][0]["atlasRectKey"],
            "revive-1"
        );
        let standing: Value =
            serde_json::from_str(&poll_action_frame_at(2_700).expect("revive settle")).unwrap();
        assert_eq!(
            standing["entities"][0]["layers"][0]["atlasRectKey"],
            "standing"
        );
        assert!(poll_action_frame_at(2_800).is_none());
        clear();
    }
}
