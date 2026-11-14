//! Fresh template grants only. Existing custody never passes through this API.
//! Plan the complete batch without issuing identities, then mint all new roots
//! before a caller publishes any inventory, task, currency or success effects.
use std::collections::BTreeSet;

use bevy_ecs::prelude::World;

use super::inventory::{
    crystal_empty_add_item_slots, item_containers_stack_together, item_stack_identity_compatible,
};
use super::item_uid_issuance::{
    issue_for_staged_inventory, refresh_world_history_floor, ItemUidIssuance,
};
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
    pub(super) allow_stack_merge: bool,
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
        Ok(Self {
            prototype,
            quantity,
            reason,
            allow_stack_merge: true,
        })
    }
}

pub(super) struct FreshGrantPlan {
    pub(super) inventory: InventoryResource,
    /// Projected from the final inventory, after all roots have real UIDs.
    pub(super) changed: Vec<ItemState>,
}

fn item_at_mut(inventory: &mut InventoryResource, belt: bool, index: usize) -> &mut ItemState {
    if belt {
        &mut inventory.belt_items[index]
    } else {
        &mut inventory.inventory_items[index]
    }
}

pub(super) fn plan_fresh_item_grants(
    resources: &InventoryResource,
    grants: &[FreshItemGrant],
) -> Result<FreshGrantPlan, String> {
    if grants.len() > 256 {
        return Err("fresh grant batch exceeds limit".into());
    }
    if !grants.is_empty()
        && matches!(
            resources.item_uid_issuance,
            ItemUidIssuance::Unavailable | ItemUidIssuance::Fenced
        )
    {
        return Err("fresh item issuance unavailable".into());
    }
    let mut staged = resources.clone();
    let mut changed = BTreeSet::new();
    let mut fresh = Vec::new();
    for grant in grants {
        let prototype = &grant.prototype;
        if grant.quantity == 0
            || prototype.unique_id != 0
            || prototype.user_item_metadata.is_some()
            || !prototype.socketed.is_empty()
            || !matches!(
                prototype.container,
                ItemContainer::Bag1
                    | ItemContainer::Bag2
                    | ItemContainer::Belt
                    | ItemContainer::Quest
            )
        {
            return Err("fresh grant cannot import an existing or nested carrier".into());
        }
        let template = crystal_item_template_for_item_key(&prototype.key)
            .ok_or("fresh grant template unavailable")?;
        let max_stack = u32::from(template.stack_size.max(1));
        validate_committed_item_state_carrier(prototype).map_err(|error| error.to_string())?;
        let mut remaining = grant.quantity;
        if max_stack > 1 && grant.allow_stack_merge {
            let allow_belt = matches!(
                prototype.container,
                ItemContainer::Bag1 | ItemContainer::Bag2 | ItemContainer::Belt
            );
            for belt in [true, false] {
                if belt && !allow_belt {
                    continue;
                }
                let items = if belt {
                    &mut staged.belt_items
                } else {
                    &mut staged.inventory_items
                };
                for (index, existing) in items.iter_mut().enumerate() {
                    let eligible = if belt {
                        existing.container == ItemContainer::Belt
                            && super::items::crystal_belt_slot_range_for_item_key(&prototype.key)
                                .is_some_and(|(start, end)| (start..end).contains(&existing.slot))
                    } else {
                        item_containers_stack_together(existing.container, prototype.container)
                    };
                    let staged_fresh = fresh.iter().any(|(fresh_belt, fresh_index, _)| {
                        *fresh_belt == belt && *fresh_index == index
                    });
                    if !eligible
                        || (existing.unique_id == 0 && !staged_fresh)
                        || existing.quantity >= max_stack
                        || !item_stack_identity_compatible(existing, prototype)
                    {
                        continue;
                    }
                    let added = remaining.min(max_stack - existing.quantity);
                    existing.quantity += added;
                    remaining -= added;
                    validate_committed_item_state_carrier(existing)
                        .map_err(|error| error.to_string())?;
                    changed.insert((belt, index));
                    if remaining == 0 {
                        break;
                    }
                }
                if remaining == 0 {
                    break;
                }
            }
        }
        while remaining > 0 {
            let (container, slot) =
                crystal_empty_add_item_slots(&staged, prototype.container, &prototype.key)
                    .into_iter()
                    .next()
                    .ok_or("fresh grant has insufficient inventory space")?;
            let mut item = prototype.clone();
            item.container = container;
            item.slot = slot;
            item.quantity = remaining.min(max_stack);
            validate_committed_item_state_carrier(&item).map_err(|error| error.to_string())?;
            remaining -= item.quantity;
            let belt = container == ItemContainer::Belt;
            let items = if belt {
                &mut staged.belt_items
            } else {
                &mut staged.inventory_items
            };
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
    let changed = changed
        .into_iter()
        .map(|(belt, index)| {
            if belt {
                staged.belt_items[index].clone()
            } else {
                staged.inventory_items[index].clone()
            }
        })
        .collect();
    Ok(FreshGrantPlan {
        inventory: staged,
        changed,
    })
}

pub(super) fn prepare_world_grants(
    world: &World,
    grants: &[FreshItemGrant],
) -> Result<FreshGrantPlan, String> {
    let result = (|| {
        refresh_world_history_floor(world)?;
        let resources = world
            .get_resource::<InventoryResource>()
            .ok_or("inventory unavailable")?;
        plan_fresh_item_grants(resources, grants)
    })();
    result.map_err(|error| {
        if let Some(config) = world.get_resource::<super::resources::RuntimeConfigResource>() {
            super::item_uid_issuance::handle_item_uid_failure(&config.config, error)
        } else {
            error
        }
    })
}

pub(super) fn publish_world_grants(
    world: &mut World,
    grants: &[FreshItemGrant],
) -> Result<Vec<ItemState>, String> {
    let plan = prepare_world_grants(world, grants)?;
    *world.resource_mut::<InventoryResource>() = plan.inventory;
    Ok(plan.changed)
}

/// Only complete fresh grants use this boundary. Custody transfers retain their
/// original identities. Validate before issuing, and install the new tree only
/// after every preorder reservation succeeds; failed reservations remain burned.
pub(super) fn assign_fresh_user_item_tree_uids(
    allocator: &crate::UserItemUidAllocator,
    item: &mut mir2_protocol::UserItem,
    reason: UserItemUidReason,
) -> Result<(), String> {
    validate_fresh_user_item_tree(item)?;
    fn maximum(item: &mir2_protocol::UserItem) -> u64 {
        item.slots
            .iter()
            .flatten()
            .map(maximum)
            .fold(item.unique_id, u64::max)
    }
    let floor = maximum(item);
    let mut previous = allocator
        .ensure_issued_through_at_least(floor)
        .map_err(super::item_uid_issuance::item_uid_error)?;
    if previous < floor {
        return Err("fresh item authority returned a lower history floor".into());
    }
    fn reserve(
        allocator: &crate::UserItemUidAllocator,
        item: &mut mir2_protocol::UserItem,
        reason: UserItemUidReason,
        previous: &mut u64,
    ) -> Result<(), String> {
        let uid = allocator
            .issue(reason)
            .map_err(super::item_uid_issuance::item_uid_error)?
            .get();
        if uid <= *previous || crate::monthly_card::skip_monthly_card_reserved_unique_id(uid) != uid
        {
            return Err("fresh item authority returned an overlapping or reserved identity".into());
        }
        *previous = uid;
        item.unique_id = uid;
        for child in item.slots.iter_mut().flatten() {
            reserve(allocator, child, reason, previous)?;
        }
        Ok(())
    }
    let mut staged = item.clone();
    reserve(allocator, &mut staged, reason, &mut previous)?;
    super::items::validate_committed_user_item_carrier(&staged)
        .map_err(|error| error.to_string())?;
    *item = staged;
    Ok(())
}

/// Fresh trees can contain several unassigned zero identities. Validate their
/// structure with private temporary IDs; these IDs never escape this copy or
/// count toward the durable history floor.
pub(super) fn validate_fresh_user_item_tree(item: &mir2_protocol::UserItem) -> Result<(), String> {
    let budget = super::items::UserItemCarrierBudget::default();
    fn bounded(
        item: &mir2_protocol::UserItem,
        depth: usize,
        nodes: &mut usize,
        budget: super::items::UserItemCarrierBudget,
    ) -> Result<(), String> {
        *nodes += 1;
        if depth > budget.max_depth
            || *nodes > budget.max_total_nodes
            || item.slots.len() > budget.max_slots_per_item
            || item.added_stats.len() > budget.max_added_stats_per_item
            || item.awake_values.len() > budget.max_awake_values_per_item
        {
            return Err("fresh item tree exceeds carrier limits".into());
        }
        for child in item.slots.iter().flatten() {
            bounded(child, depth + 1, nodes, budget)?;
        }
        Ok(())
    }
    bounded(item, 0, &mut 0, budget)?;
    fn provisional(item: &mut mir2_protocol::UserItem, next: &mut u64) {
        *next += 1;
        item.unique_id = *next;
        for child in item.slots.iter_mut().flatten() {
            provisional(child, next);
        }
    }
    let mut validation = item.clone();
    provisional(&mut validation, &mut 0);
    super::items::validate_committed_user_item_carrier(&validation)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "item_grants_tests.rs"]
pub(super) mod tests;
