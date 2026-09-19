use mir2_protocol::{ClientPacket, MirClass, MirGender, MirGridType, ServerPacket};

use super::*;
use crate::{SimulationConfig, SimulationSession};

const DAY_MS: u64 = 86_400_000;

fn authenticated_session(level: u16) -> SimulationSession {
    authenticated_character(level, MirClass::Warrior, MirGender::Male)
}

fn authenticated_character(level: u16, class: MirClass, gender: MirGender) -> SimulationSession {
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
    {
        let mut resource = session.app.world_mut().resource_mut::<SessionResource>();
        let character = resource
            .selected_character
            .as_mut()
            .expect("selected character");
        character.level = level;
        character.class = class;
        character.gender = gender;
    }
    session
        .app
        .world_mut()
        .resource_mut::<QuestResource>()
        .newcomer_v1_cadence = true;
    session
}

fn accept(session: &mut SimulationSession, quest_id: i32) -> Vec<ServerPacket> {
    super::super::packets::stage5_accept_quest_packet(session.app.world_mut(), quest_id)
}

fn finish(session: &mut SimulationSession, quest_id: i32) -> Vec<ServerPacket> {
    super::super::packets::stage5_finish_quest_packet(session.app.world_mut(), quest_id, None)
}

#[test]
fn newcomer_catalog_is_profile_gated_and_bound_to_the_real_board_object() {
    let mut session = authenticated_session(15);
    let world = session.app.world();
    let infos = newcomer_progression::quest_infos(world);
    assert_eq!(infos.len(), 10);
    assert!(infos.iter().all(|info| {
        info.npc_index == newcomer_progression::NEWCOMER_BOARD_OBJECT_ID
            && info.finish_npc_index == newcomer_progression::NEWCOMER_BOARD_OBJECT_ID
            && crystal_quest_info_by_id(info.index).is_none()
    }));
    let options = newcomer_progression::DAILY_OPTION_IDS
        .iter()
        .map(|quest_id| crystal_quest_template_by_id(*quest_id).expect("daily template"))
        .collect::<Vec<_>>();
    assert_eq!(options[0].kill_tasks[0].monster_name, "OmaFighter");
    assert_eq!(options[1].kill_tasks[0].monster_name, "Skeleton");
    assert_eq!(options[2].kill_tasks[0].monster_name, "ForestYeti");
    assert!(newcomer_progression::quest_ids_for_npc(
        world,
        newcomer_progression::NEWCOMER_BOARD_OBJECT_ID
    )
    .contains(&newcomer_progression::DAILY_BONUS_ID));
    assert!(newcomer_progression::quest_ids_for_npc(world, 17).is_empty());
    let option_info =
        newcomer_progression::quest_info_by_id_unchecked(newcomer_progression::DAILY_OPTION_IDS[0])
            .unwrap();
    assert!(crystal_quest_start_npc_matches(
        &option_info,
        newcomer_progression::NEWCOMER_BOARD_OBJECT_ID
    ));
    assert!(crystal_quest_finish_npc_matches(
        &option_info,
        newcomer_progression::NEWCOMER_BOARD_OBJECT_ID
    ));
    assert!(!crystal_quest_start_npc_matches(&option_info, 17));
    assert!(!crystal_quest_finish_npc_matches(&option_info, 17));

    session
        .app
        .world_mut()
        .resource_mut::<QuestResource>()
        .newcomer_v1_cadence = false;
    assert!(newcomer_progression::quest_infos(session.app.world()).is_empty());
    assert!(effective_crystal_quest_info_by_id(
        session.app.world(),
        newcomer_progression::DAILY_OPTION_IDS[0]
    )
    .is_none());
}

#[test]
fn daily_objective_snapshot_keeps_counts_out_of_the_label() {
    let quest_id = newcomer_progression::DAILY_OPTION_IDS[0];
    let info = newcomer_progression::quest_info_by_id_unchecked(quest_id).unwrap();
    let mut state = QuestState::from_crystal_info(&info, QuestStage::InProgress);
    state.task_progress.insert("kill:50".to_string(), 4);
    state.current = 4;

    let snapshot = state.snapshot(mir2_game_data::LanguageCode::English);
    assert_eq!(snapshot.objectives.len(), 1);
    let objective = &snapshot.objectives[0];
    assert_eq!(objective.label, "Defeat OmaFighter");
    assert_eq!((objective.current, objective.required), (4, 10));
    assert!(!objective.label.contains("{0}"));
    assert!(!objective.label.contains("/10"));
}

#[test]
fn daily_assignments_use_standard_accept_kill_finish_and_enforce_two_choices() {
    let mut session = authenticated_session(15);
    let [first, second, third] = newcomer_progression::DAILY_OPTION_IDS;
    assert!(!accept(&mut session, first).is_empty());
    assert!(!accept(&mut session, second).is_empty());
    assert!(accept(&mut session, third).iter().any(|packet| matches!(
        packet,
        ServerPacket::Chat {
            chat_type: mir2_protocol::ChatType::System,
            ..
        }
    )));

    assert!(abandon_quest(session.app.world_mut(), second));
    assert!(
        quest_log_snapshots(session.app.world(), mir2_game_data::LanguageCode::English)
            .iter()
            .any(|snapshot| snapshot.quest_id == second)
    );
    assert!(!accept(&mut session, third).is_empty());
    assert!(!can_accept_quest(session.app.world(), second));
    assert!(
        quest_log_snapshots(session.app.world(), mir2_game_data::LanguageCode::English)
            .iter()
            .all(|snapshot| snapshot.quest_id != second)
    );
    assert!(quest_dialog_links_for_npc(
        session.app.world(),
        newcomer_progression::NEWCOMER_BOARD_OBJECT_ID,
        &[],
    )
    .iter()
    .all(|link| link.target != format!("@quest:accept:{second}")));
    assert!(accept(&mut session, second).iter().any(|packet| matches!(
        packet,
        ServerPacket::Chat {
            chat_type: mir2_protocol::ChatType::System,
            ..
        }
    )));
    for _ in 0..10 {
        advance_crystal_quest_kill(session.app.world_mut(), "OmaFighter");
    }
    assert_eq!(
        quest_stage(session.app.world(), first),
        Some(QuestStage::ReadyToTurnIn)
    );
    let before_gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
    assert!(finish(&mut session, first)
        .iter()
        .any(|packet| matches!(packet, ServerPacket::CompleteQuest { .. })));
    assert_eq!(
        session.app.world().resource::<PlayerRuntimeResource>().gold,
        before_gold + 3_000
    );
    let after_first = session.app.world().resource::<PlayerRuntimeResource>().gold;
    assert!(finish(&mut session, first).is_empty());
    assert_eq!(
        session.app.world().resource::<PlayerRuntimeResource>().gold,
        after_first
    );
}

#[test]
fn bonus_counts_two_distinct_current_period_claims_and_regresses_across_reset() {
    let mut session = authenticated_session(15);
    let bonus = newcomer_progression::DAILY_BONUS_ID;
    assert!(!accept(&mut session, bonus).is_empty());
    assert_eq!(quest_progress(session.app.world(), bonus), Some((0, 2)));

    let period = quest_recurrence::daily_period_at(20 * DAY_MS);
    {
        let mut resource = session.app.world_mut().resource_mut::<QuestResource>();
        for quest_id in newcomer_progression::DAILY_OPTION_IDS[..2].iter().copied() {
            let info = newcomer_progression::quest_info_by_id_unchecked(quest_id).unwrap();
            let mut state = QuestState::from_crystal_info(&info, QuestStage::Completed);
            state.current = state.required;
            state.cadence_last_claimed_period = Some(period);
            state.cadence_high_watermark_period = Some(period);
            resource.quests.push(state);
        }
    }
    assert!(newcomer_progression::sync_daily_bonus(
        session.app.world_mut()
    ));
    assert_eq!(
        quest_stage(session.app.world(), bonus),
        Some(QuestStage::ReadyToTurnIn)
    );
    assert_eq!(quest_progress(session.app.world(), bonus), Some((2, 2)));

    assert!(quest_recurrence::refresh_states_for_test(
        &mut session
            .app
            .world_mut()
            .resource_mut::<QuestResource>()
            .quests,
        true,
        21 * DAY_MS,
    ));
    assert_eq!(
        quest_stage(session.app.world(), bonus),
        Some(QuestStage::InProgress)
    );
    assert_eq!(quest_progress(session.app.world(), bonus), Some((0, 2)));
}

#[test]
fn bonus_requires_distinct_options_and_accepts_each_two_of_three_combination() {
    let period = quest_recurrence::daily_period_at(30 * DAY_MS);
    for (left, right) in [(0, 1), (0, 2), (1, 2)] {
        let bonus_info =
            newcomer_progression::quest_info_by_id_unchecked(newcomer_progression::DAILY_BONUS_ID)
                .unwrap();
        let mut states = vec![QuestState::from_crystal_info(
            &bonus_info,
            QuestStage::InProgress,
        )];
        for option_index in [left, right] {
            let info = newcomer_progression::quest_info_by_id_unchecked(
                newcomer_progression::DAILY_OPTION_IDS[option_index],
            )
            .unwrap();
            let mut option = QuestState::from_crystal_info(&info, QuestStage::Completed);
            option.cadence_last_claimed_period = Some(period);
            option.cadence_high_watermark_period = Some(period);
            states.push(option);
        }
        assert!(newcomer_progression::sync_daily_bonus_states(
            &mut states,
            true
        ));
        assert_eq!(
            (states[0].stage, states[0].current),
            (QuestStage::ReadyToTurnIn, 2)
        );
    }

    let bonus_info =
        newcomer_progression::quest_info_by_id_unchecked(newcomer_progression::DAILY_BONUS_ID)
            .unwrap();
    let option_info =
        newcomer_progression::quest_info_by_id_unchecked(newcomer_progression::DAILY_OPTION_IDS[0])
            .unwrap();
    let mut duplicate = QuestState::from_crystal_info(&option_info, QuestStage::Completed);
    duplicate.cadence_last_claimed_period = Some(period);
    duplicate.cadence_high_watermark_period = Some(period);
    let mut states = vec![
        QuestState::from_crystal_info(&bonus_info, QuestStage::InProgress),
        duplicate.clone(),
        duplicate,
    ];
    assert!(newcomer_progression::sync_daily_bonus_states(
        &mut states,
        true
    ));
    assert_eq!(
        (states[0].stage, states[0].current),
        (QuestStage::InProgress, 1)
    );
}

#[test]
fn two_daily_finishes_unlock_and_pay_bonus_once_then_next_day_reopens_it() {
    let mut session = authenticated_session(15);
    let [first, second, _] = newcomer_progression::DAILY_OPTION_IDS;
    let bonus = newcomer_progression::DAILY_BONUS_ID;
    assert!(!accept(&mut session, bonus).is_empty());
    assert!(!accept(&mut session, first).is_empty());
    assert!(!accept(&mut session, second).is_empty());
    for _ in 0..10 {
        advance_crystal_quest_kill(session.app.world_mut(), "OmaFighter");
    }
    for _ in 0..12 {
        advance_crystal_quest_kill(session.app.world_mut(), "Skeleton");
    }
    assert!(!finish(&mut session, first).is_empty());
    assert!(!finish(&mut session, second).is_empty());
    assert_eq!(
        quest_stage(session.app.world(), bonus),
        Some(QuestStage::ReadyToTurnIn)
    );
    assert_eq!(quest_progress(session.app.world(), bonus), Some((2, 2)));

    let before_bonus = session.app.world().resource::<PlayerRuntimeResource>().gold;
    assert!(finish(&mut session, bonus)
        .iter()
        .any(|packet| matches!(packet, ServerPacket::CompleteQuest { .. })));
    assert_eq!(
        session.app.world().resource::<PlayerRuntimeResource>().gold,
        before_bonus + 5_000
    );
    let after_bonus = session.app.world().resource::<PlayerRuntimeResource>().gold;
    assert!(finish(&mut session, bonus).is_empty());
    assert_eq!(
        session.app.world().resource::<PlayerRuntimeResource>().gold,
        after_bonus
    );

    let tomorrow = quest_recurrence::server_now_millis().saturating_add(DAY_MS);
    assert!(quest_recurrence::refresh_states_for_test(
        &mut session
            .app
            .world_mut()
            .resource_mut::<QuestResource>()
            .quests,
        true,
        tomorrow,
    ));
    assert_eq!(
        quest_stage(session.app.world(), bonus),
        Some(QuestStage::Available)
    );
    assert!(!accept(&mut session, bonus).is_empty());
    assert_eq!(
        quest_stage(session.app.world(), bonus),
        Some(QuestStage::InProgress)
    );
    assert_eq!(quest_progress(session.app.world(), bonus), Some((0, 2)));
}

#[test]
fn milestone_finish_rechecks_level_and_keeps_the_gold_reward_permanent() {
    let mut session = authenticated_session(15);
    let quest_id = newcomer_progression::MILESTONE_IDS[1];
    let info = effective_crystal_quest_info_by_id(session.app.world(), quest_id).unwrap();
    assert_eq!(
        (info.min_level_needed, info.reward_gold, info.reward_exp),
        (20, 10_000, 0)
    );

    ensure_runtime_quest(session.app.world_mut(), quest_id);
    set_quest_stage(session.app.world_mut(), quest_id, QuestStage::ReadyToTurnIn);
    let before_gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
    let _ = finish(&mut session, quest_id);
    assert_eq!(
        quest_stage(session.app.world(), quest_id),
        Some(QuestStage::ReadyToTurnIn)
    );
    assert_eq!(
        session.app.world().resource::<PlayerRuntimeResource>().gold,
        before_gold
    );

    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .selected_character
        .as_mut()
        .unwrap()
        .level = 20;
    assert!(finish(&mut session, quest_id).iter().any(|packet| matches!(
        packet,
        ServerPacket::CompleteQuest { completed_quests } if completed_quests.contains(&quest_id)
    )));
    assert_eq!(
        session.app.world().resource::<PlayerRuntimeResource>().gold,
        before_gold + 10_000
    );
    assert!(!can_accept_quest(session.app.world(), quest_id));
}

#[test]
fn milestone_rewards_are_server_resolved_for_every_class_and_gender_and_pay_once() {
    struct ExpectedReward {
        name: &'static str,
        count: u16,
        index: i32,
        required_level: u8,
        required_class: u8,
        required_gender: u8,
    }

    let cases = [
        (
            MirClass::Warrior,
            [
                vec![
                    ExpectedReward {
                        name: "SharpSword",
                        count: 1,
                        index: 1196,
                        required_level: 15,
                        required_class: 7,
                        required_gender: 3,
                    },
                    ExpectedReward {
                        name: "Slaying",
                        count: 1,
                        index: 974,
                        required_level: 15,
                        required_class: 1,
                        required_gender: 3,
                    },
                ],
                vec![ExpectedReward {
                    name: "MartialSabre",
                    count: 1,
                    index: 1216,
                    required_level: 20,
                    required_class: 7,
                    required_gender: 3,
                }],
                vec![
                    ExpectedReward {
                        name: "ThickArmour(M)",
                        count: 1,
                        index: 1221,
                        required_level: 24,
                        required_class: 7,
                        required_gender: 1,
                    },
                    ExpectedReward {
                        name: "SolidGreatAxe",
                        count: 1,
                        index: 1240,
                        required_level: 26,
                        required_class: 7,
                        required_gender: 3,
                    },
                ],
            ],
        ),
        (
            MirClass::Wizard,
            [
                vec![
                    ExpectedReward {
                        name: "SharpTrident",
                        count: 1,
                        index: 1197,
                        required_level: 15,
                        required_class: 7,
                        required_gender: 3,
                    },
                    ExpectedReward {
                        name: "GreatFireBall",
                        count: 1,
                        index: 993,
                        required_level: 15,
                        required_class: 2,
                        required_gender: 3,
                    },
                ],
                vec![ExpectedReward {
                    name: "SpearWithHook",
                    count: 1,
                    index: 1217,
                    required_level: 20,
                    required_class: 7,
                    required_gender: 3,
                }],
                vec![
                    ExpectedReward {
                        name: "FireMagicRobe(M)",
                        count: 1,
                        index: 1223,
                        required_level: 24,
                        required_class: 7,
                        required_gender: 1,
                    },
                    ExpectedReward {
                        name: "SolidBronzeStaff",
                        count: 1,
                        index: 1241,
                        required_level: 26,
                        required_class: 7,
                        required_gender: 3,
                    },
                ],
            ],
        ),
        (
            MirClass::Taoist,
            [
                vec![
                    ExpectedReward {
                        name: "SharpScimitar",
                        count: 1,
                        index: 1198,
                        required_level: 15,
                        required_class: 7,
                        required_gender: 3,
                    },
                    ExpectedReward {
                        name: "SoulFireBall",
                        count: 1,
                        index: 1019,
                        required_level: 18,
                        required_class: 4,
                        required_gender: 3,
                    },
                    ExpectedReward {
                        name: "Amulet",
                        count: 100,
                        index: 712,
                        required_level: 18,
                        required_class: 31,
                        required_gender: 3,
                    },
                ],
                vec![ExpectedReward {
                    name: "KeenKrissSword",
                    count: 1,
                    index: 1218,
                    required_level: 20,
                    required_class: 7,
                    required_gender: 3,
                }],
                vec![
                    ExpectedReward {
                        name: "TaoArmour(M)",
                        count: 1,
                        index: 1225,
                        required_level: 24,
                        required_class: 7,
                        required_gender: 1,
                    },
                    ExpectedReward {
                        name: "SolidSerpentSword",
                        count: 1,
                        index: 1242,
                        required_level: 26,
                        required_class: 7,
                        required_gender: 3,
                    },
                ],
            ],
        ),
    ];

    for (class, male_rewards) in cases {
        for gender in [MirGender::Male, MirGender::Female] {
            let mut session = authenticated_character(25, class, gender);
            let expected_gold = [5_000, 10_000, 15_000];
            for (milestone_index, male_expected) in male_rewards.iter().enumerate() {
                let quest_id = newcomer_progression::MILESTONE_IDS[milestone_index];
                let info = effective_crystal_quest_info_by_id(session.app.world(), quest_id)
                    .expect("effective milestone info");
                let expected = male_expected
                    .iter()
                    .map(|reward| {
                        let female_name = match reward.name {
                            "ThickArmour(M)" => "ThickArmour(F)",
                            "FireMagicRobe(M)" => "FireMagicRobe(F)",
                            "TaoArmour(M)" => "TaoArmour(F)",
                            name => name,
                        };
                        let female_index = match reward.index {
                            1221 => 1222,
                            1223 => 1224,
                            1225 => 1226,
                            index => index,
                        };
                        let female_gender = if reward.required_gender == 1 {
                            2
                        } else {
                            reward.required_gender
                        };
                        if gender == MirGender::Female {
                            (female_name, female_index, female_gender, reward)
                        } else {
                            (reward.name, reward.index, reward.required_gender, reward)
                        }
                    })
                    .collect::<Vec<_>>();
                assert_eq!(info.rewards_fixed_item.len(), expected.len());
                for (actual, (name, index, required_gender, source)) in
                    info.rewards_fixed_item.iter().zip(&expected)
                {
                    assert_eq!(actual.item.name, *name);
                    assert_eq!(actual.count, source.count);
                    assert_eq!(actual.item.index, *index);
                    assert_eq!(actual.item.required_amount, source.required_level);
                    assert_eq!(actual.item.required_class, source.required_class);
                    assert_eq!(actual.item.required_gender, *required_gender);
                }

                let before = expected
                    .iter()
                    .map(|(name, _, _, _)| (*name, carried_quantity(session.app.world(), name)))
                    .collect::<Vec<_>>();
                let before_gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
                ensure_runtime_quest(session.app.world_mut(), quest_id);
                set_quest_stage(session.app.world_mut(), quest_id, QuestStage::ReadyToTurnIn);
                assert!(finish(&mut session, quest_id).iter().any(|packet| matches!(
                    packet,
                    ServerPacket::CompleteQuest { completed_quests } if completed_quests.contains(&quest_id)
                )));
                assert_eq!(
                    session.app.world().resource::<PlayerRuntimeResource>().gold,
                    before_gold + expected_gold[milestone_index]
                );
                for ((name, _, _, source), (_, quantity)) in expected.iter().zip(&before) {
                    assert_eq!(
                        carried_quantity(session.app.world(), name),
                        quantity + u32::from(source.count)
                    );
                }

                let quantities_after = expected
                    .iter()
                    .map(|(name, _, _, _)| carried_quantity(session.app.world(), name))
                    .collect::<Vec<_>>();
                let gold_after = session.app.world().resource::<PlayerRuntimeResource>().gold;
                assert!(finish(&mut session, quest_id).is_empty());
                assert_eq!(
                    session.app.world().resource::<PlayerRuntimeResource>().gold,
                    gold_after
                );
                assert_eq!(
                    expected
                        .iter()
                        .map(|(name, _, _, _)| carried_quantity(session.app.world(), name))
                        .collect::<Vec<_>>(),
                    quantities_after
                );
                if class == MirClass::Taoist && milestone_index == 0 {
                    assert_amulet_reward_can_be_equipped_through_packets(&mut session);
                }
            }
        }
    }
}

#[test]
fn milestone_reward_finish_is_atomic_when_the_bag_is_full() {
    let mut session = authenticated_character(15, MirClass::Warrior, MirGender::Male);
    let quest_id = newcomer_progression::MILESTONE_IDS[0];
    ensure_runtime_quest(session.app.world_mut(), quest_id);
    set_quest_stage(session.app.world_mut(), quest_id, QuestStage::ReadyToTurnIn);
    fill_bag(session.app.world_mut());

    let before_gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
    let before_sword = carried_quantity(session.app.world(), "SharpSword");
    let before_book = carried_quantity(session.app.world(), "Slaying");
    assert!(finish(&mut session, quest_id).iter().any(|packet| matches!(
        packet,
        ServerPacket::Chat {
            chat_type: mir2_protocol::ChatType::System,
            ..
        }
    )));
    assert_eq!(
        quest_stage(session.app.world(), quest_id),
        Some(QuestStage::ReadyToTurnIn)
    );
    assert_eq!(
        session.app.world().resource::<PlayerRuntimeResource>().gold,
        before_gold
    );
    assert_eq!(
        carried_quantity(session.app.world(), "SharpSword"),
        before_sword
    );
    assert_eq!(
        carried_quantity(session.app.world(), "Slaying"),
        before_book
    );
}

#[test]
fn later_milestones_keep_their_existing_gold_only_contract() {
    let session = authenticated_character(40, MirClass::Wizard, MirGender::Female);
    for (index, gold) in [(3, 25_000), (4, 40_000), (5, 60_000)] {
        let info = effective_crystal_quest_info_by_id(
            session.app.world(),
            newcomer_progression::MILESTONE_IDS[index],
        )
        .expect("later milestone info");
        assert_eq!(info.reward_gold, gold);
        assert_eq!(info.reward_exp, 0);
        assert!(info.rewards_fixed_item.is_empty());
        assert!(info.rewards_select_item.is_empty());
    }
}

fn carried_quantity(world: &bevy_ecs::prelude::World, name: &str) -> u32 {
    let inventory = world.resource::<InventoryResource>();
    inventory
        .inventory_items
        .iter()
        .chain(inventory.belt_items.iter())
        .filter(|item| {
            matches!(
                item.container,
                crate::config::ItemContainer::Bag1
                    | crate::config::ItemContainer::Bag2
                    | crate::config::ItemContainer::Belt
            ) && item.name.eq_ignore_ascii_case(name)
        })
        .map(|item| item.quantity)
        .sum()
}

fn assert_amulet_reward_can_be_equipped_through_packets(session: &mut SimulationSession) {
    let (belt_slot, unique_id) = session
        .world_snapshot()
        .belt_items
        .iter()
        .find(|item| item.name == "Amulet" && item.quantity == 100)
        .map(|item| (item.slot, item.unique_id))
        .expect("the full Amulet reward should be visible in the normal belt snapshot");
    let empty_bag_index = {
        let inventory = session.app.world().resource::<InventoryResource>();
        (0..crate::config::crystal_bag_slot_capacity(inventory.inventory_capacity))
            .find(|index| {
                let index = u8::try_from(*index).expect("bag capacity fits in u8");
                !inventory.inventory_items.iter().any(|item| {
                    super::super::inventory::inventory_index_for_item(item) == Some(index)
                })
            })
            .expect("an empty bag slot for the Amulet reward")
    };
    let raw_bag_slot =
        i32::from(crate::config::CRYSTAL_BELT_SLOT_COUNT) + i32::from(empty_bag_index);
    assert!(session
        .handle_packet(ClientPacket::MoveItem {
            grid: MirGridType::Belt,
            from: i32::from(belt_slot),
            to: raw_bag_slot,
        })
        .contains(&ServerPacket::MoveItem {
            grid: MirGridType::Belt,
            from: i32::from(belt_slot),
            to: raw_bag_slot,
            success: true,
        }));
    assert!(session
        .handle_packet(ClientPacket::EquipItem {
            grid: MirGridType::Inventory,
            unique_id,
            to: 9,
        })
        .contains(&ServerPacket::EquipItem {
            grid: MirGridType::Inventory,
            unique_id,
            to: 9,
            success: true,
        }));
    assert!(session.world_snapshot().equipment_items.iter().any(|item| {
        item.name == "Amulet"
            && item.quantity == 100
            && item.slot == crate::config::EquipmentSlot::Amulet
    }));
}

fn fill_bag(world: &mut bevy_ecs::prelude::World) {
    let mut resource = world.resource_mut::<InventoryResource>();
    let capacity = usize::from(crate::config::crystal_bag_slot_capacity(
        resource.inventory_capacity,
    ));
    let mut filler = resource
        .inventory_items
        .first()
        .cloned()
        .expect("starter inventory item");
    filler.container = crate::config::ItemContainer::Bag1;
    let mut next = resource
        .inventory_items
        .iter()
        .filter(|item| {
            matches!(
                item.container,
                crate::config::ItemContainer::Bag1 | crate::config::ItemContainer::Bag2
            )
        })
        .count();
    while next < capacity {
        let mut item = filler.clone();
        item.key = format!("milestone-full-bag-{next}");
        item.name = format!("Milestone full bag {next}");
        item.unique_id = 9_000_000 + u64::try_from(next).unwrap();
        item.slot = u8::try_from(next).unwrap();
        resource.inventory_items.push(item);
        next += 1;
    }
}

#[test]
fn disabled_profile_hides_and_freezes_saved_newcomer_rows() {
    let mut session = authenticated_session(40);
    let option = newcomer_progression::DAILY_OPTION_IDS[0];
    ensure_runtime_quest(session.app.world_mut(), option);
    set_quest_stage(session.app.world_mut(), option, QuestStage::InProgress);
    let milestone = newcomer_progression::MILESTONE_IDS[0];
    ensure_runtime_quest(session.app.world_mut(), milestone);
    set_quest_stage(
        session.app.world_mut(),
        milestone,
        QuestStage::ReadyToTurnIn,
    );
    session
        .app
        .world_mut()
        .resource_mut::<QuestResource>()
        .newcomer_v1_cadence = false;

    assert!(advance_crystal_quest_kill(session.app.world_mut(), "OmaFighter").is_empty());
    assert_eq!(quest_progress(session.app.world(), option).unwrap().0, 0);
    assert!(
        quest_log_snapshots(session.app.world(), mir2_game_data::LanguageCode::English)
            .iter()
            .all(|snapshot| !newcomer_progression::is_newcomer_quest(snapshot.quest_id))
    );
    let before_gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
    let _ = finish(&mut session, milestone);
    assert_eq!(
        session.app.world().resource::<PlayerRuntimeResource>().gold,
        before_gold
    );
    assert_eq!(
        quest_stage(session.app.world(), milestone),
        Some(QuestStage::ReadyToTurnIn)
    );
}
