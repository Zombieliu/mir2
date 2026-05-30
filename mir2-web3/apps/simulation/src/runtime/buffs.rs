use crate::config::BuffSnapshot;
use bevy_ecs::prelude::World;
use mir2_game_data::{
    localized_text_or_fallback, starter_server_data, CrystalItemTemplate, LanguageCode,
};
use mir2_protocol::{ClientBuff, ServerPacket, UserItemStat};

use super::components::{hero_entity, player_entity, PlayerVitals};
use super::crystal_compat::*;
use super::items::{crystal_item_stat_value, user_item_stat_total};
use super::packets::{object_health_info_for_entity, object_mana_info_for_entity};
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

pub(super) fn tick_buffs(world: &mut World, packets: &mut Vec<ServerPacket>) {
    let tick = super::session::runtime_tick(world);
    let object_id = player_entity(world)
        .and_then(|player| {
            world
                .entity(player)
                .get::<super::components::ObjectId>()
                .copied()
        })
        .map(|object_id| object_id.0)
        .unwrap_or_default();
    let expired_buffs = world
        .resource::<BuffResource>()
        .buffs
        .iter()
        .filter(|buff| buff.expires_at_tick <= tick)
        .map(|buff| (buff.key.clone(), crystal_buff_type_for_key(&buff.key)))
        .collect::<Vec<_>>();
    world
        .resource_mut::<BuffResource>()
        .buffs
        .retain(|buff| buff.expires_at_tick > tick);
    for (key, buff_type) in expired_buffs {
        if let Some(buff_type) = buff_type {
            packets.push(ServerPacket::RemoveBuff {
                buff_type,
                object_id,
            });
        }
        if key == "concentration" {
            packets.push(ServerPacket::SetConcentration {
                object_id,
                enabled: false,
                interrupted: false,
            });
        }
        if matches!(key.as_str(), "hiding" | "moon-light" | "dark-body") {
            let still_hidden = world
                .resource::<BuffResource>()
                .buffs
                .iter()
                .any(|buff| matches!(buff.key.as_str(), "hiding" | "moon-light" | "dark-body"));
            if !still_hidden {
                packets.push(ServerPacket::ObjectHidden {
                    object_id,
                    hidden: false,
                });
            }
        }
    }
}

pub(super) fn client_buff_packet_for_state(
    world: &World,
    buff: &BuffState,
) -> Option<ServerPacket> {
    Some(ServerPacket::AddBuff {
        buff: client_buff_for_state(world, buff)?,
    })
}

pub(super) fn client_buff_for_state(world: &World, buff: &BuffState) -> Option<ClientBuff> {
    let tick = super::session::runtime_tick(world);
    let player = player_entity(world)?;
    let object_id = world.entity(player).get::<super::components::ObjectId>()?.0;
    let mut stats = buff.stats.clone();
    stats.sort_by_key(|stat| stat.stat);

    Some(ClientBuff {
        buff_type: crystal_buff_type_for_key(&buff.key)?,
        visible: crystal_buff_visible_for_key(&buff.key),
        object_id,
        expire_time: buff
            .expires_at_tick
            .saturating_sub(tick)
            .saturating_mul(1_000)
            .min(i64::MAX as u64) as i64,
        infinite: false,
        paused: false,
        stats,
        values: Vec::new(),
    })
}

pub(super) fn crystal_buff_type_for_key(key: &str) -> Option<u8> {
    Some(match key {
        "temporal-flux" => 1,
        "hiding" => 2,
        "haste" => 3,
        "swift-feet" => 4,
        "battle-focus" | "fury" => 5,
        "soul-shield" => 6,
        "blessed-armour" | "blessed-armor" => 7,
        "light-body" => 8,
        "ultimate-enhancer" => 9,
        "protection-field" => 10,
        "rage" => 11,
        "curse" => 12,
        "moon-light" => 13,
        "dark-body" => 14,
        "concentration" => 15,
        "vampire-shot" => 16,
        "poison-shot" => 17,
        "counter-attack" => 18,
        "mental-state" => 19,
        "energy-shield" => 20,
        "magic-booster" => 21,
        "pet-enhancer" => 22,
        "immortal-skin" => 23,
        "magic-shield" => 24,
        "elemental-barrier" => 25,
        "general" => 101,
        "exp" => 102,
        "drop" => 103,
        "gold" => 104,
        "bag-weight" => 105,
        "transform" => 106,
        "lover" => 107,
        "mentee" => 108,
        "mentor" => 109,
        "guild" => 110,
        "prison" => 111,
        "rested" => 112,
        "skill" => 113,
        "clear-ring" => 114,
        "newbie" => 115,
        "impact" => 200,
        "magic" => 201,
        "taoist" => 202,
        "storm" => 203,
        "health-aid" => 204,
        "mana-aid" => 205,
        "defence" => 206,
        "magic-defence" => 207,
        "wonder-drug" => 208,
        "knapsack" => 209,
        _ => return None,
    })
}

pub(super) fn crystal_buff_key_for_type(buff_type: u8) -> Option<&'static str> {
    Some(match buff_type {
        1 => "temporal-flux",
        2 => "hiding",
        3 => "haste",
        4 => "swift-feet",
        5 => "battle-focus",
        6 => "soul-shield",
        7 => "blessed-armour",
        8 => "light-body",
        9 => "ultimate-enhancer",
        10 => "protection-field",
        11 => "rage",
        12 => "curse",
        13 => "moon-light",
        14 => "dark-body",
        15 => "concentration",
        16 => "vampire-shot",
        17 => "poison-shot",
        18 => "counter-attack",
        19 => "mental-state",
        20 => "energy-shield",
        21 => "magic-booster",
        22 => "pet-enhancer",
        23 => "immortal-skin",
        24 => "magic-shield",
        25 => "elemental-barrier",
        101 => "general",
        102 => "exp",
        103 => "drop",
        104 => "gold",
        105 => "bag-weight",
        106 => "transform",
        107 => "lover",
        108 => "mentee",
        109 => "mentor",
        110 => "guild",
        111 => "prison",
        112 => "rested",
        113 => "skill",
        114 => "clear-ring",
        115 => "newbie",
        200 => "impact",
        201 => "magic",
        202 => "taoist",
        203 => "storm",
        204 => "health-aid",
        205 => "mana-aid",
        206 => "defence",
        207 => "magic-defence",
        208 => "wonder-drug",
        209 => "knapsack",
        _ => return None,
    })
}

fn crystal_buff_visible_for_key(key: &str) -> bool {
    matches!(
        key,
        "swift-feet"
            | "battle-focus"
            | "fury"
            | "moon-light"
            | "dark-body"
            | "vampire-shot"
            | "poison-shot"
            | "counter-attack"
            | "energy-shield"
            | "magic-booster"
            | "pet-enhancer"
            | "immortal-skin"
    )
}

pub(super) fn restore_current_player_vitals(world: &mut World, heal_hp: i32, heal_mp: i32) {
    let Some(player) = player_entity(world) else {
        return;
    };

    let restored_vitals = {
        let mut entity = world.entity_mut(player);
        let mut vitals = entity.get_mut::<PlayerVitals>().expect("player vitals");
        vitals.hp = (vitals.hp + heal_hp.max(0)).min(vitals.max_hp);
        vitals.mp = (vitals.mp + heal_mp.max(0)).min(vitals.max_mp);
        *vitals
    };

    world.resource_mut::<PlayerRuntimeResource>().player_vitals = restored_vitals;
}

pub(super) fn restore_current_hero_vitals(world: &mut World, heal_hp: i32, heal_mp: i32) -> bool {
    let Some(hero) = hero_entity(world) else {
        return false;
    };

    {
        let mut entity = world.entity_mut(hero);
        let Some(mut vitals) = entity.get_mut::<PlayerVitals>() else {
            return false;
        };
        vitals.hp = (vitals.hp + heal_hp.max(0)).min(vitals.max_hp);
        vitals.mp = (vitals.mp + heal_mp.max(0)).min(vitals.max_mp);
    }

    true
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

pub(super) fn queue_crystal_normal_hero_potion_restore_amounts(
    world: &mut World,
    hp: i32,
    mp: i32,
) -> bool {
    if hp == 0 && mp == 0 {
        return false;
    }

    let mut recovery = world.resource_mut::<PotionRecoveryResource>();
    recovery.hero_pending_pot_health_amount = recovery
        .hero_pending_pot_health_amount
        .saturating_add(hp)
        .min(i32::from(u16::MAX));
    recovery.hero_pending_pot_mana_amount = recovery
        .hero_pending_pot_mana_amount
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
        if mp_tick > 0 {
            if let Some(info) = object_mana_info_for_entity(world, player) {
                packets.push(ServerPacket::ObjectMana { info });
            }
        }
    }
}

pub(super) fn tick_crystal_normal_hero_potion_restore(
    world: &mut World,
    packets: &mut Vec<ServerPacket>,
) {
    let Some(hero) = hero_entity(world) else {
        return;
    };
    let (hp_tick, mp_tick) = {
        let mut recovery = world.resource_mut::<PotionRecoveryResource>();
        let hp_tick = recovery.hero_pending_pot_health_amount.min(10);
        let mp_tick = recovery.hero_pending_pot_mana_amount.min(10);
        recovery.hero_pending_pot_health_amount -= hp_tick;
        recovery.hero_pending_pot_mana_amount -= mp_tick;
        (hp_tick, mp_tick)
    };

    if hp_tick <= 0 && mp_tick <= 0 {
        return;
    }

    if !restore_current_hero_vitals(world, hp_tick, mp_tick) {
        return;
    }
    if let Some(info) = object_health_info_for_entity(world, hero, 0) {
        packets.push(ServerPacket::ObjectHealth { info });
    }
    if mp_tick > 0 {
        if let Some(info) = object_mana_info_for_entity(world, hero) {
            packets.push(ServerPacket::ObjectMana { info });
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

pub(super) fn apply_or_stack_duration_buff(world: &mut World, next: BuffState) -> BuffState {
    let current_tick = super::session::runtime_tick(world);
    let mut buffs = world.resource_mut::<BuffResource>();
    let duration_ticks = next.expires_at_tick.saturating_sub(current_tick);
    if let Some(existing) = buffs.buffs.iter_mut().find(|buff| buff.key == next.key) {
        existing.expires_at_tick = existing
            .expires_at_tick
            .max(current_tick)
            .saturating_add(duration_ticks);
        existing.clone()
    } else {
        buffs.buffs.push(next.clone());
        next
    }
}

pub(super) fn apply_crystal_template_consumable_buffs(
    world: &mut World,
    template: &CrystalItemTemplate,
) -> Vec<ServerPacket> {
    let tick = super::session::runtime_tick(world);
    let buffs = crystal_template_consumable_buffs(template, tick);
    if buffs.is_empty() {
        return Vec::new();
    }

    let mut packets = Vec::new();
    for buff in buffs {
        let applied = apply_or_stack_duration_buff(world, buff);
        if let Some(packet) = client_buff_packet_for_state(world, &applied) {
            packets.push(packet);
        }
    }
    packets
}
