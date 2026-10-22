//! Personal half of a shared-Zone mining swing. The Zone owns cells, timing
//! and rolls; the authenticated session owns the equipped tool and ore carrier.
use super::crystal_compat::{CRYSTAL_STAT_ACCURACY, CRYSTAL_STAT_STRONG};
use super::equipment::user_item_from_equipment_state;
use super::inventory::{add_or_increment_item_with_random_metadata, crystal_empty_add_item_slots};
use super::items::{
    crystal_item_key_for_template, crystal_item_stat_value, crystal_item_template_for_item_key,
};
use super::resources::{InventoryResource, RuntimeConfigResource, SessionResource, is_in_world};
use super::session::SimulationSession;
use super::zone::{ZoneMiningSwing, ZoneMiningTool};
use crate::config::{EquipmentSlot, ItemContainer};
use bevy_ecs::prelude::World;
use mir2_game_data::crystal_item_by_name;
use mir2_protocol::ServerPacket;

/// Stage the unchanged ordinary-ore constructor against isolated inventory.
/// It chooses its usual bag/belt cell before the optional raw-stone payout.
pub(super) fn stage_shared_mining_ore(
    world: &World,
    swing: &ZoneMiningSwing,
) -> Result<(InventoryResource, Option<super::items::ItemState>), String> {
    let inventory = world.resource::<InventoryResource>().clone();
    let Some(ore) = &swing.ore else {
        return Ok((inventory, None));
    };
    let Some(template) = crystal_item_by_name(&ore.item_name) else {
        return Ok((inventory, None));
    };
    let key = crystal_item_key_for_template(&template);
    let config = &world.resource::<RuntimeConfigResource>().config;
    let destination = crystal_empty_add_item_slots(&inventory, ItemContainer::Bag1, &key)
        .into_iter()
        .next()
        .or_else(|| {
            crystal_empty_add_item_slots(&inventory, ItemContainer::Belt, &key)
                .into_iter()
                .next()
        });
    let Some((container, slot)) = destination.filter(|_| config.item_is_allowed(&template.name))
    else {
        return Ok((inventory, None));
    };
    let random =
        super::drops::crystal_random_drop_stats(&template, swing.now_ms(), swing.object_id, 0);
    let mut staged = World::new();
    staged.insert_resource(inventory);
    let item = add_or_increment_item_with_random_metadata(
        &mut staged,
        container,
        &key,
        &template.name,
        template.tooltip.as_deref().unwrap_or(""),
        slot,
        1,
        u16::from(template.weight),
        Some(ore.current_dura),
        Some(template.durability.saturating_add(random.durability_bonus)),
        random.added_attack,
        random.added_defence,
        random.added_stats,
        random.cursed,
        template.slots.saturating_add(random.socket_slots),
    );
    super::items::validate_committed_item_state_carrier(&item).map_err(|e| e.to_string())?;
    Ok((
        staged
            .remove_resource::<InventoryResource>()
            .expect("staged inventory exists"),
        Some(item),
    ))
}

pub(super) fn mining_tool(world: &World) -> Option<ZoneMiningTool> {
    if !is_in_world(world) {
        return None;
    }
    let weapon = world
        .resource::<InventoryResource>()
        .equipment_items
        .iter()
        .find(|item| item.slot == EquipmentSlot::Weapon)?;
    let template = crystal_item_template_for_item_key(&weapon.key)?;
    if !template.can_mine || weapon.durability_current == 0 {
        return None;
    }
    let exact = user_item_from_equipment_state(weapon)?;
    let total = |stat| {
        crystal_item_stat_value(&template, stat).saturating_add(
            weapon
                .added_stats
                .iter()
                .filter(|s| s.stat == stat)
                .map(|s| s.value)
                .sum::<i32>(),
        )
    };
    let stats = super::stats::player_stats(world);
    let level = world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()?
        .level;
    let delay = 1400i64
        - i64::from(stats.attack_speed()) * 60
        - i64::from((i32::from(level) * 14).min(370));
    Some(ZoneMiningTool {
        unique_id: exact.unique_id,
        durability: weapon.durability_current,
        accuracy: total(CRYSTAL_STAT_ACCURACY),
        strong: total(CRYSTAL_STAT_STRONG),
        mine_rate_percent: stats.get(103),
        attack_delay_ms: delay.max(550) as u64,
    })
}

impl SimulationSession {
    pub fn shared_mining_tool(&self) -> Option<ZoneMiningTool> {
        mining_tool(self.app.world())
    }

    /// Persist ore and tool wear together before exposing either to clients.
    /// Crystal checks inventory space here, not bag weight. A full bag skips
    /// the payout but still consumes the wall stone and tool durability.
    pub fn apply_shared_mining_swing(
        &mut self,
        swing: &ZoneMiningSwing,
    ) -> Result<Vec<ServerPacket>, String> {
        let identity = self
            .active_identity()
            .ok_or("mining requires an authenticated character")?;
        if identity.account_id != swing.account_id
            || identity.character_index != swing.character_index
            || self.shared_mining_tool().as_ref() != Some(&swing.tool)
        {
            return Err("mining character or equipped tool changed before commit".to_string());
        }
        // An unknown raw-only commit can leave the live tool unchanged. Do
        // not let workshop availability or a zero ordinary hit bypass its
        // durable write fence and advance the shared mining ticket.
        self.app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .ensure_account_store_writable()?;
        if swing.raw_stone.is_some() && swing.raw_tool_damage() == 0 {
            return Err("raw-stone mining ticket must include tool wear".into());
        }
        if super::stones::workshop_enabled(self.app.world()) {
            if let Some(index) = swing.raw_stone {
                if !mir2_game_data::RAW_STONE_INDICES.contains(&index) {
                    return Err("unrecognized workshop mining extension".into());
                }
                let template = mir2_game_data::crystal_item_by_index(index)
                    .ok_or("raw stone template unavailable")?;
                let key = crystal_item_key_for_template(&template);
                let (inventory, ordinary) = stage_shared_mining_ore(self.app.world(), swing)?;
                let destination =
                    crystal_empty_add_item_slots(&inventory, ItemContainer::Bag1, &key)
                        .into_iter()
                        .find(|(container, _)| {
                            matches!(container, ItemContainer::Bag1 | ItemContainer::Bag2)
                        });
                // Stone custody requires an actual bag cell; never mint a
                // registry record for a skipped payout or a belt-only space.
                // The explicit server workshop opt-in admits these three
                // extension indices even when the imported profile predates
                // them. Ordinary ore still uses its normal profile gate below.
                if let Some(destination) = destination {
                    self.begin_stone_workshops()
                        .map_err(|e| format!("mining actor unavailable: {e:?}"))?;
                    let checkpoint = self
                        .active_character_checkpoint()
                        .ok_or("mining checkpoint unavailable")?;
                    let (position, direction) = swing.player_transform();
                    self.force_authoritative_player_transform(position, direction);
                    match self.apply_mined_raw_stone(swing, index, destination, inventory, ordinary)
                    {
                        Ok(Some(packets)) => return Ok(packets),
                        Ok(None) => {
                            // Known exhausted ledger: settle ordinary mining below.
                            self.restore_active_character_checkpoint(&checkpoint)?;
                        }
                        Err(error) => {
                            self.restore_active_character_checkpoint(&checkpoint)?;
                            return Err(error);
                        }
                    }
                }
            }
        }
        if swing.durability_damage == 0 {
            return Ok(Vec::new());
        }
        let checkpoint = self
            .active_character_checkpoint()
            .ok_or("mining checkpoint unavailable")?;
        let (position, direction) = swing.player_transform();
        self.force_authoritative_player_transform(position, direction);
        let mut packets = Vec::new();
        let world = self.app.world_mut();
        if let Some(ore) = swing.ore.as_ref() {
            if let Some(template) = crystal_item_by_name(&ore.item_name) {
                let key = crystal_item_key_for_template(&template);
                let inventory = world.resource::<InventoryResource>();
                let config = &world.resource::<RuntimeConfigResource>().config;
                // Original AddItem prefers all unlocked bag cells for Ore,
                // then falls back to any of the six belt cells. FreeSpace
                // counts the complete inventory, even for non-potion items.
                let destination =
                    crystal_empty_add_item_slots(inventory, ItemContainer::Bag1, &key)
                        .into_iter()
                        .next()
                        .or_else(|| {
                            crystal_empty_add_item_slots(inventory, ItemContainer::Belt, &key)
                                .into_iter()
                                .next()
                        });
                if let Some((container, slot)) =
                    destination.filter(|_| config.item_is_allowed(&template.name))
                {
                    let random = super::drops::crystal_random_drop_stats(
                        &template,
                        swing.now_ms(),
                        swing.object_id,
                        0,
                    );
                    let max_dura = template.durability.saturating_add(random.durability_bonus);
                    let item = add_or_increment_item_with_random_metadata(
                        world,
                        container,
                        &key,
                        &template.name,
                        template.tooltip.as_deref().unwrap_or(""),
                        slot,
                        1,
                        u16::from(template.weight),
                        Some(ore.current_dura),
                        Some(max_dura),
                        random.added_attack,
                        random.added_defence,
                        random.added_stats,
                        random.cursed,
                        template.slots.saturating_add(random.socket_slots),
                    );
                    packets.push(ServerPacket::GainedItem {
                        item: super::items::user_item_from_item_state(&item),
                    });
                }
            }
        }
        let mut inventory = world.resource_mut::<InventoryResource>();
        let weapon = inventory
            .equipment_items
            .iter_mut()
            .find(|w| w.slot == EquipmentSlot::Weapon)
            .ok_or("mining tool disappeared")?;
        weapon.durability_current = weapon
            .durability_current
            .saturating_sub(swing.durability_damage);
        packets.push(ServerPacket::DuraChanged {
            unique_id: swing.tool.unique_id,
            current_dura: weapon.durability_current,
        });
        drop(inventory);
        super::stats::refresh_player_stats(world);
        if swing.durability_damage >= swing.tool.durability {
            if let Some(vitals) = super::components::player_entity(world)
                .and_then(|entity| super::components::entity_player_vitals(world, entity))
            {
                packets.push(ServerPacket::HealthChanged {
                    hp: vitals.hp,
                    mp: vitals.mp,
                });
            }
        }
        if let Err(error) = self.save_active_character() {
            self.restore_active_character_checkpoint(&checkpoint)
                .map_err(|restore| {
                    format!("mining save failed ({error}); rollback failed ({restore})")
                })?;
            return Err(error);
        }
        // These are owner-only asset/vitals packets. Visibility and world
        // objects belong to the shared Zone, not the private session renderer.
        Ok(packets)
    }
}
