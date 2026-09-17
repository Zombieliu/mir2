use bevy_ecs::prelude::World;
use std::collections::BTreeMap;

use super::*;

fn quest(id: i32) -> &'static Definition {
    definition(id).expect("configured V2 definition")
}

#[test]
fn v2_defines_exactly_twenty_two_main_nodes_and_four_growth_claims() {
    assert_eq!(definitions().len(), 26);
    let main = definitions()
        .iter()
        .filter(|definition| (2_110_001..=2_110_022).contains(&definition.info.index))
        .count();
    let growth = definitions()
        .iter()
        .filter(|definition| (2_120_015..=2_120_030).contains(&definition.info.index))
        .count();
    assert_eq!((main, growth), (22, 4));
    assert!(is_v2_quest(2_110_022));
    assert!(is_v2_quest(2_120_025));
    assert!(!is_v2_quest(2_100_015));
}

#[test]
fn multi_kill_and_class_practice_nodes_preserve_distinct_objectives() {
    let opening = quest(2_110_003);
    assert_eq!(opening.template.kill_tasks.len(), 2);
    assert_eq!(opening.template.kill_tasks[0].monster_name, "Scarecrow");
    assert_eq!(opening.template.kill_tasks[1].monster_name, "RakingCat");

    let practice = quest(2_110_004);
    assert_eq!(practice.template.kill_tasks.len(), 1);
    assert_eq!(practice.template.flag_tasks.len(), 1);
    let flag = &practice.flags[0];
    assert_eq!(flag.number, 2_210_041);
    assert_eq!(flag.kind, FlagKind::ClassPractice);
    assert_eq!(
        flag.requirements
            .as_ref()
            .unwrap()
            .for_class(MirClass::Wizard),
        ["FireBall learned", "FireBall damage committed"],
    );
}

#[test]
fn growth_claims_are_gated_after_the_required_main_nodes() {
    let level15 = quest(2_120_015);
    assert_eq!(level15.info.min_level_needed, 15);
    assert_eq!(level15.dependencies, vec![2_110_007]);

    let level25 = quest(2_120_025);
    assert_eq!(level25.info.min_level_needed, 25);
    assert_eq!(level25.dependencies, vec![2_110_016]);

    let node17 = quest(2_110_017);
    assert!(node17.dependencies.contains(&2_120_025));
    assert!(node17.dependencies.contains(&2_110_016));
}

#[test]
fn rewards_are_manifest_backed_and_gender_specific_armour_is_selected_safely() {
    let level25 = quest(2_120_025);
    let warrior = level25.rewards.class_items(MirClass::Warrior);
    assert_eq!(warrior[0].name_for(MirGender::Male), Some("ThickArmour(M)"));
    assert_eq!(
        warrior[0].name_for(MirGender::Female),
        Some("ThickArmour(F)")
    );
    assert!(reward_packet(&warrior[0], MirGender::Male).is_some());
    assert!(reward_packet(&warrior[0], MirGender::Female).is_some());

    let wizard = quest(2_110_003).rewards.class_items(MirClass::Wizard);
    assert!(wizard
        .iter()
        .any(|reward| reward.name.as_deref() == Some("FireBall")));
}

#[test]
fn mode_is_explicit_and_v1_profile_or_claims_are_incompatible() {
    let mut disabled = World::new();
    disabled.insert_resource(QuestResource {
        quests: Vec::new(),
        newcomer_v1_cadence: false,
        newcomer_v2_cadence: false,
    });
    assert!(!enabled(&disabled));
    assert!(!quest_template_by_id(&disabled, 2_110_003).is_some());
    assert!(can_finish(&disabled, 4));

    let mut enabled_world = World::new();
    enabled_world.insert_resource(QuestResource {
        quests: Vec::new(),
        newcomer_v1_cadence: false,
        newcomer_v2_cadence: true,
    });
    assert!(enabled(&enabled_world));
    assert!(objective_map_matches(&enabled_world, 2_110_010, "D001"));
    assert!(!objective_map_matches(&enabled_world, 2_110_010, "D401"));

    let mut v1 = World::new();
    v1.insert_resource(QuestResource {
        quests: Vec::new(),
        newcomer_v1_cadence: true,
        newcomer_v2_cadence: true,
    });
    assert!(!enabled(&v1));

    assert!(super::super::newcomer_progression::is_journey_quest(4));
    let mut saved_v1_main = World::new();
    saved_v1_main.insert_resource(QuestResource {
        quests: vec![super::super::QuestState {
            quest_id: 4,
            title: "V1 progress".to_string(),
            summary: String::new(),
            reward_preview: String::new(),
            required: 1,
            current: 1,
            stage: crate::config::QuestStage::Completed,
            task_progress: BTreeMap::new(),
            cadence_last_claimed_period: None,
            cadence_high_watermark_period: None,
        }],
        newcomer_v1_cadence: false,
        newcomer_v2_cadence: true,
    });
    assert!(!enabled(&saved_v1_main));

    let mut available_guide = World::new();
    available_guide.insert_resource(QuestResource {
        quests: vec![super::super::QuestState {
            quest_id: super::super::super::crystal_compat::GUIDE_QUEST_ID,
            title: "Guide".to_string(),
            summary: String::new(),
            reward_preview: String::new(),
            required: 1,
            current: 0,
            stage: crate::config::QuestStage::Available,
            task_progress: BTreeMap::new(),
            cadence_last_claimed_period: None,
            cadence_high_watermark_period: None,
        }],
        newcomer_v1_cadence: false,
        newcomer_v2_cadence: true,
    });
    assert!(enabled(&available_guide));
}
