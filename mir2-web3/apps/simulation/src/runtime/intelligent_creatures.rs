//! Personal creature ownership and maintenance. Shared actors own movement.
use super::resources::{intelligent_creature_default_rules, Stage5SystemsResource};
use bevy_ecs::prelude::*;
use mir2_game_data::{crystal_monster_manifest, CrystalItemTemplate};
use mir2_protocol::{ClientIntelligentCreature, IntelligentCreatureItemFilter, ServerPacket};

/// These items consume their source slot themselves: reward insertion may reuse
/// that slot, so callers must not consume it a second time.
pub(super) fn use_creature_special_item(
    world: &mut World,
    template: &CrystalItemTemplate,
    item: &super::items::ItemState,
    location: super::inventory::UseItemLocation,
) -> Option<(bool, Vec<ServerPacket>)> {
    if template.item_type != 36 || !matches!(template.shape, 21 | 25 | 26 | 28) {
        return None;
    }
    if matches!(template.shape, 21 | 25) {
        return Some(use_creature_reward_box(world, template, item, location));
    }
    use super::buffs::{apply_or_stack_duration_buff, client_buff_packet_for_state, BuffState};
    use super::crystal_compat::{CRYSTAL_STAT_BAG_WEIGHT, CRYSTAL_STAT_LUCK};
    let tick = super::session::runtime_tick(world);
    let wonder_drug = template.shape == 26;
    if wonder_drug
        && world
            .resource::<super::resources::BuffResource>()
            .buffs
            .iter()
            .any(|buff| buff.key == "wonder-drug" && !buff.real_time_expired())
    {
        return Some((
            false,
            vec![super::packets::system_message_key(
                world,
                "server.WonderDrugActive",
            )],
        ));
    }
    let (key, name, stats) = if wonder_drug {
        ("wonder-drug", "WonderDrug", item.added_stats.clone())
    } else {
        let weight =
            super::items::crystal_item_stat_value(template, CRYSTAL_STAT_LUCK).saturating_add(
                super::items::crystal_item_added_stat_value(item, CRYSTAL_STAT_LUCK),
            );
        (
            "knapsack",
            "Knapsack",
            vec![mir2_protocol::UserItemStat {
                stat: CRYSTAL_STAT_BAG_WEIGHT,
                value: weight,
            }],
        )
    };
    let applied = apply_or_stack_duration_buff(
        world,
        BuffState {
            real_time_duration: Some(super::buffs::RealTimeBuffDuration::new(u64::from(template.durability).saturating_mul(60_000))),
            key: key.into(),
            name: name.into(),
            description: String::new(),
            expires_at_tick: tick.saturating_add(u64::from(template.durability).saturating_mul(60)),
            attack_bonus: 0,
            defence_bonus: 0,
            stats,
        },
    );
    super::inventory::consume_item_at_use_location(world, location);
    super::stats::refresh_player_stats(world);
    Some((
        true,
        client_buff_packet_for_state(world, &applied)
            .into_iter()
            .collect(),
    ))
}

pub(super) fn use_creature_template(
    world: &mut World,
    template: &CrystalItemTemplate,
) -> Option<(bool, Vec<ServerPacket>)> {
    if template.item_type != 36 {
        return None;
    }
    if (0..=14).contains(&template.shape) {
        let pet_type = template.shape as u8;
        let existing = &world
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .intelligent_creatures;
        if existing.len() >= 10 || existing.iter().any(|pet| pet.pet_type == pet_type) {
            return Some((
                false,
                vec![super::packets::system_message_key(
                    world,
                    "server.AlreadyHaveCreature",
                )],
            ));
        }
        let custom_name = crystal_monster_manifest()
            .monsters
            .into_iter()
            .find(|m| m.ai == 64 && m.effect == pet_type)
            .map(|m| m.name)
            .unwrap_or_else(|| {
                [
                    "BabyPig",
                    "Chick",
                    "Kitten",
                    "BabySkeleton",
                    "Baekdon",
                    "Wimaen",
                    "BlackKitten",
                    "BabyDragon",
                    "OlympicFlame",
                    "BabySnowMan",
                    "Frog",
                    "BabyMonkey",
                    "AngryBird",
                    "Foxey",
                    "MedicalRat",
                ][usize::from(pet_type)]
                .to_owned()
            });
        let creature = ClientIntelligentCreature {
            pet_type,
            slot_index: existing.len() as i32,
            icon: 500 + i32::from(pet_type),
            custom_name,
            fullness: 7500,
            expire_binary_datetime: if template.effect == 0 {
                0
            } else {
                super::inventory::future_binary_datetime_minutes(
                    u64::from(template.effect) * 24 * 60,
                )
            },
            blackstone_time: 0,
            maintain_food_time: 0,
            pet_mode: 1,
            creature_rules: intelligent_creature_default_rules(pet_type),
            pickup_grade: 0,
            filter: IntelligentCreatureItemFilter {
                pet_pickup_all: true,
                pet_pickup_gold: false,
                pet_pickup_weapons: false,
                pet_pickup_armours: false,
                pet_pickup_helmets: false,
                pet_pickup_boots: false,
                pet_pickup_belts: false,
                pet_pickup_accessories: false,
                pet_pickup_others: false,
            },
        };
        world
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .intelligent_creatures
            .push(creature.clone());
        return Some((
            true,
            vec![
                ServerPacket::NewIntelligentCreature { creature },
                super::packets::stage5_intelligent_creature_list_packet(world),
            ],
        ));
    }
    match template.shape {
        20 => Some((true, vec![ServerPacket::IntelligentCreatureEnableRename])),
        22..=24 => {
            let mut state = world.resource_mut::<Stage5SystemsResource>();
            let active = state
                .stage5_systems
                .active_intelligent_creature()
                .map(|p| p.pet_type);
            if let Some(pet) = state
                .stage5_systems
                .intelligent_creatures
                .iter_mut()
                .find(|p| Some(p.pet_type) == active)
            {
                match template.shape {
                    22 => pet.maintain_food_time = i64::from(template.effect) * 3600,
                    23 => {
                        pet.fullness = pet
                            .fullness
                            .saturating_add(i32::from(template.effect) * 100)
                            .min(10000)
                    }
                    24 if pet.fullness == 0 => pet.fullness = 100,
                    _ => {}
                }
            }
            drop(state);
            Some((
                true,
                vec![super::packets::stage5_intelligent_creature_list_packet(
                    world,
                )],
            ))
        }
        _ => Some((false, Vec::new())),
    }
}

#[derive(Resource, Default)]
struct CreatureUpdates(bool);

pub(super) fn request_updates(world: &mut World, update: bool) -> Vec<ServerPacket> {
    world.insert_resource(CreatureUpdates(update));
    if update {
        vec![super::packets::stage5_intelligent_creature_list_packet(
            world,
        )]
    } else {
        vec![]
    }
}

#[derive(Resource, Default)]
pub(super) struct CreatureClock {
    last_ms: Option<u64>,
}

fn wall_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

pub(super) fn maintenance(world: &mut World, packets: &mut Vec<ServerPacket>) {
    maintenance_at(
        world,
        wall_ms(),
        super::inventory::current_binary_datetime(),
        packets,
    );
}

pub(super) fn maintenance_at(
    world: &mut World,
    now_ms: u64,
    now_binary: i64,
    packets: &mut Vec<ServerPacket>,
) {
    if !super::resources::is_in_world(world) {
        return;
    }
    if !world.contains_resource::<CreatureClock>() {
        world.insert_resource(CreatureClock::default());
    }
    let mut clock = world.resource_mut::<CreatureClock>();
    let due = clock
        .last_ms
        .is_some_and(|last| now_ms.saturating_sub(last) >= 1000);
    if clock.last_ms.is_none() || due {
        clock.last_ms = Some(now_ms);
    }
    drop(clock);
    if !due {
        return;
    }
    let prohibited = super::map::current_map_disallows_intelligent_creatures(world);
    let updates_requested = world
        .get_resource::<CreatureUpdates>()
        .is_some_and(|updates| updates.0);
    let mut lifecycle_changed = false;
    let mut changed = false;
    let mut produce_stone = false;
    {
        let mut resource = world.resource_mut::<Stage5SystemsResource>();
        let state = &mut resource.stage5_systems;
        let previous_len = state.intelligent_creatures.len();
        state.intelligent_creatures.retain(|pet| {
            pet.expire_binary_datetime == 0
                || super::inventory::binary_datetime_ticks(pet.expire_binary_datetime)
                    >= super::inventory::binary_datetime_ticks(now_binary)
        });
        if state.intelligent_creatures.len() != previous_len {
            lifecycle_changed = true;
            changed = true;
            for (slot, pet) in state.intelligent_creatures.iter_mut().enumerate() {
                pet.slot_index = slot as i32;
            }
        }
        if prohibited || state.active_intelligent_creature().is_none() {
            if state.summoned_intelligent_creature_type != Some(99) {
                state.summoned_intelligent_creature_type = Some(99);
                lifecycle_changed = true;
                changed = true;
            }
        }
        let active = state.active_intelligent_creature().map(|pet| pet.pet_type);
        if let Some(pet) = state
            .intelligent_creatures
            .iter_mut()
            .find(|pet| Some(pet.pet_type) == active)
        {
            if pet.maintain_food_time > 0 {
                pet.maintain_food_time -= 1;
                changed = true;
            }
            if pet.maintain_food_time == 0 && pet.fullness > 0 {
                pet.fullness -= 1;
                changed = true;
            }
            if pet.creature_rules.can_produce_blackstone {
                pet.blackstone_time = pet.blackstone_time.max(0).saturating_add(1);
                produce_stone = pet.blackstone_time >= 10800;
                changed = true;
            }
        }
    }
    if produce_stone {
        if let Some(mut reward) = produce_blackstone(world) {
            packets.append(&mut reward);
            let mut state = world.resource_mut::<Stage5SystemsResource>();
            let active = state
                .stage5_systems
                .active_intelligent_creature()
                .map(|p| p.pet_type);
            if let Some(pet) = state
                .stage5_systems
                .intelligent_creatures
                .iter_mut()
                .find(|p| Some(p.pet_type) == active)
            {
                pet.blackstone_time = 0;
            }
        }
    }
    if lifecycle_changed || (changed && updates_requested) {
        packets.push(super::packets::stage5_intelligent_creature_list_packet(
            world,
        ));
    }
}

fn produce_blackstone(world: &mut World) -> Option<Vec<ServerPacket>> {
    use super::inventory::*;
    use super::items::*;
    use crate::config::{ItemContainer, Stage5MailMessage};
    let template = mir2_game_data::crystal_item_by_name("BlackCreatureStone")?;
    let key = crystal_item_key_for_template(&template);
    let weight = u32::from(current_weight(
        world.resource::<super::resources::InventoryResource>(),
    ));
    let weight_fits = weight + u32::from(template.weight)
        <= super::stats::player_stats(world).bag_weight().max(0) as u32;
    if weight_fits
        && can_gain_item_quantity(
            world.resource::<super::resources::InventoryResource>(),
            ItemContainer::Bag1,
            &key,
            1,
        )
    {
        let item = add_or_increment_item(
            world,
            ItemContainer::Bag1,
            &key,
            &template.name,
            "",
            0,
            1,
            u16::from(template.weight),
        );
        return Some(vec![ServerPacket::GainedItem {
            item: user_item_from_item_state(&item),
        }]);
    }
    let to = world
        .resource::<super::resources::SessionResource>()
        .selected_character
        .as_ref()?
        .name
        .clone();
    let next_id = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .mail
        .iter()
        .map(|m| m.id)
        .max()
        .unwrap_or(0)
        .checked_add(1)?;
    let mut item = embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
    normalize_fresh_item_tree_unique_ids(
        world.resource::<super::resources::InventoryResource>(),
        &mut item,
        &[],
    );
    let item_json = serde_json::to_string(&item).ok()?;
    world
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .mail
        .push(Stage5MailMessage {
            id: next_id,
            delivery_nonce: crate::new_stage5_mail_delivery_nonce(),
            from: "BlackStone".into(),
            to,
            subject: "BlackStone".into(),
            body: "Your creature produced a BlackStone while your inventory was full.".into(),
            gold: 0,
            items: vec![key],
            item_states_json: vec![item_json],
            opened: false,
            locked: false,
            claimed: false,
            deleted: false,
        });
    Some(vec![super::packets::stage5_receive_mail_packet(world)])
}

impl crate::SimulationSession {
    pub fn shared_intelligent_creature_map_allowed(&self) -> bool {
        !super::map::current_map_disallows_intelligent_creatures(self.app.world())
    }
    pub fn dismiss_unspawned_shared_intelligent_creature(&mut self) -> Vec<ServerPacket> {
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .summoned_intelligent_creature_type = Some(99);
        vec![super::packets::stage5_intelligent_creature_list_packet(
            self.app.world(),
        )]
    }

    /// Persist the wallet and its receipt together before the Zone removes its retry record.
    pub fn commit_shared_intelligent_creature_operation(
        &mut self,
        pet_type: u8,
        operation_id: &str,
    ) -> Result<Vec<ServerPacket>, String> {
        let checkpoint = self
            .active_character_checkpoint()
            .ok_or("creature operation requires active character")?;
        let packets = self.record_shared_intelligent_creature_operation(pet_type, operation_id);
        if let Err(error) = self.save_active_character() {
            self.restore_active_character_checkpoint(&checkpoint)?;
            return Err(error);
        }
        Ok(packets)
    }

    pub fn record_shared_intelligent_creature_operation(
        &mut self,
        pet_type: u8,
        operation_id: &str,
    ) -> Vec<ServerPacket> {
        let mut resource = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        let state = &mut resource.stage5_systems;
        // Called only for a Zone-authorized attack. A later dismiss must not erase it.
        if operation_id.is_empty()
            || pet_type > 14
            || !state
                .intelligent_creature_operation_receipts
                .insert(operation_id.to_owned())
        {
            return Vec::new();
        }
        state.intelligent_creature_operations =
            state.intelligent_creature_operations.saturating_add(1);
        if state.intelligent_creature_operations >= 1000 {
            state.intelligent_creature_operations -= 1000;
            state.intelligent_creature_pearls =
                state.intelligent_creature_pearls.max(0).saturating_add(1);
        }
        drop(resource);
        vec![super::packets::stage5_intelligent_creature_list_packet(
            self.app.world(),
        )]
    }
}

/// Crystal chooses the smallest independently sampled score (first wins ties),
/// rather than using the chance column as roulette weights.
pub(super) fn select_creature_box_reward(
    table_key: &str,
    drop_rate: f32,
    mut draw: impl FnMut(u32) -> u32,
) -> Option<CrystalItemTemplate> {
    let table = mir2_game_data::crystal_drop_table_by_key(table_key)?;
    let mut rows = table
        .sections
        .into_iter()
        .flat_map(|s| s.entries)
        .filter_map(|entry| {
            let chance = entry.chance_denominator.filter(|chance| *chance > 0)?;
            Some((
                mir2_game_data::crystal_item_by_name(&entry.item_name)?,
                chance,
            ))
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|(item, _)| item.item_type);
    let mut selected = None;
    let mut best = u32::MAX;
    for (item, chance) in rows {
        let score = ((draw(chance) as f32 / drop_rate) as u32).max(1);
        if score < best {
            best = score;
            selected = Some(item);
        }
    }
    selected
}

fn creature_reward_draw(upper: u32) -> u32 {
    use rand_core::RngCore;
    // Rejection sampling avoids modulo bias for chances such as 10 or 300.
    let threshold = upper.wrapping_neg() % upper;
    loop {
        let value = rand_core::OsRng.next_u32();
        if value >= threshold {
            return value % upper;
        }
    }
}

pub(super) fn apply_dynamic_wonder_drug(
    item: &mut super::items::ItemState,
    effect: u8,
    box_type: u8,
) {
    use super::crystal_compat::*;
    let tier = usize::from(box_type.min(2));
    let definition = match effect {
        0 => (100, [5, 10, 20]),  // ExpRatePercent
        1 => (101, [10, 20, 50]), // ItemDropRatePercent
        2 => (CRYSTAL_STAT_HP, [50, 100, 200]),
        3 => (CRYSTAL_STAT_MP, [50, 100, 200]),
        4 => (CRYSTAL_STAT_MAX_AC, [1, 3, 5]),
        5 => (CRYSTAL_STAT_MAX_MAC, [1, 3, 5]),
        6 => (CRYSTAL_STAT_ATTACK_SPEED, [2, 3, 4]),
        _ => return,
    };
    item.durability_current = Some(1);
    item.added_stats = vec![mir2_protocol::UserItemStat {
        stat: definition.0,
        value: definition.1[tier],
    }];
}

fn use_creature_reward_box(
    world: &mut World,
    template: &CrystalItemTemplate,
    source: &super::items::ItemState,
    location: super::inventory::UseItemLocation,
) -> (bool, Vec<ServerPacket>) {
    use super::inventory::*;
    use super::items::*;
    use super::resources::InventoryResource;
    use crate::ItemContainer;
    use rand_core::RngCore;
    let mut staged = world.resource::<InventoryResource>().clone();
    let (items, index) = match location {
        UseItemLocation::Inventory(index) => (&mut staged.inventory_items, index),
        UseItemLocation::Belt(index) => (&mut staged.belt_items, index),
    };
    let Some(current) = items.get_mut(index) else {
        return (false, vec![]);
    };
    if item_unique_id(current) != item_unique_id(source) {
        return (false, vec![]);
    }
    if current.quantity > 1 {
        current.quantity -= 1;
    } else {
        items.remove(index);
    }
    let strongbox = template.shape == 25;
    let reward = select_creature_box_reward(
        if strongbox {
            "00Strongbox"
        } else {
            "00Blackstone"
        },
        mir2_game_data::crystal_creature_settings().drop_rate,
        creature_reward_draw,
    );
    let Some(reward) = reward else {
        *world.resource_mut::<InventoryResource>() = staged;
        return (
            true,
            if strongbox {
                vec![super::packets::system_message_key(
                    world,
                    "server.NothingFound",
                )]
            } else {
                vec![]
            },
        );
    };
    let reward_key = crystal_item_key_for_template(&reward);
    // Crystal's final AddItem fallback permits any empty belt slot after bags.
    let slot = crystal_empty_add_item_slots(&staged, ItemContainer::Bag1, &reward_key)
        .into_iter()
        .chain(empty_slots_for_inventory_container(
            &staged.belt_items,
            ItemContainer::Belt,
            staged.inventory_capacity,
        ))
        .next();
    let Some((container, slot)) = slot else {
        *world.resource_mut::<InventoryResource>() = staged;
        return (
            true,
            vec![super::packets::system_message_key(
                world,
                "server.NoMoreSpace",
            )],
        );
    };
    let mut item = embedded_item_state_from_template(&reward, container, slot);
    if strongbox && reward.item_type == 36 && reward.shape == 26 {
        apply_dynamic_wonder_drug(&mut item, reward.effect, template.effect);
    } else {
        let seed = rand_core::OsRng.next_u64();
        let random = super::drops::crystal_random_drop_stats(&reward, seed, seed as u32, 0);
        item.durability_max = item
            .durability_max
            .map(|max| max.saturating_add(random.durability_bonus));
        item.durability_current = item.durability_current.map(|max| {
            super::drops::crystal_drop_item_current_durability(max, seed, seed as u32, 0)
                .saturating_add(random.durability_bonus)
        });
        item.added_attack = random.added_attack;
        item.added_defence = random.added_defence;
        item.added_stats = random.added_stats;
        item.cursed = random.cursed;
        item.socket_slots = random.socket_slots;
    }
    item.unique_id =
        allocate_item_unique_id_avoiding(&staged, container, slot, std::slice::from_ref(source));
    let Ok(carrier) = try_user_item_from_item_state(&item) else {
        // Invalid server content cannot consume the box without a deliverable item.
        return (false, vec![]);
    };
    if container == ItemContainer::Belt {
        staged.belt_items.push(item);
    } else {
        staged.inventory_items.push(item);
    }
    // One state replacement commits source consumption plus exact reward custody.
    // Crystal shape 25 falls through a second consume; intentionally fix that loss.
    *world.resource_mut::<InventoryResource>() = staged;
    (true, vec![ServerPacket::GainedItem { item: carrier }])
}
