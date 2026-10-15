//! Local processing evidence only: no transport receipt, persistence or delivery claim.
use mir2_protocol::{ClientPacket, ServerPacket};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcGoldBuyRequest {
    pub item_index: u64,
    pub count: u16,
    pub panel_type: u8,
}
impl NpcGoldBuyRequest {
    pub(super) fn packet(self) -> ClientPacket {
        ClientPacket::BuyItem { item_index: self.item_index, count: self.count, panel_type: self.panel_type }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcGoldBuyRejection {
    InvalidRequest, PlayerDead, ServiceUnavailable, UnsupportedService,
    UnknownGood, InvalidQuantity, InsufficientGold, ClockUnavailable, InvalidDelivery,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NpcGoldBuyProcessingOutcome {
    /// Both live wallet and inventory writes completed. The UID names the incoming
    /// GainedItem delta; a fully merged purchase need not retain that UID in inventory.
    Committed { request: NpcGoldBuyRequest, gold_spent: u32, incoming_unique_id: u64 },
    Rejected { request: NpcGoldBuyRequest, reason: NpcGoldBuyRejection },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcGoldBuyBeforeExecution { NotAuthenticated, NotInGame, UnsupportedRuntime }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NpcGoldBuyProcessingError {
    BeforeExecution(NpcGoldBuyBeforeExecution),
    /// Known economic outcome survives a later pipeline failure; packets may not
    /// have been delivered and the failure is not an economic rejection.
    PostProcessing { outcome: NpcGoldBuyProcessingOutcome, detail: String },
    Unknown { detail: String },
}
#[derive(Debug, Clone)]
pub struct NpcGoldBuyProcessingExecution {
    pub packets: Vec<ServerPacket>,
    pub outcome: NpcGoldBuyProcessingOutcome,
}
pub(super) fn finish_npc_gold_buy_processing(
    packets: Result<Vec<ServerPacket>, String>, outcome: Option<NpcGoldBuyProcessingOutcome>,
) -> Result<NpcGoldBuyProcessingExecution, NpcGoldBuyProcessingError> {
    match (packets, outcome) {
        (Ok(packets), Some(outcome)) => Ok(NpcGoldBuyProcessingExecution { packets, outcome }),
        (Err(detail), Some(outcome)) => Err(NpcGoldBuyProcessingError::PostProcessing { outcome, detail }),
        (Err(detail), None) => Err(NpcGoldBuyProcessingError::Unknown { detail }),
        (Ok(_), None) => Err(NpcGoldBuyProcessingError::Unknown {
            detail: "purchase processing did not return an authoritative outcome".into(),
        }),
    }
}

#[cfg(test)]
#[path = "npc_gold_buy_outcome_tests.rs"]
mod tests;
