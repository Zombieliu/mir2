use super::*;

use crate::quest_model::{QuestObjective, QuestReward};

fn quest(id: i32, status: QuestStatus, objectives: Vec<QuestObjective>) -> Quest {
    Quest {
        quest_index: id,
        accept_npc_index: Some(3),
        finish_npc_index: Some(3),
        title: format!("V2 quest {id}"),
        npc_name: None,
        group: Some("BichonProvince".to_owned()),
        min_level_needed: 1,
        detail: Default::default(),
        status,
        objectives,
        rewards: vec![QuestReward::Experience { amount: 100 }],
        unknown_text: None,
    }
}

fn objective(id: &str, text: &str, current: u32, target: u32) -> QuestObjective {
    QuestObjective {
        objective_id: id.to_owned(),
        text: text.to_owned(),
        current,
        target,
    }
}

fn player(level: u32) -> PlayerStats {
    PlayerStats {
        level,
        class_name: Some("Taoist".to_owned()),
        ..Default::default()
    }
}

#[test]
fn v2_bundle_has_six_chapters_and_projects_all_twenty_six_server_tasks() {
    let guidance = QuestGuidance::from_profile_name("newcomer-v2");
    let catalog = NewcomerJourneyCatalog::from_guidance(&guidance);

    assert_eq!(catalog.profile, "newcomer-v2");
    assert_eq!(catalog.chapters.len(), 6);
    let ids = catalog
        .chapters
        .iter()
        .flat_map(|chapter| chapter.quest_ids.iter().copied())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(ids.len(), 26);
    assert!((2110001..=2110022).all(|id| ids.contains(&id)));
    assert!([2120015, 2120020, 2120025, 2120030]
        .into_iter()
        .all(|id| ids.contains(&id)));
    assert!(ids.iter().all(|id| guidance
        .entry(*id)
        .is_some_and(|entry| !entry.hint.trim().is_empty())));
}

#[test]
fn v2_chapter_selection_never_treats_level_as_completion() {
    let guidance = QuestGuidance::from_profile_name("newcomer-v2");
    let catalog = NewcomerJourneyCatalog::from_guidance(&guidance);
    let empty = QuestTracker::default();

    let unknown = catalog
        .derive(
            &guidance,
            &empty,
            &CompletedQuestTracker::default(),
            &player(30),
        )
        .expect("enabled V2 guidance projects a server-synced journey");
    assert_eq!(unknown.chapter_id, "village");
    assert_eq!(unknown.completed_count, None);

    let mut completed = CompletedQuestTracker::default();
    completed.replace_authoritative([
        2110001, 2110002, 2110003, 2110004, 2110005, 2110006, 2110007, 2110008, 2120015,
    ]);
    let projected = catalog
        .derive(&guidance, &empty, &completed, &player(30))
        .expect("authoritative history selects the next chapter");
    assert_eq!(projected.chapter_id, "cave");
    assert_eq!(projected.completed_count, Some(0));
}

#[test]
fn v2_uses_reviewed_jane_and_board_endpoints() {
    let guidance = QuestGuidance::from_profile_name("newcomer-v2");
    let catalog = NewcomerJourneyCatalog::from_guidance(&guidance);
    let tracker = QuestTracker {
        active_quests: vec![
            quest(2110001, QuestStatus::NotStarted, vec![]),
            quest(2110008, QuestStatus::ReadyToTurnIn, vec![]),
        ],
    };
    let view = catalog
        .derive(
            &guidance,
            &tracker,
            &CompletedQuestTracker::default(),
            &player(8),
        )
        .unwrap();
    let next = view.next.unwrap();
    assert_eq!(next.quest_id, 2110008);
    assert_eq!(next.action, "Return to Board");
    assert_eq!(next.location.as_deref(), Some("BichonProvince (334,259)"));

    let jane = JourneyStep::from_quest(
        &quest(2110001, QuestStatus::NotStarted, vec![]),
        catalog.quest_overrides.get(&2110001),
    );
    assert_eq!(jane.action, "Talk to Assistant Jane");
    assert_eq!(jane.location.as_deref(), Some("BichonProvince (284,606)"));
    assert_eq!(
        catalog
            .quest_overrides
            .get(&2110001)
            .and_then(|route| route.start_npc.as_ref())
            .and_then(|npc| npc.npc_id),
        Some(3)
    );
    assert_eq!(
        catalog
            .quest_overrides
            .get(&2110008)
            .and_then(|route| route.finish_npc.as_ref())
            .and_then(|npc| npc.npc_id),
        Some(24)
    );
}

#[test]
fn v2_keeps_at_most_three_server_objective_groups_in_the_hud_projection() {
    let guidance = QuestGuidance::from_profile_name("newcomer-v2");
    let catalog = NewcomerJourneyCatalog::from_guidance(&guidance);
    let source = quest(
        2110021,
        QuestStatus::InProgress,
        vec![
            objective("a", "Defeat Wooma Fighter", 1, 3),
            objective("b", "Commit class skill damage", 0, 1),
            objective("c", "Keep legal supplies", 0, 1),
            objective("d", "This fourth group is not in the compact HUD", 0, 1),
        ],
    );
    let step =
        JourneyStep::from_quest_with_objective_groups(&source, None, catalog.max_objective_groups);
    let text = step.objective.expect("objective projection");
    assert_eq!(text.matches(" • ").count(), 2);
    assert!(text.contains("Defeat Wooma Fighter"));
    assert!(!text.contains("fourth group"));
}

#[test]
fn v1_bundle_and_profile_selection_remain_available() {
    let guidance = QuestGuidance::from_profile_name("newcomer-v1");
    let catalog = NewcomerJourneyCatalog::from_guidance(&guidance);
    assert_eq!(catalog.profile, "newcomer-v1");
    assert_eq!(catalog.chapters.len(), 6);
}
