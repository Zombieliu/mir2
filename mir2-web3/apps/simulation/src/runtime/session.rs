#![cfg_attr(test, allow(unused_imports))]

use std::collections::BTreeSet;

use super::combat::combat_delay_ticks;
use super::components::{
    entity_by_object_id, entity_name, entity_object_id, player_entity, Facing, Monster,
    MonsterAgent, MonsterVitals, PlayerVitals, Position,
};
use super::crystal_compat::*;
use super::drops::{
    zone_ground_drop_snapshots_for_monster, SharedAccountInventoryTransactionReceipt,
};
use super::equipment::*;
use super::inventory::*;
use super::items::{item_unique_id, user_item_from_item_state, ItemState};
use super::map::*;
use super::npc_script::*;
use super::packets::*;
use super::quests::*;
use super::resources::{
    advance_runtime_tick, BuffResource, ElementalResource, FishingResource, GroupResource,
    HeroInventoryResource, InventoryResource, ItemRentalRecordState, ItemRentalResource,
    MapRuntimeResource, MountResource, NpcStateResource, ObjectIdAllocatorResource,
    PlayerActionTimingResource, PlayerMovementTimingResource, PlayerPermissionResource,
    PlayerRuntimeResource, PotionRecoveryResource, QuestResource, RuntimeClockResource,
    RuntimeConfigResource, RuntimeQueueResource, SessionResource, SkillResource,
    Stage5SystemsResource,
};
use super::save::*;
use super::skills::*;
use bevy_ecs::prelude::{Resource, World};

use crate::config::{
    CharacterRecord, ItemContainer, SimulationConfig, WorldEntityKind, WorldEntitySnapshot,
    WorldSnapshot,
};
use crate::runtime::zone::{
    SessionId, ZoneChatProfile, ZoneJoin, ZoneMonsterDefense, ZoneMonsterSpawn,
};
use mir2_game_data::{crystal_monster_by_name, CrystalMonsterTemplate, LanguageCode};
use mir2_protocol::{
    ChatItem, ClientBuff, ItemRentalInformation, Point, ServerPacket, Spell,
    UserItemRentalInformation,
};

#[cfg(test)]
#[allow(unused_imports)]
pub(super) use super::{
    buffs::*, combat::*, components::*, crystal_compat::*, drops::*, equipment::*, fishing::*,
    inventory::*, items::*, map::*, monster_ai::*, monsters::*, movement::*, npc::*, npc_script::*,
    packets::*, quests::*, rental::*, resources::*, save::*, skills::*, stage5::*, stats::*,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedItemRentalItemOffer {
    pub account_id: String,
    pub character_index: i32,
    pub character_name: String,
    pub partner_name: String,
    pub item_state_json: String,
    pub item_id: u64,
    pub item_name: String,
    pub days: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedItemRentalFeeOffer {
    pub account_id: String,
    pub character_index: i32,
    pub character_name: String,
    pub partner_name: String,
    pub fee: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedItemRentalAgreement {
    pub item: SharedItemRentalItemOffer,
    pub fee: SharedItemRentalFeeOffer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharedItemRentalDelivery {
    Lender(SharedItemRentalAgreement),
    Borrower(SharedItemRentalAgreement),
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
        let initial_doors =
            super::resources::DoorRegistry::from_templates(&initial_collision.collision.doors);
        app.insert_resource(MapRuntimeResource::new(
            &config,
            initial_collision.collision.region_bounds,
            initial_collision.blocked_set,
            initial_collision.closed_door_set,
            initial_doors,
            initial_collision.fishing_cells,
        ));
        app.insert_resource(super::mining::MiningResource::with_builtin_sets());
        super::mining::rebuild_mine_spots(app.world_mut());
        app.insert_resource(super::hazard::MapHazardResource::default());
        let mut inventory = InventoryResource::new(BASE_STORAGE_SLOTS);
        inventory.inventory_items = seed_inventory_items();
        inventory.belt_items = seed_belt_items();
        inventory.storage_items = seed_storage_items();
        inventory.equipment_items = seed_equipment_items();
        app.insert_resource(inventory);
        app.insert_resource(HeroInventoryResource::new());
        let mut quests = QuestResource::new();
        quests.quests = vec![QuestState::guide_training()];
        app.insert_resource(quests);
        let mut skills = SkillResource::new();
        skills.skills = seed_skills();
        app.insert_resource(skills);
        app.insert_resource(BuffResource::new());
        app.insert_resource(ElementalResource::new());
        app.insert_resource(ItemRentalResource::new());
        app.insert_resource(FishingResource::new());
        app.insert_resource(MountResource::new());
        app.insert_resource(NpcStateResource::new());
        app.insert_resource(RuntimeQueueResource::new());
        app.insert_resource(Stage5SystemsResource::new());
        app.insert_resource(GroupResource::new(&config));
        app.insert_resource(PlayerPermissionResource::new());
        app.insert_resource(super::resources::GmRuntimeResource::new());
        app.insert_resource(PotionRecoveryResource::new());
        app.insert_resource(PlayerActionTimingResource::new());
        app.insert_resource(PlayerMovementTimingResource::new());
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

    pub fn item_rental_cancel(&mut self) -> Vec<ServerPacket> {
        let packets = super::rental::cancel_item_rental_impl(self.app.world_mut());
        self.finalize_packets(packets)
    }

    pub fn trade_request(&mut self, partner_name: &str) -> Vec<ServerPacket> {
        super::packets::stage5_trade_request_packet(
            self.app.world_mut(),
            Some(partner_name.to_string()),
        )
    }

    pub fn shared_item_rental_lock_fee(
        &mut self,
    ) -> (Vec<ServerPacket>, Option<SharedItemRentalFeeOffer>) {
        let packets = super::rental::item_rental_lock_fee_impl(self.app.world_mut());
        let locked = packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ItemRentalLock {
                    success: true,
                    gold_locked: true,
                    ..
                }
            )
        });
        let offer = locked
            .then(|| build_shared_item_rental_fee_offer(self.app.world()))
            .flatten();
        (self.finalize_packets(packets), offer)
    }

    pub fn shared_item_rental_lock_item(
        &mut self,
    ) -> (Vec<ServerPacket>, Option<SharedItemRentalItemOffer>) {
        let packets = super::rental::item_rental_lock_item_impl(self.app.world_mut());
        let locked = packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ItemRentalLock {
                    success: true,
                    item_locked: true,
                    ..
                }
            )
        });
        let offer = locked
            .then(|| build_shared_item_rental_item_offer(self.app.world()))
            .flatten();
        (self.finalize_packets(packets), offer)
    }

    pub fn apply_shared_item_rental_delivery(
        &mut self,
        delivery: &SharedItemRentalDelivery,
    ) -> Vec<ServerPacket> {
        let packets = apply_shared_item_rental_delivery(self.app.world_mut(), delivery);
        self.finalize_packets(packets)
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

    pub fn passkey_login(&mut self, account_id: &str) -> Vec<ServerPacket> {
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        let characters = match login_passkey_account(&config, account_id) {
            AccountLoginResult::Success(characters) => characters,
            AccountLoginResult::Banned(ban) => {
                return vec![ServerPacket::LoginBanned {
                    reason: ban.reason,
                    expiry_binary_datetime: ban.ban_until_ms.unwrap_or_default() as i64,
                }];
            }
            AccountLoginResult::InvalidCredentials => {
                return vec![ServerPacket::Login { result: 4 }];
            }
        };
        let mut session = self.app.world_mut().resource_mut::<SessionResource>();
        session.account_id = Some(account_id.to_string());
        session.characters = characters;
        vec![ServerPacket::LoginSuccess {
            characters: session
                .characters
                .iter()
                .map(CharacterRecord::to_select_info)
                .collect(),
        }]
    }

    pub fn world_snapshot(&self) -> WorldSnapshot {
        build_world_snapshot(self.app.world())
    }

    pub fn prepare_chat_packet_for_zone(
        &mut self,
        message: String,
        linked_items: Vec<ChatItem>,
    ) -> ChatPacketPreparation {
        prepare_chat_packet(self.app.world_mut(), message, linked_items)
    }

    /// True when the GM `@LOGIN` password prompt is armed for the next chat line
    /// (`gm_commands::gm_login_pending`). The active shared-Zone chat router checks
    /// this so the candidate password line is dispatched on the personal-session
    /// path instead of being broadcast as public chat.
    pub fn gm_login_pending(&self) -> bool {
        super::gm_commands::gm_login_pending(self.app.world())
    }

    pub fn consume_zone_chat_shout_permission(&mut self, map_shout: bool, server_shout: bool) {
        if !map_shout && !server_shout {
            return;
        }
        let mut permissions = self
            .app
            .world_mut()
            .resource_mut::<PlayerPermissionResource>();
        if map_shout {
            permissions.free_map_shout = false;
        }
        if server_shout {
            permissions.free_server_shout = false;
        }
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

    pub fn active_zone_join_snapshot(&self, session_id: impl Into<String>) -> Option<ZoneJoin> {
        if !is_in_world(self.app.world()) {
            return None;
        }
        let session = self.app.world().resource::<SessionResource>();
        let account_id = session.account_id.clone()?;
        let character = session.selected_character.as_ref()?.clone();
        let snapshot = self.world_snapshot();
        let self_player = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == crate::WorldEntityKind::SelfPlayer)?;
        let stage5 = self.app.world().resource::<Stage5SystemsResource>();
        let permissions = self.app.world().resource::<PlayerPermissionResource>();
        let guild_name = (!stage5.stage5_systems.guild.name.trim().is_empty())
            .then(|| stage5.stage5_systems.guild.name.clone());
        let mentor_name = (!stage5.stage5_systems.mentor.name.trim().is_empty())
            .then(|| stage5.stage5_systems.mentor.name.clone());
        let relationship_name = (!stage5
            .stage5_systems
            .relationship
            .partner_name
            .trim()
            .is_empty())
        .then(|| stage5.stage5_systems.relationship.partner_name.clone());
        Some(ZoneJoin {
            session_id: SessionId::new(session_id.into()),
            account_id,
            character_index: character.index,
            object_id: self_player.object_id,
            name: character.name,
            class: character.class,
            gender: character.gender,
            level: character.level,
            hp: snapshot.player_hp.unwrap_or(1),
            max_hp: snapshot.player_max_hp.unwrap_or(1).max(1),
            mp: snapshot.player_mp.unwrap_or_default().max(0),
            map_file_name: snapshot.map_file_name.unwrap_or_default(),
            position: Point {
                x: self_player.x,
                y: self_player.y,
            },
            direction: self_player.direction,
            chat_profile: ZoneChatProfile {
                group_members: stage5.stage5_systems.group.members.clone(),
                guild_name,
                blocked_names: stage5.stage5_systems.social.blocked.clone(),
                mentor_name,
                relationship_name,
                is_gm: false,
                free_map_shout: permissions.free_map_shout,
                free_server_shout: permissions.free_server_shout,
            },
            combat_stats: self.zone_player_combat_stats(),
        })
    }

    pub fn force_authoritative_player_transform(
        &mut self,
        position: Point,
        direction: mir2_protocol::MirDirection,
    ) {
        let world = self.app.world_mut();
        if let Some(player) = player_entity(world) {
            world
                .entity_mut(player)
                .insert((Position(position.clone()), Facing(direction)));
        }
        {
            let mut runtime = world.resource_mut::<PlayerRuntimeResource>();
            runtime.player_position = position;
            runtime.player_direction = direction;
        }
        advance_runtime_tick(world);
    }

    /// Land chain-confirmed ore in the active player's bag (M3, WF-4) and re-render the
    /// vein from the chain-reported `stones_left` when `mine_id` maps to a configured
    /// on-chain node (M4, WF-6). Injected by the trusted relayer/gateway path only (never
    /// the raw player path). Idempotent on `idempotency_key`; additive — never touches P0
    /// server-mining.
    pub fn grant_onchain_ore(
        &mut self,
        ore_kind: &str,
        amount: u64,
        mine_id: u64,
        stones_left: u32,
        idempotency_key: &str,
    ) -> Vec<ServerPacket> {
        if !is_in_world(self.app.world()) {
            return Vec::new();
        }
        super::onchain::grant_onchain_ore(
            self.app.world_mut(),
            ore_kind,
            amount,
            mine_id,
            stones_left,
            idempotency_key,
        )
    }

    /// Credit chain-confirmed gold (from redeeming on-chain ore) to the active player.
    /// Idempotent on `idempotency_key`.
    pub fn credit_gold_from_ore(&mut self, gold: u32, idempotency_key: &str) -> Vec<ServerPacket> {
        if !is_in_world(self.app.world()) {
            return Vec::new();
        }
        super::onchain::credit_gold_from_ore(self.app.world_mut(), gold, idempotency_key)
    }

    pub fn apply_zone_player_damage(&mut self, damage: i32) {
        if damage <= 0 || !is_in_world(self.app.world()) {
            return;
        }
        let world = self.app.world_mut();
        let Some(player) = player_entity(world) else {
            return;
        };
        let updated_vitals = {
            let mut entity = world.entity_mut(player);
            entity.get_mut::<PlayerVitals>().map(|mut vitals| {
                vitals.hp = vitals.hp.saturating_sub(damage).max(0);
                *vitals
            })
        };
        if let Some(vitals) = updated_vitals {
            world.resource_mut::<PlayerRuntimeResource>().player_vitals = vitals;
            advance_runtime_tick(world);
        }
    }

    pub fn apply_zone_player_heal(&mut self, amount: i32) {
        if amount <= 0 || !is_in_world(self.app.world()) {
            return;
        }
        let world = self.app.world_mut();
        let Some(player) = player_entity(world) else {
            return;
        };
        let updated_vitals = {
            let mut entity = world.entity_mut(player);
            entity.get_mut::<PlayerVitals>().map(|mut vitals| {
                vitals.hp = vitals.hp.saturating_add(amount).min(vitals.max_hp);
                *vitals
            })
        };
        if let Some(vitals) = updated_vitals {
            world.resource_mut::<PlayerRuntimeResource>().player_vitals = vitals;
            advance_runtime_tick(world);
        }
    }

    pub fn apply_zone_player_magic_spend(&mut self, spell: Spell, mp_cost: i32, cooldown_ms: u64) {
        if !is_in_world(self.app.world()) {
            return;
        }
        let world = self.app.world_mut();
        let Some(player) = player_entity(world) else {
            return;
        };
        let updated_vitals = {
            let mut entity = world.entity_mut(player);
            entity.get_mut::<PlayerVitals>().map(|mut vitals| {
                vitals.mp = vitals.mp.saturating_sub(mp_cost.max(0)).max(0);
                *vitals
            })
        };
        if let Some(vitals) = updated_vitals {
            world.resource_mut::<PlayerRuntimeResource>().player_vitals = vitals;
        }
        if let Some(skill_key) = skill_key_for_crystal_spell(spell) {
            let tick = runtime_tick(world);
            let cooldown_ticks = combat_delay_ticks(cooldown_ms.max(1));
            if let Some(skill) = world
                .resource_mut::<SkillResource>()
                .skills
                .iter_mut()
                .find(|skill| skill.key == skill_key)
            {
                skill.cooldown_ticks = cooldown_ticks.min(u64::from(u32::MAX)) as u32;
                skill.delay_ms = i64::try_from(cooldown_ms).unwrap_or(i64::MAX);
                skill.cooldown_ends_at = tick.saturating_add(cooldown_ticks);
                skill.cast_time_ms = i64::try_from(tick.saturating_mul(1_000)).unwrap_or(i64::MAX);
            }
        }
        advance_runtime_tick(world);
    }

    pub fn apply_zone_player_buff_packets(
        &mut self,
        packets: &[ServerPacket],
        zone_object_id: u32,
    ) {
        for packet in packets {
            self.apply_zone_player_buff_packet(packet, zone_object_id);
        }
    }

    fn apply_zone_player_buff_packet(&mut self, packet: &ServerPacket, zone_object_id: u32) {
        if !is_in_world(self.app.world()) {
            return;
        }
        let local_player_object_id = player_entity(self.app.world())
            .and_then(|player| entity_object_id(self.app.world(), player));
        match packet {
            ServerPacket::AddBuff { buff }
                if local_player_object_id.is_some_and(|object_id| {
                    zone_player_buff_targets_self(buff, object_id, zone_object_id)
                }) =>
            {
                self.apply_zone_player_add_buff(buff);
            }
            ServerPacket::RemoveBuff {
                object_id,
                buff_type,
            } if local_player_object_id.is_some_and(|local_object_id| {
                *object_id == local_object_id || *object_id == zone_object_id
            }) =>
            {
                self.apply_zone_player_remove_buff(*buff_type);
            }
            _ => {}
        }
    }

    fn apply_zone_player_add_buff(&mut self, buff: &ClientBuff) {
        let Some(key) = super::buffs::crystal_buff_key_for_type(buff.buff_type) else {
            return;
        };
        let world = self.app.world_mut();
        let tick = runtime_tick(world);
        let expires_at_tick = if buff.infinite {
            u64::MAX
        } else {
            let duration_ms = u64::try_from(buff.expire_time.max(0)).unwrap_or_default();
            tick.saturating_add(combat_delay_ticks(duration_ms.max(1)))
        };
        let fallback_name = zone_buff_fallback_name(key);
        let fallback_description = format!("{fallback_name} is active.");
        let (name, description) =
            super::buffs::buff_metadata(key, &fallback_name, &fallback_description);
        super::buffs::apply_or_refresh_buff(
            world,
            super::buffs::BuffState {
                key: key.to_string(),
                name,
                description,
                expires_at_tick,
                attack_bonus: 0,
                defence_bonus: 0,
                stats: buff.stats.clone(),
            },
        );
        advance_runtime_tick(world);
    }

    fn apply_zone_player_remove_buff(&mut self, buff_type: u8) {
        let Some(key) = super::buffs::crystal_buff_key_for_type(buff_type) else {
            return;
        };
        let world = self.app.world_mut();
        let before = world.resource::<BuffResource>().buffs.len();
        world
            .resource_mut::<BuffResource>()
            .buffs
            .retain(|buff| buff.key != key);
        if world.resource::<BuffResource>().buffs.len() != before {
            advance_runtime_tick(world);
        }
    }

    pub fn commit_shared_skill_item_consumption_transaction(
        &mut self,
        spell: Spell,
    ) -> SharedAccountInventoryTransactionReceipt {
        if !is_in_world(self.app.world()) {
            return SharedAccountInventoryTransactionReceipt::skill_item_consumption(
                false,
                Vec::new(),
            );
        }
        let packets = {
            let world = self.app.world_mut();
            consume_zone_magic_inventory_components(world, spell)
        };
        let Some(packets) = packets else {
            return SharedAccountInventoryTransactionReceipt::skill_item_consumption(
                false,
                Vec::new(),
            );
        };
        SharedAccountInventoryTransactionReceipt::skill_item_consumption(true, packets)
    }

    pub fn apply_shared_entity_snapshot(&mut self, snapshot: &WorldEntitySnapshot) -> bool {
        if snapshot.kind != WorldEntityKind::Monster || !is_in_world(self.app.world()) {
            return false;
        }
        let world = self.app.world_mut();
        let Some(entity) = entity_by_object_id(world, snapshot.object_id) else {
            return false;
        };
        if !world.entity(entity).contains::<Monster>() {
            return false;
        }

        {
            let mut entity_mut = world.entity_mut(entity);
            entity_mut.insert((
                Position(Point {
                    x: snapshot.x,
                    y: snapshot.y,
                }),
                Facing(snapshot.direction),
            ));
            if let Some(mut vitals) = entity_mut.get_mut::<MonsterVitals>() {
                if let Some(max_hp) = snapshot.max_hp {
                    vitals.max_hp = max_hp.max(1);
                }
                if let Some(hp) = snapshot.hp {
                    vitals.hp = hp.clamp(0, vitals.max_hp);
                }
                if snapshot.dead {
                    vitals.hp = 0;
                }
            }
            if let Some(mut agent) = entity_mut.get_mut::<MonsterAgent>() {
                agent.dead = snapshot.dead || snapshot.hp.is_some_and(|hp| hp <= 0);
            }
        }
        advance_runtime_tick(world);
        true
    }

    pub fn zone_monster_spawn_snapshot(&self, object_id: u32) -> Option<ZoneMonsterSpawn> {
        let world = self.app.world();
        if !is_in_world(world) {
            return None;
        }

        let entity = entity_by_object_id(world, object_id)?;
        let entry = world.entity(entity);
        let agent = entry.get::<MonsterAgent>()?;
        if agent.dead {
            return None;
        }
        let vitals = entry.get::<MonsterVitals>()?;
        let position = entry.get::<Position>()?.0.clone();
        let direction = entry
            .get::<Facing>()
            .map(|facing| facing.0)
            .unwrap_or(mir2_protocol::MirDirection::Down);
        let name = entity_name(world, entity).unwrap_or_else(|| "Monster".to_string());
        let template = crystal_monster_by_name(&name);
        let object_id = entity_object_id(world, entity).unwrap_or(object_id);
        let max_hp = vitals.max_hp.max(1);

        Some(ZoneMonsterSpawn {
            object_id,
            name: name.clone(),
            name_colour_argb: -1,
            image: agent.image,
            ai: agent.ai,
            level: template.as_ref().map(|monster| monster.level).unwrap_or(1),
            max_hp,
            hp: vitals.hp.clamp(0, max_hp),
            experience: template
                .as_ref()
                .map(|monster| monster.experience)
                .unwrap_or(0),
            defense: template
                .as_ref()
                .map(zone_monster_defense_from_template)
                .unwrap_or_default(),
            position,
            direction,
            drops: zone_ground_drop_snapshots_for_monster(world, object_id, &name),
        })
    }
}

/// Project a Crystal monster template's authoritative defensive stats into the
/// shared-zone snapshot so the zone can resolve incoming player damage itself.
pub(super) fn zone_monster_defense_from_template(
    template: &CrystalMonsterTemplate,
) -> ZoneMonsterDefense {
    ZoneMonsterDefense::from_crystal_template(template)
}

fn zone_player_buff_targets_self(
    buff: &ClientBuff,
    local_object_id: u32,
    zone_object_id: u32,
) -> bool {
    buff.object_id == local_object_id || buff.object_id == zone_object_id
}

fn zone_buff_fallback_name(key: &str) -> String {
    key.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn build_shared_item_rental_fee_offer(world: &World) -> Option<SharedItemRentalFeeOffer> {
    if !is_in_world(world) {
        return None;
    }
    let session = world.resource::<SessionResource>();
    let account_id = session.account_id.clone()?;
    let character = session.selected_character.as_ref()?;
    let rental = world.resource::<ItemRentalResource>();
    let active = rental.active.as_ref()?;
    if !active.gold_locked || active.fee == 0 {
        return None;
    }

    Some(SharedItemRentalFeeOffer {
        account_id,
        character_index: character.index,
        character_name: character.name.clone(),
        partner_name: active.partner_name.clone(),
        fee: active.fee,
    })
}

fn build_shared_item_rental_item_offer(world: &World) -> Option<SharedItemRentalItemOffer> {
    if !is_in_world(world) {
        return None;
    }
    let session = world.resource::<SessionResource>();
    let account_id = session.account_id.clone()?;
    let character = session.selected_character.as_ref()?;
    let rental = world.resource::<ItemRentalResource>();
    let active = rental.active.as_ref()?;
    let item = active.deposited_item.as_ref()?;
    if !active.item_locked {
        return None;
    }

    Some(SharedItemRentalItemOffer {
        account_id,
        character_index: character.index,
        character_name: character.name.clone(),
        partner_name: active.partner_name.clone(),
        item_state_json: serde_json::to_string(item).ok()?,
        item_id: item_unique_id(item),
        item_name: item.name.clone(),
        days: active.days,
    })
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
    for (trade_slot, inventory_index) in trade.offered_slots.iter() {
        let item = inventory
            .inventory_items
            .iter()
            .find(|item| inventory_item_matches_index(item, *inventory_index))?;
        // Integrity check (F-07): never deliver an item that was swapped into the
        // offered slot after it was deposited. If the live item's id no longer
        // matches what was deposited, refuse to build the offer.
        if trade.offered_unique_ids.get(trade_slot).copied() != Some(item_unique_id(item)) {
            return None;
        }
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

    let mut staged_inventory = world
        .resource::<InventoryResource>()
        .inventory_items
        .clone();
    let mut delivered_items = Vec::new();
    for offered_item in &offer.items {
        let Ok(mut item) = serde_json::from_str::<ItemState>(&offered_item.item_state_json) else {
            return trade_offer_delivery_failed_packets(world, rollback);
        };
        let Some((container, slot)) = preferred_or_empty_trade_delivery_slot_for_items(
            &staged_inventory,
            item.container,
            item.slot,
        ) else {
            return trade_offer_delivery_failed_packets(world, rollback);
        };
        item.container = container;
        item.slot = slot;
        let unique_id = item_unique_id(&item);
        if staged_inventory
            .iter()
            .any(|existing| item_unique_id(existing) == unique_id)
        {
            let mut next_unique_id =
                allocate_item_unique_id(world.resource::<InventoryResource>(), container, slot);
            while staged_inventory
                .iter()
                .any(|existing| item_unique_id(existing) == next_unique_id)
            {
                next_unique_id = next_unique_id.saturating_add(1);
            }
            item.unique_id = next_unique_id;
        } else {
            item.unique_id = unique_id;
        }
        staged_inventory.push(item.clone());
        delivered_items.push(item);
    }

    if offer.gold > 0 {
        let mut player = world.resource_mut::<PlayerRuntimeResource>();
        player.gold = player.gold.saturating_add(offer.gold);
        packets.push(ServerPacket::GainedGold { gold: offer.gold });
    }

    for item in delivered_items {
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

fn trade_offer_delivery_failed_packets(world: &mut World, rollback: bool) -> Vec<ServerPacket> {
    let mut packets = vec![system_message_key(world, "server.YouCannotCarryAnymore")];
    if rollback {
        world
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .trade = None;
    }
    packets.push(ServerPacket::TradeCancel { unlock: false });
    packets
}

fn apply_shared_item_rental_delivery(
    world: &mut World,
    delivery: &SharedItemRentalDelivery,
) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let agreement = match delivery {
        SharedItemRentalDelivery::Lender(agreement)
        | SharedItemRentalDelivery::Borrower(agreement) => agreement,
    };
    let expiry = future_binary_datetime(u64::from(agreement.item.days));

    match delivery {
        SharedItemRentalDelivery::Lender(_) => {
            {
                let mut rental = world.resource_mut::<ItemRentalResource>();
                rental.active = None;
                rental.rented_items.push(ItemRentalRecordState {
                    item_id: agreement.item.item_id,
                    item_name: agreement.item.item_name.clone(),
                    renting_player_name: agreement.fee.character_name.clone(),
                    item_return_date_binary_datetime: expiry,
                });
            }
            {
                let mut player = world.resource_mut::<PlayerRuntimeResource>();
                player.gold = player.gold.saturating_add(agreement.fee.fee);
            }
            let rented_items = world
                .resource::<ItemRentalResource>()
                .rented_items
                .iter()
                .map(|record| ItemRentalInformation {
                    item_id: record.item_id,
                    item_name: record.item_name.clone(),
                    renting_player_name: record.renting_player_name.clone(),
                    item_return_date_binary_datetime: record.item_return_date_binary_datetime,
                })
                .collect();
            vec![
                ServerPacket::GainedGold {
                    gold: agreement.fee.fee,
                },
                ServerPacket::ConfirmItemRental,
                ServerPacket::GetRentedItems { rented_items },
            ]
        }
        SharedItemRentalDelivery::Borrower(_) => {
            let Ok(mut item) = serde_json::from_str::<ItemState>(&agreement.item.item_state_json)
            else {
                return vec![ServerPacket::CancelItemRental];
            };
            let Some((container, slot)) = preferred_or_empty_trade_delivery_slot(
                world.resource::<InventoryResource>(),
                item.container,
                item.slot,
            ) else {
                return vec![
                    system_message("Unable to accept the rental item."),
                    ServerPacket::CancelItemRental,
                ];
            };
            item.container = container;
            item.slot = slot;
            item.rental_binding_flags = super::rental::CRYSTAL_RENTAL_BINDING_FLAGS;
            item.rental_owner_name = agreement.item.character_name.clone();
            item.rental_expiry_binary_datetime = expiry;
            item.rental_locked = false;

            let mut loan_item = user_item_from_item_state(&item);
            loan_item.rental_information = Some(UserItemRentalInformation {
                owner_name: agreement.item.character_name.clone(),
                binding_flags: super::rental::CRYSTAL_RENTAL_BINDING_FLAGS,
                expiry_binary_datetime: expiry,
                rental_locked: false,
            });
            world
                .resource_mut::<InventoryResource>()
                .inventory_items
                .push(item);
            {
                let mut rental = world.resource_mut::<ItemRentalResource>();
                rental.active = None;
                rental.has_rented_item = true;
            }

            vec![
                ServerPacket::GainedItem { item: loan_item },
                ServerPacket::ConfirmItemRental,
            ]
        }
    }
}

fn preferred_or_empty_trade_delivery_slot(
    inventory: &InventoryResource,
    preferred_container: ItemContainer,
    preferred_slot: u8,
) -> Option<(ItemContainer, u8)> {
    preferred_or_empty_trade_delivery_slot_for_items(
        &inventory.inventory_items,
        preferred_container,
        preferred_slot,
    )
}

fn preferred_or_empty_trade_delivery_slot_for_items(
    items: &[ItemState],
    preferred_container: ItemContainer,
    preferred_slot: u8,
) -> Option<(ItemContainer, u8)> {
    if matches!(
        preferred_container,
        ItemContainer::Bag1 | ItemContainer::Bag2
    ) && !items
        .iter()
        .any(|item| item.container == preferred_container && item.slot == preferred_slot)
    {
        return Some((preferred_container, preferred_slot));
    }
    find_empty_inventory_item_slot(items, ItemContainer::Bag1)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
