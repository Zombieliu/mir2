//! Indivisible native application of a complete owner economic checkpoint.
//! The small custody lock fences connection retirement; it never owns a World.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use bevy::prelude::{Resource, World};
use mir2_client_bevy::{hero_model::HeroModel, inventory::{InventoryModel, ItemModel},
    mail::MailModel, read_model::UiReadModel, shop::ShopModel, skill_model::SkillModel,
    social::SocialModel, storage::StorageModel};
use mir2_client_core::{npc_purchase_host::{ConnectionToken, PurchaseBinding},
    npc_purchase_receipt::{ActorKey, SnapshotWitness}};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::Value;

const MAX_BUNDLE_BYTES: usize = 64 * 1024 * 1024;
const MAX_APPLIED: usize = 32;
const MAX_HIGH_WATER_BYTES: usize = 128 * 1024 * 1024;

/// Every projection must be built from owner_json by the trusted receiver in
/// the same owner turn. Packet cursors and older model overlays are forbidden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeNpcEconomyProjection {
    pub owner_json: String,
    pub world_json: String,
    pub ui_json: String,
    pub inventory_json: String,
    pub mail_json: String,
    pub storage_json: String,
    pub shop_json: String,
    pub skill_json: String,
    pub hero_json: String,
    pub social_json: String,
}
impl NativeNpcEconomyProjection {
    fn retained_bytes(&self) -> usize {
        [&self.owner_json, &self.world_json, &self.ui_json, &self.inventory_json,
            &self.mail_json, &self.storage_json, &self.shop_json, &self.skill_json,
            &self.hero_json, &self.social_json].iter()
            .fold(std::mem::size_of::<Self>(), |n, json| n.saturating_add(json.capacity()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeNpcEconomyError { Retired, Correlation, TooLarge, Incomplete, Decode, Projection }

#[derive(Debug)]
struct Custody {
    active: bool,
    binding: PurchaseBinding,
    applied: VecDeque<NativeNpcEconomyApplied>,
}

/// Create a new sealed gate for every accepted Begin. Retire the original gate
/// before disconnect, actor/map change, reset or capability withdrawal.
#[derive(Debug, Clone)]
pub struct NativeNpcEconomyGate { custody: Arc<Mutex<Custody>> }
impl NativeNpcEconomyGate {
    pub fn new(binding: PurchaseBinding) -> Self {
        Self { custody: Arc::new(Mutex::new(Custody { active: true, binding,
            applied: VecDeque::new() })) }
    }
    pub fn retire(&self) {
        if let Ok(mut state) = self.custody.lock() { state.active = false; state.applied.clear(); }
    }
    pub fn is_current(&self, binding: PurchaseBinding) -> bool {
        self.custody.lock().is_ok_and(|state| state.active && state.binding == binding)
    }
    pub fn accepts(&self, binding: PurchaseBinding, witness: SnapshotWitness) -> bool {
        self.is_current(binding) && correlated(binding, witness)
    }
    pub fn prepare(&self, witness: SnapshotWitness, projection: NativeNpcEconomyProjection)
        -> Result<NativeNpcEconomyBundle, NativeNpcEconomyError>
    {
        let binding = {
            let state = self.custody.lock().map_err(|_| NativeNpcEconomyError::Retired)?;
            if !state.active { return Err(NativeNpcEconomyError::Retired); }
            state.binding
        };
        if !correlated(binding, witness) { return Err(NativeNpcEconomyError::Correlation); }
        if projection.retained_bytes() > MAX_BUNDLE_BYTES { return Err(NativeNpcEconomyError::TooLarge); }
        // Predecode before admission. Decode again on application so the queue
        // retains only bounded immutable JSON, rather than two heap copies.
        let _ = Decoded::decode(&projection)?;
        if !self.is_current(binding) { return Err(NativeNpcEconomyError::Retired); }
        Ok(NativeNpcEconomyBundle { gate: self.clone(), binding, witness,
            projection: Arc::new(projection) })
    }
    /// Only the single-writer connection owner consumes this bounded completion.
    pub fn try_recv_applied(&self) -> Option<NativeNpcEconomyApplied> {
        let mut state = self.custody.lock().ok()?;
        if !state.active { return None; }
        state.applied.pop_front()
    }
    pub(crate) fn same_gate(&self, other: &Self) -> bool { Arc::ptr_eq(&self.custody, &other.custody) }
}

fn correlated(binding: PurchaseBinding, witness: SnapshotWitness) -> bool {
    witness.complete && witness.server_revision != u64::MAX
        && witness.actor == binding.actor() && witness.producer_scope == binding.producer_scope()
}

#[derive(Debug, Clone)]
pub struct NativeNpcEconomyBundle {
    gate: NativeNpcEconomyGate,
    binding: PurchaseBinding,
    witness: SnapshotWitness,
    projection: Arc<NativeNpcEconomyProjection>,
}
impl NativeNpcEconomyBundle {
    pub(crate) fn gate(&self) -> &NativeNpcEconomyGate { &self.gate }
    pub(crate) fn retained_bytes(&self) -> usize { self.projection.retained_bytes() }
    pub(crate) fn retire(&self) { self.gate.retire(); }
}

/// Cannot be constructed outside this module. Enqueue or readiness is never
/// an applied witness. Preserve this exact tuple when calling the Core host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeNpcEconomyApplied { binding: PurchaseBinding, witness: SnapshotWitness }
impl NativeNpcEconomyApplied {
    pub fn connection(self) -> ConnectionToken { self.binding.connection() }
    pub fn binding(self) -> PurchaseBinding { self.binding }
    pub fn witness(self) -> SnapshotWitness { self.witness }
}

/// Lossless client-view source covers every Stage5 domain, including economic
/// domains without a dedicated Bevy model. Access is immutable.
#[derive(Resource, Debug, Clone)]
pub struct NativeNpcEconomySource {
    binding: PurchaseBinding,
    witness: SnapshotWitness,
    projection: Arc<NativeNpcEconomyProjection>,
    owner: Arc<Value>,
    gate: NativeNpcEconomyGate,
    reset_revision: u64,
}
impl NativeNpcEconomySource {
    pub fn owner(&self) -> &Value { &self.owner }
    pub fn owner_json(&self) -> &str { &self.projection.owner_json }
    pub fn witness(&self) -> SnapshotWitness { self.witness }
    pub fn binding(&self) -> PurchaseBinding { self.binding }
}

// Reset clears exposed owner data but must not erase an actor's revision
// history. Like Core's retained ledger, capacity refusal never evicts history.
#[derive(Resource, Default)]
struct NativeNpcEconomyHighWater {
    actors: BTreeMap<ActorKey, (u64, String)>,
}
impl NativeNpcEconomyHighWater {
    fn accepts(&self, witness: SnapshotWitness, fingerprint: &String) -> bool {
        if let Some((revision, previous)) = self.actors.get(&witness.actor) {
            if *revision > witness.server_revision
                || (*revision == witness.server_revision && previous != fingerprint) { return false; }
        } else if self.actors.len() >= mir2_client_core::npc_purchase_host::MAX_RETAINED_ACTORS { return false; }
        self.actors.iter().filter(|(actor, _)| **actor != witness.actor)
            .fold(fingerprint.capacity(), |bytes, (_, (_, previous))| bytes.saturating_add(previous.capacity())) <= MAX_HIGH_WATER_BYTES
    }
}

/// Public counterpart of simulation's owner_economic_projection. Its revision
/// is CharacterSaveRecord.revision, not the world tick or a model epoch.
/// WorldSnapshotClientView exposes character name/class/gender/level through
/// SelfPlayer, but not CharacterRecord.index or private buyBack/used/rental
/// checkpoints. Copy only fields present in that actual client-view source.
/// The full original owner remains separately retained for every application.
fn owner_economic_fingerprint(owner: &Value) -> Result<String, NativeNpcEconomyError> {
    let mut fields = serde_json::Map::new();
    for key in ["playerExperience", "playerMaxExperience", "gold", "credit", "cityCurrencies",
        "inventoryCapacity", "inventoryItems", "beltItems", "equipmentItems", "storageItems",
        "heroInventoryItems", "heroEquipmentItems", "heroInventoryCapacity"] {
        if let Some(value) = owner.get(key) { fields.insert(key.into(), value.clone()); }
    }
    let object = field_unsigned(owner, "playerObjectId")?;
    let character = owner.get("entities").and_then(Value::as_array).and_then(|entities|
        entities.iter().find(|entity| entity.get("kind").and_then(Value::as_str) == Some("selfPlayer")
            && entity.get("objectId").and_then(unsigned) == Some(object)))
        .ok_or(NativeNpcEconomyError::Incomplete)?;
    let mut public_character = serde_json::Map::new();
    for key in ["name", "class", "gender", "level"] {
        if let Some(value) = character.get(key) { public_character.insert(key.into(), value.clone()); }
    }
    fields.insert("character".into(), Value::Object(public_character));
    let mut stage5 = owner.get("stage5Systems").and_then(Value::as_object)
        .cloned().ok_or(NativeNpcEconomyError::Incomplete)?;
    // The authoritative projection excludes refine deadlines because they
    // cross durable and runtime clock domains. Keep that same exception.
    stage5.remove("refine");
    fields.insert("stage5Systems".into(), Value::Object(stage5));
    serde_json::to_string(&canonical_economic_value(Value::Object(fields))).map_err(|_| NativeNpcEconomyError::Decode)
}

fn canonical_economic_value(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            // Preserve a stable representation even if another crate enables
            // serde_json's preserve_order feature in the native dependency graph.
            let mut entries: Vec<_> = object.into_iter().collect();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            Value::Object(entries.into_iter().map(|(key, value)| (key, canonical_economic_value(value))).collect())
        }
        Value::Array(array) => Value::Array(array.into_iter().map(canonical_economic_value).collect()),
        primitive => primitive,
    }
}

struct Decoded {
    owner: Value,
    world: crate::WorldSnapshot,
    ui: UiReadModel,
    inventory: InventoryModel,
    mail: MailModel,
    storage: StorageModel,
    shop: ShopModel,
    skill: SkillModel,
    hero: HeroModel,
    social: SocialModel,
}

// Duplicate object names, including escaped duplicates, must not disappear
// into Value's last-write-wins map before source completeness is inspected.
struct UniqueValue(Value);
impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = UniqueValue;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { f.write_str("unique JSON") }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Self::Value,E> { Ok(UniqueValue(Value::Bool(v))) }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value,E> { Ok(UniqueValue(v.into())) }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value,E> { Ok(UniqueValue(v.into())) }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value,E> { serde_json::Number::from_f64(v).map(|v| UniqueValue(Value::Number(v))).ok_or_else(|| E::custom("nonfinite")) }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value,E> { Ok(UniqueValue(Value::String(v.into()))) }
            fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value,E> { Ok(UniqueValue(Value::String(v))) }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value,E> { Ok(UniqueValue(Value::Null)) }
            fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value,E> { self.visit_unit() }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value,A::Error> {
                let mut out = Vec::new();
                while let Some(UniqueValue(value)) = seq.next_element()? { out.push(value); }
                Ok(UniqueValue(Value::Array(out)))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value,A::Error> {
                let mut out = serde_json::Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if out.contains_key(&key) { return Err(serde::de::Error::custom("duplicate field")); }
                    let UniqueValue(value) = map.next_value()?; out.insert(key, value);
                }
                Ok(UniqueValue(Value::Object(out)))
            }
        }
        d.deserialize_any(Visitor)
    }
}
fn value(json: &str) -> Result<Value, NativeNpcEconomyError> {
    serde_json::from_str::<UniqueValue>(json).map(|v| v.0).map_err(|_| NativeNpcEconomyError::Decode)
}
fn model<T: DeserializeOwned>(json: &str, arrays: &[&str]) -> Result<T, NativeNpcEconomyError> {
    let v = value(json)?;
    if !v.is_object() || arrays.iter().any(|key| !v.get(*key).is_some_and(Value::is_array)) {
        return Err(NativeNpcEconomyError::Incomplete);
    }
    serde_json::from_value(v).map_err(|_| NativeNpcEconomyError::Decode)
}
fn unsigned(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_str().filter(|s| *s == "0" || (!s.starts_with('0') && s.bytes().all(|b| b.is_ascii_digit())))?.parse().ok())
}
fn field_unsigned(v: &Value, key: &str) -> Result<u64, NativeNpcEconomyError> {
    v.get(key).and_then(unsigned).ok_or(NativeNpcEconomyError::Incomplete)
}
impl Decoded {
    fn decode(p: &NativeNpcEconomyProjection) -> Result<Self, NativeNpcEconomyError> {
        let owner = value(&p.owner_json)?;
        for key in ["inventoryItems", "beltItems", "equipmentItems", "storageItems", "knownSkills", "entities",
            "heroInventoryItems", "heroEquipmentItems", "heroStats", "playerCrystalStats"] {
            if !owner.get(key).is_some_and(Value::is_array) { return Err(NativeNpcEconomyError::Incomplete); }
        }
        let currencies = owner.get("cityCurrencies").and_then(Value::as_object).ok_or(NativeNpcEconomyError::Incomplete)?;
        if currencies.values().any(|v| v.as_u64().is_none_or(|n| n > u64::from(u32::MAX))) { return Err(NativeNpcEconomyError::Incomplete); }
        for key in ["currentWeight", "maxWeight", "freeBagSlots", "maxBagSlots"] {
            if owner.get(key).and_then(Value::as_u64).is_none_or(|n| n > u64::from(u16::MAX)) { return Err(NativeNpcEconomyError::Incomplete); }
        }
        if owner.get("heroInventoryCapacity").and_then(Value::as_u64).is_none_or(|n| n > u64::from(u8::MAX))
            || owner.get("playerPkPoints").and_then(Value::as_i64).is_none_or(|n| i32::try_from(n).is_err())
            || owner.get("storagePasswordLastSetBinaryDatetime").and_then(Value::as_i64).is_none() {
            return Err(NativeNpcEconomyError::Incomplete);
        }
        for key in ["heroWeights", "playerWeights"] {
            let weights = owner.get(key).ok_or(NativeNpcEconomyError::Incomplete)?;
            if key == "playerWeights" && weights.is_null() { continue; }
            if !weights.is_object() || ["bag", "wear", "hand"].iter().any(|field|
                weights.get(*field).and_then(Value::as_u64).is_none_or(|n| n > u64::from(u32::MAX))) {
                return Err(NativeNpcEconomyError::Incomplete);
            }
        }
        let vitals = owner.get("heroVitals").ok_or(NativeNpcEconomyError::Incomplete)?;
        if !vitals.is_null() && (!vitals.is_object() || ["hp", "maxHp", "mp", "maxMp"].iter().any(|field|
            vitals.get(*field).and_then(Value::as_i64).is_none_or(|n| i32::try_from(n).is_err()))) {
            return Err(NativeNpcEconomyError::Incomplete);
        }
        let systems = owner.get("stage5Systems").filter(|v| v.is_object()).ok_or(NativeNpcEconomyError::Incomplete)?;
        for key in ["group", "guild", "social", "relationship", "mentor", "gameShopIndividualPurchases", "refine", "conquest", "guildTerritory", "profession", "appearance", "itemRental"] {
            if !systems.get(key).is_some_and(Value::is_object) { return Err(NativeNpcEconomyError::Incomplete); }
        }
        for key in ["mail", "economyProjectionEventIds", "auction", "heroLearnedMagics", "nameLists", "intelligentCreatures"] {
            if !systems.get(key).is_some_and(Value::is_array) { return Err(NativeNpcEconomyError::Incomplete); }
        }
        for key in ["trade", "hero", "summonedIntelligentCreatureType", "attackMode", "petMode", "pkDecayElapsedTicks"] {
            if systems.get(key).is_none() { return Err(NativeNpcEconomyError::Incomplete); }
        }
        if systems.get("intelligentCreaturePearls").and_then(Value::as_i64).is_none() { return Err(NativeNpcEconomyError::Incomplete); }
        for key in ["hasExpandedStorage", "hasStoragePassword", "requireStoragePassword"] {
            if owner.get(key).and_then(Value::as_bool).is_none() { return Err(NativeNpcEconomyError::Incomplete); }
        }
        if owner.get("expandedStorageExpiryTimeBinaryDatetime").and_then(Value::as_i64).is_none() { return Err(NativeNpcEconomyError::Incomplete); }
        let object = field_unsigned(&owner, "playerObjectId")?;
        let self_player = owner["entities"].as_array().unwrap().iter().find(|e|
            e.get("kind").and_then(Value::as_str) == Some("selfPlayer")
                && e.get("objectId").and_then(unsigned) == Some(object)).ok_or(NativeNpcEconomyError::Incomplete)?;
        if self_player.get("name").and_then(Value::as_str).is_none()
            || !matches!(self_player.get("class").and_then(Value::as_str), Some("Warrior" | "Wizard" | "Taoist" | "Assassin" | "Archer"))
            || !matches!(self_player.get("gender").and_then(Value::as_str), Some("Male" | "Female"))
            || self_player.get("level").and_then(Value::as_u64).is_none_or(|level| level == 0 || level > u64::from(u16::MAX)) {
            return Err(NativeNpcEconomyError::Incomplete);
        }
        let world: crate::WorldSnapshot = model(&p.world_json, &["entities"])?;
        let ui: UiReadModel = model(&p.ui_json, &[])?;
        let inventory: InventoryModel = model(&p.inventory_json, &["items"])?;
        let mail: MailModel = model(&p.mail_json, &["mails"])?;
        let storage: StorageModel = model(&p.storage_json, &["items"])?;
        let shop = model(&p.shop_json, &["goods"])?;
        let skill: SkillModel = model(&p.skill_json, &["skills"])?;
        let hero = model(&p.hero_json, &[])?;
        let social_json = value(&p.social_json)?;
        if ["group", "guild", "trade"].iter().any(|k| !social_json.get(*k).is_some_and(Value::is_object)) {
            return Err(NativeNpcEconomyError::Incomplete);
        }
        let social = serde_json::from_value(social_json).map_err(|_| NativeNpcEconomyError::Decode)?;
        for (key, projected) in [("playerHp", ui.player.hp), ("playerMaxHp", ui.player.max_hp),
            ("playerMp", ui.player.mp), ("playerMaxMp", ui.player.max_mp)] {
            let actual = owner.get(key).and_then(Value::as_i64).ok_or(NativeNpcEconomyError::Incomplete)?;
            if actual != i64::from(projected) { return Err(NativeNpcEconomyError::Projection); }
        }
        let entities = owner["entities"].as_array().unwrap();
        if world.entities.len() != entities.len() { return Err(NativeNpcEconomyError::Projection); }
        for (source, projected) in entities.iter().zip(&world.entities) {
            if source.get("objectId").and_then(unsigned) != projected.object_id.parse::<u64>().ok()
                || source.get("x").and_then(Value::as_i64) != Some(i64::from(projected.x))
                || source.get("y").and_then(Value::as_i64) != Some(i64::from(projected.y))
                || source.get("level").and_then(unsigned) != projected.level.map(u64::from) {
                return Err(NativeNpcEconomyError::Projection);
            }
        }
        let gold = field_unsigned(&owner, "gold")?;
        let capacity = field_unsigned(&owner, "inventoryCapacity")?;
        let xp = owner.get("playerExperience").and_then(Value::as_i64).ok_or(NativeNpcEconomyError::Incomplete)?;
        let max_xp = owner.get("playerMaxExperience").and_then(Value::as_i64).ok_or(NativeNpcEconomyError::Incomplete)?;
        if u64::from(ui.player.gold) != gold || u64::from(inventory.gold) != gold
            || u64::from(ui.player.credit) != field_unsigned(&owner, "credit")?
            || ui.player.experience != xp || ui.player.max_experience != max_xp
            || u64::from(ui.player.level) != field_unsigned(self_player, "level")?
            || ui.player.name.as_deref() != self_player["name"].as_str()
            || !ui.player.class_name.as_deref().is_some_and(|name| self_player["class"].as_str().is_some_and(|raw| name.eq_ignore_ascii_case(raw)))
            || !ui.player.gender.as_deref().is_some_and(|name| self_player["gender"].as_str().is_some_and(|raw| name.eq_ignore_ascii_case(raw)))
            || u64::from(inventory.capacity) != capacity
            || InventoryModel::canonical_capacity(inventory.capacity) != inventory.capacity
            || u64::from(storage.size) != field_unsigned(&owner, "storageSize")?
            || Some(storage.has_expanded) != owner["hasExpandedStorage"].as_bool()
            || Some(storage.has_password) != owner["hasStoragePassword"].as_bool()
            || Some(!storage.unlocked) != owner["requireStoragePassword"].as_bool()
            || Some(storage.expiry) != owner["expandedStorageExpiryTimeBinaryDatetime"].as_i64()
            || world.player_object_id.as_deref().and_then(|s| s.parse::<u64>().ok()) != Some(object)
        { return Err(NativeNpcEconomyError::Projection); }
        check_items(&owner, &inventory.items, &["inventoryItems", "beltItems", "equipmentItems"])?;
        check_items(&owner, &storage.items, &["storageItems"])?;
        let visible_mail: Vec<&Value> = systems["mail"].as_array().unwrap().iter()
            .filter(|m| m.get("deleted").and_then(Value::as_bool) != Some(true)).collect();
        if visible_mail.len() != mail.mails.len() { return Err(NativeNpcEconomyError::Projection); }
        for (source, projected) in visible_mail.iter().zip(&mail.mails) {
            let id = source.get("mailId").or_else(|| source.get("id")).and_then(unsigned);
            if id != Some(projected.id) || projected.operation.is_some()
                || source.get("items").and_then(Value::as_array).map(Vec::len) != Some(projected.items.len())
                || source.get("gold").and_then(unsigned) != Some(u64::from(projected.gold)) {
                return Err(NativeNpcEconomyError::Projection);
            }
        }
        if skill.skills.len() != owner["knownSkills"].as_array().unwrap().len() { return Err(NativeNpcEconomyError::Projection); }
        Ok(Self { owner, world, ui, inventory, mail, storage, shop, skill, hero, social })
    }
}

fn check_items(owner: &Value, projected: &[ItemModel], fields: &[&str]) -> Result<(), NativeNpcEconomyError> {
    let source: Vec<(&str, &Value)> = fields.iter().flat_map(|key|
        owner[*key].as_array().unwrap().iter().map(move |value| (*key, value)))
        .collect();
    if source.len() != projected.len() { return Err(NativeNpcEconomyError::Projection); }
    let mut remaining: Vec<&ItemModel> = projected.iter().collect();
    for (field, item) in source {
        if !item.is_object() { return Err(NativeNpcEconomyError::Incomplete); }
        let uid = item.get("uniqueId").and_then(unsigned);
        if field != "equipmentItems" && uid.is_none() { return Err(NativeNpcEconomyError::Incomplete); }
        let count = field_unsigned(item, "quantity")?;
        let raw_slot = if field == "equipmentItems" { equipment_slot(item.get("slot"))? }
            else { field_unsigned(item, "slot")? };
        let (container, slot) = match field {
            "beltItems" if item["container"].as_str() == Some("belt") => (1, raw_slot),
            "equipmentItems" => (2, raw_slot),
            "storageItems" if item["container"].as_str() == Some("storage") => (4, raw_slot),
            "beltItems" | "storageItems" => return Err(NativeNpcEconomyError::Incomplete),
            _ => match item.get("container").and_then(Value::as_str) {
                Some("bag2") => (0, raw_slot.checked_add(40).ok_or(NativeNpcEconomyError::Projection)?),
                Some("quest") => (3, raw_slot),
                Some("bag1") => (0, raw_slot),
                _ => return Err(NativeNpcEconomyError::Incomplete),
            },
        };
        let Some(index) = remaining.iter().position(|model| model.unique_id == uid
            && u64::from(model.quantity) == count && model.container == container && u64::from(model.slot) == slot)
        else { return Err(NativeNpcEconomyError::Projection); };
        check_item_metadata(item, remaining.remove(index), field == "equipmentItems", field == "storageItems")?;
    }
    Ok(())
}

fn check_item_metadata(source: &Value, projected: &ItemModel, equipment: bool, storage: bool) -> Result<(), NativeNpcEconomyError> {
    for field in ["key", "name", "description", "grade"] {
        if source.get(field).and_then(Value::as_str).is_none() { return Err(NativeNpcEconomyError::Incomplete); }
    }
    if source.get("icon").and_then(Value::as_u64).is_none_or(|n| n > u64::from(u16::MAX)) {
        return Err(NativeNpcEconomyError::Incomplete);
    }
    let expected_key = if storage { field_unsigned(source, "uniqueId")?.to_string() }
        else { source["key"].as_str().unwrap().to_owned() };
    if projected.key != expected_key || Some(projected.name.as_str()) != source["name"].as_str()
        || Some(projected.description.as_str()) != source["description"].as_str()
        || projected.grade.as_deref() != source["grade"].as_str() {
        return Err(NativeNpcEconomyError::Projection);
    }
    // The gateway derives count-dependent icons from tooltip Info when it is
    // present. Raw icon equality only applies to the legacy direct mapping.
    if source.get("tooltipSource").is_none() && Some(u64::from(projected.icon)) != source["icon"].as_u64() {
        return Err(NativeNpcEconomyError::Projection);
    }
    let model = serde_json::to_value(projected).map_err(|_| NativeNpcEconomyError::Decode)?;
    for field in ["durabilityCurrent", "durabilityMax"] {
        let raw = source.get(field).ok_or(NativeNpcEconomyError::Incomplete)?;
        if !(raw.is_null() && !equipment) && raw.as_u64().is_none_or(|n| n > u64::from(u16::MAX)) {
            return Err(NativeNpcEconomyError::Incomplete);
        }
        if model[field] != *raw { return Err(NativeNpcEconomyError::Projection); }
    }
    for field in ["addedAttack", "addedDefence"] {
        let raw = source.get(field).ok_or(NativeNpcEconomyError::Incomplete)?;
        if raw.as_i64().is_none_or(|n| i32::try_from(n).is_err()) { return Err(NativeNpcEconomyError::Incomplete); }
        if model[field] != *raw { return Err(NativeNpcEconomyError::Projection); }
    }
    let extra: &[&str] = if equipment { &["stateImage", "shape", "attack", "defence", "addedLuck", "socketSlots"] }
        else { &["sellValue"] };
    for field in extra {
        let raw = source.get(*field).ok_or(NativeNpcEconomyError::Incomplete)?;
        if model[*field] != *raw { return Err(NativeNpcEconomyError::Projection); }
    }
    if source.get("equipSlot").is_some_and(|raw| model["equipSlot"] != *raw) { return Err(NativeNpcEconomyError::Projection); }
    Ok(())
}

// EquipmentSlot serializes names (including sparse/reordered arrays). These
// are Crystal's stable slots, matching Gateway normalized_slot rather than
// the enum declaration order or the item's array position.
fn equipment_slot(value: Option<&Value>) -> Result<u64, NativeNpcEconomyError> {
    if let Some(slot) = value.and_then(unsigned) {
        return (slot < 14).then_some(slot).ok_or(NativeNpcEconomyError::Incomplete);
    }
    let name = value.and_then(Value::as_str).ok_or(NativeNpcEconomyError::Incomplete)?;
    match name.trim().to_ascii_lowercase().replace('_', "-").as_str() {
        "weapon" => Ok(0), "armour" | "armor" => Ok(1), "helmet" => Ok(2), "torch" => Ok(3), "necklace" => Ok(4),
        "bracelet-left" | "braceletleft" | "braceletl" => Ok(5), "bracelet-right" | "braceletright" | "braceletr" => Ok(6),
        "ring-left" | "ringleft" | "ringl" => Ok(7), "ring-right" | "ringright" | "ringr" => Ok(8),
        "amulet" => Ok(9), "belt" => Ok(10), "boots" => Ok(11), "stone" => Ok(12), "mount" => Ok(13),
        _ => Err(NativeNpcEconomyError::Incomplete),
    }
}

/// Exclusive system: all ordinary consumers finish the prefix before this
/// barrier can commit. No later ordinary model drains until the next frame.
pub(crate) fn apply_pending_native_npc_economy(world: &mut World) {
    let revision = world.get_resource::<mir2_client_bevy::pending_operations::SessionResetRevision>().map_or(0, |r| r.0);
    if world.get_resource::<NativeNpcEconomySource>().is_some_and(|s| s.reset_revision != revision) {
        clear_native_npc_economy_source(world);
    }
    let bundle = world.get_resource::<crate::native_ingest::NativeInbound>()
        .and_then(|inbound| inbound.take_npc_economy_front());
    if let Some(bundle) = bundle { let _ = apply_bundle(world, bundle); }
}

fn apply_bundle(world: &mut World, bundle: NativeNpcEconomyBundle) -> bool {
    let Ok(decoded) = Decoded::decode(&bundle.projection) else { return false; };
    let Ok(fingerprint) = owner_economic_fingerprint(&decoded.owner) else { return false; };
    let Ok(mut custody) = bundle.gate.custody.lock() else { return false; };
    if !custody.active || custody.binding != bundle.binding || !correlated(bundle.binding, bundle.witness)
        || custody.applied.len() >= MAX_APPLIED { return false; }
    if world.get_resource::<NativeNpcEconomyHighWater>().is_some_and(|history| !history.accepts(bundle.witness, &fingerprint)) { return false; }
    use mir2_client_bevy::pending_operations::{PendingOperations, AuthoritativeModelRevisions,
        AuthoritativeModelDomain, mark_authoritative_refresh, reconcile_inventory_refresh,
        reconcile_mail_refresh, reconcile_storage_refresh, reconcile_shop_refresh};
    let old_inventory = world.get_resource::<InventoryModel>().cloned().unwrap_or_default();
    let old_mail = world.get_resource::<MailModel>().cloned().unwrap_or_default();
    let old_storage = world.get_resource::<StorageModel>().cloned().unwrap_or_default();
    let old_shop = world.get_resource::<ShopModel>().cloned().unwrap_or_default();
    if let Some(mut pending) = world.get_resource_mut::<PendingOperations>() {
        reconcile_inventory_refresh(&mut pending, &old_inventory, &decoded.inventory);
        reconcile_mail_refresh(&mut pending, &old_mail, &decoded.mail);
        reconcile_storage_refresh(&mut pending, &decoded.inventory, &old_storage, &decoded.storage);
        reconcile_shop_refresh(&mut pending, &old_shop, &decoded.shop);
    }
    if let Some(mut revisions) = world.get_resource_mut::<AuthoritativeModelRevisions>() {
        for domain in [AuthoritativeModelDomain::Inventory, AuthoritativeModelDomain::Mail,
            AuthoritativeModelDomain::Storage, AuthoritativeModelDomain::Shop] {
            mark_authoritative_refresh(&mut revisions, domain);
        }
    }
    // The gate stays locked from the final generation check through the pure
    // resource commit and completion publication. Retirement waits for this turn.
    world.insert_resource(crate::RuntimeWorldState { snapshot: Some(decoded.world) });
    world.insert_resource(decoded.ui);
    world.insert_resource(decoded.inventory);
    world.insert_resource(decoded.mail);
    world.insert_resource(decoded.storage);
    world.insert_resource(decoded.shop);
    world.insert_resource(decoded.skill);
    world.insert_resource(decoded.hero);
    world.insert_resource(decoded.social);
    let reset_revision = world.get_resource::<mir2_client_bevy::pending_operations::SessionResetRevision>().map_or(0, |r| r.0);
    world.insert_resource(NativeNpcEconomySource { binding: bundle.binding, witness: bundle.witness,
        projection: Arc::clone(&bundle.projection), owner: Arc::new(decoded.owner), gate: bundle.gate.clone(), reset_revision });
    if !world.contains_resource::<NativeNpcEconomyHighWater>() { world.insert_resource(NativeNpcEconomyHighWater::default()); }
    world.resource_mut::<NativeNpcEconomyHighWater>().actors.insert(bundle.witness.actor,
        (bundle.witness.server_revision, fingerprint));
    custody.applied.push_back(NativeNpcEconomyApplied { binding: bundle.binding, witness: bundle.witness });
    true
}

pub(crate) fn clear_native_npc_economy_source(world: &mut World) {
    if let Some(source) = world.remove_resource::<NativeNpcEconomySource>() { source.gate.retire(); }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use mir2_client_core::{npc_purchase_host::NpcPurchaseReceiptHost, npc_purchase_receipt::{ActorKey, OwnerScope}};
    use serde_json::json;

    pub(crate) fn fixture() -> (NativeNpcEconomyGate, SnapshotWitness, NativeNpcEconomyProjection) {
        let mut host = NpcPurchaseReceiptHost::new().unwrap();
        let connection = host.open_connection().unwrap();
        let ticket = host.request_begin(connection).unwrap();
        let binding = host.bind(ticket, ActorKey::from_server_bytes([1;32]).unwrap(),
            OwnerScope::from_server_bytes([2;32]).unwrap()).unwrap();
        let gate = NativeNpcEconomyGate::new(binding);
        let witness = SnapshotWitness { actor: binding.actor(), producer_scope: binding.producer_scope(), server_revision: 7, complete: true };
        let mut owner = json!({
            "tick":7,"mapTitle":"Bichon","mapFileName":"0","inSafeZone":false,"lightSetting":0,
            "playerObjectId": 3, "entities": [{"objectId":3,"kind":"selfPlayer","name":"Alice","ownerName":null,"ai":null,
                "level":8,"class":"Warrior","gender":"Male","x":4,"y":5,"direction":"Down","hp":0,"maxHp":0,
                "light":0,"nameColourArgb":0,"dead":false,"disposition":"friendly","sprite":null,"questIds":[]}],
            "playerHp":0,"playerMaxHp":0,"playerMp":0,"playerMaxMp":0,
            "gold":90,"credit":20,"playerExperience":50,"playerMaxExperience":100
        });
        owner.as_object_mut().unwrap().extend(json!({
            "cityCurrencies":{"bichon":0,"feitian":0},"playerCrystalStats":[],"playerPkPoints":0,
            "currentWeight":0,"playerWeights":null,"maxWeight":0,"freeBagSlots":40,"maxBagSlots":40,
            "npcGoldTradeCapacity":null,"storagePasswordLastSetBinaryDatetime":0,
            "heroInventoryItems":[],"heroEquipmentItems":[],"heroInventoryCapacity":46,
            "heroStats":[],"heroVitals":null,"heroWeights":{"bag":0,"wear":0,"hand":0}
        }).as_object().unwrap().clone());
        owner.as_object_mut().unwrap().extend(json!({
            "sceneView":null,"terrainPatches":[],"decorObjects":[],"groundDrops":[],"questLog":[],
            "activeNpcDialog":null,"npcScriptDiagnostics":[],"activeBuffs":[],"mapTransfers":[],"interactionHints":[],
            "inventoryCapacity":46,"inventoryItems":[],"beltItems":[],"equipmentItems":[],"storageItems":[],"knownSkills":[],
            "storageSize":80,"hasExpandedStorage":false,"hasStoragePassword":false,"requireStoragePassword":false,
            "expandedStorageExpiryTimeBinaryDatetime":0
        }).as_object().unwrap().clone());
        owner["stage5Systems"]=json!({"group":{"allowGroup":true,"members":[],"lootMode":"free"},
                "guild":{"name":"","members":[],"rank":"","permissions":[],"chatLog":[],"knownGuilds":[],"activeWars":[],
                    "activeWarTicksRemaining":{},"alliedGuilds":[],"allyCount":0,"allianceBroadcasts":[],"warBroadcasts":[],"notice":[],
                    "storageGold":0,"storageItems":{},"storageItemStates":{},"storageItemUsers":{}},
                "social":{"friends":[],"blocked":[],"memos":{}},
                "relationship":{"allowLoverRecall":false,"cooldownUntilMs":0,"allowMarriage":false,"partnerName":"Partner",
                    "marriedDateBinaryDatetime":0,"mapName":"","marriedDays":10,"pendingRequestFrom":null,"pendingDivorceFrom":null},
                "mentor":{"isMentor":false,"cooldownUntilMs":0,"allowMentor":false,"name":"Mentor","level":20,"online":true,
                    "menteeExp":5,"pendingRequestFrom":null,"pendingRequestLevel":0},
                "gameShopIndividualPurchases":{},
                "refine":{"ovenItemStateJson":null,"remainingMs":0,"clockEpoch":null,"collectDeadlineMs":null,"itemStates":{},"slots":{},
                    "currentItem":null,"refining":false,"ready":false,"pendingUniqueId":0,"pendingChance":0,"pendingStat":0},
                "conquest":{"castleOwner":"","activeWars":[],"eventLog":[],"taxRatePercent":0,"gold":0,"guards":[],"walls":[],"gates":[],"openGates":[]},
                "guildTerritory":{"owned":false,"mapFileName":"GA0","owner":"","leader":"","leader2":"","price":0,"rentalDaysLeft":0,"begin":0,"recallLog":[]},
                "profession":{"miningLevel":0,"ore":0,"craftedItems":[]},"appearance":{"hair":0},
                "itemRental":{"partnerName":null,"fee":0,"days":0,"hasDepositedItem":false,"depositedItemName":null,
                    "goldLocked":false,"itemLocked":false,"recordCount":0,"rentedItems":[]},
                "mail":[],"economyProjectionEventIds":["merged-1"],"auction":[],"heroLearnedMagics":[],"nameLists":[],"intelligentCreatures":[],
                "trade":null,"hero":null,"summonedIntelligentCreatureType":99,"attackMode":0,"petMode":0,"pkDecayElapsedTicks":0,
                "intelligentCreaturePearls":12
        });
        let ui = UiReadModel { player: mir2_client_bevy::read_model::PlayerStats { gold:90, credit:20,
            name:Some("Alice".into()),class_name:Some("Warrior".into()),gender:Some("Male".into()),
            crystal_stats:Some(vec![]),current_weight_known:true,
            level:8, experience:50, max_experience:100, ..Default::default() } };
        (gate, witness, NativeNpcEconomyProjection {
            owner_json:owner.to_string(),world_json:json!({"playerObjectId":"3","entities":[
                {"objectId":"3","kind":"selfPlayer","name":"Alice","level":8,"x":4,"y":5,"direction":"Down"}]}).to_string(),
            ui_json:serde_json::to_string(&ui).unwrap(),
            inventory_json:serde_json::to_string(&InventoryModel { gold:90,..Default::default() }).unwrap(),
            mail_json:serde_json::to_string(&MailModel::default()).unwrap(),
            storage_json:serde_json::to_string(&StorageModel { unlocked:true,..Default::default() }).unwrap(),
            shop_json:serde_json::to_string(&ShopModel::default()).unwrap(),
            skill_json:serde_json::to_string(&SkillModel::default()).unwrap(),
            hero_json:serde_json::to_string(&HeroModel::default()).unwrap(),
            social_json:serde_json::to_string(&SocialModel::default()).unwrap(),
        })
    }
    fn prepare(gate: &NativeNpcEconomyGate, witness: SnapshotWitness, p: NativeNpcEconomyProjection) -> NativeNpcEconomyBundle {
        gate.prepare(witness,p).unwrap()
    }
    fn source_item(uid: u64, slot: u8, container: &str, quantity: u32, name: &str) -> Value {
        json!({"uniqueId":uid,"key":uid.to_string(),"name":name,"slot":slot,"container":container,"quantity":quantity,
            "icon":0,"description":"","durabilityCurrent":null,"durabilityMax":null,"sellValue":0,"grade":"common",
            "addedAttack":0,"addedDefence":0})
    }
    fn source_equipment(uid: u64, slot: &str, name: &str) -> Value {
        json!({"uniqueId":uid,"key":uid.to_string(),"name":name,"slot":slot,"quantity":1,"icon":0,"stateImage":0,"shape":null,
            "description":"","durabilityCurrent":0,"durabilityMax":0,"grade":"common","attack":0,"defence":0,
            "addedAttack":0,"addedDefence":0,"addedLuck":0,"socketSlots":0,"sealedExpiryTimeBinaryDatetime":0,"sealedNextTimeBinaryDatetime":0})
    }

    #[test]
    fn native_npc_economy_complete_apply_publishes_original_tuple_and_lossless_domains() {
        let (gate,witness,p)=fixture(); let raw=p.owner_json.clone();
        let bundle=prepare(&gate,witness,p);
        assert_eq!(gate.try_recv_applied(),None);
        let mut world=World::new(); assert!(apply_bundle(&mut world,bundle));
        assert_eq!(world.resource::<UiReadModel>().player.gold,90);
        assert_eq!(world.resource::<InventoryModel>().gold,90);
        assert!(world.contains_resource::<MailModel>() && world.contains_resource::<StorageModel>()
            && world.contains_resource::<ShopModel>() && world.contains_resource::<SkillModel>()
            && world.contains_resource::<HeroModel>() && world.contains_resource::<SocialModel>());
        let source=world.resource::<NativeNpcEconomySource>();
        assert_eq!(source.owner_json(),raw);
        assert_eq!(source.owner()["stage5Systems"]["economyProjectionEventIds"],json!(["merged-1"]));
        assert_eq!(source.owner()["stage5Systems"]["intelligentCreaturePearls"],12);
        assert_eq!(source.owner()["stage5Systems"]["mentor"]["menteeExp"],5);
        let ack=gate.try_recv_applied().unwrap();
        assert_eq!(ack.witness(),witness); assert_eq!(ack.binding(),source.binding());
        assert_eq!(ack.connection(),source.binding().connection()); assert_eq!(gate.try_recv_applied(),None);
    }

    #[test]
    fn native_npc_economy_all_models_predecode_before_any_mutation() {
        let (gate,w,p)=fixture();
        for domain in 0..10 {
            let mut invalid=p.clone();
            let field=match domain {0=>&mut invalid.owner_json,1=>&mut invalid.world_json,2=>&mut invalid.ui_json,
                3=>&mut invalid.inventory_json,4=>&mut invalid.mail_json,5=>&mut invalid.storage_json,
                6=>&mut invalid.shop_json,7=>&mut invalid.skill_json,8=>&mut invalid.hero_json,_=>&mut invalid.social_json};
            *field="[".into(); assert_eq!(gate.prepare(w,invalid).unwrap_err(),NativeNpcEconomyError::Decode);
        }
        assert_eq!(gate.try_recv_applied(),None);
    }

    #[test]
    fn native_npc_economy_mandatory_source_domains_cannot_use_defaults() {
        let (gate,w,p)=fixture();
        let owner:Value=serde_json::from_str(&p.owner_json).unwrap();
        for key in ["playerObjectId","entities","playerHp","playerMaxHp","playerMp","playerMaxMp","gold","credit",
            "playerExperience","playerMaxExperience","inventoryCapacity","inventoryItems","beltItems","equipmentItems","storageItems",
            "knownSkills","storageSize","hasExpandedStorage","hasStoragePassword","requireStoragePassword","expandedStorageExpiryTimeBinaryDatetime",
            "stage5Systems","cityCurrencies","heroInventoryItems","heroEquipmentItems","heroInventoryCapacity","heroStats","heroWeights","heroVitals",
            "playerCrystalStats","playerPkPoints","currentWeight","maxWeight","freeBagSlots","maxBagSlots","playerWeights","storagePasswordLastSetBinaryDatetime"] {
            let mut raw=owner.clone();raw.as_object_mut().unwrap().remove(key);
            let mut q=p.clone();q.owner_json=raw.to_string();assert!(gate.prepare(w,q).is_err(),"{key}");
        }
        for key in owner["stage5Systems"].as_object().unwrap().keys() {
            let mut raw=owner.clone();raw["stage5Systems"].as_object_mut().unwrap().remove(key);
            let mut q=p.clone();q.owner_json=raw.to_string();assert!(gate.prepare(w,q).is_err(),"{key}");
        }
    }

    #[test]
    fn native_npc_economy_partial_wrong_actor_scope_and_max_rejected() {
        let (gate,w,p)=fixture();
        for invalid in [SnapshotWitness{complete:false,..w},SnapshotWitness{server_revision:u64::MAX,..w},
            SnapshotWitness{actor:ActorKey::from_server_bytes([3;32]).unwrap(),..w},
            SnapshotWitness{producer_scope:OwnerScope::from_server_bytes([4;32]).unwrap(),..w}] {
            assert_eq!(gate.prepare(invalid,p.clone()).unwrap_err(),NativeNpcEconomyError::Correlation);
        }
        assert!(gate.prepare(SnapshotWitness{server_revision:0,..w},p).is_ok());
    }

    #[test]
    fn native_npc_economy_retirement_fences_prepared_bundle() {
        let (gate,w,p)=fixture();let bundle=prepare(&gate,w,p.clone());gate.retire();
        let mut world=World::new();assert!(!apply_bundle(&mut world,bundle));
        assert!(!world.contains_resource::<UiReadModel>());assert_eq!(gate.try_recv_applied(),None);
        assert_eq!(gate.prepare(w,p).unwrap_err(),NativeNpcEconomyError::Retired);
    }

    #[test]
    fn native_npc_economy_wallet_xp_level_capacity_and_storage_mismatch_rejected() {
        let (gate,w,p)=fixture();
        for field in ["gold","credit","experience","maxExperience","level"] {
            let mut q=p.clone();let mut ui:Value=serde_json::from_str(&q.ui_json).unwrap();ui["player"][field]=json!(999);
            q.ui_json=ui.to_string();assert_eq!(gate.prepare(w,q).unwrap_err(),NativeNpcEconomyError::Projection);
        }
        let mut q=p.clone();q.inventory_json=json!({"gold":90,"capacity":54,"items":[]}).to_string();
        assert_eq!(gate.prepare(w,q).unwrap_err(),NativeNpcEconomyError::Projection);
        let mut q=p;q.storage_json=json!({"items":[],"size":81}).to_string();
        assert_eq!(gate.prepare(w,q).unwrap_err(),NativeNpcEconomyError::Projection);
    }

    #[test]
    fn native_npc_economy_uid_zero_belt_and_equipment_are_complete() {
        let (gate,w,mut p)=fixture();let mut raw:Value=serde_json::from_str(&p.owner_json).unwrap();
        raw["beltItems"]=json!([source_item(0,0,"belt",2,"potion")]);
        raw["equipmentItems"]=json!([source_equipment(11,"weapon","blade")]);p.owner_json=raw.to_string();
        p.inventory_json=json!({"capacity":46,"gold":90,"items":[
            {"uniqueId":0,"key":"0","name":"potion","quantity":2,"slot":0,"container":1,"grade":"common"},
            {"uniqueId":11,"key":"11","name":"blade","quantity":1,"slot":0,"container":2,"grade":"common","durabilityCurrent":0,"durabilityMax":0}]}).to_string();
        let bundle=prepare(&gate,w,p.clone());let mut world=World::new();assert!(apply_bundle(&mut world,bundle));
        assert_eq!(world.resource::<InventoryModel>().items[0].unique_id,Some(0));
        let mut bad:Value=serde_json::from_str(&p.inventory_json).unwrap();bad["items"].as_array_mut().unwrap().pop();
        p.inventory_json=bad.to_string();assert_eq!(gate.prepare(w,p).unwrap_err(),NativeNpcEconomyError::Projection);
    }

    #[test]
    fn native_npc_economy_sparse_named_equipment_uses_crystal_slots() {
        let (gate,w,mut p)=fixture();let mut raw:Value=serde_json::from_str(&p.owner_json).unwrap();
        raw["equipmentItems"]=json!([source_equipment(11,"ringRight","ring"),source_equipment(12,"mount","horse")]);
        p.owner_json=raw.to_string();
        let items=vec![ItemModel{unique_id:Some(11),key:"11".into(),name:"ring".into(),quantity:1,slot:8,container:2,
            grade:Some("common".into()),durability_current:Some(0),durability_max:Some(0),..Default::default()},
            ItemModel{unique_id:Some(12),key:"12".into(),name:"horse".into(),quantity:1,slot:13,container:2,
                grade:Some("common".into()),durability_current:Some(0),durability_max:Some(0),..Default::default()}];
        p.inventory_json=serde_json::to_string(&InventoryModel{gold:90,items,..Default::default()}).unwrap();
        let mut world=World::new();assert!(apply_bundle(&mut world,prepare(&gate,w,p.clone())));
        assert_eq!(world.resource::<InventoryModel>().items.iter().map(|item|item.slot).collect::<Vec<_>>(),vec![8,13]);
        raw["equipmentItems"].as_array_mut().unwrap().reverse();p.owner_json=raw.to_string();
        assert!(gate.prepare(w,p.clone()).is_ok());
        raw["equipmentItems"][0]["slot"]=json!("unknown");p.owner_json=raw.to_string();
        assert_eq!(gate.prepare(w,p).unwrap_err(),NativeNpcEconomyError::Incomplete);
        for (name,slot) in [("weapon",0),("armour",1),("helmet",2),("torch",3),("necklace",4),("braceletLeft",5),
            ("braceletRight",6),("ringLeft",7),("ringRight",8),("amulet",9),("belt",10),("boots",11),("stone",12),("mount",13)] {
            assert_eq!(equipment_slot(Some(&json!(name))),Ok(slot));
        }
        assert_eq!(equipment_slot(Some(&json!("ring-right"))),Ok(8));
    }

    #[test]
    fn native_npc_economy_missing_public_hero_city_and_character_domains_never_apply() {
        let (gate,w,p)=fixture();let owner:Value=serde_json::from_str(&p.owner_json).unwrap();
        for key in ["heroInventoryItems","heroEquipmentItems","heroInventoryCapacity","heroStats","heroVitals","heroWeights","cityCurrencies"] {
            let mut raw=owner.clone();raw.as_object_mut().unwrap().remove(key);
            let mut q=p.clone();q.owner_json=raw.to_string();
            assert_eq!(gate.prepare(w,q).unwrap_err(),NativeNpcEconomyError::Incomplete,"{key}");
        }
        for key in ["name","class","gender","level"] {
            let mut raw=owner.clone();raw["entities"][0].as_object_mut().unwrap().remove(key);
            let mut q=p.clone();q.owner_json=raw.to_string();
            assert_eq!(gate.prepare(w,q).unwrap_err(),NativeNpcEconomyError::Incomplete,"{key}");
        }
        assert_eq!(gate.try_recv_applied(),None);
    }

    #[test]
    fn native_npc_economy_real_item_vectors_reject_null_missing_slot_and_unknown_container() {
        let (gate,w,p)=fixture();let owner:Value=serde_json::from_str(&p.owner_json).unwrap();
        let mut invalids=vec![Value::Null];
        let mut missing=source_item(0,0,"bag1",2,"potion");missing.as_object_mut().unwrap().remove("slot");invalids.push(missing);
        invalids.push(source_item(0,0,"unknown",2,"potion"));
        for invalid in invalids {
            let mut raw=owner.clone();raw["inventoryItems"]=json!([invalid]);
            let mut q=p.clone();q.owner_json=raw.to_string();
            q.inventory_json=serde_json::to_string(&InventoryModel{gold:90,items:vec![ItemModel{unique_id:Some(0),key:"0".into(),name:"potion".into(),
                quantity:2,grade:Some("common".into()),..Default::default()}],..Default::default()}).unwrap();
            assert_eq!(gate.prepare(w,q).unwrap_err(),NativeNpcEconomyError::Incomplete);
        }
    }

    #[test]
    fn native_npc_economy_item_metadata_mismatch_rejects_before_application() {
        let (gate,w,mut p)=fixture();let mut raw:Value=serde_json::from_str(&p.owner_json).unwrap();
        raw["inventoryItems"]=json!([source_item(0,0,"bag1",2,"potion")]);p.owner_json=raw.to_string();
        let inventory=InventoryModel{gold:90,items:vec![ItemModel{unique_id:Some(0),key:"0".into(),name:"potion".into(),quantity:2,
            grade:Some("common".into()),..Default::default()}],..Default::default()};
        p.inventory_json=serde_json::to_string(&inventory).unwrap();assert!(gate.prepare(w,p.clone()).is_ok());
        for (field,changed) in [("key",json!("wrong")),("name",json!("fake")),("icon",json!(1)),("description",json!("fake")),
            ("grade",json!("rare")),("durabilityCurrent",json!(5)),("addedAttack",json!(5)),("sellValue",json!(5))] {
            let mut q=p.clone();let mut model:Value=serde_json::from_str(&q.inventory_json).unwrap();model["items"][0][field]=changed;
            q.inventory_json=model.to_string();assert_eq!(gate.prepare(w,q).unwrap_err(),NativeNpcEconomyError::Projection,"{field}");
        }
        assert_eq!(gate.try_recv_applied(),None);
    }

    #[test]
    fn native_npc_economy_old_revision_and_equal_revision_conflict_never_mutate() {
        let (gate,w,p)=fixture();let mut world=World::new();assert!(apply_bundle(&mut world,prepare(&gate,w,p.clone())));
        assert!(!apply_bundle(&mut world,prepare(&gate,SnapshotWitness{server_revision:6,..w},p.clone())));
        let mut q=p.clone();let mut raw:Value=serde_json::from_str(&q.owner_json).unwrap();
        raw["stage5Systems"]["mentor"]["menteeExp"]=json!(99);q.owner_json=raw.to_string();
        assert!(!apply_bundle(&mut world,prepare(&gate,w,q)));
        assert_eq!(world.resource::<NativeNpcEconomySource>().owner()["stage5Systems"]["mentor"]["menteeExp"],5);
        assert!(apply_bundle(&mut world,prepare(&gate,w,p)));
        assert_eq!(gate.try_recv_applied().unwrap().witness(),w);
        assert_eq!(gate.try_recv_applied().unwrap().witness(),w);assert_eq!(gate.try_recv_applied(),None);
    }

    #[test]
    fn native_npc_economy_bounded_completion_refuses_mutation_when_full() {
        let (gate,w,p)=fixture();let mut world=World::new();
        for _ in 0..MAX_APPLIED { assert!(apply_bundle(&mut world,prepare(&gate,w,p.clone()))); }
        let next=SnapshotWitness{server_revision:8,..w};assert!(!apply_bundle(&mut world,prepare(&gate,next,p.clone())));
        assert_eq!(world.resource::<NativeNpcEconomySource>().witness(),w);
        gate.try_recv_applied().unwrap();assert!(apply_bundle(&mut world,prepare(&gate,next,p)));
    }

    #[test]
    fn native_npc_economy_duplicate_and_escaped_duplicate_are_rejected() {
        let (gate,w,p)=fixture();
        for json in [r#"{"mails":[],"mails":[]}"#,r#"{"mails":[],"m\u0061ils":[]}"#] {
            let mut q=p.clone();q.mail_json=json.into();assert_eq!(gate.prepare(w,q).unwrap_err(),NativeNpcEconomyError::Decode);
        }
    }

    #[test]
    fn native_npc_economy_reset_clears_source_and_retires_old_ack() {
        let (gate,w,p)=fixture();let mut world=World::new();assert!(apply_bundle(&mut world,prepare(&gate,w,p)));
        world.insert_resource(mir2_client_bevy::pending_operations::SessionResetRevision(1));
        apply_pending_native_npc_economy(&mut world);
        assert!(!world.contains_resource::<NativeNpcEconomySource>());assert_eq!(gate.try_recv_applied(),None);
    }

    #[test]
    fn native_npc_economy_revision_history_survives_reset_and_fresh_binding() {
        let (gate,w,p)=fixture();let mut world=World::new();assert!(apply_bundle(&mut world,prepare(&gate,w,p.clone())));
        clear_native_npc_economy_source(&mut world);
        let (fresh,fresh_w,fresh_p)=fixture();
        assert!(!apply_bundle(&mut world,prepare(&fresh,SnapshotWitness{server_revision:6,..fresh_w},fresh_p.clone())));
        assert!(!world.contains_resource::<UiReadModel>() || world.resource::<UiReadModel>().player.gold==90);
        assert!(!world.contains_resource::<NativeNpcEconomySource>());
        assert_eq!(fresh.try_recv_applied(),None);
        assert!(apply_bundle(&mut world,prepare(&fresh,fresh_w,fresh_p)));
        assert!(fresh.try_recv_applied().is_some());
    }

    #[test]
    fn native_npc_economy_same_revision_movement_tick_and_model_epochs_refresh() {
        let (gate,w,p)=fixture();let mut world=World::new();
        assert!(apply_bundle(&mut world,prepare(&gate,w,p.clone())));
        let mut refreshed=p;
        let mut owner:Value=serde_json::from_str(&refreshed.owner_json).unwrap();
        owner["tick"]=json!(u64::MAX);
        owner["entities"][0]["x"]=json!(30);owner["entities"][0]["y"]=json!(40);
        owner["mapFileName"]=json!("changed-map");
        refreshed.owner_json=owner.to_string();
        let mut projected_world:Value=serde_json::from_str(&refreshed.world_json).unwrap();
        projected_world["entities"][0]["x"]=json!(30);projected_world["entities"][0]["y"]=json!(40);
        projected_world["mapFileName"]=json!("changed-map");refreshed.world_json=projected_world.to_string();
        let mut skills:Value=serde_json::from_str(&refreshed.skill_json).unwrap();
        skills["authority"]["sessionEpoch"]=json!(11);skills["authority"]["snapshotSerial"]=json!(12);
        refreshed.skill_json=skills.to_string();
        let mut hero:Value=serde_json::from_str(&refreshed.hero_json).unwrap();
        hero["sessionEpoch"]=json!(11);hero["revision"]=json!(12);refreshed.hero_json=hero.to_string();
        let refreshed_raw=refreshed.owner_json.clone();
        assert!(apply_bundle(&mut world,prepare(&gate,w,refreshed)));
        assert_eq!(world.resource::<NativeNpcEconomySource>().owner_json(),refreshed_raw);
        assert_eq!(world.resource::<NativeNpcEconomySource>().witness(),w);
        assert_eq!(world.resource::<SkillModel>().authority.session_epoch,11);
        assert_eq!(world.resource::<SkillModel>().authority.snapshot_serial,12);
        assert_eq!(world.resource::<HeroModel>().session_epoch,11);
        assert_eq!(world.resource::<crate::RuntimeWorldState>().snapshot.as_ref().unwrap().entities[0].x,30);
        assert_eq!(gate.try_recv_applied().unwrap().witness(),w);
        assert_eq!(gate.try_recv_applied().unwrap().witness(),w);assert_eq!(gate.try_recv_applied(),None);
    }

    #[test]
    fn native_npc_economy_same_revision_refine_clock_refresh_retains_original_source() {
        let (gate,w,p)=fixture();let mut world=World::new();
        assert!(apply_bundle(&mut world,prepare(&gate,w,p.clone())));
        let mut refreshed=p;let mut owner:Value=serde_json::from_str(&refreshed.owner_json).unwrap();
        owner["stage5Systems"]["refine"]["collectDeadlineMs"]=json!(999);
        owner["stage5Systems"]["refine"]["clockEpoch"]=json!("runtime");
        refreshed.owner_json=owner.to_string();assert!(apply_bundle(&mut world,prepare(&gate,w,refreshed)));
        assert_eq!(world.resource::<NativeNpcEconomySource>().owner()["stage5Systems"]["refine"]["collectDeadlineMs"],999);
        assert_eq!(gate.try_recv_applied().unwrap().witness(),w);
        assert_eq!(gate.try_recv_applied().unwrap().witness(),w);assert_eq!(gate.try_recv_applied(),None);
    }

    fn economic_conflict(mut change: impl FnMut(&mut NativeNpcEconomyProjection, &mut Value)) {
        let (gate,w,p)=fixture();let mut world=World::new();
        assert!(apply_bundle(&mut world,prepare(&gate,w,p.clone())));
        let original=p.owner_json.clone();let mut conflicting=p;
        let mut owner:Value=serde_json::from_str(&conflicting.owner_json).unwrap();
        change(&mut conflicting,&mut owner);conflicting.owner_json=owner.to_string();
        // A complete internally-correlated model is admitted for this check;
        // only the retained economic high-water rejects its changed contents.
        let bundle=prepare(&gate,w,conflicting);assert!(!apply_bundle(&mut world,bundle));
        assert_eq!(world.resource::<NativeNpcEconomySource>().owner_json(),original);
        assert_eq!(world.resource::<UiReadModel>().player.gold,90);
        assert_eq!(gate.try_recv_applied().unwrap().witness(),w);assert_eq!(gate.try_recv_applied(),None);
    }

    #[test]
    fn native_npc_economy_same_revision_changed_gold_conflicts() {
        economic_conflict(|p,owner| {
            owner["gold"]=json!(89);
            let mut ui:Value=serde_json::from_str(&p.ui_json).unwrap();ui["player"]["gold"]=json!(89);p.ui_json=ui.to_string();
            let mut inventory:Value=serde_json::from_str(&p.inventory_json).unwrap();inventory["gold"]=json!(89);p.inventory_json=inventory.to_string();
        });
    }

    #[test]
    fn native_npc_economy_same_revision_changed_inventory_conflicts() {
        economic_conflict(|p,owner| {
            owner["inventoryItems"]=json!([source_item(0,0,"bag1",2,"potion")]);
            p.inventory_json=json!({"capacity":46,"gold":90,"items":[
                {"uniqueId":0,"key":"0","name":"potion","quantity":2,"slot":0,"container":0,"grade":"common"}]}).to_string();
        });
    }

    #[test]
    fn native_npc_economy_same_revision_changed_mail_conflicts() {
        economic_conflict(|p,owner| {
            owner["stage5Systems"]["mail"]=json!([{"id":3,"from":"Bank","to":"Alice","subject":"Payment","body":"Settled",
                "gold":7,"items":[],"itemStatesJson":[],"opened":false,"locked":false,"claimed":false,"deleted":false}]);
            p.mail_json=serde_json::to_string(&MailModel { mails:vec![mir2_client_bevy::mail::MailMessage {
                id:3,gold:7,sender:"Bank".into(),..Default::default() }],selected_id:None }).unwrap();
        });
    }

    #[test]
    fn native_npc_economy_same_revision_changed_pearls_conflicts() {
        economic_conflict(|_,owner| { owner["stage5Systems"]["intelligentCreaturePearls"]=json!(13); });
    }

    #[test]
    fn native_npc_economy_public_fingerprint_does_not_invent_private_checkpoint_fields() {
        let (_,_,p)=fixture();let owner:Value=serde_json::from_str(&p.owner_json).unwrap();
        let fingerprint:Value=serde_json::from_str(&owner_economic_fingerprint(&owner).unwrap()).unwrap();
        for absent in ["tick","playerObjectId","knownSkills","buyBack","used","rentals","hasRentedItem"] {
            assert!(fingerprint.get(absent).is_none(),"{absent}");
        }
        assert_eq!(fingerprint["character"],json!({"name":"Alice","class":"Warrior","gender":"Male","level":8}));
        for present in ["cityCurrencies","heroInventoryItems","heroEquipmentItems","heroInventoryCapacity"] {
            assert_eq!(fingerprint[present],owner[present],"{present}");
        }
        assert!(fingerprint["stage5Systems"].get("refine").is_none());
        assert_eq!(fingerprint["stage5Systems"]["mentor"]["menteeExp"],5);
        assert_eq!(fingerprint["stage5Systems"]["relationship"]["marriedDays"],10);
    }
}
