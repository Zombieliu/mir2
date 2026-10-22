//! Read-only material conversion presentation and local input suppression.
//!
//! A protocol advertisement is display data, never owner authority. Only the
//! physical host may publish this model for its current actor. In particular,
//! `Committed` must be published after the independent complete gameplay apply
//! proof; receiving a receipt or observing inventory is insufficient.
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const QUOTE_TTL_MS: i64 = 30_000;
const MAX_CONFIRMED_QUOTES: usize = 4096;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HuntingMaterialPhase {
    #[default]
    Unknown,
    Ready,
    Quoting,
    Quoted,
    Pending,
    Committed,
    Rejected,
}

/// Stable host-owned display partition. This carries no identity authority;
/// the physical host still verifies the actor and fence before any operation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HuntingMaterialActorScope {
    pub realm: String,
    pub account_id: String,
    pub character_index: i32,
}
impl HuntingMaterialActorScope {
    pub fn new(
        realm: String,
        account_id: String,
        character_index: i32,
    ) -> Result<Self, &'static str> {
        let scope = Self {
            realm,
            account_id,
            character_index,
        };
        scope
            .is_valid()
            .then_some(scope)
            .ok_or("Invalid material display actor")
    }
    pub fn is_valid(&self) -> bool {
        let text = |value: &str, max: usize| {
            !value.is_empty()
                && value.len() <= max
                && value.trim() == value
                && !value.chars().any(char::is_control)
        };
        text(&self.realm, 128) && text(&self.account_id, 160) && self.character_index >= 0
    }
}

/// Host-owned presentation only. Absent server values stay absent; this view
/// contains neither an inferred wallet nor any inventory/application authority.
#[derive(Debug, Clone, Default, PartialEq, Eq, Resource, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct HuntingMaterialModel {
    #[serde(default)]
    pub actor_scope: Option<HuntingMaterialActorScope>,
    /// The independent huntingMaterialOwner protocol v1 advertisement.
    pub protocol_v1: bool,
    pub phase: HuntingMaterialPhase,
    pub quote_id: Option<String>,
    pub unique_id: Option<u64>,
    pub input_count: Option<u64>,
    pub quantity: Option<u64>,
    pub unit_gold: Option<u64>,
    pub gold_equivalent: Option<u64>,
    pub fee_unbound_gold: Option<u64>,
    pub license_atoms_debit: Option<u64>,
    pub valuation_version: Option<String>,
    pub created_at_ms: Option<i64>,
    pub expires_at_ms: Option<i64>,
    pub reload_required: bool,
    pub notice: Option<String>,
}

fn valid_quote_id(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.bytes().any(|byte| byte != b'0')
}

fn valid_intent(unique_id: u64, quantity: u64) -> bool {
    unique_id > 0 && quantity > 0 && quantity <= u64::from(u32::MAX)
}

impl HuntingMaterialModel {
    fn has_display_actor(&self) -> bool {
        self.actor_scope
            .as_ref()
            .is_some_and(HuntingMaterialActorScope::is_valid)
    }
    /// Conservative display checks only; the final physical send still has to
    /// revalidate the complete source capability, actor, quote and deadline.
    fn valid_quote(&self, unique_id: u64, quantity: u64, now_ms: i64) -> bool {
        if !valid_intent(unique_id, quantity)
            || self.unique_id != Some(unique_id)
            || self.quantity != Some(quantity)
            || !self.quote_id.as_deref().is_some_and(valid_quote_id)
        {
            return false;
        }
        let (Some(count), Some(unit), Some(total), Some(fee), Some(atoms)) = (
            self.input_count,
            self.unit_gold,
            self.gold_equivalent,
            self.fee_unbound_gold,
            self.license_atoms_debit,
        ) else {
            return false;
        };
        let (Some(created), Some(expires)) = (self.created_at_ms, self.expires_at_ms) else {
            return false;
        };
        count >= quantity
            && count <= u64::from(u32::MAX)
            && unit > 0
            && unit.checked_mul(quantity) == Some(total)
            && total > 0
            && total <= i64::MAX as u64
            && atoms == total
            && fee > 0
            && fee <= total
            && self.valuation_version.as_deref().is_some_and(|version| {
                !version.is_empty()
                    && version.len() <= 128
                    && version.trim() == version
                    && !version.chars().any(char::is_control)
            })
            && created >= 0
            && created.checked_add(QUOTE_TTL_MS) == Some(expires)
            && now_ms >= created
            && now_ms < expires
    }
}

/// UI intent only. It is not a native dispatch or a physical send permit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HuntingMaterialInput {
    Quote { unique_id: u64, quantity: u64 },
    Confirm { quote_id: String },
    Query { quote_id: String },
}

/// Keep one instance per realm/account/character presentation, including a
/// same-actor socket resume. An actor switch selects that actor's separate guard
/// while retaining the old actor's recovery. The host scopes all observations;
/// this view does not establish identity. Close never retires a pending operation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HuntingMaterialUi {
    model: HuntingMaterialModel,
    input: Option<HuntingMaterialInput>,
    pending_quote: Option<String>,
    confirmed_quotes: BTreeSet<String>,
    dismissed_quote: Option<String>,
}

impl HuntingMaterialUi {
    /// Each real host delivery acknowledges a consumed UI intent, including an
    /// equal Pending response. Do not call this every frame to fabricate an ack.
    /// A host Rejected phase must mean a definite failure/NotSubmitted; unknown
    /// transaction outcomes must remain Pending (or Unknown with the original id).
    pub fn observe(&mut self, model: &HuntingMaterialModel) {
        self.input = None;
        let valid_id = model.quote_id.as_deref().filter(|id| valid_quote_id(id));
        if self.pending_quote.as_deref() == valid_id && valid_id.is_some() {
            if matches!(
                model.phase,
                HuntingMaterialPhase::Committed | HuntingMaterialPhase::Rejected
            ) {
                self.pending_quote = None;
            }
        } else if self.pending_quote.is_none()
            && matches!(
                model.phase,
                HuntingMaterialPhase::Pending | HuntingMaterialPhase::Unknown
            )
        {
            self.pending_quote = valid_id.map(str::to_owned);
        }
        if model.quote_id != self.dismissed_quote || model.phase != HuntingMaterialPhase::Committed
        {
            self.dismissed_quote = None;
        }
        self.model = model.clone();
    }

    pub fn model(&self) -> &HuntingMaterialModel {
        &self.model
    }

    pub fn input(&self) -> Option<&HuntingMaterialInput> {
        self.input.as_ref()
    }

    pub fn original_quote_id(&self) -> Option<&str> {
        self.pending_quote.as_deref().or_else(|| {
            self.model
                .quote_id
                .as_deref()
                .filter(|id| valid_quote_id(id))
        })
    }

    pub fn pending(&self) -> bool {
        self.pending_quote.is_some()
            || self.model.phase == HuntingMaterialPhase::Pending
            || (self.model.phase == HuntingMaterialPhase::Unknown
                && self.model.quote_id.as_deref().is_some_and(valid_quote_id))
            || matches!(
                self.input,
                Some(HuntingMaterialInput::Confirm { .. } | HuntingMaterialInput::Query { .. })
            )
    }

    pub fn recovery_visible(&self) -> bool {
        self.pending()
            || (self.model.phase == HuntingMaterialPhase::Committed
                && self.model.quote_id.as_deref().is_some_and(valid_quote_id)
                && self.model.quote_id != self.dismissed_quote)
    }

    /// Bag selection/capacity is deliberately not inferred here. The caller
    /// resolves the actual Bag instance and the physical host captures its source.
    pub fn can_quote(&self, unique_id: u64, quantity: u64) -> bool {
        self.model.protocol_v1
            && self.model.has_display_actor()
            && !self.model.reload_required
            && valid_intent(unique_id, quantity)
            && self.input.is_none()
            && !self.pending()
            && self.confirmed_quotes.len() < MAX_CONFIRMED_QUOTES
            && matches!(
                self.model.phase,
                HuntingMaterialPhase::Ready
                    | HuntingMaterialPhase::Quoted
                    | HuntingMaterialPhase::Rejected
            )
    }

    pub fn can_confirm(&self, unique_id: u64, quantity: u64, now_ms: i64) -> bool {
        self.model.protocol_v1
            && self.model.has_display_actor()
            && self.model.phase == HuntingMaterialPhase::Quoted
            && !self.model.reload_required
            && self.input.is_none()
            && !self.pending()
            && self.confirmed_quotes.len() < MAX_CONFIRMED_QUOTES
            && self.model.valid_quote(unique_id, quantity, now_ms)
            && self
                .model
                .quote_id
                .as_ref()
                .is_some_and(|id| !self.confirmed_quotes.contains(id))
    }

    pub fn can_query(&self) -> bool {
        self.model.protocol_v1
            && self.model.has_display_actor()
            && !self.model.reload_required
            && self.input.is_none()
            && self.pending()
            && self
                .original_quote_id()
                .is_some_and(|id| self.model.quote_id.as_deref() == Some(id) && valid_quote_id(id))
    }

    /// A visible operation may always be closed, including an unresolved one.
    /// This only describes presentation; Close never unlocks its economic lane.
    pub fn can_close(&self) -> bool {
        self.pending()
            || self.input.is_some()
            || self.model.phase != HuntingMaterialPhase::Unknown
            || self.model.notice.is_some()
    }

    pub fn begin_quote(&mut self, unique_id: u64, quantity: u64) -> bool {
        if !self.can_quote(unique_id, quantity) {
            return false;
        }
        self.input = Some(HuntingMaterialInput::Quote {
            unique_id,
            quantity,
        });
        true
    }

    pub fn begin_confirm(&mut self, unique_id: u64, quantity: u64, now_ms: i64) -> bool {
        if !self.can_confirm(unique_id, quantity, now_ms) {
            return false;
        }
        let id = self.model.quote_id.as_ref().expect("validated quote id");
        self.confirmed_quotes.insert(id.clone());
        self.pending_quote = Some(id.clone());
        self.input = Some(HuntingMaterialInput::Confirm {
            quote_id: id.clone(),
        });
        true
    }

    pub fn begin_query(&mut self) -> bool {
        if !self.can_query() {
            return false;
        }
        self.input = Some(HuntingMaterialInput::Query {
            quote_id: self
                .original_quote_id()
                .expect("validated original id")
                .into(),
        });
        true
    }

    /// Close only dismisses completed presentation. Pending keeps both its
    /// original query id and any unacknowledged Confirm/Query input latch.
    pub fn close(&mut self) {
        if !self.pending() && self.model.phase == HuntingMaterialPhase::Committed {
            self.dismissed_quote = self.model.quote_id.clone();
        }
    }
}

#[cfg(test)]
#[path = "hunting_material_tests.rs"]
mod tests;
