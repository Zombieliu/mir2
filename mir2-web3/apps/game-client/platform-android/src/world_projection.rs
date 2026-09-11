//! Wire-shape adaptation only. The server remains authoritative; rendering and
//! bootstrap acceptance are owned by the shared runtime, not this adapter.
use mir2_client_bevy::read_model::PlayerStats;
use serde_json::{json, Map, Value};
use std::collections::HashSet;

pub(crate) struct Projection {
    pub request_id: u64,
    pub world: String,
    pub ui: String,
    pub map: String,
    pub entities: String,
    pub scene: ProjectedScene,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectedScene {
    pub map_file_name: String,
    pub center_x: i32,
    pub center_y: i32,
    pub width: i32,
    pub height: i32,
}

pub(crate) fn project(raw: &str, map: &str, name: &str, x: u32, y: u32) -> Option<Projection> {
    let mut world: Value = serde_json::from_str(raw).ok()?;
    let owner = object_id(&world["playerObjectId"])?;
    if world["mapFileName"].as_str()? != map {
        return None;
    }
    let entities = world["entities"].as_array()?;
    if entities.is_empty() || entities.len() > 8192 {
        return None;
    }
    let mut ids = HashSet::new();
    let mut self_player = None;
    for entity in entities {
        let id = object_id(&entity["objectId"])?;
        if !ids.insert(id) {
            return None;
        }
        for key in ["x", "y"] {
            i32::try_from(entity[key].as_u64()?).ok()?;
        }
        if !matches!(
            entity["kind"].as_str()?,
            "selfPlayer" | "player" | "monster" | "npc"
        ) {
            return None;
        }
        entity["name"].as_str()?;
        if entity["kind"] == "selfPlayer" {
            if self_player.is_some()
                || id != owner
                || entity["name"] != name
                || entity["x"] != x
                || entity["y"] != y
            {
                return None;
            }
            self_player = Some(entity);
        }
    }
    let ground_drops = match world.get("groundDrops") {
        None | Some(Value::Null) => &[][..],
        Some(value) => value.as_array()?.as_slice(),
    };
    if entities.len().saturating_add(ground_drops.len()) > 8192 {
        return None;
    }
    for drop in ground_drops {
        let id = object_id(&drop["objectId"])?;
        if !ids.insert(id) {
            return None;
        }
        i32::try_from(drop["x"].as_u64()?).ok()?;
        i32::try_from(drop["y"].as_u64()?).ok()?;
        u16::try_from(drop["icon"].as_u64()?).ok()?;
        let quantity = u32::try_from(drop["quantity"].as_u64()?).ok()?;
        let name = drop["name"].as_str()?;
        if quantity == 0 || name.is_empty() || name.chars().count() > 128 {
            return None;
        }
        i32::try_from(drop["nameColourArgb"].as_i64()?).ok()?;
        if drop.get("sourceMonster").is_some_and(|value| {
            value
                .as_str()
                .is_none_or(|value| value.chars().count() > 128)
        }) {
            return None;
        }
        match drop.pointer("/loot/kind").and_then(Value::as_str)? {
            "gold" => {
                if drop
                    .pointer("/loot/amount")
                    .and_then(Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
                    != Some(quantity)
                {
                    return None;
                }
            }
            "inventoryItem" => {}
            _ => return None,
        }
    }
    let player = self_player?;
    let mut stats = Map::new();
    for (wire, field) in [
        ("playerHp", "hp"),
        ("playerMaxHp", "maxHp"),
        ("playerMp", "mp"),
        ("playerMaxMp", "maxMp"),
        ("gold", "gold"),
        ("credit", "credit"),
        ("playerCrystalStats", "crystalStats"),
        ("playerExperience", "experience"),
        ("playerMaxExperience", "maxExperience"),
        ("currentWeight", "currentWeight"),
        ("maxWeight", "maxWeight"),
        ("mapTitle", "mapName"),
        ("inSafeZone", "inSafeZone"),
    ] {
        if let Some(value) = world.get(wire).filter(|value| !value.is_null()) {
            stats.insert(field.into(), value.clone());
        }
    }
    for (wire, field) in [
        ("name", "name"),
        ("level", "level"),
        ("class", "className"),
        ("gender", "gender"),
        ("hair", "hair"),
        ("wingEffect", "wingEffect"),
        ("guildName", "guildName"),
        ("guildRankName", "guildRankName"),
    ] {
        if let Some(value) = player.get(wire).filter(|value| !value.is_null()) {
            stats.insert(field.into(), value.clone());
        }
    }
    // Deserialize through the actual shared HUD type, including integer ranges.
    let stats: PlayerStats = serde_json::from_value(Value::Object(stats)).ok()?;
    world["playerObjectId"] = json!(owner.to_string());
    world["selectedObjectId"] = match world.get("selectedObjectId") {
        None | Some(Value::Null) => Value::Null,
        Some(value) => json!(u32::try_from(value.as_u64()?).ok()?.to_string()),
    };
    for entity in world["entities"].as_array_mut()? {
        entity["objectId"] = json!(object_id(&entity["objectId"])?.to_string());
        // Same runtime millisecond fields used by the existing Windows producer.
        let started = entity["movementStartedAt"].as_f64();
        let duration = match (started, entity["movementUntil"].as_f64()) {
            (Some(start), Some(end)) if end > start => Some(end - start),
            _ => None,
        };
        entity["movementStartedMs"] = json!(started);
        entity["movementDurationMs"] = json!(duration);
    }
    if world.get("groundDrops").is_none_or(Value::is_null) {
        world["groundDrops"] = json!([]);
    }
    for drop in world["groundDrops"].as_array_mut()? {
        let object_id = object_id(&drop["objectId"])?;
        let quantity = u32::try_from(drop["quantity"].as_u64()?).ok()?;
        let gold = drop.pointer("/loot/kind").and_then(Value::as_str) == Some("gold");
        let image = if gold {
            gold_ground_frame(quantity)
        } else {
            u16::try_from(drop["icon"].as_u64()?).ok()?
        };
        drop["objectId"] = json!(object_id.to_string());
        drop["image"] = json!(image);
        drop["dropKind"] = json!(if gold { "gold" } else { "item" });
    }
    // Use the same renderer-neutral types consumed by native runtime. This does
    // not install the optional placeholder terrain/entity rendering plugins.
    let scene_view = world.get("sceneView").filter(|view| !view.is_null());
    let center = scene_view.map(|view| view.get("center")).unwrap_or(None);
    if world.get("sceneView").is_some_and(|view| !view.is_null()) && center.is_none() {
        return None;
    }
    let (center_x, center_y) = match center {
        Some(center) => (
            i32::try_from(center["x"].as_u64()?).ok()?,
            i32::try_from(center["y"].as_u64()?).ok()?,
        ),
        // A partial snapshot can lack a viewport. Center on its already
        // validated authoritative self position, never a synthetic origin.
        None => (i32::try_from(x).ok()?, i32::try_from(y).ok()?),
    };
    let view_dimension = |key: &str, default: i32| -> Option<i32> {
        match scene_view.and_then(|view| view.get(key)) {
            None | Some(Value::Null) => Some(default),
            Some(value) => i32::try_from(value.as_i64()?)
                .ok()
                .map(|value| value.clamp(1, 128)),
        }
    };
    let scene = ProjectedScene {
        map_file_name: map.to_owned(),
        center_x,
        center_y,
        width: view_dimension("width", 19)?,
        height: view_dimension("height", 15)?,
    };
    let map_model: mir2_client_bevy::map::MapModel = serde_json::from_value(json!({
        "centerX": center_x, "centerY": center_y,
        "patches": world.get("terrainPatches").cloned().unwrap_or(json!([])),
        "timeOfDayLightSetting": world["lightSetting"].as_u64().filter(|v| *v <= 4),
    }))
    .ok()?;
    let entity_models = json!({
        "entities": world["entities"],
        "groundDrops": world["groundDrops"],
    });
    let _entity_model: mir2_client_bevy::entities::EntityModelSet =
        serde_json::from_value(entity_models.clone()).ok()?;
    let request_id = mir2_bevy_runtime::native_world_receipt::tag_world_request(&mut world)?;
    Some(Projection {
        request_id,
        world: world.to_string(),
        ui: json!({"player": stats}).to_string(),
        map: serde_json::to_string(&map_model).ok()?,
        entities: entity_models.to_string(),
        scene,
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

fn object_id(value: &Value) -> Option<u32> {
    let id = u32::try_from(value.as_u64()?).ok()?;
    (id != 0).then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> Value {
        json!({"playerObjectId": 42, "mapFileName": "0", "mapTitle": "Bichon",
            "playerHp": 30, "playerMaxHp": 50, "gold": 123,
            "sceneView": {"center": {"x": 300, "y": 630}, "width": 21, "height": 17},
            "entities": [{"objectId": 42, "kind": "selfPlayer", "name": "Fixture",
                "x": 300, "y": 630, "class": "Warrior", "level": 7,
                "movementStartedAt": 1000, "movementUntil": 1300},
                {"objectId": 43, "kind": "monster", "name": "Deer", "x": 301, "y": 630}],
            "terrainPatches": [], "decorObjects": [], "mineNodes": []})
    }

    #[test]
    fn retains_scene_and_actors_and_uses_shared_hud_type() {
        let source = snapshot();
        let projected = project(&source.to_string(), "0", "Fixture", 300, 630).unwrap();
        let world: Value = serde_json::from_str(&projected.world).unwrap();
        assert_eq!(world["playerObjectId"], "42");
        assert_eq!(world["entities"][1]["objectId"], "43");
        assert_eq!(world["sceneView"], source["sceneView"]);
        assert_eq!(world["entities"][0]["movementDurationMs"], 300.0);
        let ui: mir2_client_bevy::read_model::UiReadModel =
            serde_json::from_str(&projected.ui).unwrap();
        assert_eq!(ui.player.hp, 30);
        assert_eq!(ui.player.gold, 123);
        assert_eq!(ui.player.class_name.as_deref(), Some("Warrior"));
        assert_eq!(ui.player.map_name.as_deref(), Some("Bichon"));
        let map: mir2_client_bevy::map::MapModel = serde_json::from_str(&projected.map).unwrap();
        assert_eq!((map.center_x, map.center_y), (300, 630));
        assert_eq!(
            projected.scene,
            ProjectedScene {
                map_file_name: "0".into(),
                center_x: 300,
                center_y: 630,
                width: 21,
                height: 17,
            }
        );
        let entities: mir2_client_bevy::entities::EntityModelSet =
            serde_json::from_str(&projected.entities).unwrap();
        assert_eq!(entities.entities.len(), 2);
        assert_eq!(entities.entities[1].object_id, "43");
        assert_eq!((entities.entities[1].x, entities.entities[1].y), (301, 630));
    }

    #[test]
    fn validates_and_normalizes_authoritative_ground_drops() {
        let mut source = snapshot();
        source["groundDrops"] = json!([
            {"objectId":50,"name":"Potion","nameColourArgb":-1,"icon":7,
                "x":301,"y":630,"quantity":1,"sourceMonster":"Deer",
                "loot":{"kind":"inventoryItem","key":"potion"}},
            {"objectId":51,"name":"250 Gold","nameColourArgb":-1,"icon":0,
                "x":300,"y":631,"quantity":250,"sourceMonster":"",
                "loot":{"kind":"gold","amount":250}}
        ]);
        let projected = project(&source.to_string(), "0", "Fixture", 300, 630).unwrap();
        let world: Value = serde_json::from_str(&projected.world).unwrap();
        assert_eq!(world["groundDrops"][0]["objectId"], "50");
        assert_eq!(world["groundDrops"][0]["image"], 7);
        assert_eq!(world["groundDrops"][0]["dropKind"], "item");
        assert_eq!(world["groundDrops"][1]["image"], 114);
        assert_eq!(world["groundDrops"][1]["dropKind"], "gold");
        let models: Value = serde_json::from_str(&projected.entities).unwrap();
        assert_eq!(models["groundDrops"], world["groundDrops"]);

        source["groundDrops"][0]["objectId"] = json!(42);
        assert!(project(&source.to_string(), "0", "Fixture", 300, 630).is_none());
        source = snapshot();
        source["groundDrops"] = json!([{"objectId":50,"name":"Gold",
            "nameColourArgb":-1,"icon":0,"x":300,"y":630,"quantity":2,
            "sourceMonster":"","loot":{"kind":"gold","amount":1}}]);
        assert!(project(&source.to_string(), "0", "Fixture", 300, 630).is_none());
    }

    #[test]
    fn rejects_stale_mismatched_and_invalid_authority() {
        let raw = snapshot().to_string();
        assert!(project(&raw, "1", "Fixture", 300, 630).is_none());
        assert!(project(&raw, "0", "Other", 300, 630).is_none());
        assert!(project(&raw, "0", "Fixture", 301, 630).is_none());
        for invalid in [json!("42"), json!(42.5), json!(0), json!(4294967296u64)] {
            let mut world = snapshot();
            world["playerObjectId"] = invalid;
            assert!(project(&world.to_string(), "0", "Fixture", 300, 630).is_none());
        }
        let mut world = snapshot();
        world["entities"][1]["objectId"] = json!(42);
        assert!(project(&world.to_string(), "0", "Fixture", 300, 630).is_none());
        world = snapshot();
        world["playerHp"] = json!("30");
        assert!(project(&world.to_string(), "0", "Fixture", 300, 630).is_none());
    }

    #[test]
    fn nullable_snapshot_scalars_use_shared_defaults_without_rejecting_the_scene() {
        let mut world = snapshot();
        world["playerHp"] = Value::Null;
        world["currentWeight"] = Value::Null;
        world["entities"][0]["level"] = Value::Null;
        world["selectedObjectId"] = json!(0);
        let projection = project(&world.to_string(), "0", "Fixture", 300, 630).unwrap();
        let ui: mir2_client_bevy::read_model::UiReadModel =
            serde_json::from_str(&projection.ui).unwrap();
        assert_eq!(ui.player.hp, 0);
        assert_eq!(ui.player.current_weight, 0);
        assert_eq!(ui.player.level, 0);
    }

    #[test]
    fn scene_projection_uses_server_center_and_validates_shared_schema() {
        let mut world = snapshot();
        world["sceneView"]["center"]["x"] = json!(299);
        world["lightSetting"] = json!(3);
        world["terrainPatches"] = json!([{"x":290,"y":620,"width":20,"height":20,"kind":"grass"}]);
        let p = project(&world.to_string(), "0", "Fixture", 300, 630).unwrap();
        let map: mir2_client_bevy::map::MapModel = serde_json::from_str(&p.map).unwrap();
        assert_eq!(map.center_x, 299);
        assert_eq!(map.patches.len(), 1);
        assert_eq!(map.time_of_day_light_setting, Some(3));
        world["sceneView"] = Value::Null;
        let p = project(&world.to_string(), "0", "Fixture", 300, 630).unwrap();
        let map: mir2_client_bevy::map::MapModel = serde_json::from_str(&p.map).unwrap();
        assert_eq!((map.center_x, map.center_y), (300, 630));
        assert_eq!((p.scene.width, p.scene.height), (19, 15));
        world["sceneView"] = json!({"center":{"x":"299","y":630}});
        assert!(project(&world.to_string(), "0", "Fixture", 300, 630).is_none());
        world = snapshot();
        world["sceneView"]["width"] = json!("21");
        assert!(project(&world.to_string(), "0", "Fixture", 300, 630).is_none());
        world = snapshot();
        world["entities"][1]["direction"] = json!(7);
        assert!(project(&world.to_string(), "0", "Fixture", 300, 630).is_none());
        world = snapshot();
        world["terrainPatches"] = json!([{"x":0,"y":0,"width":1,"height":1,"kind":"unknown"}]);
        assert!(project(&world.to_string(), "0", "Fixture", 300, 630).is_none());
    }
}
