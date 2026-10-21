//! Private economic custody, independent of Crystal soul/rental restrictions.
//! No protocol packet can construct or clear this identity.
use super::super::equipment::EquipmentState;
use super::super::resources::{
    HeroInventoryResource, InventoryResource, RuntimeConfigResource, SessionResource,
};
use super::ItemState;
use crate::config::CharacterSaveRecord;
use bevy_ecs::world::World;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::runtime) struct HuntingItemBinding {
    version: u8,
    realm: String,
    account: String,
    index: i32,
    incarnation: String,
    source_ref: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BindingWire {
    version: u8,
    realm: String,
    account: String,
    index: i32,
    incarnation: String,
    source_ref: String,
}

fn token(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn incarnation(value: &str) -> bool {
    let Some(uuid) = value.strip_prefix("pg-character-v1:") else {
        return false;
    };
    uuid.len() == 36
        && uuid.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
        && uuid
            .bytes()
            .any(|byte| (b'1'..=b'9').contains(&byte) || (b'a'..=b'f').contains(&byte))
}

impl HuntingItemBinding {
    fn validate(&self) -> Result<(), String> {
        let source = self
            .source_ref
            .strip_prefix("quest-once-v1:")
            .or_else(|| self.source_ref.strip_prefix("quest-daily-v1:"));
        if self.version != 1
            || !token(&self.realm, 128)
            || !token(&self.account, 256)
            || self.index < 0
            || !incarnation(&self.incarnation)
            || !token(&self.source_ref, 512)
            || !self
                .source_ref
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_:".contains(&b))
            || source.is_none_or(|suffix| {
                let mut parts = suffix.split(':');
                parts
                    .next()
                    .and_then(|id| id.parse::<i32>().ok())
                    .is_none_or(|id| id <= 0)
                    || parts.any(str::is_empty)
            })
        {
            return Err("invalid hunting item binding".into());
        }
        Ok(())
    }

    pub(in crate::runtime) fn for_current_owner(
        world: &World,
        source_ref: &str,
    ) -> Result<Self, String> {
        let config = world
            .get_resource::<RuntimeConfigResource>()
            .ok_or("binding realm unavailable")?;
        let session = world
            .get_resource::<SessionResource>()
            .ok_or("binding owner unavailable")?;
        let value = Self {
            version: 1,
            realm: config
                .config
                .hunting_economy_realm
                .clone()
                .ok_or("binding realm unavailable")?,
            account: session
                .account_id
                .clone()
                .ok_or("binding account unavailable")?,
            index: session
                .selected_character
                .as_ref()
                .ok_or("binding character unavailable")?
                .index,
            incarnation: session
                .hunting_character_incarnation
                .clone()
                .ok_or("binding StartGame witness unavailable")?,
            source_ref: source_ref.into(),
        };
        value.validate()?;
        Ok(value)
    }

    pub(in crate::runtime) fn is_current_owner(&self, world: &World) -> bool {
        let Ok(current) = Self::for_current_owner(world, &self.source_ref) else {
            return false;
        };
        self == &current
    }

    /// Registry custody can prove only these two fields. It must not invent
    /// a live realm/incarnation grant from a stored Hero attachment.
    pub(in crate::runtime) fn matches_account_character(&self, account: &str, index: i32) -> bool {
        self.account == account && self.index == index
    }
}

impl<'de> Deserialize<'de> for HuntingItemBinding {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = BindingWire::deserialize(deserializer)?;
        let value = Self {
            version: raw.version,
            realm: raw.realm,
            account: raw.account,
            index: raw.index,
            incarnation: raw.incarnation,
            source_ref: raw.source_ref,
        };
        value.validate().map_err(serde::de::Error::custom)?;
        Ok(value)
    }
}

pub(in crate::runtime) fn has_bound(item: &ItemState) -> bool {
    let mut pending = vec![(item, 0_usize)];
    let mut nodes = 0;
    while let Some((item, depth)) = pending.pop() {
        nodes += 1;
        if depth > 8
            || nodes > 256
            || item.economic_binding.is_some()
            || item
                .socketed
                .len()
                .saturating_add(nodes)
                .saturating_add(pending.len())
                > 256
        {
            return true;
        }
        pending.extend(item.socketed.iter().map(|child| (child, depth + 1)));
    }
    false
}

pub(in crate::runtime) fn equipment_has_bound(item: &EquipmentState) -> bool {
    item.economic_binding.is_some() || item.socketed.iter().any(has_bound)
}

pub(in crate::runtime) fn belongs_to_current_owner(world: &World, item: &ItemState) -> bool {
    let mut pending = vec![(item, 0_usize)];
    let mut nodes = 0;
    while let Some((item, depth)) = pending.pop() {
        nodes += 1;
        if depth > 8
            || nodes > 256
            || item
                .socketed
                .len()
                .saturating_add(nodes)
                .saturating_add(pending.len())
                > 256
            || item
                .economic_binding
                .as_ref()
                .is_some_and(|binding| !binding.is_current_owner(world))
        {
            return false;
        }
        pending.extend(item.socketed.iter().map(|child| (child, depth + 1)));
    }
    true
}

pub(in crate::runtime) fn equipment_belongs_to_current_owner(
    world: &World,
    item: &EquipmentState,
) -> bool {
    item.economic_binding
        .as_ref()
        .is_none_or(|binding| binding.is_current_owner(world))
        && item
            .socketed
            .iter()
            .all(|child| belongs_to_current_owner(world, child))
}

/// A mismatched marked carrier is never an ordinary legacy inventory. Refuse
/// mutation before borrowing the live collections, independent of policy mode.
pub(in crate::runtime) fn inventory_belongs_to_current_owner(world: &World) -> bool {
    let personal = world
        .get_resource::<InventoryResource>()
        .is_none_or(|inventory| {
            inventory
                .inventory_items
                .iter()
                .chain(&inventory.belt_items)
                .chain(&inventory.storage_items)
                .all(|item| belongs_to_current_owner(world, item))
                && inventory
                    .equipment_items
                    .iter()
                    .all(|item| equipment_belongs_to_current_owner(world, item))
        });
    personal
        && world
            .get_resource::<HeroInventoryResource>()
            .is_none_or(|hero| {
                hero.items
                    .iter()
                    .chain(&hero.equipment)
                    .all(|item| belongs_to_current_owner(world, item))
            })
}

/// Validate the complete private load before any equipment can contribute stats.
/// The caller has authenticated the account and bound the database incarnation;
/// the previous Session actor is deliberately not an authority for a new load.
pub(in crate::runtime) fn validate_character_load(
    world: &World,
    save: &CharacterSaveRecord,
    account: &str,
    incarnation: Option<&str>,
) -> Result<(), String> {
    let realm = world
        .get_resource::<RuntimeConfigResource>()
        .and_then(|runtime| runtime.config.hunting_economy_realm.as_deref());
    let matches = |binding: &HuntingItemBinding| {
        realm == Some(binding.realm.as_str())
            && account == binding.account.as_str()
            && save.character.index == binding.index
            && incarnation == Some(binding.incarnation.as_str())
    };
    let validate_tree =
        |binding: Option<&HuntingItemBinding>, children: &[ItemState]| -> Result<(), String> {
            if binding.is_some_and(|binding| !matches(binding)) {
                return Err("saved bound item does not belong to the loading character".into());
            }
            let mut nodes = 1_usize;
            if children.len().saturating_add(nodes) > 256 {
                return Err("character binding tree exceeds its carrier budget".into());
            }
            let mut pending: Vec<_> = children.iter().map(|child| (child, 1_usize)).collect();
            while let Some((item, depth)) = pending.pop() {
                nodes += 1;
                if depth > 8
                    || nodes > 256
                    || item
                        .socketed
                        .len()
                        .saturating_add(nodes)
                        .saturating_add(pending.len())
                        > 256
                {
                    return Err("character binding tree exceeds its carrier budget".into());
                }
                if item
                    .economic_binding
                    .as_ref()
                    .is_some_and(|binding| !matches(binding))
                {
                    return Err("saved bound item does not belong to the loading character".into());
                }
                pending.extend(item.socketed.iter().map(|child| (child, depth + 1)));
            }
            Ok(())
        };
    for (label, encoded) in [
        ("inventory", &save.inventory_items_json),
        ("belt", &save.belt_items_json),
        ("storage", &save.storage_items_json),
        ("Hero inventory", &save.hero_inventory_items_json),
        ("Hero equipment", &save.hero_equipment_items_json),
    ] {
        for value in encoded {
            let item: ItemState = serde_json::from_str(value)
                .map_err(|_| format!("invalid {label} binding carrier"))?;
            validate_tree(item.economic_binding.as_ref(), &item.socketed)?;
        }
    }
    for value in &save.equipment_items_json {
        let item: EquipmentState = serde_json::from_str(value)
            .map_err(|_| "invalid equipment binding carrier".to_string())?;
        validate_tree(item.economic_binding.as_ref(), &item.socketed)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "hunting_item_binding_tests.rs"]
mod tests;
