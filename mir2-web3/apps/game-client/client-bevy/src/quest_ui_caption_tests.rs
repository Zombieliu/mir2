//! Real shared diary trees: display copy cannot alter authority or hit targets.
use super::*;
use crate::native_i18n::{self, Locale};

fn quest() -> Quest {
    Quest {
        quest_index: 2_110_001,
        accept_npc_index: Some(250),
        finish_npc_index: Some(250),
        title: "Original server title".into(),
        npc_name: None,
        group: None,
        min_level_needed: 7,
        detail: default(),
        status: crate::quest_model::QuestStatus::InProgress,
        objectives: vec![],
        rewards: vec![],
        unknown_text: None,
    }
}

fn journey() -> JourneyView {
    JourneyView {
        chapter_id: "village".into(),
        chapter_title: "R16x{chapter}".into(),
        level_range: "Lv1-7".into(),
        completed_count: Some(2),
        quest_count: Some(7),
        goal: String::new(),
        reward_summary: String::new(),
        class_hint: None,
        graduated: false,
        graduation: None,
        next: None,
        optional: vec![],
    }
}

fn tree(tracker: &QuestTracker, journey: Option<&JourneyView>, text: QuestRenderText) -> World {
    let mut world = World::new();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    Commands::new(&mut queue, &world)
        .spawn(Node::default())
        .with_children(|parent| {
            render_guided_quest_diary_panel(
                parent,
                tracker,
                &QuestGuidance::from_profile_name("newcomer-v2"),
                journey,
                &QuestUiState::default(),
                None,
                text,
            );
        });
    queue.apply(&mut world);
    world
}

#[test]
fn actual_guided_diary_ten_locale_chrome_counters_and_opaque_heading_keep_actions() {
    let tracker = QuestTracker {
        active_quests: vec![quest()],
    };
    let view = journey();
    let original_tracker = tracker.clone();
    let original_view = view.clone();
    let mut baseline = None;
    for locale in Locale::ALL {
        native_i18n::with_locale(locale, || {
            let text = QuestRenderText::default();
            let mut world = tree(&tracker, Some(&view), text);
            let texts = world
                .query::<&Text>()
                .iter(&world)
                .map(|t| t.0.clone())
                .collect::<Vec<_>>();
            for expected in [
                native_i18n::format_for(
                    locale,
                    "quest.ui.chapterProgress",
                    "",
                    &[("done", "2"), ("total", "7")],
                ),
                native_i18n::format_for(
                    locale,
                    "quest.ui.chapter",
                    "",
                    &[("chapter", "R16x{chapter}")],
                ),
                native_i18n::format_for(
                    locale,
                    "quest.ui.levelStatus",
                    "",
                    &[
                        ("level", "7"),
                        (
                            "status",
                            &native_i18n::for_locale(locale, "ui.questStage.inProgress", ""),
                        ),
                        ("area", "R16x{chapter}"),
                    ],
                ),
            ] {
                assert!(
                    texts.contains(&expected),
                    "{locale:?}: missing {expected:?} in {texts:?}"
                );
            }
            for (key, rect, enabled, action) in [
                (
                    "ui.previous",
                    QuestLogRect::new(22., 408., 62., 22.),
                    false,
                    None,
                ),
                (
                    "ui.next",
                    QuestLogRect::new(228., 408., 62., 22.),
                    false,
                    None,
                ),
                (
                    "ui.close",
                    quest_diary_layout(1.).bottom_close,
                    true,
                    Some(QuestUiButton::CloseQuestLog),
                ),
            ] {
                let (entity, _) = world
                    .query::<(Entity, &Node, &QuestUiButtonVisual)>()
                    .iter(&world)
                    .find_map(|(e, n, v)| {
                        (n.left == Val::Px(rect.left)
                            && n.top == Val::Px(rect.top)
                            && n.width == Val::Px(rect.width)
                            && n.height == Val::Px(rect.height))
                        .then_some((e, v.enabled))
                    })
                    .filter(|(_, actual)| *actual == enabled)
                    .expect("same original control rectangle");
                assert_eq!(world.get::<Button>(entity).is_some(), enabled);
                assert_eq!(world.get::<QuestUiButton>(entity).cloned(), action);
                let caption = native_i18n::for_locale(locale, key, "missing Native key");
                assert!(
                    world
                        .query::<(&Text, &ChildOf)>()
                        .iter(&world)
                        .any(|(t, p)| p.parent() == entity && t.0 == caption),
                    "{locale:?} {key}"
                );
            }
            let actual = world
                .query::<(&QuestUiButton, &Node)>()
                .iter(&world)
                .map(|(a, n)| (a.clone(), n.left, n.top, n.width, n.height))
                .collect::<Vec<_>>();
            if let Some(expected) = &baseline {
                assert_eq!(&actual, expected);
            } else {
                baseline = Some(actual);
            }
            assert_eq!(tracker, original_tracker);
            assert_eq!(view, original_view);
        });
    }
}

#[test]
fn chapter_syncing_and_progress_use_native_keys_without_fabricating_completion() {
    for locale in Locale::ALL {
        native_i18n::with_locale(locale, || {
            let mut view = journey();
            assert_eq!(
                view.progress_label(),
                native_i18n::format_for(
                    locale,
                    "quest.chapter.progress",
                    "",
                    &[("progress", "2/7")]
                )
            );
            view.completed_count = None;
            assert_eq!(
                view.progress_label(),
                native_i18n::for_locale(locale, "quest.chapter.syncing", "")
            );
            let mut world = tree(
                &QuestTracker::default(),
                Some(&view),
                QuestRenderText::default(),
            );
            let expected = native_i18n::for_locale(locale, "quest.ui.chapterSyncing", "");
            assert!(world
                .query::<&Text>()
                .iter(&world)
                .any(|text| text.0 == expected));
            assert_eq!(view.completed_count, None);
            assert_eq!(view.quest_count, Some(7));
        });
    }
}

#[test]
fn fallback_journey_header_localizes_level_range_once_and_keeps_placeholder_text() {
    for locale in Locale::ALL {
        native_i18n::with_locale(locale, || {
            let view = journey();
            let mut world = World::new();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            Commands::new(&mut queue, &world)
                .spawn(Node::default())
                .with_children(|parent| {
                    render_quest_tracker_panel(
                        parent,
                        &QuestTracker::default(),
                        &QuestUiState::default(),
                        Some(&view),
                        &EntityModelSet::default(),
                        &MapModel::default(),
                        None,
                        "Warrior",
                        None,
                        QuestRenderText::default(),
                    );
                });
            queue.apply(&mut world);
            let expected = native_i18n::format_for(
                locale,
                "quest.ui.levelChapter",
                "",
                &[("level", "1-7"), ("chapter", "R16x{chapter}")],
            );
            assert!(world
                .query::<&Text>()
                .iter(&world)
                .any(|text| text.0 == expected));
            assert_eq!(
                view.level_range, "Lv1-7",
                "wire-neutral view remains canonical"
            );
            assert_eq!(view.completed_count, Some(2));
            assert!(world.query::<&QuestUiButton>().iter(&world).all(|button| {
                !matches!(
                    button,
                    QuestUiButton::AcceptQuest { .. } | QuestUiButton::FinishQuest { .. }
                )
            }));
        });
    }
}

#[test]
fn portable_catalog_contains_exact_native_chrome_keys_instead_of_test_only_full_catalog() {
    let thin: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../packages/game-data/data/generated/portable-native-i18n.json"
    ))
    .unwrap();
    let entries = thin["entries"].as_array().unwrap();
    for key in [
        "ui.previous",
        "ui.next",
        "ui.close",
        "quest.ui.chapterProgress",
        "quest.ui.chapterSyncing",
        "quest.ui.chapter",
        "quest.ui.levelStatus",
        "quest.ui.levelTitle",
        "quest.chapter.progress",
        "quest.chapter.syncing",
        "game.006",
        "game.008",
        "game.009",
        "game.007",
    ] {
        let entry = entries
            .iter()
            .find(|entry| entry["key"] == key)
            .unwrap_or_else(|| panic!("missing portable display key {key}"));
        for locale in Locale::ALL {
            assert_eq!(
                entry["values"][locale.index()].as_str().unwrap(),
                native_i18n::for_locale(locale, key, "missing"),
                "{key} {locale:?}"
            );
        }
    }
}
