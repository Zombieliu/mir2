//! Decode the whole opt-in catalog batch before exposing any envelope to the
//! ordinary native packet handler. Transport errors never include raw payloads.

use crate::catalog_transport::decode_catalog_batch;
use crate::{ClientQuestInfo, ClientRecipeInfo, GameShopItem, ItemInfo};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogEnvelope {
    #[serde(rename = "type")]
    kind: String,
    packet: String,
    payload: Value,
}

fn valid_catalog_envelope(text: &str) -> bool {
    let Ok(envelope) = serde_json::from_str::<CatalogEnvelope>(text) else {
        return false;
    };
    let Some(payload) = envelope.payload.as_object() else {
        return false;
    };
    if envelope.kind != "packet" {
        return false;
    }
    // Validate the real shared wire structs, not just the packet name. Keep
    // each original JSON string for dispatch so no field is reconstructed or
    // dropped, including the browser-facing derived quest presentation fields.
    match envelope.packet.as_str() {
        "NewItemInfo" => {
            payload.len() == 1
                && payload
                    .get("info")
                    .is_some_and(|info| serde_json::from_value::<ItemInfo>(info.clone()).is_ok())
        }
        "NewRecipeInfo" => {
            payload.len() == 1
                && payload.get("info").is_some_and(|info| {
                    serde_json::from_value::<ClientRecipeInfo>(info.clone()).is_ok()
                })
        }
        "GameShopInfo" => {
            payload.len() == 2
                && payload.get("item").is_some_and(|item| {
                    serde_json::from_value::<GameShopItem>(item.clone()).is_ok()
                })
                && payload
                    .get("stockLevel")
                    .and_then(Value::as_i64)
                    .is_some_and(|stock| i32::try_from(stock).is_ok())
        }
        "NewQuestInfo" => {
            let Some(info) = payload.get("info") else {
                return false;
            };
            let Ok(info) = serde_json::from_value::<ClientQuestInfo>(info.clone()) else {
                return false;
            };
            payload.get("id").and_then(Value::as_i64) == Some(i64::from(info.index))
                && payload.get("name").and_then(Value::as_str) == Some(info.name.as_str())
                && payload.get("minLevelNeeded").and_then(Value::as_i64)
                    == Some(i64::from(info.min_level_needed))
                && payload.iter().all(|(key, value)| match key.as_str() {
                    "info" | "id" | "name" | "minLevelNeeded" => true,
                    "group" | "timeLimit" => value.is_string(),
                    "descriptionLines"
                    | "returnDescriptionLines"
                    | "completionDescriptionLines" => value
                        .as_array()
                        .is_some_and(|lines| lines.iter().all(Value::is_string)),
                    "objectives" => value
                        .as_array()
                        .is_some_and(|lines| lines.iter().all(Value::is_object)),
                    "rewards" => value.is_object(),
                    _ => false,
                })
        }
        _ => false,
    }
}

pub fn decode_validated_catalog_batch(bytes: &[u8]) -> Result<Vec<String>, String> {
    let texts = decode_catalog_batch(bytes).map_err(|error| error.to_string())?;
    if !texts.iter().all(|text| valid_catalog_envelope(text)) {
        return Err("invalid catalog packet envelope".to_owned());
    }
    Ok(texts)
}

