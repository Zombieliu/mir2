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
    assert!(
        catalog
            .derive(
                &guidance,
                &QuestTracker::default(),
                &completed,
                &player("Warrior", 30)
            )
            .is_some_and(|view| view.graduation.is_none())
    );

    completed.replace_authoritative(V2_GRADUATION_COMPLETION_IDS.iter().copied());
    assert!(
        catalog
            .derive(
                &guidance,
                &QuestTracker::default(),
                &completed,
                &player("Warrior", 29)
            )
            .is_some_and(|view| view.graduation.is_none())
    );
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
    assert!(
        challenge
            .instruction
            .starts_with("Reach Insect Cave N 2F from Insect Cave W 1F")
    );
    assert!(!challenge.instruction.contains("D605"));
    assert!(!challenge.instruction.contains("P176"));
}

#[test]
fn v1_and_crystal_profiles_never_project_v2_graduation_choices() {
    let mut completed = CompletedQuestTracker::default();
    completed.replace_authoritative(V2_GRADUATION_COMPLETION_IDS.iter().copied());

    let v1_guidance = QuestGuidance::from_profile_name("newcomer-v1");
    assert!(
        NewcomerJourneyCatalog::from_guidance(&v1_guidance)
            .derive(
                &v1_guidance,
                &QuestTracker::default(),
                &completed,
                &player("Warrior", 30),
            )
            .is_some_and(|view| view.graduation.is_none())
    );
    assert!(
        NewcomerJourneyCatalog::bundled()
            .derive(
                &QuestGuidance::from_profile_name("crystal"),
                &QuestTracker::default(),
                &completed,
                &player("Warrior", 30),
            )
            .is_none()
    );
}
