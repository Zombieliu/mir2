//! Durable NPC purchase actions through the actual, fenced owner runtime.
//! No action is translated into a generic BuyItem or retried on another owner.
use mir2_simulation::{NpcPurchaseDurableError, NpcPurchaseOwnerAction,
    NpcPurchaseOwnerReply, NpcPurchaseProducer, NpcPurchaseReceipt, WorldCommandExecution,
    WorldCommandKind, WorldCommandOutcome, WorldSnapshot, ZoneRuntimeHandle};
use sha2::{Digest, Sha256};
use crate::routing::ZoneOwnerLease;

#[derive(Debug, Clone)]
pub struct NpcPurchaseOwnerRouteExecution {
    pub reply: NpcPurchaseOwnerReply,
    pub execution: WorldCommandExecution,
    /// A full projection and its authority are captured in one owner turn.
    /// Neither is supplied on a partial or stale checkpoint publication.
    pub snapshot: Option<WorldSnapshot>,
    pub authority: Option<NpcPurchaseProducer>,
    pub(crate) checkpoint: Option<mir2_simulation::CharacterSaveRecord>,
}

pub(crate) fn before(detail: impl Into<String>) -> NpcPurchaseDurableError {
    NpcPurchaseDurableError::BeforeExecution { detail: detail.into() }
}
pub(crate) fn failure(detail: impl Into<String>, receipt: Option<NpcPurchaseReceipt>) -> NpcPurchaseDurableError {
    match receipt {
        Some(receipt) => NpcPurchaseDurableError::PostCommit { receipt, detail: detail.into() },
        None => NpcPurchaseDurableError::Unknown { detail: detail.into() },
    }
}
pub(crate) fn terminal_receipt(reply: &NpcPurchaseOwnerReply) -> Option<NpcPurchaseReceipt> {
    match reply {
        NpcPurchaseOwnerReply::Purchase { receipt, .. } |
        NpcPurchaseOwnerReply::Recovery { receipt: Some(receipt) } => Some(receipt.clone()),
        _ => None,
    }
}
pub(crate) fn owner_epoch(lease: &ZoneOwnerLease) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"mir2/npc-purchase/validated-owner-epoch/v1\0");
    for value in [lease.zone_id().as_str(), lease.owner_id()] {
        hash.update((value.len() as u64).to_be_bytes());
        hash.update(value.as_bytes());
    }
    hash.update(lease.fencing_token().to_be_bytes());
    hash.finalize().into()
}
pub(crate) fn action_kind(action: NpcPurchaseOwnerAction) -> WorldCommandKind {
    WorldCommandKind::NpcPurchaseOwner(match action {
        NpcPurchaseOwnerAction::Begin => "begin",
        NpcPurchaseOwnerAction::Quote { .. } => "quote",
        NpcPurchaseOwnerAction::Query { .. } => "query",
        NpcPurchaseOwnerAction::Purchase { .. } => "purchase",
    })
}

/// The caller holds a validated lease guard for this entire function and, for
/// Hosted/RPC routes, the actual runtime mutex and Zone mutation gate.
pub(crate) fn execute_owner(
    runtime: &mut ZoneRuntimeHandle, lease: &ZoneOwnerLease,
    authenticated: bool, action: NpcPurchaseOwnerAction,
) -> Result<NpcPurchaseOwnerRouteExecution, NpcPurchaseDurableError> {
    if !authenticated { return Err(before("NPC purchase owner action requires authentication")); }
    if !runtime.supports_durable_npc_purchase_owner() {
        return Err(before("durable NPC purchase owner capability is unavailable"));
    }
    let epoch = owner_epoch(lease);
    let mut known = None;
    let result = crate::session::catch_gateway_panic("durable NPC purchase owner route", || {
        let processed = match runtime.execute_npc_purchase_owner(authenticated, epoch, action) {
            Ok(processed) => processed,
            Err(error) => {
                if let NpcPurchaseDurableError::PostCommit { receipt, .. } = &error { known = Some(receipt.clone()); }
                return Err(error);
            }
        };
        known = terminal_receipt(&processed.reply);
        // Check after all shared tail work, rather than trusting a pre-tail
        // producer or a local tick. The second check refuses external changes
        // between authority verification and projection capture.
        let mut authority = runtime.npc_purchase_owner_authority(epoch);
        let mut snapshot = authority.as_ref().map(|_| runtime.world_snapshot());
        if authority != runtime.npc_purchase_owner_authority(epoch) {
            authority = None; snapshot = None;
        }
        let checkpoint = runtime.npc_purchase_owner_checkpoint(epoch);
        let active_identity = runtime.active_identity();
        let packet_count = processed.packets.len();
        let snapshot_tick = snapshot.as_ref().map(|snapshot| snapshot.tick).unwrap_or(0);
        Ok(NpcPurchaseOwnerRouteExecution {
            reply: processed.reply, authority, snapshot, checkpoint,
            execution: WorldCommandExecution { packets: processed.packets,
                outcome: WorldCommandOutcome { command_kind: action_kind(action), packet_count, snapshot_tick, active_identity },
                game_shop_purchase_outcome: None },
        })
    });
    result.unwrap_or_else(|detail| Err(failure(detail, known)))
}
