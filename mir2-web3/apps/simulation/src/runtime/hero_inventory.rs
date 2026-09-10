#[cfg(test)]
#[path = "hero_legacy_migration_tests.rs"]
mod legacy_migration_tests;
#[path = "hero_merge.rs"]
mod merge;
pub(super) use merge::merge_item;
// Hero inventory indices are raw Crystal indices: 0/1 belt, 2..capacity bag.
// Equipment is separate custody. Never infer equipped items from the backpack.
use super::items::{try_user_item_from_item_state, ItemState};
use super::resources::HeroInventoryResource;
use crate::config::CharacterSaveRecord;
use mir2_protocol::UserItem;
use std::collections::BTreeSet;

pub(super) fn restored_hero_inventory(
    save: &CharacterSaveRecord,
    items: Vec<ItemState>,
    equipment: Vec<ItemState>,
) -> Result<HeroInventoryResource, String> {
    let (capacity, legacy_40) = match save.hero_inventory_capacity {
        Some(40) if save.hero_inventory_legacy_40 => (40, true),
        Some(n @ (10 | 18 | 26 | 34 | 42)) if !save.hero_inventory_legacy_40 => (n, false),
        Some(_) => return Err("invalid saved Hero inventory capacity".into()),
        None if save.hero_inventory_legacy_40 => {
            return Err("legacy Hero capacity marker requires capacity 40".into())
        }
        None if !items.is_empty() => (40, true),
        None => (10, false),
    };
    let resource = HeroInventoryResource {
        items,
        equipment,
        capacity,
        legacy_40,
        saved_vitals: save.hero_vitals,
        registry_attachment: save.hero_registry_attachment,
    };
    validate_hero_custody(&resource)?;
    Ok(resource)
}
pub(super) fn collect_item_ids(item: &UserItem, ids: &mut BTreeSet<u64>) -> Result<(), String> {
    if item.unique_id == 0 || !ids.insert(item.unique_id) {
        return Err("duplicate or zero Hero custody item UID".into());
    }
    for child in item.slots.iter().flatten() {
        collect_item_ids(child, ids)?;
    }
    Ok(())
}
pub(super) fn validate_hero_custody(
    resource: &HeroInventoryResource,
) -> Result<BTreeSet<u64>, String> {
    let mut ids = BTreeSet::new();
    for (items, limit) in [
        (&resource.items, resource.capacity),
        (&resource.equipment, 14),
    ] {
        let mut slots = BTreeSet::new();
        for item in items {
            if item.slot >= limit || !slots.insert(item.slot) {
                return Err("invalid or duplicate Hero custody slot".into());
            }
            let carrier = try_user_item_from_item_state(item)
                .map_err(|e| format!("invalid Hero item carrier: {e:?}"))?;
            collect_item_ids(&carrier, &mut ids)?;
        }
    }
    if resource.saved_vitals.is_some_and(|v| v.hp < 0 || v.mp < 0) {
        return Err("negative saved Hero pools".into());
    }
    Ok(ids)
}

pub(super) fn reject_cross_custody_ids(
    item: &ItemState,
    hero_ids: &BTreeSet<u64>,
) -> Result<(), String> {
    fn overlaps(item: &UserItem, ids: &BTreeSet<u64>) -> bool {
        ids.contains(&item.unique_id)
            || item
                .slots
                .iter()
                .flatten()
                .any(|child| overlaps(child, ids))
    }
    if hero_ids.is_empty() {
        return Ok(());
    }
    let carrier = try_user_item_from_item_state(item)
        .map_err(|e| format!("invalid external Hero custody carrier: {e:?}"))?;
    if overlaps(&carrier, hero_ids) {
        return Err("Hero item UID collides with personal custody".into());
    }
    Ok(())
}

pub(super) fn move_item(
    world: &mut bevy_ecs::world::World,
    from: i32,
    to: i32,
) -> Vec<mir2_protocol::ServerPacket> {
    use mir2_protocol::{MirGridType, ServerPacket};
    let mut success = false;
    let ready = world
        .resource::<super::resources::Stage5SystemsResource>()
        .stage5_systems
        .hero
        .as_ref()
        .is_some_and(|hero| hero.spawned)
        && super::components::hero_entity(world).is_some();
    if ready {
        let mut state = world.resource_mut::<HeroInventoryResource>();
        if from >= 0
            && to >= 0
            && from < i32::from(state.capacity)
            && to < i32::from(state.capacity)
        {
            if let Some(index) = state
                .items
                .iter()
                .position(|item| i32::from(item.slot) == from)
            {
                if let Some(other) = state
                    .items
                    .iter()
                    .position(|item| i32::from(item.slot) == to)
                {
                    state.items[other].slot = from as u8;
                }
                state.items[index].slot = to as u8;
                success = true;
            }
        }
    }
    vec![ServerPacket::MoveItem {
        grid: MirGridType::HeroInventory,
        from,
        to,
        success,
    }]
}

/// UserItem.Weight: amulets and bait weigh once for the whole stack.
pub(super) fn item_weight(item: &ItemState) -> u32 {
    use super::{crystal_compat::*, items::crystal_item_template_for_item_key};
    match crystal_item_template_for_item_key(&item.key) {
        Some(template)
            if matches!(
                template.item_type,
                CRYSTAL_ITEM_TYPE_AMULET | CRYSTAL_ITEM_TYPE_BAIT
            ) =>
        {
            u32::from(template.weight)
        }
        Some(template) => u32::from(template.weight).saturating_mul(item.quantity),
        None => item.total_weight(),
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct HeroAppearance {
    pub weapon: i16,
    pub armour: i16,
    pub weapon_effect: i16,
    pub wing_effect: u8,
    pub light: u8,
    pub mount_type: i16,
}
pub(super) fn appearance(world: &bevy_ecs::world::World) -> HeroAppearance {
    use super::{crystal_compat::*, items::crystal_item_template_for_item_key};
    let mut looks = HeroAppearance {
        weapon: -1,
        armour: 0,
        weapon_effect: 0,
        wing_effect: 0,
        light: 0,
        mount_type: -1,
    };
    let Some(hero) = world
        .resource::<super::resources::Stage5SystemsResource>()
        .stage5_systems
        .hero
        .as_ref()
    else {
        return looks;
    };
    let state = world.resource::<HeroInventoryResource>();
    for slot in 0..14 {
        let Some(item) = state.equipment.iter().find(|item| item.slot == slot) else {
            continue;
        };
        let Some(base) = crystal_item_template_for_item_key(&item.key) else {
            continue;
        };
        if base.durability > 0 && item.durability_current.unwrap_or_default() == 0 {
            continue;
        }
        let real = mir2_game_data::crystal_real_item_for_player(&base, hero.level, hero.class);
        looks.light = looks.light.max(real.light);
        match real.item_type {
            CRYSTAL_ITEM_TYPE_WEAPON => {
                looks.weapon = real.shape;
                looks.weapon_effect = i16::from(real.effect);
            }
            CRYSTAL_ITEM_TYPE_ARMOUR => {
                looks.armour = real.shape;
                looks.wing_effect = real.effect;
            }
            CRYSTAL_ITEM_TYPE_MOUNT => looks.mount_type = real.shape,
            _ => {}
        }
    }
    looks
}

pub(super) fn expand_capacity(hero: &mut HeroInventoryResource) -> bool {
    if hero.capacity >= 42 {
        return false;
    }
    hero.capacity = hero.capacity.saturating_add(8).min(42);
    hero.legacy_40 = false;
    true
}

fn ready(world: &bevy_ecs::world::World, require_alive: bool) -> bool {
    let spawned = world
        .resource::<super::resources::Stage5SystemsResource>()
        .stage5_systems
        .hero
        .as_ref()
        .is_some_and(|hero| hero.spawned);
    spawned
        && super::components::hero_entity(world).is_some_and(|entity| {
            !require_alive
                || world
                    .get::<super::components::PlayerVitals>(entity)
                    .is_some_and(|v| v.hp > 0)
        })
}

pub(super) fn equip_item(
    world: &mut bevy_ecs::world::World,
    unique_id: u64,
    to: i32,
) -> Vec<mir2_protocol::ServerPacket> {
    use super::{
        components::current_character_index, crystal_compat::*, items::*,
        resources::PlayerPermissionResource,
    };
    use mir2_protocol::{MirGridType, ServerPacket};
    let failure = ServerPacket::EquipItem {
        grid: MirGridType::HeroInventory,
        unique_id,
        to,
        success: false,
    };
    let Some(slot) = super::equipment::equipment_slot_from_index(to) else {
        return vec![failure];
    };
    if !ready(world, true) {
        return vec![failure];
    }
    let mut staged = world.resource::<HeroInventoryResource>().clone();
    if validate_hero_custody(&staged).is_err() {
        return vec![failure];
    }
    let Some(index) = staged
        .items
        .iter()
        .position(|item| item.unique_id == unique_id)
    else {
        return vec![failure];
    };
    let mut incoming = staged.items[index].clone();
    if super::hero_ai::hero_mount::riding(world) && slot != crate::EquipmentSlot::Torch {return vec![failure];}
    if !item_state_can_equip_to_slot(&incoming, slot) {
        return vec![failure];
    }
    let hero = world
        .resource::<super::resources::Stage5SystemsResource>()
        .stage5_systems
        .hero
        .as_ref()
        .unwrap();
    if crystal_item_template_for_item_key(&incoming.key)
        .as_ref()
        .is_some_and(|template| {
            crystal_hero_item_requirement_rejected(
                world,
                hero.level,
                hero.class,
                hero.gender,
                template,
            )
        })
    {
        return vec![failure];
    }
    let owner = current_character_index(world).unwrap_or(-1);
    let bound = item_state_soul_bound_id(&incoming);
    if bound != -1 && bound != owner {
        return vec![failure];
    }
    let old_index = staged
        .equipment
        .iter()
        .position(|item| i32::from(item.slot) == to);
    let replaced = old_index.map(|index| &staged.equipment[index]);
    let unlocked = world.resource::<PlayerPermissionResource>().unlock_curse;
    if replaced.is_some_and(|item| {
        (item.cursed && !unlocked)
            || item_has_crystal_or_rental_bind_flag(item, CRYSTAL_BIND_DONT_STORE)
            || try_user_item_from_item_state(item).is_ok_and(|v| v.wedding_ring != -1)
    }) {
        return vec![failure];
    }
    let replaced_cursed = replaced.is_some_and(|item| item.cursed);
    let source_slot = incoming.slot;
    let source_container = incoming.container;
    let hand = matches!(to, 0 | 3);
    let weight = staged
        .equipment
        .iter()
        .filter(|item| i32::from(item.slot) != to && matches!(item.slot, 0 | 3) == hand)
        .map(item_weight)
        .fold(0u32, u32::saturating_add)
        .saturating_add(item_weight(&incoming));
    let limit = super::hero_ai::hero_authoritative_stat(
        world,
        if hand {
            CRYSTAL_STAT_HAND_WEIGHT
        } else {
            CRYSTAL_STAT_WEAR_WEIGHT
        },
    )
    .max(0) as u32;
    if weight > limit {
        return vec![failure];
    }
    let mut packets = Vec::new();
    if crystal_item_needs_identify(&incoming.key) && !item_state_identified(&incoming) {
        incoming.identified = Some(true);
        packets.push(ServerPacket::RefreshItem {
            item: user_item_from_item_state(&incoming),
        });
    }
    if crystal_item_has_bind_flag(&incoming.key, CRYSTAL_BIND_ON_EQUIP) && bound == -1 && owner >= 0
    {
        incoming.soul_bound_id = Some(owner);
        packets.push(ServerPacket::RefreshItem {
            item: user_item_from_item_state(&incoming),
        });
    }
    staged.items.remove(index);
    if let Some(old_index) = old_index {
        let mut old = staged.equipment.remove(old_index);
        old.slot = source_slot;
        old.container = source_container;
        staged.items.push(old);
    }
    incoming.slot = to as u8;
    staged.equipment.push(incoming);
    if validate_hero_custody(&staged).is_err() {
        return vec![failure];
    }
    *world.resource_mut::<HeroInventoryResource>() = staged;
    if replaced_cursed {
        world
            .resource_mut::<PlayerPermissionResource>()
            .unlock_curse = false;
    }
    packets.push(ServerPacket::EquipItem {
        grid: MirGridType::HeroInventory,
        unique_id,
        to,
        success: true,
    });
    packets.extend(refresh_stats_and_packets(world));
    packets
}

pub(super) fn remove_item(
    world: &mut bevy_ecs::world::World,
    unique_id: u64,
    to: i32,
) -> Vec<mir2_protocol::ServerPacket> {
    use super::{crystal_compat::CRYSTAL_STAT_BAG_WEIGHT, resources::PlayerPermissionResource};
    use mir2_protocol::{MirGridType, ServerPacket};
    let failure = ServerPacket::RemoveItem {
        grid: MirGridType::HeroInventory,
        unique_id,
        to,
        success: false,
    };
    if !ready(world, false) {
        return vec![failure];
    }
    let mut staged = world.resource::<HeroInventoryResource>().clone();
    if validate_hero_custody(&staged).is_err()
        || to < 0
        || to >= i32::from(staged.capacity)
        || staged.items.iter().any(|item| i32::from(item.slot) == to)
    {
        return vec![failure];
    }
    let Some(index) = staged
        .equipment
        .iter()
        .position(|item| item.unique_id == unique_id)
    else {
        return vec![failure];
    };
    let mut item = staged.equipment[index].clone();
    if item.cursed && !world.resource::<PlayerPermissionResource>().unlock_curse
        || try_user_item_from_item_state(&item).is_ok_and(|v| v.wedding_ring != -1)
    {
        return vec![failure];
    }
    let weight = staged
        .items
        .iter()
        .map(item_weight)
        .fold(0u32, u32::saturating_add)
        .saturating_add(item_weight(&item));
    if weight
        > super::hero_ai::hero_authoritative_stat(world, CRYSTAL_STAT_BAG_WEIGHT).max(0) as u32
    {
        return vec![failure];
    }
    let cursed = item.cursed;
    item.slot = to as u8;
    staged.equipment.remove(index);
    staged.items.push(item);
    if validate_hero_custody(&staged).is_err() {
        return vec![failure];
    }
    *world.resource_mut::<HeroInventoryResource>() = staged;
    if cursed {
        world
            .resource_mut::<PlayerPermissionResource>()
            .unlock_curse = false;
    }
    let mut packets = vec![ServerPacket::RemoveItem {
        grid: MirGridType::HeroInventory,
        unique_id,
        to,
        success: true,
    }];
    packets.extend(refresh_stats_and_packets(world));
    packets
}

fn refresh_stats_and_packets(
    world: &mut bevy_ecs::world::World,
) -> Vec<mir2_protocol::ServerPacket> {
    use super::{components::*, crystal_compat::*};
    let max_hp = super::hero_ai::hero_authoritative_stat(world, CRYSTAL_STAT_HP).max(0);
    let max_mp = super::hero_ai::hero_authoritative_stat(world, CRYSTAL_STAT_MP).max(0);
    let looks = appearance(world);
    let mut packets = super::hero_ai::hero_mount::refresh(world);
    if let Some(entity) = hero_entity(world) {
        if let Some(mut body) = world.get_mut::<CharacterBody>(entity) {
            body.weapon_shape = u16::try_from(looks.weapon).ok();
            body.armour_shape = u16::try_from(looks.armour).ok();
        }
        if let Some(id) = world.get::<ObjectId>(entity) {
            packets.push(mir2_protocol::ServerPacket::PlayerUpdate {
                object_id: id.0,
                light: looks.light,
                weapon: looks.weapon,
                weapon_effect: looks.weapon_effect,
                armour: looks.armour,
                wing_effect: looks.wing_effect,
            });
        }
        if let Some(mut vitals) = world.get_mut::<PlayerVitals>(entity) {
            vitals.max_hp = max_hp;
            vitals.max_mp = max_mp;
            vitals.hp = vitals.hp.min(max_hp);
            vitals.mp = vitals.mp.min(max_mp);
            packets.push(mir2_protocol::ServerPacket::HeroHealthChanged {
                hp: vitals.hp,
                mp: vitals.mp,
            });
        }
    }
    packets.extend(super::packets::hero_bootstrap_packets(world));
    packets
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ItemContainer, SimulationConfig, SimulationSession};
    use mir2_protocol::{ClientPacket, UserItemStat};
    fn item(uid: u64, slot: u8) -> ItemState {
        let template = mir2_game_data::crystal_item_by_index(1).unwrap();
        let mut item = super::super::items::embedded_item_state_from_template(
            &template,
            ItemContainer::Bag1,
            slot,
        );
        item.unique_id = uid;
        item.added_stats = vec![UserItemStat { stat: 12, value: 7 }];
        item
    }
    fn save() -> CharacterSaveRecord {
        CharacterSaveRecord::new(SimulationConfig::default().default_character)
    }
    #[test]
    fn hero_custody_legacy_capacity_preserves_last_slot_without_equipment_promotion() {
        let source = item(88001, 39);
        let restored = restored_hero_inventory(&save(), vec![source.clone()], vec![]).unwrap();
        assert_eq!(restored.capacity, 40);
        assert!(restored.legacy_40);
        assert!(restored.equipment.is_empty());
        assert_eq!(
            serde_json::to_value(&restored.items[0]).unwrap(),
            serde_json::to_value(source).unwrap()
        );
        let fresh = restored_hero_inventory(&save(), vec![], vec![]).unwrap();
        assert_eq!(fresh.capacity, 10);
        assert!(!fresh.legacy_40);
    }
    #[test]
    fn hero_custody_rejects_duplicate_uid_slot_and_out_of_bounds_without_modification() {
        let mut resource = HeroInventoryResource::new();
        resource.items.push(item(88001, 0));
        resource.equipment.push(item(88001, 0));
        let before = serde_json::to_value(&resource.items).unwrap();
        assert!(validate_hero_custody(&resource).is_err());
        assert_eq!(serde_json::to_value(&resource.items).unwrap(), before);
        resource.equipment[0].unique_id = 88002;
        assert!(validate_hero_custody(&resource).is_ok());
        resource.equipment[0].slot = 14;
        assert!(validate_hero_custody(&resource).is_err());
        resource.equipment.clear();
        resource.items.push(item(88003, 0));
        assert!(validate_hero_custody(&resource).is_err());
        resource.items[1].slot = 10;
        assert!(validate_hero_custody(&resource).is_err());
    }
    #[test]
    fn hero_custody_capacities_and_legacy_marker_are_explicit() {
        for capacity in [10, 18, 26, 34, 42] {
            let mut save = save();
            save.hero_inventory_capacity = Some(capacity);
            assert_eq!(
                restored_hero_inventory(&save, vec![], vec![])
                    .unwrap()
                    .capacity,
                capacity
            );
        }
        for capacity in [0, 9, 11, 40, 43, 255] {
            let mut save = save();
            save.hero_inventory_capacity = Some(capacity);
            assert!(restored_hero_inventory(&save, vec![], vec![]).is_err());
        }
        let mut save = save();
        save.hero_inventory_capacity = Some(40);
        save.hero_inventory_legacy_40 = true;
        assert!(restored_hero_inventory(&save, vec![], vec![]).is_ok());
    }
    #[test]
    fn hero_custody_equipment_exact_carrier_survives_save_preflight_relogin() {
        let mut session = SimulationSession::new(SimulationConfig::default());
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        });
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        session
            .app
            .world_mut()
            .resource_mut::<HeroInventoryResource>()
            .equipment
            .push(item(88001, 0));
        let expected = serde_json::to_value(
            &session
                .app
                .world()
                .resource::<HeroInventoryResource>()
                .equipment,
        )
        .unwrap();
        session.handle_packet(ClientPacket::LogOut);
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        let hero = session.app.world().resource::<HeroInventoryResource>();
        assert_eq!(serde_json::to_value(&hero.equipment).unwrap(), expected);
        assert!(hero.items.is_empty());
        assert_eq!(hero.capacity, 10);
    }
    #[test]
    fn hero_custody_save_preflight_rejects_cross_player_uid_collision() {
        let mut session = SimulationSession::new(SimulationConfig::default());
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        });
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        let mut save =
            super::super::save::snapshot_active_character_save(session.app.world()).unwrap();
        let duplicate = session
            .app
            .world()
            .resource::<super::super::resources::InventoryResource>()
            .inventory_items[0]
            .clone();
        save.hero_inventory_items_json = vec![serde_json::to_string(&duplicate).unwrap()];
        assert!(super::super::save::validate_character_save_record(&save).is_err());
    }
    #[test]
    fn hero_custody_ordinary_move_swaps_belt_and_bag_and_rejects_invalid_slots() {
        use mir2_protocol::{MirClass, MirGender, MirGridType, ServerPacket};
        let mut session = SimulationSession::new(SimulationConfig::default());
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        });
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        session.handle_packet(ClientPacket::NewHero {
            name: "Aide".into(),
            gender: MirGender::Female,
            class: MirClass::Taoist,
        });
        session
            .app
            .world_mut()
            .resource_mut::<HeroInventoryResource>()
            .items = vec![item(88001, 0), item(88002, 9)];
        let ack = session.handle_packet(ClientPacket::MoveItem {
            grid: MirGridType::HeroInventory,
            from: 0,
            to: 9,
        });
        assert!(ack.contains(&ServerPacket::MoveItem {
            grid: MirGridType::HeroInventory,
            from: 0,
            to: 9,
            success: true
        }));
        let inventory = &session
            .app
            .world()
            .resource::<HeroInventoryResource>()
            .items;
        assert_eq!(
            inventory
                .iter()
                .map(|v| (v.unique_id, v.slot))
                .collect::<Vec<_>>(),
            vec![(88001, 9), (88002, 0)]
        );
        let before = serde_json::to_value(inventory).unwrap();
        for (from, to) in [(9, 10), (-1, 0), (3, 0)] {
            let ack = session.handle_packet(ClientPacket::MoveItem {
                grid: MirGridType::HeroInventory,
                from,
                to,
            });
            assert!(ack.contains(&ServerPacket::MoveItem {
                grid: MirGridType::HeroInventory,
                from,
                to,
                success: false
            }));
        }
        assert_eq!(
            serde_json::to_value(
                &session
                    .app
                    .world()
                    .resource::<HeroInventoryResource>()
                    .items
            )
            .unwrap(),
            before
        );
    }
    #[test]
    fn hero_custody_backpack_never_contributes_equipment_stats_or_equipment_projection() {
        let mut session = SimulationSession::new(SimulationConfig::default());
        let mut weapon = item(88001, 2);
        weapon.equip_slot = Some(crate::EquipmentSlot::Weapon);
        weapon.durability_current = Some(1);
        session
            .app
            .world_mut()
            .resource_mut::<HeroInventoryResource>()
            .items
            .push(weapon.clone());
        assert_eq!(
            super::super::hero_ai::hero_inventory_crystal_stat_total(session.app.world(), 12),
            0
        );
        assert!(
            super::super::hero_ai::hero_inventory_equipment_slots(session.app.world())
                .iter()
                .all(Option::is_none)
        );
        session
            .app
            .world_mut()
            .resource_mut::<HeroInventoryResource>()
            .items
            .clear();
        weapon.slot = 0;
        session
            .app
            .world_mut()
            .resource_mut::<HeroInventoryResource>()
            .equipment
            .push(weapon);
        assert!(
            super::super::hero_ai::hero_inventory_crystal_stat_total(session.app.world(), 12) > 0
        );
        let equipment = super::super::hero_ai::hero_inventory_equipment_slots(session.app.world());
        assert_eq!(equipment[0].as_ref().unwrap().unique_id, 88001);
        assert_eq!(equipment.iter().filter(|slot| slot.is_some()).count(), 1);
    }
    #[test]
    fn hero_custody_ordinary_equip_swap_and_remove_preserve_exact_uid() {
        use mir2_protocol::{MirClass, MirGender, MirGridType, ServerPacket};
        let mut session = SimulationSession::new(SimulationConfig::default());
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        });
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        session.handle_packet(ClientPacket::NewHero {
            name: "Aide".into(),
            gender: MirGender::Female,
            class: MirClass::Taoist,
        });
        let template = mir2_game_data::crystal_item_by_index(221).unwrap();
        let mut sword = super::super::items::embedded_item_state_from_template(
            &template,
            ItemContainer::Bag1,
            2,
        );
        sword.unique_id = 88001;
        let mut replacement = sword.clone();
        replacement.unique_id = 88002;
        replacement.slot = 3;
        session
            .app
            .world_mut()
            .resource_mut::<HeroInventoryResource>()
            .items = vec![sword, replacement];
        for unique_id in [88001, 88002] {
            let packets = session.handle_packet(ClientPacket::EquipItem {
                grid: MirGridType::HeroInventory,
                unique_id,
                to: 0,
            });
            assert!(
                packets.contains(&ServerPacket::EquipItem {
                    grid: MirGridType::HeroInventory,
                    unique_id,
                    to: 0,
                    success: true
                }),
                "{packets:?}"
            );
        }
        let hero = session.app.world().resource::<HeroInventoryResource>();
        assert_eq!(hero.equipment[0].unique_id, 88002);
        assert_eq!((hero.items[0].unique_id, hero.items[0].slot), (88001, 3));
        let before = serde_json::to_value((&hero.items, &hero.equipment)).unwrap();
        let packets = session.handle_packet(ClientPacket::RemoveItem {
            grid: MirGridType::HeroInventory,
            unique_id: 88002,
            to: 3,
        });
        assert!(packets.contains(&ServerPacket::RemoveItem {
            grid: MirGridType::HeroInventory,
            unique_id: 88002,
            to: 3,
            success: false
        }));
        let hero = session.app.world().resource::<HeroInventoryResource>();
        assert_eq!(
            serde_json::to_value((&hero.items, &hero.equipment)).unwrap(),
            before
        );
        let packets = session.handle_packet(ClientPacket::RemoveItem {
            grid: MirGridType::HeroInventory,
            unique_id: 88002,
            to: 1,
        });
        assert!(packets.contains(&ServerPacket::RemoveItem {
            grid: MirGridType::HeroInventory,
            unique_id: 88002,
            to: 1,
            success: true
        }));
        let hero = session.app.world().resource::<HeroInventoryResource>();
        assert!(hero.equipment.is_empty());
        assert_eq!(
            validate_hero_custody(hero).unwrap(),
            BTreeSet::from([88001, 88002])
        );
    }
    #[test]
    fn hero_custody_bootstrap_uses_source_pools_and_restores_remaining_pools() {
        use mir2_protocol::{MirClass, MirGender, ServerPacket, Spell};
        let mut session = SimulationSession::new(SimulationConfig::default());
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        });
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        session.handle_packet(ClientPacket::NewHero {
            name: "Aide".into(),
            gender: MirGender::Female,
            class: MirClass::Taoist,
        });
        let actor = super::super::components::hero_entity(session.app.world()).unwrap();
        let source = mir2_game_data::crystal_hero_settings().base_stats(MirClass::Taoist);
        let expected_hp = mir2_game_data::calculate_crystal_hero_base_stat(
            source.stats.iter().find(|stat| stat.stat == 12).unwrap(),
            MirClass::Taoist,
            1,
        );
        assert_eq!(
            session
                .app
                .world()
                .get::<super::super::components::PlayerVitals>(actor)
                .unwrap()
                .max_hp,
            expected_hp
        );
        session
            .app
            .world_mut()
            .get_mut::<super::super::components::PlayerVitals>(actor)
            .unwrap()
            .hp = 1;
        session
            .app
            .world_mut()
            .resource_mut::<super::super::resources::Stage5SystemsResource>()
            .stage5_systems
            .hero_learned_magics
            .push(crate::config::Stage5HeroMagicState {
                spell: Spell::Healing,
                level: 1,
                key: 17,
                experience: 12,
            });
        session.handle_packet(ClientPacket::LogOut);
        let packets = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        assert!(packets.iter().any(
            |packet| matches!(packet,ServerPacket::HeroBaseStatsInfo {stats} if stats==source)
        ));
        let info = packets
            .iter()
            .find_map(|packet| match packet {
                ServerPacket::HeroInformation { info } => Some(info),
                _ => None,
            })
            .unwrap();
        assert_eq!(info.hp, 1);
        assert_eq!(info.inventory.as_ref().unwrap().len(), 10);
        assert_eq!(info.equipment.as_ref().unwrap().len(), 14);
        assert_eq!(
            info.max_experience,
            mir2_game_data::crystal_hero_settings().max_experience(1)
        );
        assert_eq!(info.magics.len(), 1);
        assert_eq!(
            (
                info.magics[0].spell,
                info.magics[0].level,
                info.magics[0].key,
                info.magics[0].experience
            ),
            (Spell::Healing, 1, 17, 12)
        );
        let template = mir2_game_data::crystal_magic_by_spell("Healing").unwrap();
        assert_eq!(info.magics[0].icon, template.icon);
        assert_eq!(info.magics[0].base_cost, template.base_cost);
        let snapshot = session.world_snapshot();
        assert_eq!(snapshot.hero_inventory_capacity, 10);
        assert_eq!(snapshot.hero_vitals.unwrap().hp, 1);
        assert_eq!(snapshot.hero_vitals.unwrap().max_hp, expected_hp);
        assert!(snapshot
            .hero_stats
            .iter()
            .any(|stat| stat.stat == 12 && stat.value == expected_hp));
        assert_eq!(snapshot.hero_weights.bag, 0);
        assert_eq!(snapshot.hero_weights.wear, 0);
        assert!(snapshot.hero_equipment_items.is_empty());
        let client = serde_json::to_value(snapshot.client_view()).unwrap();
        assert_eq!(client["heroVitals"]["hp"], 1);
        assert_eq!(client["heroWeights"]["bag"], 0);
    }
    #[test]
    fn hero_custody_transfer_return_reserves_nested_identity_without_renumbering() {
        use super::super::resources::InventoryResource;
        use mir2_protocol::{MirClass, MirGender, ServerPacket};
        let mut session = SimulationSession::new(SimulationConfig::default());
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        });
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        session.handle_packet(ClientPacket::NewHero {
            name: "Aide".into(),
            gender: MirGender::Female,
            class: MirClass::Taoist,
        });
        let source = item(88001, 2);
        let mut expected = try_user_item_from_item_state(&source).unwrap();
        expected.slots = vec![Some(
            try_user_item_from_item_state(&item(88002, 0)).unwrap(),
        )];
        let source = super::super::items::try_item_state_from_user_item(source, &expected).unwrap();
        session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_items = vec![source];
        let packets = session.handle_packet(ClientPacket::TransferHeroItem { from: 2, to: 9 });
        assert!(
            packets.contains(&ServerPacket::TransferHeroItem {
                from: 2,
                to: 9,
                success: true
            }),
            "{packets:?}"
        );
        super::super::item_custody::refresh(session.app.world_mut()).unwrap();
        let inventory = session.app.world().resource::<InventoryResource>();
        for id in [88001, 88002] {
            assert!(super::super::inventory::inventory_unique_id_is_used(
                inventory, id
            ));
        }
        assert_eq!(
            try_user_item_from_item_state(
                &session
                    .app
                    .world()
                    .resource::<HeroInventoryResource>()
                    .items[0]
            )
            .unwrap(),
            expected
        );
        let failed = session.handle_packet(ClientPacket::TakeBackHeroItem { from: 10, to: 0 });
        assert!(failed.contains(&ServerPacket::TakeBackHeroItem {
            from: 10,
            to: 0,
            success: false
        }));
        let packets = session.handle_packet(ClientPacket::TakeBackHeroItem { from: 9, to: 0 });
        assert!(
            packets.contains(&ServerPacket::TakeBackHeroItem {
                from: 9,
                to: 0,
                success: true
            }),
            "{packets:?}"
        );
        let inventory = session.app.world().resource::<InventoryResource>();
        assert_eq!(
            try_user_item_from_item_state(&inventory.inventory_items[0]).unwrap(),
            expected
        );
        assert!(!inventory.reserved_item_unique_ids.contains(&88001));
        assert!(!inventory.reserved_item_unique_ids.contains(&88002));
        assert!(session
            .app
            .world()
            .resource::<HeroInventoryResource>()
            .items
            .is_empty());
    }
    #[test]
    fn hero_custody_capacity_expansion_keeps_existing_slots_and_caps_at_42() {
        let mut hero = HeroInventoryResource::new();
        hero.items.push(item(88001, 9));
        for expected in [18, 26, 34, 42] {
            assert!(expand_capacity(&mut hero));
            assert_eq!(hero.capacity, expected);
        }
        let before = serde_json::to_value(&hero.items).unwrap();
        assert!(!expand_capacity(&mut hero));
        assert_eq!(serde_json::to_value(&hero.items).unwrap(), before);
        hero.capacity = 40;
        hero.legacy_40 = true;
        hero.items[0].slot = 39;
        assert!(expand_capacity(&mut hero));
        assert_eq!(hero.capacity, 42);
        assert!(!hero.legacy_40);
        assert_eq!(hero.items[0].slot, 39);
    }
    #[test]
    fn hero_custody_auto_pot_accepts_source_stats_and_rejects_forged_item_grids() {
        use super::super::resources::Stage5SystemsResource;
        use mir2_protocol::{MirClass, MirGender, MirGridType, ServerPacket};
        let mut session = SimulationSession::new(SimulationConfig::default());
        session.handle_packet(ClientPacket::Login { account_id: "demo".into(), password: "demo".into() });
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        session.handle_packet(ClientPacket::NewHero { name: "Aide".into(), gender: MirGender::Female, class: MirClass::Taoist });
        session.app.world_mut().resource_mut::<Stage5SystemsResource>().stage5_systems.hero.as_mut().unwrap().auto_pot = true;
        for (stat, value) in [(12, 72), (13, 43)] {
            assert!(session.handle_packet(ClientPacket::SetAutoPotValue { stat, value }).contains(&ServerPacket::SetAutoPotValue { stat, value }));
        }
        for stat in [0, 1, 255] {
            assert!(session.handle_packet(ClientPacket::SetAutoPotValue { stat, value: 99 }).is_empty());
        }
        let hero = session.app.world().resource::<Stage5SystemsResource>().stage5_systems.hero.as_ref().unwrap();
        assert_eq!((hero.auto_hp_percent, hero.auto_mp_percent), (72, 43));
        let catalog = mir2_game_data::crystal_item_manifest();
        let potion = catalog.items.iter().find(|item| item.item_type == super::super::crystal_compat::CRYSTAL_ITEM_TYPE_POTION && item.shape <= 1).unwrap().item_index;
        let other = catalog.items.iter().find(|item| item.item_type != super::super::crystal_compat::CRYSTAL_ITEM_TYPE_POTION).unwrap().item_index;
        for grid in [MirGridType::HeroHpItem, MirGridType::HeroMpItem] {
            let packets = session.handle_packet(ClientPacket::SetAutoPotItem { grid, item_index: potion });
            assert!(packets.contains(&ServerPacket::SetAutoPotItem { grid: grid as u8, item_index: potion }));
            assert!(session.handle_packet(ClientPacket::SetAutoPotItem { grid, item_index: other }).is_empty());
        }
        assert!(session.handle_packet(ClientPacket::SetAutoPotItem { grid: MirGridType::Inventory, item_index: potion }).is_empty());
        let hero = session.app.world().resource::<Stage5SystemsResource>().stage5_systems.hero.as_ref().unwrap();
        assert_eq!((hero.hp_item_index, hero.mp_item_index), (potion, potion));
    }

}
