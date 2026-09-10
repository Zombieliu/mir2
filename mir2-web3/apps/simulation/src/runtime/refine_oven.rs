//! Authoritative refine target custody. Collected refinement belongs to the
//! item's existing UserItem fields, not to a singleton session job.
use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use bevy_ecs::prelude::World;
use mir2_protocol::ServerPacket;

use super::components::current_player_is_dead;
use super::crystal_compat::{
    CRYSTAL_BIND_DONT_UPGRADE, CRYSTAL_STAT_MAX_DC, CRYSTAL_STAT_MAX_MC, CRYSTAL_STAT_MAX_SC,
};
use super::inventory::item_matches_inventory_unique_id;
use super::item_custody;
use super::items::{
    item_has_crystal_or_rental_bind_flag, try_item_state_from_user_item,
    try_user_item_from_item_state, ItemState,
};
use super::npc::current_crystal_npc_service_in_range;
use super::packets::{
    apply_refine_success, refine_components, refine_deterministic_1_99, refine_is_weapon,
    refine_success_chance_crystal, refine_target_stat, refine_weapon_added_stat_sum,
    refine_weapon_luck, refine_weapon_required,
};
use super::resources::{
    is_in_world, runtime_tick, InventoryResource, PlayerRuntimeResource, RuntimeConfigResource,
    Stage5SystemsResource,
};
use crate::config::Stage5RefineState;

fn clock() -> &'static (String, Instant) {
    static CLOCK: OnceLock<(String, Instant)> = OnceLock::new();
    CLOCK.get_or_init(|| {
        (
            format!(
                "{}:{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ),
            Instant::now(),
        )
    })
}
fn now_ms() -> u64 {
    clock().1.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

fn remaining_at(state: &Stage5RefineState, epoch: &str, now: u64) -> u64 {
    if state.clock_epoch.as_deref() == Some(epoch) {
        state
            .collect_deadline_ms
            .map(|deadline| deadline.saturating_sub(now))
            .unwrap_or(state.remaining_ms)
    } else {
        state.remaining_ms
    }
}

/// Same server: retain the monotonic deadline, including offline character time.
/// New server: use the last saved remaining duration, excluding server downtime.
pub(super) fn restore_timer(state: &mut Stage5RefineState) -> Result<(), String> {
    validate(state)?;
    if state.oven_item_state_json.is_none() {
        return Ok(());
    }
    let remaining = remaining_at(state, &clock().0, now_ms());
    let deadline = now_ms()
        .checked_add(remaining)
        .ok_or("refine deadline overflow")?;
    state.remaining_ms = remaining;
    state.collect_deadline_ms = Some(deadline);
    state.clock_epoch = Some(clock().0.clone());
    state.refining = true;
    state.ready = remaining == 0;
    Ok(())
}

pub(super) fn snapshot_timer(state: &mut Stage5RefineState, for_save: bool) {
    if state.oven_item_state_json.is_none() {
        return;
    }
    state.remaining_ms = remaining_at(state, &clock().0, now_ms());
    state.ready = state.remaining_ms == 0;
    state.refining = true;
    if !for_save {
        state.clock_epoch = None;
        state.collect_deadline_ms = None;
    }
}

pub(super) fn validate(state: &Stage5RefineState) -> Result<(), String> {
    if let Some(encoded) = &state.oven_item_state_json {
        let key = state
            .current_item
            .as_deref()
            .ok_or("refine oven target key missing")?;
        let item = item_custody::decode(key, encoded)?;
        let wire = try_user_item_from_item_state(&item).map_err(|e| e.to_string())?;
        if wire.refine_added == 0
            || wire.refined_value > 3
            || !state.refining
            || state.pending_unique_id != 0
            || state.pending_stat != 0
            || state.pending_chance != 0
        {
            return Err("invalid refine oven target/phase".into());
        }
        if state.clock_epoch.is_some() != state.collect_deadline_ms.is_some() {
            return Err("incomplete refine clock anchor".into());
        }
    } else if state.remaining_ms != 0
        || state.clock_epoch.is_some()
        || state.collect_deadline_ms.is_some()
    {
        return Err("refine timer has no target".into());
    }
    Ok(())
}

pub(super) fn service(world: &World, label: &str) -> bool {
    is_in_world(world)
        && !current_player_is_dead(world)
        && current_crystal_npc_service_in_range(world).is_some_and(|s| s.label_key == label)
}
fn failed() -> Vec<ServerPacket> {
    vec![ServerPacket::NPCCollectRefine { success: false }]
}
fn encoded_stat(stat: Option<u8>) -> u8 {
    match stat {
        Some(CRYSTAL_STAT_MAX_DC) => 1,
        Some(CRYSTAL_STAT_MAX_MC) => 2,
        Some(CRYSTAL_STAT_MAX_SC) => 3,
        _ => 0,
    }
}
fn decoded_stat(value: u8) -> Option<u8> {
    match value {
        1 => Some(CRYSTAL_STAT_MAX_DC),
        2 => Some(CRYSTAL_STAT_MAX_MC),
        3 => Some(CRYSTAL_STAT_MAX_SC),
        _ => None,
    }
}

/// The previous implementation kept an immediately-ready target in the bag.
/// Migrate only an unambiguous existing instance; never recreate a missing one.
pub(super) fn migrate_legacy_pending(
    items: &mut [ItemState],
    state: &mut Stage5RefineState,
) -> Result<(), String> {
    if state.oven_item_state_json.is_some() || state.pending_unique_id == 0 {
        return Ok(());
    }
    if !state.refining || !state.ready {
        return Err("ambiguous legacy refine phase".into());
    }
    let key = state
        .current_item
        .as_deref()
        .ok_or("legacy refine key missing")?;
    let matching: Vec<_> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item_matches_inventory_unique_id(item, state.pending_unique_id))
        .map(|(index, _)| index)
        .collect();
    if matching.len() != 1 {
        return Err("legacy refine target missing or ambiguous".into());
    }
    let index = matching[0];
    if items[index].key != key {
        return Err("legacy refine key mismatch".into());
    }
    let mut wire = try_user_item_from_item_state(&items[index]).map_err(|e| e.to_string())?;
    if wire.refine_added != 0 {
        return Err("legacy refine duplicates item-owned refinement".into());
    }
    if state.pending_stat != 0 && encoded_stat(Some(state.pending_stat)) == 0 {
        return Err("invalid legacy refine stat".into());
    }
    wire.refine_added = 1;
    wire.refined_value = encoded_stat(Some(state.pending_stat));
    wire.refine_success_chance = i32::from(state.pending_chance);
    items[index] =
        try_item_state_from_user_item(items[index].clone(), &wire).map_err(|e| e.to_string())?;
    state.current_item = None;
    state.refining = false;
    state.ready = false;
    state.pending_unique_id = 0;
    state.pending_chance = 0;
    state.pending_stat = 0;
    Ok(())
}

pub(super) fn menu(world: &World) -> Vec<ServerPacket> {
    let state = &world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .refine;
    vec![ServerPacket::NPCRefine {
        rate: world.resource::<RuntimeConfigResource>().config.refine_cost as f32,
        refining: state.oven_item_state_json.is_some() || state.refining,
    }]
}

pub(super) fn start(world: &mut World, unique_id: u64) -> Vec<ServerPacket> {
    if !service(world, "REFINE") || item_custody::refresh(world).is_err() {
        return failed();
    }
    let state = &world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .refine;
    // Legacy key-only jobs remain held; an incoming request cannot overwrite them.
    if state.oven_item_state_json.is_some()
        || state.refining
        || state.current_item.is_some()
        || state.pending_unique_id != 0
    {
        return failed();
    }
    let inventory = world.resource::<InventoryResource>();
    let Some(index) = inventory
        .inventory_items
        .iter()
        .position(|i| item_matches_inventory_unique_id(i, unique_id))
    else {
        return failed();
    };
    let item = inventory.inventory_items[index].clone();
    let Ok(mut wire) = try_user_item_from_item_state(&item) else {
        return failed();
    };
    if !refine_is_weapon(&item)
        || wire.refine_added != 0
        || item_has_crystal_or_rental_bind_flag(&item, CRYSTAL_BIND_DONT_UPGRADE)
        || !item_custody::can_take(inventory, &item)
    {
        return failed();
    }
    let ingredients: Result<Vec<_>, String> = state
        .slots
        .iter()
        .map(|(slot, key)| {
            item_custody::decode(
                key,
                state
                    .item_states
                    .get(slot)
                    .ok_or("legacy refine ingredient missing")?,
            )
        })
        .collect();
    let Ok(ingredients) = ingredients else {
        return failed();
    };
    let config = &world.resource::<RuntimeConfigResource>().config;
    let Some(cost) = u32::try_from(refine_weapon_required(&item))
        .ok()
        .and_then(|level| level.checked_mul(10))
        .and_then(|v| v.checked_mul(config.refine_cost))
    else {
        return failed();
    };
    if world.resource::<PlayerRuntimeResource>().gold < cost {
        return failed();
    }
    let duration = config.refine_duration_ms;
    let Some(deadline) = now_ms().checked_add(duration) else {
        return failed();
    };
    let components = refine_components(&ingredients);
    let (stat, strength) = refine_target_stat(&components);
    wire.refined_value = encoded_stat(stat);
    wire.refine_added = 1;
    wire.refine_success_chance = refine_success_chance_crystal(
        &components,
        strength,
        refine_weapon_required(&item),
        refine_weapon_added_stat_sum(&item),
        refine_weapon_luck(&item),
        true,
    );
    let Ok(target) = try_item_state_from_user_item(item, &wire) else {
        return failed();
    };
    let Some(encoded) = item_custody::encode(&target) else {
        return failed();
    };
    let mut staged = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .clone();
    staged.refine = Stage5RefineState {
        oven_item_state_json: Some(encoded),
        current_item: Some(target.key.clone()),
        refining: true,
        ready: duration == 0,
        remaining_ms: duration,
        clock_epoch: Some(clock().0.clone()),
        collect_deadline_ms: Some(deadline),
        ..Default::default()
    };
    if item_custody::reserved_ids(&staged).is_err() {
        return failed();
    }
    world
        .resource_mut::<InventoryResource>()
        .inventory_items
        .remove(index);
    world.resource_mut::<PlayerRuntimeResource>().gold -= cost;
    world.resource_mut::<Stage5SystemsResource>().stage5_systems = staged;
    item_custody::refresh(world).expect("validated oven custody");
    let mut packets = vec![
        ServerPacket::LoseGold { gold: cost },
        ServerPacket::RefineItem { unique_id },
    ];
    if duration == 0 {
        packets.extend(collect_ready(world));
    }
    packets
}

pub(super) fn collect(world: &mut World) -> Vec<ServerPacket> {
    if !service(world, "REFINECOLLECT") {
        return failed();
    }
    collect_ready(world)
}
fn collect_ready(world: &mut World) -> Vec<ServerPacket> {
    if item_custody::refresh(world).is_err() {
        return failed();
    }
    let state = &world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .refine;
    if remaining_at(state, &clock().0, now_ms()) != 0 {
        return failed();
    }
    let Some(item) = state
        .current_item
        .as_deref()
        .zip(state.oven_item_state_json.as_deref())
        .and_then(|(key, encoded)| item_custody::decode(key, encoded).ok())
    else {
        return failed();
    };
    let Some((inventory, changed)) =
        item_custody::plan_return(world.resource::<InventoryResource>(), &item, None, false)
    else {
        return failed();
    };
    let mut packets =
        item_custody::returned_packets(world.resource::<InventoryResource>(), &changed);
    *world.resource_mut::<InventoryResource>() = inventory;
    let state = &mut world
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .refine;
    // Material custody is independent and must not be destroyed by collection.
    state.oven_item_state_json = None;
    state.current_item = None;
    state.remaining_ms = 0;
    state.clock_epoch = None;
    state.collect_deadline_ms = None;
    state.refining = false;
    state.ready = false;
    item_custody::refresh(world).expect("validated oven collection");
    packets.push(ServerPacket::NPCCollectRefine { success: true });
    packets
}

pub(super) fn check(world: &mut World, unique_id: u64) -> Vec<ServerPacket> {
    if !service(world, "REFINECHECK") {
        return failed();
    }
    let inventory = world.resource::<InventoryResource>();
    let Some(index) = inventory
        .inventory_items
        .iter()
        .position(|i| item_matches_inventory_unique_id(i, unique_id))
    else {
        return failed();
    };
    let mut item = inventory.inventory_items[index].clone();
    let Ok(wire) = try_user_item_from_item_state(&item) else {
        return failed();
    };
    if wire.refine_added == 0 || wire.refined_value > 3 {
        return failed();
    }
    let tick = runtime_tick(world);
    let stat = if refine_deterministic_1_99(tick, unique_id, 0x05A1) > wire.refine_success_chance {
        None
    } else {
        decoded_stat(wire.refined_value)
    };
    if let Some(stat) = stat {
        let multiplier = if refine_deterministic_1_99(tick, unique_id, 0xC817) < 10 {
            2
        } else {
            1
        };
        apply_refine_success(&mut item, stat, i32::from(wire.refine_added) * multiplier);
        let Some(metadata) = item.user_item_metadata.as_mut() else {
            return failed();
        };
        metadata.refine_added = 0;
        metadata.refined_value = 0;
        metadata.refine_success_chance = 0;
        let Ok(item_wire) = try_user_item_from_item_state(&item) else {
            return failed();
        };
        world.resource_mut::<InventoryResource>().inventory_items[index] = item;
        vec![ServerPacket::ItemUpgraded { item: item_wire }]
    } else {
        world
            .resource_mut::<InventoryResource>()
            .inventory_items
            .remove(index);
        vec![ServerPacket::RefineItem { unique_id }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deadline_boundary_and_server_epoch_are_distinct() {
        let state = Stage5RefineState {
            remaining_ms: 500,
            clock_epoch: Some("server-a".into()),
            collect_deadline_ms: Some(1000),
            ..Default::default()
        };
        assert_eq!(remaining_at(&state, "server-a", 999), 1);
        assert_eq!(remaining_at(&state, "server-a", 1000), 0);
        assert_eq!(remaining_at(&state, "server-a", 2000), 0);
        assert_eq!(remaining_at(&state, "server-b", 2000), 500);
    }
}
