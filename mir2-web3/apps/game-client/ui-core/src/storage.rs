//! Shared ordinary personal-storage request/receipt contract.
//!
//! This is deliberately a protocol seam, not a renderer. Hosts may create a
//! request from a future storage control and must correlate the authoritative
//! receipt by every request field before clearing pending state.

use serde::{Deserialize, Serialize};

pub const NATIVE_STORAGE_RECEIPT_PROTOCOL: &str = "nativeStorageReceiptV1";
pub const STORAGE_REQUEST_ID_MIN_BYTES: usize = 1;
pub const STORAGE_REQUEST_ID_MAX_BYTES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StorageOperation {
    StoreItem,
    TakeBackItem,
}

impl StorageOperation {
    pub const fn wire_type(self) -> &'static str {
        match self {
            Self::StoreItem => "storeItemV2",
            Self::TakeBackItem => "takeBackItemV2",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageRequest {
    pub request_id: String,
    pub operation: StorageOperation,
    pub from: i32,
    pub to: i32,
}

impl StorageRequest {
    pub fn new(
        request_id: String,
        operation: StorageOperation,
        from: i32,
        to: i32,
    ) -> Option<Self> {
        let request = Self {
            request_id,
            operation,
            from,
            to,
        };
        request.is_valid().then_some(request)
    }

    pub fn is_valid(&self) -> bool {
        self.from >= 0 && self.to >= 0 && is_valid_request_id(&self.request_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageReceipt {
    pub protocol: String,
    pub request_id: String,
    pub operation: StorageOperation,
    pub from: i32,
    pub to: i32,
    pub success: bool,
}

impl StorageReceipt {
    pub fn matches_request(&self, request: &StorageRequest) -> bool {
        self.protocol == NATIVE_STORAGE_RECEIPT_PROTOCOL
            && self.request_id == request.request_id
            && self.operation == request.operation
            && self.from == request.from
            && self.to == request.to
    }

    pub fn is_valid(&self) -> bool {
        self.protocol == NATIVE_STORAGE_RECEIPT_PROTOCOL
            && self.from >= 0
            && self.to >= 0
            && is_valid_request_id(&self.request_id)
    }
}

pub fn request_id_for_sequence(sequence: u64) -> String {
    format!("st-{sequence:016}")
}

/// Zero is an exhausted sentinel. Callers must reject it rather than wrap or
/// reuse a request id after `u64::MAX`.
pub fn next_request_sequence(sequence: u64) -> u64 {
    sequence.checked_add(1).unwrap_or(0)
}

pub fn is_printable_ascii(value: &str) -> bool {
    value.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
}

pub fn is_valid_request_id(value: &str) -> bool {
    (STORAGE_REQUEST_ID_MIN_BYTES..=STORAGE_REQUEST_ID_MAX_BYTES).contains(&value.len())
        && is_printable_ascii(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_ids_are_monotonic_bounded_and_printable() {
        assert_eq!(request_id_for_sequence(1), "st-0000000000000001");
        assert_eq!(next_request_sequence(u64::MAX), 0);
        assert!(is_valid_request_id("st-1"));
        assert!(!is_valid_request_id(""));
        assert!(!is_valid_request_id("st-1\n"));
        assert!(!is_valid_request_id(&"x".repeat(65)));
    }

    #[test]
    fn receipt_requires_exact_operation_and_coordinates() {
        let request =
            StorageRequest::new("st-1".into(), StorageOperation::StoreItem, 3, 9).unwrap();
        let receipt: StorageReceipt = serde_json::from_value(json!({
            "protocol": NATIVE_STORAGE_RECEIPT_PROTOCOL,
            "requestId": "st-1",
            "operation": "storeItem",
            "from": 3,
            "to": 9,
            "success": true
        }))
        .unwrap();
        assert!(receipt.is_valid());
        assert!(receipt.matches_request(&request));
        assert!(!receipt.matches_request(
            &StorageRequest::new("st-1".into(), StorageOperation::TakeBackItem, 3, 9,).unwrap()
        ));
        assert!(!receipt.matches_request(
            &StorageRequest::new("st-2".into(), StorageOperation::StoreItem, 3, 9,).unwrap()
        ));
    }
}
