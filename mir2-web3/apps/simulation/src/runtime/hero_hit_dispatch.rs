//! Hero-only impact adjudication; the shared combat queue receives an already resolved net hit.
use bevy_ecs::prelude::World;
use mir2_protocol::{ServerPacket,ObjectEffectInfo};
use super::hero_hit_math::{self,Attacker,Defender,Weights};
use super::super::{combat::{CrystalDefence,PendingCombatAction,PendingCombatTarget},components::{entity_name,ObjectId,MonsterCombatStats,MonsterPoisonState,NpcPetState},crystal_compat::*,resources::runtime_tick};
pub(super) fn resolve(world:&mut World,action:&mut PendingCombatAction,packets:&mut Vec<ServerPacket>)->bool {
    let PendingCombatTarget::Monster(target)=action.target else{return true;};
    let Some(id)=world.get::<ObjectId>(target).map(|id|id.0) else{return false;};
    let Some(name)=entity_name(world,target) else{return false;};
    let Some(base)=mir2_game_data::crystal_monster_by_name(&name) else{return false;};
    let stats=super::super::hero_stats::compute(world);
    let attacker=Attacker{accuracy:stats.get(CRYSTAL_STAT_ACCURACY),attack_bonus:stats.get(108),critical_rate:stats.get(CRYSTAL_STAT_CRITICAL_RATE),critical_damage:stats.get(CRYSTAL_STAT_CRITICAL_DAMAGE)};
    let pet_bonus=world.get::<NpcPetState>(target).map(|pet|i32::from(pet.pet_level)*2).unwrap_or(0);
    let poison=world.get::<MonsterPoisonState>(target).filter(|p|p.expires_at_tick>runtime_tick(world)).map(|p|p.poison).unwrap_or(0);
    // The audited v117 Server.MirDB has 555 monsters and no nonzero Stat.MagicResist.
    // Dynamic monster buff statistics are not represented by the current personal ECS.
    let defender=Defender{min_ac:base.min_ac.saturating_add(pet_bonus),max_ac:base.max_ac.saturating_add(pet_bonus),min_mac:base.min_mac.saturating_add(pet_bonus),max_mac:base.max_mac.saturating_add(pet_bonus),agility:world.get::<MonsterCombatStats>(target).map(|s|s.agility).unwrap_or(base.agility),magic_resist:0,armour_rate:if poison&2!=0{0.5}else{1.0},damage_rate:if poison&16!=0{1.5}else{1.0}};
    let weights=Weights{magic_resist:CRYSTAL_MAGIC_RESIST_WEIGHT as u32,critical_rate:CRYSTAL_CRITICAL_RATE_WEIGHT,critical_damage:CRYSTAL_CRITICAL_DAMAGE_WEIGHT};
    let hit=hero_hit_math::resolve(action.damage,action.defence,attacker,defender,weights,|bound|super::hero_combat_math::below(world,bound));
    if hit.damage<=0 {packets.push(ServerPacket::DamageIndicator{object_id:id,damage:0,damage_type:1});return false;}
    if hit.critical {
        packets.push(ServerPacket::ObjectEffect{info:ObjectEffectInfo{object_id:id,effect:11,effect_type:0,delay_time:0,time:0}});
        packets.push(ServerPacket::DamageIndicator{object_id:id,damage:0,damage_type:2});
    }
    action.damage=hit.damage;action.defence=CrystalDefence::None;true
}
