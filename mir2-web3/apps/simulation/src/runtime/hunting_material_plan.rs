//! Main-server material planning. Nothing here deserializes an item capability
//! from a packet, writes live ECS, or performs an economic transaction.
use bevy_ecs::world::World;
use mir2_economy_service::MaterialConversionQuoteSource;
use serde_json::Value;

use super::inventory::{
    allocate_item_unique_id_avoiding, find_empty_inventory_item_slot,
    inventory_container_and_slot_for_index, inventory_index_for_item, inventory_unique_id_is_used,
    is_valid_inventory_slot, item_list_unique_id_is_used,
};
use super::items::hunting_item_binding::inventory_belongs_to_current_owner;
use super::items::hunting_item_conversion::{
    convert_bound_root, source_binding_digest, ConversionIdentity,
};
use super::items::{
    crystal_item_index_for_item_state, try_user_item_from_item_state,
    validate_committed_item_state_carrier, ItemState,
};
use super::resources::{HeroInventoryResource, InventoryResource};
use super::save::encode_state_vec;
use crate::config::{CharacterSaveRecord, ItemContainer};

#[derive(Clone)]
pub(super) struct MaterialItemSource {
    item: ItemState,
    quantity: u32,
    item_digest: String,
    binding_digest: String,
}

#[derive(Clone)]
pub(super) struct MaterialItemPlan {
    before_inventory: Value,
    before_items: Vec<ItemState>,
    after_items: Vec<ItemState>,
    output_unique_id: u64,
}

fn inventory_preimage(inventory: &InventoryResource) -> Result<Value, String> {
    serde_json::to_value((
        &inventory.reserved_item_unique_ids,
        inventory.inventory_capacity,
        &inventory.inventory_items,
        &inventory.belt_items,
        &inventory.storage_items,
        &inventory.equipment_items,
    ))
    .map_err(|_| "material inventory preimage unavailable".into())
}

fn item_digest(item: &ItemState) -> Result<String, String> {
    let value = serde_json::to_value(item).map_err(|_| "material item digest unavailable")?;
    crate::config::hunting_material::material_item_digest(&value)
}

fn eligible_root(item: &ItemState) -> Result<(), String> {
    if item.unique_id == 0
        || item.quantity == 0
        || !matches!(item.container, ItemContainer::Bag1 | ItemContainer::Bag2)
        || item.equip_slot.is_some()
        || item.economic_binding.is_none()
        || item.economic_conversion.is_some()
        || !item.socketed.is_empty()
        || item.user_item_metadata.as_ref().is_some_and(|metadata| {
            metadata.slots.iter().any(Option::is_some)
                || metadata
                    .captured_socket_positions
                    .as_ref()
                    .is_some_and(|slots| slots.iter().any(Option::is_some))
        })
    {
        return Err("material conversion requires a plain bound bag root".into());
    }
    validate_committed_item_state_carrier(item).map_err(|_| "invalid material item carrier")?;
    let mut transferable = item.clone();
    transferable.economic_binding = None;
    if !super::social_economy::stage5_trade_item_can_enter(&transferable)
        || item.sealed_expiry_time_binary_datetime != 0
        || item.sealed_next_time_binary_datetime != 0
    {
        return Err("material has another transfer restriction".into());
    }
    Ok(())
}

/// The config authority calls the same typed source checks inside its complete
/// checkpoint/CAS boundary. This does not accept a client snapshot or mint items.
pub(crate) fn validate_hunting_material_quote_source(
    realm: &str,
    account: &str,
    incarnation: &str,
    index: i32,
    before: &CharacterSaveRecord,
    input: &MaterialConversionQuoteSource,
) -> Result<(), String> {
    if before.character.index != index
        || before.revision != input.input_save_revision
        || input.quantity == 0
        || input.quantity > input.input_count
    {
        return Err("material source checkpoint identity changed".into());
    }
    let items = before
        .inventory_items_json
        .iter()
        .map(|item| {
            serde_json::from_str::<ItemState>(item)
                .map_err(|_| "invalid material source checkpoint".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let matches = items
        .iter()
        .filter(|item| item.unique_id == input.input_unique_id)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err("material source UID is not unique in bag".into());
    }
    let item = matches[0];
    eligible_root(item)?;
    let logical_slot = inventory_index_for_item(item).ok_or("invalid material source bag cell")?;
    if !is_valid_inventory_slot(logical_slot, before.inventory_capacity)
        || inventory_container_and_slot_for_index(logical_slot) != Some((item.container, item.slot))
        || items
            .iter()
            .filter(|other| inventory_index_for_item(other) == Some(logical_slot))
            .count()
            != 1
    {
        return Err("material source bag cell is ambiguous".into());
    }
    let owner = serde_json::to_value(
        item.economic_binding
            .as_ref()
            .ok_or("unbound material source")?,
    )
    .map_err(|_| "invalid material owner")?;
    if owner["realm"].as_str() != Some(realm)
        || owner["account"].as_str() != Some(account)
        || owner["index"].as_i64() != Some(i64::from(index))
        || owner["incarnation"].as_str() != Some(incarnation)
        || crystal_item_index_for_item_state(item) != input.item_index
        || u64::from(item.quantity) != input.input_count
        || item_digest(item)? != input.input_item_digest
        || source_binding_digest(item)? != input.source_binding_digest
    {
        return Err("material typed source does not match the original quote".into());
    }
    Ok(())
}

impl MaterialItemSource {
    pub(super) fn capture(world: &World, unique_id: u64, quantity: u64) -> Result<Self, String> {
        if unique_id == 0 || quantity == 0 || !inventory_belongs_to_current_owner(world) {
            return Err("material source owner unavailable".into());
        }
        let quantity = u32::try_from(quantity).map_err(|_| "material quantity overflow")?;
        let inventory = world.resource::<InventoryResource>();
        let index = inventory
            .inventory_items
            .iter()
            .position(|item| item.unique_id == unique_id)
            .ok_or("material source is not in the personal bag")?;
        let item = &inventory.inventory_items[index];
        let logical_slot = inventory_index_for_item(item).ok_or("invalid material bag slot")?;
        if !is_valid_inventory_slot(logical_slot, inventory.inventory_capacity)
            || inventory_container_and_slot_for_index(logical_slot)
                != Some((item.container, item.slot))
            || item.quantity < quantity
            || inventory
                .inventory_items
                .iter()
                .filter(|other| inventory_index_for_item(other) == Some(logical_slot))
                .count()
                != 1
        {
            return Err("material conversion requires a plain bound bag root".into());
        }
        // Economic conversion never removes the original game's own trade,
        // soul, rental or sealing rules. Refuse a result that remains ineligible.
        eligible_root(item)?;
        let mut other = inventory.clone();
        other.inventory_items.remove(index);
        let hero = world.get_resource::<HeroInventoryResource>();
        if inventory_unique_id_is_used(&other, unique_id)
            || hero.is_some_and(|hero| {
                item_list_unique_id_is_used(&hero.items, unique_id)
                    || item_list_unique_id_is_used(&hero.equipment, unique_id)
            })
        {
            return Err("material source UID is ambiguous".into());
        }
        Ok(Self {
            item: item.clone(),
            quantity,
            item_digest: item_digest(item)?,
            binding_digest: source_binding_digest(item)?,
        })
    }

    pub(super) fn quote_source(&self, revision: u64) -> MaterialConversionQuoteSource {
        MaterialConversionQuoteSource {
            item_index: crystal_item_index_for_item_state(&self.item),
            input_unique_id: self.item.unique_id,
            input_count: u64::from(self.item.quantity),
            quantity: u64::from(self.quantity),
            input_save_revision: revision,
            input_item_digest: self.item_digest.clone(),
            source_binding_digest: self.binding_digest.clone(),
        }
    }

    pub(super) fn plan(
        &self,
        world: &World,
        identity: ConversionIdentity<'_>,
    ) -> Result<MaterialItemPlan, String> {
        let current = Self::capture(world, self.item.unique_id, u64::from(self.quantity))?;
        if current.item_digest != self.item_digest || current.binding_digest != self.binding_digest
        {
            return Err("material source changed after quote".into());
        }
        let inventory = world.resource::<InventoryResource>();
        let mut after_items = inventory.inventory_items.clone();
        let index = after_items
            .iter()
            .position(|item| item.unique_id == self.item.unique_id)
            .ok_or("material source missing")?;
        let mut converted = convert_bound_root(world, &current.item, identity)?;
        converted.quantity = self.quantity;
        if self.quantity == self.item.quantity {
            after_items[index] = converted.clone();
        } else {
            let (container, slot) = find_empty_inventory_item_slot(
                &after_items,
                self.item.container,
                inventory.inventory_capacity,
            )
            .ok_or("material conversion requires an empty bag slot")?;
            let mut allocator = inventory.clone();
            allocator.reserved_item_unique_ids.insert(0);
            let hero = world.get_resource::<HeroInventoryResource>();
            let reserved = hero
                .map(|hero| {
                    hero.items
                        .iter()
                        .chain(&hero.equipment)
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            converted.container = container;
            converted.slot = slot;
            converted.unique_id =
                allocate_item_unique_id_avoiding(&allocator, container, slot, &reserved);
            if converted.unique_id == 0 || converted.unique_id == self.item.unique_id {
                return Err("material output UID unavailable".into());
            }
            after_items[index].quantity -= self.quantity;
            after_items.push(converted.clone());
        }
        validate_committed_item_state_carrier(&converted)
            .map_err(|_| "invalid converted material carrier")?;
        try_user_item_from_item_state(&converted).map_err(|_| "invalid converted material wire")?;
        Ok(MaterialItemPlan {
            before_inventory: inventory_preimage(inventory)?,
            before_items: inventory.inventory_items.clone(),
            after_items,
            output_unique_id: converted.unique_id,
        })
    }
}

impl MaterialItemPlan {
    pub(super) fn output_unique_id(&self) -> u64 {
        self.output_unique_id
    }
    pub(super) fn checkpoint(
        &self,
        before: &CharacterSaveRecord,
    ) -> Result<CharacterSaveRecord, String> {
        if before.inventory_items_json != encode_state_vec(&self.before_items) {
            return Err("material complete source checkpoint changed".into());
        }
        let mut after = before.clone();
        after.inventory_items_json = encode_state_vec(&self.after_items);
        Ok(after)
    }
    /// Publish only over the original complete item preimage. The caller must
    /// also check its opaque process-local mutation witness and owner authority.
    pub(super) fn apply_to_inventory(&self, inventory: &mut InventoryResource) -> bool {
        if inventory_preimage(inventory).ok().as_ref() != Some(&self.before_inventory) {
            return false;
        }
        inventory.inventory_items = self.after_items.clone();
        true
    }
}

#[cfg(test)]
#[path = "hunting_material_plan_tests.rs"]
mod tests;
