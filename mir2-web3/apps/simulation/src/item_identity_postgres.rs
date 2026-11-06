//! Unregistered candidate: private, synchronous PostgreSQL mint authority.
//! This is a separate source transaction, before any business account locks.
//! Committed ranges can be wasted, but may never be reconstructed or rewound.

use super::item_identity_reservation::{CommittedItemIds, ReservationPlan};
use postgres::Client;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum IdentitySourceError {
    Source(String),
    InvalidState,
    NotBootstrapped,
    Stale,
    InvalidReservation,
    CommitOutcomeUnknown,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct IdentitySourceState {
    pub(crate) high_watermark: u64,
    pub(crate) version: i64,
    pub(crate) bootstrapped: bool,
    pub(crate) census_sha256: Option<String>,
}

/// Intentionally no production constructor yet. The future exclusive bootstrap
/// scanner must account for ALL authoritative carriers/external stores before
/// constructing this capability. It cannot be deserialized from a normal save.
pub(crate) struct CompleteIdentityCensus {
    high_watermark: u64,
    sha256: String,
}

impl CompleteIdentityCensus {
    #[cfg(test)]
    pub(crate) fn isolated_fixture(high_watermark: u64) -> Self {
        Self {
            high_watermark,
            sha256: "a".repeat(64),
        }
    }
    pub(crate) fn into_parts(self) -> (u64, String) {
        (self.high_watermark, self.sha256)
    }
}

fn source(e: postgres::Error) -> IdentitySourceError {
    IdentitySourceError::Source(e.to_string())
}

fn decode(row: postgres::Row) -> Result<IdentitySourceState, IdentitySourceError> {
    let high: String = row.try_get(0).map_err(source)?;
    let state = IdentitySourceState {
        high_watermark: high
            .parse()
            .map_err(|_| IdentitySourceError::InvalidState)?,
        version: row.try_get(1).map_err(source)?,
        bootstrapped: row.try_get(2).map_err(source)?,
        census_sha256: row.try_get(3).map_err(source)?,
    };
    if state.version <= 0
        || (!state.bootstrapped && (state.high_watermark != 0 || state.census_sha256.is_some()))
        || (state.bootstrapped && !state.census_sha256.as_deref().is_some_and(valid_digest))
    {
        return Err(IdentitySourceError::InvalidState);
    }
    Ok(state)
}

fn valid_digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

pub(crate) fn load(client: &mut Client) -> Result<IdentitySourceState, IdentitySourceError> {
    decode(client.query_one(
        "SELECT high_watermark::text, store_version, bootstrapped, census_sha256 FROM shared_item_identity_allocator WHERE singleton=TRUE", &[]
    ).map_err(source)?)
}

/// A maintenance-only capability plus exact uninitialized source expectation.
/// Not called automatically by migration, startup, or normal item gain.
pub(crate) fn bootstrap(
    client: &mut Client,
    expected: &IdentitySourceState,
    census: CompleteIdentityCensus,
) -> Result<IdentitySourceState, IdentitySourceError> {
    if expected.bootstrapped
        || expected.high_watermark != 0
        || expected.census_sha256.is_some()
        || expected.version <= 0
        || expected.version == i64::MAX
        || !valid_digest(&census.sha256)
    {
        return Err(IdentitySourceError::InvalidState);
    }
    let mut tx = client.transaction().map_err(source)?;
    let rows = tx.execute(
        "UPDATE shared_item_identity_allocator SET high_watermark=$1::text::numeric, store_version=store_version+1, bootstrapped=TRUE, census_sha256=$2 WHERE singleton=TRUE AND store_version=$3 AND high_watermark=0 AND NOT bootstrapped AND census_sha256 IS NULL",
        &[&census.high_watermark.to_string(), &census.sha256, &expected.version],
    ).map_err(source)?;
    if rows != 1 {
        return Err(IdentitySourceError::Stale);
    }
    tx.commit()
        .map_err(|_| IdentitySourceError::CommitOutcomeUnknown)?;
    Ok(IdentitySourceState {
        high_watermark: census.high_watermark,
        version: expected.version + 1,
        bootstrapped: true,
        census_sha256: Some(census.sha256),
    })
}

pub(crate) fn reserve(
    client: &mut Client,
    expected: &IdentitySourceState,
    count: u32,
) -> Result<CommittedItemIds, IdentitySourceError> {
    reserve_inner(client, expected, count, CommitFault::None)
}

enum CommitFault {
    None,
    #[cfg(test)]
    Rollback,
    #[cfg(test)]
    LoseCommittedResponse,
}

fn reserve_inner(
    client: &mut Client,
    expected: &IdentitySourceState,
    count: u32,
    fault: CommitFault,
) -> Result<CommittedItemIds, IdentitySourceError> {
    if !expected.bootstrapped {
        return Err(IdentitySourceError::NotBootstrapped);
    }
    let digest = expected
        .census_sha256
        .as_deref()
        .filter(|s| valid_digest(s))
        .ok_or(IdentitySourceError::InvalidState)?;
    let plan = ReservationPlan::prepare(expected.version, expected.high_watermark, count)
        .map_err(|_| IdentitySourceError::InvalidReservation)?;
    let (version, old_high, new_high) = plan.source_cas();
    let mut tx = client.transaction().map_err(source)?;
    // UPDATE acquires the single allocator row lock. Concurrent callers with the
    // same source snapshot serialize; only one can match exact version/high.
    let row = tx.query_opt(
        "UPDATE shared_item_identity_allocator SET high_watermark=$1::text::numeric, store_version=store_version+1 WHERE singleton=TRUE AND store_version=$2 AND high_watermark=$3::text::numeric AND bootstrapped=TRUE AND census_sha256=$4 RETURNING store_version,high_watermark::text",
        &[&new_high.to_string(), &version, &old_high.to_string(), &digest],
    ).map_err(source)?.ok_or(IdentitySourceError::Stale)?;
    let receipt_version: i64 = row.try_get(0).map_err(source)?;
    let receipt_high: String = row.try_get(1).map_err(source)?;
    let receipt_high: u64 = receipt_high
        .parse()
        .map_err(|_| IdentitySourceError::InvalidState)?;
    // Validate receipt BEFORE commit; only constructing a cursor happens later.
    if receipt_version != version + 1 || receipt_high != new_high {
        return Err(IdentitySourceError::InvalidState);
    }
    #[cfg(test)]
    if matches!(fault, CommitFault::Rollback) {
        tx.rollback().map_err(source)?;
        return Err(IdentitySourceError::Source(
            "injected known rollback".into(),
        ));
    }
    tx.commit()
        .map_err(|_| IdentitySourceError::CommitOutcomeUnknown)?;
    #[cfg(test)]
    if matches!(fault, CommitFault::LoseCommittedResponse) {
        return Err(IdentitySourceError::CommitOutcomeUnknown);
    }
    let _ = fault;
    CommittedItemIds::from_committed_source(plan, receipt_version, receipt_high)
        .map_err(|_| IdentitySourceError::InvalidState)
}

#[cfg(test)]
#[path = "item_identity_postgres_tests.rs"]
mod tests;
