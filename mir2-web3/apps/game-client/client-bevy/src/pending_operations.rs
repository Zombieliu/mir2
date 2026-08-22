//! Lifecycle guard for server-authoritative native UI operations.
//!
//! A pending entry suppresses an identical logical operation until the host
//! applies a relevant authoritative read model, reports an explicit failure,
//! or resets the session. There is deliberately no timeout: elapsed client
//! time is not proof that the server accepted or rejected an operation.

use std::collections::HashSet;

use bevy::prelude::{Resource, SystemSet};
use serde::{Deserialize, Serialize};

pub const MAX_PENDING_OPERATIONS: usize = 128;

/// Shared schedule boundary used by the runtime ingest path and native UI.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PendingLifecycleSet {
    /// Native model/reset messages are applied here.
    Ingest,
    /// Native-only UI/session resources are reset after ingestion.
    UiReset,
}

/// Authoritative model families whose revisions are observable by adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthoritativeModelDomain {
    Inventory,
    Mail,
    Shop,
    /// Cash GameShop catalog/stock refreshes are observable, but they are not
    /// purchase acknowledgements.  The domain revision is diagnostic only.
    GameShop,
    Storage,
    Quest,
}

/// Stable identity of one logical operation.
///
/// Password values are intentionally absent: secrets must not live in a hash
/// set or debug output. A second password operation of the same kind remains
/// blocked until authoritative storage state arrives.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PendingOperationKey {
    Buy {
        item_index: u64,
        count: u16,
    },
    GameShop(String),
    ClaimMail(u64),
    DeleteMail(u64),
    ReadMail(u64),
    SendMail {
        recipient: String,
        message: String,
        gold: u32,
        attachment_unique_ids: Vec<u64>,
    },
    Sell {
        unique_id: u64,
        count: u16,
    },
    Repair(u64),
    SpecialRepair(u64),
    StorageDeposit {
        unique_id: u64,
        from: i32,
        to: i32,
    },
    StorageWithdraw {
        unique_id: u64,
        from: i32,
        to: i32,
    },
    Drop {
        unique_id: u64,
        count: u16,
        hero_inventory: bool,
    },
    Move {
        grid: String,
        unique_id: u64,
        from: i32,
        to: i32,
    },
    Merge {
        grid_from: String,
        grid_to: String,
        id_from: u64,
        id_to: u64,
    },
    Split {
        grid: String,
        unique_id: u64,
        count: u16,
    },
    StorageUnlock,
    StorageSetPassword,
    StorageRemovePassword,
    StorageExpand,
    QuestAccept {
        npc_index: u32,
        quest_index: i32,
    },
    QuestFinish {
        quest_index: i32,
        selected_item_index: i32,
    },
    QuestAbandon {
        quest_index: i32,
    },
}

/// Correlatable Crystal inventory mutation acknowledgement forwarded by the
/// native Gateway. `SplitItem` (without the `1` suffix) is intentionally not
/// represented because it lacks the source id/count and cannot release a key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "camelCase")]
pub enum InventoryOperationAck {
    Drop {
        unique_id: u64,
        count: u16,
        hero_inventory: bool,
        success: bool,
    },
    Move {
        grid: String,
        from: i32,
        to: i32,
        success: bool,
    },
    Merge {
        grid_from: String,
        grid_to: String,
        id_from: u64,
        id_to: u64,
        success: bool,
    },
    Split {
        grid: String,
        unique_id: u64,
        count: u16,
        success: bool,
    },
    Sell {
        unique_id: u64,
        count: u16,
        success: bool,
    },
}

impl InventoryOperationAck {
    pub fn success(&self) -> bool {
        match self {
            Self::Drop { success, .. }
            | Self::Move { success, .. }
            | Self::Merge { success, .. }
            | Self::Split { success, .. }
            | Self::Sell { success, .. } => *success,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Drop { .. } => "Drop",
            Self::Move { .. } => "Move",
            Self::Merge { .. } => "Merge",
            Self::Split { .. } => "Split",
            Self::Sell { .. } => "Sell",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Resource)]
pub struct InventoryOperationFeedback {
    pub last: Option<InventoryOperationAck>,
}

/// ACK and NACK both terminate the exact in-flight command; neither mutates
/// inventory state. The next authoritative snapshot remains the source of
/// truth for item contents and slots.
pub fn apply_inventory_operation_ack(
    pending: &mut PendingOperations,
    feedback: &mut InventoryOperationFeedback,
    ack: InventoryOperationAck,
) -> usize {
    let released = pending.release_matching(|key| match (&ack, key) {
        (
            InventoryOperationAck::Drop {
                unique_id,
                count,
                hero_inventory,
                ..
            },
            PendingOperationKey::Drop {
                unique_id: pending_id,
                count: pending_count,
                hero_inventory: pending_hero,
            },
        ) => unique_id == pending_id && count == pending_count && hero_inventory == pending_hero,
        (
            InventoryOperationAck::Move { grid, from, to, .. },
            PendingOperationKey::Move {
                grid: pending_grid,
                from: pending_from,
                to: pending_to,
                ..
            },
        ) => grid.eq_ignore_ascii_case(pending_grid) && from == pending_from && to == pending_to,
        (
            InventoryOperationAck::Merge {
                grid_from,
                grid_to,
                id_from,
                id_to,
                ..
            },
            PendingOperationKey::Merge {
                grid_from: pending_grid_from,
                grid_to: pending_grid_to,
                id_from: pending_id_from,
                id_to: pending_id_to,
            },
        ) => {
            grid_from.eq_ignore_ascii_case(pending_grid_from)
                && grid_to.eq_ignore_ascii_case(pending_grid_to)
                && id_from == pending_id_from
                && id_to == pending_id_to
        }
        (
            InventoryOperationAck::Split {
                grid,
                unique_id,
                count,
                ..
            },
            PendingOperationKey::Split {
                grid: pending_grid,
                unique_id: pending_id,
                count: pending_count,
            },
        ) => {
            grid.eq_ignore_ascii_case(pending_grid)
                && unique_id == pending_id
                && count == pending_count
        }
        (
            InventoryOperationAck::Sell {
                unique_id, count, ..
            },
            PendingOperationKey::Sell {
                unique_id: pending_id,
                count: pending_count,
            },
        ) => unique_id == pending_id && count == pending_count,
        _ => false,
    });
    feedback.last = Some(ack);
    released
}

/// Correlatable Crystal warehouse acknowledgement. Store/TakeBack packets do
/// not echo the item id, so their strongest available identity is the exact
/// source/destination pair. Password result packets identify their operation
/// explicitly through the packet kind and `removing` flag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "camelCase")]
pub enum StorageOperationAck {
    Deposit { from: i32, to: i32, success: bool },
    Withdraw { from: i32, to: i32, success: bool },
    Unlock { success: bool },
    SetPassword { success: bool },
    RemovePassword { success: bool },
    Expand { success: bool },
}

impl StorageOperationAck {
    pub fn success(&self) -> bool {
        match self {
            Self::Deposit { success, .. }
            | Self::Withdraw { success, .. }
            | Self::Unlock { success }
            | Self::SetPassword { success }
            | Self::RemovePassword { success }
            | Self::Expand { success } => *success,
        }
    }
}

/// ACK and NACK both terminate only the matching warehouse operation. Model
/// contents remain server-authoritative and are updated by the subsequent
/// storage/inventory refresh rather than by this acknowledgement.
pub fn apply_storage_operation_ack(
    pending: &mut PendingOperations,
    ack: &StorageOperationAck,
) -> usize {
    pending.release_matching(|key| match (ack, key) {
        (
            StorageOperationAck::Deposit { from, to, .. },
            PendingOperationKey::StorageDeposit {
                from: pending_from,
                to: pending_to,
                ..
            },
        ) => from == pending_from && to == pending_to,
        (
            StorageOperationAck::Withdraw { from, to, .. },
            PendingOperationKey::StorageWithdraw {
                from: pending_from,
                to: pending_to,
                ..
            },
        ) => from == pending_from && to == pending_to,
        (StorageOperationAck::Unlock { .. }, PendingOperationKey::StorageUnlock)
        | (StorageOperationAck::SetPassword { .. }, PendingOperationKey::StorageSetPassword)
        | (
            StorageOperationAck::RemovePassword { .. },
            PendingOperationKey::StorageRemovePassword,
        )
        | (StorageOperationAck::Expand { .. }, PendingOperationKey::StorageExpand) => true,
        _ => false,
    })
}

/// Bounded set of operations awaiting authoritative completion evidence.
#[derive(Debug, Default, Resource)]
pub struct PendingOperations {
    entries: HashSet<PendingOperationKey>,
}

impl PendingOperations {
    /// Register a logical operation. Returns `false` for an exact duplicate.
    pub fn try_begin(&mut self, key: PendingOperationKey) -> bool {
        if self.entries.contains(&key) || self.entries.len() >= MAX_PENDING_OPERATIONS {
            return false;
        }
        self.entries.insert(key)
    }

    /// Explicit host-side ACK/NACK hook. Revisions alone never call this;
    /// uncorrelatable operations remain locked until a true session reset.
    pub fn release(&mut self, key: &PendingOperationKey) -> bool {
        self.entries.remove(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Clear every operation except the one exact GameShop request protected
    /// by a terminal-session receipt boundary. This is not an ACK: the key is
    /// retained until the typed receipt is applied later in the same or a
    /// subsequent Bevy frame.
    pub fn retain_exact_game_shop(&mut self, request_id: &str) {
        self.entries.retain(
            |key| matches!(key, PendingOperationKey::GameShop(pending) if pending == request_id),
        );
    }

    pub fn contains(&self, key: &PendingOperationKey) -> bool {
        self.entries.contains(key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Mail ACKs carry no request id. The native gateway therefore permits
    /// only one claim and one send at a time; these class checks mirror that
    /// transport boundary before an intent enters the queue.
    pub fn has_pending_mail_claim(&self) -> bool {
        self.entries
            .iter()
            .any(|key| matches!(key, PendingOperationKey::ClaimMail(_)))
    }

    pub fn has_pending_mail_send(&self) -> bool {
        self.entries
            .iter()
            .any(|key| matches!(key, PendingOperationKey::SendMail { .. }))
    }

    pub fn has_pending_mail_operation(&self) -> bool {
        self.has_pending_mail_claim() || self.has_pending_mail_send()
    }

    fn release_matching(&mut self, mut proven: impl FnMut(&PendingOperationKey) -> bool) -> usize {
        let before = self.entries.len();
        self.entries.retain(|key| !proven(key));
        before - self.entries.len()
    }
}

/// Monotonic counters proving that a renderer-neutral authoritative model was
/// applied. They are diagnostic and testable; no UI mutation is inferred from
/// a counter alone.
#[derive(Debug, Default, Resource)]
pub struct AuthoritativeModelRevisions {
    inventory: u64,
    mail: u64,
    shop: u64,
    game_shop: u64,
    storage: u64,
    quest: u64,
    session_generation: u64,
}

impl AuthoritativeModelRevisions {
    pub fn get(&self, domain: AuthoritativeModelDomain) -> u64 {
        match domain {
            AuthoritativeModelDomain::Inventory => self.inventory,
            AuthoritativeModelDomain::Mail => self.mail,
            AuthoritativeModelDomain::Shop => self.shop,
            AuthoritativeModelDomain::GameShop => self.game_shop,
            AuthoritativeModelDomain::Storage => self.storage,
            AuthoritativeModelDomain::Quest => self.quest,
        }
    }

    pub fn advance(&mut self, domain: AuthoritativeModelDomain) -> u64 {
        let revision = match domain {
            AuthoritativeModelDomain::Inventory => &mut self.inventory,
            AuthoritativeModelDomain::Mail => &mut self.mail,
            AuthoritativeModelDomain::Shop => &mut self.shop,
            AuthoritativeModelDomain::GameShop => &mut self.game_shop,
            AuthoritativeModelDomain::Storage => &mut self.storage,
            AuthoritativeModelDomain::Quest => &mut self.quest,
        };
        *revision = revision.wrapping_add(1);
        *revision
    }

    pub fn reset_session(&mut self) {
        self.inventory = 0;
        self.mail = 0;
        self.shop = 0;
        self.game_shop = 0;
        self.storage = 0;
        self.quest = 0;
        self.session_generation = self.session_generation.wrapping_add(1);
    }

    pub fn session_generation(&self) -> u64 {
        self.session_generation
    }
}

/// Generation observed by both the runtime read-model reset and native UI
/// reset systems. Logout/disconnect and `DataReset` advance it.
#[derive(Debug, Default, Resource)]
pub struct SessionResetRevision(pub u64);

impl SessionResetRevision {
    pub fn request(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(1);
        self.0
    }
}

/// One reset-revision-scoped exception to the ordinary session boundary.
///
/// All account/session models are still reset. Only the exact GameShop
/// correlation needed to consume a receipt already accepted by the transport
/// is retained. A later ordinary reset advances the revision and therefore
/// cannot accidentally reuse this exception for another account.
#[derive(Debug, Clone, Default, Resource)]
pub struct SessionResetGameShopPreservation {
    revision: u64,
    receipt: Option<crate::game_shop::GameShopReceipt>,
    consumed: bool,
}

impl SessionResetGameShopPreservation {
    pub fn receipt_for(&self, revision: u64) -> Option<&crate::game_shop::GameShopReceipt> {
        (self.revision == revision)
            .then_some(self.receipt.as_ref())
            .flatten()
    }

    fn set(&mut self, revision: u64, receipt: crate::game_shop::GameShopReceipt) {
        self.revision = revision;
        self.receipt = Some(receipt);
        self.consumed = false;
    }

    /// Mark the exact revision/request as consumed by the runtime model. The
    /// receipt remains available through the rest of `Update` so overlay reset
    /// and reconciliation can apply the same authoritative result.
    pub fn mark_consumed(&mut self, revision: u64, request_id: &str) -> bool {
        if self.revision != revision
            || !self
                .receipt
                .as_ref()
                .is_some_and(|receipt| receipt.request_id == request_id)
        {
            return false;
        }
        self.consumed = true;
        true
    }

    /// Clear only a receipt proven consumed for the active reset revision.
    /// Hosts call this after the complete `Update` schedule has run.
    pub fn clear_if_consumed(&mut self, revision: u64) -> bool {
        if self.revision != revision || !self.consumed {
            return false;
        }
        self.receipt = None;
        self.consumed = false;
        true
    }

    /// An ordinary later reset cannot inherit an older account's exception.
    pub fn clear_if_stale(&mut self, revision: u64) -> bool {
        if self.receipt.is_none() || self.revision == revision {
            return false;
        }
        self.receipt = None;
        self.consumed = false;
        true
    }
}

/// Record a successful authoritative decode.
///
/// A decode or periodic snapshot is not completion evidence. Call one of the
/// operation-specific reconciliation functions below, or release an exact key
/// from an ACK/NACK, when the host can prove what changed.
pub fn mark_authoritative_refresh(
    revisions: &mut AuthoritativeModelRevisions,
    domain: AuthoritativeModelDomain,
) {
    revisions.advance(domain);
}

fn item_by_id(
    model: &crate::inventory::InventoryModel,
    unique_id: u64,
) -> Option<&crate::inventory::ItemModel> {
    model
        .items
        .iter()
        .find(|item| item.unique_id == Some(unique_id))
}

fn quantity_by_id(model: &crate::inventory::InventoryModel, unique_id: u64) -> u32 {
    item_by_id(model, unique_id)
        .map(|item| item.quantity)
        .unwrap_or(0)
}

fn has_ambiguous_replacement_without_instance_id(
    old: &crate::inventory::InventoryModel,
    new: &crate::inventory::InventoryModel,
    unique_id: u64,
) -> bool {
    let Some(previous) = item_by_id(old, unique_id) else {
        return false;
    };
    item_by_id(new, unique_id).is_none()
        && new.items.iter().any(|item| {
            item.unique_id.is_none()
                && item.key == previous.key
                && item.container == previous.container
        })
}

/// Release only inventory operations whose exact old→new effect is visible.
pub fn reconcile_inventory_refresh(
    pending: &mut PendingOperations,
    old: &crate::inventory::InventoryModel,
    new: &crate::inventory::InventoryModel,
) -> usize {
    pending.release_matching(|key| match key {
        PendingOperationKey::Drop {
            unique_id, count, ..
        }
        | PendingOperationKey::Sell { unique_id, count }
        | PendingOperationKey::Split {
            unique_id, count, ..
        } => {
            if has_ambiguous_replacement_without_instance_id(old, new, *unique_id) {
                return false;
            }
            let old_quantity = quantity_by_id(old, *unique_id);
            let new_quantity = quantity_by_id(new, *unique_id);
            old_quantity >= u32::from(*count)
                && new_quantity <= old_quantity.saturating_sub(u32::from(*count))
        }
        PendingOperationKey::Move {
            grid,
            unique_id,
            from,
            to,
        } if grid.eq_ignore_ascii_case("inventory") => {
            let old_at_source = old.items.iter().any(|item| {
                item.container == 0
                    && i32::try_from(item.slot).ok() == Some(*from)
                    && item.unique_id == Some(*unique_id)
            });
            let new_at_target = new.items.iter().any(|item| {
                item.container == 0
                    && i32::try_from(item.slot).ok() == Some(*to)
                    && item.unique_id == Some(*unique_id)
            });
            old_at_source && new_at_target
        }
        PendingOperationKey::Merge { id_from, id_to, .. } => {
            if has_ambiguous_replacement_without_instance_id(old, new, *id_from)
                || has_ambiguous_replacement_without_instance_id(old, new, *id_to)
            {
                return false;
            }
            let old_from = quantity_by_id(old, *id_from);
            let old_to = quantity_by_id(old, *id_to);
            let new_from = quantity_by_id(new, *id_from);
            let new_to = quantity_by_id(new, *id_to);
            old_from > new_from && new_to > old_to
        }
        _ => false,
    })
}

/// Release mail operations only when the addressed message proves the result.
pub fn reconcile_mail_refresh(
    pending: &mut PendingOperations,
    old: &crate::mail::MailModel,
    new: &crate::mail::MailModel,
) -> usize {
    let feedback = new.operation_feedback();
    pending.release_matching(|key| match (feedback, key) {
        (
            Some(crate::mail::MailOperationFeedback {
                kind: crate::mail::MailOperationKind::Send,
                ..
            }),
            PendingOperationKey::SendMail { .. },
        ) => true,
        (
            Some(crate::mail::MailOperationFeedback {
                kind: crate::mail::MailOperationKind::Collect,
                mail_id: Some(id),
                ..
            }),
            PendingOperationKey::ClaimMail(pending_id),
        ) => id == pending_id,
        (
            Some(crate::mail::MailOperationFeedback {
                kind: crate::mail::MailOperationKind::Delete,
                mail_id: Some(id),
                ..
            }),
            PendingOperationKey::DeleteMail(pending_id),
        ) => id == pending_id,
        (
            Some(crate::mail::MailOperationFeedback {
                kind: crate::mail::MailOperationKind::Read,
                mail_id: Some(id),
                ..
            }),
            PendingOperationKey::ReadMail(pending_id),
        ) => id == pending_id,
        (None, PendingOperationKey::ReadMail(id)) => {
            old.mails.iter().any(|mail| mail.id == *id && !mail.read)
                && new.mails.iter().any(|mail| mail.id == *id && mail.read)
        }
        (None, PendingOperationKey::ClaimMail(id)) => {
            old.mails.iter().any(|mail| mail.id == *id && !mail.claimed)
                && new.mails.iter().any(|mail| mail.id == *id && mail.claimed)
        }
        (None, PendingOperationKey::DeleteMail(id)) => {
            old.mails.iter().any(|mail| mail.id == *id)
                && !new.mails.iter().any(|mail| mail.id == *id)
        }
        _ => false,
    })
}

/// NPC shop stock is proof only for finite-stock buys. Unlimited-stock buys,
/// repairs and any unrelated refresh remain pending without a correlatable ACK.
pub fn reconcile_shop_refresh(
    pending: &mut PendingOperations,
    old: &crate::shop::ShopModel,
    new: &crate::shop::ShopModel,
) -> usize {
    pending.release_matching(|key| match key {
        PendingOperationKey::Buy { item_index, count } => {
            let Some(old_good) = old.goods.iter().find(|good| good.unique_id == *item_index) else {
                return false;
            };
            if old_good.stock < 0 {
                return false;
            }
            let new_stock = new
                .goods
                .iter()
                .find(|good| good.unique_id == *item_index)
                .map(|good| good.stock)
                .unwrap_or(0);
            new_stock <= old_good.stock.saturating_sub(i32::from(*count))
        }
        _ => false,
    })
}

fn storage_item_by_id(
    storage: &crate::storage::StorageModel,
    unique_id: u64,
) -> Option<&crate::inventory::ItemModel> {
    storage
        .items
        .iter()
        .find(|item| item.unique_id == Some(unique_id))
}

/// Reconcile storage metadata and transfers against the exact addressed item.
pub fn reconcile_storage_refresh(
    pending: &mut PendingOperations,
    inventory: &crate::inventory::InventoryModel,
    old: &crate::storage::StorageModel,
    new: &crate::storage::StorageModel,
) -> usize {
    pending.release_matching(|key| match key {
        PendingOperationKey::StorageDeposit { unique_id, to, .. } => {
            item_by_id(inventory, *unique_id).is_none()
                && storage_item_by_id(new, *unique_id)
                    .is_some_and(|item| i32::try_from(item.slot).ok() == Some(*to))
        }
        PendingOperationKey::StorageWithdraw { unique_id, to, .. } => {
            storage_item_by_id(new, *unique_id).is_none()
                && item_by_id(inventory, *unique_id)
                    .is_some_and(|item| i32::try_from(item.slot).ok() == Some(*to))
        }
        PendingOperationKey::StorageUnlock => old.has_password && !old.unlocked && new.unlocked,
        PendingOperationKey::StorageSetPassword => !old.has_password && new.has_password,
        PendingOperationKey::StorageRemovePassword => old.has_password && !new.has_password,
        PendingOperationKey::StorageExpand => {
            (!old.has_expanded && new.has_expanded) || new.size > old.size
        }
        _ => false,
    })
}

/// Release quest submissions only when that exact quest changes lifecycle.
#[cfg(feature = "native-ui")]
pub fn reconcile_quest_refresh(
    pending: &mut PendingOperations,
    old: &crate::quest_model::QuestTracker,
    new: &crate::quest_model::QuestTracker,
) -> usize {
    pending.release_matching(|key| match key {
        PendingOperationKey::QuestAccept { quest_index, .. } => {
            let old_active = old
                .active_quests
                .iter()
                .any(|quest| quest.quest_index == *quest_index && quest.status.is_active());
            let new_active = new
                .active_quests
                .iter()
                .any(|quest| quest.quest_index == *quest_index && quest.status.is_active());
            !old_active && new_active
        }
        PendingOperationKey::QuestFinish { quest_index, .. } => {
            let old_quest = old
                .active_quests
                .iter()
                .find(|quest| quest.quest_index == *quest_index);
            let new_quest = new
                .active_quests
                .iter()
                .find(|quest| quest.quest_index == *quest_index);
            old_quest.is_some_and(|quest| quest.status.is_active())
                && new_quest.is_none_or(|quest| quest.status.is_finished())
        }
        PendingOperationKey::QuestAbandon { quest_index } => {
            let old_quest = old
                .active_quests
                .iter()
                .find(|quest| quest.quest_index == *quest_index);
            let new_quest = new
                .active_quests
                .iter()
                .find(|quest| quest.quest_index == *quest_index);
            old_quest
                .is_some_and(|quest| quest.status == crate::quest_model::QuestStatus::InProgress)
                && new_quest
                    .is_none_or(|quest| quest.status != crate::quest_model::QuestStatus::InProgress)
        }
        _ => false,
    })
}

/// Start a new local session boundary without pretending any operation
/// succeeded. Used by native Logout transitions; the runtime consumes the
/// generation on the next ingest phase.
pub fn request_session_reset(
    reset: &mut SessionResetRevision,
    revisions: &mut AuthoritativeModelRevisions,
    pending: &mut PendingOperations,
) {
    reset.request();
    revisions.reset_session();
    pending.clear();
}

/// Reset the session while retaining exactly one already-correlated GameShop
/// receipt until the normal receipt ingest system consumes it. Invalid receipt
/// shapes fail closed without mutating reset state.
pub fn request_session_reset_preserving_exact_game_shop_receipt(
    reset: &mut SessionResetRevision,
    revisions: &mut AuthoritativeModelRevisions,
    pending: &mut PendingOperations,
    preservation: &mut SessionResetGameShopPreservation,
    receipt: crate::game_shop::GameShopReceipt,
) -> bool {
    if !receipt.is_valid() {
        return false;
    }
    let request_id = receipt.request_id.clone();
    let revision = reset.request();
    revisions.reset_session();
    pending.retain_exact_game_shop(&request_id);
    if !pending.contains(&PendingOperationKey::GameShop(request_id.clone())) {
        let _ = pending.try_begin(PendingOperationKey::GameShop(request_id));
    }
    preservation.set(revision, receipt);
    true
}

#[cfg(feature = "native-ui")]
mod native_ui {
    use bevy::prelude::*;

    use crate::crystal_ui::overlays::{
        NativePlayerUiIntentQueue, NativePlayerUiState, UiEffectQueue,
    };
    use crate::native_shell::{NativeShellModel, NativeShellScreen, NativeUiIntentQueue};
    use crate::quest_model::{
        CombatTargetModel, GroundPickupModel, NearbyNpcModel, NpcDialogModel, QuestTracker,
    };
    use crate::quest_ui::{NpcDialogNav, QuestUiIntentQueue, QuestUiState};

    use super::{
        request_session_reset, AuthoritativeModelRevisions, InventoryOperationFeedback,
        PendingOperations, SessionResetGameShopPreservation, SessionResetRevision,
    };

    #[derive(Debug, Default, Resource)]
    pub struct NativeSessionBoundaryTracker {
        initialized: bool,
        had_active_session: bool,
        observed_reset_revision: u64,
    }

    #[derive(Debug, Default, Resource)]
    pub struct OverlayResetTracker(pub u64);

    #[derive(Debug, Default, Resource)]
    pub struct QuestResetTracker(pub u64);

    fn has_active_character(screen: NativeShellScreen) -> bool {
        matches!(
            screen,
            NativeShellScreen::StartingGame | NativeShellScreen::InGame
        )
    }

    /// Detect Logout/disconnect screen transitions after UI input has run.
    pub fn observe_native_session_boundary(
        shell: Res<NativeShellModel>,
        mut tracker: ResMut<NativeSessionBoundaryTracker>,
        mut reset: ResMut<SessionResetRevision>,
        mut revisions: ResMut<AuthoritativeModelRevisions>,
        mut pending: ResMut<PendingOperations>,
    ) {
        let active = has_active_character(shell.screen);
        let reset_already_requested = tracker.observed_reset_revision != reset.0;
        if tracker.initialized && tracker.had_active_session && !active && !reset_already_requested
        {
            request_session_reset(&mut reset, &mut revisions, &mut pending);
        }
        tracker.initialized = true;
        tracker.had_active_session = active;
        tracker.observed_reset_revision = reset.0;
    }

    /// Clear overlay-only selections, drafts and queues after a session reset.
    pub fn apply_overlay_session_reset(
        reset: Res<SessionResetRevision>,
        mut tracker: ResMut<OverlayResetTracker>,
        mut player_ui: ResMut<NativePlayerUiState>,
        mut mail_compose: ResMut<crate::crystal_ui::overlays::MailComposeUi>,
        mut player_intents: ResMut<NativePlayerUiIntentQueue>,
        mut shell_intents: ResMut<NativeUiIntentQueue>,
        mut effects: ResMut<UiEffectQueue>,
        mut pending: ResMut<PendingOperations>,
        mut inventory_feedback: ResMut<InventoryOperationFeedback>,
        preservation: Res<SessionResetGameShopPreservation>,
    ) {
        if tracker.0 == reset.0 {
            return;
        }
        tracker.0 = reset.0;
        let preserved_receipt = preservation.receipt_for(reset.0);
        if let Some(receipt) = preserved_receipt {
            player_ui.reset_session_preserving_exact_game_shop_receipt(receipt);
        } else {
            player_ui.reset_session();
        }
        *mail_compose = Default::default();
        player_intents.clear();
        shell_intents.drain().for_each(drop);
        effects.drain();
        if let Some(receipt) = preserved_receipt {
            pending.retain_exact_game_shop(&receipt.request_id);
        } else {
            pending.clear();
        }
        inventory_feedback.last = None;
    }

    /// Clear quest tracking, NPC dialog, target state and unforwarded intents.
    pub fn apply_quest_session_reset(
        reset: Res<SessionResetRevision>,
        mut tracker: ResMut<QuestResetTracker>,
        mut quests: ResMut<QuestTracker>,
        mut dialog: ResMut<NpcDialogModel>,
        mut nearby: ResMut<NearbyNpcModel>,
        mut target: ResMut<CombatTargetModel>,
        mut pickups: ResMut<GroundPickupModel>,
        mut quest_ui: ResMut<QuestUiState>,
        mut nav: ResMut<NpcDialogNav>,
        mut intents: ResMut<QuestUiIntentQueue>,
    ) {
        if tracker.0 == reset.0 {
            return;
        }
        tracker.0 = reset.0;
        *quests = QuestTracker::default();
        *dialog = NpcDialogModel::default();
        *nearby = NearbyNpcModel::default();
        *target = CombatTargetModel::default();
        *pickups = GroundPickupModel::default();
        quest_ui.reset();
        nav.clear();
        intents.clear();
    }
}

#[cfg(feature = "native-ui")]
pub use native_ui::{
    apply_overlay_session_reset, apply_quest_session_reset, observe_native_session_boundary,
    NativeSessionBoundaryTracker, OverlayResetTracker, QuestResetTracker,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_duplicates_are_blocked_but_distinct_resources_are_not_merged() {
        let mut pending = PendingOperations::default();
        assert!(pending.try_begin(PendingOperationKey::ClaimMail(10)));
        assert!(!pending.try_begin(PendingOperationKey::ClaimMail(10)));
        assert!(pending.try_begin(PendingOperationKey::ClaimMail(11)));
        assert!(pending.try_begin(PendingOperationKey::DeleteMail(10)));
        assert_eq!(pending.len(), 3);
    }

    #[test]
    fn decoded_refresh_advances_revisions_but_never_unlocks_any_domain() {
        let mut pending = PendingOperations::default();
        let mut revisions = AuthoritativeModelRevisions::default();
        let keys = [
            PendingOperationKey::Split {
                grid: "inventory".into(),
                unique_id: 42,
                count: 1,
            },
            PendingOperationKey::ReadMail(7),
            PendingOperationKey::Buy {
                item_index: 9,
                count: 1,
            },
            PendingOperationKey::StorageExpand,
            PendingOperationKey::QuestAccept {
                npc_index: 3,
                quest_index: 11,
            },
        ];
        for key in keys.iter().cloned() {
            assert!(pending.try_begin(key));
        }
        for domain in [
            AuthoritativeModelDomain::Inventory,
            AuthoritativeModelDomain::Mail,
            AuthoritativeModelDomain::Shop,
            AuthoritativeModelDomain::GameShop,
            AuthoritativeModelDomain::Storage,
            AuthoritativeModelDomain::Quest,
        ] {
            mark_authoritative_refresh(&mut revisions, domain);
            assert_eq!(revisions.get(domain), 1);
        }
        for key in &keys {
            assert!(pending.contains(key), "periodic decode released {key:?}");
        }
    }

    fn item(id: u64, quantity: u32, slot: u32, container: u8) -> crate::inventory::ItemModel {
        crate::inventory::ItemModel {
            unique_id: Some(id),
            key: format!("template-{id}"),
            name: format!("Item {id}"),
            quantity,
            slot,
            container,
            ..crate::inventory::ItemModel::default()
        }
    }

    fn mail(id: u64) -> crate::mail::MailMessage {
        crate::mail::MailMessage {
            id,
            sender: "System".into(),
            subject: "Subject".into(),
            body: "Body".into(),
            gold: 10,
            items: vec![crate::mail::MailAttachment {
                name: Some("Item".into()),
                ..Default::default()
            }],
            operation: None,
            claimed: false,
            locked: false,
            read: false,
        }
    }

    #[test]
    fn mail_refresh_releases_only_proven_read_claim_and_delete_keys() {
        let mut pending = PendingOperations::default();
        for key in [
            PendingOperationKey::ReadMail(1),
            PendingOperationKey::ClaimMail(2),
            PendingOperationKey::DeleteMail(3),
            PendingOperationKey::ReadMail(4),
        ] {
            assert!(pending.try_begin(key));
        }
        let old = crate::mail::MailModel {
            mails: vec![mail(1), mail(2), mail(3), mail(4)],
            selected_id: None,
        };
        let mut read = mail(1);
        read.read = true;
        let mut claimed = mail(2);
        claimed.claimed = true;
        let new = crate::mail::MailModel {
            mails: vec![read, claimed, mail(4)],
            selected_id: None,
        };

        assert_eq!(reconcile_mail_refresh(&mut pending, &old, &new), 3);
        assert!(pending.contains(&PendingOperationKey::ReadMail(4)));
    }

    #[test]
    fn mail_result_releases_success_and_failure_without_refresh_guessing() {
        let mut pending = PendingOperations::default();
        let send = PendingOperationKey::SendMail {
            recipient: "Receiver".into(),
            message: "Hello".into(),
            gold: 10,
            attachment_unique_ids: vec![77],
        };
        let claim = PendingOperationKey::ClaimMail(7);
        assert!(pending.try_begin(send.clone()));
        assert!(pending.try_begin(claim.clone()));
        let old = crate::mail::MailModel {
            mails: vec![mail(7)],
            selected_id: None,
        };
        let new = crate::mail::MailModel {
            mails: vec![
                mail(7),
                crate::mail::MailMessage {
                    id: u64::MAX,
                    sender: String::new(),
                    subject: String::new(),
                    body: String::new(),
                    gold: 0,
                    items: Vec::new(),
                    operation: Some(crate::mail::MailOperationFeedback {
                        kind: crate::mail::MailOperationKind::Collect,
                        success: false,
                        mail_id: Some(7),
                    }),
                    claimed: false,
                    locked: true,
                    read: true,
                },
            ],
            selected_id: None,
        };
        assert_eq!(reconcile_mail_refresh(&mut pending, &old, &new), 1);
        assert!(pending.contains(&send));
        assert!(!pending.contains(&claim));

        let send_result = crate::mail::MailModel {
            mails: vec![crate::mail::MailMessage {
                id: u64::MAX,
                sender: String::new(),
                subject: String::new(),
                body: String::new(),
                gold: 0,
                items: Vec::new(),
                operation: Some(crate::mail::MailOperationFeedback {
                    kind: crate::mail::MailOperationKind::Send,
                    success: true,
                    mail_id: None,
                }),
                claimed: false,
                locked: true,
                read: true,
            }],
            selected_id: None,
        };
        assert_eq!(reconcile_mail_refresh(&mut pending, &old, &send_result), 1);
        assert!(pending.is_empty());
    }

    #[test]
    fn unchanged_inventory_snapshot_does_not_unlock_move_or_split() {
        let old = crate::inventory::InventoryModel {
            gold: 0,
            items: vec![item(10, 5, 0, 0)],
        };
        let mut pending = PendingOperations::default();
        let move_key = PendingOperationKey::Move {
            grid: "inventory".into(),
            unique_id: 10,
            from: 0,
            to: 1,
        };
        let split_key = PendingOperationKey::Split {
            grid: "inventory".into(),
            unique_id: 10,
            count: 2,
        };
        pending.try_begin(move_key.clone());
        pending.try_begin(split_key.clone());

        assert_eq!(reconcile_inventory_refresh(&mut pending, &old, &old), 0);
        assert!(pending.contains(&move_key));
        assert!(pending.contains(&split_key));

        let mut unaddressable = old.clone();
        unaddressable.items[0].unique_id = None;
        unaddressable.items[0].quantity = 3;
        assert_eq!(
            reconcile_inventory_refresh(&mut pending, &old, &unaddressable),
            0,
            "a template-only snapshot cannot prove which instance changed"
        );
        assert!(pending.contains(&move_key));
        assert!(pending.contains(&split_key));
    }

    #[test]
    fn exact_ack_or_nack_releases_only_the_correlated_inventory_key() {
        let mut pending = PendingOperations::default();
        let exact = PendingOperationKey::Split {
            grid: "inventory".into(),
            unique_id: 10,
            count: 2,
        };
        let other = PendingOperationKey::Split {
            grid: "inventory".into(),
            unique_id: 11,
            count: 2,
        };
        pending.try_begin(exact.clone());
        pending.try_begin(other.clone());
        let mut feedback = InventoryOperationFeedback::default();

        assert_eq!(
            apply_inventory_operation_ack(
                &mut pending,
                &mut feedback,
                InventoryOperationAck::Split {
                    grid: "Inventory".into(),
                    unique_id: 10,
                    count: 2,
                    success: false,
                }
            ),
            1
        );
        assert!(!pending.contains(&exact));
        assert!(pending.contains(&other));
        assert_eq!(
            feedback.last.as_ref().map(InventoryOperationAck::success),
            Some(false)
        );
    }

    #[test]
    fn sell_nack_releases_only_matching_item_and_count() {
        let mut pending = PendingOperations::default();
        let exact = PendingOperationKey::Sell {
            unique_id: 88,
            count: 2,
        };
        let other_count = PendingOperationKey::Sell {
            unique_id: 88,
            count: 1,
        };
        assert!(pending.try_begin(exact.clone()));
        assert!(pending.try_begin(other_count.clone()));
        let mut feedback = InventoryOperationFeedback::default();

        assert_eq!(
            apply_inventory_operation_ack(
                &mut pending,
                &mut feedback,
                InventoryOperationAck::Sell {
                    unique_id: 88,
                    count: 2,
                    success: false,
                },
            ),
            1
        );
        assert!(!pending.contains(&exact));
        assert!(pending.contains(&other_count));
        assert_eq!(
            feedback.last.as_ref().map(InventoryOperationAck::label),
            Some("Sell")
        );
    }

    #[test]
    fn inventory_refresh_releases_only_exact_drop_move_merge_split_and_sell_evidence() {
        let old = crate::inventory::InventoryModel {
            gold: 100,
            items: vec![
                item(10, 5, 0, 0),
                item(20, 3, 2, 0),
                item(21, 1, 3, 0),
                item(30, 4, 4, 0),
                item(40, 2, 5, 0),
            ],
        };
        let new = crate::inventory::InventoryModel {
            gold: 100,
            items: vec![
                item(10, 3, 0, 0),
                item(20, 1, 2, 0),
                item(21, 3, 3, 0),
                item(30, 3, 4, 0),
                item(40, 2, 8, 0),
            ],
        };
        let mut pending = PendingOperations::default();
        for key in [
            PendingOperationKey::Drop {
                unique_id: 10,
                count: 2,
                hero_inventory: false,
            },
            PendingOperationKey::Merge {
                grid_from: "inventory".into(),
                grid_to: "inventory".into(),
                id_from: 20,
                id_to: 21,
            },
            PendingOperationKey::Split {
                grid: "inventory".into(),
                unique_id: 30,
                count: 1,
            },
            PendingOperationKey::Move {
                grid: "inventory".into(),
                unique_id: 40,
                from: 5,
                to: 8,
            },
            PendingOperationKey::Sell {
                unique_id: 30,
                count: 1,
            },
        ] {
            pending.try_begin(key);
        }
        assert_eq!(reconcile_inventory_refresh(&mut pending, &old, &new), 5);
        assert!(pending.is_empty());
    }

    #[test]
    fn shop_refresh_proves_finite_stock_buy_but_not_unlimited_or_repair() {
        let good = |id, stock| crate::shop::ShopGood {
            unique_id: id,
            name: format!("Good {id}"),
            price: 10,
            count: 1,
            stock,
            panel_type: 0,
            ..crate::shop::ShopGood::default()
        };
        let old = crate::shop::ShopModel {
            goods: vec![good(1, 5), good(2, -1)],
            selected_id: None,
            ..Default::default()
        };
        let new = crate::shop::ShopModel {
            goods: vec![good(1, 3), good(2, -1)],
            selected_id: None,
            ..Default::default()
        };
        let mut pending = PendingOperations::default();
        let finite = PendingOperationKey::Buy {
            item_index: 1,
            count: 2,
        };
        let unlimited = PendingOperationKey::Buy {
            item_index: 2,
            count: 1,
        };
        let repair = PendingOperationKey::Repair(99);
        pending.try_begin(finite.clone());
        pending.try_begin(unlimited.clone());
        pending.try_begin(repair.clone());

        assert_eq!(reconcile_shop_refresh(&mut pending, &old, &new), 1);
        assert!(!pending.contains(&finite));
        assert!(pending.contains(&unlimited));
        assert!(pending.contains(&repair));
    }

    #[test]
    fn cash_game_shop_refresh_is_diagnostic_only_without_request_id() {
        let mut revisions = AuthoritativeModelRevisions::default();
        mark_authoritative_refresh(&mut revisions, AuthoritativeModelDomain::GameShop);
        assert_eq!(revisions.get(AuthoritativeModelDomain::GameShop), 1);
        mark_authoritative_refresh(&mut revisions, AuthoritativeModelDomain::GameShop);
        assert_eq!(revisions.get(AuthoritativeModelDomain::GameShop), 2);

        mark_authoritative_refresh(&mut revisions, AuthoritativeModelDomain::GameShop);
        assert_eq!(revisions.get(AuthoritativeModelDomain::GameShop), 3);
    }

    #[test]
    fn storage_ack_and_nack_release_only_the_correlatable_operation() {
        let mut pending = PendingOperations::default();
        let deposit = PendingOperationKey::StorageDeposit {
            unique_id: 55,
            from: 3,
            to: 9,
        };
        let other_deposit = PendingOperationKey::StorageDeposit {
            unique_id: 56,
            from: 4,
            to: 10,
        };
        let unlock = PendingOperationKey::StorageUnlock;
        let remove = PendingOperationKey::StorageRemovePassword;
        for key in [
            deposit.clone(),
            other_deposit.clone(),
            unlock.clone(),
            remove.clone(),
        ] {
            assert!(pending.try_begin(key));
        }

        assert_eq!(
            apply_storage_operation_ack(
                &mut pending,
                &StorageOperationAck::Deposit {
                    from: 3,
                    to: 9,
                    success: false,
                },
            ),
            1
        );
        assert!(!pending.contains(&deposit));
        assert!(pending.contains(&other_deposit));

        assert_eq!(
            apply_storage_operation_ack(
                &mut pending,
                &StorageOperationAck::Unlock { success: false },
            ),
            1
        );
        assert!(!pending.contains(&unlock));
        assert!(pending.contains(&remove));
        assert!(!StorageOperationAck::Unlock { success: false }.success());
    }

    #[test]
    fn storage_refresh_requires_exact_transfer_or_metadata_transition() {
        let inventory = crate::inventory::InventoryModel {
            gold: 0,
            items: vec![item(60, 1, 7, 0)],
        };
        let old = crate::storage::StorageModel {
            size: 30,
            has_password: true,
            unlocked: false,
            items: vec![item(60, 1, 2, 4)],
            ..Default::default()
        };
        let new = crate::storage::StorageModel {
            size: 42,
            has_password: true,
            unlocked: true,
            has_expanded: true,
            items: Vec::new(),
            ..Default::default()
        };
        let mut pending = PendingOperations::default();
        for key in [
            PendingOperationKey::StorageWithdraw {
                unique_id: 60,
                from: 2,
                to: 7,
            },
            PendingOperationKey::StorageUnlock,
            PendingOperationKey::StorageExpand,
            PendingOperationKey::StorageRemovePassword,
        ] {
            pending.try_begin(key);
        }
        assert_eq!(
            reconcile_storage_refresh(&mut pending, &inventory, &old, &new),
            3
        );
        assert!(pending.contains(&PendingOperationKey::StorageRemovePassword));
    }

    #[cfg(feature = "native-ui")]
    #[test]
    fn quest_refresh_releases_only_the_quest_with_a_proven_lifecycle_change() {
        let quest = |quest_index, status| crate::quest_model::Quest {
            quest_index,
            accept_npc_index: Some(1),
            finish_npc_index: Some(2),
            title: format!("Quest {quest_index}"),
            npc_name: None,
            status,
            objectives: Vec::new(),
            rewards: Vec::new(),
            unknown_text: None,
        };
        let old = crate::quest_model::QuestTracker {
            active_quests: vec![quest(20, crate::quest_model::QuestStatus::ReadyToTurnIn)],
        };
        let new = crate::quest_model::QuestTracker {
            active_quests: vec![quest(10, crate::quest_model::QuestStatus::InProgress)],
        };
        let mut pending = PendingOperations::default();
        let accept = PendingOperationKey::QuestAccept {
            npc_index: 1,
            quest_index: 10,
        };
        let finish = PendingOperationKey::QuestFinish {
            quest_index: 20,
            selected_item_index: -1,
        };
        let abandon = PendingOperationKey::QuestAbandon { quest_index: 40 };
        let unrelated = PendingOperationKey::QuestAccept {
            npc_index: 1,
            quest_index: 30,
        };
        pending.try_begin(accept.clone());
        pending.try_begin(finish.clone());
        pending.try_begin(abandon.clone());
        pending.try_begin(unrelated.clone());

        assert_eq!(reconcile_quest_refresh(&mut pending, &old, &new), 2);
        assert!(!pending.contains(&accept));
        assert!(!pending.contains(&finish));
        assert!(pending.contains(&abandon));
        assert!(pending.contains(&unrelated));
    }

    #[cfg(feature = "native-ui")]
    #[test]
    fn quest_refresh_releases_abandon_only_after_authoritative_lifecycle_change() {
        let quest = |status| crate::quest_model::Quest {
            quest_index: 40,
            accept_npc_index: Some(1),
            finish_npc_index: Some(2),
            title: "Quest 40".to_owned(),
            npc_name: None,
            status,
            objectives: Vec::new(),
            rewards: Vec::new(),
            unknown_text: None,
        };
        let old = crate::quest_model::QuestTracker {
            active_quests: vec![quest(crate::quest_model::QuestStatus::InProgress)],
        };
        let unchanged = old.clone();
        let aborted = crate::quest_model::QuestTracker {
            active_quests: vec![quest(crate::quest_model::QuestStatus::Aborted)],
        };
        let key = PendingOperationKey::QuestAbandon { quest_index: 40 };
        let mut pending = PendingOperations::default();
        assert!(pending.try_begin(key.clone()));
        assert_eq!(reconcile_quest_refresh(&mut pending, &old, &unchanged), 0);
        assert!(pending.contains(&key));
        assert_eq!(reconcile_quest_refresh(&mut pending, &old, &aborted), 1);
        assert!(!pending.contains(&key));
    }

    #[test]
    fn session_reset_clears_pending_without_claiming_success() {
        let mut pending = PendingOperations::default();
        let mut revisions = AuthoritativeModelRevisions::default();
        let mut reset = SessionResetRevision::default();
        pending.try_begin(PendingOperationKey::StorageExpand);
        revisions.advance(AuthoritativeModelDomain::Storage);

        request_session_reset(&mut reset, &mut revisions, &mut pending);
        assert!(pending.is_empty());
        assert_eq!(reset.0, 1);
        assert_eq!(revisions.get(AuthoritativeModelDomain::Storage), 0);
        assert_eq!(revisions.session_generation(), 1);
    }

    #[test]
    fn preserving_reset_keeps_only_exact_game_shop_key_for_receipt_ingest() {
        let receipt = crate::game_shop::GameShopReceipt {
            protocol: "nativeGameShopReceiptV1".to_owned(),
            request_id: "gs-preserve".to_owned(),
            success: false,
            g_index: 31,
            quantity: 2,
            price_type: 1,
            new_stock_level: None,
            mail_id: None,
            code: Some(crate::game_shop::GameShopFailureCode::InsufficientCurrency),
        };
        let mut pending = PendingOperations::default();
        assert!(pending.try_begin(PendingOperationKey::DeleteMail(9)));
        assert!(pending.try_begin(PendingOperationKey::GameShop(receipt.request_id.clone())));
        let mut reset = SessionResetRevision::default();
        let mut revisions = AuthoritativeModelRevisions::default();
        let mut preservation = SessionResetGameShopPreservation::default();

        assert!(request_session_reset_preserving_exact_game_shop_receipt(
            &mut reset,
            &mut revisions,
            &mut pending,
            &mut preservation,
            receipt.clone(),
        ));
        assert_eq!(reset.0, 1);
        assert_eq!(pending.len(), 1);
        assert!(pending.contains(&PendingOperationKey::GameShop(receipt.request_id.clone())));
        assert_eq!(preservation.receipt_for(1), Some(&receipt));
        assert!(preservation.receipt_for(2).is_none());
        assert!(!preservation.clear_if_consumed(1));
        assert!(!preservation.mark_consumed(2, &receipt.request_id));
        assert!(preservation.mark_consumed(1, &receipt.request_id));
        assert!(!preservation.clear_if_consumed(2));
        assert!(preservation.clear_if_consumed(1));
        assert!(preservation.receipt_for(1).is_none());

        preservation.set(7, receipt.clone());
        assert!(!preservation.clear_if_stale(7));
        assert!(preservation.clear_if_stale(8));
        assert!(preservation.receipt_for(7).is_none());
    }

    #[test]
    fn pending_operations_are_bounded_and_fail_closed() {
        let mut pending = PendingOperations::default();
        for id in 0..MAX_PENDING_OPERATIONS as u64 {
            assert!(pending.try_begin(PendingOperationKey::ClaimMail(id)));
        }
        assert!(!pending.try_begin(PendingOperationKey::ClaimMail(
            MAX_PENDING_OPERATIONS as u64
        )));
        assert_eq!(pending.len(), MAX_PENDING_OPERATIONS);
    }
}

#[cfg(all(test, feature = "native-ui"))]
mod native_ui_tests {
    use bevy::prelude::*;

    use crate::crystal_ui::overlays::{
        NativePlayerUiIntent, NativePlayerUiIntentQueue, NativePlayerUiState, UiEffectQueue,
    };
    use crate::native_shell::{NativeUiIntent, NativeUiIntentQueue};
    use crate::quest_model::{
        CombatTargetModel, CombatTargetUpdate, GroundPickupModel, NearbyNpcModel, NpcDialogModel,
        Quest, QuestStatus, QuestTracker,
    };
    use crate::quest_ui::{NpcDialogNav, QuestUiIntent, QuestUiIntentQueue, QuestUiState};

    use super::*;

    #[test]
    fn account_a_to_b_reset_clears_ui_tracking_targets_and_unforwarded_intents() {
        let mut app = App::new();
        app.init_resource::<SessionResetRevision>()
            .init_resource::<SessionResetGameShopPreservation>()
            .init_resource::<OverlayResetTracker>()
            .init_resource::<QuestResetTracker>()
            .init_resource::<crate::crystal_ui::overlays::MailComposeUi>()
            .init_resource::<PendingOperations>()
            .init_resource::<InventoryOperationFeedback>()
            .init_resource::<NativePlayerUiState>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<NativeUiIntentQueue>()
            .init_resource::<UiEffectQueue>()
            .init_resource::<QuestTracker>()
            .init_resource::<NpcDialogModel>()
            .init_resource::<NearbyNpcModel>()
            .init_resource::<CombatTargetModel>()
            .init_resource::<GroundPickupModel>()
            .init_resource::<QuestUiState>()
            .init_resource::<NpcDialogNav>()
            .init_resource::<QuestUiIntentQueue>()
            .add_systems(
                Update,
                (apply_overlay_session_reset, apply_quest_session_reset).chain(),
            );

        {
            let mut ui = app.world_mut().resource_mut::<NativePlayerUiState>();
            ui.chat_draft = "account-a draft".to_owned();
            ui.inspect = Some(crate::crystal_ui::overlays::ItemInspect {
                container: 0,
                slot: 1,
                key: "10".to_owned(),
                name: "A item".to_owned(),
                quantity: 1,
            });
            ui.core.panel = mir2_ui_core::state::UiPanel::Inventory;
        }
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .push_intent(NativePlayerUiIntent::Chat {
                message: "A-only".to_owned(),
            });
        app.world_mut()
            .resource_mut::<NativeUiIntentQueue>()
            .push(NativeUiIntent::Logout);
        app.world_mut().resource_mut::<UiEffectQueue>().push(
            mir2_ui_core::effect::UiEffect::GatewayCommand(
                mir2_ui_core::effect::GatewayCommand::Logout,
            ),
        );
        app.world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .push_intent(QuestUiIntent::AttackTarget { object_id: 77 });
        app.world_mut()
            .resource_mut::<PendingOperations>()
            .try_begin(PendingOperationKey::ClaimMail(5));
        {
            let mut state = app.world_mut().resource_mut::<QuestUiState>();
            state.selected_quest_index = Some(7);
            state.tracking_quest_index = Some(7);
            state.set_feedback("A quest", false);
        }
        app.world_mut()
            .resource_mut::<QuestTracker>()
            .active_quests
            .push(Quest {
                quest_index: 7,
                accept_npc_index: Some(1),
                finish_npc_index: Some(2),
                title: "A quest".to_owned(),
                npc_name: Some("A NPC".to_owned()),
                status: QuestStatus::InProgress,
                objectives: Vec::new(),
                rewards: Vec::new(),
                unknown_text: None,
            });
        app.world_mut()
            .resource_mut::<CombatTargetModel>()
            .apply(CombatTargetUpdate {
                object_id: 77,
                name: "A target".to_owned(),
                hp: 10,
                max_hp: 20,
                is_player: false,
            });

        app.world_mut()
            .resource_mut::<SessionResetRevision>()
            .request();
        app.update();

        assert_eq!(
            *app.world().resource::<NativePlayerUiState>(),
            NativePlayerUiState::default()
        );
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .is_empty());
        assert!(app.world().resource::<NativeUiIntentQueue>().is_empty());
        assert_eq!(app.world().resource::<UiEffectQueue>().len(), 0);
        assert!(app.world().resource::<PendingOperations>().is_empty());
        assert!(app
            .world()
            .resource::<QuestTracker>()
            .active_quests
            .is_empty());
        assert!(app.world().resource::<CombatTargetModel>().target.is_none());
        assert_eq!(
            app.world().resource::<QuestUiState>().tracking_quest_index,
            None
        );
        assert!(app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .drain_intents()
            .is_empty());
    }

    #[test]
    fn dangerous_intent_queue_blocks_double_press_and_distinct_mail_ids() {
        let mut pending = PendingOperations::default();
        let mut queue = NativePlayerUiIntentQueue::default();

        assert!(queue.push_pending_intent(
            &mut pending,
            NativePlayerUiIntent::ClaimMail { mail_id: 10 },
        ));
        assert!(!queue.push_pending_intent(
            &mut pending,
            NativePlayerUiIntent::ClaimMail { mail_id: 10 },
        ));
        assert!(!queue.push_pending_intent(
            &mut pending,
            NativePlayerUiIntent::ClaimMail { mail_id: 11 },
        ));

        let first_send = NativePlayerUiIntent::SendMail {
            recipient: "A".into(),
            message: "one".into(),
            gold: 1,
            attachment_unique_ids: vec![7],
        };
        let second_send = NativePlayerUiIntent::SendMail {
            recipient: "B".into(),
            message: "two".into(),
            gold: 2,
            attachment_unique_ids: vec![8],
        };
        assert!(!queue.push_pending_intent(&mut pending, first_send.clone()));
        assert_eq!(queue.drain_intents().len(), 1);

        let mut send_pending = PendingOperations::default();
        let mut send_queue = NativePlayerUiIntentQueue::default();
        assert!(send_queue.push_pending_intent(&mut send_pending, first_send));
        assert!(!send_queue.push_pending_intent(&mut send_pending, second_send));
        assert!(!send_queue.push_pending_intent(
            &mut send_pending,
            NativePlayerUiIntent::ClaimMail { mail_id: 12 },
        ));
        assert_eq!(send_queue.drain_intents().len(), 1);
    }

    #[test]
    fn leaving_in_game_requests_session_reset_without_a_timeout() {
        let mut app = App::new();
        app.insert_resource(crate::native_shell::NativeShellModel {
            screen: crate::native_shell::NativeShellScreen::InGame,
            ..Default::default()
        })
        .init_resource::<NativeSessionBoundaryTracker>()
        .init_resource::<SessionResetRevision>()
        .init_resource::<AuthoritativeModelRevisions>()
        .init_resource::<PendingOperations>()
        .add_systems(Update, observe_native_session_boundary);
        app.world_mut()
            .resource_mut::<PendingOperations>()
            .try_begin(PendingOperationKey::DeleteMail(9));
        app.update();

        app.world_mut()
            .resource_mut::<crate::native_shell::NativeShellModel>()
            .screen = crate::native_shell::NativeShellScreen::Login;
        app.update();

        assert_eq!(app.world().resource::<SessionResetRevision>().0, 1);
        assert!(app.world().resource::<PendingOperations>().is_empty());
    }

    #[test]
    fn shell_boundary_does_not_duplicate_an_already_ingested_session_reset() {
        let mut app = App::new();
        app.insert_resource(crate::native_shell::NativeShellModel {
            screen: crate::native_shell::NativeShellScreen::InGame,
            ..Default::default()
        })
        .init_resource::<NativeSessionBoundaryTracker>()
        .init_resource::<SessionResetRevision>()
        .init_resource::<AuthoritativeModelRevisions>()
        .init_resource::<PendingOperations>()
        .add_systems(Update, observe_native_session_boundary);
        app.update();

        app.world_mut()
            .resource_mut::<SessionResetRevision>()
            .request();
        app.world_mut()
            .resource_mut::<crate::native_shell::NativeShellModel>()
            .screen = crate::native_shell::NativeShellScreen::ConnectionLost;
        app.update();

        assert_eq!(
            app.world().resource::<SessionResetRevision>().0,
            1,
            "the shell transition must consume, not duplicate, the runtime boundary"
        );
    }
}
