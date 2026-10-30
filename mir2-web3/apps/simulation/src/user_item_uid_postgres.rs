//! PostgreSQL ordinary-item identities share the existing player-drop floor.
//!
//! Opening this authority never provisions schema, performs a census, or seeds
//! a counter. An administrator must first census complete retained history via
//! the existing audited initialization boundary. Mutation lock order is always
//! the live Zone lease followed by this namespace's one UID authority row.
//! Successful issuance commits the floor before returning an identity; gaps
//! are safe and cannot be reclaimed following an uncertain acknowledgement.

use crate::{
    user_item_uid::{UserItemUid, UserItemUidError, UserItemUidReason, UserItemUidStore},
    SimulationConfig, ZoneEconomyWritePermit,
};
use postgres::{Client, GenericClient, NoTls, Transaction};
#[cfg(any(test, feature = "test-support"))]
use std::sync::atomic::{AtomicBool, Ordering};
use std::{fmt, time::Duration};

const QUERY_LIMITS: &str = "SET lock_timeout='250ms'; SET statement_timeout='2000ms'; \
    SET idle_in_transaction_session_timeout='5000ms'";

fn unavailable(operation: &'static str) -> UserItemUidError {
    UserItemUidError::DatabaseAuthorityUnavailable { operation }
}

fn rejected(reason: &'static str) -> UserItemUidError {
    UserItemUidError::DatabaseAuthorityRejected { reason }
}

fn connect(database_url: &str) -> Result<Client, UserItemUidError> {
    let mut config = database_url
        .parse::<postgres::Config>()
        .map_err(|_| unavailable("connection configuration"))?;
    config.connect_timeout(Duration::from_secs(2));
    let mut client = config
        .connect(NoTls)
        .map_err(|_| unavailable("connection"))?;
    client
        .batch_execute(QUERY_LIMITS)
        .map_err(|_| unavailable("query limits"))?;
    Ok(client)
}

fn read_namespace(client: &mut impl GenericClient) -> Result<String, UserItemUidError> {
    let namespace = client
        .query_one("SELECT current_schema()::text", &[])
        .map_err(|_| unavailable("source namespace lookup"))?
        .try_get::<_, Option<String>>(0)
        .map_err(|_| unavailable("source namespace decoding"))?
        .ok_or_else(|| rejected("source namespace is missing"))?;
    if namespace.is_empty() || namespace.len() > 1024 || namespace.chars().any(char::is_control) {
        return Err(rejected("source namespace is invalid"));
    }
    Ok(namespace)
}

fn require_namespace(
    client: &mut impl GenericClient,
    expected_namespace: &str,
) -> Result<(), UserItemUidError> {
    if read_namespace(client)? != expected_namespace {
        return Err(rejected("source namespace binding changed"));
    }
    Ok(())
}

fn read_existing_floor(
    client: &mut impl GenericClient,
    namespace: &str,
    lock: bool,
) -> Result<u64, UserItemUidError> {
    // Validate the original audited census as well as the mutable high-water
    // mark. Minting never creates either record or infers a replacement floor.
    let sql = if lock {
        "SELECT a.issued_through::text, a.census_digest, a.initialized_by, a.initialized_at_ms, \
         EXISTS(SELECT 1 FROM public.zone_player_drop_item_uid_census_audit c \
           WHERE c.source_namespace=a.source_namespace AND c.census_digest=a.census_digest \
             AND c.administrator_id=a.initialized_by AND c.recorded_at_ms=a.initialized_at_ms \
             AND c.censused_floor<=a.issued_through) \
         FROM public.zone_player_drop_item_uid_authority a \
         WHERE a.source_namespace=$1 AND a.singleton=TRUE FOR UPDATE OF a"
    } else {
        "SELECT a.issued_through::text, a.census_digest, a.initialized_by, a.initialized_at_ms, \
         EXISTS(SELECT 1 FROM public.zone_player_drop_item_uid_census_audit c \
           WHERE c.source_namespace=a.source_namespace AND c.census_digest=a.census_digest \
             AND c.administrator_id=a.initialized_by AND c.recorded_at_ms=a.initialized_at_ms \
             AND c.censused_floor<=a.issued_through) \
         FROM public.zone_player_drop_item_uid_authority a \
         WHERE a.source_namespace=$1 AND a.singleton=TRUE"
    };
    let row = client
        .query_opt(sql, &[&namespace])
        .map_err(|_| unavailable("censused authority lookup"))?
        .ok_or_else(|| rejected("explicit complete-history census is required for item UIDs"))?;
    let floor_text: String = row
        .try_get(0)
        .map_err(|_| rejected("censused authority floor is corrupt"))?;
    let floor = floor_text
        .parse::<u64>()
        .map_err(|_| rejected("censused authority floor is corrupt"))?;
    let digest: String = row
        .try_get(1)
        .map_err(|_| rejected("census metadata is corrupt"))?;
    let administrator: String = row
        .try_get(2)
        .map_err(|_| rejected("census metadata is corrupt"))?;
    let initialized_at_ms: i64 = row
        .try_get(3)
        .map_err(|_| rejected("census metadata is corrupt"))?;
    let audit_present: bool = row
        .try_get(4)
        .map_err(|_| rejected("census audit is corrupt"))?;
    if floor.to_string() != floor_text
        || digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || administrator.trim().is_empty()
        || administrator.len() > 1024
        || administrator.chars().any(char::is_control)
        || initialized_at_ms <= 0
        || !audit_present
    {
        return Err(rejected("complete-history UID census audit is invalid"));
    }
    Ok(floor)
}

/// Lock the live lease and the same previously censused UID row used by
/// ordinary issuance and player-drop batches. This does not commit and confers
/// no reusable authority outside the caller's transaction. A drop publisher
/// may inspect its immutable batch receipt after this lock, and reserve only
/// when no receipt exists, so retries do not advance the floor.
pub fn lock_existing_ordinary_item_uid_authority_in_transaction(
    transaction: &mut Transaction<'_>,
    database_url: &str,
    expected_namespace: &str,
    permit: &ZoneEconomyWritePermit,
) -> Result<u64, UserItemUidError> {
    permit
        .lock_and_validate(
            transaction,
            database_url,
            permit.fence().zone_id(),
            permit.fence().fencing_token(),
        )
        .map_err(|_| rejected("live source write permit is required"))?;
    require_namespace(transaction, expected_namespace)?;
    let floor = read_existing_floor(transaction, expected_namespace, true)?;
    // Admission may have waited behind another issuer holding this row. Its
    // COMMIT acknowledgement can freeze the shared source after our initial
    // lease check, so recheck once the complete mutation lock set is held.
    permit
        .validate_source_write_state()
        .map_err(|_| rejected("live source write permit is required"))?;
    Ok(floor)
}

fn checked_ordinary_batch(floor: u64, count: usize) -> Result<Vec<u64>, UserItemUidError> {
    if !(1..=1024).contains(&count) {
        return Err(rejected(
            "ordinary UID batch size must be between 1 and 1024",
        ));
    }
    let mut through = floor;
    let mut ids = Vec::with_capacity(count);
    for _ in 0..count {
        through =
            crate::player_drop::player_drop_next_ordinary_item_uid(through).map_err(|_| {
                UserItemUidError::Exhausted {
                    issued_through: floor,
                    // The existing PostgreSQL authority has no file generation.
                    generation: 0,
                }
            })?;
        ids.push(through);
    }
    Ok(ids)
}

fn update_floor(
    transaction: &mut Transaction<'_>,
    namespace: &str,
    floor: u64,
) -> Result<(), UserItemUidError> {
    let changed = transaction
        .execute(
            "UPDATE public.zone_player_drop_item_uid_authority \
             SET issued_through=$1::text::numeric WHERE source_namespace=$2 AND singleton=TRUE",
            &[&floor.to_string(), &namespace],
        )
        .map_err(|_| unavailable("authority floor update"))?;
    if changed != 1 {
        return Err(rejected("censused authority disappeared"));
    }
    Ok(())
}

/// Reserve ordinary identities from the existing shared floor, skipping the
/// deterministic monthly-card range. The caller must retain the live permit
/// and commit this transaction together with any required batch receipts.
/// This function never commits and returns no valid committed identity on its
/// own. On any error the caller must roll the transaction back.
pub fn reserve_ordinary_item_uids_in_transaction(
    transaction: &mut Transaction<'_>,
    database_url: &str,
    expected_namespace: &str,
    permit: &ZoneEconomyWritePermit,
    count: usize,
) -> Result<Vec<u64>, UserItemUidError> {
    if !(1..=1024).contains(&count) {
        return Err(rejected(
            "ordinary UID batch size must be between 1 and 1024",
        ));
    }
    let floor = lock_existing_ordinary_item_uid_authority_in_transaction(
        transaction,
        database_url,
        expected_namespace,
        permit,
    )?;
    let ids = checked_ordinary_batch(floor, count)?;
    update_floor(
        transaction,
        expected_namespace,
        *ids.last()
            .ok_or_else(|| rejected("ordinary UID batch is empty"))?,
    )?;
    Ok(ids)
}

/// A server-internal handle bound to the exact source connection, account
/// namespace and opaque Zone write permit. It exposes no schema/census API.
pub(crate) struct PostgresUserItemUidAuthority {
    database_url: String,
    namespace: String,
    permit: ZoneEconomyWritePermit,
    #[cfg(any(test, feature = "test-support"))]
    lose_next_commit_acknowledgement: AtomicBool,
}

impl fmt::Debug for PostgresUserItemUidAuthority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresUserItemUidAuthority")
            .field("source", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl PostgresUserItemUidAuthority {
    pub(crate) fn open_existing(
        database_url: &str,
        permit: ZoneEconomyWritePermit,
    ) -> Result<Self, UserItemUidError> {
        permit
            .validate_target_database(database_url)
            .map_err(|_| rejected("source connection binding mismatch"))?;
        let mut client = connect(database_url)?;
        let namespace = read_namespace(&mut client)?;
        read_existing_floor(&mut client, &namespace, false)?;
        Ok(Self {
            database_url: database_url.to_owned(),
            namespace,
            permit,
            #[cfg(any(test, feature = "test-support"))]
            lose_next_commit_acknowledgement: AtomicBool::new(false),
        })
    }

    fn connect(&self) -> Result<Client, UserItemUidError> {
        self.permit
            .validate_target_database(&self.database_url)
            .map_err(|_| rejected("source connection binding mismatch"))?;
        let mut client = connect(&self.database_url)?;
        require_namespace(&mut client, &self.namespace)?;
        Ok(client)
    }

    fn commit(&self, transaction: Transaction<'_>) -> Result<(), UserItemUidError> {
        // A different Zone sharing this source can freeze its gate while this
        // transaction prepares a UID. Do not knowingly submit after that freeze.
        self.permit
            .validate_source_write_state()
            .map_err(|_| rejected("live source write permit is required"))?;
        transaction.commit().map_err(|_| {
            self.permit.commit_outcome_unknown();
            UserItemUidError::DatabaseCommitOutcomeUnknown
        })?;
        #[cfg(any(test, feature = "test-support"))]
        if self
            .lose_next_commit_acknowledgement
            .swap(false, Ordering::SeqCst)
        {
            self.permit.commit_outcome_unknown();
            return Err(UserItemUidError::DatabaseCommitOutcomeUnknown);
        }
        Ok(())
    }
}

impl UserItemUidStore for PostgresUserItemUidAuthority {
    fn requires_postgres_source(&self) -> bool {
        true
    }

    fn issue_one(&self, _reason: UserItemUidReason) -> Result<UserItemUid, UserItemUidError> {
        let mut client = self.connect()?;
        let mut transaction = client
            .transaction()
            .map_err(|_| unavailable("issuance transaction"))?;
        let ids = reserve_ordinary_item_uids_in_transaction(
            &mut transaction,
            &self.database_url,
            &self.namespace,
            &self.permit,
            1,
        )?;
        let uid = *ids
            .first()
            .ok_or_else(|| rejected("ordinary UID batch is empty"))?;
        self.commit(transaction)?;
        UserItemUid::new(uid)
    }

    fn issued_through(&self) -> Result<u64, UserItemUidError> {
        // Read-only reconciliation deliberately remains usable after a source
        // write freeze or lease takeover; it cannot mint a replacement value.
        let mut client = self.connect()?;
        read_existing_floor(&mut client, &self.namespace, false)
    }

    fn ensure_issued_through_at_least(&self, floor: u64) -> Result<u64, UserItemUidError> {
        let mut client = self.connect()?;
        let mut transaction = client
            .transaction()
            .map_err(|_| unavailable("floor transaction"))?;
        // Even an equal/lower requested floor must validate the current owner
        // and source write state; it must not become a frozen-owner bypass.
        let current = lock_existing_ordinary_item_uid_authority_in_transaction(
            &mut transaction,
            &self.database_url,
            &self.namespace,
            &self.permit,
        )?;
        let through = current.max(floor);
        if through > current {
            update_floor(&mut transaction, &self.namespace, through)?;
        }
        self.commit(transaction)?;
        Ok(through)
    }

    fn validate_postgres_source(&self, config: &SimulationConfig) -> Result<(), UserItemUidError> {
        self.permit
            .validate_target_database(&self.database_url)
            .and_then(|_| self.permit.validate_source_configuration(config))
            .map_err(|_| rejected("live source configuration binding mismatch"))?;
        self.connect().map(|_| ())
    }

    #[cfg(any(test, feature = "test-support"))]
    fn lose_next_postgres_commit_acknowledgement_for_test(&self) -> Result<(), UserItemUidError> {
        self.lose_next_commit_acknowledgement
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map(|_| ())
            .map_err(|_| rejected("PostgreSQL acknowledgement fault is already armed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_batches_are_checked_before_any_floor_write() {
        assert_eq!(checked_ordinary_batch(0, 3).unwrap(), vec![1, 2, 3]);
        assert!(checked_ordinary_batch(0, 0).is_err());
        assert!(checked_ordinary_batch(0, 1025).is_err());
        assert!(checked_ordinary_batch(u64::MAX, 1).is_err());
        assert!(checked_ordinary_batch(u64::MAX - 1, 2).is_err());
        assert_eq!(
            checked_ordinary_batch(u64::MAX - 1, 1).unwrap(),
            vec![u64::MAX]
        );
        let monthly_range_start = 0x001d_0000_0000_0000;
        let after_monthly_range = 0x001e_0000_0000_0000;
        assert_eq!(
            checked_ordinary_batch(monthly_range_start - 2, 3).unwrap(),
            vec![
                monthly_range_start - 1,
                after_monthly_range,
                after_monthly_range + 1
            ]
        );
    }

    #[test]
    fn database_diagnostics_contain_only_static_labels() {
        for error in [
            unavailable("connection"),
            rejected("live source write permit is required"),
            UserItemUidError::DatabaseCommitOutcomeUnknown,
        ] {
            let display = error.to_string();
            let debug = format!("{error:?}");
            assert!(!display.contains("postgresql://"));
            assert!(!debug.contains("postgresql://"));
            assert!(std::error::Error::source(&error).is_none());
        }
    }
}
