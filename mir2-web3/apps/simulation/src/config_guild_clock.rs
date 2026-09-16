//! Guild minute-clock fencing rules. Repository/driver integration is deliberately
//! separate: a clock transition must be committed with every affected guild.
use super::*;

pub(crate) const GUILD_MINUTE_MS: u64 = 60_000;
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GuildClockRecord {
    pub generation: u64,
    pub owner_token: Option<String>,
    pub lease_expires_ms: u64,
    pub minute_anchor_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GuildClockLease {
    pub owner_token: String,
    pub generation: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GuildClockAdvance {
    pub next: GuildClockRecord,
    pub admitted_minutes: u64,
}
impl GuildClockRecord {
    pub fn lease(&self) -> Option<GuildClockLease> {
        Some(GuildClockLease {
            owner_token: self.owner_token.clone()?,
            generation: self.generation,
        })
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.generation > i64::MAX as u64
            || self.lease_expires_ms > i64::MAX as u64
            || self.minute_anchor_ms > i64::MAX as u64
        {
            return Err("guild clock generation exhausted".into());
        }
        match &self.owner_token {
            Some(token)
                if token.len() == 32
                    && token.bytes().all(|byte| byte.is_ascii_hexdigit())
                    && self.generation > 0
                    && self.lease_expires_ms > self.minute_anchor_ms =>
            {
                Ok(())
            }
            None if self.lease_expires_ms == 0 => Ok(()),
            _ => Err("invalid guild clock lease state".into()),
        }
    }
    /// `now_ms` is supplied only by the repository's authoritative clock. PG
    /// reads clock_timestamp(); File's single owner supplies its process clock.
    pub fn acquire(&self, owner_token: &str, now_ms: u64, ttl_ms: u64) -> Result<Self, String> {
        self.validate()?;
        if ttl_ms == 0 {
            return Err("guild clock lease TTL must be positive".into());
        }
        let expires = now_ms
            .checked_add(ttl_ms)
            .ok_or("guild clock lease time exhausted")?;
        if self.owner_token.is_some() && self.lease_expires_ms > now_ms {
            if self.owner_token.as_deref() != Some(owner_token) {
                return Err("guild clock lease busy".into());
            }
            let mut renewed = self.clone();
            renewed.lease_expires_ms = renewed.lease_expires_ms.max(expires);
            renewed.validate()?;
            return Ok(renewed);
        }
        let next = Self {
            generation: self
                .generation
                .checked_add(1)
                .filter(|value| *value <= i64::MAX as u64)
                .ok_or("guild clock generation exhausted")?,
            owner_token: Some(owner_token.into()),
            lease_expires_ms: expires,
            // A takeover never charges the previous owner's offline interval.
            minute_anchor_ms: now_ms,
        };
        next.validate()?;
        Ok(next)
    }
    pub fn advance(
        &self,
        lease: &GuildClockLease,
        now_ms: u64,
        ttl_ms: u64,
    ) -> Result<GuildClockAdvance, String> {
        if self.lease_expires_ms <= now_ms {
            return Err("guild clock lease lost".into());
        }
        self.advance_retained(lease, now_ms, ttl_ms)
    }
    /// Only a coordinator retaining this exact successful owner/generation may
    /// use this while holding the repository's exclusive clock lock. Expiry
    /// permits another owner to acquire; it does not itself prove takeover.
    pub(in crate::config) fn advance_retained(
        &self,
        lease: &GuildClockLease,
        now_ms: u64,
        ttl_ms: u64,
    ) -> Result<GuildClockAdvance, String> {
        self.validate()?;
        if ttl_ms == 0 {
            return Err("guild clock lease TTL must be positive".into());
        }
        if self.lease() != Some(lease.clone()) {
            return Err("guild clock lease lost".into());
        }
        // Crystal Envir processes one global guild interval, then schedules
        // Time + Minute. A delayed loop does not back-charge missed intervals.
        let admitted_minutes =
            u64::from(now_ms.saturating_sub(self.minute_anchor_ms) >= GUILD_MINUTE_MS);
        let mut next = self.clone();
        if admitted_minutes == 1 {
            next.minute_anchor_ms = now_ms;
        }
        next.lease_expires_ms = next.lease_expires_ms.max(
            now_ms
                .checked_add(ttl_ms)
                .ok_or("guild clock lease time exhausted")?,
        );
        next.validate()?;
        Ok(GuildClockAdvance {
            next,
            admitted_minutes,
        })
    }
    /// Restore business time while never restoring an old fencing token. The
    /// repository must additionally CAS the successful commit receipt versions.
    pub fn invalidate_preserving_anchor(&self, generation_floor: u64) -> Result<Self, String> {
        Ok(Self {
            generation: self
                .generation
                .max(generation_floor)
                .checked_add(1)
                .filter(|value| *value <= i64::MAX as u64)
                .ok_or("guild clock generation exhausted")?,
            owner_token: None,
            lease_expires_ms: 0,
            minute_anchor_ms: self.minute_anchor_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    #[test]
    fn guild_clock_retained_lease_never_advances_a_reused_owner_generation() {
        let first = GuildClockRecord::default().acquire(A, 1000, 30000).unwrap();
        let lease = first.lease().unwrap();
        let late = first.advance_retained(&lease, 61000, 30000).unwrap();
        assert_eq!(late.admitted_minutes, 1);
        assert_eq!(late.next.generation, first.generation);
        let replacement = first.acquire(A, 61000, 30000).unwrap();
        assert!(replacement.generation > lease.generation);
        assert!(replacement.advance_retained(&lease, 62000, 30000).is_err());
    }
    #[test]
    fn guild_clock_takeover_reanchors_and_fences_every_old_generation() {
        let original = GuildClockRecord::default()
            .acquire(A, 1000, 120000)
            .unwrap();
        let old_lease = original.lease().unwrap();
        assert!(original.acquire(B, 2000, 120000).is_err());
        let renewed = original.acquire(A, 61000, 120000).unwrap();
        assert_eq!(renewed.minute_anchor_ms, 1000);
        let advanced = renewed.advance(&old_lease, 61000, 120000).unwrap();
        assert_eq!(advanced.admitted_minutes, 1);
        assert_eq!(
            advanced
                .next
                .advance(&old_lease, 61000, 120000)
                .unwrap()
                .admitted_minutes,
            0
        );
        let takeover = advanced.next.acquire(B, 9_000_000, 120000).unwrap();
        assert_eq!(takeover.minute_anchor_ms, 9_000_000);
        assert!(takeover.advance(&old_lease, 9_000_001, 120000).is_err());
        assert_eq!(
            takeover
                .advance(&takeover.lease().unwrap(), 9_000_001, 120000)
                .unwrap()
                .admitted_minutes,
            0
        );
        assert!(takeover
            .advance(&takeover.lease().unwrap(), 9_120_000, 120000)
            .is_err());
    }
    #[test]
    fn guild_clock_compensation_restores_anchor_without_reviving_token() {
        let before = GuildClockRecord::default()
            .acquire(A, 1000, 120000)
            .unwrap();
        let failed = before
            .advance(&before.lease().unwrap(), 61000, 120000)
            .unwrap();
        let restored = before
            .invalidate_preserving_anchor(failed.next.generation)
            .unwrap();
        assert_eq!(restored.minute_anchor_ms, before.minute_anchor_ms);
        assert!(restored.generation > failed.next.generation);
        assert!(restored.owner_token.is_none());
        assert!(restored
            .advance(&before.lease().unwrap(), 61001, 120000)
            .is_err());
        let recovered = restored.acquire(B, 62000, 120000).unwrap();
        assert!(recovered.generation > restored.generation);
        assert_eq!(recovered.minute_anchor_ms, 62000);
        assert!(GuildClockRecord {
            generation: i64::MAX as u64,
            ..Default::default()
        }
        .acquire(A, 0, 1)
        .is_err());
    }
}

/// A snapshot read while holding the database singleton row lock. Callers must
/// take this lock before any guild/account lock and retain the same transaction
/// through the guild writes and commit. No public client packet constructs it.
#[derive(Debug)]
pub(super) struct LockedPostgresGuildClock {
    pub record: GuildClockRecord,
    pub version: i64,
    pub now_ms: u64,
}
impl LockedPostgresGuildClock {
    pub fn lock(transaction: &mut Transaction<'_>) -> Result<Self, String> {
        let row=transaction.query_one("SELECT generation,owner_token,lease_expires_ms,minute_anchor_ms,store_version FROM shared_guild_clock WHERE singleton=TRUE FOR UPDATE",&[]).map_err(|e|format!("guild clock lock failed: {e}"))?;
        let nonnegative = |field: &str| -> Result<u64, String> {
            u64::try_from(row.get::<_, i64>(field))
                .map_err(|_| format!("negative guild clock {field}"))
        };
        let record = GuildClockRecord {
            generation: nonnegative("generation")?,
            owner_token: row.get("owner_token"),
            lease_expires_ms: nonnegative("lease_expires_ms")?,
            minute_anchor_ms: nonnegative("minute_anchor_ms")?,
        };
        record.validate()?;
        let version: i64 = row.get("store_version");
        if version <= 0 {
            return Err("invalid guild clock store version".into());
        }
        // Read time after acquiring the lock: time spent waiting for another
        // transaction must not turn an already expired lease into a valid one.
        let row = transaction
            .query_one(
                "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
                &[],
            )
            .map_err(|e| format!("guild clock database time failed: {e}"))?;
        let now_ms = u64::try_from(row.get::<_, i64>(0))
            .map_err(|_| "negative guild clock database time")?;
        Ok(Self {
            record,
            version,
            now_ms,
        })
    }
    fn write(
        &self,
        transaction: &mut Transaction<'_>,
        next: &GuildClockRecord,
    ) -> Result<i64, String> {
        next.validate()?;
        let version = self
            .version
            .checked_add(1)
            .ok_or("guild clock version exhausted")?;
        let count=transaction.execute("UPDATE shared_guild_clock SET generation=$1,owner_token=$2,lease_expires_ms=$3,minute_anchor_ms=$4,store_version=$5,updated_at=clock_timestamp() WHERE singleton=TRUE AND store_version=$6",&[&(next.generation as i64),&next.owner_token,&(next.lease_expires_ms as i64),&(next.minute_anchor_ms as i64),&version,&self.version]).map_err(|e|format!("guild clock write failed: {e}"))?;
        if count != 1 {
            return Err("stale guild clock commit receipt".into());
        }
        Ok(version)
    }
    pub fn acquire(
        &self,
        transaction: &mut Transaction<'_>,
        owner: &str,
        ttl_ms: u64,
    ) -> Result<(GuildClockRecord, i64), String> {
        let next = self.record.acquire(owner, self.now_ms, ttl_ms)?;
        let version = self.write(transaction, &next)?;
        Ok((next, version))
    }
    pub fn advance(
        &self,
        transaction: &mut Transaction<'_>,
        lease: &GuildClockLease,
        ttl_ms: u64,
    ) -> Result<(GuildClockAdvance, i64), String> {
        let advance = self.record.advance(lease, self.now_ms, ttl_ms)?;
        let version = self.write(transaction, &advance.next)?;
        Ok((advance, version))
    }
    pub(in crate::config) fn advance_retained(
        &self,
        transaction: &mut Transaction<'_>,
        lease: &GuildClockLease,
        ttl_ms: u64,
    ) -> Result<(GuildClockAdvance, i64), String> {
        let advance = self.record.advance_retained(lease, self.now_ms, ttl_ms)?;
        let version = self.write(transaction, &advance.next)?;
        Ok((advance, version))
    }
    /// A failed mirror may restore the old business anchor only while the exact
    /// receipt is still current. Higher generation fences all attempted owners;
    /// the caller must CAS/restore the matching guild receipt versions as well.
    pub fn compensate(
        &self,
        transaction: &mut Transaction<'_>,
        expected_receipt: i64,
        before: &GuildClockRecord,
    ) -> Result<(GuildClockRecord, i64), String> {
        if self.version != expected_receipt {
            return Err("stale guild clock compensation receipt".into());
        }
        let next = before.invalidate_preserving_anchor(self.record.generation)?;
        let version = self.write(transaction, &next)?;
        Ok((next, version))
    }
    pub fn invalidate_for_restore(
        &self,
        transaction: &mut Transaction<'_>,
        restored_business: &GuildClockRecord,
    ) -> Result<(GuildClockRecord, i64), String> {
        let next = restored_business.invalidate_preserving_anchor(self.record.generation)?;
        let version = self.write(transaction, &next)?;
        Ok((next, version))
    }
}

#[cfg(test)]
#[path = "config_guild_clock_pg_tests.rs"]
mod postgres_tests;

#[path = "config_guild_clock_transactions.rs"]
pub(super) mod transactions;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GuildClockSourceVersion {
    pub record: GuildClockRecord,
    pub version: i64,
}
pub(super) fn validate_store(store: &AccountStore) -> Result<(), String> {
    if store.schema_version > ACCOUNT_STORE_SCHEMA_VERSION {
        return Err("unsupported future account-store schema".into());
    }
    if store.schema_version < 4 && store.guild_clock.is_some() {
        return Err("legacy account store cannot contain guild clock authority".into());
    }
    if let Some(clock) = &store.guild_clock {
        clock.validate()?;
    }
    Ok(())
}
pub(super) fn validate_store_scope(
    original: &AccountStore,
    staged: &AccountStore,
    scope: AccountStoreMutationScope<'_>,
) -> Result<(), AccountStoreTransactionScopeError> {
    validate_store(staged).map_err(AccountStoreTransactionScopeError::InvalidGuildState)?;
    if !matches!(scope, AccountStoreMutationScope::FullRestore) {
        if original.guild_clock != staged.guild_clock {
            return Err(AccountStoreTransactionScopeError::SourceMetadataChanged {
                field: "guild_clock",
            });
        }
        if original.source_guild_clock_version != staged.source_guild_clock_version {
            return Err(AccountStoreTransactionScopeError::SourceMetadataChanged {
                field: "source_guild_clock_version",
            });
        }
    }
    Ok(())
}

pub(super) fn load_postgres(
    client: &mut impl postgres::GenericClient,
) -> Result<GuildClockSourceVersion, String> {
    let row=client.query_one("SELECT generation,owner_token,lease_expires_ms,minute_anchor_ms,store_version FROM shared_guild_clock WHERE singleton=TRUE",&[]).map_err(|e|format!("guild clock load failed: {e}"))?;
    let nonnegative = |field: &str| {
        u64::try_from(row.get::<_, i64>(field)).map_err(|_| format!("negative guild clock {field}"))
    };
    let record = GuildClockRecord {
        generation: nonnegative("generation")?,
        owner_token: row.get("owner_token"),
        lease_expires_ms: nonnegative("lease_expires_ms")?,
        minute_anchor_ms: nonnegative("minute_anchor_ms")?,
    };
    record.validate()?;
    let version: i64 = row.get("store_version");
    if version <= 0 {
        return Err("invalid guild clock source version".into());
    }
    Ok(GuildClockSourceVersion { record, version })
}
#[derive(Debug, Clone)]
pub(super) enum GuildClockMutation {
    Invalidate {
        business: GuildClockRecord,
    },
    Compensate {
        business: GuildClockRecord,
        expected_clock_version: i64,
    },
}
impl GuildClockMutation {
    pub fn write(
        &self,
        transaction: &mut Transaction<'_>,
    ) -> Result<GuildClockSourceVersion, String> {
        let clock = LockedPostgresGuildClock::lock(transaction)?;
        let (record, version) = match self {
            Self::Invalidate { business } => clock.invalidate_for_restore(transaction, business)?,
            Self::Compensate {
                business,
                expected_clock_version,
            } => clock.compensate(transaction, *expected_clock_version, business)?,
        };
        Ok(GuildClockSourceVersion { record, version })
    }
}

#[cfg(test)]
#[path = "config_guild_clock_schema_tests.rs"]
pub(super) mod schema_tests;

pub(super) const CLOCK_COMMIT_OUTCOME_UNKNOWN: &str = "GUILD_CLOCK_COMMIT_OUTCOME_UNKNOWN";
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GuildClockTransactionError {
    NotCommitted(String),
    CommitOutcomeUnknown(String),
}
impl std::fmt::Display for GuildClockTransactionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotCommitted(reason) => write!(f, "{reason}"),
            Self::CommitOutcomeUnknown(reason) => {
                write!(f, "{CLOCK_COMMIT_OUTCOME_UNKNOWN}: {reason}")
            }
        }
    }
}
impl std::error::Error for GuildClockTransactionError {}
impl From<String> for GuildClockTransactionError {
    fn from(reason: String) -> Self {
        Self::NotCommitted(reason)
    }
}
impl From<&str> for GuildClockTransactionError {
    fn from(reason: &str) -> Self {
        Self::NotCommitted(reason.into())
    }
}
pub(super) fn classify_commit_error(error: postgres::Error) -> GuildClockTransactionError {
    // A PostgreSQL ERROR response to COMMIT means the transaction aborted.
    // A transport/protocol failure cannot prove whether COMMIT took effect.
    if error.as_db_error().is_some() {
        GuildClockTransactionError::NotCommitted(format!("guild clock commit rejected: {error}"))
    } else {
        GuildClockTransactionError::CommitOutcomeUnknown(format!(
            "guild clock commit response unavailable: {error}"
        ))
    }
}
