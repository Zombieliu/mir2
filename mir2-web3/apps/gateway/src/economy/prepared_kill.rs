//! A trusted source participant in the *same* transaction as a kill's ledger.
//! There is deliberately no wire/serde constructor for the write capability.
use super::*;
mod runtime_bridge;

impl PostgresEconomyStore {
    /// Recovery precedes private gameplay preparation, including all random
    /// selections. This path can read a committed award after its old lease dies.
    pub(super) fn lookup_prepared_kill(
        &self,
        envelope: &EconomyTransactionEnvelope,
    ) -> Result<Option<(EconomyTransactionReceipt, PreparedKillProjection)>, String> {
        envelope.validate_inner(true)?;
        if envelope.metadata.get("operation").map(String::as_str)
            != Some("preparedMonsterKillExperience")
        {
            return Err("prepared kill lookup requires its exact operation".into());
        }
        let mut client = self.connect()?;
        mir2_simulation::apply_migrations(&mut client)?;
        let mut tx = client.transaction().map_err(|e| e.to_string())?;
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
            &[&envelope.idempotency_key],
        )
        .map_err(|e| e.to_string())?;
        let row = tx.query_opt("SELECT tx.receipt,outbox.payload FROM game_economy_transactions tx JOIN game_economy_outbox outbox ON outbox.event_id=tx.event_id WHERE tx.idempotency_key=$1",
            &[&envelope.idempotency_key]).map_err(|e| e.to_string())?;
        row.map(|row| {
            let stored: EconomyTransactionReceipt =
                serde_json::from_value(row.get(0)).map_err(|e| e.to_string())?;
            let event: EconomyOutboxEvent =
                serde_json::from_value(row.get(1)).map_err(|e| e.to_string())?;
            let receipt = duplicate_receipt_from_stored(&event, &stored, envelope)?;
            Ok((
                receipt,
                event
                    .source_checkpoint
                    .ok_or("prepared kill checkpoint missing")?,
            ))
        })
        .transpose()
    }
}

pub(super) trait PreparedKillSource {
    fn identity(&self) -> &ActiveSessionIdentity;
    fn award_hash(&self) -> &str;
    /// Derived from the complete before/after source, not request parameters.
    fn expected_legs(&self) -> &[EconomyLeg];
    fn expected_before(&self) -> &BTreeMap<EconomyBalanceKey, i64>;
    /// Lock sorted Guild rows before the economy character advisory locks.
    fn lock_guilds(&self, transaction: &mut Transaction<'_>) -> Result<(), String>;
    /// CAS the entire source checkpoint and Guild receipts, using this transaction.
    fn publish_source(
        &self,
        transaction: &mut Transaction<'_>,
    ) -> Result<PreparedKillProjection, String>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedKillProjection {
    identity: ActiveSessionIdentity,
    award_hash: String,
    // Keep Eq on the existing outbox API; validate the typed save on both boundaries.
    checkpoint: serde_json::Value,
}

impl PreparedKillProjection {
    pub(super) fn new(
        identity: ActiveSessionIdentity,
        award_hash: String,
        checkpoint: &mir2_simulation::CharacterSaveRecord,
    ) -> Result<Self, String> {
        let result = Self {
            identity,
            award_hash,
            checkpoint: serde_json::to_value(checkpoint).map_err(|e| e.to_string())?,
        };
        result.validate()?;
        Ok(result)
    }
    fn validate(&self) -> Result<(), String> {
        EconomyBalanceKey::experience(&self.identity.account_id, self.identity.character_index)
            .validate()?;
        if self.award_hash.len() != 64 || !self.award_hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("invalid prepared kill award hash".into());
        }
        let checkpoint: mir2_simulation::CharacterSaveRecord =
            serde_json::from_value(self.checkpoint.clone())
                .map_err(|e| format!("invalid prepared kill checkpoint: {e}"))?;
        if checkpoint.character.index != self.identity.character_index {
            return Err("prepared kill checkpoint character mismatch".into());
        }
        Ok(())
    }
    pub(super) fn digest(&self) -> Result<String, String> {
        self.validate()?;
        let mut hash = Sha256::new();
        hash.update(b"mir2.prepared-kill-checkpoint.v1\0");
        hash.update(serde_json::to_vec(self).map_err(|e| e.to_string())?);
        Ok(hex_lower(&hash.finalize()))
    }
}

pub(super) fn validate_source(
    envelope: &EconomyTransactionEnvelope,
    source: &dyn PreparedKillSource,
) -> Result<(), String> {
    envelope.validate_inner(true)?;
    if envelope.transaction_kind != EconomyTransactionKind::Adjustment
        || envelope.metadata.get("operation").map(String::as_str)
            != Some("preparedMonsterKillExperience")
        || envelope.metadata.get("awardHash").map(String::as_str) != Some(source.award_hash())
        || envelope.metadata.get("sourceAccount").map(String::as_str)
            != Some(source.identity().account_id.as_str())
        || envelope.metadata.get("sourceCharacter")
            != Some(&source.identity().character_index.to_string())
        || envelope.legs != source.expected_legs()
        || envelope.legs.iter().any(|leg| {
            leg.balance.account_id != source.identity().account_id
                || leg.balance.character_index != source.identity().character_index
        })
    {
        return Err("prepared kill source does not bind the economy envelope".into());
    }
    EconomyBalanceKey::experience(
        &source.identity().account_id,
        source.identity().character_index,
    )
    .validate()?;
    let keys = envelope
        .legs
        .iter()
        .map(|leg| leg.balance.clone())
        .collect::<BTreeSet<_>>();
    if keys != source.expected_before().keys().cloned().collect()
        || source.expected_before().values().any(|amount| *amount < 0)
    {
        return Err("prepared kill source balance baseline mismatch".into());
    }
    if source.award_hash().len() != 64
        || !source.award_hash().bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("invalid prepared kill award hash".into());
    }
    Ok(())
}

pub(super) fn lock_live_zone_lease(
    transaction: &mut Transaction<'_>,
    envelope: &EconomyTransactionEnvelope,
) -> Result<(), String> {
    let generation = i64::try_from(envelope.fencing_generation)
        .map_err(|_| "Zone fencing generation overflow")?;
    // Read clock_timestamp *after* obtaining the row lock. A waiter must not use
    // the time it started waiting as proof that the lease is still alive.
    let row = transaction
        .query_opt(
            "SELECT fencing_token,expires_at_ms FROM zone_owner_leases WHERE zone_id=$1 FOR UPDATE",
            &[&envelope.zone_id],
        )
        .map_err(|e| format!("prepared kill lease lock failed: {e}"))?
        .ok_or("prepared kill requires an authoritative Zone lease")?;
    let now: i64 = transaction
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
            &[],
        )
        .map_err(|e| e.to_string())?
        .get(0);
    if row.get::<_, i64>(0) != generation || row.get::<_, i64>(1) <= now {
        return Err("prepared kill Zone lease is expired or fenced".into());
    }
    Ok(())
}

pub(super) fn validate_projection(
    envelope: &EconomyTransactionEnvelope,
    projection: &PreparedKillProjection,
) -> Result<(), String> {
    projection.validate()?;
    if envelope.metadata.get("awardHash") != Some(&projection.award_hash)
        || envelope.metadata.get("sourceAccount") != Some(&projection.identity.account_id)
        || envelope.metadata.get("sourceCharacter")
            != Some(&projection.identity.character_index.to_string())
    {
        return Err("prepared kill committed checkpoint identity mismatch".into());
    }
    let checkpoint: mir2_simulation::CharacterSaveRecord =
        serde_json::from_value(projection.checkpoint.clone()).map_err(|e| e.to_string())?;
    if checkpoint
        .guild_experience_journal
        .applied_kill_receipts
        .get(&envelope.idempotency_key)
        != Some(&projection.award_hash)
    {
        return Err("prepared kill checkpoint lacks the exact source receipt".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
