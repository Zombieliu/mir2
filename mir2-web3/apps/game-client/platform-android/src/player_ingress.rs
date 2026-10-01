//! Owner lifetime/queue adapter for the shared Windows player presentation.
//! No bootstrap, account binding, combat, purchase, custody or save authority.
use mir2_client_bevy::{
    native_player_ingress::{
        apply_wallet_delta, update_wallet_from_snapshot, wallet_value, NativeUiPlayerCursor,
        WalletState,
    },
    read_model::UiReadModel,
};
use serde_json::{json, Map, Value};

const MAX_SNAPSHOT_BYTES: usize = 1024 * 1024;
const MAX_PACKET_BYTES: usize = 16 * 1024;
const MAX_MODEL_BYTES: usize = 64 * 1024;

#[derive(Default)]
pub(crate) struct AndroidPlayerIngress {
    identity: Option<(u32, String)>,
    cursor: NativeUiPlayerCursor,
    wallet: Option<WalletState>,
    latest_ui: Option<String>,
    latest_wallet: Option<String>,
}

impl AndroidPlayerIngress {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn clear_scene(&mut self) {
        // Keep personal state during same-character map loading, never the old
        // map/safe-zone label. Removing optional fields cannot enlarge the model.
        if self.identity.is_some() {
            self.cursor.map_name = None;
            self.cursor.in_safe_zone = None;
            self.latest_ui = Some(self.cursor.to_read_model_json().to_string());
            self.latest_wallet = wallet_patch(&self.cursor);
        }
    }

    /// Called only after the host validates the snapshot's self/map/position.
    pub(crate) fn snapshot(&mut self, raw: &str) -> Result<String, &'static str> {
        if raw.len() > MAX_SNAPSHOT_BYTES {
            return Err("Player snapshot too large");
        }
        let world: Value = serde_json::from_str(raw).map_err(|_| "Invalid player snapshot")?;
        let owner = world["playerObjectId"]
            .as_u64()
            .and_then(|id| u32::try_from(id).ok())
            .filter(|id| *id != 0)
            .ok_or("Missing player owner")?;
        let actors = world["entities"]
            .as_array()
            .ok_or("Missing player entities")?;
        let mut selves = actors.iter().filter(|actor| actor["kind"] == "selfPlayer");
        let actor = selves.next().ok_or("Missing self player")?;
        if selves.next().is_some() || actor["objectId"].as_u64() != Some(u64::from(owner)) {
            return Err("Invalid self player");
        }
        let name = actor["name"].as_str().ok_or("Missing player character")?;
        if name.trim().is_empty() || name.chars().count() > 128 {
            return Err("Invalid player character");
        }
        validate_text_fields(actor)?;
        validate_text_fields(&world)?;
        if let Some(stats) = world
            .get("playerCrystalStats")
            .filter(|value| !value.is_null())
        {
            let stats = stats.as_array().ok_or("Invalid player stat list")?;
            if stats.len() > 256
                || stats.iter().any(|stat| {
                    stat["stat"]
                        .as_u64()
                        .and_then(|id| u8::try_from(id).ok())
                        .is_none()
                        || stat["value"]
                            .as_i64()
                            .and_then(|value| i32::try_from(value).ok())
                            .is_none()
                })
            {
                return Err("Invalid player stats");
            }
        }
        let identity = (owner, name.to_owned());
        let changed = self.identity.as_ref().is_some_and(|old| old != &identity);
        let mut cursor = if changed {
            NativeUiPlayerCursor::default()
        } else {
            self.cursor.clone()
        };
        let mut wallet = if changed { None } else { self.wallet };
        cursor.observe_world_snapshot(&world);
        update_wallet_from_snapshot(&mut wallet, &world);
        let ui = checked_model(&cursor)?;
        self.identity = Some(identity);
        self.cursor = cursor;
        self.wallet = wallet;
        self.latest_ui = Some(ui.clone());
        self.latest_wallet = wallet_patch(&self.cursor);
        Ok(ui)
    }

    pub(crate) fn packet(&mut self, raw: &str) -> Result<bool, &'static str> {
        if raw.len() > MAX_PACKET_BYTES {
            return Err("Player packet too large");
        }
        let event: Value = serde_json::from_str(raw).map_err(|_| "Invalid player packet")?;
        if event["type"] != "packet" {
            return Ok(false);
        }
        let Some(packet) = event["packet"].as_str().filter(|packet| {
            matches!(
                *packet,
                "UserInformation" | "GainedGold" | "LoseGold" | "GainedCredit" | "LoseCredit"
            )
        }) else {
            return Ok(false);
        };
        let Some((owner, name)) = &self.identity else {
            return Ok(false);
        };
        let payload = event["payload"]
            .as_object()
            .ok_or("Invalid player payload")?;
        if payload
            .get("hero")
            .is_some_and(|value| value.as_bool() != Some(false))
            || payload
                .get("objectId")
                .is_some_and(|value| value.as_u64() != Some(u64::from(*owner)))
            || (packet == "UserInformation" && !payload.contains_key("objectId"))
        {
            return Ok(false);
        }
        let mut cursor = self.cursor.clone();
        let mut wallet = self.wallet;
        if packet == "UserInformation" {
            if payload
                .get("name")
                .is_some_and(|value| value.as_str() != Some(name.as_str()))
            {
                return Ok(false);
            }
            validate_information(&event["payload"])?;
            cursor.observe_user_information(&event["payload"]);
            update_wallet_from_snapshot(&mut wallet, &event["payload"]);
        } else {
            let field = if packet.ends_with("Gold") {
                "gold"
            } else {
                "credit"
            };
            // A server delta is not an absolute bootstrap. Never invent a zero
            // base for a legacy snapshot that did not supply this wallet field.
            if (field == "gold" && cursor.gold.is_none())
                || (field == "credit" && cursor.credit.is_none())
            {
                return Err("Missing authoritative wallet base");
            }
            let value = apply_wallet_delta(
                &mut wallet,
                &mut None,
                field,
                wallet_value(&event["payload"], field),
                packet.starts_with("Gained"),
            )
            .ok_or("Invalid wallet delta")?;
            if field == "gold" {
                cursor.gold = Some(value)
            } else {
                cursor.credit = Some(value)
            }
        }
        let ui = checked_model(&cursor)?;
        self.cursor = cursor;
        self.wallet = wallet;
        self.latest_ui = Some(ui);
        self.latest_wallet = wallet_patch(&self.cursor);
        Ok(true)
    }

    pub(crate) fn flush(
        &mut self,
        mut push_ui: impl FnMut(String) -> bool,
        mut push_wallet: impl FnMut(String) -> bool,
    ) -> bool {
        let Some(ui) = self.latest_ui.as_ref() else {
            return true;
        };
        if !push_ui(ui.clone()) {
            return false;
        }
        if let Some(wallet) = self.latest_wallet.as_ref() {
            if !push_wallet(wallet.clone()) {
                return false;
            }
        }
        // Complete latest absolute values, not receipts. Refresh known wallet
        // fields with every UI model: the runtime consumes wallet patches after
        // UI/inventory snapshots. Partial enqueue retries this pair; newer data
        // may replace it, but no delta is re-applied and no ACK is invented.
        self.latest_ui = None;
        self.latest_wallet = None;
        true
    }
}

fn checked_model(cursor: &NativeUiPlayerCursor) -> Result<String, &'static str> {
    let model = cursor.to_read_model_json();
    let _: UiReadModel =
        serde_json::from_value(model.clone()).map_err(|_| "Invalid player model")?;
    let raw = model.to_string();
    if raw.len() > MAX_MODEL_BYTES {
        return Err("Player model too large");
    }
    Ok(raw)
}

fn wallet_patch(cursor: &NativeUiPlayerCursor) -> Option<String> {
    let mut patch = Map::new();
    if let Some(gold) = cursor.gold {
        patch.insert("gold".into(), json!(gold));
    }
    if let Some(credit) = cursor.credit {
        patch.insert("credit".into(), json!(credit));
    }
    (!patch.is_empty()).then(|| Value::Object(patch).to_string())
}

fn validate_text_fields(payload: &Value) -> Result<(), &'static str> {
    for field in [
        "name",
        "class",
        "className",
        "gender",
        "genderKey",
        "guildName",
        "guildRankName",
        "guildRank",
        "title",
        "mapTitle",
        "mapName",
    ] {
        if payload
            .get(field)
            .filter(|value| !value.is_null())
            .is_some_and(|value| value.as_str().is_none_or(|text| text.chars().count() > 512))
        {
            return Err("Invalid player text");
        }
    }
    Ok(())
}

fn validate_information(payload: &Value) -> Result<(), &'static str> {
    validate_text_fields(payload)?;
    let mut typed = payload
        .as_object()
        .ok_or("Invalid player information")?
        .clone();
    for (alias, field) in [
        ("playerHp", "hp"),
        ("playerMaxHp", "maxHp"),
        ("playerMp", "mp"),
        ("playerMaxMp", "maxMp"),
        ("playerExperience", "experience"),
        ("playerMaxExperience", "maxExperience"),
        ("class", "className"),
        ("genderKey", "gender"),
        ("mapTitle", "mapName"),
        ("title", "mapName"),
        ("in_safe_zone", "inSafeZone"),
    ] {
        if !typed.contains_key(field) {
            if let Some(value) = payload.get(alias) {
                typed.insert(field.into(), value.clone());
            }
        }
    }
    typed.retain(|_, value| !value.is_null());
    let _: mir2_client_bevy::read_model::PlayerStats = serde_json::from_value(Value::Object(typed))
        .map_err(|_| "Invalid player information fields")?;
    Ok(())
}

#[cfg(test)]
mod tests;
