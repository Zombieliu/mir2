//! Task hunting areas from authored spawn data, never live monster positions.
use crate::big_map::BigMapPoint;
use crate::quest_model::{QuestStatus, QuestTracker};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestHuntRegion {
    pub primary: bool,
    pub monster_index: i32,
    pub name: String,
    pub center: BigMapPoint,
    pub radius: i32,
    pub remaining: u32,
}

pub fn active_hunt_regions(
    tracker: &QuestTracker,
    map_index: i32,
    player: BigMapPoint,
) -> Vec<QuestHuntRegion> {
    active_hunt_regions_for_primary(tracker, map_index, player, None)
}

pub fn active_hunt_regions_for_primary(
    tracker: &QuestTracker,
    map_index: i32,
    player: BigMapPoint,
    primary: Option<i32>,
) -> Vec<QuestHuntRegion> {
    let config = crate::quest_practice::newcomer_config();
    let Some(map) = mir2_game_data::crystal_respawn_manifest_ref()
        .maps
        .iter()
        .find(|map| map.map_index == map_index)
    else {
        return Vec::new();
    };
    let mut regions = Vec::new();
    let mut quests: Vec<_> = tracker.active_quests.iter().collect();
    quests.sort_by_key(|quest| Some(quest.quest_index) != primary);
    for quest in quests {
        if quest.status != QuestStatus::InProgress {
            continue;
        }
        let Some(definition) = config["quests"].as_array().and_then(|quests| {
            quests
                .iter()
                .find(|q| q["id"].as_i64() == Some(i64::from(quest.quest_index)))
        }) else {
            continue;
        };
        if !definition["maps"].as_array().is_some_and(|maps| {
            maps.iter()
                .any(|name| name.as_str() == Some(map.map_file_name.as_str()))
        }) {
            continue;
        }
        let kills = definition["kills"].as_array();
        let practice_pending = definition["flags"].as_array().into_iter().flatten().enumerate()
            .any(|(index, flag)| flag["kind"] == "classPractice"
                && quest.objectives.get(kills.map_or(0, Vec::len) + index)
                    .is_some_and(|progress| progress.current < progress.target));
        for (index, kill) in definition["kills"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            // Server objectives use the configured kill order. Do not infer
            // unfinished targets when their authoritative progress is absent.
            let Some(progress) = quest.objectives.get(index) else {
                continue;
            };
            if (progress.current >= progress.target && !practice_pending) || progress.target == 0 {
                continue;
            }
            let Some(monster_index) = kill["monsterIndex"]
                .as_i64()
                .and_then(|n| i32::try_from(n).ok())
            else {
                continue;
            };
            if regions.iter().any(|region: &QuestHuntRegion| region.monster_index == monster_index) {
                continue;
            }
            // V2 has explicit training footholds. The imported whole-map
            // spawn (250,250), spread 250 is not a useful arrival destination.
            let mut training = config["trainingSpawns"].as_array().into_iter().flatten()
                .filter(|spawn| spawn["sourceQuestId"].as_i64() == Some(i64::from(quest.quest_index))
                    && spawn["mapFileName"].as_str() == Some(map.map_file_name.as_str())
                    && spawn["monsterIndex"].as_i64() == Some(i64::from(monster_index))
                    && spawn["count"].as_u64().unwrap_or(0) > 0)
                .filter_map(|spawn| Some(QuestHuntRegion {
                    primary: Some(quest.quest_index) == primary,
                    monster_index,
                    name: spawn["monster"].as_str()?.to_owned(),
                    center: BigMapPoint {
                        x: i32::try_from(spawn["position"]["x"].as_i64()?).ok()?,
                        y: i32::try_from(spawn["position"]["y"].as_i64()?).ok()?,
                    },
                    radius: spawn["spread"].as_i64().unwrap_or(0).clamp(2, i64::from(i32::MAX)) as i32,
                    remaining: progress.target.saturating_sub(progress.current),
                })).collect::<Vec<_>>();
            if !training.is_empty() {
                training.sort_by_key(|region| player.x.abs_diff(region.center.x).max(player.y.abs_diff(region.center.y)));
                regions.extend(training);
                continue;
            }
            let nearest = map
                .respawns
                .iter()
                .filter(|spawn| spawn.monster_index == monster_index && spawn.count > 0)
                .min_by_key(|spawn| {
                    let dx = i64::from(spawn.location.x) - i64::from(player.x);
                    let dy = i64::from(spawn.location.y) - i64::from(player.y);
                    dx * dx + dy * dy
                });
            if let Some(spawn) = nearest {
                if regions
                    .iter()
                    .any(|region: &QuestHuntRegion| region.monster_index == monster_index)
                {
                    continue;
                }
                regions.push(QuestHuntRegion {
                    primary: Some(quest.quest_index) == primary,
                    monster_index,
                    name: spawn.monster_name.clone(),
                    center: BigMapPoint {
                        x: spawn.location.x,
                        y: spawn.location.y,
                    },
                    radius: i32::from(spawn.spread).max(1),
                    remaining: progress.target.saturating_sub(progress.current),
                });
            }
        }
    }
    regions.truncate(3);
    regions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quest_model::{Quest, QuestObjective};

    #[test]
    fn wooma_practice_uses_three_actual_footholds_even_after_kill_quota() {
        let mut tracker = QuestTracker { active_quests: vec![Quest {
            quest_index: 2110021, status: QuestStatus::InProgress,
            objectives: vec![
                QuestObjective { objective_id: "2110021:0".into(), text: "WoomaFighter".into(), current: 1, target: 3 },
                QuestObjective { objective_id: "2110021:1".into(), text: "Complete your class practice".into(), current: 0, target: 1 },
            ],
            accept_npc_index: None, finish_npc_index: None, title: "Prove your class tactics".into(),
            npc_name: None, group: None, min_level_needed: 28, detail: Default::default(), rewards: vec![], unknown_text: None,
        }] };
        let map = mir2_game_data::crystal_map_respawns_ref("D022").unwrap();
        let player = BigMapPoint { x: 109, y: 196 };
        let regions = active_hunt_regions_for_primary(&tracker, map.map_index, player, Some(2110021));
        assert_eq!(regions.len(), 3);
        assert_eq!(regions[0].center, BigMapPoint { x: 280, y: 340 });
        for region in &regions {
            assert_eq!(region.radius, 3);
            assert!(region.primary);
            assert_eq!(region.remaining, 2);
            assert_ne!(region.center, BigMapPoint { x: 250, y: 250 });
        }
        tracker.active_quests[0].objectives[0].current = 3;
        assert_eq!(active_hunt_regions(&tracker, map.map_index, player).len(), 3, "practice still needs monsters");
        tracker.active_quests[0].objectives[1].current = 1;
        assert!(active_hunt_regions(&tracker, map.map_index, player).is_empty());
    }

    #[test]
    fn a1_remaining_cats_use_real_bichon_spawn_and_completed_targets_disappear() {
        let mut tracker = QuestTracker {
            active_quests: vec![Quest {
                quest_index: 2110003,
                status: QuestStatus::InProgress,
                objectives: vec![
                    QuestObjective {
                        objective_id: "2110003:0".into(),
                        text: "Scarecrow".into(),
                        current: 2,
                        target: 2,
                    },
                    QuestObjective {
                        objective_id: "2110003:1".into(),
                        text: "RakingCat".into(),
                        current: 0,
                        target: 2,
                    },
                ],
                accept_npc_index: None,
                finish_npc_index: None,
                title: "Protect the village".into(),
                npc_name: None,
                group: None,
                min_level_needed: 3,
                detail: Default::default(),
                rewards: vec![],
                unknown_text: None,
            }],
        };
        let map = mir2_game_data::crystal_map_respawns_ref("0").unwrap();
        let player = BigMapPoint { x: 290, y: 614 };
        let regions = active_hunt_regions(&tracker, map.map_index, player);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].name, "RakingCat");
        assert_eq!(regions[0].center, BigMapPoint { x: 340, y: 550 });
        assert_eq!(regions[0].remaining, 2);
        assert!(!regions[0].primary);
        let focused = active_hunt_regions_for_primary(&tracker, map.map_index, player, Some(2110003));
        assert!(focused[0].primary);
        assert_eq!(focused[0].center, regions[0].center);
        assert_eq!(focused[0].remaining, regions[0].remaining);
        assert!(active_hunt_regions(&tracker, -1, player).is_empty());
        tracker.active_quests[0].objectives[1].current = 2;
        assert!(active_hunt_regions(&tracker, map.map_index, player).is_empty());
    }
}
