//! Pure packet-first player presentation extracted verbatim from frozen Windows3f5e61533.
//! Hosts own authentication, identity, lifecycle and queues; this module owns no
//! combat, purchase, custody or save rules. Wallet deltas here are received public
//! server events, never optimistic responses to a client command.
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WalletState {
    pub gold: u32,
    pub credit: u32,
}

/// Packet-first authoritative HUD cursor for the native client.
///
/// Gateway snapshots may be partial during bootstrap, reconnect, and map
/// transitions. A missing JSON field means "no update"; it must not erase the
/// complete `UserInformation` values that were already delivered.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NativeUiPlayerCursor {
    pub npc_shop_uses_pearls: bool,
    pub npc_shop_hide_added_stats: bool,
    pub hp: Option<i32>,
    pub max_hp: Option<i32>,
    pub mp: Option<i32>,
    pub max_mp: Option<i32>,
    pub gold: Option<u32>,
    pub credit: Option<u32>,
    pub crystal_stats: Option<Vec<crate::read_model::CrystalPlayerStatModel>>,
    pub level: Option<u32>,
    pub experience: Option<i64>,
    pub max_experience: Option<i64>,
    pub current_weight: Option<u16>,
    pub player_weights: Option<crate::read_model::PlayerWeights>,
    pub max_weight: Option<u16>,
    pub name: Option<String>,
    pub class_name: Option<String>,
    pub gender: Option<String>,
    pub hair: Option<u8>,
    pub wing_effect: Option<u8>,
    pub guild_name: Option<String>,
    pub guild_rank_name: Option<String>,
    pub map_name: Option<String>,
    pub in_safe_zone: Option<bool>,
}

impl NativeUiPlayerCursor {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn observe_world_snapshot(&mut self, payload: &Value) {
        // Full world snapshots own this optional block; absent/null must clear old weights.
        self.player_weights = payload
            .get("playerWeights")
            .and_then(|value| serde_json::from_value(value.clone()).ok());
        if let Some(value) = value_i32(payload.get("playerHp")) {
            self.hp = Some(value);
        }
        if let Some(value) = value_i32(payload.get("playerMaxHp")) {
            self.max_hp = Some(value);
        }
        if let Some(value) = value_i32(payload.get("playerMp")) {
            self.mp = Some(value);
        }
        if let Some(value) = value_i32(payload.get("playerMaxMp")) {
            self.max_mp = Some(value);
        }
        if let Some(value) = value_u32(payload.get("gold")) {
            self.gold = Some(value);
        }
        if let Some(value) = value_u32(payload.get("credit")) {
            self.credit = Some(value);
        }
        if let Some(value) = payload.get("playerCrystalStats") {
            if let Ok(stats) = serde_json::from_value::<
                Vec<crate::read_model::CrystalPlayerStatModel>,
            >(value.clone())
            {
                self.crystal_stats = Some(stats);
            }
        }
        if let Some(value) = value_i64(payload.get("playerExperience")) {
            self.experience = Some(value);
        }
        if let Some(value) = value_i64(payload.get("playerMaxExperience")) {
            self.max_experience = Some(value);
        }
        if let Some(value) =
            value_u32(payload.get("currentWeight")).and_then(|value| u16::try_from(value).ok())
        {
            self.current_weight = Some(value);
        }
        if let Some(value) =
            value_u32(payload.get("maxWeight")).and_then(|value| u16::try_from(value).ok())
        {
            self.max_weight = Some(value);
        }
        if let Some(value) = value_string(payload.get("mapTitle")) {
            self.map_name = Some(value);
        }
        if let Some(value) = payload.get("inSafeZone").and_then(Value::as_bool) {
            self.in_safe_zone = Some(value);
        }

        let player_object_id = value_u32(payload.get("playerObjectId"));
        let self_player = payload
            .get("entities")
            .and_then(Value::as_array)
            .and_then(|entities| {
                entities.iter().find(|entity| {
                    entity.get("kind").and_then(Value::as_str) == Some("selfPlayer")
                        || player_object_id.is_some_and(|object_id| {
                            value_u32(entity.get("objectId")) == Some(object_id)
                        })
                })
            });
        if let Some(value) = value_u32(self_player.and_then(|entity| entity.get("level"))) {
            self.level = Some(value);
        }
        if let Some(value) = value_string(self_player.and_then(|entity| entity.get("name"))) {
            self.name = Some(value);
        }
        if let Some(value) = value_string(
            self_player.and_then(|entity| entity.get("class").or_else(|| entity.get("className"))),
        ) {
            self.class_name = Some(value);
        }
        if let Some(value) = value_string(
            self_player.and_then(|entity| entity.get("gender").or_else(|| entity.get("genderKey"))),
        ) {
            self.gender = Some(value);
        }
        if let Some(value) = value_u32(self_player.and_then(|entity| entity.get("hair")))
            .and_then(|value| u8::try_from(value).ok())
        {
            self.hair = Some(value);
        }
        if let Some(value) = value_u32(self_player.and_then(|entity| {
            entity
                .get("wingEffect")
                .or_else(|| entity.get("wing_effect"))
        }))
        .and_then(|value| u8::try_from(value).ok())
        {
            // Zero is an explicit authoritative clear; a missing field is a
            // partial snapshot and must preserve the previous value.
            self.wing_effect = Some(value);
        }
        if let Some(value) = value_string(self_player.and_then(|entity| entity.get("guildName"))) {
            self.guild_name = Some(value);
        }
        if let Some(value) = value_string(self_player.and_then(|entity| {
            entity
                .get("guildRankName")
                .or_else(|| entity.get("guildRank"))
        })) {
            self.guild_rank_name = Some(value);
        }
    }

    pub fn observe_user_information(&mut self, payload: &Value) {
        if let Some(value) = value_i32(payload.get("hp").or_else(|| payload.get("playerHp"))) {
            self.hp = Some(value);
        }
        if let Some(value) = value_i32(payload.get("maxHp").or_else(|| payload.get("playerMaxHp")))
        {
            self.max_hp = Some(value);
        }
        if let Some(value) = value_i32(payload.get("mp").or_else(|| payload.get("playerMp"))) {
            self.mp = Some(value);
        }
        if let Some(value) = value_i32(payload.get("maxMp").or_else(|| payload.get("playerMaxMp")))
        {
            self.max_mp = Some(value);
        }
        if let Some(value) = value_u32(payload.get("gold")) {
            self.gold = Some(value);
        }
        if let Some(value) = value_u32(payload.get("credit")) {
            self.credit = Some(value);
        }
        if let Some(value) = value_u32(payload.get("level")) {
            self.level = Some(value);
        }
        if let Some(value) = value_i64(
            payload
                .get("experience")
                .or_else(|| payload.get("playerExperience")),
        ) {
            self.experience = Some(value);
        }
        if let Some(value) = value_i64(
            payload
                .get("maxExperience")
                .or_else(|| payload.get("playerMaxExperience")),
        ) {
            self.max_experience = Some(value);
        }
        if let Some(value) =
            value_u32(payload.get("currentWeight")).and_then(|value| u16::try_from(value).ok())
        {
            self.current_weight = Some(value);
        }
        if let Some(value) =
            value_u32(payload.get("maxWeight")).and_then(|value| u16::try_from(value).ok())
        {
            self.max_weight = Some(value);
        }
        if let Some(value) = value_string(payload.get("name")) {
            self.name = Some(value);
        }
        if let Some(value) = value_string(payload.get("class").or_else(|| payload.get("className")))
        {
            self.class_name = Some(value);
        }
        if let Some(value) =
            value_string(payload.get("gender").or_else(|| payload.get("genderKey")))
        {
            self.gender = Some(value);
        }
        if let Some(value) =
            value_u32(payload.get("hair")).and_then(|value| u8::try_from(value).ok())
        {
            self.hair = Some(value);
        }
        if let Some(value) = value_string(payload.get("guildName")) {
            self.guild_name = Some(value);
        }
        if let Some(value) = value_string(
            payload
                .get("guildRankName")
                .or_else(|| payload.get("guildRank")),
        ) {
            self.guild_rank_name = Some(value);
        }
        if let Some(value) = payload
            .get("inSafeZone")
            .or_else(|| payload.get("in_safe_zone"))
            .and_then(Value::as_bool)
        {
            self.in_safe_zone = Some(value);
        }
        self.observe_map_identity(payload);
    }

    pub fn observe_map_identity(&mut self, payload: &Value) {
        if let Some(value) = value_string(
            payload
                .get("title")
                .or_else(|| payload.get("mapTitle"))
                .or_else(|| payload.get("mapName")),
        ) {
            self.map_name = Some(value);
        }
    }

    pub fn to_read_model_json(&self) -> Value {
        json!({
            "player": {
                "hp": self.hp.unwrap_or_default(),
                "maxHp": self.max_hp.unwrap_or_default(),
                "mp": self.mp.unwrap_or_default(),
                "maxMp": self.max_mp.unwrap_or_default(),
                "gold": self.gold.unwrap_or_default(),
                "credit": self.credit.unwrap_or_default(),
                "crystalStats": self.crystal_stats.clone(),
                "level": self.level.unwrap_or_default(),
                "experience": self.experience.unwrap_or_default(),
                "maxExperience": self.max_experience.unwrap_or_default(),
                "currentWeight": self.current_weight.unwrap_or_default(),
                "currentWeightKnown":self.current_weight.is_some(),
                "weights":self.player_weights,
                "maxWeight": self.max_weight.unwrap_or_default(),
                "name": self.name,
                "className": self.class_name,
                "gender": self.gender,
                "hair": self.hair,
                "wingEffect": self.wing_effect,
                "guildName": self.guild_name,
                "guildRankName": self.guild_rank_name,
                "mapName": self.map_name,
                "inSafeZone": self.in_safe_zone.unwrap_or(false),
            }
        })
    }
}

pub fn update_wallet_from_snapshot(last_wallet: &mut Option<WalletState>, payload: &Value) {
    let mut wallet = last_wallet.unwrap_or_default();
    let mut changed = false;
    if let Some(gold) = value_u32(payload.get("gold")) {
        wallet.gold = gold;
        changed = true;
    }
    if let Some(credit) = value_u32(payload.get("credit")) {
        wallet.credit = credit;
        changed = true;
    }
    if changed {
        *last_wallet = Some(wallet);
    }
}

pub fn merge_wallet_into_world(
    last_world_payload: &mut Option<Value>,
    wallet: Option<WalletState>,
) {
    let Some(payload) = last_world_payload.as_mut() else {
        return;
    };
    merge_wallet_into_payload(payload, wallet);
}

pub fn merge_wallet_into_payload(payload: &mut Value, wallet: Option<WalletState>) {
    let Some(wallet) = wallet else {
        return;
    };
    payload["gold"] = json!(wallet.gold);
    payload["credit"] = json!(wallet.credit);
}

pub fn apply_wallet_delta(
    last_wallet: &mut Option<WalletState>,
    last_world_payload: &mut Option<Value>,
    field: &str,
    amount: Option<u32>,
    gained: bool,
) -> Option<u32> {
    let amount = amount?;
    if last_wallet.is_none() {
        let mut wallet = WalletState::default();
        if let Some(payload) = last_world_payload.as_ref() {
            wallet.gold = value_u32(payload.get("gold")).unwrap_or_default();
            wallet.credit = value_u32(payload.get("credit")).unwrap_or_default();
        }
        *last_wallet = Some(wallet);
    }
    let wallet = last_wallet.as_mut()?;
    let value = match field {
        "gold" if gained => {
            wallet.gold = wallet.gold.saturating_add(amount);
            wallet.gold
        }
        "gold" => {
            wallet.gold = wallet.gold.saturating_sub(amount);
            wallet.gold
        }
        "credit" if gained => {
            wallet.credit = wallet.credit.saturating_add(amount);
            wallet.credit
        }
        "credit" => {
            wallet.credit = wallet.credit.saturating_sub(amount);
            wallet.credit
        }
        _ => return None,
    };
    merge_wallet_into_world(last_world_payload, *last_wallet);
    Some(value)
}

pub fn wallet_value(payload: &Value, field: &str) -> Option<u32> {
    value_u32(payload.get(field))
        .or_else(|| value_u32(payload.get("value")))
        .or_else(|| value_u32(payload.get("amount")))
}

fn value_u32(value: Option<&Value>) -> Option<u32> {
    value.and_then(|value| {
        value
            .as_u64()
            .and_then(|number| u32::try_from(number).ok())
            .or_else(|| value.as_str()?.parse::<u32>().ok())
    })
}

fn value_i32(value: Option<&Value>) -> Option<i32> {
    value.and_then(|value| {
        value
            .as_i64()
            .and_then(|number| i32::try_from(number).ok())
            .or_else(|| value.as_str()?.parse::<i32>().ok())
    })
}

fn value_i64(value: Option<&Value>) -> Option<i64> {
    value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str()?.parse::<i64>().ok())
    })
}

fn value_string(value: Option<&Value>) -> Option<String> {
    value.and_then(|value| match value {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    })
}
