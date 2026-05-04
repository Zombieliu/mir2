use crate::config::BuffSnapshot;
use bevy_ecs::prelude::World;
use mir2_game_data::{
    localized_text_or_fallback, starter_server_data, CrystalItemTemplate, LanguageCode,
};
use mir2_protocol::{ServerPacket, UserItemStat};

use super::components::{player_entity, PlayerVitals};
use super::crystal_compat::*;
use super::items::{crystal_item_stat_value, user_item_stat_total};
use super::packets::object_health_info_for_entity;
use super::resources::{BuffResource, PlayerRuntimeResource, PotionRecoveryResource};

#[derive(Debug, Clone)]
pub(super) struct BuffState {
    pub(super) key: String,
    pub(super) name: String,
    pub(super) description: String,
    pub(super) expires_at_tick: u64,
    pub(super) attack_bonus: i32,
    pub(super) defence_bonus: i32,
    pub(super) stats: Vec<UserItemStat>,
}

impl BuffState {
    pub(super) fn snapshot(&self, tick: u64, language: LanguageCode) -> BuffSnapshot {
        BuffSnapshot {
            key: self.key.clone(),
            name: localized_buff_name(language, &self.key, &self.name),
            description: localized_buff_description(language, &self.key, &self.description),
            remaining_ticks: self.expires_at_tick.saturating_sub(tick) as u32,
            attack_bonus: buff_attack_bonus(self),
            defence_bonus: buff_defence_bonus(self),
        }
    }
}

pub(super) fn localized_buff_base_key(key: &str) -> Option<&'static str> {
    match key {
        "battle-focus" => Some("content.buff.battleFocus"),
        _ => None,
    }
}

pub(super) fn localized_buff_name(language: LanguageCode, key: &str, fallback: &str) -> String {
    localized_buff_base_key(key)
        .map(|base| localized_text_or_fallback(language, &format!("{base}.name"), fallback))
        .unwrap_or_else(|| fallback.to_string())
}

pub(super) fn localized_buff_description(
    language: LanguageCode,
    key: &str,
    fallback: &str,
) -> String {
    localized_buff_base_key(key)
        .map(|base| localized_text_or_fallback(language, &format!("{base}.description"), fallback))
        .unwrap_or_else(|| fallback.to_string())
}

pub(super) fn buff_attack_bonus(buff: &BuffState) -> i32 {
    buff.attack_bonus
        .max(user_item_stat_total(&buff.stats, CRYSTAL_STAT_MAX_DC))
}

pub(super) fn buff_defence_bonus(buff: &BuffState) -> i32 {
    buff.defence_bonus
        .max(user_item_stat_total(&buff.stats, CRYSTAL_STAT_MAX_AC))
}

pub(super) fn buff_metadata(
    key: &str,
    fallback_name: &str,
    fallback_description: &str,
) -> (String, String) {
    if let Some(buff) = starter_server_data()
        .buffs
        .into_iter()
        .find(|buff| buff.key == key)
    {
        (buff.name, buff.description)
    } else {
        (fallback_name.to_string(), fallback_description.to_string())
    }
}

pub(super) fn apply_or_refresh_buff(world: &mut World, next: BuffState) {
    let mut buffs = world.resource_mut::<BuffResource>();
    if let Some(existing) = buffs.buffs.iter_mut().find(|buff| buff.key == next.key) {
        *existing = next;
    } else {
        buffs.buffs.push(next);
    }
}

pub(super) fn tick_buffs(world: &mut World) {
    let tick = super::session::runtime_tick(world);
    world
        .resource_mut::<BuffResource>()
        .buffs
        .retain(|buff| buff.expires_at_tick > tick);
}

pub(super) fn restore_current_player_vitals(world: &mut World, heal_hp: i32, heal_mp: i32) {
    let Some(player) = player_entity(world) else {
        return;
    };

    let restored_vitals = {
        let mut entity = world.entity_mut(player);
        let mut vitals = entity.get_mut::<PlayerVitals>().expect("player vitals");
        vitals.hp = (vitals.hp + heal_hp.max(0)).min(vitals.max_hp);
        vitals.mp = (vitals.mp + heal_mp.max(0)).min(100);
        *vitals
    };

    world.resource_mut::<PlayerRuntimeResource>().player_vitals = restored_vitals;
}

pub(super) fn queue_crystal_normal_potion_restore(
    world: &mut World,
    template: &CrystalItemTemplate,
) -> bool {
    let hp = crystal_item_stat_value(template, CRYSTAL_STAT_HP).max(0);
    let mp = crystal_item_stat_value(template, CRYSTAL_STAT_MP).max(0);
    queue_crystal_normal_potion_restore_amounts(world, hp, mp)
}

pub(super) fn queue_crystal_normal_potion_restore_amounts(
    world: &mut World,
    hp: i32,
    mp: i32,
) -> bool {
    if hp == 0 && mp == 0 {
        return false;
    }

    let mut recovery = world.resource_mut::<PotionRecoveryResource>();
    recovery.pending_pot_health_amount = recovery
        .pending_pot_health_amount
        .saturating_add(hp)
        .min(i32::from(u16::MAX));
    recovery.pending_pot_mana_amount = recovery
        .pending_pot_mana_amount
        .saturating_add(mp)
        .min(i32::from(u16::MAX));
    true
}

pub(super) fn tick_crystal_normal_potion_restore(
    world: &mut World,
    packets: &mut Vec<ServerPacket>,
) {
    let (hp_tick, mp_tick) = {
        let mut recovery = world.resource_mut::<PotionRecoveryResource>();
        let hp_tick = recovery.pending_pot_health_amount.min(10);
        let mp_tick = recovery.pending_pot_mana_amount.min(10);
        recovery.pending_pot_health_amount -= hp_tick;
        recovery.pending_pot_mana_amount -= mp_tick;
        (hp_tick, mp_tick)
    };

    if hp_tick <= 0 && mp_tick <= 0 {
        return;
    }

    restore_current_player_vitals(world, hp_tick, mp_tick);
    if let Some(player) = player_entity(world) {
        if let Some(info) = object_health_info_for_entity(world, player, 0) {
            packets.push(ServerPacket::ObjectHealth { info });
        }
    }
}

pub(super) fn crystal_consumable_buff(
    key: &str,
    name: &str,
    description: &str,
    expires_at_tick: u64,
    stats: Vec<UserItemStat>,
) -> Option<BuffState> {
    if stats.is_empty() {
        return None;
    }

    Some(BuffState {
        key: key.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        expires_at_tick,
        attack_bonus: user_item_stat_total(&stats, CRYSTAL_STAT_MAX_DC),
        defence_bonus: user_item_stat_total(&stats, CRYSTAL_STAT_MAX_AC),
        stats,
    })
}

pub(super) fn crystal_template_consumable_buffs(
    template: &CrystalItemTemplate,
    current_tick: u64,
) -> Vec<BuffState> {
    let duration_ticks = u64::from(template.durability).saturating_mul(60);
    if duration_ticks == 0 {
        return Vec::new();
    }

    let expires_at_tick = current_tick.saturating_add(duration_ticks);
    let buff_from_stat = |key: &str, name: &str, description: &str, stat: u8| {
        crystal_consumable_buff(
            key,
            name,
            description,
            expires_at_tick,
            (crystal_item_stat_value(template, stat) > 0)
                .then(|| {
                    vec![UserItemStat {
                        stat,
                        value: crystal_item_stat_value(template, stat),
                    }]
                })
                .unwrap_or_default(),
        )
    };

    match template.shape {
        CRYSTAL_POTION_SHAPE_BUFF => [
            buff_from_stat(
                "impact",
                "Impact",
                "Crystal impact potion buff is active.",
                CRYSTAL_STAT_MAX_DC,
            ),
            buff_from_stat(
                "magic",
                "Magic",
                "Crystal magic potion buff is active.",
                CRYSTAL_STAT_MAX_MC,
            ),
            buff_from_stat(
                "taoist",
                "Taoist",
                "Crystal taoist potion buff is active.",
                CRYSTAL_STAT_MAX_SC,
            ),
            buff_from_stat(
                "storm",
                "Storm",
                "Crystal storm potion buff is active.",
                CRYSTAL_STAT_ATTACK_SPEED,
            ),
            buff_from_stat(
                "health-aid",
                "Health Aid",
                "Crystal health-aid potion buff is active.",
                CRYSTAL_STAT_HP,
            ),
            buff_from_stat(
                "mana-aid",
                "Mana Aid",
                "Crystal mana-aid potion buff is active.",
                CRYSTAL_STAT_MP,
            ),
            buff_from_stat(
                "defence",
                "Defence",
                "Crystal defence potion buff is active.",
                CRYSTAL_STAT_MAX_AC,
            ),
            buff_from_stat(
                "magic-defence",
                "Magic Defence",
                "Crystal magic-defence potion buff is active.",
                CRYSTAL_STAT_MAX_MAC,
            ),
        ]
        .into_iter()
        .flatten()
        .collect(),
        CRYSTAL_POTION_SHAPE_EXP => buff_from_stat(
            "exp",
            "EXP",
            "Crystal experience potion buff is active.",
            CRYSTAL_STAT_LUCK,
        )
        .into_iter()
        .collect(),
        CRYSTAL_POTION_SHAPE_DROP => buff_from_stat(
            "drop",
            "Drop",
            "Crystal drop-rate potion buff is active.",
            CRYSTAL_STAT_LUCK,
        )
        .into_iter()
        .collect(),
        _ => Vec::new(),
    }
}

pub(super) fn apply_or_stack_duration_buff(world: &mut World, next: BuffState) {
    let current_tick = super::session::runtime_tick(world);
    let mut buffs = world.resource_mut::<BuffResource>();
    let duration_ticks = next.expires_at_tick.saturating_sub(current_tick);
    if let Some(existing) = buffs.buffs.iter_mut().find(|buff| buff.key == next.key) {
        existing.expires_at_tick = existing
            .expires_at_tick
            .max(current_tick)
            .saturating_add(duration_ticks);
    } else {
        buffs.buffs.push(next);
    }
}

pub(super) fn apply_crystal_template_consumable_buffs(
    world: &mut World,
    template: &CrystalItemTemplate,
) -> bool {
    let tick = super::session::runtime_tick(world);
    let buffs = crystal_template_consumable_buffs(template, tick);
    if buffs.is_empty() {
        return false;
    }

    for buff in buffs {
        apply_or_stack_duration_buff(world, buff);
    }
    true
}
