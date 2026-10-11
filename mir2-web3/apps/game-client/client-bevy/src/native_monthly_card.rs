//! Presentation-only monthly access state. The server owns all dates and grants.
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthlyCardStatus {
    pub required: bool,
    pub active: bool,
    pub expires_at_ms: Option<u64>,
    pub server_now_ms: u64,
    pub remaining_ms: u64,
    pub can_enter_game: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MonthlyCardOperation {
    Status,
    Redeem,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthlyCardReply {
    pub request_id: Option<u64>,
    pub operation: MonthlyCardOperation,
    pub status: Option<MonthlyCardStatus>,
    pub error: Option<String>,
    pub replayed: Option<bool>,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MonthlyCardFocus {
    #[default]
    Code,
    Redeem,
    Refresh,
    Billing,
    Close,
}
impl MonthlyCardFocus {
    pub fn next(self, reverse: bool) -> Self {
        match (self, reverse) {
            (Self::Code, false) | (Self::Refresh, true) => Self::Redeem,
            (Self::Redeem, false) | (Self::Billing, true) => Self::Refresh,
            (Self::Refresh, false) | (Self::Close, true) => Self::Billing,
            (Self::Billing, false) | (Self::Code, true) => Self::Close,
            _ => Self::Code,
        }
    }
}
#[derive(Clone, Default, PartialEq, Eq)]
pub struct MonthlyCardPanel {
    pub request_id: u64,
    pub submitted_code: String,
    pub open: bool,
    pub focus: MonthlyCardFocus,
    pub status: Option<MonthlyCardStatus>,
    pub code: String,
    pub pending: bool,
    pub command_sent: bool,
    pub request_started_at_ms: u64,
    pub message: Option<String>,
}
impl std::fmt::Debug for MonthlyCardPanel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MonthlyCardPanel")
            .field("open", &self.open)
            .field("status", &self.status)
            .field("code", &"[REDACTED]")
            .field("pending", &self.pending)
            .finish()
    }
}
impl MonthlyCardPanel {
    pub fn append(&mut self, text: &str) -> bool {
        if !self.open || self.pending || self.focus != MonthlyCardFocus::Code {
            return false;
        }
        let before = self.code.len();
        for c in text.trim().chars() {
            if self.code.len() < 47 && (c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                self.code.push(c);
            }
        }
        self.code.len() != before
    }
    pub fn can_submit(&self) -> bool {
        !self.pending && self.code.len() == 47 && self.code.starts_with("MC1-")
    }
    pub fn accept(&mut self, reply: MonthlyCardReply) {
        if reply.operation == MonthlyCardOperation::Redeem
            && reply.request_id != Some(self.request_id)
        {
            return;
        }
        if let Some(status) = reply.status {
            self.status = Some(status);
        }
        if reply.operation == MonthlyCardOperation::Redeem {
            self.pending = false;
            self.command_sent = false;
            if reply.error.is_none() {
                if self.code == self.submitted_code {
                    self.code.clear();
                }
                self.message = Some(
                    if reply.replayed == Some(true) {
                        "monthlyCardAlreadyRedeemed"
                    } else {
                        "monthlyCardRedeemed"
                    }
                    .into(),
                );
            }
        }
        if let Some(error) = reply.error {
            self.message = Some(error);
        }
    }
    pub fn expire_pending_request(&mut self, now: u64) -> bool {
        if !self.pending || now.saturating_sub(self.request_started_at_ms) < 15_000 {
            return false;
        }
        self.pending = false;
        self.command_sent = false;
        self.message = Some("monthlyCardResultUnconfirmed".into());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keyboard_focus_reaches_billing_in_both_directions() {
        let sequence = [
            MonthlyCardFocus::Code,
            MonthlyCardFocus::Redeem,
            MonthlyCardFocus::Refresh,
            MonthlyCardFocus::Billing,
            MonthlyCardFocus::Close,
        ];
        for (index, focus) in sequence.iter().enumerate() {
            assert_eq!(focus.next(false), sequence[(index + 1) % sequence.len()]);
            assert_eq!(
                focus.next(true),
                sequence[(index + sequence.len() - 1) % sequence.len()]
            );
        }
    }
    fn status(active: bool) -> MonthlyCardStatus {
        MonthlyCardStatus {
            required: true,
            active,
            expires_at_ms: Some(123),
            server_now_ms: 2,
            remaining_ms: 121,
            can_enter_game: active,
        }
    }
    #[test]
    fn monthly_card_clipboard_and_input_are_bounded_and_private() {
        let mut card = MonthlyCardPanel {
            open: true,
            ..Default::default()
        };
        assert!(card.append(&format!(" MC1-{} ", "A".repeat(43))));
        assert!(card.can_submit());
        assert!(!format!("{card:?}").contains(&card.code));
        assert!(!card.append("more"));
        card.pending = true;
        assert!(!card.append("A"));
    }
    #[test]
    fn monthly_card_unsolicited_status_does_not_settle_an_inflight_redemption() {
        let mut card = MonthlyCardPanel {
            pending: true,
            request_id: 1,
            code: "keep-code".into(),
            submitted_code: "keep-code".into(),
            ..Default::default()
        };
        card.accept(MonthlyCardReply {
            request_id: None,
            operation: MonthlyCardOperation::Status,
            status: Some(status(false)),
            error: None,
            replayed: None,
        });
        assert!(card.pending && card.code == "keep-code");
        card.accept(MonthlyCardReply {
            request_id: Some(1),
            operation: MonthlyCardOperation::Redeem,
            status: Some(status(true)),
            error: None,
            replayed: Some(false),
        });
        assert!(!card.pending && card.code.is_empty());
        assert!(card.status.unwrap().can_enter_game);
    }
    #[test]
    fn monthly_card_unknown_result_keeps_the_code_for_safe_retry() {
        let mut card = MonthlyCardPanel {
            pending: true,
            command_sent: true,
            code: "keep-code".into(),
            request_started_at_ms: 100,
            ..Default::default()
        };
        assert!(!card.expire_pending_request(15_099));
        assert!(card.expire_pending_request(15_100));
        assert!(!card.pending && !card.command_sent && card.code == "keep-code");
    }
    #[test]
    fn monthly_card_late_receipt_cannot_settle_a_different_request() {
        let mut card = MonthlyCardPanel {
            pending: true,
            request_id: 2,
            code: "new-code".into(),
            submitted_code: "new-code".into(),
            ..Default::default()
        };
        card.accept(MonthlyCardReply {
            request_id: Some(1),
            operation: MonthlyCardOperation::Redeem,
            status: Some(status(true)),
            error: None,
            replayed: Some(false),
        });
        assert!(card.pending && card.code == "new-code" && card.status.is_none());
        card.accept(MonthlyCardReply {
            request_id: Some(2),
            operation: MonthlyCardOperation::Redeem,
            status: Some(status(true)),
            error: None,
            replayed: Some(true),
        });
        assert!(!card.pending && card.code.is_empty());
        assert_eq!(card.message.as_deref(), Some("monthlyCardAlreadyRedeemed"));
    }
}
