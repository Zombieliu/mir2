//! Shared definitions for the two town task stewards. These are additional
//! content, deliberately separate from the imported Crystal manifests.
use std::{collections::BTreeSet, sync::OnceLock};

use mir2_protocol::{ClientQuestInfo, Point};
use serde::Deserialize;

use crate::{CrystalNpcInfoTemplate, CrystalQuestKillTaskTemplate, CrystalQuestPacketTemplate};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PeriodicCadence {
    Daily,
    Weekly,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PeriodicQuestCatalog {
    pub schema: u8,
    pub profile: String,
    pub time_zone: String,
    pub npcs: Vec<PeriodicNpc>,
    pub quests: Vec<PeriodicQuest>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PeriodicNpc {
    pub object_id: u32,
    pub name: String,
    pub map: String,
    pub map_index: i32,
    pub x: i32,
    pub y: i32,
    pub image: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PeriodicQuest {
    pub id: i32,
    pub min_level: i32,
    pub max_level: i32,
    pub cadence: PeriodicCadence,
    pub slot: u8,
    pub title: String,
    pub summary: String,
    pub reward_level_span: u16,
    pub gold: u32,
    pub kills: Vec<PeriodicKill>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PeriodicKill {
    pub monster: String,
    pub monster_index: i32,
    pub count: u32,
    pub maps: Vec<String>,
    pub x: i32,
    pub y: i32,
}

pub fn catalog() -> &'static PeriodicQuestCatalog {
    static CATALOG: OnceLock<PeriodicQuestCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let value: PeriodicQuestCatalog = serde_json::from_str(include_str!(
            "../../../config/quest-guidance/daily-weekly-v1.json"
        ))
        .expect("periodic quest catalog must decode");
        validate(&value).expect("periodic quest catalog must be valid");
        value
    })
}

pub fn quest(id: i32) -> Option<&'static PeriodicQuest> {
    catalog().quests.iter().find(|quest| quest.id == id)
}

pub fn npc(id: u32) -> Option<&'static PeriodicNpc> {
    catalog().npcs.iter().find(|npc| npc.object_id == id)
}

pub fn is_periodic(id: i32) -> bool {
    quest(id).is_some()
}

pub fn validate(value: &PeriodicQuestCatalog) -> Result<(), String> {
    if value.schema != 1
        || value.profile != "daily-weekly-v1"
        || value.time_zone != "UTC+8"
        || value.npcs.len() != 2
        || value.quests.len() != 20
    {
        return Err("unsupported periodic quest catalog".into());
    }
    let mut ids = BTreeSet::new();
    for npc in &value.npcs {
        if !matches!(npc.object_id, 2180 | 2181)
            || !ids.insert(npc.object_id)
            || npc.name.trim().is_empty()
            || !matches!(npc.map.as_str(), "0" | "3")
            || npc.x < 0
            || npc.y < 0
        {
            return Err("invalid or duplicate task steward".into());
        }
    }
    let mut quest_ids = BTreeSet::new();
    let mut slots = BTreeSet::new();
    for quest in &value.quests {
        let cadence = match quest.cadence {
            PeriodicCadence::Daily => 0,
            PeriodicCadence::Weekly => 1,
        };
        if !(92001..=92020).contains(&quest.id)
            || !quest_ids.insert(quest.id)
            || ![(10, 14), (15, 24), (25, 34), (35, 50)]
                .contains(&(quest.min_level, quest.max_level))
            || quest.slot > if cadence == 0 { 2 } else { 1 }
            || !slots.insert((quest.min_level, cadence, quest.slot))
            || quest.title.trim().is_empty()
            || quest.summary.trim().is_empty()
            || quest.reward_level_span
                != match quest.cadence {
                    PeriodicCadence::Daily => 5,
                    PeriodicCadence::Weekly => 10,
                }
            || quest.gold == 0
            || quest.kills.is_empty()
        {
            return Err("invalid periodic quest or slot".into());
        }
        let mut monsters = BTreeSet::new();
        for kill in &quest.kills {
            if kill.monster_index <= 0
                || !monsters.insert(kill.monster_index)
                || kill.monster.trim().is_empty()
                || kill.count == 0
                || kill.maps.is_empty()
                || kill.maps.iter().any(|map| map.trim().is_empty())
                || kill.x < 0
                || kill.y < 0
            {
                return Err("invalid periodic hunt objective".into());
            }
        }
    }
    Ok(())
}

pub fn npc_templates() -> Vec<CrystalNpcInfoTemplate> {
    let ids: Vec<i32> = catalog().quests.iter().map(|quest| quest.id).collect();
    catalog()
        .npcs
        .iter()
        .map(|npc| CrystalNpcInfoTemplate {
            npc_index: npc.object_id as i32,
            loaded_object_id: Some(npc.object_id),
            map_index: npc.map_index,
            map_file_name: Some(npc.map.clone()),
            file_name: format!("PeriodicTasks/{}", npc.object_id),
            script_key: String::new(),
            name: npc.name.clone(),
            location: Point { x: npc.x, y: npc.y },
            image: npc.image,
            rate: 1,
            price_rate: 1.0,
            collect_quest_indexes: ids.clone(),
            finish_quest_indexes: ids.clone(),
            time_visible: false,
            hour_start: 0,
            minute_start: 0,
            hour_end: 0,
            minute_end: 0,
            min_level: 0,
            max_level: 0,
            day_of_week: String::new(),
            class_required: String::new(),
            conquest: 0,
            flag_needed: 0,
            show_on_big_map: true,
            big_map_icon: 0,
            can_teleport_to: false,
            conquest_visible: false,
        })
        .collect()
}

impl PeriodicQuest {
    pub fn info(&self, experience: u32, gold: u32) -> ClientQuestInfo {
        let group = match self.cadence {
            PeriodicCadence::Daily => "Daily Tasks",
            PeriodicCadence::Weekly => "Weekly Tasks",
        };
        let reset = match self.cadence {
            PeriodicCadence::Daily => "Daily reset: 00:00 server time (UTC+8).",
            PeriodicCadence::Weekly => "Weekly reset: Monday 00:00 server time (UTC+8).",
        };
        ClientQuestInfo {
            index: self.id,
            npc_index: 2180,
            name: self.title.clone(),
            group: group.into(),
            description: vec![
                self.summary.clone(),
                reset.into(),
                "Bichon and Mongchon share these tasks and rewards.".into(),
                "Unfinished tasks keep their progress after reset.".into(),
                "Rewards are fixed when you accept the task.".into(),
            ],
            task_description: self
                .kills
                .iter()
                .map(|kill| format!("Kill {} {}", kill.count, kill.monster))
                .collect(),
            return_description: vec!["Return to either town Task Steward.".into()],
            completion_description: vec!["Task reward claimed for this period.".into()],
            min_level_needed: self.min_level,
            max_level_needed: self.max_level,
            quest_needed: 0,
            class_needed: 0,
            quest_type: 1,
            time_limit_in_seconds: 0,
            reward_gold: gold,
            reward_exp: experience,
            reward_credit: 0,
            rewards_fixed_item: Vec::new(),
            rewards_select_item: Vec::new(),
            finish_npc_index: 2180,
        }
    }

    pub fn template(&self) -> CrystalQuestPacketTemplate {
        let info = self.info(0, self.gold);
        CrystalQuestPacketTemplate {
            index: self.id,
            name: self.title.clone(),
            group: info.group,
            file_name: format!("PeriodicTasks/{}", self.id),
            required_min_level: self.min_level,
            required_max_level: self.max_level,
            required_quest: 0,
            required_class: 0,
            quest_type: 1,
            goto_message: "Return to either town Task Steward.".into(),
            kill_message: String::new(),
            item_message: String::new(),
            flag_message: String::new(),
            time_limit_in_seconds: 0,
            npc_index: 2180,
            finish_npc_index: 2180,
            carry_items: Vec::new(),
            kill_tasks: self
                .kills
                .iter()
                .map(|kill| CrystalQuestKillTaskTemplate {
                    monster_index: kill.monster_index,
                    monster_name: kill.monster.clone(),
                    count: kill.count,
                    message: String::new(),
                })
                .collect(),
            item_tasks: Vec::new(),
            flag_tasks: Vec::new(),
            payload_len: 0,
            payload_hex: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periodic_hunts_reference_existing_profile_monsters_and_respawns() {
        let profile = crate::platinum_176_profile();
        for quest in &catalog().quests {
            for kill in &quest.kills {
                let monster = crate::crystal_monster_by_index(kill.monster_index)
                    .expect("hunt monster is in the canonical manifest");
                assert_eq!(monster.name, kill.monster);
                assert!(!crate::content_profile_monster_is_boss(
                    &profile,
                    &monster.name
                ));
                for map in &kill.maps {
                    assert!(profile
                        .map_whitelist
                        .iter()
                        .any(|entry| entry.file_name == *map));
                    let map = crate::crystal_map_respawns_ref(map).expect("hunt map exists");
                    assert!(
                        map.respawns
                            .iter()
                            .any(|spawn| spawn.monster_index == kill.monster_index
                                && spawn.count > 0),
                        "{} has no {} population",
                        map.map_file_name,
                        kill.monster
                    );
                }
            }
        }
    }

    #[test]
    fn periodic_additions_do_not_reuse_imported_npc_quest_ids_or_tiles() {
        let imported_npcs = crate::crystal_npc_info_manifest_ref();
        let imported_quests = crate::crystal_quest_packet_manifest();
        for npc in &catalog().npcs {
            assert!(!imported_npcs
                .npcs
                .iter()
                .any(|old| old.loaded_object_id == Some(npc.object_id)));
            assert!(!imported_npcs
                .npcs
                .iter()
                .any(|old| old.loaded_object_id.is_some()
                    && old.map_file_name.as_deref() == Some(npc.map.as_str())
                    && old.location == Point { x: npc.x, y: npc.y }));
            assert_eq!(
                crate::crystal_map_respawns_ref(&npc.map).unwrap().map_index,
                npc.map_index
            );
        }
        for quest in &catalog().quests {
            assert!(!imported_quests
                .quests
                .iter()
                .any(|old| old.index == quest.id));
        }
        // Catalog access must not inject additions into the canonical imports.
        assert_eq!(imported_npcs.npcs.len(), imported_npcs.total_npcs);
        assert_eq!(imported_quests.quests.len(), imported_quests.total_quests);
    }

    #[test]
    fn every_supported_band_has_five_slots_and_only_experience_gold_rewards() {
        for (min, max) in [(10, 14), (15, 24), (25, 34), (35, 50)] {
            let quests: Vec<_> = catalog()
                .quests
                .iter()
                .filter(|quest| quest.min_level == min && quest.max_level == max)
                .collect();
            let daily: Vec<_> = quests
                .iter()
                .filter(|quest| quest.cadence == PeriodicCadence::Daily)
                .collect();
            let weekly: Vec<_> = quests
                .iter()
                .filter(|quest| quest.cadence == PeriodicCadence::Weekly)
                .collect();
            assert_eq!(daily.len(), 3);
            assert_eq!(weekly.len(), 2);
            assert!(daily.iter().all(|quest| quest.reward_level_span == 5));
            assert!(weekly.iter().all(|quest| quest.reward_level_span == 10));
            for quest in quests {
                let info = quest.info(1, quest.gold);
                assert!(info.rewards_fixed_item.is_empty() && info.rewards_select_item.is_empty());
                assert_eq!(info.reward_credit, 0);
                assert_eq!(info.class_needed, 0);
            }
        }
    }

    #[test]
    fn invalid_periodic_slot_objective_and_duplicate_npc_are_rejected() {
        let original = include_str!("../../../config/quest-guidance/daily-weekly-v1.json");
        let decode = || serde_json::from_str::<PeriodicQuestCatalog>(original).unwrap();
        let mut duplicate_slot = decode();
        duplicate_slot.quests[1].slot = duplicate_slot.quests[0].slot;
        assert!(validate(&duplicate_slot).is_err());
        let mut empty_objective = decode();
        empty_objective.quests[0].kills[0].count = 0;
        assert!(validate(&empty_objective).is_err());
        let mut duplicate_npc = decode();
        duplicate_npc.npcs[1].object_id = duplicate_npc.npcs[0].object_id;
        assert!(validate(&duplicate_npc).is_err());
        let mut invalid_reward_span = decode();
        invalid_reward_span.quests[0].reward_level_span = 10;
        assert!(validate(&invalid_reward_span).is_err());
        let mut invalid_weekly_span = decode();
        invalid_weekly_span.quests[3].reward_level_span = 5;
        assert!(validate(&invalid_weekly_span).is_err());
    }
}
