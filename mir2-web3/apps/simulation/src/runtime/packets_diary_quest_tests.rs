use bevy_ecs::prelude::World;
use mir2_protocol::{ClientPacket, ServerPacket};

use super::{
    active_npc_allows_quest_request, newcomer_diary_template_allows_quest_request,
};
use crate::config::{ItemContainer, QuestStage, SimulationConfig};
use crate::SimulationSession;

use super::super::inventory::add_or_increment_item;
use super::super::quests::{
    advance_crystal_quest_item_task, ensure_runtime_quest, quest_stage,
};
use super::super::resources::{
    InventoryResource, NpcStateResource, PlayerRuntimeResource, QuestResource,
    RuntimeConfigResource, SessionResource,
};

fn quest_guard_world(newcomer_v1: bool) -> World {
    let config = SimulationConfig::default();
    let mut world = World::new();
    world.insert_resource(RuntimeConfigResource::new(&config));
    world.insert_resource(NpcStateResource::new());
    let mut quests = QuestResource::new();
    quests.newcomer_v1_cadence = newcomer_v1;
    world.insert_resource(quests);
    world
}

fn authenticated_session(level: u16) -> SimulationSession {
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
        .newcomer_v1_cadence = true;
    session
}

fn accept_diary_quest(session: &mut SimulationSession, quest_id: i32) -> Vec<ServerPacket> {
    session.handle_packet(ClientPacket::AcceptQuest {
        npc_index: 0,
        quest_index: quest_id,
    })
}

fn finish_diary_quest(
    session: &mut SimulationSession,
    quest_id: i32,
    selected_item_index: i32,
) -> Vec<ServerPacket> {
    session.handle_packet(ClientPacket::FinishQuest {
        quest_index: quest_id,
        selected_item_index,
    })
}

fn has_taken_packet(packets: &[ServerPacket], quest_id: i32) -> bool {
    packets.iter().any(|packet| {
        matches!(packet, ServerPacket::ChangeQuest { quest_id: id, taken: true, .. } if *id == quest_id)
    })
}

fn has_remove_packet(packets: &[ServerPacket], quest_id: i32) -> bool {
    packets.iter().any(|packet| {
        matches!(packet, ServerPacket::ChangeQuest { quest_id: id, taken: false, quest_state: 2, .. } if *id == quest_id)
    })
}

fn seed_completed_prerequisite_for_test(session: &mut SimulationSession, quest_id: i32) {
    let world = session.app.world_mut();
    ensure_runtime_quest(world, quest_id);
    world
        .resource_mut::<QuestResource>()
        .quests
        .iter_mut()
        .find(|quest| quest.quest_id == quest_id)
        .expect("seeded prerequisite")
        .stage = QuestStage::Completed;
}

fn inventory_quantity(session: &SimulationSession, key: &str) -> u32 {
    session
        .app
        .world()
        .resource::<InventoryResource>()
        .inventory_items
        .iter()
        .filter(|item| item.key == key)
        .map(|item| item.quantity)
        .sum()
}

#[test]
fn newcomer_diary_allows_only_the_zero_npc_side_of_real_templates() {
    let world = quest_guard_world(true);

    // Quest 22 (Forest Yeti's Threat) starts and finishes through the Diary.
    assert!(active_npc_allows_quest_request(
        &world,
        22,
        Some(0),
        false,
        None,
    ));
    assert!(active_npc_allows_quest_request(
        &world, 22, None, true, None,
    ));

    // Quest 26 starts through the Diary but must finish at its configured NPC.
    assert!(active_npc_allows_quest_request(
        &world,
        26,
        Some(0),
        false,
        None,
    ));
    assert!(!active_npc_allows_quest_request(
        &world, 26, None, true, None,
    ));
    assert!(!active_npc_allows_quest_request(
        &world,
        22,
        Some(3),
        false,
        None,
    ));
    assert!(!active_npc_allows_quest_request(
        &world,
        1,
        Some(0),
        false,
        None,
    ));

    // No current Crystal template has this shape. Keep the fallback-to-start
    // finish semantics safe if one is imported later.
    assert!(!newcomer_diary_template_allows_quest_request(
        3, 0, None, true,
    ));
}

#[test]
fn crystal_profile_still_requires_an_active_npc_dialog() {
    let world = quest_guard_world(false);

    assert!(!active_npc_allows_quest_request(
        &world,
        22,
        Some(0),
        false,
        None,
    ));
    assert!(!active_npc_allows_quest_request(
        &world, 22, None, true, None,
    ));
}

#[test]
fn newcomer_nonzero_npc_templates_cannot_bypass_dialog_authorization_with_zero() {
    let world = quest_guard_world(true);

    assert!(!active_npc_allows_quest_request(
        &world,
        1,
        Some(0),
        false,
        None,
    ));
    assert!(!active_npc_allows_quest_request(
        &world, 1, None, true, None,
    ));
}

#[test]
fn fresh_diary_accept_packets_still_enforce_level_and_prerequisite() {
    let mut below_level = authenticated_session(5);
    let rejected = accept_diary_quest(&mut below_level, 22);
    assert!(!has_taken_packet(&rejected, 22));
    assert_eq!(quest_stage(below_level.app.world(), 22), None);

    below_level
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .selected_character
        .as_mut()
        .expect("selected character")
        .level = 6;
    assert!(has_taken_packet(
        &accept_diary_quest(&mut below_level, 22),
        22,
    ));

    let mut missing_prerequisite = authenticated_session(7);
    let rejected = accept_diary_quest(&mut missing_prerequisite, 23);
    assert!(!has_taken_packet(&rejected, 23));
    assert_eq!(quest_stage(missing_prerequisite.app.world(), 23), None);

    // Unit fixture only: this seeds history to isolate packet authorization
    // from the much longer live q22 combat journey.
    seed_completed_prerequisite_for_test(&mut missing_prerequisite, 22);
    assert!(has_taken_packet(
        &accept_diary_quest(&mut missing_prerequisite, 23),
        23,
    ));
}

#[test]
fn diary_finish_packets_still_enforce_items_selection_and_idempotent_rewards() {
    let mut session = authenticated_session(7);
    // Unit fixture only: live completion of q22 is covered by the player journey.
    seed_completed_prerequisite_for_test(&mut session, 22);
    assert!(has_taken_packet(&accept_diary_quest(&mut session, 23), 23));

    let premature = finish_diary_quest(&mut session, 23, -1);
    assert!(!has_remove_packet(&premature, 23));
    assert_eq!(
        quest_stage(session.app.world(), 23),
        Some(QuestStage::InProgress)
    );

    // Unit fixture only: reproduce the two quest drops and their authoritative
    // objective progress without spawning or killing live monsters.
    add_or_increment_item(
        session.app.world_mut(),
        ItemContainer::Quest,
        "crystal-item-1113",
        "OmaTeeth",
        "Quest objective fixture.",
        0,
        2,
        1,
    );
    assert!(advance_crystal_quest_item_task(
        session.app.world_mut(),
        23,
        "item:1113",
        2,
    ));
    assert_eq!(
        quest_stage(session.app.world(), 23),
        Some(QuestStage::ReadyToTurnIn)
    );

    let gold_before = session.app.world().resource::<PlayerRuntimeResource>().gold;
    let missing_selection = finish_diary_quest(&mut session, 23, -1);
    assert!(!has_remove_packet(&missing_selection, 23));
    assert_eq!(
        quest_stage(session.app.world(), 23),
        Some(QuestStage::ReadyToTurnIn)
    );
    assert_eq!(inventory_quantity(&session, "crystal-item-1113"), 2);
    assert_eq!(
        session.app.world().resource::<PlayerRuntimeResource>().gold,
        gold_before
    );

    let completed = finish_diary_quest(&mut session, 23, 0);
    assert!(has_remove_packet(&completed, 23));
    assert_eq!(
        quest_stage(session.app.world(), 23),
        Some(QuestStage::Completed)
    );
    assert_eq!(inventory_quantity(&session, "crystal-item-1113"), 0);
    assert_eq!(inventory_quantity(&session, "crystal-item-1182"), 1);
    let gold_after = session.app.world().resource::<PlayerRuntimeResource>().gold;
    assert_eq!(gold_after, gold_before + 83);

    let duplicate = finish_diary_quest(&mut session, 23, 0);
    assert!(!has_remove_packet(&duplicate, 23));
    assert_eq!(inventory_quantity(&session, "crystal-item-1182"), 1);
    assert_eq!(
        session.app.world().resource::<PlayerRuntimeResource>().gold,
        gold_after
    );
}
