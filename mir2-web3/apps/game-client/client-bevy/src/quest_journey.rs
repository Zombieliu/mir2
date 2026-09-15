//! Presentation-only newcomer journey projection.
//!
//! Chapter metadata comes from a bundled reviewed profile. Quest availability,
//! objective progress, completion and rewards remain server-authored read
//! models. This module never infers completion from level, items, or hints.

use std::collections::BTreeMap;

use bevy::prelude::Resource;
use serde::Deserialize;

use crate::quest_guidance::{QuestGuidance, QuestGuidanceCategory};
use crate::quest_model::{CompletedQuestTracker, Quest, QuestReward, QuestStatus, QuestTracker};
use crate::read_model::PlayerStats;

const NEWCOMER_JOURNEY_JSON: &str =
    include_str!("../../../../config/quest-guidance/newcomer-journey-v1.json");

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JourneyChapter {
    pub id: String,
    pub title: String,
    pub min_level: u32,
    pub max_level: u32,
    #[serde(default)]
    pub quest_ids: Vec<i32>,
    #[serde(default)]
    pub class_quest_ids: BTreeMap<String, Vec<i32>>,
    pub goal: String,
    pub reward_summary: String,
    #[serde(default)]
    pub class_hints: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JourneyDocument {
    schema: u32,
    profile: String,
    max_level: u32,
    chapters: Vec<JourneyChapter>,
    #[serde(default)]
    quest_overrides: Vec<JourneyQuestOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JourneyQuestOverride {
    quest_id: i32,
    #[serde(default)]
    start_in_diary: bool,
    #[serde(default)]
    finish_in_diary: bool,
    #[serde(default)]
    start_npc: Option<JourneyNpcLocation>,
    #[serde(default)]
    finish_npc: Option<JourneyNpcLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JourneyNpcLocation {
    name: String,
    map_name: String,
    map_file_name: String,
    x: i32,
    y: i32,
}

#[derive(Resource, Debug, Clone)]
pub struct NewcomerJourneyCatalog {
    max_level: u32,
    chapters: Vec<JourneyChapter>,
    quest_overrides: BTreeMap<i32, JourneyQuestOverride>,
}

impl NewcomerJourneyCatalog {
    pub fn from_json(json: &str) -> Result<Self, String> {
        let document: JourneyDocument =
            serde_json::from_str(json).map_err(|error| format!("invalid journey JSON: {error}"))?;
        if document.schema != 1 || !document.profile.eq_ignore_ascii_case("newcomer-v1") {
            return Err("unsupported newcomer journey profile".to_owned());
        }
        if document.max_level == 0 || document.chapters.is_empty() {
            return Err("newcomer journey must contain a positive level range and chapters".into());
        }
        for chapter in &document.chapters {
            if chapter.id.trim().is_empty()
                || chapter.title.trim().is_empty()
                || chapter.min_level == 0
                || chapter.min_level > chapter.max_level
                || chapter.max_level > document.max_level
                || chapter.goal.trim().is_empty()
            {
                return Err(format!("invalid newcomer journey chapter {}", chapter.id));
            }
        }
        Ok(Self {
            max_level: document.max_level,
            chapters: document.chapters,
            quest_overrides: document
                .quest_overrides
                .into_iter()
                .map(|entry| (entry.quest_id, entry))
                .collect(),
        })
    }

    pub fn bundled() -> Self {
        Self::from_json(NEWCOMER_JOURNEY_JSON)
            .expect("bundled newcomer journey profile must be valid JSON")
    }

    pub fn chapter_for_level(&self, level: u32) -> Option<&JourneyChapter> {
        (level <= self.max_level)
            .then(|| {
                self.chapters
                    .iter()
                    .find(|chapter| (chapter.min_level..=chapter.max_level).contains(&level))
            })
            .flatten()
    }

    pub fn derive(
        &self,
        guidance: &QuestGuidance,
        tracker: &QuestTracker,
        completed: &CompletedQuestTracker,
        player: &PlayerStats,
    ) -> Option<JourneyView> {
        if !guidance.is_enabled() {
            return None;
        }
        let class_name = player.class_name.as_deref();
        let class_known = self
            .chapters
            .iter()
            .all(|chapter| case_insensitive_value(&chapter.class_quest_ids, class_name).is_some());
        let chapter = if completed.known && class_known {
            self.chapters
                .iter()
                .find(|chapter| {
                    chapter
                        .quest_ids_for_class(class_name)
                        .iter()
                        .any(|quest_id| !completed.contains(*quest_id))
                })
                .or_else(|| self.chapters.last())?
        } else {
            self.chapters
                .iter()
                .find(|chapter| {
                    let ids = chapter.quest_ids_for_class(class_name);
                    tracker.active_quests.iter().any(|quest| {
                        ids.contains(&quest.quest_index)
                            && matches!(
                                quest.status,
                                QuestStatus::NotStarted
                                    | QuestStatus::InProgress
                                    | QuestStatus::ReadyToTurnIn
                            )
                    })
                })
                .or_else(|| self.chapter_for_level(player.level.min(self.max_level)))?
        };
        let route_ids = chapter.quest_ids_for_class(class_name);
        let mut all_route_ids = self
            .chapters
            .iter()
            .flat_map(|chapter| chapter.quest_ids_for_class(class_name))
            .collect::<Vec<_>>();
        all_route_ids.sort_unstable();
        all_route_ids.dedup();
        let progress_known = completed.known && class_known;
        let graduated = progress_known
            && self.chapters.iter().all(|chapter| {
                chapter
                    .quest_ids_for_class(class_name)
                    .iter()
                    .all(|quest_id| completed.contains(*quest_id))
            });
        let completed_count = progress_known.then(|| {
            route_ids
                .iter()
                .filter(|quest_id| completed.contains(**quest_id))
                .count()
        });

        let mut candidates = tracker
            .active_quests
            .iter()
            .enumerate()
            .filter_map(|(position, quest)| {
                guidance.entry(quest.quest_index)?;
                if completed.contains(quest.quest_index) {
                    return None;
                }
                let status_rank = match quest.status {
                    QuestStatus::ReadyToTurnIn if all_route_ids.contains(&quest.quest_index) => 0,
                    QuestStatus::InProgress if all_route_ids.contains(&quest.quest_index) => 1,
                    QuestStatus::NotStarted if route_ids.contains(&quest.quest_index) => 2,
                    _ => return None,
                };
                Some((
                    status_rank,
                    guidance.sort_key(quest.quest_index, position),
                    quest,
                ))
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(status, guidance, _)| (*status, *guidance));
        let next = candidates.first().map(|(_, _, quest)| {
            JourneyStep::from_quest(quest, self.quest_overrides.get(&quest.quest_index))
        });

        let mut optional = tracker
            .active_quests
            .iter()
            .filter(|quest| {
                !completed.contains(quest.quest_index)
                    && guidance.entry(quest.quest_index).is_some_and(|entry| {
                        entry.category == QuestGuidanceCategory::Optional
                            && matches!(
                                quest.status,
                                QuestStatus::NotStarted
                                    | QuestStatus::InProgress
                                    | QuestStatus::ReadyToTurnIn
                            )
                    })
            })
            .map(JourneyQuestSummary::from)
            .collect::<Vec<_>>();
        optional.sort_by_key(|quest| guidance.sort_key(quest.quest_id, usize::MAX));

        Some(JourneyView {
            chapter_id: chapter.id.clone(),
            chapter_title: chapter.title.clone(),
            level_range: format!("Lv{}-{}", chapter.min_level, chapter.max_level),
            completed_count,
            quest_count: progress_known.then_some(route_ids.len()),
            goal: chapter.goal.clone(),
            reward_summary: chapter.reward_summary.clone(),
            class_hint: chapter.class_hint(class_name).map(str::to_owned),
            graduated,
            next,
            optional,
        })
    }
}

impl Default for NewcomerJourneyCatalog {
    fn default() -> Self {
        Self::bundled()
    }
}

impl JourneyChapter {
    fn quest_ids_for_class(&self, class_name: Option<&str>) -> Vec<i32> {
        let mut ids = self.quest_ids.clone();
        if let Some(class_ids) = case_insensitive_value(&self.class_quest_ids, class_name) {
            ids.extend(class_ids.iter().copied());
        }
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    fn class_hint(&self, class_name: Option<&str>) -> Option<&str> {
        case_insensitive_value(&self.class_hints, class_name)
            .map(String::as_str)
            .filter(|hint| !hint.trim().is_empty())
    }
}

fn case_insensitive_value<'a, T>(
    values: &'a BTreeMap<String, T>,
    key: Option<&str>,
) -> Option<&'a T> {
    let key = key?.trim();
    values
        .iter()
        .find_map(|(candidate, value)| candidate.eq_ignore_ascii_case(key).then_some(value))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JourneyView {
    pub chapter_id: String,
    pub chapter_title: String,
    pub level_range: String,
    pub completed_count: Option<usize>,
    pub quest_count: Option<usize>,
    pub goal: String,
    pub reward_summary: String,
    pub class_hint: Option<String>,
    pub graduated: bool,
    pub next: Option<JourneyStep>,
    pub optional: Vec<JourneyQuestSummary>,
}

impl JourneyView {
    pub fn progress_label(&self) -> String {
        match (self.completed_count, self.quest_count) {
            (Some(completed), Some(total)) => format!("Chapter {completed}/{total}"),
            _ => "Chapter progress syncing".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JourneyStep {
    pub quest_id: i32,
    pub title: String,
    pub action: String,
    pub location: Option<String>,
    pub objective: Option<String>,
    pub reward: Option<String>,
}

impl JourneyStep {
    fn from_quest(quest: &Quest, route: Option<&JourneyQuestOverride>) -> Self {
        let (action, location) = match quest.status {
            QuestStatus::ReadyToTurnIn if route.is_some_and(|route| route.finish_in_diary) => {
                ("Open Quest Diary to finish".to_owned(), None)
            }
            QuestStatus::ReadyToTurnIn => npc_action(
                "Return to",
                quest.npc_name.as_deref(),
                route.and_then(|route| route.finish_npc.as_ref()),
                "Turn in this quest",
            ),
            QuestStatus::InProgress => ("Continue this quest".to_owned(), None),
            QuestStatus::NotStarted if route.is_some_and(|route| route.start_in_diary) => {
                ("Open Quest Diary to accept".to_owned(), None)
            }
            QuestStatus::NotStarted => npc_action(
                "Talk to",
                quest.npc_name.as_deref(),
                route.and_then(|route| route.start_npc.as_ref()),
                "Accept from its quest giver",
            ),
            _ => (String::new(), None),
        };
        let objective = quest
            .objectives
            .iter()
            .find(|objective| !objective.is_complete())
            .or_else(|| quest.objectives.first())
            .map(|objective| {
                format!(
                    "{} ({})",
                    inline_text(&objective.text),
                    objective.progress_label()
                )
            });
        let reward = {
            let visible = quest
                .rewards
                .iter()
                .filter(|reward| !matches!(reward, QuestReward::Item { quantity: 0, .. }))
                .map(QuestReward::label)
                .collect::<Vec<_>>();
            (!visible.is_empty()).then(|| visible.join(", "))
        };
        Self {
            quest_id: quest.quest_index,
            title: quest.title.clone(),
            action,
            location,
            objective,
            reward,
        }
    }
}

fn inline_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn npc_action(
    verb: &str,
    live_name: Option<&str>,
    location: Option<&JourneyNpcLocation>,
    fallback: &str,
) -> (String, Option<String>) {
    let Some(location) = location else {
        return (
            live_name
                .map(|name| format!("{verb} {name}"))
                .unwrap_or_else(|| fallback.to_owned()),
            None,
        );
    };
    let name = live_name.unwrap_or(&location.name);
    let map = if location.map_name.trim().is_empty() {
        location.map_file_name.as_str()
    } else {
        location.map_name.as_str()
    };
    (
        format!("{verb} {name}"),
        Some(format!("{map} ({},{})", location.x, location.y)),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JourneyQuestSummary {
    pub quest_id: i32,
    pub title: String,
    pub status: String,
}

impl From<&Quest> for JourneyQuestSummary {
    fn from(quest: &Quest) -> Self {
        Self {
            quest_id: quest.quest_index,
            title: quest.title.clone(),
            status: quest.status.label(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    use crate::quest_model::{QuestObjective, QuestReward};

    fn quest(id: i32, title: &str, status: QuestStatus) -> Quest {
        Quest {
            quest_index: id,
            accept_npc_index: Some(3),
            finish_npc_index: Some(4),
            title: title.to_owned(),
            npc_name: Some("Guide".to_owned()),
            group: Some("BichonProvince".to_owned()),
            min_level_needed: 1,
            detail: Default::default(),
            status,
            objectives: vec![QuestObjective {
                objective_id: format!("{id}:0"),
                text: "Defeat monsters".to_owned(),
                current: 1,
                target: 5,
            }],
            rewards: vec![QuestReward::Experience { amount: 200 }],
            unknown_text: None,
        }
    }

    fn warrior(level: u32) -> PlayerStats {
        PlayerStats {
            level,
            class_name: Some("Warrior".to_owned()),
            ..Default::default()
        }
    }

    #[test]
    fn bundled_profile_has_six_contiguous_chapters_through_level_thirty() {
        let catalog = NewcomerJourneyCatalog::bundled();
        assert_eq!(catalog.max_level, 30);
        assert_eq!(catalog.chapters.len(), 6);
        for level in 1..=30 {
            assert!(
                catalog.chapter_for_level(level).is_some(),
                "missing level {level}"
            );
        }
        assert!(catalog.chapter_for_level(31).is_none());
    }

    #[test]
    fn chapter_progress_uses_only_authoritative_completed_ids() {
        let catalog = NewcomerJourneyCatalog::bundled();
        let guidance = QuestGuidance::from_profile_name("newcomer-v1");
        let tracker = QuestTracker::default();
        let unknown = CompletedQuestTracker::default();
        let view = catalog
            .derive(&guidance, &tracker, &unknown, &warrior(5))
            .unwrap();
        assert_eq!(view.progress_label(), "Chapter progress syncing");

        let mut completed = CompletedQuestTracker::default();
        completed.replace_authoritative([1, 4, 7]);
        let view = catalog
            .derive(&guidance, &tracker, &completed, &warrior(5))
            .unwrap();
        // Quest 4 is optional teaching content and is absent from the chapter route.
        assert_eq!((view.completed_count, view.quest_count), (Some(2), Some(8)));
        assert_eq!(view.progress_label(), "Chapter 2/8");
    }

    #[test]
    fn main_step_prefers_turn_in_and_does_not_promote_optional_teaching_quest() {
        let catalog = NewcomerJourneyCatalog::bundled();
        let guidance = QuestGuidance::from_profile_name("newcomer-v1");
        let tracker = QuestTracker {
            active_quests: vec![
                quest(1, "Main active", QuestStatus::InProgress),
                quest(4, "Optional meat", QuestStatus::ReadyToTurnIn),
                quest(5, "Main turn-in", QuestStatus::ReadyToTurnIn),
            ],
        };
        let view = catalog
            .derive(
                &guidance,
                &tracker,
                &CompletedQuestTracker::default(),
                &warrior(3),
            )
            .unwrap();
        assert_eq!(view.next.as_ref().map(|step| step.quest_id), Some(5));
        assert_eq!(
            view.optional
                .iter()
                .map(|quest| quest.quest_id)
                .collect::<Vec<_>>(),
            [4]
        );
    }

    #[test]
    fn class_hint_is_presentation_text_and_never_a_completion_signal() {
        let catalog = NewcomerJourneyCatalog::bundled();
        let guidance = QuestGuidance::from_profile_name("newcomer-v1");
        let completed = CompletedQuestTracker::default();
        let warrior = catalog
            .derive(&guidance, &QuestTracker::default(), &completed, &warrior(1))
            .unwrap();
        let wizard = catalog
            .derive(
                &guidance,
                &QuestTracker::default(),
                &completed,
                &PlayerStats {
                    level: 1,
                    class_name: Some("wizard".to_owned()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_ne!(warrior.class_hint, wizard.class_hint);
        assert_eq!(warrior.completed_count, None);
        assert_eq!(wizard.completed_count, None);
    }

    #[test]
    fn next_step_omits_fixed_items_with_zero_authoritative_quantity() {
        let mut source = quest(1, "No phantom sword", QuestStatus::InProgress);
        source.rewards = vec![QuestReward::Item {
            item_id: "37".to_owned(),
            name: "BrokenSword".to_owned(),
            quantity: 0,
            icon: None,
            selection_index: None,
            tooltip_source: None,
        }];

        let step = JourneyStep::from_quest(&source, None);
        assert_eq!(step.reward, None);
    }

    #[test]
    fn next_step_objective_collapses_multiline_server_text_to_one_layout_line() {
        let mut source = quest(1, "Welcome", QuestStatus::NotStarted);
        source.objectives[0].text =
            "Welcome to Bichon Province.\nBefore you begin, talk to Assistant Jane.".to_owned();

        let step = JourneyStep::from_quest(&source, None);
        assert_eq!(
            step.objective.as_deref(),
            Some("Welcome to Bichon Province. Before you begin, talk to Assistant Jane. (1 / 5)")
        );
        assert!(!step
            .objective
            .as_deref()
            .unwrap()
            .chars()
            .any(|character| matches!(character, '\n' | '\r')));
    }

    #[test]
    fn next_step_uses_reviewed_location_when_giver_is_outside_live_entities() {
        let source = quest(1, "Find Jane", QuestStatus::NotStarted);
        let route = JourneyQuestOverride {
            quest_id: 1,
            start_in_diary: false,
            finish_in_diary: false,
            start_npc: Some(JourneyNpcLocation {
                name: "Assistant Jane".to_owned(),
                map_name: "BichonProvince".to_owned(),
                map_file_name: "0".to_owned(),
                x: 284,
                y: 606,
            }),
            finish_npc: None,
        };

        let mut without_live_name = source;
        without_live_name.npc_name = None;
        let step = JourneyStep::from_quest(&without_live_name, Some(&route));
        assert_eq!(step.action, "Talk to Assistant Jane");
        assert_eq!(step.location.as_deref(), Some("BichonProvince (284,606)"));
    }

    #[test]
    fn diary_accept_and_finish_take_priority_over_npc_fallbacks() {
        let route = JourneyQuestOverride {
            quest_id: 22,
            start_in_diary: true,
            finish_in_diary: true,
            start_npc: None,
            finish_npc: None,
        };
        let start = quest(22, "Diary start", QuestStatus::NotStarted);
        let finish = quest(22, "Diary finish", QuestStatus::ReadyToTurnIn);

        assert_eq!(
            JourneyStep::from_quest(&start, Some(&route)).action,
            "Open Quest Diary to accept"
        );
        assert_eq!(
            JourneyStep::from_quest(&finish, Some(&route)).action,
            "Open Quest Diary to finish"
        );
    }

    #[test]
    fn extra_experience_does_not_skip_an_unfinished_earlier_chapter_quest() {
        let catalog = NewcomerJourneyCatalog::bundled();
        let guidance = QuestGuidance::from_profile_name("newcomer-v1");
        let tracker = QuestTracker {
            active_quests: vec![quest(9, "Learn Fencing", QuestStatus::NotStarted)],
        };
        let mut completed = CompletedQuestTracker::default();
        completed.replace_authoritative([1, 2, 3, 5, 6, 7, 8]);

        let view = catalog
            .derive(&guidance, &tracker, &completed, &warrior(6))
            .unwrap();
        assert_eq!(view.chapter_id, "village");
        assert_eq!(view.next.as_ref().map(|step| step.quest_id), Some(9));
        assert_eq!(view.progress_label(), "Chapter 7/8");
    }

    #[test]
    fn completed_full_class_route_is_a_graduated_journey_even_when_overleveled() {
        let catalog = NewcomerJourneyCatalog::bundled();
        let guidance = QuestGuidance::from_profile_name("newcomer-v1");
        let all_ids = catalog
            .chapters
            .iter()
            .flat_map(|chapter| chapter.quest_ids_for_class(Some("Warrior")))
            .collect::<BTreeSet<_>>();
        assert_eq!(all_ids.len(), 53);
        let mut completed = CompletedQuestTracker::default();
        completed.replace_authoritative(all_ids);

        let view = catalog
            .derive(
                &guidance,
                &QuestTracker::default(),
                &completed,
                &warrior(42),
            )
            .unwrap();
        assert!(view.graduated);
        assert_eq!(view.chapter_id, "island");
        assert_eq!(view.next, None);
        assert_eq!(view.completed_count, view.quest_count);
    }

    #[test]
    fn unknown_history_keeps_an_active_earlier_chapter_visible_when_overleveled() {
        let catalog = NewcomerJourneyCatalog::bundled();
        let guidance = QuestGuidance::from_profile_name("newcomer-v1");
        let tracker = QuestTracker {
            active_quests: vec![quest(9, "Learn Fencing", QuestStatus::InProgress)],
        };
        let view = catalog
            .derive(
                &guidance,
                &tracker,
                &CompletedQuestTracker::default(),
                &warrior(30),
            )
            .unwrap();
        assert_eq!(view.chapter_id, "village");
        assert_eq!(view.next.as_ref().map(|step| step.quest_id), Some(9));
        assert_eq!(view.completed_count, None);
    }

    #[test]
    fn crystal_mode_has_no_journey_projection() {
        assert!(NewcomerJourneyCatalog::bundled()
            .derive(
                &QuestGuidance::from_profile_name("crystal"),
                &QuestTracker::default(),
                &CompletedQuestTracker::default(),
                &warrior(1),
            )
            .is_none());
    }
}
