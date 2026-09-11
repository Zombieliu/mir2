//! Packet-first Android entity projection over the last authoritative snapshot.
//!
//! This cache does not predict movement or invent actors. It applies only
//! server packets to the renderer-neutral entity model and, when available,
//! to the exact native entity-render state built from the same snapshot.

use serde_json::{json, Value};
use std::{collections::HashSet, sync::Mutex};

const MAX_PACKET_BYTES: usize = 16 * 1024;
const MAX_ENTITIES: usize = 8192;
const CELL_WIDTH: f64 = 48.0;
const CELL_HEIGHT: f64 = 32.0;

#[derive(Default)]
struct LiveEntityCache {
    request_id: u64,
    models: Option<Value>,
    render_request_id: u64,
    render: Option<Value>,
}

static LIVE_ENTITIES: Mutex<LiveEntityCache> = Mutex::new(LiveEntityCache {
    request_id: 0,
    models: None,
    render_request_id: 0,
    render: None,
});

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
    let Ok(value) = serde_json::from_str::<Value>(json_text) else {
        return false;
    };
    if !valid_models(&value) {
        return false;
    }
    let mut cache = LIVE_ENTITIES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache.request_id = request_id;
    cache.models = Some(value);
    if cache.render_request_id != request_id {
        cache.render_request_id = 0;
        cache.render = None;
    }
    true
}

pub(crate) fn install_render(json_text: &str) -> bool {
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
    if !valid_render(&value) {
        return false;
    }
    let mut cache = LIVE_ENTITIES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
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

pub(crate) fn apply_packet(json_text: &str) -> LiveEntityPacketOutcome {
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
        "ObjectWalk" | "ObjectRun" | "ObjectBackStep" | "ObjectTurn" | "ObjectHarvest"
        | "ObjectHarvested" => object_id(body)
            .zip(location(body))
            .map(|(object_id, position)| {
                EntityMutation::Move(object_id, position, direction(body).map(str::to_owned))
            }),
        "ObjectAttack" | "ObjectStruck" | "ObjectDashAttack" => object_id(body)
            .zip(location(body))
            .map(|(object_id, position)| {
                EntityMutation::Move(object_id, position, direction(body).map(str::to_owned))
            }),
        "ObjectRemove" | "ObjectTeleportOut" => object_id(body).map(EntityMutation::Remove),
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
    let changed = apply_models(&mut models, &mutation);
    if !changed {
        cache.models = Some(models);
        return LiveEntityPacketOutcome::Ignored;
    }

    let render = if cache.render_request_id == request_id {
        if let Some(mut value) = cache.render.take() {
            let encoded = apply_render(&mut value, &mutation).then(|| value.to_string());
            cache.render = Some(value);
            encoded
        } else {
            None
        }
    } else {
        None
    };
    let encoded_models = models.to_string();
    cache.models = Some(models);
    LiveEntityPacketOutcome::Applied {
        models: encoded_models,
        render,
    }
}

#[derive(Debug)]
enum EntityMutation {
    MoveSelf((i32, i32), Option<String>),
    Move(u32, (i32, i32), Option<String>),
    Remove(u32),
    Spawn(Value),
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

fn spawn(payload: &serde_json::Map<String, Value>, kind: &str) -> Option<Value> {
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
    let direction = direction(payload);
    Some(json!({
        "objectId": object_id.to_string(),
        "kind": kind,
        "name": name,
        "x": x,
        "y": y,
        "level": level,
        "direction": direction,
    }))
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
                .is_some_and(|layers| {
                    layers.iter().all(|layer| {
                        layer
                            .get("left")
                            .and_then(Value::as_f64)
                            .is_some_and(f64::is_finite)
                            && layer
                                .get("top")
                                .and_then(Value::as_f64)
                                .is_some_and(f64::is_finite)
                    })
                })
    })
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
        EntityMutation::Remove(object_id) => {
            let previous = entities.len();
            entities.retain(|entity| entity_id(entity) != Some(*object_id));
            entities.len() != previous
        }
        EntityMutation::Spawn(entity) => {
            let object_id = entity_id(entity).expect("spawn was validated");
            if let Some(existing) = entities
                .iter_mut()
                .find(|existing| entity_id(existing) == Some(object_id))
            {
                *existing = entity.clone();
            } else if entities.len() < MAX_ENTITIES {
                entities.push(entity.clone());
            } else {
                return false;
            }
            true
        }
    }
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
    let Some(entities) = render.get_mut("entities").and_then(Value::as_array_mut) else {
        return false;
    };
    match mutation {
        EntityMutation::MoveSelf(position, _) => entities
            .iter_mut()
            .find(|entity| entity.get("isSelf").and_then(Value::as_bool) == Some(true))
            .is_some_and(|entity| set_render_position(entity, *position)),
        EntityMutation::Move(object_id, position, _) => entities
            .iter_mut()
            .find(|entity| entity_id(entity) == Some(*object_id))
            .is_some_and(|entity| set_render_position(entity, *position)),
        EntityMutation::Remove(object_id) => {
            entities.retain(|entity| entity_id(entity) != Some(*object_id));
            true
        }
        EntityMutation::Spawn(_) => true,
    }
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
    entity["gridX"] = json!(x);
    entity["gridY"] = json!(y);
    let Some(layers) = entity.get_mut("layers").and_then(Value::as_array_mut) else {
        return false;
    };
    for layer in layers {
        let Some(left) = layer.get("left").and_then(Value::as_f64) else {
            return false;
        };
        let Some(top) = layer.get("top").and_then(Value::as_f64) else {
            return false;
        };
        layer["left"] = json!(left + dx);
        layer["top"] = json!(top + dy);
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
        clear();
        assert!(install_models(
            r#"{"entities":[{"objectId":"42","kind":"selfPlayer","name":"Self","x":300,"y":630,"level":7,"direction":"Down"},{"objectId":"43","kind":"monster","name":"Deer","x":301,"y":630,"level":null,"direction":"Left"}]}"#,
            9,
        ));
        assert!(install_render(
            r#"{"_nativeWorldRequest":9,"enabled":true,"entities":[{"objectId":"42","isSelf":true,"gridX":300,"gridY":630,"layers":[{"left":480.0,"top":352.0}]},{"objectId":"43","isSelf":false,"gridX":301,"gridY":630,"layers":[{"left":528.0,"top":352.0}]}]}"#,
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
        assert_eq!(apply_packet("not-json"), LiveEntityPacketOutcome::Rejected);
        clear();
    }

    #[test]
    fn authoritative_spawn_aliases_and_turn_update_the_neutral_object_layer() {
        clear();
        assert!(install_models(
            r#"{"entities":[{"objectId":"42","kind":"selfPlayer","name":"Self","x":300,"y":630,"level":7,"direction":"Down"}]}"#,
            10,
        ));
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
        clear();
    }
}
