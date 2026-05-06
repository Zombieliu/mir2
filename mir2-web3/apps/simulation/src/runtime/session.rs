#![cfg_attr(test, allow(unused_imports))]

use std::collections::BTreeSet;

use super::crystal_compat::*;
use super::equipment::*;
use super::inventory::*;
use super::items::{item_unique_id, user_item_from_item_state, ItemState};
use super::map::*;
use super::npc_script::*;
use super::packets::*;
use super::quests::*;
use super::resources::{
    BuffResource, FishingResource, GroupResource, InventoryResource, ItemRentalResource,
    MapRuntimeResource, MountResource, NpcStateResource, ObjectIdAllocatorResource,
    PlayerPermissionResource, PlayerRuntimeResource, PotionRecoveryResource, QuestResource,
    RuntimeClockResource, RuntimeConfigResource, RuntimeQueueResource, SessionResource,
    SkillResource, Stage5SystemsResource,
};
use super::save::*;
use super::skills::*;
use bevy_ecs::prelude::{Resource, World};

use crate::config::{ItemContainer, SimulationConfig, WorldSnapshot};
use mir2_game_data::LanguageCode;
use mir2_protocol::ServerPacket;

#[cfg(test)]
#[allow(unused_imports)]
pub(super) use super::{
    buffs::*, combat::*, components::*, crystal_compat::*, drops::*, equipment::*, fishing::*,
    inventory::*, items::*, map::*, monster_ai::*, monsters::*, movement::*, npc::*, npc_script::*,
    packets::*, quests::*, rental::*, resources::*, save::*, skills::*, stage5::*,
};
#[cfg(test)]
#[allow(unused_imports)]
pub(super) use mir2_game_data::{
    crystal_item_by_index, crystal_npc_info_by_script_key, crystal_quest_packet_payloads,
    format_localized_text, localized_text_or_fallback,
};
#[cfg(test)]
#[allow(unused_imports)]
pub(super) use mir2_protocol::UserItemStat;

pub(super) use super::packets::{
    hint_chat_key, hint_chat_key_args, localized_map_title, localized_monster_name_key,
    system_message, system_message_key, system_message_key_args,
};
pub(super) use super::resources::{current_language, is_in_world, runtime_tick, set_runtime_tick};

#[derive(Debug)]
pub(super) struct HeadlessRuntime {
    world: World,
}

impl HeadlessRuntime {
    pub(super) fn new() -> Self {
        Self {
            world: World::new(),
        }
    }

    pub(super) fn insert_resource<R: Resource>(&mut self, resource: R) {
        self.world.insert_resource(resource);
    }

    pub(super) fn world(&self) -> &World {
        &self.world
    }

    pub(super) fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }
}

#[derive(Debug)]
pub struct SimulationSession {
    pub(super) app: HeadlessRuntime,
    pub(super) visible_objects: BTreeSet<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSessionIdentity {
    pub account_id: String,
    pub character_index: i32,
    pub character_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedTradeOfferItem {
    pub item_state_json: String,
    pub key: String,
    pub unique_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedTradeOffer {
    pub account_id: String,
    pub character_index: i32,
    pub character_name: String,
    pub partner_name: String,
    pub gold: u32,
    pub items: Vec<SharedTradeOfferItem>,
}

impl SimulationSession {
    pub fn new(config: SimulationConfig) -> Self {
        let mut app = HeadlessRuntime::new();
        let initial_collision = runtime_map_collision_data(&config.map.file_name)
            .or_else(|| runtime_map_collision_data(&config.map_collision.map_file_name))
            .unwrap_or_else(|| runtime_map_collision_from_template(config.map_collision.clone()));
        app.insert_resource(RuntimeConfigResource::new(&config));
        app.insert_resource(SessionResource::new(&config));
        app.insert_resource(PlayerRuntimeResource::new(&config));
        app.insert_resource(MapRuntimeResource::new(
            &config,
            initial_collision.collision.region_bounds,
            initial_collision.blocked_set,
            initial_collision.closed_door_set,
        ));
        let mut inventory = InventoryResource::new(BASE_STORAGE_SLOTS);
        inventory.inventory_items = seed_inventory_items();
        inventory.belt_items = seed_belt_items();
        inventory.storage_items = seed_storage_items();
        inventory.equipment_items = seed_equipment_items();
        app.insert_resource(inventory);
        let mut quests = QuestResource::new();
        quests.quests = vec![QuestState::guide_training()];
        app.insert_resource(quests);
        let mut skills = SkillResource::new();
        skills.skills = seed_skills();
        app.insert_resource(skills);
        app.insert_resource(BuffResource::new());
        app.insert_resource(ItemRentalResource::new());
        app.insert_resource(FishingResource::new());
        app.insert_resource(MountResource::new());
        app.insert_resource(NpcStateResource::new());
        app.insert_resource(RuntimeQueueResource::new());
        app.insert_resource(Stage5SystemsResource::new());
        app.insert_resource(GroupResource::new(&config));
        app.insert_resource(PlayerPermissionResource::new());
        app.insert_resource(PotionRecoveryResource::new());
        app.insert_resource(RuntimeClockResource::new());
        app.insert_resource(ObjectIdAllocatorResource::new());
        app.insert_resource(CrystalNpcRandomState::new());
        rebuild_world(app.world_mut());
        Self {
            app,
            visible_objects: BTreeSet::new(),
        }
    }

    pub fn set_language(&mut self, language: LanguageCode) {
        self.app
            .world_mut()
            .resource_mut::<SessionResource>()
            .language = language;
    }

    pub fn set_language_code(&mut self, code: &str) -> Result<LanguageCode, String> {
        let Some(language) = LanguageCode::parse(code) else {
            return Err(format!("unsupported language: {code}"));
        };
        self.set_language(language);
        Ok(language)
    }

    pub fn save_active_character(&self) {
        persist_active_character_save(self.app.world());
    }

    pub fn refresh_active_external_mail(&mut self) -> bool {
        refresh_active_external_mail(self.app.world_mut())
    }

    pub fn item_rental_request(&mut self, partner_name: &str, renting: bool) -> Vec<ServerPacket> {
        super::rental::item_rental_request_impl(
            self.app.world_mut(),
            Some(partner_name.to_string()),
            renting,
        )
    }

    pub fn trade_request(&mut self, partner_name: &str) -> Vec<ServerPacket> {
        super::packets::stage5_trade_request_packet(
            self.app.world_mut(),
            Some(partner_name.to_string()),
        )
    }

    pub fn shared_trade_confirm(&mut self) -> (Vec<ServerPacket>, Option<SharedTradeOffer>) {
        let offer = build_shared_trade_offer(self.app.world());
        let packets = super::packets::stage5_trade_confirm_packet(self.app.world_mut(), true);
        let confirmed = packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::TradeConfirm));
        let packets = self.finalize_packets(packets);
        if confirmed {
            (packets, offer)
        } else {
            (packets, None)
        }
    }

    pub fn shared_trade_cancel(&mut self, unlock: bool) -> Vec<ServerPacket> {
        let packets = if unlock {
            super::packets::stage5_trade_confirm_packet(self.app.world_mut(), false)
        } else {
            super::packets::stage5_trade_cancel_packet(self.app.world_mut())
        };
        self.finalize_packets(packets)
    }

    pub fn apply_shared_trade_delivery(&mut self, offer: &SharedTradeOffer) -> Vec<ServerPacket> {
        let packets = apply_shared_trade_offer(self.app.world_mut(), offer, false);
        self.finalize_packets(packets)
    }

    pub fn rollback_shared_trade_offer(&mut self, offer: &SharedTradeOffer) -> Vec<ServerPacket> {
        let packets = apply_shared_trade_offer(self.app.world_mut(), offer, true);
        self.finalize_packets(packets)
    }

    pub fn on_connect(&self) -> Vec<ServerPacket> {
        vec![ServerPacket::Connected]
    }

    pub fn world_snapshot(&self) -> WorldSnapshot {
        build_world_snapshot(self.app.world())
    }

    pub fn active_identity(&self) -> Option<ActiveSessionIdentity> {
        let session = self.app.world().resource::<SessionResource>();
        let account_id = session.account_id.clone()?;
        let character = session.selected_character.as_ref()?;
        Some(ActiveSessionIdentity {
            account_id,
            character_index: character.index,
            character_name: character.name.clone(),
        })
    }
}

fn build_shared_trade_offer(world: &World) -> Option<SharedTradeOffer> {
    if !is_in_world(world) {
        return None;
    }
    let session = world.resource::<SessionResource>();
    let account_id = session.account_id.clone()?;
    let character = session.selected_character.as_ref()?;
    let stage5 = world.resource::<Stage5SystemsResource>();
    let trade = stage5.stage5_systems.trade.as_ref()?;
    if trade.completed {
        return None;
    }

    let inventory = world.resource::<InventoryResource>();
    let mut items = Vec::new();
    for inventory_index in trade.offered_slots.values() {
        let item = inventory
            .inventory_items
            .iter()
            .find(|item| inventory_item_matches_index(item, *inventory_index))?;
        let item_state_json = serde_json::to_string(item).ok()?;
        items.push(SharedTradeOfferItem {
            item_state_json,
            key: item.key.clone(),
            unique_id: item_unique_id(item),
        });
    }

    Some(SharedTradeOffer {
        account_id,
        character_index: character.index,
        character_name: character.name.clone(),
        partner_name: trade.partner.clone(),
        gold: trade.offered_gold,
        items,
    })
}

fn apply_shared_trade_offer(
    world: &mut World,
    offer: &SharedTradeOffer,
    rollback: bool,
) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let mut packets = Vec::new();
    if offer.gold > 0 {
        let mut player = world.resource_mut::<PlayerRuntimeResource>();
        player.gold = player.gold.saturating_add(offer.gold);
        packets.push(ServerPacket::GainedGold { gold: offer.gold });
    }

    for offered_item in &offer.items {
        let Ok(mut item) = serde_json::from_str::<ItemState>(&offered_item.item_state_json) else {
            continue;
        };
        let Some((container, slot)) = preferred_or_empty_trade_delivery_slot(
            world.resource::<InventoryResource>(),
            item.container,
            item.slot,
        ) else {
            packets.push(system_message("Trade delivery failed: inventory is full."));
            continue;
        };
        item.container = container;
        item.slot = slot;
        let user_item = user_item_from_item_state(&item);
        world
            .resource_mut::<InventoryResource>()
            .inventory_items
            .push(item);
        packets.push(ServerPacket::GainedItem { item: user_item });
    }

    if rollback {
        world
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .trade = None;
        packets.push(ServerPacket::TradeCancel { unlock: false });
    }

    packets
}

fn preferred_or_empty_trade_delivery_slot(
    inventory: &InventoryResource,
    preferred_container: ItemContainer,
    preferred_slot: u8,
) -> Option<(ItemContainer, u8)> {
    if matches!(
        preferred_container,
        ItemContainer::Bag1 | ItemContainer::Bag2
    ) && !inventory
        .inventory_items
        .iter()
        .any(|item| item.container == preferred_container && item.slot == preferred_slot)
    {
        return Some((preferred_container, preferred_slot));
    }
    find_empty_inventory_item_slot(&inventory.inventory_items, ItemContainer::Bag1)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
