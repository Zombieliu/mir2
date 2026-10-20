//! Read-only native repair presentation. Only the physical native host may
//! publish this model; neither UI input nor a wallet observation proves a
//! repair committed or that its complete snapshot was applied.
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HuntingRepairMode {
    #[default]
    Unknown,
    Legacy,
    HuntingV1,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HuntingRepairPhase {
    #[default]
    Idle,
    Quoting,
    Quoted,
    Pending,
    Committed,
    Rejected,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Resource, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HuntingRepairModel {
    pub mode: HuntingRepairMode,
    pub phase: HuntingRepairPhase,
    pub quote_id: Option<String>,
    pub unique_id: Option<u64>,
    pub special: bool,
    pub cost_bound_gold: Option<u64>,
    pub bound_gold: Option<u64>,
    pub unbound_gold: Option<u64>,
    pub expires_at_ms: Option<i64>,
    pub reload_required: bool,
    pub notice: Option<String>,
}

pub(crate) fn valid_quote_id(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.bytes().any(|byte| byte != b'0')
}

/// A local input guard, separate from the host-owned economic read model.
/// It only suppresses duplicate clicks while the host consumes an intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HuntingRepairInput {
    Quote,
    Confirm,
    Query,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct HuntingRepairUi {
    pub model: HuntingRepairModel,
    pub input: Option<HuntingRepairInput>,
    dismissed_quote: Option<String>,
}

impl HuntingRepairUi {
    /// Every host delivery (including an equal-valued Pending response) is an
    /// input acknowledgement. It never derives success from local inventory.
    pub fn observe(&mut self, model: &HuntingRepairModel) {
        self.model = model.clone();
        self.input = None;
        if model.mode != HuntingRepairMode::HuntingV1
            || model.quote_id != self.dismissed_quote
            || !matches!(model.phase, HuntingRepairPhase::Committed)
        {
            self.dismissed_quote = None;
        }
    }

    pub fn pending(&self) -> bool {
        self.model.mode == HuntingRepairMode::HuntingV1
            && (self.model.phase == HuntingRepairPhase::Pending
                || matches!(
                    self.input,
                    Some(HuntingRepairInput::Confirm | HuntingRepairInput::Query)
                ))
            && self.model.quote_id.as_deref().is_some_and(valid_quote_id)
    }

    pub fn recovery_visible(&self) -> bool {
        self.pending()
            || (self.model.mode == HuntingRepairMode::HuntingV1
                && self.model.phase == HuntingRepairPhase::Committed
                && self.model.quote_id.as_deref().is_some_and(valid_quote_id)
                && self.model.quote_id != self.dismissed_quote)
    }

    pub fn can_quote(&self) -> bool {
        self.model.mode == HuntingRepairMode::HuntingV1
            && !self.model.reload_required
            && self.input.is_none()
            && matches!(
                self.model.phase,
                HuntingRepairPhase::Idle
                    | HuntingRepairPhase::Quoted
                    | HuntingRepairPhase::Rejected
            )
    }

    pub fn can_confirm(&self, unique_id: u64, special: bool) -> bool {
        self.model.mode == HuntingRepairMode::HuntingV1
            && self.model.phase == HuntingRepairPhase::Quoted
            && !self.model.reload_required
            && self.input.is_none()
            && self.model.unique_id == Some(unique_id)
            && self.model.special == special
            && self.model.cost_bound_gold.is_some()
            && self.model.expires_at_ms.is_some_and(|value| value >= 0)
            && self.model.quote_id.as_deref().is_some_and(valid_quote_id)
    }

    pub fn can_query(&self) -> bool {
        self.pending() && !matches!(self.input, Some(HuntingRepairInput::Query))
    }

    pub fn close(&mut self) {
        // Closing only dismisses known complete presentation. Unknown commit
        // outcomes retain the original query, independent of NPC/item state.
        if !self.pending() {
            self.dismissed_quote = self.model.quote_id.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quoted() -> HuntingRepairModel {
        HuntingRepairModel {
            mode: HuntingRepairMode::HuntingV1,
            phase: HuntingRepairPhase::Quoted,
            quote_id: Some("ab".repeat(32)),
            unique_id: Some(u64::MAX),
            special: true,
            cost_bound_gold: Some(0),
            expires_at_ms: Some(30_000),
            ..Default::default()
        }
    }

    #[test]
    fn hunting_presentation_defaults_unknown_and_preserves_exact_u64() {
        let mut ui = HuntingRepairUi::default();
        assert_eq!(ui.model.mode, HuntingRepairMode::Unknown);
        assert!(!ui.can_quote() && !ui.can_confirm(u64::MAX, true));
        let model = quoted();
        let encoded = serde_json::to_string(&model).unwrap();
        assert_eq!(
            serde_json::from_str::<HuntingRepairModel>(&encoded).unwrap(),
            model
        );
        ui.observe(&model);
        assert!(ui.can_confirm(u64::MAX, true));
        assert!(!ui.can_confirm(u64::MAX - 1, true) && !ui.can_confirm(u64::MAX, false));
        assert_eq!(ui.model.bound_gold, None);
        ui.observe(&HuntingRepairModel {
            unique_id: Some(0),
            ..model
        });
        assert!(
            ui.can_confirm(0, true),
            "zero is a valid server item identity"
        );
    }

    #[test]
    fn pending_close_keeps_only_original_query_and_host_ack_unlocks_query() {
        let mut ui = HuntingRepairUi::default();
        ui.observe(&quoted());
        ui.input = Some(HuntingRepairInput::Confirm);
        ui.close();
        assert!(ui.recovery_visible() && ui.can_query());
        assert!(!ui.can_quote() && !ui.can_confirm(u64::MAX, true));
        ui.input = Some(HuntingRepairInput::Query);
        assert!(!ui.can_query());
        let pending = HuntingRepairModel {
            phase: HuntingRepairPhase::Pending,
            ..quoted()
        };
        ui.observe(&pending);
        assert!(ui.can_query());
        ui.input = Some(HuntingRepairInput::Query);
        ui.observe(&pending);
        assert!(
            ui.can_query(),
            "equal host response must allow the next explicit Query"
        );
    }

    #[test]
    fn committed_is_readonly_until_close_and_invalid_ids_never_enable_actions() {
        let mut ui = HuntingRepairUi::default();
        let committed = HuntingRepairModel {
            phase: HuntingRepairPhase::Committed,
            ..quoted()
        };
        ui.observe(&committed);
        assert!(ui.recovery_visible());
        assert!(!ui.can_quote() && !ui.can_confirm(u64::MAX, true) && !ui.can_query());
        ui.close();
        ui.observe(&committed);
        assert!(!ui.recovery_visible());
        for quote_id in ["0".repeat(64), "AB".repeat(32), "ab".repeat(31)] {
            ui.observe(&HuntingRepairModel {
                quote_id: Some(quote_id),
                ..quoted()
            });
            assert!(!ui.can_confirm(u64::MAX, true));
        }
    }
}
