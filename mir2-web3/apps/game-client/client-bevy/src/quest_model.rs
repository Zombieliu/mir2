//! Presentation read models for native gameplay UI surfaces.
//!
//! These models are authored as purely authoritative server-driven read models.
//! They apply deterministic updates and never make game-state mutations such as
//! granting rewards, mutating inventory, or deciding quest completion.

use std::collections::VecDeque;

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

/// Maximum number of quest objectives shown in compact HUD labels.
const MAX_COMPACT_OBJECTIVES: usize = 3;
/// Maximum number of recent pickup entries kept for HUD toast/replay.
const MAX_RECENT_PICKUPS: usize = 4;

/// Canonical quest status as observed from server authoritative updates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QuestStatus {
    NotStarted,
    InProgress,
    ReadyToTurnIn,
    Completed,
    Failed,
    Aborted,
    Unknown(String),
}

impl QuestStatus {
    pub fn from_server(value: &str) -> Self {
        match value {
            "NotStarted" | "not_started" | "notStarted" => Self::NotStarted,
            "InProgress" | "in_progress" | "inProgress" | "Active" => Self::InProgress,
            "ReadyToTurnIn" | "ready_to_turn_in" | "readyToTurnIn" => Self::ReadyToTurnIn,
            "Completed" | "completed" => Self::Completed,
            "Failed" | "failed" => Self::Failed,
            "Aborted" | "aborted" => Self::Aborted,
            unknown => Self::Unknown(unknown.to_owned()),
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::InProgress | Self::ReadyToTurnIn)
    }

    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Aborted)
    }

    pub fn label(&self) -> String {
        match self {
            Self::NotStarted => "Not Started".to_owned(),
            Self::InProgress => "In Progress".to_owned(),
            Self::ReadyToTurnIn => "Ready to Turn In".to_owned(),
            Self::Completed => "Completed".to_owned(),
            Self::Failed => "Failed".to_owned(),
            Self::Aborted => "Aborted".to_owned(),
            Self::Unknown(value) => value.clone(),
        }
    }
}

/// One quest objective with explicit progress counters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestObjective {
    pub objective_id: String,
    pub text: String,
    pub current: u32,
    pub target: u32,
}

impl QuestObjective {
    pub fn progress_ratio(&self) -> f32 {
        if self.target == 0 {
            return 0.0;
        }
        (self.current as f32 / self.target as f32).clamp(0.0, 1.0)
    }

    pub fn progress_label(&self) -> String {
        format!("{} / {}", self.current, self.target)
    }

    pub fn is_complete(&self) -> bool {
        self.target > 0 && self.current >= self.target
    }
}

/// One reward entry from a server quest packet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QuestReward {
    Gold {
        amount: u32,
    },
    Experience {
        amount: u32,
    },
    Item {
        item_id: String,
        name: String,
        quantity: u32,
    },
    Unknown {
        label: String,
    },
}

impl QuestReward {
    pub fn label(&self) -> String {
        match self {
            Self::Gold { amount } => format!("{amount} Gold"),
            Self::Experience { amount } => format!("{amount} Exp"),
            Self::Item { name, quantity, .. } => {
                if *quantity == 1 {
                    format!("{name}")
                } else {
                    format!("{name} x{quantity}")
                }
            }
            Self::Unknown { label } => label.clone(),
        }
    }
}

/// One authoritative quest payload from server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quest {
    /// Crystal quest index used by `AcceptQuest` / `FinishQuest` packets.
    pub quest_index: i32,
    /// Crystal NPC index that offers the quest.
    pub accept_npc_index: Option<u32>,
    /// Crystal NPC index that receives the finished quest.
    pub finish_npc_index: Option<u32>,
    pub title: String,
    pub npc_name: Option<String>,
    pub status: QuestStatus,
    pub objectives: Vec<QuestObjective>,
    pub rewards: Vec<QuestReward>,
    pub unknown_text: Option<String>,
}

impl Quest {
    pub fn progress_label(&self) -> String {
        if self.objectives.is_empty() {
            return String::new();
        }
        self.objectives
            .iter()
            .map(QuestObjective::progress_label)
            .collect::<Vec<_>>()
            .join(" | ")
    }

    pub fn rewards_label(&self) -> String {
        if self.rewards.is_empty() {
            return "No reward".to_owned();
        }
        self.rewards
            .iter()
            .map(QuestReward::label)
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn compact_label(&self) -> String {
        let status = self.status.label();
        let objectives = self
            .objectives
            .iter()
            .take(MAX_COMPACT_OBJECTIVES)
            .map(|objective| format!("{} ({})", objective.text, objective.progress_label()))
            .collect::<Vec<_>>()
            .join(" | ");
        if objectives.is_empty() {
            format!("{} [{}]", self.title, status)
        } else {
            format!("{} [{}] {}", self.title, status, objectives)
        }
    }

    pub fn has_unknown_status_text(&self) -> bool {
        matches!(self.status, QuestStatus::Unknown(_))
    }
}

/// Snapshot from server (authoritative quest list).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestSnapshot {
    pub quests: Vec<Quest>,
}

/// Single-objective progress patch from server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestProgressUpdate {
    pub quest_index: i32,
    pub objective_id: String,
    pub text: Option<String>,
    pub current: u32,
    pub target: Option<u32>,
}

/// Quest completion packet surface from server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestCompleteUpdate {
    pub quest_index: i32,
    pub status: QuestStatus,
    pub rewards: Vec<QuestReward>,
    pub unknown_text: Option<String>,
}

/// Shared quest tracker resource used by HUD and quest windows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Resource)]
#[serde(rename_all = "camelCase")]
pub struct QuestTracker {
    pub active_quests: Vec<Quest>,
}

impl QuestTracker {
    fn find_index(&self, quest_index: i32) -> Option<usize> {
        self.active_quests
            .iter()
            .position(|quest| quest.quest_index == quest_index)
    }

    /// Deterministic replacement from authoritative snapshot.
    pub fn apply(&mut self, update: QuestSnapshot) {
        self.active_quests.clear();
        for quest in update.quests {
            self.upsert(quest);
        }
    }

    /// Insert or replace an existing quest by Crystal quest index.
    pub fn upsert(&mut self, quest: Quest) {
        if let Some(index) = self.find_index(quest.quest_index) {
            self.active_quests[index] = quest;
        } else {
            self.active_quests.push(quest);
        }
    }

    /// Update one objective in one active quest.
    ///
    /// If the quest or objective is missing, this call is ignored. This keeps
    /// the model strictly authoritative: only server-driven updates alter the
    /// state.
    pub fn change(&mut self, update: QuestProgressUpdate) -> bool {
        let Some(index) = self.find_index(update.quest_index) else {
            return false;
        };
        let Some(objective) = self.active_quests[index]
            .objectives
            .iter_mut()
            .find(|objective| objective.objective_id == update.objective_id)
        else {
            return false;
        };

        objective.current = update.current;
        if let Some(target) = update.target {
            objective.target = target;
        }
        if let Some(text) = update.text {
            objective.text = text;
        }
        true
    }

    /// Mark quest as complete/failed etc and attach authoritative rewards.
    ///
    /// This method only reflects server outcome; it does not grant any reward in
    /// presentation code.
    pub fn complete(&mut self, update: QuestCompleteUpdate) -> bool {
        let Some(index) = self.find_index(update.quest_index) else {
            return false;
        };

        let quest = &mut self.active_quests[index];
        quest.status = update.status;
        quest.rewards = update.rewards;
        if update.unknown_text.is_some() {
            quest.unknown_text = update.unknown_text;
        }
        true
    }

    /// Remove one quest from tracking.
    pub fn remove(&mut self, quest_index: i32) -> Option<Quest> {
        let Some(index) = self.find_index(quest_index) else {
            return None;
        };
        Some(self.active_quests.remove(index))
    }

    /// Clear all read-model state for quest tracker + related surface.
    pub fn reset(&mut self) {
        self.active_quests.clear();
    }

    /// Short HUD string for top-most tracked quest.
    pub fn compact_head_label(&self) -> String {
        let Some(quest) = self
            .active_quests
            .iter()
            .find(|quest| quest.status.is_active())
            .or_else(|| self.active_quests.first())
        else {
            return "No Active Quests".to_owned();
        };

        let objective_text = quest
            .objectives
            .iter()
            .take(MAX_COMPACT_OBJECTIVES)
            .map(|objective| format!("{} ({})", objective.text, objective.progress_label()))
            .collect::<Vec<_>>()
            .join(" | ");

        if objective_text.is_empty() {
            format!("{} [{}]", quest.title, quest.status.label())
        } else {
            format!(
                "{} [{}] | {}",
                quest.title,
                quest.status.label(),
                objective_text
            )
        }
    }

    /// Rewards display for one quest.
    pub fn compact_reward_label(&self) -> String {
        let Some(quest) = self
            .active_quests
            .iter()
            .find(|quest| quest.status.is_finished())
            .or_else(|| self.active_quests.first())
        else {
            return "No Rewards".to_owned();
        };
        quest.rewards_label()
    }
}

/// One line in NPC dialog panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NpcDialogLine {
    pub text: String,
}

/// One selectable option in NPC dialog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NpcDialogOption {
    pub option_id: String,
    pub label: String,
    pub enabled: bool,
}

/// Full server-driven dialog payload for one NPC session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NpcDialogUpdate {
    pub npc_object_id: u32,
    pub npc_name: Option<String>,
    pub lines: Vec<String>,
    pub options: Vec<NpcDialogOption>,
    pub open: bool,
    pub replace: bool,
}

/// Renderer-neutral NPC dialog model for native HUD/overlay.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Resource)]
#[serde(rename_all = "camelCase")]
pub struct NpcDialogModel {
    pub is_open: bool,
    pub npc_object_id: Option<u32>,
    pub npc_name: Option<String>,
    pub lines: Vec<NpcDialogLine>,
    pub options: Vec<NpcDialogOption>,
}

impl NpcDialogModel {
    /// Apply authoritative update from server.
    pub fn apply(&mut self, update: NpcDialogUpdate) {
        self.is_open = update.open;
        if !update.open {
            self.lines.clear();
            self.options.clear();
            self.npc_object_id = None;
            self.npc_name = None;
            return;
        }

        self.npc_object_id = Some(update.npc_object_id);
        self.npc_name = update.npc_name;

        if update.replace {
            self.lines.clear();
            self.options.clear();
        }

        self.lines
            .extend(update.lines.into_iter().map(|text| NpcDialogLine { text }));
        self.options.extend(update.options);
    }

    /// Close panel and clear payload.
    pub fn close(&mut self) {
        self.apply(NpcDialogUpdate {
            npc_object_id: 0,
            npc_name: None,
            lines: Vec::new(),
            options: Vec::new(),
            open: false,
            replace: true,
        })
    }

    /// Single compact text line for HUD.
    pub fn compact_label(&self) -> String {
        if !self.is_open {
            return "No NPC Dialog".to_owned();
        }

        let source = self
            .npc_name
            .clone()
            .or_else(|| self.npc_object_id.map(|object_id| object_id.to_string()))
            .unwrap_or_else(|| "Unknown".to_owned());

        let body = self
            .lines
            .iter()
            .map(|line| line.text.clone())
            .collect::<Vec<_>>()
            .join(" / ");

        if body.is_empty() {
            format!("{source}")
        } else {
            format!("{source}: {body}")
        }
    }
}

/// One authoritative NPC entity currently present in the scene snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NearbyNpc {
    pub object_id: u32,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub quest_indexes: Vec<i32>,
    pub distance: u32,
}

/// Nearby NPC list sorted by distance from the authoritative self position.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Resource)]
#[serde(rename_all = "camelCase")]
pub struct NearbyNpcModel {
    pub npcs: Vec<NearbyNpc>,
}

impl NearbyNpcModel {
    pub fn nearest(&self) -> Option<&NearbyNpc> {
        self.npcs.first()
    }

    pub fn reset(&mut self) {
        self.npcs.clear();
    }
}

/// Authoritative target HP patch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatTarget {
    pub object_id: u32,
    pub name: String,
    pub hp: i32,
    pub max_hp: i32,
    pub is_player: bool,
}

impl CombatTarget {
    fn normalize_hp(&mut self) {
        let max_hp = self.max_hp.max(0);
        self.max_hp = max_hp;
        if max_hp == 0 {
            self.hp = 0;
            return;
        }
        self.hp = self.hp.clamp(0, max_hp);
    }

    pub fn hp_ratio(&self) -> f32 {
        if self.max_hp == 0 {
            0.0
        } else {
            self.hp as f32 / self.max_hp as f32
        }
    }

    pub fn hp_label(&self) -> String {
        format!("{} / {}", self.hp, self.max_hp)
    }

    pub fn is_dead(&self) -> bool {
        self.hp <= 0
    }
}

/// Server update for current combat target.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatTargetUpdate {
    pub object_id: u32,
    pub name: String,
    pub hp: i32,
    pub max_hp: i32,
    pub is_player: bool,
}

/// One target-focused read model resource.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Resource)]
#[serde(rename_all = "camelCase")]
pub struct CombatTargetModel {
    pub target: Option<CombatTarget>,
}

impl CombatTargetModel {
    pub fn apply(&mut self, update: CombatTargetUpdate) {
        let mut target = CombatTarget {
            object_id: update.object_id,
            name: update.name,
            hp: update.hp,
            max_hp: update.max_hp,
            is_player: update.is_player,
        };
        target.normalize_hp();
        self.target = Some(target);
    }

    pub fn clear(&mut self) {
        self.target = None;
    }

    pub fn compact_label(&mut self) -> String {
        let Some(target) = self.target.as_ref() else {
            return "No Target".to_owned();
        };

        let hp = target.hp_label();
        let status = if target.is_dead() { "[Dead]" } else { "" };
        format!("{} {} {}", target.name, status, hp)
    }
}

/// One ground-item pickup event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentPickup {
    /// Authoritative ground-drop object id accepted by `PickUp`.
    pub object_id: Option<u32>,
    pub key: String,
    pub label: String,
    pub amount: u32,
    pub from_npc: Option<String>,
}

impl RecentPickup {
    pub fn compact_label(&self) -> String {
        if self.amount <= 1 {
            return self.label.clone();
        }
        format!("{} x{}", self.label, self.amount)
    }
}

/// Ground pickup event model.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Resource)]
#[serde(rename_all = "camelCase")]
pub struct GroundPickupModel {
    pub recent: VecDeque<RecentPickup>,
}

impl GroundPickupModel {
    pub fn upsert(&mut self, pickup: RecentPickup) {
        if let Some(index) = self
            .recent
            .iter()
            .position(|candidate| candidate.key == pickup.key)
        {
            self.recent.remove(index);
        }

        self.recent.push_front(pickup);
        while self.recent.len() > MAX_RECENT_PICKUPS {
            self.recent.pop_back();
        }
    }

    pub fn reset(&mut self) {
        self.recent.clear();
    }

    /// HUD line for newest item, fallback to most recent list.
    pub fn compact_label(&self) -> String {
        self.recent
            .front()
            .map_or("No Recent Pickup".to_owned(), RecentPickup::compact_label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_quest_in_progress() -> Quest {
        Quest {
            quest_index: 1,
            accept_npc_index: Some(3),
            finish_npc_index: Some(4),
            title: "Bandit Hunt".to_owned(),
            npc_name: Some("Guard".to_owned()),
            status: QuestStatus::InProgress,
            objectives: vec![QuestObjective {
                objective_id: "o1".to_owned(),
                text: "Kill 3 monsters".to_owned(),
                current: 1,
                target: 3,
            }],
            rewards: vec![QuestReward::Gold { amount: 100 }],
            unknown_text: None,
        }
    }

    fn sample_quest_updated_progress() -> Quest {
        Quest {
            quest_index: 1,
            accept_npc_index: Some(3),
            finish_npc_index: Some(4),
            title: "Bandit Hunt".to_owned(),
            npc_name: Some("Guard".to_owned()),
            status: QuestStatus::InProgress,
            objectives: vec![QuestObjective {
                objective_id: "o1".to_owned(),
                text: "Kill 3 monsters".to_owned(),
                current: 3,
                target: 3,
            }],
            rewards: vec![QuestReward::Gold { amount: 100 }],
            unknown_text: None,
        }
    }

    #[test]
    fn authoritative_quest_apply_change_complete_and_remove_work() {
        let mut tracker = QuestTracker::default();

        tracker.apply(QuestSnapshot {
            quests: vec![sample_quest_in_progress()],
        });
        assert_eq!(tracker.active_quests.len(), 1);

        let changed = tracker.change(QuestProgressUpdate {
            quest_index: 1,
            objective_id: "o1".to_owned(),
            text: Some("Kill 3 monsters".to_owned()),
            current: 2,
            target: Some(3),
        });
        assert!(changed);
        assert_eq!(tracker.active_quests[0].objectives[0].current, 2);

        tracker.upsert(sample_quest_updated_progress());
        assert_eq!(tracker.active_quests[0].objectives[0].current, 3);

        let complete = tracker.complete(QuestCompleteUpdate {
            quest_index: 1,
            status: QuestStatus::Completed,
            rewards: vec![
                QuestReward::Experience { amount: 80 },
                QuestReward::Item {
                    item_id: "potion_hp".to_owned(),
                    name: "Potion".to_owned(),
                    quantity: 2,
                },
            ],
            unknown_text: None,
        });
        assert!(complete);
        assert_eq!(tracker.active_quests[0].status, QuestStatus::Completed);
        assert_eq!(tracker.active_quests[0].rewards.len(), 2);

        let removed = tracker.remove(1);
        assert!(removed.is_some());
        assert!(tracker.active_quests.is_empty());
    }

    #[test]
    fn no_duplicate_quest_entry_after_apply_or_upsert() {
        let mut tracker = QuestTracker::default();
        let duplicate = Quest {
            quest_index: 1,
            accept_npc_index: Some(3),
            finish_npc_index: Some(4),
            title: "First Title".to_owned(),
            npc_name: Some("Guard".to_owned()),
            status: QuestStatus::InProgress,
            objectives: Vec::new(),
            rewards: Vec::new(),
            unknown_text: None,
        };

        tracker.apply(QuestSnapshot {
            quests: vec![duplicate.clone(), duplicate.clone()],
        });
        assert_eq!(tracker.active_quests.len(), 1);
        assert_eq!(tracker.active_quests[0].title, "First Title");

        tracker.upsert(Quest {
            title: "Second Title".to_owned(),
            ..duplicate
        });
        assert_eq!(tracker.active_quests.len(), 1);
        assert_eq!(tracker.active_quests[0].title, "Second Title");
    }

    #[test]
    fn progress_and_completion_reward_labels_are_stable() {
        let quest = sample_quest_updated_progress();
        assert_eq!(quest.progress_label(), "3 / 3");
        assert_eq!(
            quest.compact_label(),
            "Bandit Hunt [In Progress] Kill 3 monsters (3 / 3)"
        );

        let label = Quest {
            rewards: vec![
                QuestReward::Gold { amount: 50 },
                QuestReward::Item {
                    item_id: "item_01".to_owned(),
                    name: "Potion".to_owned(),
                    quantity: 1,
                },
                QuestReward::Item {
                    item_id: "item_02".to_owned(),
                    name: "Arrow".to_owned(),
                    quantity: 3,
                },
            ],
            ..sample_quest_updated_progress()
        };
        assert_eq!(label.rewards_label(), "50 Gold, Potion, Arrow x3");
    }

    #[test]
    fn target_hp_is_clamped_and_displayed() {
        let mut target_model = CombatTargetModel::default();
        target_model.apply(CombatTargetUpdate {
            object_id: 1,
            name: "Spider".to_owned(),
            hp: 999,
            max_hp: 500,
            is_player: false,
        });
        assert_eq!(target_model.compact_label(), "Spider  500 / 500");

        target_model.apply(CombatTargetUpdate {
            object_id: 1,
            name: "Spider".to_owned(),
            hp: -8,
            max_hp: 500,
            is_player: false,
        });
        assert_eq!(target_model.compact_label(), "Spider [Dead] 0 / 500");
    }

    #[test]
    fn dialog_updates_replace_then_close_cleanly() {
        let mut dialog = NpcDialogModel::default();
        dialog.apply(NpcDialogUpdate {
            npc_object_id: 101,
            npc_name: Some("Blacksmith".to_owned()),
            lines: vec!["Hello".to_owned(), "I can upgrade".to_owned()],
            options: vec![NpcDialogOption {
                option_id: "o1".to_owned(),
                label: "Yes".to_owned(),
                enabled: true,
            }],
            open: true,
            replace: true,
        });

        assert!(dialog.is_open);
        assert_eq!(dialog.lines.len(), 2);
        assert!(dialog.compact_label().contains("Blacksmith"));

        dialog.apply(NpcDialogUpdate {
            npc_object_id: 101,
            npc_name: Some("Blacksmith".to_owned()),
            lines: vec!["Final line".to_owned()],
            options: Vec::new(),
            open: true,
            replace: true,
        });
        assert_eq!(dialog.lines.len(), 1);
        assert_eq!(dialog.lines[0].text, "Final line");

        dialog.close();
        assert!(!dialog.is_open);
        assert!(dialog.lines.is_empty());
        assert_eq!(dialog.compact_label(), "No NPC Dialog");
    }

    #[test]
    fn pickup_feedback_keeps_recent_list_and_preserves_label() {
        let mut pickup_model = GroundPickupModel::default();
        pickup_model.upsert(RecentPickup {
            object_id: Some(1),
            key: "a".to_owned(),
            label: "Potion".to_owned(),
            amount: 1,
            from_npc: None,
        });
        pickup_model.upsert(RecentPickup {
            object_id: Some(2),
            key: "b".to_owned(),
            label: "Gold".to_owned(),
            amount: 50,
            from_npc: Some("Monster".to_owned()),
        });
        pickup_model.upsert(RecentPickup {
            object_id: Some(3),
            key: "c".to_owned(),
            label: "Potion".to_owned(),
            amount: 2,
            from_npc: None,
        });
        pickup_model.upsert(RecentPickup {
            object_id: Some(4),
            key: "d".to_owned(),
            label: "Arrow".to_owned(),
            amount: 7,
            from_npc: Some("Vendor".to_owned()),
        });
        pickup_model.upsert(RecentPickup {
            object_id: Some(5),
            key: "e".to_owned(),
            label: "Boots".to_owned(),
            amount: 1,
            from_npc: None,
        });

        assert_eq!(pickup_model.recent.len(), MAX_RECENT_PICKUPS);
        assert_eq!(pickup_model.compact_label(), "Boots");

        pickup_model.upsert(RecentPickup {
            object_id: Some(4),
            key: "d".to_owned(),
            label: "Arrow".to_owned(),
            amount: 9,
            from_npc: Some("Vendor".to_owned()),
        });
        assert_eq!(pickup_model.compact_label(), "Arrow x9");
        assert_eq!(pickup_model.recent.len(), MAX_RECENT_PICKUPS);

        pickup_model.reset();
        assert_eq!(pickup_model.compact_label(), "No Recent Pickup");
    }

    #[test]
    fn unknown_quest_status_and_npc_text_are_preserved() {
        let unknown_status = QuestStatus::from_server("QuestStatus.WAITING_FOR_PLAYER");
        let quest = Quest {
            quest_index: -1,
            accept_npc_index: None,
            finish_npc_index: None,
            title: "Mystery".to_owned(),
            npc_name: Some("UnknownNPC".to_owned()),
            status: unknown_status.clone(),
            objectives: Vec::new(),
            rewards: vec![QuestReward::Unknown {
                label: "ServerTextReward".to_owned(),
            }],
            unknown_text: Some("literal narrative line".to_owned()),
        };

        assert!(quest.has_unknown_status_text());
        assert_eq!(quest.status.label(), "QuestStatus.WAITING_FOR_PLAYER");

        let mut dialog = NpcDialogModel::default();
        dialog.apply(NpcDialogUpdate {
            npc_object_id: 999,
            npc_name: Some("UnknownNPC".to_owned()),
            lines: vec!["line: [A]".to_owned()],
            options: Vec::new(),
            open: true,
            replace: true,
        });

        assert_eq!(dialog.compact_label(), "UnknownNPC: line: [A]");
        assert_eq!(
            quest.unknown_text,
            Some("literal narrative line".to_owned())
        );
    }
}
