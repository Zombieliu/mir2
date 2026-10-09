//! Shared production rules. No world/session copies, network I/O or implicit grants.
//! The authenticated economy authority must persist each returned transition together
//! with its actual inventory changes. A prepared transition is not a durable receipt.
mod catalog;
mod jobs;
mod planning;
mod stones;
pub use stones::*;
pub use catalog::*;
pub use jobs::*;
pub use planning::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductionError {
    InvalidCatalog(String),
    Overflow,
    InvalidBatch,
    Unavailable(String),
    MissingMaterial(String),
    InsufficientGold,
    InvalidInventory,
    InvalidRequest,
    Unauthorized,
    Busy,
    UnknownJob,
    TooEarly,
    AlreadyFinished,
    RequestConflict,
    LedgerFull,
    InvalidCheckpoint,
}
impl std::fmt::Display for ProductionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for ProductionError {}
pub(crate) fn add(a: u64, b: u64) -> Result<u64, ProductionError> {
    a.checked_add(b).ok_or(ProductionError::Overflow)
}
pub(crate) fn mul(a: u64, b: u64) -> Result<u64, ProductionError> {
    a.checked_mul(b).ok_or(ProductionError::Overflow)
}

/// Hash only the runtime-interpreted fields in canonical, ordered JSON.
pub(crate) fn digest<T: serde::Serialize>(value: &T) -> Result<String, ProductionError> {
    use sha2::{Digest, Sha256};
    let bytes = serde_json::to_vec(value).map_err(|_| ProductionError::InvalidCheckpoint)?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}
pub(crate) fn valid_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
