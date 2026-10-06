//! Android projection of authoritative ground drops into the shared pickup UI.
//!
//! The server owns object identity and pickup success. This adapter only sorts
//! the already-validated renderer-neutral snapshot around the authoritative
//! self-player position and exposes the same bounded model used by desktop.

use mir2_client_bevy::quest_model::{GroundPickupModel, RecentPickup};
use serde_json::Value;
use std::collections::VecDeque;

const MAX_MODEL_BYTES: usize = 2 * 1024 * 1024;
const MAX_WORLD_OBJECTS: usize = 8192;
const MAX_GROUND_PICKUPS: usize = 4;

pub(crate) fn project(models_json: &str) -> Option<GroundPickupModel> {
    if models_json.len() > MAX_MODEL_BYTES {
        return None;
    }
    let models: Value = serde_json::from_str(models_json).ok()?;
    let entities = models.get("entities")?.as_array()?;
    let drops = match models.get("groundDrops") {
        None => &[][..],
        Some(value) => value.as_array()?.as_slice(),
    };
    if entities
        .len()
        .checked_add(drops.len())
        .is_none_or(|count| count > MAX_WORLD_OBJECTS)
    {
        return None;
    }

    let mut self_positions = entities.iter().filter_map(|entity| {
        (entity.get("kind").and_then(Value::as_str) == Some("selfPlayer"))
            .then(|| Some((coordinate(entity.get("x")?)?, coordinate(entity.get("y")?)?)))?
    });
    let (player_x, player_y) = self_positions.next()?;
    if self_positions.next().is_some() {
        return None;
    }

    let mut pickups = drops
        .iter()
        .map(|drop| {
            let object_id = object_id(drop.get("objectId")?)?;
            let x = coordinate(drop.get("x")?)?;
            let y = coordinate(drop.get("y")?)?;
            let label = drop.get("name")?.as_str()?;
            let amount = drop
                .get("quantity")?
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .filter(|value| *value > 0)?;
            if label.is_empty() || label.chars().count() > 128 {
                return None;
            }
            let from_npc = match drop.get("sourceMonster") {
                None | Some(Value::Null) => None,
                Some(value) => {
                    let source = value.as_str()?;
                    if source.chars().count() > 128 {
                        return None;
                    }
                    (!source.trim().is_empty()).then(|| source.to_owned())
                }
            };
            Some((
                player_x.abs_diff(x).max(player_y.abs_diff(y)),
                RecentPickup {
                    object_id: Some(object_id),
                    key: format!("object:{object_id}"),
                    label: label.to_owned(),
                    amount,
                    from_npc,
                },
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    pickups.sort_by_key(|(distance, pickup)| (*distance, pickup.object_id.unwrap_or_default()));
    let recent = pickups
        .into_iter()
        .take(MAX_GROUND_PICKUPS)
        .map(|(_, pickup)| pickup)
        .collect::<VecDeque<_>>();
    Some(GroundPickupModel { recent })
}

fn object_id(value: &Value) -> Option<u32> {
    let object_id = value
        .as_str()
        .and_then(|value| value.parse::<u32>().ok())
        .or_else(|| value.as_u64().and_then(|value| u32::try_from(value).ok()))?;
    (object_id != 0).then_some(object_id)
}

fn coordinate(value: &Value) -> Option<i32> {
    i32::try_from(value.as_i64()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authoritative_drops_are_sorted_bounded_and_keep_server_identity() {
        let models = serde_json::json!({
            "entities": [
                {"objectId":"7","kind":"selfPlayer","name":"Self","x":100,"y":100},
                {"objectId":"8","kind":"monster","name":"Deer","x":101,"y":100}
            ],
            "groundDrops": [
                {"objectId":"50","name":"Far","x":104,"y":100,"quantity":1},
                {"objectId":"44","name":"Potion","x":101,"y":100,"quantity":2,"sourceMonster":"Deer"},
                {"objectId":"43","name":"Gold","x":100,"y":101,"quantity":250},
                {"objectId":"60","name":"Third","x":102,"y":100,"quantity":1},
                {"objectId":"61","name":"Fourth","x":103,"y":100,"quantity":1}
            ]
        });
        let projected = project(&models.to_string()).expect("valid authoritative pickups");
        assert_eq!(projected.recent.len(), 4);
        assert_eq!(projected.recent[0].object_id, Some(43));
        assert_eq!(projected.recent[1].object_id, Some(44));
        assert_eq!(projected.recent[1].compact_label(), "Potion x2");
        assert_eq!(projected.recent[1].from_npc.as_deref(), Some("Deer"));
        assert_eq!(projected.recent[3].object_id, Some(61));
    }

    #[test]
    fn malformed_or_ambiguous_authority_is_rejected_atomically() {
        for models in [
            serde_json::json!({"entities":[],"groundDrops":[]}),
            serde_json::json!({"entities":[
                {"kind":"selfPlayer","x":1,"y":1},
                {"kind":"selfPlayer","x":2,"y":2}
            ],"groundDrops":[]}),
            serde_json::json!({"entities":[{"kind":"selfPlayer","x":1,"y":1}],
                "groundDrops":[{"objectId":"5","name":"Bad","x":1,"y":1,"quantity":0}]}),
        ] {
            assert!(project(&models.to_string()).is_none());
        }
    }
}
