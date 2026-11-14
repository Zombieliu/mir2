//! Explicit local owner-route evidence. This is not a network receipt or replay contract.
use std::fmt;

use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{
    NpcGoldBuyProcessingError, NpcGoldBuyProcessingExecution, NpcGoldBuyProcessingOutcome,
    NpcGoldBuyRequest, WorldCommand, WorldCommandExecution,
};

#[derive(Debug, Clone)]
pub struct NpcGoldBuyRouteExecution {
    pub execution: WorldCommandExecution,
    pub processing_outcome: NpcGoldBuyProcessingOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NpcGoldBuyRouteError {
    BeforeExecution { detail: String },
    Processing(NpcGoldBuyProcessingError),
}

impl fmt::Display for NpcGoldBuyRouteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BeforeExecution { detail } => formatter.write_str(detail),
            Self::Processing(error) => write!(formatter, "NPC gold purchase processing: {error:?}"),
        }
    }
}

impl std::error::Error for NpcGoldBuyRouteError {}

pub(crate) fn npc_gold_buy_command(request: NpcGoldBuyRequest) -> WorldCommand {
    WorldCommand::ClientPacket(ClientPacket::BuyItem {
        item_index: request.item_index,
        count: request.count,
        panel_type: request.panel_type,
    })
}

pub(crate) fn npc_gold_buy_route_failure(
    detail: String,
    outcome: Option<NpcGoldBuyProcessingOutcome>,
) -> NpcGoldBuyRouteError {
    NpcGoldBuyRouteError::Processing(match outcome {
        Some(outcome) => NpcGoldBuyProcessingError::PostProcessing { outcome, detail },
        None => NpcGoldBuyProcessingError::Unknown { detail },
    })
}

pub(crate) fn finish_npc_gold_buy_packets(
    packets: Result<Vec<ServerPacket>, String>,
    outcome: Option<NpcGoldBuyProcessingOutcome>,
) -> Result<NpcGoldBuyProcessingExecution, NpcGoldBuyProcessingError> {
    match (packets, outcome) {
        (Ok(packets), Some(outcome)) => Ok(NpcGoldBuyProcessingExecution { packets, outcome }),
        (Err(detail), Some(outcome)) => {
            Err(NpcGoldBuyProcessingError::PostProcessing { outcome, detail })
        }
        (Err(detail), None) => Err(NpcGoldBuyProcessingError::Unknown { detail }),
        (Ok(_), None) => Err(NpcGoldBuyProcessingError::Unknown {
            detail: "owner pipeline did not return an authoritative NPC purchase outcome".into(),
        }),
    }
}
