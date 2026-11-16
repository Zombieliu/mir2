use mir2_protocol::{ClientPacket, ServerPacket};

use super::*;
use crate::{SimulationConfig, SimulationSession};

fn authenticated_session(level: u16, newcomer_v1: bool) -> SimulationSession {
    let mut session = SimulationSession::new(SimulationConfig::default());
    let packets = session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    let character_index = packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::LoginSuccess { characters } => characters.first().map(|item| item.index),
            _ => None,
        })
        .expect("demo character");
    session.handle_packet(ClientPacket::StartGame { character_index });
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .selected_character
        .as_mut()
        .expect("selected character")
        .level = level;
    session
        .app
        .world_mut()
        .resource_mut::<QuestResource>()
        .newcomer_v1_cadence = newcomer_v1;
    session
}

fn insert_active_crystal_quest(session: &mut SimulationSession, quest_id: i32) {
    let world = session.app.world_mut();
    let info = effective_crystal_quest_info_by_id(world, quest_id).expect("quest info");
    let state = quest_state_from_effective_info(world, &info, QuestStage::InProgress);
    let quests = &mut world.resource_mut::<QuestResource>().quests;
    quests.retain(|quest| quest.quest_id != quest_id);
    quests.push(state);
}

#[test]
fn journey_json_overrides_info_and_templates_only_in_newcomer_mode() {
    let file = serde_json::from_str::<serde_json::Value>(include_str!(
        "../../../../config/quest-guidance/newcomer-journey-v1.json"
    ))
    .expect("journey config");
    let overrides = file["questOverrides"].as_array().expect("quest overrides");
    assert_eq!(overrides.len(), 63);

    let mut session = authenticated_session(30, false);
    for rule in overrides {
        let quest_id = rule["questId"].as_i64().expect("quest id") as i32;
        let raw_info = crystal_quest_info_by_id(quest_id).expect("Crystal quest info");
        let raw_template = crystal_quest_template_by_id(quest_id).expect("Crystal quest template");
        assert_eq!(
            effective_crystal_quest_info_by_id(session.app.world(), quest_id),
            Some(raw_info.clone()),
            "quest {quest_id} changed with the profile disabled"
        );
        assert_eq!(
            effective_crystal_quest_template_by_id(session.app.world(), quest_id),
            Some(raw_template.clone()),
            "quest {quest_id} template changed with the profile disabled"
        );
    }

    session
        .app
        .world_mut()
        .resource_mut::<QuestResource>()
        .newcomer_v1_cadence = true;
    for rule in overrides {
        let quest_id = rule["questId"].as_i64().expect("quest id") as i32;
        let info = effective_crystal_quest_info_by_id(session.app.world(), quest_id)
            .expect("effective quest info");
        let template = effective_crystal_quest_template_by_id(session.app.world(), quest_id)
            .expect("effective quest template");
        let raw_info = crystal_quest_info_by_id(quest_id).expect("Crystal quest info");
        assert_eq!(
            info.reward_exp,
            rule["rewardExperience"].as_u64().expect("reward XP") as u32,
            "quest {quest_id} reward mismatch"
        );
        assert_eq!(info.rewards_fixed_item, raw_info.rewards_fixed_item);
        assert_eq!(info.rewards_select_item, raw_info.rewards_select_item);
        if let Some(cap) = rule["killCountCap"].as_u64() {
            assert!(
                template
                    .kill_tasks
                    .iter()
                    .all(|task| task.count <= cap as u32),
                "quest {quest_id} kill cap was not applied"
            );
        }
        if let Some(cap) = rule["itemCountCap"].as_u64() {
            assert!(
                template
                    .item_tasks
                    .iter()
                    .all(|task| u64::from(task.count) <= cap),
                "quest {quest_id} item cap was not applied"
            );
        }
        if let Some(required) = rule["requiredQuestId"].as_i64() {
            assert_eq!(
                info.quest_needed, required as i32,
                "quest {quest_id} info prerequisite"
            );
            assert_eq!(
                template.required_quest, required as i32,
                "quest {quest_id} template prerequisite"
            );
        }
        if let Some(lines) = rule["taskDescription"].as_array() {
            assert_eq!(
                info.task_description,
                lines
                    .iter()
                    .map(|line| line.as_str().expect("task line").to_string())
                    .collect::<Vec<_>>(),
                "quest {quest_id} task copy mismatch"
            );
        }
    }

    for (quest_id, expected_book) in [(8, "Fencing"), (11, "FireBall"), (14, "Healing")] {
        let info = effective_crystal_quest_info_by_id(session.app.world(), quest_id).unwrap();
        assert!(
            info.rewards_fixed_item
                .iter()
                .chain(info.rewards_select_item.iter())
                .any(|reward| reward.item.name == expected_book && reward.count > 0),
            "quest {quest_id} no longer teaches {expected_book} through its book reward"
        );
    }
}

#[test]
fn reward_preview_omits_zero_count_items_but_keeps_real_rewards() {
    for quest_id in [37, 41, 112] {
        let info = crystal_quest_info_by_id(quest_id).expect("quest info with zero reward");
        let preview = crystal_quest_reward_preview(&info);
        let zero_names = info
            .rewards_fixed_item
            .iter()
            .chain(info.rewards_select_item.iter())
            .filter(|reward| reward.count == 0)
            .map(|reward| reward.item.name.as_str())
            .collect::<Vec<_>>();
        assert!(
            !zero_names.is_empty(),
            "quest {quest_id} test fixture drifted"
        );
        for name in zero_names {
            assert!(
                !preview.contains(name),
                "quest {quest_id} falsely previews zero-count {name}: {preview}"
            );
        }
    }

    let fencing = crystal_quest_info_by_id(8).expect("Fencing quest");
    assert!(crystal_quest_reward_preview(&fencing).contains("Fencing x1"));
}

#[test]
fn reconcile_keeps_progress_and_completed_rewards_when_profile_changes() {
    let mut session = authenticated_session(10, true);
    insert_active_crystal_quest(&mut session, 4);
    let template = effective_crystal_quest_template_by_id(session.app.world(), 4).unwrap();
    let task_key = crystal_item_task_key(template.item_tasks[0].item_index);
    {
        let mut quests = session.app.world_mut().resource_mut::<QuestResource>();
        let quest = quests
            .quests
            .iter_mut()
            .find(|quest| quest.quest_id == 4)
            .unwrap();
        quest.task_progress.insert(task_key.clone(), 1);
    }
    reconcile_effective_quest_states(session.app.world_mut());
    let quest = session
        .app
        .world()
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|quest| quest.quest_id == 4)
        .unwrap();
    assert_eq!(
        (quest.required, quest.current, quest.stage),
        (1, 1, QuestStage::ReadyToTurnIn)
    );

    session
        .app
        .world_mut()
        .resource_mut::<QuestResource>()
        .newcomer_v1_cadence = false;
    reconcile_effective_quest_states(session.app.world_mut());
    let quest = session
        .app
        .world()
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|quest| quest.quest_id == 4)
        .unwrap();
    assert_eq!(quest.current, 1);
    assert!(quest.required > 1);
    assert_eq!(quest.stage, QuestStage::InProgress);
    assert_eq!(quest.task_progress.get(&task_key), Some(&1));

    let completed_required = quest.required;
    session
        .app
        .world_mut()
        .resource_mut::<QuestResource>()
        .quests
        .iter_mut()
        .find(|quest| quest.quest_id == 4)
        .unwrap()
        .stage = QuestStage::Completed;
    session
        .app
        .world_mut()
        .resource_mut::<QuestResource>()
        .newcomer_v1_cadence = true;
    reconcile_effective_quest_states(session.app.world_mut());
    let quest = session
        .app
        .world()
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|quest| quest.quest_id == 4)
        .unwrap();
    assert_eq!(quest.stage, QuestStage::Completed);
    assert_eq!(quest.required, completed_required);
    assert_eq!(quest.task_progress.get(&task_key), Some(&1));
}

#[test]
fn q4_deer_harvest_guarantees_one_needed_task_item_without_duplicate_rolls() {
    let mut session = authenticated_session(10, true);
    insert_active_crystal_quest(&mut session, 4);
    let tick = super::super::resources::runtime_tick(session.app.world());
    let miss_object_id = (300_000..301_000)
        .find(|object_id| {
            !super::super::drops::resolved_monster_drop_templates_at_tick(
                *object_id,
                "Deer",
                tick,
            )
            .iter()
            .any(|drop| matches!(drop, super::super::drops::ResolvedDropTemplate::Item { name, quest_required: true, .. } if name == "DeerMeat"))
        })
        .expect("a deterministic DeerMeat miss");
    let hit_object_id = (301_000..302_000)
        .find(|object_id| {
            super::super::drops::resolved_monster_drop_templates_at_tick(
                *object_id,
                "Deer",
                tick,
            )
            .iter()
            .any(|drop| matches!(drop, super::super::drops::ResolvedDropTemplate::Item { name, quest_required: true, .. } if name == "DeerMeat"))
        })
        .expect("a deterministic DeerMeat hit");
    assert_eq!(
        super::super::drops::prepare_harvest_drops(
            session.app.world(),
            hit_object_id,
            "Deer",
        )
        .iter()
        .filter(|drop| matches!(drop, super::super::drops::ResolvedDropTemplate::Item { name, quest_required: true, .. } if name == "DeerMeat"))
        .count(),
        1,
        "an original Q roll must not receive a second guaranteed copy"
    );
    let prepared =
        super::super::drops::prepare_harvest_drops(session.app.world(), miss_object_id, "Deer");
    let guaranteed = prepared
        .iter()
        .filter(|drop| matches!(drop, super::super::drops::ResolvedDropTemplate::Item { name, quest_required: true, quantity: 1, .. } if name == "DeerMeat"))
        .count();
    assert_eq!(guaranteed, 1);

    let mut crystal = authenticated_session(10, false);
    insert_active_crystal_quest(&mut crystal, 4);
    assert!(!super::super::drops::prepare_harvest_drops(
        crystal.app.world(),
        miss_object_id,
        "Deer",
    )
    .iter()
    .any(|drop| matches!(drop, super::super::drops::ResolvedDropTemplate::Item { name, quest_required: true, .. } if name == "DeerMeat")));

    let (key, name, durability_current, durability_max) = prepared
        .iter()
        .find_map(|drop| match drop {
            super::super::drops::ResolvedDropTemplate::Item {
                key,
                name,
                durability_current,
                durability_max,
                quest_required: true,
                ..
            } if name == "DeerMeat" => Some((
                key.clone(),
                name.clone(),
                *durability_current,
                *durability_max,
            )),
            _ => None,
        })
        .unwrap();
    let mut packets = Vec::new();
    assert!(super::super::drops::try_gain_crystal_quest_drop(
        session.app.world_mut(),
        &key,
        &name,
        1,
        durability_current,
        durability_max,
        &mut packets,
    ));
    assert_eq!(
        quest_stage(session.app.world(), 4),
        Some(QuestStage::ReadyToTurnIn)
    );
    assert!(!super::super::drops::prepare_harvest_drops(
        session.app.world(),
        miss_object_id,
        "Deer",
    )
    .iter()
    .any(|drop| matches!(drop, super::super::drops::ResolvedDropTemplate::Item { name, quest_required: true, .. } if name == "DeerMeat")));

    let experience_before = session
        .app
        .world()
        .resource::<PlayerRuntimeResource>()
        .experience;
    assert!(complete_quest_with_selection(
        session.app.world_mut(),
        4,
        None
    ));
    assert_eq!(
        session
            .app
            .world()
            .resource::<PlayerRuntimeResource>()
            .experience,
        experience_before + 80
    );
    assert!(!complete_quest_with_selection(
        session.app.world_mut(),
        4,
        None
    ));
}

#[test]
fn shared_zone_kill_settlement_uses_newcomer_objective_caps() {
    let mut newcomer = authenticated_session(10, true);
    insert_active_crystal_quest(&mut newcomer, 5);
    for (offset, monster) in (0_u32..5).zip(std::iter::repeat("Deer")) {
        newcomer.commit_shared_monster_kill_award_transaction(410_000 + offset, monster, 0);
    }
    for (offset, monster) in (0_u32..5).zip(std::iter::repeat("Scarecrow")) {
        newcomer.commit_shared_monster_kill_award_transaction(420_000 + offset, monster, 0);
    }
    assert_eq!(
        quest_stage(newcomer.app.world(), 5),
        Some(QuestStage::ReadyToTurnIn)
    );

    let mut crystal = authenticated_session(10, false);
    insert_active_crystal_quest(&mut crystal, 5);
    for (offset, monster) in (0_u32..5).zip(std::iter::repeat("Deer")) {
        crystal.commit_shared_monster_kill_award_transaction(430_000 + offset, monster, 0);
    }
    for (offset, monster) in (0_u32..5).zip(std::iter::repeat("Scarecrow")) {
        crystal.commit_shared_monster_kill_award_transaction(440_000 + offset, monster, 0);
    }
    assert_eq!(
        quest_stage(crystal.app.world(), 5),
        Some(QuestStage::InProgress)
    );
}

#[test]
fn shared_normal_q_miss_is_supplemented_once_but_profile_off_and_full_bag_are_not() {
    let mut newcomer = authenticated_session(10, true);
    insert_active_crystal_quest(&mut newcomer, 2);
    let tick = super::super::resources::runtime_tick(newcomer.app.world());
    let miss_object_id = (500_000..501_000)
        .find(|object_id| {
            !super::super::drops::resolved_monster_drop_templates_at_tick(
                *object_id,
                "Scarecrow",
                tick,
            )
            .iter()
            .any(|drop| matches!(drop, super::super::drops::ResolvedDropTemplate::Item { name, .. } if name == "GingerTea"))
        })
        .expect("a deterministic GingerTea miss");
    let receipt =
        newcomer.commit_shared_monster_kill_award_transaction(miss_object_id, "Scarecrow", 0);
    assert_eq!(
        receipt
            .packets
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::GainedItem { item } if item.item_index == 1112))
            .count(),
        1
    );
    assert_eq!(
        quest_stage(newcomer.app.world(), 2),
        Some(QuestStage::ReadyToTurnIn)
    );

    let mut crystal = authenticated_session(10, false);
    insert_active_crystal_quest(&mut crystal, 2);
    let receipt =
        crystal.commit_shared_monster_kill_award_transaction(miss_object_id, "Scarecrow", 0);
    assert!(!receipt.packets.iter().any(
        |packet| matches!(packet, ServerPacket::GainedItem { item } if item.item_index == 1112)
    ));
    assert_eq!(
        quest_stage(crystal.app.world(), 2),
        Some(QuestStage::InProgress)
    );

    let mut full = authenticated_session(10, true);
    insert_active_crystal_quest(&mut full, 2);
    for slot in 0..40_u8 {
        add_or_increment_item(
            full.app.world_mut(),
            crate::config::ItemContainer::Quest,
            &format!("journey-full-{slot}"),
            &format!("Journey Filler {slot}"),
            "Test quest inventory filler.",
            slot,
            1,
            0,
        );
    }
    let receipt = full.commit_shared_monster_kill_award_transaction(miss_object_id, "Scarecrow", 0);
    assert!(!receipt.packets.iter().any(
        |packet| matches!(packet, ServerPacket::GainedItem { item } if item.item_index == 1112)
    ));
    assert_eq!(
        quest_stage(full.app.world(), 2),
        Some(QuestStage::InProgress)
    );
}

#[test]
fn harvest_native_ingredient_roll_is_preserved_beside_one_quest_item() {
    let mut session = authenticated_session(20, true);
    insert_active_crystal_quest(&mut session, 25);
    let tick = super::super::resources::runtime_tick(session.app.world());
    let object_id = (610_000..611_000)
        .find(|object_id| {
            super::super::drops::resolved_monster_drop_templates_at_tick(
                *object_id,
                "CannibalPlant",
                tick,
            )
            .iter()
            .any(|drop| matches!(drop, super::super::drops::ResolvedDropTemplate::Item { name, quest_required: false, .. } if name == "CannibalLeaf"))
        })
        .expect("a deterministic ordinary CannibalLeaf roll");
    let raw = super::super::drops::resolved_monster_drop_templates_at_tick(
        object_id,
        "CannibalPlant",
        tick,
    );
    let ordinary_leaf_before = raw
        .iter()
        .filter_map(|drop| match drop {
            super::super::drops::ResolvedDropTemplate::Item {
                name,
                quantity,
                quest_required: false,
                ..
            } if name == "CannibalLeaf" => Some(*quantity),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut supplemented = raw;
    super::super::drops::supplement_guaranteed_quest_drops(
        session.app.world(),
        object_id,
        "CannibalPlant",
        &mut supplemented,
    );
    let ordinary_leaf_after = supplemented
        .iter()
        .filter_map(|drop| match drop {
            super::super::drops::ResolvedDropTemplate::Item {
                name,
                quantity,
                quest_required: false,
                ..
            } if name == "CannibalLeaf" => Some(*quantity),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(ordinary_leaf_after, ordinary_leaf_before);
    assert_eq!(
        supplemented
            .iter()
            .filter(|drop| matches!(drop, super::super::drops::ResolvedDropTemplate::Item { name, quest_required: true, quantity: 1, .. } if name == "CannibalLeaf"))
            .count(),
        1
    );

    let mut packets = Vec::new();
    for drop in supplemented {
        let super::super::drops::ResolvedDropTemplate::Item {
            key,
            name,
            quantity,
            durability_current,
            durability_max,
            quest_required: true,
            ..
        } = drop
        else {
            continue;
        };
        assert!(super::super::drops::try_gain_crystal_quest_drop(
            session.app.world_mut(),
            &key,
            &name,
            quantity,
            durability_current,
            durability_max,
            &mut packets,
        ));
    }
    let gained = packets
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::GainedItem { item } => Some(item.item_index),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        gained.iter().filter(|index| **index == 1114).count(),
        1,
        "CannibalStem packets: {packets:?}"
    );
    assert_eq!(
        gained.iter().filter(|index| **index == 866).count(),
        1,
        "CannibalLeaf packets: {packets:?}"
    );
    let quest = session
        .app
        .world()
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|quest| quest.quest_id == 25)
        .unwrap();
    assert_eq!((quest.current, quest.required), (2, 2));
    assert_eq!(quest.stage, QuestStage::ReadyToTurnIn);
}
