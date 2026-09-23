//! Authoritative Hero state; separate from the player's inventory, skills and vitals.
use bevy::prelude::Resource;
use mir2_protocol::{BaseStats, ClientMagic, HeroUserInformation};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Resource, Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeroModel {
    #[serde(default)]
    pub hero_generation: u64,
    #[serde(default)]
    pub actor_candidates: Vec<HeroActorState>,
    #[serde(default)]
    pub magic_clocks: Vec<HeroMagicClock>,
    #[serde(default)]
    pub skill_snapshot_serial: u64,
    #[serde(default)]
    pub skill_key_ack: Option<crate::skill_model::SkillKeyAck>,
    #[serde(default)]
    pub learned_keys: Option<Vec<HeroLearnedKey>>,
    #[serde(default)]
    pub stats: Option<Vec<crate::read_model::CrystalPlayerStatModel>>,
    #[serde(default)]
    pub weights: Option<HeroWeights>,
    #[serde(default)]
    pub item_result_serial: u64,
    #[serde(default)]
    pub item_result_receipt: bool,
    #[serde(default)]
    pub last_item_result: Option<(String, Value)>,
    #[serde(default)]
    pub session_epoch: u64,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub info: Option<HeroUserInformation>,
    #[serde(default)]
    pub base_stats: Option<BaseStats>,
    #[serde(default)]
    pub spawned: bool,
    #[serde(default)]
    pub riding_mount: Option<bool>,
    #[serde(default)]
    pub fishing: Option<bool>,
    #[serde(default)]
    pub poison: Option<u16>,
    #[serde(default)]
    pub inventory_view: crate::inventory::InventoryModel,
    #[serde(default)]
    pub auto_pot_view: crate::inventory::InventoryModel,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HeroActorState {
    pub object_id: u32,
    pub name: Option<String>,
    pub class: Option<String>,
    pub gender: Option<String>,
    pub spawned: Option<bool>,
    pub riding_mount: Option<bool>,
    pub fishing: Option<bool>,
    pub poison: Option<u16>,
}
impl HeroActorState {
    fn matches(&self, info: &HeroUserInformation) -> bool {
        self.object_id == info.object_id
            && self.name.as_ref().is_none_or(|v| v == &info.name)
            && self
                .class
                .as_ref()
                .is_none_or(|v| v == &format!("{:?}", info.class))
            && self
                .gender
                .as_ref()
                .is_none_or(|v| v == &format!("{:?}", info.gender))
    }
    fn observe(&mut self, packet: &str, p: &Value) {
        match packet {
            "ObjectHero" => {
                self.name = p["name"].as_str().map(str::to_owned);
                self.class = p["class"].as_str().map(str::to_owned);
                self.gender = p["gender"].as_str().map(str::to_owned);
                self.spawned = Some(true);
                self.riding_mount = p["ridingMount"].as_bool();
                self.fishing = p["fishing"].as_bool();
                self.poison = p["poison"].as_u64().and_then(|v| u16::try_from(v).ok());
            }
            "ObjectRemove" => self.spawned = Some(false),
            "MountUpdate" => self.riding_mount = p["ridingMount"].as_bool(),
            "FishingUpdate" => self.fishing = p["fishing"].as_bool(),
            "ObjectPoisoned" => {
                self.poison = p["poison"].as_u64().and_then(|v| u16::try_from(v).ok())
            }
            _ => {}
        }
    }
}
/// Local monotonic anchor shared by transport and rendering in this process.
pub fn hero_clock_ms() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_millis()
        .min(u64::MAX as u128) as u64
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeroMagicClock {
    pub spell: mir2_protocol::Spell,
    pub received_ms: u64,
    /// Original CreateClientMagic sends CastTime - Envir.Time (negative elapsed).
    pub cast_offset_ms: i64,
    pub delay_ms: i64,
}
impl HeroMagicClock {
    pub fn remaining_ms(&self, now_ms: u64) -> u64 {
        let remaining = i128::from(self.received_ms)
            + i128::from(self.cast_offset_ms)
            + i128::from(self.delay_ms)
            - i128::from(now_ms);
        remaining.clamp(0, i128::from(u64::MAX)) as u64
    }
    pub fn dialog_frame(&self, now_ms: u64) -> Option<u16> {
        let left = self.remaining_ms(now_ms);
        if left < 100 || self.delay_ms < 34 {
            return None;
        }
        let per_frame = self.delay_ms / 34;
        let frame = (34_i128 - i128::from(left) / i128::from(per_frame)).clamp(0, 34);
        Some(1290 + frame as u16)
    }
}
/// A complete Hero snapshot receipt, never detached from its authoritative keys.
#[derive(Debug, Default, Resource)]
pub struct HeroModelReceipts(pub std::collections::VecDeque<HeroModel>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeroLearnedKey {
    pub spell: mir2_protocol::Spell,
    pub key: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeroKeyPending {
    pub hero_generation: u64,
    pub request_id: u64,
    pub session_epoch: u64,
    pub object_id: u32,
    pub snapshot_serial: u64,
    pub spell: mir2_protocol::Spell,
    pub key: u8,
    pub old_key: u8,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeroKeyOutcome {
    Applied,
    Unchanged,
    Rejected,
}
impl HeroKeyPending {
    /// Only a processed matching receipt and its own Hero authority may settle a draft.
    pub fn outcome(&self, model: &HeroModel) -> Option<HeroKeyOutcome> {
        let ack = model.skill_key_ack.as_ref()?;
        if model.hero_generation != self.hero_generation
            || model.session_epoch != self.session_epoch
            || model.info.as_ref()?.object_id != self.object_id
            || model.skill_snapshot_serial <= self.snapshot_serial
            || ack.request_id != self.request_id
            || ack.key != self.key
            || ack.old_key != self.old_key
            || serde_json::from_value::<mir2_protocol::Spell>(Value::String(ack.spell.clone())).ok()
                != Some(self.spell)
        {
            return None;
        }
        let actual = model
            .learned_keys
            .as_ref()?
            .iter()
            .find(|key| key.spell == self.spell)?
            .key;
        // Source C.MagicKey 0/0 enters the PLAYER branch. Its accepted bit cannot
        // establish a Hero mutation, even when both actors know the same spell.
        if self.key == 0 && self.old_key == 0 {
            return Some(if ack.accepted && actual == 0 {
                HeroKeyOutcome::Unchanged
            } else {
                HeroKeyOutcome::Rejected
            });
        }
        Some(if ack.accepted && actual == self.key {
            HeroKeyOutcome::Applied
        } else {
            HeroKeyOutcome::Rejected
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeroWeights {
    pub bag: i32,
    pub wear: i32,
    pub hand: i32,
}
impl HeroModel {
    pub fn observe_snapshot(&mut self, payload: &Value) -> bool {
        self.item_result_receipt = false;
        let Some(stage) = payload.get("stage5Systems") else {
            return false;
        };
        let mut changed = false;
        self.skill_key_ack = payload
            .get("skillKeyAck")
            .filter(|v| !v.is_null())
            .and_then(|v| serde_json::from_value::<crate::skill_model::SkillKeyAck>(v.clone()).ok())
            .filter(|ack| {
                ack.request_id != 0 && !ack.spell.is_empty() && ack.key <= 24 && ack.old_key <= 24
            });
        let mut valid_keys = false;
        if let Some(keys) = stage
            .get("heroLearnedMagics")
            .and_then(|v| serde_json::from_value::<Vec<HeroLearnedKey>>(v.clone()).ok())
            .filter(|keys| {
                keys.len() <= 256
                    && keys
                        .iter()
                        .all(|m| m.key == 0 || (17..=24).contains(&m.key))
            })
        {
            valid_keys = true;
            self.skill_snapshot_serial = self.skill_snapshot_serial.saturating_add(1);
            if let Some(info) = self.info.as_mut() {
                for magic in &mut info.magics {
                    if let Some(key) = keys.iter().find(|k| k.spell == magic.spell) {
                        magic.key = key.key;
                    }
                }
            }
            self.learned_keys = Some(keys);
            changed = true;
        }
        // A receipt is useful only with the key set from that exact snapshot.
        if !valid_keys {
            self.skill_key_ack = None;
        }
        if let Some(stats) = payload
            .get("heroStats")
            .or_else(|| stage.get("heroStats"))
            .and_then(|v| {
                serde_json::from_value::<Vec<crate::read_model::CrystalPlayerStatModel>>(v.clone())
                    .ok()
            })
        {
            if self.stats.as_ref() != Some(&stats) {
                self.stats = Some(stats);
                changed = true;
            }
        }
        if let Some(weights) = payload
            .get("heroWeights")
            .or_else(|| stage.get("heroWeights"))
            .and_then(|v| serde_json::from_value::<HeroWeights>(v.clone()).ok())
        {
            if self.weights != Some(weights) {
                self.weights = Some(weights);
                changed = true;
            }
        }
        if let Some(spawned) = stage
            .get("hero")
            .and_then(|hero| hero.get("spawned"))
            .and_then(Value::as_bool)
        {
            if spawned != self.spawned {
                self.spawned = spawned;
                changed = true;
            }
        }
        if changed {
            self.revision = self.revision.saturating_add(1);
        }
        changed
    }

    pub fn apply_packet(&mut self, packet: &str, payload: &Value) -> bool {
        self.apply_packet_at(packet, payload, hero_clock_ms())
    }
    pub fn apply_packet_at(&mut self, packet: &str, payload: &Value, now_ms: u64) -> bool {
        self.item_result_receipt = false;
        let previous_result = self.item_result_serial;
        if matches!(
            packet,
            "ObjectHero" | "ObjectRemove" | "MountUpdate" | "FishingUpdate" | "ObjectPoisoned"
        ) {
            if let Some(id) = payload["objectId"]
                .as_u64()
                .and_then(|v| u32::try_from(v).ok())
            {
                let mut incoming = HeroActorState {
                    object_id: id,
                    ..Default::default()
                };
                incoming.observe(packet, payload);
                if self
                    .info
                    .as_ref()
                    .is_none_or(|info| !incoming.matches(info))
                {
                    if packet != "ObjectRemove"
                        || self.actor_candidates.iter().any(|c| c.object_id == id)
                    {
                        if let Some(old) =
                            self.actor_candidates.iter_mut().find(|c| c.object_id == id)
                        {
                            old.observe(packet, payload);
                        } else {
                            if self.actor_candidates.len() >= 64 {
                                self.actor_candidates.remove(0);
                            }
                            self.actor_candidates.push(incoming);
                        }
                    }
                    return false;
                }
            }
        }
        let changed = match packet {
            "UpdateHeroSpawnState" => {
                let Some(state) = payload
                    .get("state")
                    .and_then(Value::as_u64)
                    .filter(|v| *v <= 3)
                else {
                    return false;
                };
                self.spawned = state >= 2;
                true
            }

            "MergeItem"
                if ["gridFrom", "gridTo"].iter().any(|key| {
                    matches!(
                        payload.get(*key).and_then(Value::as_str),
                        Some("HeroInventory" | "HeroEquipment")
                    )
                }) =>
            {
                if self.info.is_none() || payload.get("success").and_then(Value::as_bool).is_none()
                {
                    return false;
                }
                self.item_result_serial = self.item_result_serial.saturating_add(1);
                self.last_item_result = Some((packet.into(), payload.clone()));
                true
            }
            "TransferHeroItem" | "TakeBackHeroItem" => {
                if self.info.is_none() || payload.get("success").and_then(Value::as_bool).is_none()
                {
                    return false;
                }
                self.item_result_serial = self.item_result_serial.saturating_add(1);
                self.last_item_result = Some((packet.into(), payload.clone()));
                true
            }
            "SetAutoPotValue" => {
                let (Some(info), Some(stat), Some(value)) = (
                    self.info.as_mut(),
                    payload.get("stat").and_then(Value::as_u64),
                    payload.get("value").and_then(Value::as_u64),
                ) else {
                    return false;
                };
                if !matches!(stat, 12 | 13) || value > 99 {
                    return false;
                }
                if stat == 12 {
                    info.auto_hp_percent = value as u8;
                } else {
                    info.auto_mp_percent = value as u8;
                }
                self.item_result_serial = self.item_result_serial.saturating_add(1);
                self.last_item_result = Some((packet.into(), payload.clone()));
                true
            }
            "SetAutoPotItem" => {
                let (Some(info), Some(grid), Some(index)) = (
                    self.info.as_mut(),
                    payload.get("grid").and_then(Value::as_u64),
                    payload
                        .get("item_index")
                        .or_else(|| payload.get("itemIndex"))
                        .and_then(Value::as_i64)
                        .and_then(|v| i32::try_from(v).ok()),
                ) else {
                    return false;
                };
                if grid == mir2_protocol::MirGridType::HeroHpItem as u64 {
                    info.hp_item_index = index;
                } else if grid == mir2_protocol::MirGridType::HeroMpItem as u64 {
                    info.mp_item_index = index;
                } else {
                    return false;
                }
                self.item_result_serial = self.item_result_serial.saturating_add(1);
                self.last_item_result = Some((packet.into(), payload.clone()));
                true
            }
            "MoveItem" | "EquipItem" | "RemoveItem" | "UseItem"
                if payload.get("grid").and_then(Value::as_str) == Some("HeroInventory") =>
            {
                let Some(success) = payload.get("success").and_then(Value::as_bool) else {
                    return false;
                };
                let Some(info) = self.info.as_mut() else {
                    return false;
                };
                if packet == "MoveItem" && success {
                    let (Some(from), Some(to), Some(items)) = (
                        payload
                            .get("from")
                            .and_then(Value::as_u64)
                            .and_then(|v| usize::try_from(v).ok()),
                        payload
                            .get("to")
                            .and_then(Value::as_u64)
                            .and_then(|v| usize::try_from(v).ok()),
                        info.inventory.as_mut(),
                    ) else {
                        return false;
                    };
                    if from >= items.len() || to >= items.len() {
                        return false;
                    }
                    items.swap(from, to);
                }
                self.item_result_serial = self.item_result_serial.saturating_add(1);
                self.last_item_result = Some((packet.to_owned(), payload.clone()));
                true
            }
            "DeleteItem" => {
                let (Some(uid), Some(count), Some(items)) = (
                    payload.get("uniqueId").and_then(Value::as_u64),
                    payload
                        .get("count")
                        .and_then(Value::as_u64)
                        .and_then(|v| u16::try_from(v).ok()),
                    self.info.as_mut().and_then(|info| info.inventory.as_mut()),
                ) else {
                    return false;
                };
                let Some(slot) = items
                    .iter_mut()
                    .find(|item| item.as_ref().is_some_and(|item| item.unique_id == uid))
                else {
                    return false;
                };
                if slot.as_ref().unwrap().count <= count {
                    *slot = None;
                } else {
                    slot.as_mut().unwrap().count -= count;
                }
                true
            }
            "HeroInformation" => {
                let Some(info) = payload
                    .get("info")
                    .and_then(|v| serde_json::from_value::<HeroUserInformation>(v.clone()).ok())
                else {
                    return false;
                };
                if info.inventory.as_ref().is_some_and(|v| v.len() > 42)
                    || info.equipment.as_ref().is_some_and(|v| v.len() != 14)
                    || info.magics.len() > 256
                {
                    return false;
                }
                if self.info.as_ref().is_none_or(|old| {
                    old.object_id != info.object_id
                        || old.name != info.name
                        || old.class != info.class
                        || old.gender != info.gender
                }) {
                    self.hero_generation = self.hero_generation.saturating_add(1);
                    self.spawned = false;
                    self.riding_mount = None;
                    self.fishing = None;
                    self.poison = None;
                    self.stats = None;
                    self.weights = None;
                    self.learned_keys = None;
                    self.skill_key_ack = None;
                }
                self.magic_clocks = info
                    .magics
                    .iter()
                    .map(|magic| HeroMagicClock {
                        spell: magic.spell,
                        received_ms: now_ms,
                        cast_offset_ms: magic.cast_time,
                        delay_ms: magic.delay,
                    })
                    .collect();
                if let Some(index) = self
                    .actor_candidates
                    .iter()
                    .position(|state| state.matches(&info))
                {
                    let actor = self.actor_candidates.remove(index);
                    if let Some(spawned) = actor.spawned {
                        self.spawned = spawned;
                    }
                    self.riding_mount = actor.riding_mount;
                    self.fishing = actor.fishing;
                    self.poison = actor.poison;
                }
                self.actor_candidates
                    .retain(|state| state.object_id != info.object_id);
                self.info = Some(info);
                true
            }
            "HeroBaseStatsInfo" => {
                let Some(stats) = payload
                    .get("stats")
                    .and_then(|v| serde_json::from_value::<BaseStats>(v.clone()).ok())
                else {
                    return false;
                };
                self.base_stats = Some(stats);
                true
            }
            "HeroHealthChanged" => {
                let (Some(hp), Some(mp), Some(info)) = (
                    payload
                        .get("hp")
                        .and_then(Value::as_i64)
                        .and_then(|v| i32::try_from(v).ok()),
                    payload
                        .get("mp")
                        .and_then(Value::as_i64)
                        .and_then(|v| i32::try_from(v).ok()),
                    self.info.as_mut(),
                ) else {
                    return false;
                };
                info.hp = hp;
                info.mp = mp;
                true
            }
            "MountUpdate" | "FishingUpdate" | "ObjectPoisoned" => {
                if self.info.as_ref().is_none_or(|info| {
                    payload["objectId"].as_u64() != Some(u64::from(info.object_id))
                }) {
                    return false;
                }
                match packet {
                    "MountUpdate" => {
                        let Some(value) = payload["ridingMount"].as_bool() else {
                            return false;
                        };
                        self.riding_mount = Some(value);
                    }
                    "FishingUpdate" => {
                        let Some(value) = payload["fishing"].as_bool() else {
                            return false;
                        };
                        self.fishing = Some(value);
                    }
                    _ => {
                        let Some(value) = payload["poison"]
                            .as_u64()
                            .and_then(|v| u16::try_from(v).ok())
                        else {
                            return false;
                        };
                        self.poison = Some(value);
                    }
                }
                true
            }
            "ObjectHero" | "ObjectRemove" | "ObjectDied" => {
                let Some(info) = self.info.as_mut() else {
                    return false;
                };
                let id = payload
                    .get("objectId")
                    .or_else(|| payload.get("info").and_then(|info| info.get("object_id")))
                    .and_then(Value::as_u64);
                if id != Some(u64::from(info.object_id)) {
                    return false;
                }
                match packet {
                    "ObjectHero" => {
                        self.spawned = true;
                        self.riding_mount = payload["ridingMount"].as_bool();
                        self.fishing = payload["fishing"].as_bool();
                        self.poison = payload["poison"]
                            .as_u64()
                            .and_then(|v| u16::try_from(v).ok());
                    }
                    "ObjectRemove" => self.spawned = false,
                    _ => info.hp = 0,
                }
                true
            }
            "NewMagic" if payload.get("hero").and_then(Value::as_bool) == Some(true) => {
                let Some(magic) = payload
                    .get("magic")
                    .and_then(|v| serde_json::from_value::<ClientMagic>(v.clone()).ok())
                else {
                    return false;
                };
                let Some(info) = self.info.as_mut() else {
                    return false;
                };
                let clock = HeroMagicClock {
                    spell: magic.spell,
                    received_ms: now_ms,
                    cast_offset_ms: magic.cast_time,
                    delay_ms: magic.delay,
                };
                if let Some(old) = info.magics.iter_mut().find(|old| old.spell == magic.spell) {
                    *old = magic;
                } else if info.magics.len() < 256 {
                    info.magics.push(magic);
                } else {
                    return false;
                }
                self.magic_clocks.retain(|old| old.spell != clock.spell);
                self.magic_clocks.push(clock);
                true
            }
            "ObjectMagic" => {
                if payload.get("cast").and_then(Value::as_bool) != Some(true) {
                    return false;
                }
                let Some(info) = self.info.as_ref() else {
                    return false;
                };
                if payload.get("objectId").and_then(Value::as_u64)
                    != Some(u64::from(info.object_id))
                {
                    return false;
                }
                let Some(spell) = payload
                    .get("spell")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                else {
                    return false;
                };
                let Some(magic) = info.magics.iter().find(|m| m.spell == spell) else {
                    return false;
                };
                self.magic_clocks.retain(|clock| clock.spell != spell);
                self.magic_clocks.push(HeroMagicClock {
                    spell,
                    received_ms: now_ms,
                    cast_offset_ms: 0,
                    delay_ms: magic.delay,
                });
                true
            }
            "MagicDelay" | "MagicLeveled" => {
                let Some(info) = self.info.as_mut() else {
                    return false;
                };
                if payload.get("objectId").and_then(Value::as_u64)
                    != Some(u64::from(info.object_id))
                {
                    return false;
                }
                let Some(spell) = payload
                    .get("spell")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                else {
                    return false;
                };
                let Some(magic) = info.magics.iter_mut().find(|magic| magic.spell == spell) else {
                    return false;
                };
                if packet == "MagicDelay" {
                    let Some(delay) = payload
                        .get("delay")
                        .and_then(Value::as_i64)
                        .and_then(|v| i64::try_from(v).ok())
                    else {
                        return false;
                    };
                    magic.delay = delay;
                    if let Some(clock) = self
                        .magic_clocks
                        .iter_mut()
                        .find(|clock| clock.spell == spell)
                    {
                        clock.delay_ms = delay;
                    }
                } else {
                    let (Some(level), Some(experience)) = (
                        payload
                            .get("level")
                            .and_then(Value::as_u64)
                            .and_then(|v| u8::try_from(v).ok()),
                        payload
                            .get("experience")
                            .and_then(Value::as_u64)
                            .and_then(|v| u16::try_from(v).ok()),
                    ) else {
                        return false;
                    };
                    magic.level = level;
                    magic.experience = experience;
                }
                true
            }
            // Actorless S.MagicCast is the player's clock, never an inferred Hero cast.
            _ => false,
        };
        if changed {
            self.item_result_receipt = self.item_result_serial != previous_result;
            self.skill_key_ack = None;
            self.revision = self.revision.saturating_add(1);
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn info() -> HeroUserInformation {
        HeroUserInformation {
            object_id: 12,
            name: "Hero".into(),
            class: mir2_protocol::MirClass::Warrior,
            gender: mir2_protocol::MirGender::Male,
            level: 1,
            hair: 0,
            hp: 10,
            mp: 5,
            experience: 0,
            max_experience: 100,
            inventory: Some(vec![None; 10]),
            equipment: Some(vec![None; 14]),
            magics: vec![],
            auto_pot: false,
            auto_hp_percent: 30,
            auto_mp_percent: 30,
            hp_item_index: 0,
            mp_item_index: 0,
        }
    }
    #[test]
    fn hero_cast_clock_reanchors_negative_elapsed_without_restarting_cooldown() {
        let magic: ClientMagic = serde_json::from_value(serde_json::json!({
            "name":"Fire Ball","spell":"FireBall","base_cost":1,"level_cost":0,"icon":1,
            "level1":1,"level2":2,"level3":3,"need1":1,"need2":2,"need3":3,
            "level":1,"key":17,"experience":0,"delay":3400,"range":8,"cast_time":-1000
        }))
        .unwrap();
        let mut full = info();
        full.magics.push(magic);
        let mut model = HeroModel::default();
        assert!(model.apply_packet_at("HeroInformation", &serde_json::json!({"info":full}), 100));
        assert_eq!(model.magic_clocks[0].remaining_ms(100), 2400);
        assert_eq!(model.magic_clocks[0].dialog_frame(100), Some(1300));
        full.magics[0].cast_time = -2000;
        model.apply_packet_at("HeroInformation", &serde_json::json!({"info":full}), 1100);
        assert_eq!(model.magic_clocks[0].remaining_ms(1100), 1400);
        assert!(!model.apply_packet_at(
            "ObjectMagic",
            &serde_json::json!({"objectId":11,"spell":"FireBall","cast":true}),
            1200
        ));
        assert!(!model.apply_packet_at(
            "MagicCast",
            &serde_json::json!({"spell":"FireBall"}),
            1200
        ));
        assert!(!model.apply_packet_at(
            "ObjectMagic",
            &serde_json::json!({"objectId":12,"spell":"FireBall","cast":false}),
            1200
        ));
        assert_eq!(model.magic_clocks[0].remaining_ms(1200), 1300);
        assert!(model.apply_packet_at(
            "ObjectMagic",
            &serde_json::json!({"objectId":12,"spell":"FireBall","cast":true}),
            1200
        ));
        assert_eq!(model.magic_clocks[0].remaining_ms(1200), 3400);
        model.apply_packet_at(
            "MagicDelay",
            &serde_json::json!({"objectId":12,"spell":"FireBall","delay":2000}),
            1300,
        );
        assert_eq!(model.magic_clocks[0].remaining_ms(1300), 1900);
        full.magics[0].cast_time = -999999;
        model.apply_packet_at("HeroInformation", &serde_json::json!({"info":full}), 1400);
        assert_eq!(model.magic_clocks[0].remaining_ms(1400), 0);
        assert_eq!(model.magic_clocks[0].dialog_frame(1400), None);
        let long = HeroMagicClock {
            spell: mir2_protocol::Spell::FireBall,
            received_ms: 0,
            cast_offset_ms: 0,
            delay_ms: 34000,
        };
        assert_eq!(long.dialog_frame(33800), Some(1324));
        assert_eq!(long.dialog_frame(33901), None);
        let short = HeroMagicClock {
            delay_ms: 300,
            ..long.clone()
        };
        assert_eq!(short.dialog_frame(0), Some(1290));
        let zero_divisor = HeroMagicClock {
            delay_ms: 33,
            cast_offset_ms: 100,
            ..long
        };
        assert_eq!(zero_divisor.dialog_frame(0), None);
    }
    #[test]
    fn full_bootstrap_and_live_health_are_independent_of_player_data() {
        let mut model = HeroModel::default();
        assert!(!model.apply_packet("HeroHealthChanged", &serde_json::json!({"hp":30,"mp":9})));
        assert!(model.apply_packet("HeroInformation", &serde_json::json!({"info":info()})));
        assert_eq!(
            model
                .info
                .as_ref()
                .unwrap()
                .inventory
                .as_ref()
                .unwrap()
                .len(),
            10
        );
        assert!(model.apply_packet("HeroHealthChanged", &serde_json::json!({"hp":7,"mp":2})));
        assert_eq!(model.info.as_ref().unwrap().hp, 7);
        assert!(!model.apply_packet("HealthChanged", &serde_json::json!({"hp":999,"mp":999})));
        assert!(!model.apply_packet("MagicCast", &serde_json::json!({"spell":"FireBall"})));
        assert_eq!(model.revision, 2);
        let copy: HeroModel =
            serde_json::from_value(serde_json::to_value(&model).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(model).unwrap(),
            serde_json::to_value(copy).unwrap()
        );
    }
    #[test]
    fn hero_receipt_uses_exact_actor_epoch_keys_and_preserves_zero_zero_source_routing() {
        let mut model = HeroModel {
            session_epoch: 7,
            info: Some(info()),
            ..Default::default()
        };
        let mut pending = HeroKeyPending {
            hero_generation: 0,
            request_id: 51,
            session_epoch: 7,
            object_id: 12,
            snapshot_serial: 0,
            spell: mir2_protocol::Spell::FireBall,
            key: 0,
            old_key: 0,
        };
        // The player FireBall was bound to F1 and now cleared by the source 0/0 branch.
        // Hero FireBall stays unbound: player success is not a Hero Applied outcome.
        assert!(model.observe_snapshot(&serde_json::json!({
            "knownSkills":[{"spell":"FireBall","hotkey":0}],
            "stage5Systems":{"heroLearnedMagics":[{"spell":"FireBall","key":0}]},
            "skillKeyAck":{"requestId":51,"spell":"FireBall","key":0,"oldKey":0,"accepted":true}
        })));
        assert_eq!(pending.outcome(&model), Some(HeroKeyOutcome::Unchanged));
        model.skill_key_ack.as_mut().unwrap().accepted = false;
        assert_eq!(pending.outcome(&model), Some(HeroKeyOutcome::Rejected));
        model.skill_key_ack.as_mut().unwrap().accepted = true;
        pending.object_id = 13;
        assert_eq!(pending.outcome(&model), None);
        pending.object_id = 12;
        pending.session_epoch = 8;
        assert_eq!(pending.outcome(&model), None);
        pending.session_epoch = 7;
        pending.key = 24;
        assert!(model.observe_snapshot(&serde_json::json!({
            "stage5Systems":{"heroLearnedMagics":[{"spell":"FireBall","key":24}]},
            "skillKeyAck":{"requestId":51,"spell":"FireBall","key":24,"oldKey":0,"accepted":true}
        })));
        assert_eq!(pending.outcome(&model), Some(HeroKeyOutcome::Applied));
        pending.request_id = 52;
        assert_eq!(pending.outcome(&model), None);
        // Malformed/missing authoritative keys never reuse the previous set with a new receipt.
        model.observe_snapshot(&serde_json::json!({"stage5Systems":{},"skillKeyAck":{"requestId":52,"spell":"FireBall","key":24,"oldKey":0,"accepted":true}}));
        assert!(model.skill_key_ack.is_none());
    }
    #[test]
    fn early_actor_is_applied_only_after_exact_hero_information_and_identity_match() {
        let mut model = HeroModel::default();
        assert!(!model.apply_packet("ObjectHero",&serde_json::json!({"objectId":12,"name":"Hero","class":"Warrior","gender":"Male","ridingMount":true,"fishing":false,"poison":0})));
        assert!(model.info.is_none());
        assert!(!model.spawned);
        model.apply_packet(
            "MountUpdate",
            &serde_json::json!({"objectId":12,"ridingMount":false}),
        );
        model.apply_packet(
            "ObjectHero",
            &serde_json::json!({"objectId":99,"name":"Other","ridingMount":true}),
        );
        model.apply_packet("HeroInformation", &serde_json::json!({"info":info()}));
        assert!(model.spawned);
        assert_eq!(model.riding_mount, Some(false));
        assert!(!model.apply_packet(
            "MountUpdate",
            &serde_json::json!({"objectId":99,"ridingMount":true})
        ));
        assert_eq!(model.riding_mount, Some(false));
        let mut next = info();
        next.name = "Replacement".into();
        model.apply_packet("HeroInformation", &serde_json::json!({"info":next}));
        assert!(!model.spawned);
        assert_eq!(model.riding_mount, None);
        let reset = HeroModel::default();
        assert!(reset.actor_candidates.is_empty());
    }
    #[test]
    fn item_result_receipt_marks_each_operation_but_not_later_snapshots_or_health() {
        let mut model = HeroModel {
            info: Some(info()),
            ..Default::default()
        };
        assert!(model.apply_packet(
            "UseItem",
            &serde_json::json!({"grid":"HeroInventory","uniqueId":71,"success":false})
        ));
        assert!(model.item_result_receipt);
        assert!(model.apply_packet(
            "SetAutoPotValue",
            &serde_json::json!({"stat":12,"value":30})
        ));
        assert!(model.item_result_receipt);
        assert_eq!(model.item_result_serial, 2);
        assert!(model.apply_packet("HeroHealthChanged", &serde_json::json!({"hp":7,"mp":2})));
        assert!(!model.item_result_receipt);
        model.observe_snapshot(&serde_json::json!({"stage5Systems":{}}));
        assert!(!model.item_result_receipt);
    }
    #[test]
    fn auto_pot_echo_uses_crystal_stats_and_switching_identity_invalidates_pending_generation() {
        let mut model = HeroModel::default();
        model.apply_packet("HeroInformation", &serde_json::json!({"info":info()}));
        assert!(!model.apply_packet("SetAutoPotValue", &serde_json::json!({"stat":0,"value":40})));
        assert!(model.apply_packet(
            "SetAutoPotValue",
            &serde_json::json!({"stat":12,"value":40})
        ));
        assert_eq!(model.info.as_ref().unwrap().auto_hp_percent, 40);
        assert_eq!(model.info.as_ref().unwrap().auto_mp_percent, 30);
        let generation = model.hero_generation;
        let mut changed = info();
        changed.name = "Different Hero".into();
        model.apply_packet("HeroInformation", &serde_json::json!({"info":changed}));
        assert!(model.hero_generation > generation);
        assert_eq!(model.info.as_ref().unwrap().object_id, 12);
    }
}
