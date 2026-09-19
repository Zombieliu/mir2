//! Hero-specific stats, following HumanObject.RefreshStats (1729..2330).
//! Base values use HeroBaseStats; final caps deliberately use ClassBaseStats.
use super::{
    hero_inventory::item_weight,
    items::try_user_item_from_item_state,
    resources::{HeroInventoryResource, Stage5SystemsResource},
};
use bevy_ecs::world::World;
use mir2_protocol::{ServerPacket, ServerPacketId, Spell, UserItemStat};
use std::collections::BTreeMap;
use super::item_sets::apply_sets;
#[derive(Default, Debug, Clone)]
pub(super) struct HeroStats {
    pub values: BTreeMap<u8, i32>,
    pub bag: u32,
    pub wear: u32,
    pub hand: u32,
}
impl HeroStats {
    pub fn get(&self, stat: u8) -> i32 {
        self.values.get(&stat).copied().unwrap_or_default()
    }
    fn add(&mut self, stat: u8, value: i32) {
        let old = self.get(stat);
        self.values.insert(stat, old.saturating_add(value));
    }
    pub fn snapshot(&self) -> Vec<UserItemStat> {
        self.values
            .iter()
            .map(|(&stat, &value)| UserItemStat { stat, value })
            .collect()
    }
}

fn carrier_weight(
    item: &mir2_protocol::UserItem,
    template: &mir2_game_data::CrystalItemTemplate,
) -> u32 {
    u32::from(template.weight).saturating_mul(if matches!(template.item_type, 8 | 30) {
        1
    } else {
        u32::from(item.count)
    })
}
fn add_item_stats(
    stats: &mut HeroStats,
    item: &mir2_protocol::UserItem,
    real: &mir2_game_data::CrystalItemTemplate,
) {
    for entry in &real.stats {
        stats.add(entry.stat, entry.value);
    }
    for entry in &item.added_stats {
        stats.add(entry.stat, entry.value);
    }
}
fn add_weight(stats: &mut HeroStats, kind: u8, weight: u32) {
    if matches!(kind, 1 | 12) {
        stats.hand = stats.hand.saturating_add(weight);
    } else {
        stats.wear = stats.wear.saturating_add(weight);
    }
}
fn add_awake(stats: &mut HeroStats, item: &mir2_protocol::UserItem) {
    // Original Awake.GetAwakeValue accumulates into a byte.
    let value = i32::from(
        item.awake_values
            .iter()
            .fold(0u8, |sum, &v| sum.wrapping_add(v)),
    );
    let pair = match item.awake_type {
        1 => Some((4, 5)),
        2 => Some((6, 7)),
        3 => Some((8, 9)),
        4 => Some((0, 1)),
        5 => Some((2, 3)),
        6 => Some((12, 13)),
        _ => None,
    };
    if let Some((a, b)) = pair {
        stats.add(a, value);
        stats.add(b, value);
    }
}
pub(super) fn compute(world: &World) -> HeroStats {
    compute_with_status(world, super::hero_ai::hero_mount::riding(world), &super::hero_ai::hero_buffs::stats(world))
}
pub(super) fn compute_with_status(
    world: &World,
    riding: bool,
    buffs: &[UserItemStat],
) -> HeroStats {
    let systems = &world.resource::<Stage5SystemsResource>().stage5_systems;
    let Some(hero) = systems.hero.as_ref() else {
        return HeroStats::default();
    };
    let source = mir2_game_data::crystal_hero_settings().base_stats(hero.class);
    let mut stats = HeroStats::default();
    for entry in &source.stats {
        stats.values.insert(
            entry.stat,
            mir2_game_data::calculate_crystal_hero_base_stat(entry, hero.class, hero.level),
        );
    }
    stats.values.insert(107, 1);
    let inventory = world.resource::<HeroInventoryResource>();
    stats.bag = inventory
        .items
        .iter()
        .map(item_weight)
        .fold(0u32, u32::saturating_add);
    let mut sets = Vec::new();
    let mut special = 0i16;
    for slot in 0..14 {
        let Some(item) = inventory.equipment.iter().find(|item| item.slot == slot) else {
            continue;
        };
        let Ok(carrier) = try_user_item_from_item_state(item) else {
            continue;
        };
        let Some(base) = mir2_game_data::crystal_item_by_index(carrier.item_index) else {
            continue;
        };
        let real = mir2_game_data::crystal_real_item_for_player(&base, hero.level, hero.class);
        add_weight(&mut stats, real.item_type, carrier_weight(&carrier, &base));
        if base.durability > 0 && carrier.current_dura == 0 {
            continue;
        }
        // HumanObject performs this source shape check before stats/sets/sockets.
        if matches!(base.shape, 49 | 50) {
            continue;
        }
        add_item_stats(&mut stats, &carrier, &real);
        add_awake(&mut stats, &carrier);
        special |= real.unique;
        if real.item_type != 19 || riding {
            for socket in carrier.slots.iter().flatten() {
                let Some(base) = mir2_game_data::crystal_item_by_index(socket.item_index) else {
                    continue;
                };
                let real =
                    mir2_game_data::crystal_real_item_for_player(&base, hero.level, hero.class);
                add_weight(&mut stats, real.item_type, carrier_weight(socket, &base));
                if base.durability > 0 && socket.current_dura == 0 {
                    continue;
                }
                add_item_stats(&mut stats, socket, &real);
                special |= real.unique;
                if real.unique & 0x0200 != 0 {
                    stats.values.insert(107, 3);
                }
            }
        }
        sets.push((slot, real.item_set, real.item_type));
    }
    if special & 0x20 != 0 {
        for stat in [16, 17, 18] {
            stats.values.insert(stat, stats.get(stat).saturating_mul(2));
        }
    }
    apply_sets(&mut stats.values, &sets);
    for magic in &systems.hero_learned_magics {
        let level = usize::from(magic.level.min(3));
        match magic.spell {
            Spell::Fencing => stats.add(10, (level * 3) as i32),
            Spell::Slaying => {
                stats.add(10, level as i32);
                stats.add(5, [5, 6, 7, 8][level]);
            }
            Spell::SpiritSword => stats.add(10, [0, 3, 5, 8][level]),
            _ => {}
        }
    }
    for buff in buffs {
        stats.add(buff.stat, buff.value);
    }
    for (stat, rate) in [
        (12, 46),
        (13, 47),
        (1, 40),
        (3, 41),
        (5, 42),
        (7, 43),
        (9, 44),
        (14, 45),
    ] {
        let value = stats.get(stat);
        let rate = stats.get(rate);
        stats.add(
            stat,
            ((i64::from(value) * i64::from(rate)) / 100)
                .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        );
    }
    if let Some(payload) = mir2_game_data::crystal_base_stats_info_packet_payload(hero.class) {
        if let ServerPacket::BaseStatsInfo { stats: source } =
            super::packets::decode_crystal_payload(ServerPacketId::BaseStatsInfo, payload)
        {
            for cap in source.caps {
                stats
                    .values
                    .insert(cap.stat, stats.get(cap.stat).min(cap.value));
            }
        }
    }
    for stat in (0..=9).chain([12, 13]) {
        stats.values.insert(stat, stats.get(stat).max(0));
    }
    for (min, max) in [(4, 5), (6, 7), (8, 9)] {
        stats.values.insert(min, stats.get(min).min(stats.get(max)));
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hero_stats_sets_use_distinct_types_and_allow_two_complete_sets() {
        let mut stats = HeroStats::default();
        apply_sets(&mut stats.values, &[(7, 9, 7), (8, 9, 7)]);
        assert_eq!(stats.get(12), 0);
        apply_sets(&mut stats.values, &[(7, 9, 7), (8, 9, 7), (4, 9, 5), (3, 9, 5)]);
        assert_eq!(stats.get(12), 100);
        let mut smash = HeroStats::default();
        apply_sets(
            &mut smash.values,
            &[(7, 5, 7), (8, 5, 7), (5, 5, 6), (6, 5, 6), (4, 5, 5)],
        );
        assert_eq!(smash.get(14), 2);
        assert_eq!(smash.get(5), 3);
    }
    #[test]
    fn hero_stats_mir_combinations_stack_by_actual_equipment_slot() {
        let mut stats = HeroStats::default();
        let slots = [0, 1, 2, 4, 5, 6, 7, 8, 10, 11].map(|slot| (slot, 12, 1));
        apply_sets(&mut stats.values, &slots);
        assert_eq!(stats.get(16), 120);
        assert_eq!(stats.get(18), 27);
        assert_eq!(stats.get(17), 34);
        assert_eq!(stats.get(12), 70);
        assert_eq!(stats.get(5), 4);
    }
    #[test]
    fn hero_stats_real_equipment_awake_rates_and_source_caps() {
        use crate::{ItemContainer, SimulationConfig, SimulationSession};
        use mir2_protocol::{ClientPacket, MirClass, MirGender};
        let mut session = SimulationSession::new(SimulationConfig::default());
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        });
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        session.handle_packet(ClientPacket::NewHero {
            name: "Aide".into(),
            class: MirClass::Taoist,
            gender: MirGender::Female,
        });
        let baseline = compute(session.app.world());
        let template = mir2_game_data::crystal_item_by_index(221).unwrap();
        let mut item = super::super::items::embedded_item_state_from_template(
            &template,
            ItemContainer::Bag1,
            0,
        );
        item.unique_id = 88001;
        let mut carrier = try_user_item_from_item_state(&item).unwrap();
        carrier.added_stats = vec![
            UserItemStat {
                stat: 12,
                value: 100,
            },
            UserItemStat {
                stat: 46,
                value: 50,
            },
            UserItemStat {
                stat: 30,
                value: 99,
            },
        ];
        carrier.awake_type = 6;
        carrier.awake_values = vec![5];
        item = super::super::items::try_item_state_from_user_item(item, &carrier).unwrap();
        session
            .app
            .world_mut()
            .resource_mut::<HeroInventoryResource>()
            .equipment
            .push(item);
        let stats = compute(session.app.world());
        let expected = baseline.get(12) + 105;
        assert_eq!(stats.get(12), expected + expected / 2);
        assert_eq!(stats.get(30), 2);
        assert_eq!(stats.hand, 4);
        assert_eq!(stats.wear, 0);
        session
            .app
            .world_mut()
            .resource_mut::<HeroInventoryResource>()
            .equipment[0]
            .durability_current = Some(0);
        assert_eq!(compute(session.app.world()).get(12), baseline.get(12));
        assert_eq!(compute(session.app.world()).hand, 4);
    }
}
