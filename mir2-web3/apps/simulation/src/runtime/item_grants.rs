//! Fresh template grants only. Existing custody never passes through this API.
//! Plan the complete batch without issuing identities, then mint all new roots
//! before a caller publishes any inventory, task, currency or success effects.
use std::collections::BTreeSet;

use bevy_ecs::prelude::World;

use super::inventory::{
    crystal_empty_add_item_slots, item_containers_stack_together, item_stack_identity_compatible,
};
use super::item_uid_issuance::{issue_for_staged_inventory, refresh_world_history_floor, ItemUidIssuance};
use super::items::{
    crystal_item_template_for_item_key, embedded_item_state_from_template,
    validate_committed_item_state_carrier, ItemState,
};
use super::resources::InventoryResource;
use crate::{ItemContainer, UserItemUidReason};

#[derive(Debug, Clone)]
pub(super) struct FreshItemGrant {
    pub(super) prototype: ItemState,
    pub(super) quantity: u32,
    pub(super) reason: UserItemUidReason,
}

impl FreshItemGrant {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn plain(
        container: ItemContainer,
        key: &str,
        name: &str,
        description: &str,
        preferred_slot: u8,
        quantity: u32,
        weight: u16,
        reason: UserItemUidReason,
    ) -> Result<Self, String> {
        let template = crystal_item_template_for_item_key(key)
            .ok_or_else(|| format!("fresh grant template unavailable: {key}"))?;
        let mut prototype = embedded_item_state_from_template(&template, container, preferred_slot);
        prototype.key = key.into();
        prototype.name = name.into();
        prototype.description = description.into();
        prototype.weight = weight;
        // Preserve the ordinary grant helper's functional fields and template
        // derived protocol defaults, so existing plain stacks can still merge.
        prototype.icon = super::items::item_icon_for_key(key);
        prototype.identified = None;
        prototype.grade = match template.grade {
            1 => crate::config::ItemGrade::Common,
            2 => crate::config::ItemGrade::Rare,
            3 => crate::config::ItemGrade::Legendary,
            _ => crate::config::ItemGrade::None,
        };
        prototype.heal_hp = 0;
        prototype.heal_mp = 0;
        Ok(Self { prototype, quantity, reason })
    }
}

pub(super) struct FreshGrantPlan {
    pub(super) inventory: InventoryResource,
    /// Projected from the final inventory, after all roots have real UIDs.
    pub(super) changed: Vec<ItemState>,
}

fn item_at_mut(inventory: &mut InventoryResource, belt: bool, index: usize) -> &mut ItemState {
    if belt { &mut inventory.belt_items[index] } else { &mut inventory.inventory_items[index] }
}

pub(super) fn plan_fresh_item_grants(
    resources: &InventoryResource,
    grants: &[FreshItemGrant],
) -> Result<FreshGrantPlan, String> {
    if grants.len() > 256 {
        return Err("fresh grant batch exceeds limit".into());
    }
    if !grants.is_empty() && matches!(resources.item_uid_issuance, ItemUidIssuance::Unavailable | ItemUidIssuance::Fenced) {
        return Err("fresh item issuance unavailable".into());
    }
    let mut staged = resources.clone();
    let mut changed = BTreeSet::new();
    let mut fresh = Vec::new();
    for grant in grants {
        let prototype = &grant.prototype;
        if grant.quantity == 0 || prototype.unique_id != 0 || prototype.user_item_metadata.is_some()
            || !prototype.socketed.is_empty() || !matches!(prototype.container, ItemContainer::Bag1 | ItemContainer::Bag2 | ItemContainer::Belt | ItemContainer::Quest)
        {
            return Err("fresh grant cannot import an existing or nested carrier".into());
        }
        let template = crystal_item_template_for_item_key(&prototype.key)
            .ok_or("fresh grant template unavailable")?;
        let max_stack = u32::from(template.stack_size.max(1));
        validate_committed_item_state_carrier(prototype).map_err(|error| error.to_string())?;
        let mut remaining = grant.quantity;
        if max_stack > 1 {
            let allow_belt = matches!(prototype.container, ItemContainer::Bag1 | ItemContainer::Bag2 | ItemContainer::Belt);
            for belt in [true, false] {
                if belt && !allow_belt { continue; }
                let items = if belt { &mut staged.belt_items } else { &mut staged.inventory_items };
                for (index, existing) in items.iter_mut().enumerate() {
                    let eligible = if belt {
                        existing.container == ItemContainer::Belt
                            && super::items::crystal_belt_slot_range_for_item_key(&prototype.key)
                                .is_some_and(|(start, end)| (start..end).contains(&existing.slot))
                    } else {
                        item_containers_stack_together(existing.container, prototype.container)
                    };
                    let staged_fresh = fresh.iter().any(|(fresh_belt, fresh_index, _)| *fresh_belt == belt && *fresh_index == index);
                    if !eligible || (existing.unique_id == 0 && !staged_fresh) || existing.quantity >= max_stack
                        || !item_stack_identity_compatible(existing, prototype) { continue; }
                    let added = remaining.min(max_stack - existing.quantity);
                    existing.quantity += added;
                    remaining -= added;
                    validate_committed_item_state_carrier(existing).map_err(|error| error.to_string())?;
                    changed.insert((belt, index));
                    if remaining == 0 { break; }
                }
                if remaining == 0 { break; }
            }
        }
        while remaining > 0 {
            let (container, slot) = crystal_empty_add_item_slots(&staged, prototype.container, &prototype.key)
                .into_iter().next().ok_or("fresh grant has insufficient inventory space")?;
            let mut item = prototype.clone();
            item.container = container;
            item.slot = slot;
            item.quantity = remaining.min(max_stack);
            validate_committed_item_state_carrier(&item).map_err(|error| error.to_string())?;
            remaining -= item.quantity;
            let belt = container == ItemContainer::Belt;
            let items = if belt { &mut staged.belt_items } else { &mut staged.inventory_items };
            let index = items.len();
            items.push(item);
            changed.insert((belt, index));
            fresh.push((belt, index, grant.reason));
        }
    }
    // Nothing above allocates a UID or mutates the live inventory. A failed
    // issuance burns any preceding IDs but discards the entire staged batch.
    for (belt, index, reason) in fresh {
        let item = item_at_mut(&mut staged, belt, index);
        let (container, slot) = (item.container, item.slot);
        let uid = issue_for_staged_inventory(&staged, reason, container, slot)?;
        item_at_mut(&mut staged, belt, index).unique_id = uid;
    }
    let changed = changed.into_iter().map(|(belt, index)| {
        if belt { staged.belt_items[index].clone() } else { staged.inventory_items[index].clone() }
    }).collect();
    Ok(FreshGrantPlan { inventory: staged, changed })
}

pub(super) fn prepare_world_grants(world: &World, grants: &[FreshItemGrant]) -> Result<FreshGrantPlan, String> {
    refresh_world_history_floor(world)?;
    let resources = world.get_resource::<InventoryResource>().ok_or("inventory unavailable")?;
    plan_fresh_item_grants(resources, grants)
}

pub(super) fn publish_world_grants(world: &mut World, grants: &[FreshItemGrant]) -> Result<Vec<ItemState>, String> {
    let plan = prepare_world_grants(world, grants)?;
    *world.resource_mut::<InventoryResource>() = plan.inventory;
    Ok(plan.changed)
}

#[cfg(test)]
#[path = "item_grants_tests.rs"]
pub(super) mod tests;
