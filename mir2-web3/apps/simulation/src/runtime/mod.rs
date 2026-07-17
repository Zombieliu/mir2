mod buffs;
mod combat;
mod components;
mod crystal_compat;
mod door;
mod drops;
mod equipment;
mod fishing;
mod gm_commands;
mod hazard;
mod hero_ai;
mod inventory;
mod items;
mod leveling;
mod map;
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
mod rental;
mod resources;
mod save;
mod session;
mod skills;
mod social_economy;
mod stage5;
mod stats;
pub mod zone;

/// Crystal's symmetric object-data/AOI radius around the local player.
pub const CRYSTAL_OBJECT_DATA_RANGE: i32 = crystal_compat::CRYSTAL_DATA_RANGE;

pub use drops::{
    SharedAccountInventoryTransactionKind, SharedAccountInventoryTransactionReceipt,
    SharedGroundDropPickupCommit,
};
pub use map::set_crystal_full_world_zone_collision;
pub use npc_script::CrystalNpcSavedValue as SharedNpcSavedValue;
pub use packets::{ChatPacketPreparation, PreparedChatPacket};
pub use session::{
    ActiveSessionIdentity, SharedItemRentalAgreement, SharedItemRentalDelivery,
    SharedItemRentalFeeOffer, SharedItemRentalItemOffer, SharedTradeOffer, SimulationSession,
};
pub use zone::{
    PlayerId, SessionId, ZoneBounds, ZoneChatItem, ZoneChatProfile, ZoneCollision, ZoneCommand,
    ZoneJoin, ZoneKey, ZoneManager, ZoneMonsterDefense, ZoneMonsterKillAward, ZoneMonsterSpawn,
    ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
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
