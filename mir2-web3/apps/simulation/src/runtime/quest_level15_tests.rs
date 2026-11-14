use super::*;
use crate::{SimulationConfig, SimulationSession};

#[test]
fn zero_count_crystal_rewards_neither_reserve_slots_nor_grant_items() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    for quest_id in [37, 41] {
        let info = crystal_quest_info_by_id(quest_id).unwrap();
        let zero_rewards = info
            .rewards_fixed_item
            .iter()
            .filter(|reward| reward.count == 0)
            .collect::<Vec<_>>();
        assert!(
            !zero_rewards.is_empty(),
            "q{quest_id} has a source zero-count reward"
        );
        assert_eq!(
            crystal_quest_reward_slots_needed(session.app.world(), zero_rewards.iter().copied()),
            0
        );
        let before = session.world_snapshot().inventory_items;
        for reward in zero_rewards {
            grant_crystal_quest_reward_item(session.app.world_mut(), reward);
        }
        assert_eq!(session.world_snapshot().inventory_items, before);
    }
}

#[test]
fn instructor_quests_require_the_exact_oma_monster() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    for quest_id in [8, 11, 14] {
        ensure_runtime_quest(session.app.world_mut(), quest_id);
        set_quest_stage(session.app.world_mut(), quest_id, QuestStage::InProgress);
        let template = crystal_quest_template_by_id(quest_id).unwrap();
        assert!(template
            .kill_tasks
            .iter()
            .any(|t| t.monster_index == 48 && t.monster_name == "Oma"));
    }
    for unrelated in ["Oma0", "OmaFighter", "OmaWarrior", "OmaGuard", "OmaKing"] {
        assert!(advance_crystal_quest_kill(session.app.world_mut(), unrelated).is_empty());
        for quest_id in [8, 11, 14] {
            assert_eq!(quest_progress(session.app.world(), quest_id).unwrap().0, 0);
        }
    }
    for (name, expected) in [("Oma", 1), ("oma", 2)] {
        let packets = advance_crystal_quest_kill(session.app.world_mut(), name);
        for quest_id in [8, 11, 14] {
            assert_eq!(
                quest_progress(session.app.world(), quest_id).unwrap().0,
                expected
            );
            assert!(packets.iter().any(|p| matches!(p,
                ServerPacket::ChangeQuest { quest_id: id, .. } if *id == quest_id)));
        }
    }
}
