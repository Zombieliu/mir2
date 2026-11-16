use super::*;

fn player(class_name: &str, level: u32) -> PlayerStats {
    PlayerStats {
        level,
        class_name: Some(class_name.to_owned()),
        ..Default::default()
    }
}

#[test]
fn graduation_requires_the_authoritative_v2_route_growth_claims_and_level() {
    let guidance = QuestGuidance::from_profile_name("newcomer-v2");
    let catalog = NewcomerJourneyCatalog::from_guidance(&guidance);
    let mut completed = CompletedQuestTracker::default();
    completed.replace_authoritative(
        V2_GRADUATION_COMPLETION_IDS
            .iter()
            .copied()
            .filter(|quest_id| *quest_id != 2_120_030),
    );
    assert!(catalog
        .derive(
            &guidance,
            &QuestTracker::default(),
            &completed,
            &player("Warrior", 30)
        )
        .is_some_and(|view| view.graduation.is_none()));

    completed.replace_authoritative(V2_GRADUATION_COMPLETION_IDS.iter().copied());
    assert!(catalog
        .derive(
            &guidance,
            &QuestTracker::default(),
            &completed,
            &player("Warrior", 29)
        )
        .is_some_and(|view| view.graduation.is_none()));
}

#[test]
fn graduation_projects_only_the_matching_class_three_local_targets() {
    let guidance = QuestGuidance::from_profile_name("newcomer-v2");
    let catalog = NewcomerJourneyCatalog::from_guidance(&guidance);
    let mut completed = CompletedQuestTracker::default();
    completed.replace_authoritative(V2_GRADUATION_COMPLETION_IDS.iter().copied());

    let wizard = catalog
        .derive(
            &guidance,
            &QuestTracker::default(),
            &completed,
            &player("Wizard", 30),
        )
        .and_then(|view| view.graduation)
        .expect("authoritative V2 completion unlocks graduation guidance");
    assert_eq!(wizard.title, "Wooma Graduation");
    assert_eq!(wizard.options.len(), 3);
    assert_eq!(
        wizard
            .option(GraduationDirection::Equipment)
            .map(|option| option.title.as_str()),
        Some("WarMageStaff")
    );
    assert_eq!(
        wizard
            .option(GraduationDirection::Skill)
            .map(|option| option.title.as_str()),
        Some("ThunderStorm")
    );
    let challenge = wizard
        .option(GraduationDirection::Challenge)
        .expect("shared free-roam direction");
    assert_eq!(challenge.summary, "Medium difficulty · Insect Cave N 2F");
    assert!(challenge
        .instruction
        .starts_with("Reach Insect Cave N 2F from Insect Cave W 1F"));
    assert!(!challenge.instruction.contains("D605"));
    assert!(!challenge.instruction.contains("P176"));
}

#[test]
fn graduation_names_and_map_use_the_shared_ten_language_catalog() {
    use crate::native_i18n::{self, Locale};

    let guidance = QuestGuidance::from_profile_name("newcomer-v2");
    let catalog = NewcomerJourneyCatalog::from_guidance(&guidance);
    let mut completed = CompletedQuestTracker::default();
    completed.replace_authoritative(V2_GRADUATION_COMPLETION_IDS.iter().copied());
    let map = mir2_game_data::crystal_map_respawns_ref("D605").unwrap();
    assert_eq!(map.map_index, 186);

    for locale in Locale::ALL {
        native_i18n::with_locale(locale, || {
            for (class, item_index, raw_item) in [
                ("Warrior", 244, "JudgementMace"),
                ("Wizard", 259, "WarMageStaff"),
                ("Taoist", 272, "SoulSpringWand"),
            ] {
                let view = catalog
                    .derive(
                        &guidance,
                        &QuestTracker::default(),
                        &completed,
                        &player(class, 30),
                    )
                    .and_then(|view| view.graduation)
                    .unwrap();
                let equipment = view.option(GraduationDirection::Equipment).unwrap();
                assert_eq!(
                    equipment.title,
                    native_i18n::for_locale(
                        locale,
                        &format!("content.item.{item_index}.name"),
                        raw_item,
                    ),
                    "{locale:?}/{class}"
                );
                let challenge = view.option(GraduationDirection::Challenge).unwrap();
                let map_name =
                    native_i18n::for_locale(locale, "content.map.186.name", "Insect Cave N 2F");
                assert_eq!(
                    challenge.summary,
                    native_i18n::format_for(
                        locale,
                        "quest.feedback.83",
                        "{difficulty} difficulty · {map}",
                        &[
                            ("difficulty", &native_i18n::tr("Medium")),
                            ("map", &map_name)
                        ],
                    )
                );
                assert_eq!(
                    challenge.title,
                    native_i18n::for_locale(
                        locale,
                        "quest.journey.copy.56",
                        "Insect Cave N 2F sweep",
                    )
                );
                assert_ne!(challenge.title, map_name);
                for text in [&equipment.title, &challenge.title, &challenge.summary] {
                    for raw in [
                        raw_item,
                        "N_2FofDungeon",
                        "D605",
                        "P176",
                        "content.item.",
                        "content.map.",
                    ] {
                        assert!(!text.contains(raw), "{locale:?}/{class}: {raw}");
                    }
                }
            }
        });
    }
}

#[test]
fn v1_and_crystal_profiles_never_project_v2_graduation_choices() {
    let mut completed = CompletedQuestTracker::default();
    completed.replace_authoritative(V2_GRADUATION_COMPLETION_IDS.iter().copied());

    let v1_guidance = QuestGuidance::from_profile_name("newcomer-v1");
    assert!(NewcomerJourneyCatalog::from_guidance(&v1_guidance)
        .derive(
            &v1_guidance,
            &QuestTracker::default(),
            &completed,
            &player("Warrior", 30),
        )
        .is_some_and(|view| view.graduation.is_none()));
    assert!(NewcomerJourneyCatalog::bundled()
        .derive(
            &QuestGuidance::from_profile_name("crystal"),
            &QuestTracker::default(),
            &completed,
            &player("Warrior", 30),
        )
        .is_none());
}
