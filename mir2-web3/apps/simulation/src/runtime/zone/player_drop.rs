//! Private world custody for ordinary player drops. These types never enter client snapshots.
use super::{SessionId, ZoneKey, ZoneOutbound};
use crate::player_drop::PlayerDropWorldIntent;
use mir2_protocol::Point;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerDropEffectIdentity {
    world_effect_id: String,
    intent_digest: String,
    expires_at_unix_ms: u64,
}
impl PlayerDropEffectIdentity {
    pub fn world_effect_id(&self) -> &str {
        &self.world_effect_id
    }
    pub fn intent_digest(&self) -> &str {
        &self.intent_digest
    }
    pub fn expires_at_unix_ms(&self) -> u64 {
        self.expires_at_unix_ms
    }
    pub(crate) fn from_intent(intent: &PlayerDropWorldIntent) -> Self {
        Self {
            world_effect_id: intent.world_effect_id().to_owned(),
            intent_digest: intent.intent_digest().to_owned(),
            expires_at_unix_ms: intent.expires_at_unix_ms(),
        }
    }
    pub(crate) fn pickup_key(&self) -> String {
        format!("player-drop-pickup:{}", self.world_effect_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerDropPlacementKind {
    Gold,
    Item,
}
impl PlayerDropPlacementKind {
    pub(crate) fn range(self) -> i32 {
        match self {
            Self::Gold => 5,
            Self::Item => 1,
        }
    }
}

/// A retained slot, not an asset-minting capability. PostgreSQL work is performed
/// after this value has been returned and the Zone writer has been released.
/// It is authenticated by the frozen committed journal result, and is never
/// imported from or exported as durable business-world checkpoint state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerDropPlacementReservation {
    pub(crate) zone: ZoneKey,
    pub(crate) world_effect_id: String,
    pub(crate) session_id: SessionId,
    pub(crate) player_object_id: u32,
    pub(crate) origin: Point,
    pub(crate) position: Point,
    pub(crate) kind: PlayerDropPlacementKind,
    pub(crate) reserved_at_unix_ms: u64,
}
impl PlayerDropPlacementReservation {
    pub fn zone(&self) -> &ZoneKey {
        &self.zone
    }
    pub fn world_effect_id(&self) -> &str {
        &self.world_effect_id
    }
    pub fn position(&self) -> &Point {
        &self.position
    }
    pub fn kind(&self) -> PlayerDropPlacementKind {
        self.kind
    }
    pub fn reserved_at_unix_ms(&self) -> u64 {
        self.reserved_at_unix_ms
    }
    pub fn origin(&self) -> &Point {
        &self.origin
    }
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }
    pub fn player_object_id(&self) -> u32 {
        self.player_object_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerDropCustodyState {
    Live {
        object_id: u32,
        drop_generation: u64,
    },
    Consumed,
    Expired,
    Destroyed,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlayerDropEffectRecord {
    pub intent: PlayerDropWorldIntent,
    pub state: PlayerDropCustodyState,
}
#[derive(Debug)]
pub struct PlayerDropApplyResult {
    pub object_id: Option<u32>,
    pub replay: bool,
    pub outbounds: Vec<ZoneOutbound>,
}
