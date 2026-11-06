//! Shared native GameShop purchase contract.
//!
//! These types describe only the opt-in native WebSocket envelope. The
//! server remains authoritative for payment, stock and mail.

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

pub const NATIVE_GAME_SHOP_RECEIPT_PROTOCOL: &str = "nativeGameShopReceiptV1";
pub const NATIVE_GAME_SHOP_RECEIPT_CAPABILITY: &str = "nativeGameShopReceiptV1";
pub const NATIVE_GAME_SHOP_GIFT_RECEIPT_PROTOCOL: &str = "nativeGameShopGiftReceiptV1";
pub const NATIVE_GAME_SHOP_GIFT_RECEIPT_CAPABILITY: &str = "nativeGameShopGiftReceiptV1";
pub const GAME_SHOP_RECIPIENT_MIN_CHARS: usize = 3;
pub const GAME_SHOP_RECIPIENT_MAX_CHARS: usize = 15;
pub const GAME_SHOP_REQUEST_ID_MIN_BYTES: usize = 1;
pub const GAME_SHOP_REQUEST_ID_MAX_BYTES: usize = 64;
pub const GAME_SHOP_FAILURE_CODE_MAX_BYTES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameShopFailureCode {
    InvalidRequest,
    RequestInFlight,
    NotInGame,
    InvalidQuantity,
    UnknownProduct,
    ClassUnavailable,
    PaymentUnavailable,
    StockUnavailable,
    InsufficientCurrency,
    MailFull,
    RecipientUnavailable,
    SelfGiftUnavailable,
    GiftUnavailable,
    CommitFailed,
    Unknown(String),
}

impl GameShopFailureCode {
    pub fn as_str(&self) -> &str {
        match self {
            Self::InvalidRequest => "invalidRequest",
            Self::RequestInFlight => "requestInFlight",
            Self::NotInGame => "notInGame",
            Self::InvalidQuantity => "invalidQuantity",
            Self::UnknownProduct => "unknownProduct",
            Self::ClassUnavailable => "classUnavailable",
            Self::PaymentUnavailable => "paymentUnavailable",
            Self::StockUnavailable => "stockUnavailable",
            Self::InsufficientCurrency => "insufficientCurrency",
            Self::MailFull => "mailFull",
            Self::RecipientUnavailable => "recipientUnavailable",
            Self::SelfGiftUnavailable => "selfGiftUnavailable",
            Self::GiftUnavailable => "giftUnavailable",
            Self::CommitFailed => "commitFailed",
            Self::Unknown(value) => value,
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        if !(1..=GAME_SHOP_FAILURE_CODE_MAX_BYTES).contains(&value.len())
            || !is_printable_ascii(value)
        {
            return None;
        }
        Some(match value {
            "invalidRequest" => Self::InvalidRequest,
            "requestInFlight" => Self::RequestInFlight,
            "notInGame" => Self::NotInGame,
            "invalidQuantity" => Self::InvalidQuantity,
            "unknownProduct" => Self::UnknownProduct,
            "classUnavailable" => Self::ClassUnavailable,
            "paymentUnavailable" => Self::PaymentUnavailable,
            "stockUnavailable" => Self::StockUnavailable,
            "insufficientCurrency" => Self::InsufficientCurrency,
            "mailFull" => Self::MailFull,
            "recipientUnavailable" => Self::RecipientUnavailable,
            "selfGiftUnavailable" => Self::SelfGiftUnavailable,
            "giftUnavailable" => Self::GiftUnavailable,
            "commitFailed" => Self::CommitFailed,
            other => Self::Unknown(other.to_owned()),
        })
    }

    pub fn is_valid(&self) -> bool {
        Self::parse(self.as_str()).is_some()
    }
}

impl Serialize for GameShopFailureCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for GameShopFailureCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).ok_or_else(|| de::Error::custom("failure code must be printable ASCII"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameShopRequest {
    pub request_id: String,
    pub g_index: i32,
    pub quantity: u8,
    pub price_type: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipient_name: Option<String>,
}

impl GameShopRequest {
    pub fn new(request_id: String, g_index: i32, quantity: u8, price_type: i32) -> Option<Self> {
        let request = Self {
            request_id,
            g_index,
            quantity,
            price_type,
            recipient_name: None,
        };
        request.is_valid().then_some(request)
    }

    /// Gift is a distinct, Credits-only operation. Trim once before freezing
    /// the request; the exact trimmed name must be echoed by its receipt.
    pub fn new_gift(
        request_id: String,
        g_index: i32,
        quantity: u8,
        recipient_name: String,
    ) -> Option<Self> {
        let request = Self {
            request_id,
            g_index,
            quantity,
            price_type: 0,
            recipient_name: Some(recipient_name.trim().to_owned()),
        };
        request.is_valid().then_some(request)
    }

    pub fn protocol(&self) -> &'static str {
        if self.recipient_name.is_some() {
            NATIVE_GAME_SHOP_GIFT_RECEIPT_PROTOCOL
        } else {
            NATIVE_GAME_SHOP_RECEIPT_PROTOCOL
        }
    }

    /// Preserve the whole operation across terminal/reset boundaries. A gift
    /// must never be reconstructed as a purchase for the sender.
    pub fn from_receipt(receipt: &GameShopReceipt) -> Option<Self> {
        if !receipt.is_valid() {
            return None;
        }
        let request = Self {
            request_id: receipt.request_id.clone(),
            g_index: receipt.g_index,
            quantity: receipt.quantity,
            price_type: receipt.price_type,
            recipient_name: receipt.recipient_name.clone(),
        };
        request.is_valid().then_some(request)
    }

    pub fn is_valid(&self) -> bool {
        self.g_index >= 0
            && (1..=99).contains(&self.quantity)
            && matches!(self.price_type, 0 | 1)
            && is_valid_request_id(&self.request_id)
            && self
                .recipient_name
                .as_ref()
                .is_none_or(|name| self.price_type == 0 && is_valid_recipient_name(name))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameShopReceipt {
    pub protocol: String,
    pub request_id: String,
    pub success: bool,
    pub g_index: i32,
    pub quantity: u8,
    pub price_type: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipient_name: Option<String>,
    #[serde(default)]
    pub new_stock_level: Option<i32>,
    #[serde(default)]
    pub mail_id: Option<u64>,
    #[serde(default)]
    pub code: Option<GameShopFailureCode>,
}

impl GameShopReceipt {
    pub fn matches_request(&self, request: &GameShopRequest) -> bool {
        self.protocol == request.protocol()
            && self.request_id == request.request_id
            && self.g_index == request.g_index
            && self.quantity == request.quantity
            && self.price_type == request.price_type
            && self.recipient_name == request.recipient_name
    }

    pub fn is_valid(&self) -> bool {
        let shape_is_valid = if self.success {
            self.code.is_none() && self.mail_id.is_some()
        } else {
            self.code.as_ref().is_some_and(|code| {
                self.mail_id.is_none()
                    && (code.as_str() == "stockUnavailable" || self.new_stock_level.is_none())
            })
        };
        let operation_is_valid = match self.recipient_name.as_ref() {
            Some(name) => {
                self.protocol == NATIVE_GAME_SHOP_GIFT_RECEIPT_PROTOCOL
                    && self.price_type == 0
                    && is_valid_recipient_name(name)
            }
            None => self.protocol == NATIVE_GAME_SHOP_RECEIPT_PROTOCOL,
        };
        operation_is_valid
            && is_valid_request_id(&self.request_id)
            && self.g_index >= 0
            && (1..=99).contains(&self.quantity)
            && matches!(self.price_type, 0 | 1)
            && self.code.as_ref().is_none_or(GameShopFailureCode::is_valid)
            && shape_is_valid
            && self.new_stock_level.is_none_or(|stock| stock >= 0)
    }
}

/// Crystal Globals/Envir.CharacterReg: 3–15 BMP Chinese, ASCII alphanumeric
/// or underscore. Never truncate an overlong recipient into another identity.
pub fn is_valid_recipient_name(value: &str) -> bool {
    value == value.trim()
        && (GAME_SHOP_RECIPIENT_MIN_CHARS..=GAME_SHOP_RECIPIENT_MAX_CHARS)
            .contains(&value.chars().count())
        && value.len() <= GAME_SHOP_RECIPIENT_MAX_CHARS * 3
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || character == '_'
                || ('\u{4e00}'..='\u{9fa5}').contains(&character)
        })
}

pub fn request_id_for_sequence(sequence: u64) -> String {
    format!("gs-{sequence:016}")
}

/// Advance the per-session request sequence. Zero is a permanently exhausted
/// sentinel: after `u64::MAX` has been used, callers must fail closed instead
/// of wrapping or reusing a request id.
pub fn next_request_sequence(sequence: u64) -> u64 {
    sequence.checked_add(1).unwrap_or(0)
}

pub fn is_printable_ascii(value: &str) -> bool {
    value.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
}

pub fn is_valid_request_id(value: &str) -> bool {
    let bytes = value.len();
    (GAME_SHOP_REQUEST_ID_MIN_BYTES..=GAME_SHOP_REQUEST_ID_MAX_BYTES).contains(&bytes)
        && is_printable_ascii(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_ids_are_bounded_and_shared() {
        assert_eq!(request_id_for_sequence(1), "gs-0000000000000001");
        assert_eq!(next_request_sequence(u64::MAX), 0);
        assert!(is_valid_request_id("gs-1"));
        assert!(!is_valid_request_id(""));
        assert!(!is_valid_request_id("bad\nline"));
        assert!(!is_valid_request_id(&"x".repeat(65)));
    }

    #[test]
    fn receipt_requires_protocol_and_exact_request_fields() {
        let request = GameShopRequest::new("gs-1".into(), 31, 2, 1).unwrap();
        let receipt: GameShopReceipt = serde_json::from_value(json!({
            "protocol": NATIVE_GAME_SHOP_RECEIPT_PROTOCOL,
            "requestId": "gs-1", "success": true, "gIndex": 31,
            "quantity": 2, "priceType": 1, "newStockLevel": null, "mailId": 1842
        }))
        .unwrap();
        assert!(receipt.is_valid());
        assert!(receipt.matches_request(&request));
        assert!(!receipt.matches_request(&GameShopRequest::new("gs-2".into(), 31, 2, 1).unwrap()));
    }

    #[test]
    fn buy_serialization_keeps_its_original_shape_and_gift_is_credits_only() {
        let buy = GameShopRequest::new("gs-1".into(), 31, 2, 1).unwrap();
        assert_eq!(
            serde_json::to_value(&buy).unwrap(),
            json!({
                "requestId": "gs-1", "gIndex": 31, "quantity": 2, "priceType": 1
            })
        );
        let gift = GameShopRequest::new_gift("gs-2".into(), 31, 2, "  好友_123  ".into()).unwrap();
        assert_eq!(gift.recipient_name.as_deref(), Some("好友_123"));
        assert_eq!(gift.price_type, 0);
        assert_eq!(gift.protocol(), NATIVE_GAME_SHOP_GIFT_RECEIPT_PROTOCOL);
        assert_eq!(
            serde_json::to_value(&gift).unwrap(),
            json!({
                "requestId": "gs-2", "gIndex": 31, "quantity": 2, "priceType": 0,
                "recipientName": "好友_123"
            })
        );
        let mut gold_gift = gift;
        gold_gift.price_type = 1;
        assert!(!gold_gift.is_valid());
    }

    #[test]
    fn gift_recipient_respects_crystal_character_identity_without_truncation() {
        for name in [
            "",
            "ab",
            "CharacterName1234",
            "好".repeat(16).as_str(),
            "好友🙂",
            "Friend-1",
            "Friend Name",
            "Friend\n",
            "éclair",
        ] {
            assert!(!is_valid_recipient_name(name), "{name:?}");
        }
        let chinese = "好".repeat(15);
        assert_eq!(chinese.len(), 45);
        assert!(is_valid_recipient_name(&chinese));
        for name in ["a_1", "CharacterName12", "好友一"] {
            assert!(is_valid_recipient_name(name), "{name:?}");
        }
        assert!(GameShopRequest::new_gift("gs-1".into(), 31, 1, "x".repeat(16)).is_none());
    }

    #[test]
    fn gift_receipt_requires_operation_and_recipient_and_preserves_both_on_reset() {
        let request = GameShopRequest::new_gift("gs-1".into(), 31, 1, "Friend_1".into()).unwrap();
        let receipt: GameShopReceipt = serde_json::from_value(json!({
            "protocol": NATIVE_GAME_SHOP_GIFT_RECEIPT_PROTOCOL, "requestId": "gs-1",
            "success": true, "gIndex": 31, "quantity": 1, "priceType": 0,
            "recipientName": "Friend_1", "mailId": 1842
        }))
        .unwrap();
        assert!(receipt.is_valid());
        assert!(receipt.matches_request(&request));
        assert_eq!(
            GameShopRequest::from_receipt(&receipt),
            Some(request.clone())
        );
        let mut ui = crate::state::UiState::default();
        assert!(ui.preserve_exact_game_shop_receipt_boundary(&receipt));
        assert_eq!(ui.game_shop_pending, Some(request.clone()));
        let mut other = receipt.clone();
        other.recipient_name = Some("Friend_2".into());
        assert!(other.is_valid());
        assert!(!other.matches_request(&request));
        assert!(!ui.apply_game_shop_receipt(other));
        let mut buy_protocol = receipt.clone();
        buy_protocol.protocol = NATIVE_GAME_SHOP_RECEIPT_PROTOCOL.into();
        assert!(!buy_protocol.is_valid());
        assert!(!buy_protocol.matches_request(&request));
        assert!(GameShopRequest::from_receipt(&buy_protocol).is_none());
        assert!(ui.apply_game_shop_receipt(receipt));
        assert!(ui.game_shop_pending.is_none());
    }

    #[test]
    fn failures_are_stable_printable_codes() {
        let code = GameShopFailureCode::parse("insufficientCurrency").unwrap();
        assert_eq!(code.as_str(), "insufficientCurrency");
        assert_eq!(
            serde_json::to_string(&code).unwrap(),
            "\"insufficientCurrency\""
        );
        assert!(GameShopFailureCode::parse("bad\ncode").is_none());
        assert!(GameShopFailureCode::parse(&"x".repeat(64)).is_some());
        assert!(GameShopFailureCode::parse(&"x".repeat(65)).is_none());
    }

    #[test]
    fn receipt_success_and_failure_shapes_are_mutually_exclusive() {
        let base = GameShopReceipt {
            recipient_name: None,
            protocol: NATIVE_GAME_SHOP_RECEIPT_PROTOCOL.into(),
            request_id: "gs-1".into(),
            success: true,
            g_index: 31,
            quantity: 1,
            price_type: 1,
            new_stock_level: Some(3),
            mail_id: Some(1842),
            code: None,
        };
        assert!(base.is_valid());

        let mut success_with_code = base.clone();
        success_with_code.code = Some(GameShopFailureCode::CommitFailed);
        assert!(!success_with_code.is_valid());
        let mut success_without_mail = base.clone();
        success_without_mail.mail_id = None;
        assert!(!success_without_mail.is_valid());

        let mut failure = base.clone();
        failure.success = false;
        failure.mail_id = None;
        failure.new_stock_level = None;
        failure.code = Some(GameShopFailureCode::InsufficientCurrency);
        assert!(failure.is_valid());

        let mut failure_with_mail = failure.clone();
        failure_with_mail.mail_id = Some(1842);
        assert!(!failure_with_mail.is_valid());
        let mut failure_without_code = failure.clone();
        failure_without_code.code = None;
        assert!(!failure_without_code.is_valid());

        let mut stock_failure = failure;
        stock_failure.code = Some(GameShopFailureCode::StockUnavailable);
        stock_failure.new_stock_level = Some(0);
        assert!(stock_failure.is_valid());
        let mut unrelated_failure_with_stock = stock_failure.clone();
        unrelated_failure_with_stock.code = Some(GameShopFailureCode::MailFull);
        assert!(!unrelated_failure_with_stock.is_valid());
    }
}
