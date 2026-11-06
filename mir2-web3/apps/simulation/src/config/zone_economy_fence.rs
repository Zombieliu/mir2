//! Opaque authority for an external ledger sharing the account source database.
//! Admission and receipt lookup are read-only. A new mutation must recheck the
//! shared write state and lock the existing Zone lease in its own transaction.
use super::zone_save_fence::{self, ZoneSaveFence};
use super::{AccountStoreDatabaseMode, SimulationConfig};
use postgres::Transaction;

pub const REJECTED: &str = "ZONE_ECONOMY_WRITE_PERMIT_REJECTED";

/// Issued only from an already bound SourceOfTruth runtime configuration. No
/// constructor, serde implementation or database URL getter exposes authority
/// through a player command, checkpoint, receipt or diagnostic output.
#[derive(Clone)]
pub struct ZoneEconomyWritePermit {
    source_config: SimulationConfig,
    source_database_url: String,
    fence: ZoneSaveFence,
}

impl std::fmt::Debug for ZoneEconomyWritePermit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ZoneEconomyWritePermit")
            .field("authority", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl ZoneEconomyWritePermit {
    pub fn fence(&self) -> &ZoneSaveFence {
        &self.fence
    }

    /// Match the complete connection configuration, including credentials and
    /// startup options such as search_path. Endpoint/database equality alone
    /// would permit a different account namespace in the same PostgreSQL DB.
    /// This pure check remains available for read-only receipt reconciliation.
    pub fn validate_target_database(&self, ledger_database_url: &str) -> Result<(), String> {
        if ledger_database_url != self.source_database_url {
            return Err(REJECTED.into());
        }
        Ok(())
    }

    /// Context matching grants no fresh mutation admission. In particular an
    /// old committed receipt can still be read after an Unknown write freeze.
    pub fn validate_context(&self, zone_id: &str, generation: u64) -> Result<(), String> {
        if zone_id != self.fence.zone_id() || generation != self.fence.fencing_token() {
            return Err(REJECTED.into());
        }
        Ok(())
    }

    /// Inspect the shared source freeze without admitting a SQL mutation.
    /// A caller must still validate the lease inside its own transaction.
    pub fn validate_source_write_state(&self) -> Result<(), String> {
        self.source_config.ensure_account_store_writable()
    }

    /// A server-owned issuer must retain the same source and shared Unknown
    /// gate as its runtime. Equal connection strings alone do not establish
    /// that a separately constructed config shares the original write freeze.
    pub(crate) fn validate_source_configuration(
        &self,
        config: &SimulationConfig,
    ) -> Result<(), String> {
        config
            .validate_zone_save_fence_binding()
            .map_err(|_| REJECTED.to_string())?;
        if config.zone_save_operation.is_some()
            || config.account_store_database_url.as_deref()
                != Some(self.source_database_url.as_str())
            || config.zone_save_fence.as_ref() != Some(&self.fence)
            || !std::sync::Arc::ptr_eq(&config.account_store, &self.source_config.account_store)
            || !std::sync::Arc::ptr_eq(
                &config.account_store_persist_lock,
                &self.source_config.account_store_persist_lock,
            )
            || !std::sync::Arc::ptr_eq(
                &config.account_store_write_state,
                &self.source_config.account_store_write_state,
            )
        {
            return Err(REJECTED.into());
        }
        Ok(())
    }

    /// The ledger must call this before taking economy/account mutation locks.
    /// The shared lease row and its DB-time check remain in this transaction
    /// until the caller confirms COMMIT or rolls the entire mutation back.
    pub fn lock_and_validate(
        &self,
        transaction: &mut Transaction<'_>,
        ledger_database_url: &str,
        zone_id: &str,
        generation: u64,
    ) -> Result<(), String> {
        self.validate_mutation_admission(ledger_database_url, zone_id, generation)?;
        self.fence
            .lock_and_validate(transaction, AccountStoreDatabaseMode::SourceOfTruth)?;
        // A different writer can publish Unknown while this transaction waits
        // for the lease. A queued request must not rely on its earlier check.
        self.validate_mutation_admission(ledger_database_url, zone_id, generation)
    }

    fn validate_mutation_admission(
        &self,
        ledger_database_url: &str,
        zone_id: &str,
        generation: u64,
    ) -> Result<(), String> {
        self.validate_target_database(ledger_database_url)?;
        self.validate_context(zone_id, generation)?;
        self.source_config.ensure_account_store_writable()
    }

    /// A failed COMMIT acknowledgement or transaction-worker panic must freeze
    /// every shared source clone. Deliberately accepts no raw driver message.
    pub fn commit_outcome_unknown(&self) -> String {
        let error =
            zone_save_fence::commit_outcome_unknown("external economy commit outcome unavailable");
        self.source_config
            .freeze_account_store_writes(error.clone());
        error
    }
}

impl SimulationConfig {
    /// A durable source change is known to have committed, but its live Session
    /// could not publish the matching revision. Never admit another stale save.
    /// Fixed server-owned text keeps backend diagnostics out of this gate.
    pub(crate) fn freeze_committed_source_publication(&self) -> String {
        self.freeze_account_store_writes("SOURCE_COMMITTED_LIVE_PUBLICATION_FAILED".into())
    }

    pub(crate) fn freeze_unknown_item_uid_publication(&self) {
        // Caller supplies no raw backend diagnostic or player-controlled text.
        // File authorities persist their ordinary source freeze marker too.
        self.freeze_account_store_writes(
            crate::runtime::item_uid_issuance::ITEM_UID_COMMIT_OUTCOME_UNKNOWN.into(),
        );
    }

    /// Receipt lookup may obtain this server-only binding while writes are
    /// frozen. Only lock_and_validate may admit a new external mutation.
    pub fn zone_economy_write_permit(&self) -> Result<ZoneEconomyWritePermit, String> {
        self.validate_zone_save_fence_binding()?;
        if self.zone_save_operation.is_some() {
            return Err(REJECTED.into());
        }
        let fence = self.zone_save_fence.clone().ok_or(REJECTED)?;
        let source_database_url = self.account_store_database_url.clone().ok_or(REJECTED)?;
        Ok(ZoneEconomyWritePermit {
            source_config: self.clone(),
            source_database_url,
            fence,
        })
    }
}

#[cfg(test)]
#[path = "zone_economy_fence_tests.rs"]
mod tests;
