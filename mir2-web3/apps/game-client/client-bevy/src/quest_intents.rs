//! Shared bounded quest UI intent handoff for native and portable Bevy surfaces.

use std::collections::VecDeque;

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::pending_operations::{PendingOperationKey, PendingOperations};

pub const MAX_QUEUED_INTENTS: usize = 24;

/// Player-side intents emitted by the in-game UI, to be bridged by host layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum QuestUiIntent {
    /// Detail-window action, separately scoped from world-click interaction.
    InteractQuestNpc { quest_index: i32, npc_object_id: u32 },
    InteractNpc {
        npc_object_id: u32,
    },
    SelectNpcDialog {
        target: String,
    },
    AcceptQuest {
        npc_index: u32,
        quest_index: i32,
    },
    FinishQuest {
        quest_index: i32,
        selected_item_index: i32,
    },
    AbandonQuest {
        #[serde(rename = "questIndex")]
        quest_index: i32,
    },
    ShareQuest {
        #[serde(rename = "questIndex")]
        quest_index: i32,
    },
    AttackTarget {
        object_id: u32,
    },
    /// Host-resolved Crystal Alt+world-click on a tile without a target.
    /// The gateway/server remains authoritative over nearby carcass selection.
    HarvestDirection {
        direction: String,
    },
    PickUpObject {
        object_id: u32,
    },
    PickUpTile,
}

impl QuestUiIntent {
    pub fn pending_key(&self) -> Option<PendingOperationKey> {
        match self {
            Self::AcceptQuest {
                npc_index,
                quest_index,
            } => Some(PendingOperationKey::QuestAccept {
                npc_index: *npc_index,
                quest_index: *quest_index,
            }),
            Self::FinishQuest {
                quest_index,
                selected_item_index,
            } => Some(PendingOperationKey::QuestFinish {
                quest_index: *quest_index,
                selected_item_index: *selected_item_index,
            }),
            Self::AbandonQuest { quest_index } => Some(PendingOperationKey::QuestAbandon {
                quest_index: *quest_index,
            }),
            Self::InteractNpc { .. }
            | Self::InteractQuestNpc { .. }
            | Self::SelectNpcDialog { .. }
            | Self::ShareQuest { .. }
            | Self::AttackTarget { .. }
            | Self::HarvestDirection { .. }
            | Self::PickUpObject { .. }
            | Self::PickUpTile => None,
        }
    }
}

#[derive(Resource, Debug, Default)]
pub struct QuestUiIntentQueue {
    /// Commands that reached the host bridge but could not enter its bounded
    /// producer lane. They always drain before newly generated UI input.
    pub(crate) retry_intents: VecDeque<QuestUiIntent>,
    intents: VecDeque<QuestUiIntent>,
    overflow_count: u64,
}

impl QuestUiIntentQueue {
    /// Queue new UI input without evicting an older intent. At capacity the
    /// incoming intent is rejected explicitly, preserving FIFO and any
    /// protected host-backpressure retries.
    pub fn push_intent(&mut self, intent: QuestUiIntent) -> bool {
        if self.len() >= MAX_QUEUED_INTENTS {
            self.overflow_count = self.overflow_count.saturating_add(1);
            return false;
        }
        self.intents.push_back(intent);
        true
    }

    /// Retain host-send failures as the highest-priority FIFO. The bridge calls
    /// this only after draining a bounded batch, so every failed item normally
    /// fits. If another producer populated the queue in between, unsent new
    /// input is evicted from the back before a retry is ever sacrificed.
    pub fn retain_failed_intents(
        &mut self,
        failed: impl IntoIterator<Item = QuestUiIntent>,
    ) -> Vec<QuestUiIntent> {
        let mut dropped = Vec::new();
        for intent in failed {
            if self.len() >= MAX_QUEUED_INTENTS {
                if let Some(evicted) = self.intents.pop_back() {
                    self.overflow_count = self.overflow_count.saturating_add(1);
                    dropped.push(evicted);
                } else {
                    // A full queue made solely of older retries cannot accept
                    // another retry without becoming unbounded. Return it to
                    // the host so any matching pending operation is released
                    // explicitly instead of being stranded forever.
                    self.overflow_count = self.overflow_count.saturating_add(1);
                    dropped.push(intent);
                    continue;
                }
            }
            self.retry_intents.push_back(intent);
        }
        dropped
    }

    pub fn drain_intents(&mut self) -> Vec<QuestUiIntent> {
        let mut drained = Vec::with_capacity(self.len());
        drained.extend(self.retry_intents.drain(..));
        drained.extend(self.intents.drain(..));
        drained
    }

    pub fn push_pending_intent(
        &mut self,
        pending: &mut PendingOperations,
        intent: QuestUiIntent,
    ) -> bool {
        let Some(key) = intent.pending_key() else {
            return self.push_intent(intent);
        };
        if !pending.try_begin(key.clone()) {
            return false;
        }
        if self.push_intent(intent) {
            true
        } else {
            pending.release(&key);
            false
        }
    }

    pub fn clear(&mut self) {
        self.retry_intents.clear();
        self.intents.clear();
    }

    /// A manual move, cancellation or target switch supersedes unsent combat
    /// requests. Keep quest and NPC operations in their original FIFO order.
    pub fn clear_attack_intents(&mut self) -> usize {
        let before = self.len();
        self.retry_intents
            .retain(|intent| !matches!(intent, QuestUiIntent::AttackTarget { .. }));
        self.intents
            .retain(|intent| !matches!(intent, QuestUiIntent::AttackTarget { .. }));
        before - self.len()
    }

    pub fn is_empty(&self) -> bool {
        self.retry_intents.is_empty() && self.intents.is_empty()
    }

    pub fn len(&self) -> usize {
        self.retry_intents.len() + self.intents.len()
    }

    pub fn is_full(&self) -> bool {
        self.len() >= MAX_QUEUED_INTENTS
    }

    pub fn retry_len(&self) -> usize {
        self.retry_intents.len()
    }

    pub fn overflow_count(&self) -> u64 {
        self.overflow_count
    }
}
