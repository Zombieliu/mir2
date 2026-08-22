//! Shared native GameShop purchase contract.
//!
//! These types describe only the opt-in native WebSocket envelope. The
//! server remains authoritative for payment, stock and mail.

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

pub const NATIVE_GAME_SHOP_RECEIPT_PROTOCOL: &str = "nativeGameShopReceiptV1";
pub const NATIVE_GAME_SHOP_RECEIPT_CAPABILITY: &str = "nativeGameShopReceiptV1";
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
}

impl GameShopRequest {
    pub fn new(request_id: String, g_index: i32, quantity: u8, price_type: i32) -> Option<Self> {
        let request = Self { request_id, g_index, quantity, price_type };
        request.is_valid().then_some(request)
    }

    pub fn is_valid(&self) -> bool {
        self.g_index >= 0
            && (1..=99).contains(&self.quantity)
            && matches!(self.price_type, 0 | 1)
            && is_valid_request_id(&self.request_id)
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
    #[serde(default)]
    pub new_stock_level: Option<i32>,
    #[serde(default)]
    pub mail_id: Option<u64>,
    #[serde(default)]
    pub code: Option<GameShopFailureCode>,
}

impl GameShopReceipt {
    pub fn matches_request(&self, request: &GameShopRequest) -> bool {
        self.protocol == NATIVE_GAME_SHOP_RECEIPT_PROTOCOL
            && self.request_id == request.request_id
            && self.g_index == request.g_index
            && self.quantity == request.quantity
            && self.price_type == request.price_type
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
        self.protocol == NATIVE_GAME_SHOP_RECEIPT_PROTOCOL
            && is_valid_request_id(&self.request_id)
            && self.g_index >= 0
            && (1..=99).contains(&self.quantity)
            && matches!(self.price_type, 0 | 1)
            && self.code.as_ref().is_none_or(GameShopFailureCode::is_valid)
            && shape_is_valid
            && self.new_stock_level.is_none_or(|stock| stock >= 0)
    }
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
        })).unwrap();
        assert!(receipt.is_valid());
        assert!(receipt.matches_request(&request));
        assert!(!receipt.matches_request(&GameShopRequest::new("gs-2".into(), 31, 2, 1).unwrap()));
    }

    #[test]
    fn failures_are_stable_printable_codes() {
        let code = GameShopFailureCode::parse("insufficientCurrency").unwrap();
        assert_eq!(code.as_str(), "insufficientCurrency");
        assert_eq!(serde_json::to_string(&code).unwrap(), "\"insufficientCurrency\"");
        assert!(GameShopFailureCode::parse("bad\ncode").is_none());
        assert!(GameShopFailureCode::parse(&"x".repeat(64)).is_some());
        assert!(GameShopFailureCode::parse(&"x".repeat(65)).is_none());
    }

    #[test]
    fn receipt_success_and_failure_shapes_are_mutually_exclusive() {
        let base = GameShopReceipt {
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
