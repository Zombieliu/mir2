//! Presentation-only matching between active quest objectives and visible monsters.
//!
//! Authored V2 kill identities are paired with authoritative progress counters;
//! legacy Crystal tasks retain packet-label matching. Both use visible entity
//! names without deciding quest progress or combat results.

use crate::entities::{EntityKind, EntityModel, EntityModelSet};
use crate::quest_model::{Quest, QuestStatus, QuestTracker};

#[derive(Debug, Clone, Copy)]
pub struct NearestQuestMonster<'a> {
    pub entity: &'a EntityModel,
    pub distance: u32,
}

/// Whether an in-progress, incomplete objective names this monster.
pub fn quest_targets_monster(quest: &Quest, monster_name: &str) -> bool {
    if quest.status != QuestStatus::InProgress {
        return false;
    }
    if let Some(definition) = crate::quest_destination::authored_quest_definition(quest.quest_index) {
        // V2 snapshots order kill objectives before flags. Arrival/equipment/
        // practice descriptions are not monster identities, even if their text
        // includes a name such as "Oma Cave". Missing progress never implies a kill.
        return definition["kills"].as_array().into_iter().flatten().enumerate()
            .any(|(index, kill)| {
                quest.objectives.get(index).is_some_and(|objective| {
                    objective.target > 0 && !objective.is_complete()
                }) && kill["monster"].as_str().is_some_and(|configured| {
                    let expected = normalized_words(configured);
                    let actual = normalized_words(monster_name);
                    !expected.is_empty() && expected.len() == actual.len()
                        && expected.iter().zip(&actual).all(|(a, b)| equivalent_word(a, b))
                })
            });
    }
    quest.status == QuestStatus::InProgress
        && quest
            .objectives
            .iter()
            .filter(|objective| !objective.is_complete())
            .any(|objective| objective_mentions_monster(&objective.text, monster_name))
}

/// Whether any current quest still needs this monster.
pub fn tracker_targets_monster(tracker: &QuestTracker, monster_name: &str) -> bool {
    tracker
        .active_quests
        .iter()
        .any(|quest| quest_targets_monster(quest, monster_name))
}

/// Find the closest visible monster that advances the requested quest.
pub fn nearest_quest_monster<'a>(
    tracker: &QuestTracker,
    quest_index: Option<i32>,
    entities: &'a EntityModelSet,
    center_x: i32,
    center_y: i32,
) -> Option<NearestQuestMonster<'a>> {
    entities
        .entities
        .iter()
        .filter(|entity| entity.kind == EntityKind::Monster)
        .filter(|entity| {
            tracker.active_quests.iter().any(|quest| {
                quest_index.is_none_or(|index| quest.quest_index == index)
                    && quest_targets_monster(quest, &entity.name)
            })
        })
        .filter(|entity| entity.object_id.parse::<u32>().is_ok())
        .map(|entity| NearestQuestMonster {
            entity,
            distance: center_x.abs_diff(entity.x).max(center_y.abs_diff(entity.y)),
        })
        .min_by_key(|target| (target.distance, target.entity.object_id.as_str()))
}

fn objective_mentions_monster(objective: &str, monster_name: &str) -> bool {
    let objective_words = normalized_words(objective);
    let monster_words = normalized_words(monster_name);
    if monster_words.is_empty() || objective_words.len() < monster_words.len() {
        return false;
    }

    objective_words.windows(monster_words.len()).any(|window| {
        window
            .iter()
            .zip(&monster_words)
            .all(|(objective, monster)| equivalent_word(objective, monster))
    })
}

fn normalized_words(value: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut previous_was_lowercase = false;

    for character in value.chars() {
        if character.is_alphanumeric() {
            if previous_was_lowercase && character.is_uppercase() && !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            current.extend(character.to_lowercase());
            previous_was_lowercase = character.is_lowercase();
        } else {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            previous_was_lowercase = false;
        }
    }
    if !current.is_empty() {
        words.push(current);
    }

    words.retain(|word| word != "s");
    words
}

fn equivalent_word(left: &str, right: &str) -> bool {
    left == right || singular(left) == singular(right)
}

fn singular(word: &str) -> &str {
    if word.len() > 3 && word.ends_with('s') && !word.ends_with("ss") {
        &word[..word.len() - 1]
    } else {
        word
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quest_model::{QuestDetailText, QuestObjective};

    fn quest(text: &str, current: u32, target: u32) -> Quest {
        Quest {
            quest_index: 5,
            accept_npc_index: Some(5),
            finish_npc_index: Some(5),
            title: "The Smith's Test".to_owned(),
            npc_name: Some("Smith".to_owned()),
            group: Some("BichonProvince".to_owned()),
            min_level_needed: 1,
            detail: QuestDetailText::default(),
            status: QuestStatus::InProgress,
            objectives: vec![QuestObjective {
                objective_id: "5:0".to_owned(),
                text: text.to_owned(),
                current,
                target,
            }],
            rewards: Vec::new(),
            unknown_text: None,
        }
    }

    fn monster(object_id: &str, name: &str, x: i32, y: i32) -> EntityModel {
        EntityModel {
            object_id: object_id.to_owned(),
            kind: EntityKind::Monster,
            name: name.to_owned(),
            x,
            y,
            level: None,
            direction: None,
        }
    }

    #[test]
    fn matches_crystal_plural_possessive_and_camel_case_labels() {
        assert!(objective_mentions_monster(
            "Eliminate 10 Scarecrow's",
            "Scarecrow"
        ));
        assert!(objective_mentions_monster(
            "Defeat 5 Forest Yeti",
            "ForestYeti"
        ));
        assert!(objective_mentions_monster(
            "Collect OmaTeeth from Omas",
            "Oma"
        ));
        assert!(!objective_mentions_monster("Return when eligible", "Hen"));
    }

    #[test]
    fn only_incomplete_in_progress_objectives_mark_targets() {
        let active = quest("Kill Deer", 2, 3);
        assert!(quest_targets_monster(&active, "Deer"));

        let complete = quest("Kill Deer", 3, 3);
        assert!(!quest_targets_monster(&complete, "Deer"));

        let mut ready = quest("Kill Deer", 2, 3);
        ready.status = QuestStatus::ReadyToTurnIn;
        assert!(!quest_targets_monster(&ready, "Deer"));
    }

    #[test]
    fn nearest_target_is_selected_for_the_requested_quest() {
        let tracker = QuestTracker {
            active_quests: vec![quest("Kill Deer", 0, 3)],
        };
        let entities = EntityModelSet {
            entities: vec![
                monster("20", "Deer", 110, 100),
                monster("10", "Deer", 103, 102),
                monster("30", "Scarecrow", 101, 100),
            ],
        };

        let nearest = nearest_quest_monster(&tracker, Some(5), &entities, 100, 100).unwrap();
        assert_eq!(nearest.entity.object_id, "10");
        assert_eq!(nearest.distance, 3);
        assert!(nearest_quest_monster(&tracker, Some(99), &entities, 100, 100).is_none());
    }

    #[test]
    fn authored_target_arrival_flag_never_marks_a_named_monster() {
        let mut arrival = quest("Reach the Oma Cave entrance", 0, 1);
        arrival.quest_index = 2_110_009;
        assert!(!quest_targets_monster(&arrival, "Oma"));
        let tracker = QuestTracker { active_quests: vec![arrival] };
        let entities = EntityModelSet { entities: vec![monster("20", "Oma", 420, 91)] };
        assert!(nearest_quest_monster(&tracker, Some(2_110_009), &entities, 429, 82).is_none());
        assert!(!tracker_targets_monster(&tracker, "Oma"));
    }

    #[test]
    fn authored_target_hunts_use_configured_kill_order_not_flag_or_display_text() {
        let mut cats = quest("已击败稻草人", 2, 2);
        cats.quest_index = 2_110_003;
        cats.objectives.push(QuestObjective {
            objective_id: "2110003:1".into(), text: "击败钉耙猫".into(), current: 0, target: 2,
        });
        cats.objectives.push(QuestObjective {
            objective_id: "flag".into(), text: "Reach the Oma Cave entrance".into(), current: 0, target: 1,
        });
        assert!(!quest_targets_monster(&cats, "Scarecrow"));
        assert!(quest_targets_monster(&cats, "RakingCat"));
        assert!(!quest_targets_monster(&cats, "Cat"), "a partial configured name is not the target species");
        assert!(!quest_targets_monster(&cats, "Oma"));
        cats.objectives[1].current = 2;
        assert!(!quest_targets_monster(&cats, "RakingCat"));
        cats.objectives.clear();
        assert!(!quest_targets_monster(&cats, "RakingCat"));
    }
}
