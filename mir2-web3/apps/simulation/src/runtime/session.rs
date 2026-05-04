#![cfg_attr(test, allow(unused_imports))]

use std::collections::BTreeSet;

use super::crystal_compat::*;
use super::equipment::*;
use super::inventory::*;
use super::map::*;
use super::npc_script::*;
use super::packets::*;
use super::quests::*;
use super::resources::{
    BuffResource, GroupResource, InventoryResource, MapRuntimeResource, NpcStateResource,
    ObjectIdAllocatorResource, PlayerPermissionResource, PlayerRuntimeResource,
    PotionRecoveryResource, QuestResource, RuntimeClockResource, RuntimeConfigResource,
    RuntimeQueueResource, SessionResource, SkillResource, Stage5SystemsResource,
};
use super::save::*;
use super::skills::*;
use bevy_ecs::prelude::{Resource, World};

use crate::config::{SimulationConfig, WorldSnapshot};
use mir2_game_data::LanguageCode;
use mir2_protocol::ServerPacket;

#[cfg(test)]
#[allow(unused_imports)]
pub(super) use super::{
    buffs::*, combat::*, components::*, crystal_compat::*, drops::*, equipment::*, inventory::*,
    items::*, map::*, monster_ai::*, monsters::*, movement::*, npc::*, npc_script::*, packets::*,
    quests::*, resources::*, save::*, skills::*, stage5::*,
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

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
