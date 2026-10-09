//! Authoritative Hero state; separate from the player's inventory, skills and vitals.
use bevy::prelude::Resource;
use mir2_protocol::{BaseStats, ClientMagic, HeroUserInformation};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use crate::inventory::ItemModel;
use serde_json::Value;

#[derive(Resource, Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeroModel {
    /// Public owner-checkpoint identity and XP. Packet-only object ID/hair
    /// remain in info; neither is manufactured from a personal checkpoint.
    #[serde(default)]
    pub snapshot_identity: Option<HeroSnapshotIdentity>,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeroSnapshotIdentity {
    pub name: String,
    pub class: mir2_protocol::MirClass,
    pub gender: mir2_protocol::MirGender,
    pub level: u16,
    pub experience: i64,
    pub max_experience: i64,
    pub behaviour: u8,
    pub spawned: bool,
    pub auto_pot: bool,
    pub auto_hp_percent: u8,
    pub auto_mp_percent: u8,
    pub hp_item_index: i32,
    pub mp_item_index: i32,
    pub vitals: Option<HeroSnapshotVitals>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeroSnapshotVitals { pub hp:i32,pub max_hp:i32,pub mp:i32,pub max_mp:i32 }
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
    static START: std::sync::OnceLock<web_time::Instant> = std::sync::OnceLock::new();
    START
        .get_or_init(web_time::Instant::now)
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
    /// The renderer uses checkpoint XP/max-XP whenever that exact public
    /// source is present. Legacy packet-only sessions keep their old display.
    pub fn snapshot_read_model(&self) -> Option<crate::read_model::UiReadModel> {
        let hero=self.snapshot_identity.as_ref()?;
        let mut player=crate::read_model::PlayerStats{name:Some(hero.name.clone()),class_name:Some(format!("{:?}",hero.class)),
            gender:Some(format!("{:?}",hero.gender)),level:u32::from(hero.level),experience:hero.experience,max_experience:hero.max_experience,
            crystal_stats:self.stats.clone(),..Default::default()};
        if let Some(v)=hero.vitals {player.hp=v.hp;player.max_hp=v.max_hp;player.mp=v.mp;player.max_mp=v.max_mp;}
        Some(crate::read_model::UiReadModel{player})
    }
    pub fn observe_snapshot(&mut self, payload: &Value) -> bool {
        self.item_result_receipt = false;
        let Some(stage) = payload.get("stage5Systems") else {
            return false;
        };
        let mut changed = false;
        if payload.get("heroMaxExperience").is_some() {
            let identity=stage.get("hero").filter(|hero|!hero.is_null()).and_then(|hero| {
                let mut source=hero.as_object()?.clone();
                source.insert("maxExperience".into(),payload.get("heroMaxExperience")?.clone());
                source.insert("vitals".into(),payload.get("heroVitals")?.clone());
                serde_json::from_value::<HeroSnapshotIdentity>(Value::Object(source)).ok().filter(|hero|hero.level>0 && hero.max_experience>=0)
            });
            if self.snapshot_identity!=identity {self.snapshot_identity=identity;changed=true;}
        }
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
                    if self.snapshot_identity.as_ref().is_some_and(|source|source.name!=info.name||source.class!=info.class||source.gender!=info.gender) {
                        self.snapshot_identity=None;
                    }
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


/// A complete owner projection error, independent of host provenance or settlement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeroOwnerProjectionError {
    Incomplete,
    Decode,
    Projection,
}

// Shared strict owner projection preserves complete item and tooltip carriers
// before the presentation models apply tolerant defaults.
fn decode_source<T: DeserializeOwned>(raw: Value) -> Result<T, HeroOwnerProjectionError> {
    serde_json::from_value(raw).map_err(|_| HeroOwnerProjectionError::Incomplete)
}

fn required_string<'a>(source: &'a Value, key: &str) -> Result<&'a str, HeroOwnerProjectionError> {
    source.get(key).and_then(Value::as_str).ok_or(HeroOwnerProjectionError::Incomplete)
}
fn required_bool(source: &Value, key: &str) -> Result<bool, HeroOwnerProjectionError> {
    source.get(key).and_then(Value::as_bool).ok_or(HeroOwnerProjectionError::Incomplete)
}
fn source_array<'a>(source: &'a Value, key: &str) -> Result<&'a Vec<Value>, HeroOwnerProjectionError> {
    source.get(key).and_then(Value::as_array).ok_or(HeroOwnerProjectionError::Incomplete)
}


pub fn project_owner_item_tooltip(raw: Option<&Value>) -> Result<Option<crate::inventory::CrystalItemTooltipSourceModel>, HeroOwnerProjectionError> {
    let Some(raw) = raw else { return Ok(None); };
    if !raw.is_object() { return Err(HeroOwnerProjectionError::Incomplete); }
    fn info(raw: &Value) -> Result<(), HeroOwnerProjectionError> {
        let parsed: crate::inventory::CrystalItemInfoModel = decode_source(raw.clone())?;
        let shape = serde_json::to_value(parsed).map_err(|_| HeroOwnerProjectionError::Decode)?;
        if shape.as_object().unwrap().keys().any(|key| raw.get(key).is_none())
            || source_array(raw,"stats")?.iter().any(|s| s.get("stat").and_then(Value::as_u64).is_none()
                || s.get("value").and_then(Value::as_i64).is_none()) { return Err(HeroOwnerProjectionError::Incomplete); }
        Ok(())
    }
    info(raw.get("info").ok_or(HeroOwnerProjectionError::Incomplete)?)?;
    if let Some(real) = raw.get("realInfo").filter(|v| !v.is_null()) { info(real)?; }
    if let Some(user) = raw.get("userItem").filter(|v| !v.is_null()) {
        if !crate::npc_shop_buy::full_npc_gold_user_item(user) { return Err(HeroOwnerProjectionError::Incomplete); }
    }
    for key in ["socketInfos","realSocketInfos"] {
        if let Some(rows) = raw.get(key) {
            for row in rows.as_array().ok_or(HeroOwnerProjectionError::Incomplete)? {
                if !row.is_null() { info(row)?; }
            }
        }
    }
    decode_source(raw.clone()).map(Some)
}

pub fn project_owner_item(raw: &Value, container: u8, slot: u64) -> Result<ItemModel, HeroOwnerProjectionError> {
    let mut item = raw.as_object().cloned().ok_or(HeroOwnerProjectionError::Incomplete)?;
    item.insert("container".into(), container.into());item.insert("slot".into(), slot.into());
    let mut projected: ItemModel = decode_source(Value::Object(item))?;
    projected.tooltip_source = project_owner_item_tooltip(raw.get("tooltipSource"))?;
    if let Some(source) = &projected.tooltip_source { projected.icon = source.user_item_image(projected.quantity); }
    validate_owner_item_metadata(raw,&projected,false,false)?;
    Ok(projected)
}

pub fn project_owner_hero(owner: &Value) -> Result<HeroModel, HeroOwnerProjectionError> {
    let stage = owner.get("stage5Systems").ok_or(HeroOwnerProjectionError::Incomplete)?;
    let mut hero = HeroModel::default();
    let maximum=owner.get("heroMaxExperience").ok_or(HeroOwnerProjectionError::Incomplete)?;
    let identity=stage.get("hero").ok_or(HeroOwnerProjectionError::Incomplete)?;
    if identity.is_null() {
        if !maximum.is_null() { return Err(HeroOwnerProjectionError::Incomplete); }
    } else if maximum.as_i64().is_none_or(|xp|xp<0) { return Err(HeroOwnerProjectionError::Incomplete); }
    let keys = source_array(stage,"heroLearnedMagics")?;
    if keys.len()>256 || keys.iter().any(|key| key.get("spell").and_then(Value::as_str).is_none()
        || key.get("key").and_then(Value::as_u64).is_none_or(|key| key != 0 && !(17..=24).contains(&key))) {
        return Err(HeroOwnerProjectionError::Incomplete);
    }
    let _: Vec<HeroLearnedKey> = decode_source(Value::Array(keys.clone()))?;
    for stat in source_array(owner,"heroStats")? {
        if stat.get("stat").and_then(Value::as_u64).is_none() || stat.get("value").and_then(Value::as_i64).is_none() {
            return Err(HeroOwnerProjectionError::Incomplete);
        }
    }
    let weights = owner.get("heroWeights").ok_or(HeroOwnerProjectionError::Incomplete)?;
    let _: HeroWeights = decode_source(weights.clone())?;
    if let Some(identity) = stage.get("hero").filter(|v| !v.is_null()) {
        for key in ["name","class","gender"] { required_string(identity,key)?; }
        for key in ["level","behaviour","autoHpPercent","autoMpPercent"] { field_unsigned(identity,key)?; }
        for key in ["spawned","autoPot"] { required_bool(identity,key)?; }
        for key in ["experience","hpItemIndex","mpItemIndex"] {
            if identity.get(key).and_then(Value::as_i64).is_none() { return Err(HeroOwnerProjectionError::Incomplete); }
        }
    }
    hero.observe_snapshot(owner);
    if !identity.is_null() && hero.snapshot_identity.is_none() { return Err(HeroOwnerProjectionError::Incomplete); }
    hero.inventory_view.capacity = u16::try_from(field_unsigned(owner,"heroInventoryCapacity")?).map_err(|_| HeroOwnerProjectionError::Incomplete)?;
    for raw in source_array(owner,"heroInventoryItems")? {
        let slot = field_unsigned(raw,"slot")?;
        if raw.get("container").and_then(Value::as_str) != Some("bag1") || slot >= u64::from(hero.inventory_view.capacity) {
            return Err(HeroOwnerProjectionError::Incomplete);
        }
        hero.inventory_view.items.push(project_owner_item(raw,0,slot)?);
    }
    for raw in source_array(owner,"heroEquipmentItems")? {
        let slot = field_unsigned(raw,"slot")?;
        if slot>=14 || raw.get("container").and_then(Value::as_str)!=Some("bag1") { return Err(HeroOwnerProjectionError::Incomplete); }
        hero.inventory_view.items.push(project_owner_item(raw,2,slot)?);
    }
    Ok(hero)
}

fn unsigned(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_str().filter(|s| *s == "0" || (!s.starts_with('0') && s.bytes().all(|b| b.is_ascii_digit())))?.parse().ok())
}
fn field_unsigned(v: &Value, key: &str) -> Result<u64, HeroOwnerProjectionError> {
    v.get(key).and_then(unsigned).ok_or(HeroOwnerProjectionError::Incomplete)
}
pub fn validate_owner_item_metadata(source: &Value, projected: &ItemModel, equipment: bool, storage: bool) -> Result<(), HeroOwnerProjectionError> {
    for field in ["key", "name", "description", "grade"] {
        if source.get(field).and_then(Value::as_str).is_none() { return Err(HeroOwnerProjectionError::Incomplete); }
    }
    if source.get("icon").and_then(Value::as_u64).is_none_or(|n| n > u64::from(u16::MAX)) {
        return Err(HeroOwnerProjectionError::Incomplete);
    }
    let expected_key = if storage { field_unsigned(source, "uniqueId")?.to_string() }
        else { source["key"].as_str().unwrap().to_owned() };
    if projected.key != expected_key || Some(projected.name.as_str()) != source["name"].as_str()
        || Some(projected.description.as_str()) != source["description"].as_str()
        || projected.grade.as_deref() != source["grade"].as_str() {
        return Err(HeroOwnerProjectionError::Projection);
    }
    // The gateway derives count-dependent icons from tooltip Info when it is
    // present. Raw icon equality only applies to the legacy direct mapping.
    if source.get("tooltipSource").is_none() && Some(u64::from(projected.icon)) != source["icon"].as_u64() {
        return Err(HeroOwnerProjectionError::Projection);
    }
    let tooltip=project_owner_item_tooltip(source.get("tooltipSource"))?;
    if tooltip.as_ref().and_then(|tooltip|tooltip.user_item.as_ref()).is_some_and(|user|
        source.get("uniqueId").and_then(unsigned).is_some_and(|id|id!=user.unique_id)
        || source.get("quantity").and_then(unsigned)!=Some(u64::from(user.count))) {
        return Err(HeroOwnerProjectionError::Incomplete);
    }
    if projected.tooltip_source!=tooltip || tooltip.as_ref().is_some_and(|source|projected.icon!=source.user_item_image(projected.quantity)) {
        return Err(HeroOwnerProjectionError::Projection);
    }
    let model = serde_json::to_value(projected).map_err(|_| HeroOwnerProjectionError::Decode)?;
    for field in ["durabilityCurrent", "durabilityMax"] {
        let raw = source.get(field).ok_or(HeroOwnerProjectionError::Incomplete)?;
        if !(raw.is_null() && !equipment) && raw.as_u64().is_none_or(|n| n > u64::from(u16::MAX)) {
            return Err(HeroOwnerProjectionError::Incomplete);
        }
        if model[field] != *raw { return Err(HeroOwnerProjectionError::Projection); }
    }
    for field in ["addedAttack", "addedDefence"] {
        let raw = source.get(field).ok_or(HeroOwnerProjectionError::Incomplete)?;
        if raw.as_i64().is_none_or(|n| i32::try_from(n).is_err()) { return Err(HeroOwnerProjectionError::Incomplete); }
        if model[field] != *raw { return Err(HeroOwnerProjectionError::Projection); }
    }
    let extra: &[&str] = if equipment { &["stateImage", "shape", "attack", "defence", "addedLuck", "socketSlots"] }
        else { &["sellValue"] };
    for field in extra {
        let raw = source.get(*field).ok_or(HeroOwnerProjectionError::Incomplete)?;
        if model[*field] != *raw { return Err(HeroOwnerProjectionError::Projection); }
    }
    if source.get("equipSlot").is_some_and(|raw| model["equipSlot"] != *raw) { return Err(HeroOwnerProjectionError::Projection); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_npc_economy_hero_visible_xp_uses_checkpoint_without_packet_identity() {
        let mut model=HeroModel::default();
        let snapshot=serde_json::json!({"heroMaxExperience":200,"heroVitals":{"hp":20,"maxHp":30,"mp":10,"maxMp":15},
            "heroStats":[],"heroWeights":{"bag":1,"wear":2,"hand":3},"stage5Systems":{"heroLearnedMagics":[],
                "hero":{"name":"Hero","class":"Warrior","gender":"Male","level":2,"experience":77,"behaviour":0,"spawned":true,
                    "autoPot":false,"autoHpPercent":30,"autoMpPercent":40,"hpItemIndex":0,"mpItemIndex":0}}});
        assert!(model.observe_snapshot(&snapshot));assert!(model.info.is_none());
        let display=model.snapshot_read_model().unwrap();assert_eq!((display.player.experience,display.player.max_experience),(77,200));
        assert_eq!(display.player.hair,None);assert_eq!(display.player.hp,20);
        let mut packet=info();packet.experience=1;packet.max_experience=100;
        model.apply_packet_at("HeroInformation",&serde_json::json!({"info":packet}),0);
        assert_eq!(model.snapshot_read_model().unwrap().player.experience,77);
        let mut next=snapshot;next["stage5Systems"]["hero"]["experience"]=serde_json::json!(88);next["heroMaxExperience"]=serde_json::json!(300);
        assert!(model.observe_snapshot(&next));assert_eq!(model.snapshot_read_model().unwrap().player.max_experience,300);
    }
    #[test]
    fn native_npc_economy_hero_absent_or_partial_maximum_cannot_invent_visible_pair() {
        let mut model=HeroModel::default();
        model.observe_snapshot(&serde_json::json!({"heroMaxExperience":null,"heroVitals":null,"stage5Systems":{"hero":null}}));
        assert!(model.snapshot_read_model().is_none());
        model.observe_snapshot(&serde_json::json!({"heroMaxExperience":100,"heroVitals":null,"stage5Systems":{"hero":{"name":"Partial","experience":2}}}));
        assert!(model.snapshot_read_model().is_none());
    }
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

    fn complete_owner_hero() -> Value {
        serde_json::json!({"heroMaxExperience":200,"heroVitals":{"hp":20,"maxHp":30,"mp":10,"maxMp":15},
            "heroStats":[],"heroWeights":{"bag":1,"wear":2,"hand":3},"heroInventoryCapacity":10,
            "heroInventoryItems":[],"heroEquipmentItems":[],"stage5Systems":{"heroLearnedMagics":[],
                "hero":{"name":"Hero","class":"Warrior","gender":"Male","level":2,"experience":77,
                    "behaviour":0,"spawned":true,"autoPot":false,"autoHpPercent":30,"autoMpPercent":40,
                    "hpItemIndex":0,"mpItemIndex":0}}})
    }
    fn complete_owner_item(uid: u64, slot: u8) -> Value {
        serde_json::json!({"uniqueId":uid,"key":uid.to_string(),"name":"Hero potion","slot":slot,"container":"bag1",
            "quantity":2,"icon":0,"description":"Owned item","durabilityCurrent":null,"durabilityMax":null,
            "sellValue":7,"grade":"common","addedAttack":0,"addedDefence":0})
    }
    #[test]
    fn shared_hero_owner_preserves_wide_identity_items_and_sparse_slots() {
        let mut owner = complete_owner_hero();
        owner["stage5Systems"]["hero"]["experience"] = serde_json::json!(i64::MIN);
        owner["heroMaxExperience"] = serde_json::json!(i64::MAX);
        owner["heroInventoryItems"] = serde_json::json!([complete_owner_item(0, 3)]);
        owner["heroEquipmentItems"] = serde_json::json!([complete_owner_item(u64::MAX, 13)]);
        // The production boundary receives raw JSON; no JavaScript number round trip.
        let raw = serde_json::to_string(&owner).unwrap();
        let parsed: Value = serde_json::from_str(&raw).unwrap();
        let hero = project_owner_hero(&parsed).unwrap();
        let identity = hero.snapshot_identity.as_ref().unwrap();
        assert_eq!((identity.experience, identity.max_experience), (i64::MIN, i64::MAX));
        assert_eq!(hero.inventory_view.capacity, 10);
        assert_eq!(hero.inventory_view.items.len(), 2);
        let bag = &hero.inventory_view.items[0];
        let equipment = &hero.inventory_view.items[1];
        assert_eq!((bag.unique_id, bag.container, bag.slot), (Some(0), 0, 3));
        assert_eq!((equipment.unique_id, equipment.container, equipment.slot), (Some(u64::MAX), 2, 13));
        assert_eq!((bag.name.as_str(), bag.description.as_str(), bag.sell_value), ("Hero potion", "Owned item", 7));
        assert!(hero.info.is_none() && hero.base_stats.is_none() && hero.magic_clocks.is_empty());
        assert!(hero.auto_pot_view.items.is_empty());
    }
    #[test]
    fn shared_hero_owner_rejects_partial_identity_slots_keys_and_withdrawal_pair() {
        use HeroOwnerProjectionError::Incomplete;
        let owner = complete_owner_hero();
        let mut cases = Vec::new();
        let mut missing = owner.clone(); missing.as_object_mut().unwrap().remove("heroMaxExperience"); cases.push(missing);
        let mut missing = owner.clone(); missing.as_object_mut().unwrap().remove("heroStats"); cases.push(missing);
        let mut bad = owner.clone(); bad["stage5Systems"]["hero"]["level"] = 0.into(); cases.push(bad);
        let mut bad = owner.clone(); bad["heroMaxExperience"] = (-1).into(); cases.push(bad);
        let mut bad = owner.clone(); bad["stage5Systems"]["hero"] = Value::Null; cases.push(bad);
        let mut bad = owner.clone(); bad["heroInventoryItems"] = serde_json::json!([complete_owner_item(1,10)]); cases.push(bad);
        let mut bad = owner.clone(); let mut item = complete_owner_item(1,0); item["container"] = "bag2".into();
        bad["heroInventoryItems"] = serde_json::json!([item]); cases.push(bad);
        let mut bad = owner.clone(); bad["heroEquipmentItems"] = serde_json::json!([complete_owner_item(1,14)]); cases.push(bad);
        let mut bad = owner.clone(); bad["stage5Systems"]["heroLearnedMagics"] = serde_json::json!([{"spell":"FireBall","key":16}]); cases.push(bad);
        let mut bad = owner.clone(); bad["stage5Systems"]["heroLearnedMagics"] = serde_json::json!(vec![serde_json::json!({"spell":"FireBall","key":17});257]); cases.push(bad);
        for (index, bad) in cases.iter().enumerate() { assert_eq!(project_owner_hero(bad).unwrap_err(), Incomplete, "case {index}"); }
        let mut withdrawn = owner.clone();
        withdrawn["stage5Systems"]["hero"] = Value::Null; withdrawn["heroMaxExperience"] = Value::Null;
        let hero = project_owner_hero(&withdrawn).unwrap();
        assert!(hero.snapshot_identity.is_none() && hero.info.is_none());
        assert_eq!(hero.inventory_view.capacity, 10);
        let mut full = owner;
        full["stage5Systems"]["heroLearnedMagics"] = serde_json::json!(vec![serde_json::json!({"spell":"FireBall","key":24});256]);
        assert_eq!(project_owner_hero(&full).unwrap().learned_keys.unwrap().len(), 256);
    }
    #[test]
    fn shared_hero_owner_keeps_complete_tooltip_carriers_and_rejects_conflicting_custody() {
        use crate::inventory::{CrystalItemInfoModel, CrystalUserItemModel};
        let mut owner = complete_owner_hero();
        let info = CrystalItemInfoModel { name:"Base potion".into(), image:33, ..Default::default() };
        let real = CrystalItemInfoModel { name:"Viewer potion".into(), image:99, ..Default::default() };
        let socket = CrystalItemInfoModel { name:"Socket".into(), image:66, ..Default::default() };
        let user = CrystalUserItemModel { unique_id:u64::MAX, count:2, ..Default::default() };
        let mut item = complete_owner_item(u64::MAX,3);
        item["tooltipSource"] = serde_json::json!({"info":info,"realInfo":real,"userItem":user,
            "socketInfos":[null,socket.clone()],"realSocketInfos":[socket,null]});
        owner["heroInventoryItems"] = serde_json::json!([item.clone()]);
        let hero = project_owner_hero(&owner).unwrap();
        let projected = &hero.inventory_view.items[0];
        let tooltip = projected.tooltip_source.as_ref().unwrap();
        assert_eq!(projected.icon, 33);
        assert_eq!(tooltip.real_info.as_ref().unwrap().image, 99);
        assert_eq!(tooltip.user_item.as_ref().unwrap().unique_id, u64::MAX);
        assert_eq!(tooltip.socket_infos.len(), 2); assert!(tooltip.socket_infos[0].is_none());
        assert!(tooltip.real_socket_infos[1].is_none());
        for field in ["info", "realInfo"] {
            let mut bad = owner.clone(); bad["heroInventoryItems"][0]["tooltipSource"][field].as_object_mut().unwrap().remove("image");
            assert_eq!(project_owner_hero(&bad).unwrap_err(), HeroOwnerProjectionError::Incomplete, "{field}");
        }
        let mut bad = owner.clone(); bad["heroInventoryItems"][0]["tooltipSource"]["userItem"]["count"] = 3.into();
        assert_eq!(project_owner_hero(&bad).unwrap_err(), HeroOwnerProjectionError::Incomplete);
        let mut bad = owner; bad["heroInventoryItems"][0]["tooltipSource"]["userItem"]["unique_id"] = 0.into();
        assert_eq!(project_owner_hero(&bad).unwrap_err(), HeroOwnerProjectionError::Incomplete);
        let mut conflicting = projected.clone(); conflicting.description = "Replacement".into();
        assert_eq!(validate_owner_item_metadata(&item,&conflicting,false,false), Err(HeroOwnerProjectionError::Projection));
    }
}
