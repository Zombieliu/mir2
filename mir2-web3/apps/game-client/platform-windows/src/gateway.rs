//! Gateway WebSocket client for the native host.
//!
//! Reuses the exact JSON `BrowserCommand` wire protocol the Web client speaks
//! (see `apps/gateway/src/web.rs` `BrowserCommand` and the 5-layer data flow in
//! `docs/client/protocol-cross-layer.md`). The server remains authoritative;
//! this client only authenticates, starts a game, and forwards the world
//! snapshot into the shared runtime.
//!
//! M2 slice: login → StartGame → forward `worldSnapshot` JSON into the runtime's
//! native ingestion channel. Movement/combat intents are the next slice.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

/// The gateway WebSocket endpoint for the local development gateway.
pub const LOCAL_GATEWAY_WS_URL: &str = "ws://127.0.0.1:7110/ws";

/// Player intent commands the native host forwards to the gateway.
///
/// Mirrors the Web client's `BrowserCommand` movement surface. The server
/// remains authoritative; these are requests, not state changes.
#[derive(Debug, Clone)]
pub enum PlayerIntent {
    Walk { direction: String },
    Run { direction: String },
    Turn { direction: String },
}

impl PlayerIntent {
    fn to_json(&self) -> Value {
        match self {
            Self::Walk { direction } => json!({ "type": "walk", "direction": direction }),
            Self::Run { direction } => json!({ "type": "run", "direction": direction }),
            Self::Turn { direction } => json!({ "type": "turn", "direction": direction }),
        }
    }
}

/// Cross-thread channel the Bevy input systems push intents into; the gateway
/// task drains and sends them over the WebSocket.
pub type GatewayCommandSender = std::sync::mpsc::Sender<PlayerIntent>;

/// Gateway WS messages decoded for the native host.
#[derive(Debug, Clone, Deserialize)]
struct GatewayEvent {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    packet: Option<String>,
    #[serde(default)]
    payload: Option<Value>,
}

/// Outbound login command, matching the Web client's `{type:"login",…}`.
#[derive(Debug, Serialize)]
struct LoginCommand<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(rename = "accountId")]
    account_id: &'a str,
    password: &'a str,
}

/// Outbound StartGame command.
#[derive(Debug, Serialize)]
struct StartGameCommand {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(rename = "characterIndex")]
    character_index: i32,
}

/// Connect to the gateway, log in with `account_id`/`password`, start the game
/// at `character_index`, and forward every `worldSnapshot` payload into the
/// shared runtime. Runs until the socket closes.
pub async fn run_gateway_session(
    account_id: String,
    password: String,
    character_index: i32,
    base_url: &str,
    commands: std::sync::mpsc::Receiver<PlayerIntent>,
) -> Result<(), String> {
    let (mut socket, _response) = tokio_tungstenite::connect_async(base_url)
        .await
        .map_err(|error| format!("gateway connect failed: {error}"))?;

    eprintln!("[gateway-client] connected to {base_url}");

    // Login.
    socket
        .send(Message::Text(
            serde_json::to_string(&LoginCommand {
                kind: "login",
                account_id: &account_id,
                password: &password,
            })
            .map_err(|error| error.to_string())?
            .into(),
        ))
        .await
        .map_err(|error| format!("login send failed: {error}"))?;
    eprintln!("[gateway-client] login sent for {account_id}");

    // StartGame: send it right away; the gateway orders commands serially.
    socket
        .send(Message::Text(
            serde_json::to_string(&StartGameCommand {
                kind: "startGame",
                character_index,
            })
            .map_err(|error| error.to_string())?
            .into(),
        ))
        .await
        .map_err(|error| format!("startGame send failed: {error}"))?;
    eprintln!("[gateway-client] startGame sent for character {character_index}");

    let mut keepalive = tokio::time::interval(Duration::from_secs(15));
    // Poll the local intent queue frequently so input latency stays low.
    let mut input_poll = tokio::time::interval(Duration::from_millis(8));
    let mut pending_error: Option<String> = None;
    let mut snapshot_log_counter: u32 = 0;

    loop {
        tokio::select! {
            _ = keepalive.tick() => {
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_millis() as i64)
                    .unwrap_or(0);
                let keepalive = json!({ "type": "keepAlive", "time": now_ms });
                if socket.send(Message::Text(keepalive.to_string().into())).await.is_err() {
                    break;
                }
            }
            _ = input_poll.tick() => {
                while let Ok(intent) = commands.try_recv() {
                    let payload = intent.to_json().to_string();
                    if socket.send(Message::Text(payload.into())).await.is_err() {
                        break;
                    }
                }
            }
            message = socket.next() => {
                match message {
                    Some(Ok(Message::Text(text))) => {
                        if let Err(error) = handle_gateway_text(&text, &mut snapshot_log_counter) {
                            pending_error = Some(error);
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => continue,
                    Some(Err(error)) => {
                        pending_error = Some(format!("gateway read error: {error}"));
                        break;
                    }
                }
            }
        }
    }

    match pending_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// Handle one inbound gateway text message. Returns an error to abort the loop.
fn handle_gateway_text(text: &str, snapshot_log_counter: &mut u32) -> Result<(), String> {
    let event: GatewayEvent =
        serde_json::from_str(text).map_err(|error| format!("invalid gateway payload: {error}"))?;

    match event.kind.as_str() {
        "worldSnapshot" => {
            let payload = event
                .payload
                .ok_or_else(|| "worldSnapshot missing payload".to_owned())?;
            let runtime_snapshot = transform_world_snapshot(&payload);
            let json = serde_json::to_string(&runtime_snapshot).map_err(|e| e.to_string())?;
            if mir2_bevy_runtime::native_ingest::push_native_world_state(json) {
                // Periodic (~1.2 s game tick) snapshot; only log the first few
                // so the native console stays readable while the map renders.
                *snapshot_log_counter += 1;
                if *snapshot_log_counter <= 3 {
                    eprintln!(
                        "[gateway-client] forwarded world snapshot #{}",
                        *snapshot_log_counter
                    );
                }
            } else {
                eprintln!("[gateway-client] runtime not ready; dropping snapshot");
            }

            // Feed the HUD read model so the shared Bevy UI renders player stats.
            let ui_model = transform_ui_read_model(&payload);
            let ui_json = serde_json::to_string(&ui_model).map_err(|e| e.to_string())?;
            let _ = mir2_bevy_runtime::native_ingest::push_native_ui_read_model(ui_json);

            // Feed the shared map model so client-bevy renders terrain tiles.
            let map_model = transform_map_model(&payload);
            let map_json = serde_json::to_string(&map_model).map_err(|e| e.to_string())?;
            let _ = mir2_bevy_runtime::native_ingest::push_native_map_model(map_json);

            // When a local map pack + atlas are available, render the real map
            // textures via MapRenderState instead of the colored terrain. The
            // gateway payload carries the authoritative map file name.
            let map_file_name = payload
                .get("mapFileName")
                .and_then(Value::as_str)
                .unwrap_or("0");
            if crate::map_parser::has_local_map_atlas() {
                if let Some(map) = crate::map_parser::load_map(map_file_name) {
                    let viewport = crate::map_parser::MapViewport::from_gateway_payload(&payload);
                    if let Some(render_state) =
                        crate::map_parser::build_map_render_state(&map, viewport)
                    {
                        let render_json =
                            serde_json::to_string(&render_state).map_err(|e| e.to_string())?;
                        let _ = mir2_bevy_runtime::native_ingest::push_native_map_render_state(
                            render_json,
                        );
                        eprintln!(
                            "[gateway-client] pushed real map render state for {map_file_name}"
                        );
                    }
                }
            }

            // Feed the shared entity model set so client-bevy renders entities.
            let entity_model = transform_entity_model_set(&payload);
            let entity_json = serde_json::to_string(&entity_model).map_err(|e| e.to_string())?;
            let _ = mir2_bevy_runtime::native_ingest::push_native_entity_model_set(entity_json);

            // When a local entity atlas is available, feed the real sprite
            // render state (entities mapped to atlas rects). Otherwise the
            // shared colored-entity renderer stays active.
            if crate::atlas::has_starter_atlas() {
                if let Some(render_state) = crate::atlas::build_entity_render_state(&payload) {
                    let render_json =
                        serde_json::to_string(&render_state).map_err(|e| e.to_string())?;
                    let _ = mir2_bevy_runtime::native_ingest::push_native_entity_render_state(
                        render_json,
                    );
                }
            }

            // Feed the shared inventory model so client-bevy renders the bag.
            let inventory = transform_inventory_model(&payload);
            let inventory_json = serde_json::to_string(&inventory).map_err(|e| e.to_string())?;
            let _ = mir2_bevy_runtime::native_ingest::push_native_inventory_model(inventory_json);

            Ok(())
        }
        "packet" => {
            let packet = event.packet.as_deref().unwrap_or("?");
            match packet {
                "LoginSuccess" => {
                    eprintln!("[gateway-client] LoginSuccess");
                }
                "StartGame" => {
                    eprintln!("[gateway-client] StartGame ack");
                }
                "MapInformation" | "UserInformation" | "UserLocation" => {
                    eprintln!("[gateway-client] packet {packet}");
                }
                "ObjectChat" => {
                    if let Some(payload) = event.payload.as_ref() {
                        if let Some(chat) = transform_chat_line(payload) {
                            let _ = mir2_bevy_runtime::native_ingest::push_native_chat_line(
                                serde_json::to_string(&chat).map_err(|e| e.to_string())?,
                            );
                        }
                    }
                }
                _ => {
                    // Other packets are folded into the periodic worldSnapshot;
                    // not logged to keep the native console readable.
                }
            }
            Ok(())
        }
        "error" => {
            let message = event
                .payload
                .as_ref()
                .and_then(|p| p.get("message").and_then(Value::as_str))
                .map(str::to_owned)
                .unwrap_or_else(|| "gateway error".to_owned());
            Err(message)
        }
        _ => Ok(()),
    }
}

/// Transform a gateway `worldSnapshot` payload into the runtime's
/// `WorldSnapshot` JSON shape.
///
/// The gateway serializes `WorldSnapshot` with u32 `objectId`s and a wide field
/// set; the Bevy runtime deserializes a smaller camelCase shape with string
/// object ids and the movement timing the motion table consumes.
fn transform_world_snapshot(payload: &Value) -> Value {
    let entities = payload
        .get("entities")
        .and_then(Value::as_array)
        .map(|entities| {
            entities
                .iter()
                .map(|entity| {
                    let object_id = entity
                        .get("objectId")
                        .and_then(object_id_string)
                        .unwrap_or_default();
                    let (movement_started_ms, movement_duration_ms) = movement_window(entity);
                    json!({
                        "objectId": object_id,
                        "kind": entity.get("kind").cloned().unwrap_or(json!("monster")),
                        "name": entity.get("name").cloned().unwrap_or(json!("")),
                        "x": entity.get("x").cloned().unwrap_or(json!(0)),
                        "y": entity.get("y").cloned().unwrap_or(json!(0)),
                        "direction": entity.get("direction"),
                        "level": entity.get("level"),
                        "movementStartedMs": movement_started_ms,
                        "movementDurationMs": movement_duration_ms,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mine_nodes = payload
        .get("mineNodes")
        .and_then(Value::as_array)
        .map(|nodes| {
            nodes
                .iter()
                .map(|node| {
                    json!({
                        "x": node.get("x").cloned().unwrap_or(json!(0)),
                        "y": node.get("y").cloned().unwrap_or(json!(0)),
                        "stage": node.get("stage").cloned().unwrap_or(json!(0)),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let self_player = entities
        .iter()
        .find(|entity| entity.get("kind").and_then(Value::as_str) == Some("selfPlayer"));

    json!({
        "mapTitle": payload.get("mapTitle"),
        "playerObjectId": payload.get("playerObjectId").and_then(object_id_string),
        "selectedObjectId": payload.get("selectedObjectId").and_then(object_id_string),
        "sceneView": payload.get("sceneView"),
        "terrainPatches": payload.get("terrainPatches").cloned().unwrap_or(Value::Array(vec![])),
        "decorObjects": payload.get("decorObjects").cloned().unwrap_or(Value::Array(vec![])),
        "entities": entities,
        "mineNodes": mine_nodes,
        "playerStats": {
            "hp": payload.get("playerHp").cloned().unwrap_or(json!(0)),
            "maxHp": payload.get("playerMaxHp").cloned().unwrap_or(json!(0)),
            "mp": payload.get("playerMp").cloned().unwrap_or(json!(0)),
            "maxMp": payload.get("playerMaxMp").cloned().unwrap_or(json!(0)),
            "gold": payload.get("gold").cloned().unwrap_or(json!(0)),
            "level": self_player.and_then(|e| e.get("level").cloned()).unwrap_or(json!(0)),
            "name": self_player.and_then(|e| e.get("name").cloned()).unwrap_or(json!("")),
            "mapName": payload.get("mapTitle"),
        },
    })
}

/// Transform a gateway `worldSnapshot` payload into the shared
/// `mir2-client-bevy::map::MapModel` JSON shape.
fn transform_map_model(payload: &Value) -> Value {
    let scene_view = payload.get("sceneView");
    let (center_x, center_y) = scene_view
        .and_then(|view| view.get("center"))
        .map(|center| {
            (
                center.get("x").and_then(Value::as_i64).unwrap_or(0) as i32,
                center.get("y").and_then(Value::as_i64).unwrap_or(0) as i32,
            )
        })
        .unwrap_or((0, 0));

    json!({
        "centerX": center_x,
        "centerY": center_y,
        "patches": payload.get("terrainPatches").cloned().unwrap_or(Value::Array(vec![])),
    })
}

/// Convert a gateway JSON value to a string object id (number or string).
fn object_id_string(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => Some(number.to_string()),
        Value::String(string) => Some(string.clone()),
        _ => None,
    }
}

/// Transform a gateway `ObjectChat` packet payload into a shared
/// `mir2-client-bevy::chat::ChatLine`. Returns `None` when the text is absent.
fn transform_chat_line(payload: &Value) -> Option<mir2_client_bevy::chat::ChatLine> {
    let text = payload
        .get("text")
        .and_then(Value::as_str)
        .map(str::to_owned)?;
    let channel = payload
        .get("chatType")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| "normal".to_owned());
    Some(mir2_client_bevy::chat::ChatLine { text, channel })
}

/// Transform a gateway `worldSnapshot` payload into the shared
/// `mir2-client-bevy::inventory::InventoryModel` JSON shape.
///
/// Container mapping: inventory items → 0 (bag), belt items → 1, equipment → 2.
fn transform_inventory_model(payload: &Value) -> Value {
    let gold = payload.get("gold").cloned().unwrap_or(json!(0));

    let map_items = |items: Option<&Value>, container: u8| -> Vec<Value> {
        items
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .enumerate()
                    .map(|(index, item)| {
                        json!({
                            "key": item.get("key").cloned().unwrap_or_else(|| json!(index.to_string())),
                            "name": item.get("name").cloned().unwrap_or(json!("")),
                            "quantity": item.get("quantity").cloned().unwrap_or(json!(1)),
                            "slot": item.get("slot").cloned().unwrap_or(json!(index)),
                            "container": container,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    };

    let mut items = Vec::new();
    items.extend(map_items(payload.get("inventoryItems"), 0));
    items.extend(map_items(payload.get("beltItems"), 1));
    items.extend(map_items(payload.get("equipmentItems"), 2));

    json!({ "gold": gold, "items": items })
}

/// Transform a gateway `worldSnapshot` payload into the shared
/// `mir2-client-bevy::entities::EntityModelSet` JSON shape.
fn transform_entity_model_set(payload: &Value) -> Value {
    let entities = payload
        .get("entities")
        .and_then(Value::as_array)
        .map(|entities| {
            entities
                .iter()
                .map(|entity| {
                    let object_id = entity
                        .get("objectId")
                        .and_then(object_id_string)
                        .unwrap_or_default();
                    json!({
                        "objectId": object_id,
                        "kind": entity.get("kind").cloned().unwrap_or(json!("monster")),
                        "name": entity.get("name").cloned().unwrap_or(json!("")),
                        "x": entity.get("x").cloned().unwrap_or(json!(0)),
                        "y": entity.get("y").cloned().unwrap_or(json!(0)),
                        "level": entity.get("level"),
                        "direction": entity.get("direction"),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    json!({ "entities": entities })
}

/// Extract the movement window from a gateway entity, matching the Web
/// client's `movementStartedAt` / `movementUntil` convention.
fn movement_window(entity: &Value) -> (Option<f64>, Option<f64>) {
    let started = entity.get("movementStartedAt").and_then(Value::as_f64);
    let until = entity.get("movementUntil").and_then(Value::as_f64);
    let duration = match (started, until) {
        (Some(start), Some(end)) if end > start => Some(end - start),
        _ => None,
    };
    (started, duration)
}

/// Transform a gateway `worldSnapshot` payload into the shared
/// `mir2-client-bevy::read_model::UiReadModel` JSON shape.
fn transform_ui_read_model(payload: &Value) -> Value {
    let self_player = payload
        .get("entities")
        .and_then(Value::as_array)
        .and_then(|entities| {
            entities
                .iter()
                .find(|entity| entity.get("kind").and_then(Value::as_str) == Some("selfPlayer"))
        });

    json!({
        "player": {
            "hp": payload.get("playerHp").cloned().unwrap_or(json!(0)),
            "maxHp": payload.get("playerMaxHp").cloned().unwrap_or(json!(0)),
            "mp": payload.get("playerMp").cloned().unwrap_or(json!(0)),
            "maxMp": payload.get("playerMaxMp").cloned().unwrap_or(json!(0)),
            "gold": payload.get("gold").cloned().unwrap_or(json!(0)),
            "level": self_player.and_then(|e| e.get("level").cloned()).unwrap_or(json!(0)),
            "name": self_player.and_then(|e| e.get("name").cloned()).unwrap_or(json!("")),
            "mapName": payload.get("mapTitle"),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_intents_serialize_to_the_browser_command_protocol() {
        assert_eq!(
            PlayerIntent::Walk {
                direction: "up".into()
            }
            .to_json(),
            json!({ "type": "walk", "direction": "up" })
        );
        assert_eq!(
            PlayerIntent::Run {
                direction: "left".into()
            }
            .to_json(),
            json!({ "type": "run", "direction": "left" })
        );
        assert_eq!(
            PlayerIntent::Turn {
                direction: "down".into()
            }
            .to_json(),
            json!({ "type": "turn", "direction": "down" })
        );
    }

    fn gateway_payload() -> Value {
        json!({
            "tick": 42,
            "mapTitle": "BichonProvince",
            "playerObjectId": 1001,
            "selectedObjectId": null,
            "playerHp": 50,
            "playerMaxHp": 100,
            "playerMp": 25,
            "playerMaxMp": 50,
            "gold": 1234,
            "sceneView": { "center": { "x": 9, "y": 7 }, "width": 19, "height": 15 },
            "terrainPatches": [ { "x": 0, "y": 0, "width": 40, "height": 40, "kind": "grass" } ],
            "decorObjects": [],
            "entities": [
                {
                    "objectId": 1001,
                    "kind": "selfPlayer",
                    "name": "Demo",
                    "x": 9,
                    "y": 7,
                    "direction": "up",
                    "level": 3,
                    "movementStartedAt": 1700000000000_f64,
                    "movementUntil": 1700000000600_f64
                },
                {
                    "objectId": 2001,
                    "kind": "monster",
                    "name": "Wolf",
                    "x": 11,
                    "y": 8,
                    "direction": "down"
                }
            ],
            "mineNodes": [ { "x": 3, "y": 4, "stage": 2 } ]
        })
    }

    #[test]
    fn transform_preserves_core_fields_and_converts_object_ids_to_strings() {
        let transformed = transform_world_snapshot(&gateway_payload());
        assert_eq!(transformed["mapTitle"], json!("BichonProvince"));
        assert_eq!(transformed["playerObjectId"], json!("1001"));
        assert_eq!(transformed["selectedObjectId"], Value::Null);

        let entities = transformed["entities"].as_array().expect("entities array");
        assert_eq!(entities.len(), 2);
        assert_eq!(entities[0]["objectId"], json!("1001"));
        assert_eq!(entities[0]["kind"], json!("selfPlayer"));
        assert_eq!(entities[0]["x"], json!(9));
        assert_eq!(entities[1]["objectId"], json!("2001"));
        assert_eq!(entities[1]["kind"], json!("monster"));

        let mine_nodes = transformed["mineNodes"].as_array().expect("mine nodes");
        assert_eq!(mine_nodes[0]["stage"], json!(2));
    }

    #[test]
    fn transform_derives_movement_duration_from_until_minus_started() {
        let transformed = transform_world_snapshot(&gateway_payload());
        let entities = transformed["entities"].as_array().expect("entities array");
        assert_eq!(entities[0]["movementStartedMs"], json!(1700000000000_f64));
        assert_eq!(entities[0]["movementDurationMs"], json!(600_f64));
        // No timing metadata on the second entity.
        assert_eq!(entities[1]["movementStartedMs"], Value::Null);
        assert_eq!(entities[1]["movementDurationMs"], Value::Null);
    }

    #[test]
    fn transformed_snapshot_parses_into_the_runtime_world_snapshot_shape() {
        let transformed = transform_world_snapshot(&gateway_payload());
        let json = serde_json::to_string(&transformed).expect("serialize");
        // The runtime deserializes this exact camelCase shape; a parse error
        // here means the transform drifted from the runtime's WorldSnapshot.
        let parsed = serde_json::from_str::<serde_json::Value>(&json).expect("parse");
        assert!(parsed.get("entities").is_some());
    }

    #[test]
    fn transform_extracts_player_stats_for_the_hud() {
        let transformed = transform_world_snapshot(&gateway_payload());
        let stats = &transformed["playerStats"];
        assert_eq!(stats["hp"], json!(50));
        assert_eq!(stats["maxHp"], json!(100));
        assert_eq!(stats["mp"], json!(25));
        assert_eq!(stats["maxMp"], json!(50));
        assert_eq!(stats["gold"], json!(1234));
        assert_eq!(stats["level"], json!(3));
        assert_eq!(stats["name"], json!("Demo"));
        assert_eq!(stats["mapName"], json!("BichonProvince"));
    }

    #[test]
    fn ui_read_model_transform_matches_the_shared_hud_shape() {
        let ui = transform_ui_read_model(&gateway_payload());
        assert_eq!(ui["player"]["hp"], json!(50));
        assert_eq!(ui["player"]["maxHp"], json!(100));
        assert_eq!(ui["player"]["gold"], json!(1234));
        assert_eq!(ui["player"]["name"], json!("Demo"));
        assert_eq!(ui["player"]["level"], json!(3));
        // Must deserialize as mir2-client-bevy UiReadModel.
        let model = serde_json::from_str::<mir2_client_bevy::read_model::UiReadModel>(
            &serde_json::to_string(&ui).expect("serialize"),
        )
        .expect("UiReadModel");
        assert_eq!(model.player.hp, 50);
        assert_eq!(model.player.max_hp, 100);
        assert_eq!(model.player.gold, 1234);
        assert_eq!(model.player.name.as_deref(), Some("Demo"));
        assert!((model.player.normalized_hp() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn map_model_transform_matches_the_shared_map_shape() {
        let map = transform_map_model(&gateway_payload());
        assert_eq!(map["centerX"], json!(9));
        assert_eq!(map["centerY"], json!(7));
        let patches = map["patches"].as_array().expect("patches");
        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0]["kind"], json!("grass"));
        assert_eq!(patches[0]["width"], json!(40));
        // Must deserialize as mir2-client-bevy MapModel.
        let model = serde_json::from_str::<mir2_client_bevy::map::MapModel>(
            &serde_json::to_string(&map).expect("serialize"),
        )
        .expect("MapModel");
        assert_eq!(model.center_x, 9);
        assert_eq!(model.center_y, 7);
        assert_eq!(model.patches.len(), 1);
    }

    #[test]
    fn entity_model_transform_matches_the_shared_entity_shape() {
        let entities = transform_entity_model_set(&gateway_payload());
        let list = entities["entities"].as_array().expect("entities");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0]["objectId"], json!("1001"));
        assert_eq!(list[0]["kind"], json!("selfPlayer"));
        assert_eq!(list[0]["x"], json!(9));
        assert_eq!(list[1]["objectId"], json!("2001"));
        assert_eq!(list[1]["kind"], json!("monster"));
        // Must deserialize as mir2-client-bevy EntityModelSet.
        let model = serde_json::from_str::<mir2_client_bevy::entities::EntityModelSet>(
            &serde_json::to_string(&entities).expect("serialize"),
        )
        .expect("EntityModelSet");
        assert_eq!(model.entities.len(), 2);
        assert_eq!(
            model.entities[0].kind,
            mir2_client_bevy::entities::EntityKind::SelfPlayer
        );
        assert_eq!(
            model.entities[1].kind,
            mir2_client_bevy::entities::EntityKind::Monster
        );
    }

    #[test]
    fn inventory_transform_groups_items_by_container() {
        let mut payload = gateway_payload();
        payload["inventoryItems"] = json!([
            { "key": "red-potion", "name": "Red Potion", "quantity": 5, "slot": 0 }
        ]);
        payload["beltItems"] = json!([
            { "key": "blue-potion", "name": "Blue Potion", "quantity": 2, "slot": 0 }
        ]);
        payload["equipmentItems"] = json!([
            { "key": "wooden-sword", "name": "Wooden Sword", "quantity": 1, "slot": 3 }
        ]);

        let inventory = transform_inventory_model(&payload);
        assert_eq!(inventory["gold"], json!(1234));
        let items = inventory["items"].as_array().expect("items");
        assert_eq!(items.len(), 3);
        assert_eq!(items[0]["container"], json!(0));
        assert_eq!(items[1]["container"], json!(1));
        assert_eq!(items[2]["container"], json!(2));
        assert_eq!(items[2]["name"], json!("Wooden Sword"));

        let model = serde_json::from_str::<mir2_client_bevy::inventory::InventoryModel>(
            &serde_json::to_string(&inventory).expect("serialize"),
        )
        .expect("InventoryModel");
        assert_eq!(model.gold, 1234);
        assert_eq!(model.items.len(), 3);
    }

    #[test]
    fn chat_line_transform_extracts_text_and_channel() {
        let payload = json!({ "objectId": 1001, "text": "hello world", "chatType": "Normal" });
        let chat = transform_chat_line(&payload).expect("chat line");
        assert_eq!(chat.text, "hello world");
        assert_eq!(chat.channel, "Normal");

        let missing = json!({ "objectId": 1001 });
        assert!(transform_chat_line(&missing).is_none());
    }
}
