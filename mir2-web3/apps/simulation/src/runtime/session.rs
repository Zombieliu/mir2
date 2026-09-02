#![cfg_attr(test, allow(unused_imports))]

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::combat::{apply_damage_to_current_player, combat_delay_ticks, set_skill_toggle_state};
use super::components::{
    entity_by_object_id, entity_name, entity_object_id, player_entity, Facing, Monster,
    MonsterAgent, MonsterVitals, PlayerVitals, Position, SpawnSlotRef,
};
use super::crystal_compat::*;
use super::drops::{
    drop_player_death_penalty, zone_ground_drop_snapshots_for_monster,
    SharedAccountInventoryTransactionReceipt,
};
use super::equipment::*;
use super::inventory::*;
use super::items::{
    item_unique_id, try_user_item_from_item_state, validate_committed_item_state_carrier, ItemState,
};
use super::map::*;
use super::monsters::{
    apply_shared_monster_death_state, apply_shared_monster_revive_state,
    reset_shared_monster_harvest_state, spawn_shared_monster_snapshot, MonsterRespawnSchedule,
    MonsterSpawnTable,
};
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
    GroundDropSnapshot, ItemContainer, SimulationConfig, WorldEntityKind, WorldEntitySnapshot,
    WorldSnapshot, CRYSTAL_MAX_INVENTORY_CAPACITY,
};
use crate::runtime::zone::{
    SessionId, ZoneChatProfile, ZoneJoin, ZoneMonsterDefense, ZoneMonsterRespawnPolicy,
    ZoneMonsterSpawn,
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
    pub(super) dirty_economy_projection_event_ids: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveSessionIdentity {
    pub account_id: String,
    pub character_index: i32,
    pub character_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasskeyRecoveryPreflight {
    ExistingEligible,
    Missing,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedTradeOfferItem {
    pub item_state_json: String,
    pub key: String,
    pub unique_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedTradeOffer {
    #[serde(default)]
    pub settlement_nonce: String,
    pub account_id: String,
    pub character_index: i32,
    pub character_name: String,
    pub partner_name: String,
    pub gold: u32,
    pub items: Vec<SharedTradeOfferItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedSkillItemConsumptionComponent {
    pub item_key: String,
    pub quantity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedItemRentalItemOffer {
    #[serde(default)]
    pub transaction_nonce: String,
    pub account_id: String,
    pub character_index: i32,
    pub character_name: String,
    pub partner_name: String,
    pub item_state_json: String,
    pub item_id: u64,
    pub item_name: String,
    pub days: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedItemRentalFeeOffer {
    #[serde(default)]
    pub transaction_nonce: String,
    pub account_id: String,
    pub character_index: i32,
    pub character_name: String,
    pub partner_name: String,
    pub fee: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedItemRentalAgreement {
    pub item: SharedItemRentalItemOffer,
    pub fee: SharedItemRentalFeeOffer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
        inventory.inventory_capacity = CRYSTAL_MAX_INVENTORY_CAPACITY;
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
            dirty_economy_projection_event_ids: BTreeSet::new(),
        }
    }

    pub fn rebind_account_store(&mut self, authoritative: &SimulationConfig) {
        self.app
            .world_mut()
            .resource_mut::<RuntimeConfigResource>()
            .config
            .rebind_account_store_from(authoritative);
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

    pub fn save_active_character(&self) -> Result<(), String> {
        persist_active_character_save(self.app.world())
    }

    pub fn save_active_character_for_logout(&self) -> Result<(), String> {
        persist_active_character_save_for_logout(self.app.world())
    }

    pub fn has_shared_economy_projection_event(&self, event_id: &str) -> bool {
        self.app
            .world()
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .economy_projection_event_ids
            .contains(event_id)
    }

    /// Record the external ledger event in the same durable character snapshot
    /// as its already-applied private projection. If persistence fails the
    /// in-memory marker remains: a same-process retry persists it without
    /// replaying, while a crash restores the prior state and safely replays.
    pub fn persist_shared_economy_projection_event(
        &mut self,
        event_id: &str,
    ) -> Result<(), String> {
        if event_id.len() != 64
            || !event_id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("economy projection event ID must be lowercase SHA-256 hex".to_string());
        }
        let already_durable = self.has_shared_economy_projection_event(event_id)
            && !self.dirty_economy_projection_event_ids.contains(event_id);
        if already_durable {
            return Ok(());
        }

        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .economy_projection_event_ids
            .insert(event_id.to_string());
        self.dirty_economy_projection_event_ids
            .insert(event_id.to_string());
        let result = self.save_active_character();
        if result.is_ok() {
            self.dirty_economy_projection_event_ids.remove(event_id);
        }
        result
    }

    /// Materialize a durable ground-drop reward into the private character
    /// snapshot exactly once. The event marker is saved with the asset change;
    /// a same-process save retry observes the dirty marker, while a crash
    /// restores the prior snapshot and safely replays from the durable row.
    pub fn apply_shared_ground_drop_projection(
        &mut self,
        event_id: &str,
        drop: &GroundDropSnapshot,
    ) -> Result<Vec<ServerPacket>, String> {
        if self.has_shared_economy_projection_event(event_id) {
            self.persist_shared_economy_projection_event(event_id)?;
            return Ok(Vec::new());
        }
        if !self.can_commit_shared_ground_drop_pickup(drop) {
            return Err("ground-drop projection currently cannot fit".to_string());
        }
        self.apply_new_shared_economy_projection_atomically(event_id, |session| {
            let receipt = session.commit_shared_ground_drop_pickup_transaction(drop);
            if !receipt.committed {
                return Err("ground-drop projection application failed".to_string());
            }
            Ok(receipt.packets)
        })
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

    pub fn has_active_shared_trade_state(&self) -> bool {
        self.app
            .world()
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .trade
            .is_some()
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

    /// Materialize one side of a durably committed two-party trade and persist
    /// the external event marker in the same character snapshot. Replays are
    /// no-ops once the marker is durable; a same-process save retry only
    /// retries persistence and never applies the assets twice.
    pub fn apply_shared_trade_settlement_projection(
        &mut self,
        event_id: &str,
        own_offer: &SharedTradeOffer,
        incoming_offer: &SharedTradeOffer,
    ) -> Result<Vec<ServerPacket>, String> {
        if event_id.len() != 64
            || !event_id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("trade projection event ID must be lowercase SHA-256 hex".to_string());
        }
        if self.has_shared_economy_projection_event(event_id) {
            self.persist_shared_economy_projection_event(event_id)?;
            return Ok(Vec::new());
        }

        let packets = self.apply_new_shared_economy_projection_atomically(event_id, |session| {
            apply_shared_trade_settlement_projection(
                session.app.world_mut(),
                own_offer,
                incoming_offer,
            )
        })?;
        Ok(self.finalize_packets(packets))
    }

    /// Apply a newly seen durable economy projection only when its private
    /// character snapshot, including the event marker, can be saved. The
    /// projection functions fully preflight before their first mutation; once
    /// they do mutate, a save failure must restore the exact prior character
    /// checkpoint so a pending durable row can retry and emit its packets.
    fn apply_new_shared_economy_projection_atomically<F>(
        &mut self,
        event_id: &str,
        apply: F,
    ) -> Result<Vec<ServerPacket>, String>
    where
        F: FnOnce(&mut Self) -> Result<Vec<ServerPacket>, String>,
    {
        let checkpoint = self.active_character_checkpoint().ok_or_else(|| {
            "economy projection requires an active character checkpoint".to_string()
        })?;
        let dirty_before = self.dirty_economy_projection_event_ids.clone();
        let packets = apply(self)?;

        if let Err(save_error) = self.persist_shared_economy_projection_event(event_id) {
            self.restore_active_character_checkpoint(&checkpoint)
                .map_err(|restore_error| {
                    format!(
                        "economy projection persistence failed ({save_error}); checkpoint rollback failed ({restore_error})"
                    )
                })?;
            self.dirty_economy_projection_event_ids = dirty_before;
            self.dirty_economy_projection_event_ids.remove(event_id);
            return Err(save_error);
        }

        Ok(packets)
    }

    pub fn rollback_shared_trade_offer(&mut self, offer: &SharedTradeOffer) -> Vec<ServerPacket> {
        let packets = apply_shared_trade_offer(self.app.world_mut(), offer, true);
        self.finalize_packets(packets)
    }

    pub fn on_connect(&self) -> Vec<ServerPacket> {
        vec![ServerPacket::Connected]
    }

    pub fn standard_login_recovery_preflight(
        config: &SimulationConfig,
        account_id: &str,
        password: &str,
    ) -> Result<bool, String> {
        standard_login_recovery_preflight(config, account_id, password)
            .map(|result| matches!(result, RecoveryLoginPreflight::Eligible))
    }

    pub fn passkey_login_recovery_preflight(
        config: &SimulationConfig,
        account_id: &str,
    ) -> Result<PasskeyRecoveryPreflight, String> {
        passkey_login_recovery_preflight(config, account_id).map(|result| match result {
            RecoveryLoginPreflight::Eligible => PasskeyRecoveryPreflight::ExistingEligible,
            RecoveryLoginPreflight::Missing => PasskeyRecoveryPreflight::Missing,
            RecoveryLoginPreflight::Banned(_) | RecoveryLoginPreflight::Rejected => {
                PasskeyRecoveryPreflight::Rejected
            }
        })
    }

    /// Trusted Gateway-only provisioning boundary. The caller must have verified the
    /// external passkey token and must hold recovery-journal clearance for this identity.
    /// This capability is intentionally absent from WorldCommand and Zone RPC.
    pub fn provision_passkey_account(
        config: &SimulationConfig,
        account_id: &str,
    ) -> Result<(), String> {
        super::save::provision_passkey_account(config, account_id)
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
        let select_infos = account_select_infos(&config, account_id);
        let mut session = self.app.world_mut().resource_mut::<SessionResource>();
        session.account_id = Some(account_id.to_string());
        session.characters = characters;
        // A runtime starts with demo fixtures. Passkey login must not carry that
        // selected character into another account: StartGame persists any active
        // character before loading the requested slot.
        session.selected_character = None;
        session.clear_active_save_revision();
        vec![ServerPacket::LoginSuccess {
            characters: select_infos,
        }]
    }

    pub fn world_snapshot(&self) -> WorldSnapshot {
        build_world_snapshot(self.app.world())
    }

    pub fn current_map_shared_entity_snapshots(&self) -> Vec<WorldEntitySnapshot> {
        collect_current_map_shared_entity_snapshots(self.app.world())
    }

    pub fn local_player_object_id(&self) -> Option<u32> {
        let world = self.app.world();
        player_entity(world).and_then(|entity| entity_object_id(world, entity))
    }

    pub fn has_active_intelligent_creature_auto_pickup(&self) -> bool {
        self.app
            .world()
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .intelligent_creatures
            .iter()
            .any(|creature| {
                creature.pet_mode != 0
                    && creature.creature_rules.auto_pickup_enabled
                    && creature.fullness >= creature.creature_rules.minimal_fullness.max(0)
                    && creature.creature_rules.auto_pickup_range > 0
            })
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

    /// Select an account for trusted recovery-journal replay without accepting
    /// or retaining authentication material. This is deliberately not exposed
    /// through `ClientPacket`; callers must validate the journal first.
    pub fn select_account_for_recovery(&mut self, account_id: &str) -> Result<(), String> {
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        let characters = config
            .account_store
            .lock()
            .map_err(|_| "account store lock poisoned during recovery replay".to_string())?
            .accounts
            .get(account_id)
            .map(|account| account.characters.clone())
            .ok_or_else(|| "recovery account does not exist".to_string())?;
        let mut session = self.app.world_mut().resource_mut::<SessionResource>();
        session.account_id = Some(account_id.to_string());
        session.characters = characters;
        session.selected_character = None;
        session.clear_active_save_revision();
        Ok(())
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
        let player_runtime = self.app.world().resource::<PlayerRuntimeResource>();
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
                active_guild_wars: stage5.stage5_systems.guild.active_wars.clone(),
                blocked_names: stage5.stage5_systems.social.blocked.clone(),
                mentor_name,
                relationship_name,
                is_gm: false,
                free_map_shout: permissions.free_map_shout,
                free_server_shout: permissions.free_server_shout,
                attack_mode: stage5.stage5_systems.attack_mode,
                pk_points: player_runtime.pk_points,
                in_safe_zone: snapshot.in_safe_zone,
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

    pub fn reconcile_current_map_monster_activation(&mut self) {
        if !is_in_world(self.app.world()) {
            return;
        }
        reconcile_monster_activation(self.app.world_mut());
    }

    pub fn force_authoritative_player_vitals(&mut self, hp: Option<i32>, mp: Option<i32>) {
        if (hp.is_none() && mp.is_none()) || !is_in_world(self.app.world()) {
            return;
        }
        let world = self.app.world_mut();
        let Some(player) = player_entity(world) else {
            return;
        };
        let updated_vitals = {
            let mut entity = world.entity_mut(player);
            entity.get_mut::<PlayerVitals>().map(|mut vitals| {
                if let Some(hp) = hp {
                    vitals.hp = hp.clamp(0, vitals.max_hp);
                }
                if let Some(mp) = mp {
                    vitals.mp = mp.clamp(0, vitals.max_mp);
                }
                *vitals
            })
        };
        if let Some(vitals) = updated_vitals {
            world.resource_mut::<PlayerRuntimeResource>().player_vitals = vitals;
            advance_runtime_tick(world);
        }
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

    pub fn apply_zone_player_damage(&mut self, damage: i32) -> bool {
        if damage <= 0 || !is_in_world(self.app.world()) {
            return false;
        }
        let world = self.app.world_mut();
        let outcome = apply_damage_to_current_player(world, damage, &mut Vec::new());
        if outcome.applied {
            advance_runtime_tick(world);
        }
        outcome.died
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

    pub fn apply_zone_unlawful_player_kill(&mut self, points: i32) -> i32 {
        if points <= 0 || !is_in_world(self.app.world()) {
            return 0;
        }
        let world = self.app.world_mut();
        let pk_points = {
            let mut runtime = world.resource_mut::<PlayerRuntimeResource>();
            runtime.pk_points = runtime.pk_points.saturating_add(points);
            runtime.pk_points
        };
        advance_runtime_tick(world);
        pk_points
    }

    pub fn zone_player_name_colour_argb(&self) -> i32 {
        if !is_in_world(self.app.world()) {
            return -1;
        }
        current_player_name_colour_argb(self.app.world())
    }

    pub fn apply_zone_player_death_penalty(&mut self) -> Vec<ServerPacket> {
        if !is_in_world(self.app.world()) {
            return Vec::new();
        }
        let packets = drop_player_death_penalty(self.app.world_mut());
        if !packets.is_empty() {
            advance_runtime_tick(self.app.world_mut());
        }
        packets
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

    /// Apply authoritative shared-Zone monster incarnation boundaries to the
    /// personal Crystal compatibility runtime. The Zone owns combat, the
    /// wall-clock respawn schedule, and the public harvest lifecycle; Session
    /// retains only the personal Crystal harvest/drop projection.
    /// Mirroring death prevents the still-live private entity from immediately
    /// respawning a Zone corpse; mirroring explicit revive clears the previous
    /// incarnation's harvest state.
    pub fn apply_shared_monster_lifecycle_packets(&mut self, packets: &[ServerPacket]) {
        if !is_in_world(self.app.world()) {
            return;
        }
        let world = self.app.world_mut();
        for packet in packets {
            let object_id = match packet {
                ServerPacket::ObjectDied { info } => info.object_id,
                ServerPacket::ObjectHealth { info } if info.percent == 0 => info.object_id,
                ServerPacket::ObjectRevived { info } => info.object_id,
                _ => continue,
            };
            let Some(entity) = entity_by_object_id(world, object_id) else {
                continue;
            };
            if !world.entity(entity).contains::<Monster>() {
                continue;
            }
            match packet {
                ServerPacket::ObjectDied { info } => {
                    apply_shared_monster_death_state(
                        world,
                        entity,
                        Some(&info.location),
                        Some(info.direction),
                    );
                }
                ServerPacket::ObjectHealth { .. } => {
                    apply_shared_monster_death_state(world, entity, None, None);
                }
                ServerPacket::ObjectRevived { .. } => {
                    apply_shared_monster_revive_state(world, entity);
                }
                _ => {}
            }
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
            ServerPacket::SpellToggle {
                object_id,
                spell: Spell::CounterAttack,
                can_use: false,
            } if local_player_object_id.is_some_and(|local_object_id| {
                *object_id == local_object_id || *object_id == zone_object_id
            }) =>
            {
                set_skill_toggle_state(self.app.world_mut(), Spell::CounterAttack, false);
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

    pub fn shared_skill_item_consumption_components(
        &self,
        spell: Spell,
    ) -> Option<Vec<SharedSkillItemConsumptionComponent>> {
        if !is_in_world(self.app.world()) {
            return None;
        }
        super::skills::zone_magic_inventory_components(self.app.world(), spell)
    }

    pub fn shared_skill_item_param(&self, spell: Spell) -> u8 {
        if !is_in_world(self.app.world()) {
            return 0;
        }
        super::skills::zone_magic_inventory_item_param(self.app.world(), spell)
    }

    pub fn apply_shared_entity_snapshot(&mut self, snapshot: &WorldEntitySnapshot) -> bool {
        if snapshot.kind != WorldEntityKind::Monster || !is_in_world(self.app.world()) {
            return false;
        }
        let world = self.app.world_mut();
        let entity = match entity_by_object_id(world, snapshot.object_id) {
            Some(entity) => entity,
            None => match spawn_shared_monster_snapshot(world, snapshot) {
                Some(entity) => entity,
                None => return false,
            },
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
        if !snapshot.dead && !snapshot.hp.is_some_and(|hp| hp <= 0) {
            reset_shared_monster_harvest_state(world, entity);
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
        let is_conquest_battlefield_object = matches!(agent.ai, 80..=82);
        let object_id = entity_object_id(world, entity).unwrap_or(object_id);
        let max_hp = vitals.max_hp.max(1);

        Some(ZoneMonsterSpawn {
            object_id,
            name: name.clone(),
            name_colour_argb: -1,
            image: agent.image,
            ai: agent.ai,
            disposition: Some(agent.disposition),
            level: template.as_ref().map(|monster| monster.level).unwrap_or(1),
            max_hp,
            hp: vitals.hp.clamp(0, max_hp),
            experience: if is_conquest_battlefield_object {
                0
            } else {
                template
                    .as_ref()
                    .map(|monster| monster.experience)
                    .unwrap_or(0)
            },
            move_speed_ms: template
                .as_ref()
                .map(|monster| u64::from(monster.move_speed))
                .unwrap_or_else(|| agent.move_interval_ticks.saturating_mul(1_000)),
            attack_speed_ms: template
                .as_ref()
                .map(|monster| u64::from(monster.attack_speed))
                .unwrap_or_else(|| agent.attack_interval_ticks.saturating_mul(1_000)),
            friendly_guild: None,
            defense: template
                .as_ref()
                .map(zone_monster_defense_from_template)
                .unwrap_or_default(),
            position,
            direction,
            respawn: zone_monster_respawn_policy(world, entity),
            drops: if is_conquest_battlefield_object {
                Vec::new()
            } else {
                zone_ground_drop_snapshots_for_monster(world, object_id, &name)
            },
        })
    }
}

fn zone_monster_respawn_policy(
    world: &World,
    entity: bevy_ecs::entity::Entity,
) -> Option<ZoneMonsterRespawnPolicy> {
    let spawn_ref = world.entity(entity).get::<SpawnSlotRef>()?;
    let rule = world
        .get_resource::<MonsterSpawnTable>()?
        .rules
        .get(spawn_ref.rule_index)?;
    Some(zone_monster_respawn_policy_from_schedule(
        &rule.respawn_schedule,
        u32::try_from(spawn_ref.rule_index).ok()?,
        u32::try_from(spawn_ref.slot_index).ok()?,
    ))
}

fn zone_monster_respawn_policy_from_schedule(
    schedule: &MonsterRespawnSchedule,
    rule_index: u32,
    slot_index: u32,
) -> ZoneMonsterRespawnPolicy {
    let (
        minimum_delay_ms,
        base_delay_ms,
        random_delay_step_ms,
        random_delay_steps,
        random_delay_subtract_steps,
    ) = match schedule {
        MonsterRespawnSchedule::FixedTicks {
            delay_ticks,
            random_delay_ticks,
        } => (
            0,
            delay_ticks.saturating_mul(1_000),
            1_000,
            random_delay_ticks.saturating_add(1),
            0,
        ),
        MonsterRespawnSchedule::CrystalMinutes {
            delay_minutes,
            random_delay_minutes,
        } => {
            let steps = if *random_delay_minutes == 0 {
                1
            } else {
                u64::from(*random_delay_minutes).saturating_mul(2)
            };
            (
                60_000,
                u64::from(*delay_minutes).saturating_mul(60_000),
                60_000,
                steps,
                u64::from(*random_delay_minutes),
            )
        }
    };
    ZoneMonsterRespawnPolicy {
        minimum_delay_ms,
        base_delay_ms,
        random_delay_step_ms,
        random_delay_steps,
        random_delay_subtract_steps,
        rule_index,
        slot_index,
    }
}

#[cfg(test)]
mod zone_respawn_policy_tests {
    use super::*;
    use crate::config::ContentProfileRuntime;
    use mir2_game_data::crystal_respawn_manifest;

    #[test]
    fn platinum_evil_big_ape_uses_exact_d10_r30_crystal_wall_clock_profile() {
        let profile = ContentProfileRuntime::platinum_176();
        let respawn = crystal_respawn_manifest()
            .maps
            .into_iter()
            .find(|map| map.map_file_name == "D1002")
            .and_then(|map| {
                map.respawns
                    .into_iter()
                    .find(|respawn| respawn.monster_name == "EvilBigApe")
            })
            .expect("D1002 EvilBigApe production respawn");
        let random_delay_minutes = profile.monster_respawn_random_delay_minutes(
            &respawn.monster_name,
            respawn.random_delay_minutes,
        );
        assert_eq!(respawn.delay_minutes, 10);
        assert_eq!(random_delay_minutes, 30);
        let policy = zone_monster_respawn_policy_from_schedule(
            &MonsterRespawnSchedule::CrystalMinutes {
                delay_minutes: respawn.delay_minutes,
                random_delay_minutes,
            },
            7,
            3,
        );
        assert_eq!(policy.minimum_delay_ms, 60_000);
        assert_eq!(policy.base_delay_ms, 10 * 60_000);
        assert_eq!(policy.random_delay_steps, 60);
        assert_eq!(policy.random_delay_subtract_steps, 30);
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
        transaction_nonce: active.transaction_nonce.clone(),
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
    if !active.item_locked || validate_committed_item_state_carrier(item).is_err() {
        return None;
    }

    Some(SharedItemRentalItemOffer {
        transaction_nonce: active.transaction_nonce.clone(),
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
    if trade.completed
        || trade.settlement_nonce.len() != 32
        || !trade
            .settlement_nonce
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
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
        settlement_nonce: trade.settlement_nonce.clone(),
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
    let inventory_capacity = world.resource::<InventoryResource>().inventory_capacity;
    let mut delivered_items = Vec::new();
    for offered_item in &offer.items {
        let Ok(mut item) = serde_json::from_str::<ItemState>(&offered_item.item_state_json) else {
            return trade_offer_delivery_failed_packets(world, rollback);
        };
        if validate_committed_item_state_carrier(&item).is_err()
            || offered_item.key != item.key
            || offered_item.unique_id != item_unique_id(&item)
        {
            return trade_offer_delivery_failed_packets(world, rollback);
        }
        let Some((container, slot)) = preferred_or_empty_trade_delivery_slot_for_items(
            &staged_inventory,
            item.container,
            item.slot,
            inventory_capacity,
        ) else {
            return trade_offer_delivery_failed_packets(world, rollback);
        };
        item.container = container;
        item.slot = slot;
        normalize_incoming_item_tree_unique_ids(
            world.resource::<InventoryResource>(),
            &mut item,
            &staged_inventory,
        );
        let Ok(user_item) = try_user_item_from_item_state(&item) else {
            return trade_offer_delivery_failed_packets(world, rollback);
        };
        staged_inventory.push(item.clone());
        delivered_items.push((item, user_item));
    }

    if offer.gold > 0 {
        let mut player = world.resource_mut::<PlayerRuntimeResource>();
        player.gold = player.gold.saturating_add(offer.gold);
        packets.push(ServerPacket::GainedGold { gold: offer.gold });
    }

    for (item, user_item) in delivered_items {
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

fn apply_shared_trade_settlement_projection(
    world: &mut World,
    own_offer: &SharedTradeOffer,
    incoming_offer: &SharedTradeOffer,
) -> Result<Vec<ServerPacket>, String> {
    if !is_in_world(world) {
        return Err("trade projection requires an active character".to_string());
    }
    if own_offer.settlement_nonce.len() != 32
        || incoming_offer.settlement_nonce.len() != 32
        || own_offer.settlement_nonce == incoming_offer.settlement_nonce
        || !own_offer
            .settlement_nonce
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || !incoming_offer
            .settlement_nonce
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || !own_offer
            .partner_name
            .eq_ignore_ascii_case(&incoming_offer.character_name)
        || !incoming_offer
            .partner_name
            .eq_ignore_ascii_case(&own_offer.character_name)
    {
        return Err("trade projection offers are not a valid reciprocal pair".to_string());
    }

    let (active_account_id, active_character_index, active_character_name) = {
        let session = world.resource::<SessionResource>();
        let account_id = session
            .account_id
            .clone()
            .ok_or_else(|| "trade projection requires an active account".to_string())?;
        let character = session
            .selected_character
            .as_ref()
            .ok_or_else(|| "trade projection requires an active character".to_string())?;
        (account_id, character.index, character.name.clone())
    };
    if active_account_id != own_offer.account_id
        || active_character_index != own_offer.character_index
        || !active_character_name.eq_ignore_ascii_case(&own_offer.character_name)
    {
        return Err("trade projection identity does not match the active character".to_string());
    }

    let outgoing_already_debited = {
        let systems = world.resource::<Stage5SystemsResource>();
        match systems.stage5_systems.trade.as_ref() {
            Some(trade)
                if trade.settlement_nonce == own_offer.settlement_nonce
                    && trade
                        .partner
                        .eq_ignore_ascii_case(&incoming_offer.character_name) =>
            {
                trade.completed
            }
            Some(_) => {
                return Err(
                    "trade projection conflicts with a different durable trade state".to_string(),
                );
            }
            // A crash may restore a checkpoint from before the trade UI was
            // opened. Such a snapshot cannot contain the outgoing debit,
            // because debit and the matching completed trade state are saved
            // atomically in one CharacterSaveRecord.
            None => false,
        }
    };

    let current_gold = world.resource::<PlayerRuntimeResource>().gold;
    let gold_after_outgoing = if outgoing_already_debited {
        current_gold
    } else {
        current_gold
            .checked_sub(own_offer.gold)
            .ok_or_else(|| "trade projection outgoing gold is unavailable".to_string())?
    };
    let final_gold = gold_after_outgoing
        .checked_add(incoming_offer.gold)
        .ok_or_else(|| "trade projection incoming gold exceeds the character cap".to_string())?;

    let mut staged_inventory = world
        .resource::<InventoryResource>()
        .inventory_items
        .clone();
    let inventory_capacity = world.resource::<InventoryResource>().inventory_capacity;
    let mut outgoing_deleted_items = Vec::new();
    if !outgoing_already_debited {
        let mut outgoing_ids = BTreeSet::new();
        for offered_item in &own_offer.items {
            let item = staged_inventory
                .iter()
                .find(|item| item_unique_id(item) == offered_item.unique_id)
                .ok_or_else(|| "trade projection outgoing item is unavailable".to_string())?;
            if offered_item.key != item.key
                || validate_committed_item_state_carrier(item).is_err()
                || serde_json::to_string(item).ok().as_deref()
                    != Some(offered_item.item_state_json.as_str())
                || !outgoing_ids.insert(offered_item.unique_id)
            {
                return Err("trade projection outgoing item integrity mismatch".to_string());
            }
            let count = u16::try_from(item.quantity).map_err(|_| {
                "trade projection outgoing item quantity exceeds protocol range".to_string()
            })?;
            outgoing_deleted_items.push((offered_item.unique_id, count));
        }
        staged_inventory.retain(|item| !outgoing_ids.contains(&item_unique_id(item)));
    }

    let mut delivered_items = Vec::new();
    for offered_item in &incoming_offer.items {
        let mut item = serde_json::from_str::<ItemState>(&offered_item.item_state_json)
            .map_err(|error| format!("decode trade projection item: {error}"))?;
        if validate_committed_item_state_carrier(&item).is_err()
            || offered_item.key != item.key
            || offered_item.unique_id != item_unique_id(&item)
            || staged_inventory
                .iter()
                .any(|existing| item_unique_id(existing) == offered_item.unique_id)
        {
            return Err("trade projection incoming item integrity mismatch".to_string());
        }
        let (container, slot) = preferred_or_empty_trade_delivery_slot_for_items(
            &staged_inventory,
            item.container,
            item.slot,
            inventory_capacity,
        )
        .ok_or_else(|| "trade projection has no free inventory slot".to_string())?;
        item.container = container;
        item.slot = slot;
        let user_item = try_user_item_from_item_state(&item)
            .map_err(|error| format!("encode trade projection item: {error}"))?;
        staged_inventory.push(item.clone());
        delivered_items.push((item, user_item));
    }

    world.resource_mut::<PlayerRuntimeResource>().gold = final_gold;
    world.resource_mut::<InventoryResource>().inventory_items = staged_inventory;
    world
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .trade = None;

    let mut packets = Vec::new();
    if !outgoing_already_debited && own_offer.gold > 0 {
        packets.push(ServerPacket::LoseGold {
            gold: own_offer.gold,
        });
    }
    packets.extend(
        outgoing_deleted_items
            .into_iter()
            .map(|(unique_id, count)| ServerPacket::DeleteItem { unique_id, count }),
    );
    if incoming_offer.gold > 0 {
        packets.push(ServerPacket::GainedGold {
            gold: incoming_offer.gold,
        });
    }
    packets.extend(
        delivered_items
            .into_iter()
            .map(|(_, item)| ServerPacket::GainedItem { item }),
    );
    Ok(packets)
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

fn shared_rental_delivery_matches_active_state(
    world: &World,
    delivery: &SharedItemRentalDelivery,
) -> bool {
    let agreement = match delivery {
        SharedItemRentalDelivery::Lender(agreement)
        | SharedItemRentalDelivery::Borrower(agreement) => agreement,
    };
    if agreement.item.transaction_nonce.is_empty()
        || agreement.fee.transaction_nonce.is_empty()
        || agreement.item.transaction_nonce == agreement.fee.transaction_nonce
        || agreement.item.days == 0
        || agreement.item.days > 30
        || agreement.fee.fee == 0
        || !agreement
            .item
            .partner_name
            .eq_ignore_ascii_case(&agreement.fee.character_name)
        || !agreement
            .fee
            .partner_name
            .eq_ignore_ascii_case(&agreement.item.character_name)
    {
        return false;
    }

    let session = world.resource::<SessionResource>();
    let Some(account_id) = session.account_id.as_deref() else {
        return false;
    };
    let Some(character) = session.selected_character.as_ref() else {
        return false;
    };
    let rental = world.resource::<ItemRentalResource>();
    let Some(active) = rental.active.as_ref() else {
        return false;
    };

    match delivery {
        SharedItemRentalDelivery::Lender(_) => {
            active.transaction_nonce == agreement.item.transaction_nonce
                && account_id == agreement.item.account_id
                && character.index == agreement.item.character_index
                && character
                    .name
                    .eq_ignore_ascii_case(&agreement.item.character_name)
                && active
                    .partner_name
                    .eq_ignore_ascii_case(&agreement.fee.character_name)
                && active.days == agreement.item.days
                && active.item_locked
                && active.deposited_item.as_ref().is_some_and(|deposited| {
                    item_unique_id(deposited) == agreement.item.item_id
                        && deposited.name == agreement.item.item_name
                        && serde_json::to_string(deposited).ok().as_deref()
                            == Some(agreement.item.item_state_json.as_str())
                })
                && rental.rented_items.len() < 3
        }
        SharedItemRentalDelivery::Borrower(_) => {
            active.transaction_nonce == agreement.fee.transaction_nonce
                && account_id == agreement.fee.account_id
                && character.index == agreement.fee.character_index
                && character
                    .name
                    .eq_ignore_ascii_case(&agreement.fee.character_name)
                && active
                    .partner_name
                    .eq_ignore_ascii_case(&agreement.item.character_name)
                && active.fee == agreement.fee.fee
                && active.gold_locked
                && active.deposited_item.is_none()
                && !rental.has_rented_item
        }
    }
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
    let Ok(agreement_item) = serde_json::from_str::<ItemState>(&agreement.item.item_state_json)
    else {
        return vec![ServerPacket::CancelItemRental];
    };
    if validate_committed_item_state_carrier(&agreement_item).is_err()
        || agreement.item.item_id != item_unique_id(&agreement_item)
        || agreement.item.item_name != agreement_item.name
        || !shared_rental_delivery_matches_active_state(world, delivery)
    {
        return vec![ServerPacket::CancelItemRental];
    }
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
            let mut item = agreement_item;
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
            normalize_incoming_item_tree_unique_ids(
                world.resource::<InventoryResource>(),
                &mut item,
                &[],
            );
            if validate_committed_item_state_carrier(&item).is_err() {
                return vec![ServerPacket::CancelItemRental];
            }

            let Ok(mut loan_item) = try_user_item_from_item_state(&item) else {
                return vec![ServerPacket::CancelItemRental];
            };
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
        inventory.inventory_capacity,
    )
}

fn preferred_or_empty_trade_delivery_slot_for_items(
    items: &[ItemState],
    preferred_container: ItemContainer,
    preferred_slot: u8,
    inventory_capacity: u16,
) -> Option<(ItemContainer, u8)> {
    if matches!(
        preferred_container,
        ItemContainer::Bag1 | ItemContainer::Bag2
    ) {
        let logical_slot = match preferred_container {
            ItemContainer::Bag1 => preferred_slot,
            ItemContainer::Bag2 => 40u8.saturating_add(preferred_slot),
            _ => unreachable!(),
        };
        if is_valid_inventory_slot(logical_slot, inventory_capacity)
            && !items
                .iter()
                .any(|item| item.container == preferred_container && item.slot == preferred_slot)
        {
            return Some((preferred_container, preferred_slot));
        }
    }
    find_empty_inventory_item_slot(items, ItemContainer::Bag1, inventory_capacity)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
