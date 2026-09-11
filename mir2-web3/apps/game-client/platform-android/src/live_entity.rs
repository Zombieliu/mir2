//! Packet-first Android entity projection over the last authoritative snapshot.
//!
//! This cache does not predict movement or invent actors. It applies only
//! server packets to the renderer-neutral entity model and, when available,
//! to the exact native entity-render state built from the same snapshot.

use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    sync::{LazyLock, Mutex},
    time::Instant,
};

const MAX_PACKET_BYTES: usize = 16 * 1024;
const MAX_RENDER_BYTES: usize = 64 * 1024 * 1024;
const MAX_ENTITIES: usize = 8192;
const MAX_ACTION_POSES: usize = 128;
const CELL_WIDTH: f64 = 48.0;
const CELL_HEIGHT: f64 = 32.0;

#[derive(Default)]
struct LiveEntityCache {
    request_id: u64,
    models: Option<Value>,
    render_request_id: u64,
    render: Option<Value>,
    hidden: HashMap<u32, HiddenEntity>,
    tombstones: HashSet<u32>,
    actions: HashMap<u32, ActiveAction>,
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
    },
    Ignored,
    Rejected,
}

pub(crate) fn clear() {
    *LIVE_ENTITIES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = LiveEntityCache::default();
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
    true
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
        | "ObjectRangeAttack" | "ObjectStruck" | "ObjectDashAttack" => object_id(body)
            .zip(location(body))
            .map(|(object_id, position)| EntityMutation::Action {
                object_id,
                position,
                direction: action_direction(body, &models, object_id),
                action: packet_action(packet, body, &models, object_id),
                started_ms: now_ms,
                life_state: None,
            }),
        "ObjectHealth" => object_id(body)
            .zip(percent(body))
            .map(|(object_id, percent)| EntityMutation::Health { object_id, percent }),
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
        | EntityMutation::Action { object_id, .. } => Some(*object_id),
        EntityMutation::Move(object_id, ..) => Some(*object_id),
        EntityMutation::Spawn(spawn) => entity_id(&spawn.model),
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
    match mutation {
        EntityMutation::Move(object_id, ..) => {
            actions.remove(object_id);
        }
        _ => {}
    }
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
            let model_changed = take_entity(models, *object_id).is_some();
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
            (model_changed || render_changed, render_changed)
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
    },
    Hide(u32),
    Show(u32),
    Remove(u32),
    Spawn(EntitySpawn),
}

#[derive(Debug)]
struct EntitySpawn {
    model: Value,
    prototype: Option<Value>,
    position: (i32, i32),
    direction: Option<String>,
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

fn percent(payload: &serde_json::Map<String, Value>) -> Option<u8> {
    u8::try_from(payload.get("percent")?.as_u64()?)
        .ok()
        .filter(|value| *value <= 100)
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
    let model = json!({
        "objectId": object_id.to_string(),
        "kind": kind,
        "name": name,
        "x": x,
        "y": y,
        "level": level,
        "direction": direction,
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
    if entities.is_empty() || entities.len() > MAX_ENTITIES {
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
        EntityMutation::Health { object_id, percent } => entities
            .iter_mut()
            .find(|entity| entity_id(entity) == Some(*object_id))
            .is_some_and(|entity| patch_model_health(entity, *percent)),
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
        EntityMutation::Hide(_) | EntityMutation::Show(_) | EntityMutation::Remove(_) => false,
    }
}

fn patch_model_health(entity: &mut Value, percent: u8) -> bool {
    let changed = entity.get("hpPercent").and_then(Value::as_u64) != Some(u64::from(percent));
    entity["hpPercent"] = json!(percent);
    patch_model_dead(entity, percent == 0) | changed
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
        EntityMutation::Health { object_id, percent } => entities
            .iter_mut()
            .find(|entity| entity_id(entity) == Some(*object_id))
            .is_some_and(|entity| patch_render_dead(entity, *percent == 0)),
        EntityMutation::Spawn(spawn) => {
            if let Some(existing) = entities
                .iter_mut()
                .find(|entity| entity_id(entity) == entity_id(&spawn.model))
            {
                return patch_render_transform(
                    existing,
                    spawn.position,
                    spawn.direction.as_deref(),
                );
            }
            center.is_some_and(|center| spawn_render_from_prototype(entities, spawn, center))
        }
        EntityMutation::Hide(_) | EntityMutation::Show(_) | EntityMutation::Remove(_) => false,
    }
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
    let Some(descriptor) = render
        .get("entities")
        .and_then(Value::as_array)
        .and_then(|entities| {
            entities
                .iter()
                .find(|entity| entity_id(entity) == Some(object_id))
        })
        .and_then(|entity| entity.get("actionLayers"))
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
        let phase = usize::try_from(elapsed / action.interval_ms).unwrap_or(usize::MAX);
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
    let opacity = if dead { 0.45 } else { 1.0 };
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
        let LiveEntityPacketOutcome::Applied { models, render } = apply_packet(
            r#"{"type":"packet","packet":"ObjectWalk","payload":{"objectId":43,"x":302,"y":631,"direction":"DownRight"}}"#,
        ) else {
            panic!("movement should apply");
        };
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

        let LiveEntityPacketOutcome::Applied { models, render } = apply_packet(
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

        let LiveEntityPacketOutcome::Applied { models, render } = apply_packet(
            r#"{"type":"packet","packet":"ObjectHealth","payload":{"objectId":43,"percent":0,"expire":0}}"#,
        ) else {
            panic!("health should apply");
        };
        let models: Value = serde_json::from_str(&models).unwrap();
        assert_eq!(models["entities"][1]["hpPercent"], 0);
        assert_eq!(models["entities"][1]["dead"], true);
        let render: Value = serde_json::from_str(render.as_deref().unwrap()).unwrap();
        assert_eq!(render["entities"][1]["layers"][0]["opacity"], 0.45);

        let LiveEntityPacketOutcome::Applied { models, render } =
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

        let LiveEntityPacketOutcome::Applied { models, render } = apply_packet_at(
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

        let LiveEntityPacketOutcome::Applied { models, render } = apply_packet_at(
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

        let LiveEntityPacketOutcome::Applied { models, render } = apply_packet_at(
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

        let LiveEntityPacketOutcome::Applied { models, render } = apply_packet_at(
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
