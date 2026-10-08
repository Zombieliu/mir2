use super::*;
use super::super::item_grants::tests::{trusted_session, inventory_image, plain};
use std::fs;

fn reward_info(session: &mut crate::SimulationSession) -> ClientQuestInfo {
    let mut info = crystal_quest_info_by_id(37).unwrap();
    info.rewards_fixed_item = vec![QuestItemReward {
        item: item_info_from_crystal_template(crystal_item_by_name("GoldOre").unwrap()), count: 2,
    }];
    info.rewards_select_item.clear();
    ensure_runtime_quest(session.app.world_mut(), info.index);
    set_quest_stage(session.app.world_mut(), info.index, QuestStage::ReadyToTurnIn);
    info
}

fn state_image(session: &crate::SimulationSession) -> String {
    let player = session.app.world().resource::<PlayerRuntimeResource>();
    format!("{}|{:?}|{}|{}|{}", inventory_image(session), session.app.world().resource::<QuestResource>().quests,
        player.gold, player.credit, player.experience)
}

#[test]
fn crystal_reward_uid_failure_preserves_quest_items_currency_and_experience() {
    let (mut session, _, path) = trusted_session("quest-fail", 5_000_000);
    let info = reward_info(&mut session);
    let mut task = plain("CopperRing", 1, ItemContainer::Quest).prototype;
    task.unique_id = 300_001;
    session.app.world_mut().resource_mut::<InventoryResource>().inventory_items.push(task);
    let before = state_image(&session);
    fs::write(path, "broken authority").unwrap();
    assert!(!complete_crystal_quest(session.app.world_mut(), &info, None));
    assert_eq!(state_image(&session), before);
}

#[test]
fn crystal_reward_mid_batch_exhaustion_has_no_partial_completion() {
    let (mut session, allocator, _) = trusted_session("quest-exhaust", u64::MAX - 1);
    let info = reward_info(&mut session);
    let before = state_image(&session);
    assert!(!complete_crystal_quest(session.app.world_mut(), &info, None));
    assert_eq!(state_image(&session), before);
    assert_eq!(allocator.issued_through().unwrap(), u64::MAX);
}

#[test]
fn ordinary_finish_quest_issues_rewards_once_and_keeps_saved_stage() {
    let (mut session, allocator, _) = trusted_session("quest-success", 5_000_000);
    // A real source quest with fixed item reward uses the ordinary finish helper.
    let info = (1..140).filter_map(crystal_quest_info_by_id).find(|info|
        info.rewards_select_item.is_empty() && info.rewards_fixed_item.iter().any(|reward| reward.count > 0)
            && effective_crystal_quest_template_by_id(session.app.world(), info.index).is_some()
    ).unwrap();
    ensure_runtime_quest(session.app.world_mut(), info.index);
    set_quest_stage(session.app.world_mut(), info.index, QuestStage::ReadyToTurnIn);
    assert!(complete_quest_with_selection(session.app.world_mut(), info.index, None));
    assert_eq!(quest_stage(session.app.world(), info.index), Some(QuestStage::Completed));
    let high = allocator.issued_through().unwrap();
    assert!(high > 5_000_000);
    let before = state_image(&session);
    assert!(!complete_quest_with_selection(session.app.world_mut(), info.index, None));
    assert_eq!(state_image(&session), before);
    assert_eq!(allocator.issued_through().unwrap(), high);
}

#[test]
fn carry_batch_failure_keeps_accept_stage_and_success_uses_quest_grid() {
    let (mut session, allocator, path) = trusted_session("quest-carry", 5_000_000);
    let template = crystal_quest_packet_manifest().quests.iter().find(|template|
        !template.carry_items.is_empty() && template.carry_items.iter().all(|task| crystal_item_by_index(task.item_index).is_some())
    ).unwrap().clone();
    ensure_runtime_quest(session.app.world_mut(), template.index);
    let before = state_image(&session);
    let authority_bytes = fs::read(&path).unwrap();
    fs::write(&path, "broken").unwrap();
    assert!(!grant_crystal_carry_items(session.app.world_mut(), &template));
    assert_eq!(state_image(&session), before);
    // Only repair this test fixture's deliberately corrupt state, never product recovery.
    fs::write(&path, authority_bytes).unwrap();
    assert!(grant_crystal_carry_items(session.app.world_mut(), &template));
    let inv = session.app.world().resource::<InventoryResource>();
    assert!(inv.inventory_items.iter().any(|item| item.container == ItemContainer::Quest && item.unique_id > 5_000_000));
    assert!(allocator.issued_through().unwrap() > 5_000_000);
}
