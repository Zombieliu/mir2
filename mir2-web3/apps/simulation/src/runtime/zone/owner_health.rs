//! Exact owner pool notifications are presentation, never life/economy authority.
use super::types::{SessionId, ZonePlayer};
use serde::{Deserialize, Serialize};

/// Current server-owned admission and life. It is not a client protocol shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ZoneOwnerHealthCursor {
    pub session_id: SessionId,
    pub online_owner: String,
    pub object_id: u32,
    pub life_generation: u64,
    pub dead: bool,
    pub health_sequence: u64,
}

/// One committed pool operation, including MP-only and intermediate changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ZoneOwnerHealthChange {
    pub cursor: ZoneOwnerHealthCursor,
    pub hp_before: i32,
    pub mp_before: i32,
    pub hp: i32,
    pub mp: i32,
}

#[derive(Debug, Clone)]
pub(super) struct PendingOwnerHealthChange {
    pub life_generation: u64,
    pub dead: bool,
    pub health_sequence: u64,
    pub hp_before: i32,
    pub mp_before: i32,
    pub hp: i32,
    pub mp: i32,
}

impl ZonePlayer {
    /// The caller names an actual pool operation, not a snapshot reconciliation.
    /// Crystal ChangeMP emits for a nonzero negative amount even at MP0: its
    /// zero-amount guard precedes the lower clamp. Keep that operation explicit.
    pub(super) fn change_owner_mp(&mut self, amount: i32) {
        if amount == 0 {
            return;
        }
        let before = (self.hp, self.mp);
        self.mp = self.mp.saturating_add(amount).max(0);
        self.record_owner_health_change(before, true);
    }

    pub(super) fn set_owner_hp(&mut self, amount: i32) {
        if self.hp == amount {
            return;
        }
        let before = (self.hp, self.mp);
        self.hp = amount.clamp(0, self.max_hp.max(1));
        self.record_owner_health_change(before, true);
    }

    pub(super) fn record_owner_health_change(
        &mut self,
        before: (i32, i32),
        notify_unchanged: bool,
    ) {
        if !notify_unchanged && before == (self.hp, self.mp) {
            return;
        }
        self.owner_health_sequence = self
            .owner_health_sequence
            .checked_add(1)
            .expect("Zone owner health sequence exhausted");
        self.pending_owner_health_changes
            .push(PendingOwnerHealthChange {
                life_generation: self.life_generation,
                dead: self.dead || self.hp == 0,
                health_sequence: self.owner_health_sequence,
                hp_before: before.0,
                mp_before: before.1,
                hp: self.hp,
                mp: self.mp,
            });
    }
}
