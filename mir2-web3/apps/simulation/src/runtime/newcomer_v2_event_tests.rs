use super::super::super::resources::SessionResource;
use super::*;
use crate::{SimulationConfig, SimulationSession};
use mir2_protocol::{ClientPacket, MirClass, ServerPacket};

fn session(class: MirClass) -> SimulationSession {
    session_with_config(class, SimulationConfig::default())
}

fn session_with_config(class: MirClass, config: SimulationConfig) -> SimulationSession {
    let mut session = SimulationSession::new(config);
    let packets = session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    let index = packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::LoginSuccess { characters } => {
                characters.first().map(|character| character.index)
            }
            _ => None,
        })
        .unwrap();
    session.handle_packet(ClientPacket::StartGame {
        character_index: index,
    });
    let world = session.app.world_mut();
    world.resource_mut::<QuestResource>().quests.clear();
    world.resource_mut::<QuestResource>().newcomer_v1_cadence = false;
    world.resource_mut::<QuestResource>().newcomer_v2_cadence = true;
    world
        .resource_mut::<SessionResource>()
        .selected_character
        .as_mut()
        .unwrap()
        .class = class;
    session
}

fn begin(session: &mut SimulationSession, id: i32) {
    super::super::begin_quest(session.app.world_mut(), id);
}

fn acceptance(world: &World, id: i32) -> u64 {
    world
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|quest| quest.quest_id == id)
        .unwrap()
        .task_progress
        .keys()
        .find_map(|key| {
            key.strip_prefix(ACCEPTED_AT)
                .and_then(|value| value.parse::<u64>().ok())
        })
        .unwrap()
}

fn learn(world: &mut World, spell: &str) {
    world
        .resource_mut::<SkillResource>()
        .skills
        .push(super::super::super::skills::crystal_skill_state(spell, 0).unwrap());
}

#[test]
fn newcomer_v2_training_requires_post_acceptance_damage_and_the_correct_map() {
    let mut session = session(MirClass::Wizard);
    let world = session.app.world_mut();
    learn(world, "FireBall");
    begin(&mut session, 2_110_004);
    let world = session.app.world_mut();
    let stamp = acceptance(world, 2_110_004);
    let flag = newcomer_v2::flag_objectives(world, 2_110_004)[0].number;
    assert!(super::super::advance_crystal_quest_flag(world, flag).is_empty());
    assert_eq!(
        super::super::quest_stage(world, 2_110_004),
        Some(QuestStage::InProgress)
    );
    assert!(record_spell_damage(world, "FireBall", stamp.saturating_sub(1)).is_empty());
    world
        .resource_mut::<MapRuntimeResource>()
        .current_map
        .file_name = "D401".into();
    assert!(record_spell_damage(world, "FireBall", stamp + 1).is_empty());
    assert!(!world
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|quest| quest.quest_id == 2_110_004)
        .unwrap()
        .task_progress
        .keys()
        .any(|key| key.starts_with("v2:damage:")));
    world
        .resource_mut::<MapRuntimeResource>()
        .current_map
        .file_name = "0".into();
    assert!(!record_spell_damage(world, "FireBall", stamp + 2).is_empty());
    let quest = world
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|quest| quest.quest_id == 2_110_004)
        .unwrap();
    assert_eq!(
        quest.task_progress.get(&crystal_flag_task_key(flag)),
        Some(&1)
    );
    assert_eq!(quest.stage, QuestStage::InProgress); // The two Oma kills are independent.
    super::super::advance_crystal_quest_kill(world, "OmaFighter");
    super::super::advance_crystal_quest_kill(world, "Oma");
    assert_eq!(
        super::super::quest_stage(world, 2_110_004),
        Some(QuestStage::InProgress)
    );
    super::super::advance_crystal_quest_kill(world, "Oma");
    assert_eq!(
        super::super::quest_stage(world, 2_110_004),
        Some(QuestStage::ReadyToTurnIn)
    );
}

#[test]
fn newcomer_v2_kill_and_reposition_evidence_cannot_cross_regions() {
    let mut session = session(MirClass::Wizard);
    begin(&mut session, 2_110_010);
    let world = session.app.world_mut();
    assert!(super::super::advance_crystal_quest_kill(world, "Skeleton").is_empty());
    world
        .resource_mut::<MapRuntimeResource>()
        .current_map
        .file_name = "D001".into();
    assert!(!super::super::advance_crystal_quest_kill(world, "Skeleton").is_empty());
    let quest = world
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|quest| quest.quest_id == 2_110_010)
        .unwrap();
    assert_eq!(quest.current, 1);
    assert!(record_legal_reposition(world).is_empty());
}

#[test]
fn newcomer_v2_between_attacks_requires_damage_move_damage_in_order() {
    let mut session = session(MirClass::Wizard);
    learn(session.app.world_mut(), "Lightning");
    learn(session.app.world_mut(), "FireWall");
    session
        .app
        .world_mut()
        .resource_mut::<MapRuntimeResource>()
        .current_map
        .file_name = "D022".into();
    begin(&mut session, 2_110_021);
    let world = session.app.world_mut();
    let stamp = acceptance(world, 2_110_021);
    record_legal_reposition(world);
    record_spell_damage(world, "Lightning", stamp + 1);
    record_spell_damage(world, "FireWall", stamp + 2);
    let flag = newcomer_v2::flag_objectives(world, 2_110_021)[0].number;
    assert_ne!(
        world
            .resource::<QuestResource>()
            .quests
            .iter()
            .find(|quest| quest.quest_id == 2_110_021)
            .unwrap()
            .task_progress
            .get(&crystal_flag_task_key(flag)),
        Some(&1)
    );
    record_legal_reposition(world);
    assert_ne!(
        world
            .resource::<QuestResource>()
            .quests
            .iter()
            .find(|quest| quest.quest_id == 2_110_021)
            .unwrap()
            .task_progress
            .get(&crystal_flag_task_key(flag)),
        Some(&1)
    );
    record_spell_damage(world, "Lightning", stamp + 3);
    assert_eq!(
        world
            .resource::<QuestResource>()
            .quests
            .iter()
            .find(|quest| quest.quest_id == 2_110_021)
            .unwrap()
            .task_progress
            .get(&crystal_flag_task_key(flag)),
        Some(&1)
    );
}

#[test]
fn newcomer_v2_initial_report_requires_a_successful_real_acceptance_result() {
    let mut session = session(MirClass::Warrior);
    begin(&mut session, 2_110_001);
    let world = session.app.world_mut();
    assert!(observe_committed_command(world, CommandContext::Accept(2_110_001), &[]).is_empty());
    assert_eq!(
        super::super::quest_stage(world, 2_110_001),
        Some(QuestStage::InProgress)
    );
    let packet = super::super::crystal_quest_update_packet(world, 2_110_001).unwrap();
    assert!(!observe_committed_command(world, CommandContext::Npc(3), &[packet]).is_empty());
    assert_eq!(
        super::super::quest_stage(world, 2_110_001),
        Some(QuestStage::ReadyToTurnIn)
    );
}

#[test]
fn newcomer_v2_jane_public_packet_flow_completes_once_and_awards_the_real_starter_items() {
    let mut session = session_with_config(
        MirClass::Wizard,
        SimulationConfig::default().with_crystal_world_runtime(),
    );
    session.force_authoritative_player_transform(
        Point { x: 284, y: 607 },
        mir2_protocol::MirDirection::Up,
    );
    session.handle_packet(ClientPacket::CallNpc {
        object_id: 3,
        key: "@main".into(),
    });
    let accepted = session.handle_packet(ClientPacket::AcceptQuest {
        npc_index: 3,
        quest_index: 2_110_001,
    });
    assert!(accepted.iter().any(|packet| matches!(
        packet,
        ServerPacket::ChangeQuest {
            quest_id: 2_110_001,
            taken: true,
            completed: true,
            ..
        }
    )));
    session.handle_packet(ClientPacket::CallNpc {
        object_id: 3,
        key: "@main".into(),
    });
    let gold = session.world_snapshot().gold;
    let inventory = session.app.world().resource::<InventoryResource>();
    let before: Vec<_> = inventory
        .inventory_items
        .iter()
        .chain(&inventory.belt_items)
        .cloned()
        .collect();
    let finished = session.handle_packet(ClientPacket::FinishQuest {
        quest_index: 2_110_001,
        selected_item_index: -1,
    });
    assert_eq!(
        super::super::quest_stage(session.app.world(), 2_110_001),
        Some(QuestStage::Completed)
    );
    assert_eq!(session.world_snapshot().gold, gold + 100);
    assert!(finished.iter().any(
        |packet| matches!(packet, ServerPacket::CompleteQuest { completed_quests }
        if completed_quests.contains(&2_110_001))
    ));
    let inventory = session.app.world().resource::<InventoryResource>();
    for (name, count) in [
        ("WoodenSword", 1),
        ("(HP)DrugSmall", 10),
        ("(MP)DrugSmall", 10),
    ] {
        let index = mir2_game_data::crystal_item_by_name(name)
            .unwrap()
            .item_index;
        let matching = |key: &str| {
            super::super::super::items::crystal_item_template_for_item_key(key)
                .is_some_and(|template| template.item_index == index)
        };
        let old: u32 = before
            .iter()
            .filter(|item| matching(&item.key))
            .map(|item| item.quantity)
            .sum();
        let new: u32 = inventory
            .inventory_items
            .iter()
            .chain(&inventory.belt_items)
            .filter(|item| matching(&item.key))
            .map(|item| item.quantity)
            .sum();
        assert_eq!(new, old + count, "reward quantity for {name}");
    }
    let gold = session.world_snapshot().gold;
    session.handle_packet(ClientPacket::FinishQuest {
        quest_index: 2_110_001,
        selected_item_index: -1,
    });
    assert_eq!(session.world_snapshot().gold, gold);
}

#[test]
fn newcomer_v2_growth_capacity_failure_retains_claim_and_currency() {
    let mut session = session(MirClass::Warrior);
    let world = session.app.world_mut();
    world
        .resource_mut::<SessionResource>()
        .selected_character
        .as_mut()
        .unwrap()
        .level = 15;
    let info = super::super::effective_crystal_quest_info_by_id(world, 2_110_007).unwrap();
    world
        .resource_mut::<QuestResource>()
        .quests
        .push(super::super::QuestState::from_crystal_info(
            &info,
            QuestStage::Completed,
        ));
    begin(&mut session, 2_120_015);
    let world = session.app.world_mut();
    world
        .resource_mut::<InventoryResource>()
        .inventory_items
        .clear();
    world.resource_mut::<InventoryResource>().inventory_capacity = 0;
    let before_gold = world.resource::<PlayerRuntimeResource>().gold;
    assert!(!super::super::complete_quest_with_selection(
        world, 2_120_015, None
    ));
    assert_eq!(
        super::super::quest_stage(world, 2_120_015),
        Some(QuestStage::ReadyToTurnIn)
    );
    assert_eq!(world.resource::<PlayerRuntimeResource>().gold, before_gold);
    assert!(world
        .resource::<InventoryResource>()
        .inventory_items
        .is_empty());
}

#[test]
fn newcomer_v2_board_uses_loaded_object_id_and_public_claim_flow() {
    let mut session = session_with_config(
        MirClass::Taoist,
        SimulationConfig::default().with_crystal_world_runtime(),
    );
    let world = session.app.world_mut();
    world
        .resource_mut::<SessionResource>()
        .selected_character
        .as_mut()
        .unwrap()
        .level = 20;
    for id in (2_110_001..=2_110_012).chain([2_120_015]) {
        let info = super::super::effective_crystal_quest_info_by_id(world, id).unwrap();
        world.resource_mut::<QuestResource>().quests.push(
            super::super::QuestState::from_crystal_info(&info, QuestStage::Completed),
        );
    }
    let info = super::super::effective_crystal_quest_info_by_id(world, 2_120_020).unwrap();
    assert!(super::super::crystal_quest_start_npc_matches(&info, 24));
    assert!(super::super::crystal_quest_finish_npc_matches(&info, 24));
    assert!(!super::super::crystal_quest_start_npc_matches(&info, 17));
    session.force_authoritative_player_transform(
        Point { x: 334, y: 260 },
        mir2_protocol::MirDirection::Up,
    );
    session.handle_packet(ClientPacket::CallNpc {
        object_id: 24,
        key: "@main".into(),
    });
    session.handle_packet(ClientPacket::AcceptQuest {
        npc_index: 24,
        quest_index: 2_120_020,
    });
    assert_eq!(
        super::super::quest_stage(session.app.world(), 2_120_020),
        Some(QuestStage::ReadyToTurnIn)
    );
    session.handle_packet(ClientPacket::CallNpc {
        object_id: 24,
        key: "@main".into(),
    });
    session.handle_packet(ClientPacket::FinishQuest {
        quest_index: 2_120_020,
        selected_item_index: -1,
    });
    assert_eq!(
        super::super::quest_stage(session.app.world(), 2_120_020),
        Some(QuestStage::Completed)
    );
    assert!(session
        .app
        .world()
        .resource::<InventoryResource>()
        .inventory_items
        .iter()
        .any(|item| item.name == "KeenKrissSword"));
}

#[test]
fn newcomer_v2_final_report_advances_through_the_public_interact_command() {
    let mut session = session_with_config(
        MirClass::Warrior,
        SimulationConfig::default().with_crystal_world_runtime(),
    );
    let world = session.app.world_mut();
    world
        .resource_mut::<SessionResource>()
        .selected_character
        .as_mut()
        .unwrap()
        .level = 30;
    for id in (2_110_001..=2_110_021).chain([2_120_015, 2_120_020, 2_120_025]) {
        let info = super::super::effective_crystal_quest_info_by_id(world, id).unwrap();
        world.resource_mut::<QuestResource>().quests.push(
            super::super::QuestState::from_crystal_info(&info, QuestStage::Completed),
        );
    }
    session.handle_packet(ClientPacket::AcceptQuest {
        npc_index: 0,
        quest_index: 2_110_022,
    });
    assert_eq!(
        super::super::quest_stage(session.app.world(), 2_110_022),
        Some(QuestStage::InProgress)
    );
    session.force_authoritative_player_transform(
        Point { x: 334, y: 260 },
        mir2_protocol::MirDirection::Up,
    );
    session.interact(3);
    assert_eq!(
        super::super::quest_stage(session.app.world(), 2_110_022),
        Some(QuestStage::InProgress)
    );
    session.interact(24);
    assert_eq!(
        super::super::quest_stage(session.app.world(), 2_110_022),
        Some(QuestStage::ReadyToTurnIn)
    );
}

#[test]
fn newcomer_v2_saved_progress_cannot_switch_back_and_claim_v1_or_legacy_rewards() {
    let mut session = session(MirClass::Warrior);
    let world = session.app.world_mut();
    let info = super::super::effective_crystal_quest_info_by_id(world, 2_110_001).unwrap();
    world
        .resource_mut::<QuestResource>()
        .quests
        .push(super::super::QuestState::from_crystal_info(
            &info,
            QuestStage::Completed,
        ));
    world.resource_mut::<QuestResource>().newcomer_v2_cadence = false;
    world.resource_mut::<QuestResource>().newcomer_v1_cadence = true;
    assert!(!super::super::can_accept_quest(
        world,
        super::super::super::crystal_compat::GUIDE_QUEST_ID
    ));
    assert!(!super::super::can_accept_quest(world, 4));
    assert!(!super::super::can_accept_quest(world, 2_100_015));
    assert!(!super::super::completed_quest_ids(world).contains(&2_110_001));
    assert_eq!(
        super::super::quest_stage(world, 2_110_001),
        Some(QuestStage::Completed)
    );
}
