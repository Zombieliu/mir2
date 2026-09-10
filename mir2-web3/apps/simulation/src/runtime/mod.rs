mod big_map;
mod buffs;
mod combat;
mod components;
mod crystal_compat;
mod door;
mod drops;
mod equipment;
mod experience_rates;
pub use experience_rates::apply_crystal_experience_rates;
mod social_experience;
mod player_weights;
pub use social_experience::{crystal_social_experience_peer_eligible, ExperienceCharacter, ExperiencePresence};
mod fishing;
mod gm_commands;
mod hazard;
mod hero_ai;
mod hero_inventory;
mod hero_registry_validation;
mod hero_registry_carriers;
mod hero_registry_scan;
pub(crate) use hero_registry_scan::validate_shared_hero_physical_custody;
pub(crate) use hero_registry_validation::validate_shared_hero_carriers;
mod hero_stats;
mod inventory;
mod item_custody;
mod item_sets;
mod items;
mod leveling;
mod map;
mod map_events;
mod mining;
mod monster_ai;
mod monsters;
mod movement;
mod npc;
mod npc_script;
mod onchain;
mod packets;
mod pathfind;
mod quests;
mod refine_oven;
mod rental;
mod resources;
mod save;
mod session;
mod shared_guilds;
pub(crate) use shared_guilds::buffs::advance_minutes as advance_shared_guild_minutes;
mod shared_marriage;
mod shared_relationships;
mod shared_social_buffs;
pub use shared_marriage::{
    validate_shared_marriage_request, SharedMarriageMutation, SharedMarriageProfile,
};
pub use shared_relationships::{
    validate_shared_mentor_request, SharedMentorMutation, SharedMentorProfile,
};
mod skills;
mod social_economy;
mod stage5;
mod stats;
pub mod zone;

/// Crystal's symmetric object-data/AOI radius around the local player.
pub const CRYSTAL_OBJECT_DATA_RANGE: i32 = crystal_compat::CRYSTAL_DATA_RANGE;

pub use drops::{
    SharedAccountInventoryTransactionKind, SharedAccountInventoryTransactionReceipt,
    SharedGroundDropPickupCommit, SharedInventoryItemDrop,
};
pub use map::set_crystal_full_world_zone_collision;
pub use monsters::crystal_world_respawn_spawns;
pub use npc_script::CrystalNpcSavedValue as SharedNpcSavedValue;
pub use packets::{ChatPacketPreparation, PreparedChatPacket};
pub use save::{reset_account_password_after_recovery, validate_commercial_identity_credentials};
pub use session::{
    ActiveSessionIdentity, PasskeyRecoveryPreflight, SharedItemRentalAgreement,
    SharedItemRentalDelivery, SharedItemRentalFeeOffer, SharedItemRentalItemOffer,
    SharedSkillItemConsumptionComponent, SharedTradeOffer, SharedTradeOfferItem, SimulationSession,
};
pub use stage5::{GameShopPurchaseExecution, GameShopPurchaseFailure, GameShopPurchaseOutcome};
pub use zone::{
    CreatureOperation, CreatureOwner, CreaturePickupIntent,
    gate5_demo_scenario, run_zone_replay_scenario, zone_id_for_key, GroundDropClaimTicket,
    PlayerId, SessionId, ZoneBossRewardAudit, ZoneBounds, ZoneChatItem, ZoneChatProfile,
    ZoneCollision, ZoneCommand, ZoneInput, ZoneJoin, ZoneKey, ZoneManager, ZoneMapMetadata,
    ZoneMonsterDefense, ZoneMonsterKillAward, ZoneMonsterRespawnPolicy, ZoneMonsterSpawn,
    ZoneNativeMonsterSnapshot, ZoneNpcTeleportConfig, ZoneNpcTeleportDestination, ZoneOutbound,
    ZoneOutput, ZonePlayerCombatStats, ZoneReplayCombatStats, ZoneReplayCommand, ZoneReplayEngine,
    ZoneReplayReport, ZoneReplayScenario, ZoneReplicaCheckpoint, ZoneRuntime, ZoneStandbyReplica,
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
pub use shared_mentor_rewards::{
    SharedMentorAccountingReceipt, SharedMentorBreakReason, SharedMentorLiveCheckpoint,
};

mod intelligent_creatures;

#[cfg(test)]
mod intelligent_creature_item_tests;

mod shared_guild_experience;
pub use shared_guild_experience::SharedMonsterKillCommitFailure;
