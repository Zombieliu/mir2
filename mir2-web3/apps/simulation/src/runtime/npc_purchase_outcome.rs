//! Authoritative local processing evidence for all NPC purchase branches.
//! These results are not durable, correlated network receipts or delivery ACKs.
use mir2_protocol::{ClientPacket, ServerPacket};
use super::npc_gold_buy_outcome::{NpcGoldBuyRequest, NpcGoldBuyRejection};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcPurchaseRequest {
    pub item_index: u64,
    pub count: u16,
    pub panel_type: u8,
}
impl NpcPurchaseRequest {
    pub(super) fn packet(self) -> ClientPacket {
        ClientPacket::BuyItem { item_index: self.item_index, count: self.count, panel_type: self.panel_type }
    }
}
impl From<NpcGoldBuyRequest> for NpcPurchaseRequest {
    fn from(request: NpcGoldBuyRequest) -> Self {
        Self { item_index: request.item_index, count: request.count, panel_type: request.panel_type }
    }
}
impl From<NpcPurchaseRequest> for NpcGoldBuyRequest {
    fn from(request: NpcPurchaseRequest) -> Self {
        Self { item_index: request.item_index, count: request.count, panel_type: request.panel_type }
    }
}

/// Determined by the actual server service, independently of the goods source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcPurchaseCurrency { Gold, Pearls }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcPurchaseSource { Trade, BuyBack, Used }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcPurchaseRejection {
    InvalidRequest, PlayerDead, ServiceUnavailable, UnsupportedService,
    UnknownGood, InvalidQuantity, InsufficientCurrency, ClockUnavailable, InvalidDelivery,
}
impl From<NpcGoldBuyRejection> for NpcPurchaseRejection {
    fn from(reason: NpcGoldBuyRejection) -> Self {
        match reason {
            NpcGoldBuyRejection::InvalidRequest => Self::InvalidRequest,
            NpcGoldBuyRejection::PlayerDead => Self::PlayerDead,
            NpcGoldBuyRejection::ServiceUnavailable => Self::ServiceUnavailable,
            NpcGoldBuyRejection::UnsupportedService => Self::UnsupportedService,
            NpcGoldBuyRejection::UnknownGood => Self::UnknownGood,
            NpcGoldBuyRejection::InvalidQuantity => Self::InvalidQuantity,
            NpcGoldBuyRejection::InsufficientGold => Self::InsufficientCurrency,
            NpcGoldBuyRejection::ClockUnavailable => Self::ClockUnavailable,
            NpcGoldBuyRejection::InvalidDelivery => Self::InvalidDelivery,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NpcPurchaseProcessingOutcome {
    /// Wallet, inventory and any resale stock updates completed in memory.
    /// `admitted_count` is the actual delivery count after the legacy resale clamp.
    /// The UID identifies the incoming delta; a fully merged delivery can consume it.
    Committed {
        request: NpcPurchaseRequest,
        currency: NpcPurchaseCurrency,
        source: NpcPurchaseSource,
        charged: u32,
        admitted_count: u16,
        incoming_unique_id: u64,
    },
    Rejected { request: NpcPurchaseRequest, reason: NpcPurchaseRejection },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcPurchaseBeforeExecution { NotAuthenticated, NotInGame, UnsupportedRuntime }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NpcPurchaseProcessingError {
    BeforeExecution(NpcPurchaseBeforeExecution),
    PostProcessing { outcome: NpcPurchaseProcessingOutcome, detail: String },
    Unknown { detail: String },
}
#[derive(Debug, Clone)]
pub struct NpcPurchaseProcessingExecution {
    pub packets: Vec<ServerPacket>,
    pub outcome: NpcPurchaseProcessingOutcome,
}
pub(super) fn finish_npc_purchase_processing(
    packets: Result<Vec<ServerPacket>, String>, outcome: Option<NpcPurchaseProcessingOutcome>,
) -> Result<NpcPurchaseProcessingExecution, NpcPurchaseProcessingError> {
    match (packets, outcome) {
        (Ok(packets), Some(outcome)) => Ok(NpcPurchaseProcessingExecution { packets, outcome }),
        (Err(detail), Some(outcome)) => Err(NpcPurchaseProcessingError::PostProcessing { outcome, detail }),
        (Err(detail), None) => Err(NpcPurchaseProcessingError::Unknown { detail }),
        (Ok(_), None) => Err(NpcPurchaseProcessingError::Unknown {
            detail: "purchase processing did not return an authoritative outcome".into(),
        }),
    }
}

#[cfg(test)]
#[path = "npc_purchase_outcome_tests.rs"]
mod tests;
