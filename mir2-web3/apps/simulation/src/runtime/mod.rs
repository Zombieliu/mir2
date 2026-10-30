mod big_map;
mod billing;
mod billing_monthly_card;
mod buffs;
mod combat;
mod components;
mod crystal_compat;
mod door;
mod drops;
mod equipment;
mod experience_rates;
pub use experience_rates::apply_crystal_experience_rates;
mod player_weights;
mod prepared_player_drop;
mod zone_source_recovery;
pub(crate) fn validate_zone_source_recovery_checkpoint(
    save: &crate::CharacterSaveRecord,
) -> Result<(), String> {
    save::validate_character_save_record(save)
}
pub(crate) fn validate_player_drop_exact_item(item: &mir2_protocol::UserItem) -> Result<(), String> {
    items::validate_committed_user_item_carrier(item).map_err(|error| error.to_string())
}
pub(crate) fn validate_player_drop_recorded_request(
    request: &crate::PlayerDropRequest,
    operation: &crate::PlayerDropPreparedOperation,
) -> Result<(), String> {
    prepared_player_drop::validate_recorded_request(request, operation)
}
mod social_experience;
pub use social_experience::{
    crystal_social_experience_peer_eligible, ExperienceCharacter, ExperiencePresence,
};
mod fishing;
mod gm_commands;
mod hazard;
mod hero_ai;
mod hero_inventory;
mod hero_registry_carriers;
mod hero_registry_scan;
mod hero_registry_validation;
pub(crate) use hero_registry_scan::validate_shared_hero_physical_custody;
pub(crate) use hero_registry_validation::validate_shared_hero_carriers;
#[cfg(test)]
mod equipment_storage_merge_tests;
mod hero_stats;
mod inventory;
mod item_custody;
mod item_grants;
mod item_sets;
pub(crate) mod item_uid_issuance;
mod items;
mod leveling;
mod map;
mod map_events;
mod mining;
mod mining_player;
mod monster_ai;
mod monsters;
mod movement;
mod npc;
mod npc_conquest;
mod npc_conquest_trade;
mod npc_gold_buy_outcome;
mod npc_input;
mod npc_population;
mod source_map_io;
pub(crate) mod source_map_loading;
pub use npc_gold_buy_outcome::{
    NpcGoldBuyBeforeExecution, NpcGoldBuyProcessingError, NpcGoldBuyProcessingExecution,
    NpcGoldBuyProcessingOutcome, NpcGoldBuyRejection, NpcGoldBuyRequest,
};
mod npc_purchase_outcome;
mod npc_purchase_snapshot;
mod npc_purchase_transaction;
mod production;
pub(crate) mod procurement;
mod stones;
mod stone_npc;
pub use stone_npc::StoneWorkshopError;
pub use production::{
    PersonalProductionConfig, PersonalProductionFacility, PersonalProductionState,
    ProductionDurableError, ProductionDurableExecution,
};
pub use npc_purchase_outcome::{
    NpcPurchaseBeforeExecution, NpcPurchaseCurrency, NpcPurchaseProcessingError,
    NpcPurchaseProcessingExecution, NpcPurchaseProcessingOutcome, NpcPurchaseRejection,
    NpcPurchaseRequest, NpcPurchaseSource,
};
pub use npc_purchase_transaction::{
    NpcPurchaseDurableError, NpcPurchaseDurableExecution, NpcPurchaseProducer, NpcPurchaseReceipt,
};
#[cfg(test)]
mod npc_buff_action_tests;
#[cfg(test)]
mod npc_catalog_lookup_tests;
#[cfg(test)]
mod npc_dialog_admission_tests;
mod npc_gold_trade_expiry;
#[cfg(test)]
mod npc_parser_omission_tests;
#[cfg(test)]
mod npc_random_source_guard_tests;
mod npc_script;
#[cfg(test)]
mod npc_source_guard_tests;
#[cfg(test)]
mod npc_spawn_action_tests;
mod npc_spawn_actions;
mod npc_world_plan;
mod onchain;
mod packets;
mod pathfind;
mod quests;
#[cfg(test)]
mod storage_rental_tests;
pub(crate) use quests::periodic_quests::target_is_periodic as periodic_quest_target;
mod ranking_inspect;
mod recovery;
pub use ranking_inspect::{RankingInspectIdentity, RankingInspectOnlineProjection};

/// Server-owned presence, never a field supplied by a player packet. An online
/// owner without a current projection must not fall back to its saved equipment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RankingInspectPresence {
    Offline,
    Online(Option<RankingInspectOnlineProjection>),
    Unknown,
}

mod refine_oven;
mod rental;
mod resources;
pub use recovery::RecoveryCharacterLoadError;
mod save;
mod session;
mod shared_guilds;
pub(crate) use shared_guilds::buffs::advance_minutes as advance_shared_guild_minutes;
mod default_npc_events;
#[cfg(test)]
mod default_npc_events_tests;
mod npc_shared_guild_actions;
mod shared_experience_profile;
mod shared_group_projection;
mod shared_marriage;
mod shared_pet_progress;
mod shared_player_kills;
mod shared_relationships;
pub use default_npc_events::{
    DefaultNpcEnterMapTicket, DefaultNpcEvent, DefaultNpcEventSnapshot, QueuedDefaultNpcEvent,
};
#[cfg(test)]
mod refine_durable_tests;
#[cfg(test)]
mod shared_player_kill_tests;
mod shared_social_buffs;
pub use shared_marriage::{
    validate_shared_marriage_request, SharedMarriageMutation, SharedMarriageProfile,
};
pub use shared_relationships::{
    validate_shared_mentor_request, SharedMentorMutation, SharedMentorProfile,
    SHARED_MENTOR_DURATION_MS, SHARED_MENTOR_LEVEL_GAP,
};
mod game_shop_gift;
mod skills;
mod social_economy;
mod stage5;
mod stats;
pub mod zone;
pub use zone::ZoneMentorBankAttribution;
pub use zone::{PlayerDropApplyResult, PlayerDropCustodyState, PlayerDropEffectIdentity,
    PlayerDropPlacementKind, PlayerDropPlacementReservation};
pub use zone::{ZoneConquestDefenseSample, ZoneConquestMembership, ZoneConquestPlayerSample};

/// Crystal's symmetric object-data/AOI radius around the local player.
pub const CRYSTAL_OBJECT_DATA_RANGE: i32 = crystal_compat::CRYSTAL_DATA_RANGE;

pub use drops::{
    SharedAccountInventoryTransactionKind, SharedAccountInventoryTransactionReceipt,
    SharedGroundDropPickupCommit, SharedInventoryItemDrop,
};
pub use map::set_crystal_full_world_zone_collision;
pub use monsters::crystal_world_respawn_spawns;
pub use npc_script::CrystalNpcSavedValue as SharedNpcSavedValue;
pub use packets::{ChatPacketPreparation, PreparedChatPacket, RankingInspectRequest};
pub use save::{reset_account_password_after_recovery, validate_commercial_identity_credentials};
pub use session::{
    ActiveSessionIdentity, LocalPlayerVitalsSnapshot, PasskeyRecoveryPreflight,
    SharedItemRentalAgreement, SharedItemRentalDelivery, SharedItemRentalFeeOffer,
    SharedItemRentalItemOffer, SharedSkillItemConsumptionComponent, SharedTradeOffer,
    SharedTradeOfferItem, SimulationSession,
};
pub use stage5::{GameShopPurchaseExecution, GameShopPurchaseFailure, GameShopPurchaseOutcome};
pub use zone::{
    gate5_demo_scenario, run_zone_replay_scenario, zone_id_for_key, CellCoord, CellSize,
    CreatureOperation, CreatureOwner, CreaturePickupIntent, GroundDropClaimTicket,
    MapEffectOwnerRef, PartitionId, PartitionKey, PartitionLocation, PlayerId, SessionId,
    TopologyRevision, ZonePartitionDescriptor,
    ZoneBossRewardAudit, ZoneBounds, ZoneChatItem, ZoneChatProfile, ZoneCollision, ZoneCommand,
    ZoneExperiencePartner, ZoneExperienceProfile, ZoneExperienceRateBuff, ZoneExperienceRateSource,
    ZoneExperienceRateStat, ZoneExperienceSelection, ZoneFiveClassPracticeReceipt,
    ZoneFiveClassPracticeSpell, ZoneGuildExperienceMembership, ZoneInput, ZoneJoin,
    ZoneJourneyEventKind, ZoneJourneyEventReceipt, ZoneJourneyPhysicalTechnique, ZoneKey,
    ZoneMagicPracticeReceipt, ZoneMagicPracticeSpell, ZoneManaSettlement, ZoneManager,
    ZoneMapMetadata, ZoneMinedOre, ZoneMiningSwing, ZoneMiningTool, ZoneMonsterDefense,
    ZoneMonsterKillAward, ZoneMonsterRespawnPolicy, ZoneMonsterSpawn, ZoneNativeMonsterSnapshot,
    ZoneNpcPopulationReadSet, ZoneNpcTeleportConfig, ZoneNpcTeleportDestination, ZoneOutbound,
    ZoneOutput, ZoneOwnedPetPlayerKillReceipt, ZoneOwnerHealthChange, ZoneOwnerHealthCursor,
    ZonePetExperienceAdmission, ZonePlayerAppearance, ZonePlayerCombatStats, ZoneReplayCombatStats,
    ZoneReplayCommand, ZoneReplayEngine, ZoneReplayReport, ZoneReplayScenario,
    ZoneReplicaCheckpoint, ZoneRuntime, ZoneSavedPetSnapshot, ZoneStandbyReplica,
    ZoneVitalSettlement,
};

pub fn zone_ground_drop_snapshots_for_monster_at_tick(
    monster_object_id: u32,
    monster_name: &str,
    current_tick: u64,
) -> Vec<crate::config::GroundDropSnapshot> {
    drops::zone_ground_drop_snapshots_for_monster_at_tick(
        monster_object_id,
        monster_name,
        current_tick,
    )
}

pub fn intelligent_creature_allows_ground_drop(
    creature: &mir2_protocol::ClientIntelligentCreature,
    drop: &crate::config::GroundDropSnapshot,
) -> bool {
    match &drop.loot {
        crate::config::GroundDropLootSnapshot::Gold { .. } => {
            creature.filter.pet_pickup_all || creature.filter.pet_pickup_gold
        }
        crate::config::GroundDropLootSnapshot::InventoryItem { key, .. } => {
            if creature.pickup_grade > 0
                && drops::crystal_item_grade_for_key(key) < creature.pickup_grade
            {
                return false;
            }
            if creature.filter.pet_pickup_all {
                return true;
            }
            let Some(template) = items::crystal_item_template_for_item_key(key) else {
                return creature.filter.pet_pickup_others;
            };
            match template.item_type {
                crystal_compat::CRYSTAL_ITEM_TYPE_WEAPON => creature.filter.pet_pickup_weapons,
                crystal_compat::CRYSTAL_ITEM_TYPE_ARMOUR => creature.filter.pet_pickup_armours,
                crystal_compat::CRYSTAL_ITEM_TYPE_HELMET => creature.filter.pet_pickup_helmets,
                crystal_compat::CRYSTAL_ITEM_TYPE_BOOTS => creature.filter.pet_pickup_boots,
                crystal_compat::CRYSTAL_ITEM_TYPE_BELT => creature.filter.pet_pickup_belts,
                crystal_compat::CRYSTAL_ITEM_TYPE_NECKLACE
                | crystal_compat::CRYSTAL_ITEM_TYPE_BRACELET
                | crystal_compat::CRYSTAL_ITEM_TYPE_RING => creature.filter.pet_pickup_accessories,
                _ => creature.filter.pet_pickup_others,
            }
        }
    }
}

#[cfg(test)]
mod mentor_reward_tests;

mod shared_mentor_rewards;
pub use shared_mentor_rewards::{SharedMentorAccountingReceipt, SharedMentorBreakReason};

mod intelligent_creatures;

#[cfg(test)]
mod intelligent_creature_item_tests;

#[cfg(test)]
mod session_lightweight_snapshot_tests;

mod shared_guild_experience;
pub use shared_guild_experience::SharedMonsterKillCommitFailure;
