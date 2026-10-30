//! Server-only capability for saving an authoritative Zone's personal state.
//! The lease row stays locked until the account/save transaction commits.
use super::{AccountStoreDatabaseMode, AccountStoreMutationPlan, SimulationConfig};
use postgres::Transaction;

pub const COMMIT_OUTCOME_UNKNOWN: &str = "ZONE_SAVE_COMMIT_OUTCOME_UNKNOWN";
pub const REJECTED: &str = "ZONE_SAVE_FENCE_REJECTED";
pub const UNAVAILABLE: &str = "ZONE_SAVE_FENCE_UNAVAILABLE";

/// This capability is constructed by the authoritative host, never deserialized
/// from a client command, a save snapshot or a recovery journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneSaveFence {
    zone_id: String,
    owner_id: String,
    fencing_token: u64,
}

impl ZoneSaveFence {
    pub fn new(zone_id: String, owner_id: String, fencing_token: u64) -> Result<Self, String> {
        let valid_id = |value: &str| {
            !value.is_empty()
                && value.len() <= 1024
                && value.trim() == value
                && !value.chars().any(char::is_control)
        };
        if !valid_id(&zone_id)
            || !valid_id(&owner_id)
            || fencing_token == 0
            || fencing_token > i64::MAX as u64
        {
            return Err(REJECTED.to_string());
        }
        Ok(Self {
            zone_id,
            owner_id,
            fencing_token,
        })
    }

    pub fn zone_id(&self) -> &str {
        &self.zone_id
    }
    pub fn owner_id(&self) -> &str {
        &self.owner_id
    }
    pub fn fencing_token(&self) -> u64 {
        self.fencing_token
    }

    pub(super) fn lock_and_validate(
        &self,
        transaction: &mut Transaction<'_>,
        mode: AccountStoreDatabaseMode,
    ) -> Result<(), String> {
        if mode != AccountStoreDatabaseMode::SourceOfTruth {
            return Err(REJECTED.to_string());
        }
        // Local settings cannot leak through the pooled connection. The first
        // row touched is always the lease, before clock/guild/account locks.
        transaction
            .batch_execute(
                "SET LOCAL lock_timeout = '250ms'; \
             SET LOCAL statement_timeout = '2000ms'; \
             SET LOCAL idle_in_transaction_session_timeout = '5000ms'",
            )
            .map_err(|_| UNAVAILABLE.to_string())?;
        let row = transaction
            .query_opt(
                "SELECT owner_id, fencing_token, expires_at_ms, heartbeat_at_ms \
             FROM public.zone_owner_leases WHERE zone_id = $1 FOR UPDATE",
                &[&self.zone_id],
            )
            .map_err(|_| UNAVAILABLE.to_string())?
            .ok_or_else(|| REJECTED.to_string())?;
        let owner: String = row.try_get(0).map_err(|_| UNAVAILABLE.to_string())?;
        let token: i64 = row.try_get(1).map_err(|_| UNAVAILABLE.to_string())?;
        let expires: i64 = row.try_get(2).map_err(|_| UNAVAILABLE.to_string())?;
        let heartbeat: i64 = row.try_get(3).map_err(|_| UNAVAILABLE.to_string())?;
        // Read wall time only after the lock has been acquired. Time computed
        // before a contended lock can incorrectly authorize an expired owner.
        let now: i64 = transaction
            .query_one(
                "SELECT floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint",
                &[],
            )
            .map_err(|_| UNAVAILABLE.to_string())?
            .try_get(0)
            .map_err(|_| UNAVAILABLE.to_string())?;
        if owner != self.owner_id
            || token <= 0
            || token as u64 != self.fencing_token
            || expires <= 0
            || heartbeat <= 0
            || heartbeat > expires
            || now <= 0
            || expires <= now
        {
            return Err(REJECTED.to_string());
        }
        Ok(())
    }
}

pub(super) fn commit_outcome_unknown(reason: &str) -> String {
    format!(
        "{COMMIT_OUTCOME_UNKNOWN}: {}: {reason}",
        super::ACCOUNT_STORE_COMMIT_OUTCOME_UNKNOWN
    )
}

pub(super) fn classify_commit_error(_error: postgres::Error) -> String {
    // A server error at COMMIT is not sufficient proof of non-publication:
    // statement_completion_unknown (40003), transaction_resolution_unknown
    // (08007), fatal responses and future SQLSTATEs can leave it uncertain.
    // Conservatively freeze every failed COMMIT, including known rollback
    // cases, without exposing driver details or inviting a source replay.
    commit_outcome_unknown("commit outcome unavailable")
}

impl SimulationConfig {
    pub fn with_zone_save_fence(mut self, fence: ZoneSaveFence) -> Result<Self, String> {
        self.validate_zone_save_fence_binding()?;
        self.zone_save_fence = Some(fence);
        Ok(self)
    }

    pub fn zone_save_fence(&self) -> Option<&ZoneSaveFence> {
        self.zone_save_fence.as_ref()
    }

    pub(super) fn validate_zone_save_fence_binding(&self) -> Result<(), String> {
        if self.account_store_database_mode != AccountStoreDatabaseMode::SourceOfTruth
            || self.account_store_path.is_some()
            || self.file_authority.is_some()
        {
            return Err(REJECTED.to_string());
        }
        let url = self
            .account_store_database_url
            .as_deref()
            .ok_or_else(|| REJECTED.to_string())?;
        super::validate_postgres_account_store_database_url(url)
            .map_err(|_| REJECTED.to_string())?;
        let parsed: super::PostgresClientConfig = url.parse().map_err(|_| REJECTED.to_string())?;
        if parsed.get_hosts().is_empty() || parsed.get_dbname().is_none_or(str::is_empty) {
            return Err(REJECTED.to_string());
        }
        Ok(())
    }

    pub(super) fn bind_zone_save_fence_to_plan(&self, plan: &mut AccountStoreMutationPlan) {
        plan.zone_save_fence = self.zone_save_fence.clone();
        #[cfg(test)]
        if plan.zone_save_fence.is_some() {
            plan.zone_save_fence_test_fault = self
                .zone_save_fence_test_fault
                .lock()
                .expect("zone-save-fence test fault mutex poisoned")
                .take();
        }
    }
}

/// Test-only acknowledgement loss after a real successful COMMIT. These are
/// bounded fault labels, not claims of a native socket-level failure injection.
#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub(super) enum TestFault {
    PanicAfterCommit,
    LoseResponseAfterCommit,
}

#[cfg(test)]
#[path = "zone_save_fence_tests.rs"]
mod tests;
