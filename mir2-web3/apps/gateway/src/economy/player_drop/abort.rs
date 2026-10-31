//! A successor may revoke an unpublished Prepared operation after fencing its
//! original writer. This records evidence; it never restores old character data.
use super::*;

fn canonical_digest(domain: &[u8], value: &serde_json::Value) -> Result<String, String> {
    fn canonical(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(values) => serde_json::Value::Object(
                values
                    .into_iter()
                    .collect::<BTreeMap<_, _>>()
                    .into_iter()
                    .map(|(key, value)| (key, canonical(value)))
                    .collect(),
            ),
            serde_json::Value::Array(values) => {
                serde_json::Value::Array(values.into_iter().map(canonical).collect())
            }
            value => value,
        }
    }
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(serde_json::to_vec(&canonical(value.clone())).map_err(|e| e.to_string())?);
    Ok(hex_lower(&digest.finalize()))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerDropAbortAudit {
    version: u8,
    recovery_id: String,
    reason: String,
    operation_digest: String,
    recovery_owner_id: String,
    recovery_epoch: u64,
    aborted_at_ms: u64,
    unpublished_event_id: String,
    canonical_source_relation: String,
    canonical_checkpoint: serde_json::Value,
    canonical_checkpoint_digest: String,
    canonical_account_digest: String,
    account_store_version: i64,
    character_save_version: i64,
}
impl PlayerDropAbortAudit {
    pub fn recovery_id(&self) -> &str {
        &self.recovery_id
    }
    pub fn reason(&self) -> &str {
        &self.reason
    }
    pub fn recovery_owner_id(&self) -> &str {
        &self.recovery_owner_id
    }
    pub fn recovery_epoch(&self) -> u64 {
        self.recovery_epoch
    }
    pub fn aborted_at_ms(&self) -> u64 {
        self.aborted_at_ms
    }
    pub fn canonical_source_relation(&self) -> &str {
        &self.canonical_source_relation
    }
    pub fn canonical_checkpoint_digest(&self) -> &str {
        &self.canonical_checkpoint_digest
    }
    pub fn canonical_checkpoint(&self) -> Result<mir2_simulation::CharacterSaveRecord, String> {
        serde_json::from_value(self.canonical_checkpoint.clone()).map_err(|e| e.to_string())
    }
    pub(super) fn digest(&self) -> Result<String, String> {
        canonical_digest(
            b"mir2.player-drop-abort-audit.v1\0",
            &serde_json::to_value(self).map_err(|e| e.to_string())?,
        )
    }
    pub(super) fn validate_operation(
        &self,
        operation: &PlayerDropPreparedOperation,
    ) -> Result<(), String> {
        operation.validate_integrity()?;
        validate_action_identity(&self.recovery_id, &self.reason)?;
        let (legs, _) = source_delta(operation)?;
        let event_id = source_envelope(operation, legs).event_id_inner(true)?;
        let canonical = self.canonical_checkpoint()?;
        let relation = canonical_relation(operation, &canonical, &event_id)?;
        let hex = |value: &str| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        if self.version != 1
            || self.operation_digest != operation.operation_digest()
            || self.recovery_epoch <= operation.binding().owner_epoch()
            || self.recovery_epoch > i64::MAX as u64
            || self.recovery_owner_id.trim().is_empty()
            || self.aborted_at_ms == 0
            || self.aborted_at_ms > i64::MAX as u64
            || self.unpublished_event_id != event_id
            || self.canonical_source_relation != relation
            || self.canonical_checkpoint_digest
                != canonical_digest(
                    b"mir2.player-drop-abort-canonical-checkpoint.v1\0",
                    &self.canonical_checkpoint,
                )?
            || !hex(&self.canonical_account_digest)
            || self.account_store_version < 0
            || self.character_save_version < 0
        {
            return Err("player drop abort audit does not bind its exact operation and canonical source evidence".into());
        }
        Ok(())
    }
}

fn validate_action_identity(recovery_id: &str, reason: &str) -> Result<(), String> {
    if recovery_id.trim() != recovery_id
        || recovery_id.is_empty()
        || recovery_id.len() > 512
        || recovery_id.chars().any(char::is_control)
        || reason.trim() != reason
        || reason.is_empty()
        || reason.len() > 4096
        || reason.chars().any(char::is_control)
    {
        return Err("invalid player drop abort recovery identity or reason".into());
    }
    Ok(())
}

fn canonical_relation(
    operation: &PlayerDropPreparedOperation,
    canonical: &mir2_simulation::CharacterSaveRecord,
    event_id: &str,
) -> Result<&'static str, String> {
    let before = operation.before();
    if canonical.character.index != before.character.index
        || canonical.character.name != before.character.name
        || canonical.character.class != before.character.class
        || canonical.character.gender != before.character.gender
        || canonical.revision < before.revision
    {
        return Err(
            "player drop abort canonical character incarnation or revision conflict".into(),
        );
    }
    let canonical = serde_json::to_value(canonical).map_err(|e| e.to_string())?;
    if canonical == serde_json::to_value(operation.checkpoint()).map_err(|e| e.to_string())? {
        return Err("player drop abort found its source after-image without a ledger receipt; explicit reconciliation required".into());
    }
    let save: mir2_simulation::CharacterSaveRecord =
        serde_json::from_value(canonical.clone()).map_err(|e| e.to_string())?;
    let stage5 = save
        .stage5_systems_json
        .as_deref()
        .map(serde_json::from_str::<mir2_simulation::Stage5SystemsState>)
        .transpose()
        .map_err(|e| e.to_string())?;
    if stage5
        .as_ref()
        .is_some_and(|state| state.economy_projection_event_ids.contains(event_id))
    {
        return Err("player drop abort found an original source publication marker".into());
    }
    if canonical == serde_json::to_value(before).map_err(|e| e.to_string())? {
        Ok("exactBefore")
    } else if save.revision > before.revision {
        Ok("newerCanonicalPreserved")
    } else {
        Err("player drop abort found a changed canonical source at the original revision".into())
    }
}

impl PostgresEconomyStore {
    /// Invoke only on a quiesced successor before promotion. The successor
    /// fences the old writer and retires this operation; it never authorizes a
    /// retry, compensating credit, UID reuse or a global source unfreeze.
    ///
    /// Later legitimate source progress is allowed and captured verbatim. Equal
    /// revision conflicts, an exact old after-image, or a publication marker
    /// without its atomic receipt remain unresolved and fail closed.
    pub fn abort_prepared_player_drop_operation(
        &self,
        permit: &ZoneEconomyWritePermit,
        binding: &PlayerDropOperationBinding,
        recovery_id: &str,
        reason: &str,
    ) -> Result<PlayerDropOperationRecord, String> {
        binding.validate_integrity()?;
        validate_action_identity(recovery_id, reason)?;
        permit.validate_target_database(&self.database_url)?;
        let fence = permit.fence();
        if fence.zone_id() != binding.zone_id() || fence.fencing_token() <= binding.owner_epoch() {
            return Err("player drop abort requires an authoritative successor epoch newer than the original writer".into());
        }
        let mut client = self.player_drop_connect()?;
        let mut tx = client.transaction().map_err(|e| e.to_string())?;
        zone_fence::bound_transaction(&mut tx)?;
        // Same idempotency mutex as source+ledger commit. Acquiring it proves a
        // previous attempt finished; the current lease then fences every late one.
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
            &[&binding.operation_key()],
        )
        .map_err(|e| e.to_string())?;
        permit.lock_and_validate(
            &mut tx,
            &self.database_url,
            binding.zone_id(),
            fence.fencing_token(),
        )?;
        let row=tx.query_opt("SELECT operation,status,event_id,committed_at_ms FROM public.zone_player_drop_operations WHERE source_namespace=current_schema() AND operation_key=$1 FOR UPDATE",
            &[&binding.operation_key()]).map_err(|e|e.to_string())?.ok_or("player drop abort requires its original durable Prepared row")?;
        let operation: PlayerDropPreparedOperation =
            serde_json::from_value(row.get(0)).map_err(|e| e.to_string())?;
        operation.validate_integrity()?;
        if operation.binding() != binding {
            return Err("player drop abort original binding conflict".into());
        }
        let status: String = row.get(1);
        if status == "Aborted" {
            let record = decode_record(
                tx.query_one(
                    &format!("{RECORD_QUERY} WHERE op.operation_key=$1"),
                    &[&binding.operation_key()],
                )
                .map_err(|e| e.to_string())?,
                connection_binding_digest(&self.database_url),
            )?;
            let audit = record
                .abort_audit()
                .ok_or("player drop abort duplicate lacks its immutable audit")?;
            if audit.recovery_id() != recovery_id
                || audit.reason() != reason
                || audit.recovery_owner_id() != fence.owner_id()
                || audit.recovery_epoch() != fence.fencing_token()
            {
                return Err("player drop abort recovery identity was already settled with different evidence".into());
            }
            return Ok(record);
        }
        if status != "Prepared"
            || row.get::<_, Option<String>>(2).is_some()
            || row.get::<_, Option<i64>>(3).is_some()
        {
            return Err("player drop abort cannot revoke a source-committed world effect".into());
        }
        let (legs, _) = source_delta(&operation)?;
        let event_id = source_envelope(&operation, legs).event_id_inner(true)?;
        let publication=tx.query_one("SELECT EXISTS(SELECT 1 FROM game_economy_transactions WHERE idempotency_key=$1 OR event_id=$2),EXISTS(SELECT 1 FROM game_economy_outbox WHERE idempotency_key=$1 OR event_id=$2)",
            &[&binding.operation_key(),&event_id]).map_err(|e|e.to_string())?;
        if publication.get::<_, bool>(0) || publication.get::<_, bool>(1) {
            return Err("player drop abort found an original atomic publication event".into());
        }
        // Read/lock both authoritative representations, never repair either by
        // copying this operation's older before/after data into a current save.
        let account = tx
            .query_opt(
                "SELECT raw_json,store_version FROM accounts WHERE account_id=$1 FOR UPDATE",
                &[&binding.account_id()],
            )
            .map_err(|e| e.to_string())?
            .ok_or("player drop abort canonical account missing")?;
        let raw_account: serde_json::Value = account.get(0);
        let typed_account: mir2_simulation::AccountRecord =
            serde_json::from_value(raw_account.clone()).map_err(|e| e.to_string())?;
        let canonical = typed_account
            .saves
            .get(&binding.character_index())
            .ok_or("player drop abort canonical character missing")?;
        let saved=tx.query_opt("SELECT snapshot_json,save_version FROM character_saves WHERE account_id=$1 AND character_index=$2 FOR UPDATE",&[&binding.account_id(),&binding.character_index()]).map_err(|e|e.to_string())?.ok_or("player drop abort canonical save projection missing")?;
        let projected: mir2_simulation::CharacterSaveRecord =
            serde_json::from_value(saved.get(0)).map_err(|e| e.to_string())?;
        let canonical_checkpoint = serde_json::to_value(canonical).map_err(|e| e.to_string())?;
        if serde_json::to_value(projected).map_err(|e| e.to_string())? != canonical_checkpoint {
            return Err("player drop abort canonical account/save projection conflict".into());
        }
        let aborted_at_ms: i64 = tx
            .query_one(
                "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
                &[],
            )
            .map_err(|e| e.to_string())?
            .get(0);
        let aborted_at_ms = u64::try_from(aborted_at_ms)
            .map_err(|_| "player drop abort database clock unavailable")?;
        let audit = PlayerDropAbortAudit {
            version: 1,
            recovery_id: recovery_id.into(),
            reason: reason.into(),
            operation_digest: operation.operation_digest().into(),
            recovery_owner_id: fence.owner_id().into(),
            recovery_epoch: fence.fencing_token(),
            aborted_at_ms,
            unpublished_event_id: event_id.clone(),
            canonical_source_relation: canonical_relation(&operation, canonical, &event_id)?.into(),
            canonical_checkpoint_digest: canonical_digest(
                b"mir2.player-drop-abort-canonical-checkpoint.v1\0",
                &canonical_checkpoint,
            )?,
            canonical_checkpoint,
            canonical_account_digest: canonical_digest(
                b"mir2.player-drop-abort-canonical-account.v1\0",
                &raw_account,
            )?,
            account_store_version: account.get(1),
            character_save_version: saved.get(1),
        };
        audit.validate_operation(&operation)?;
        let encoded = serde_json::to_value(&audit).map_err(|e| e.to_string())?;
        let digest = audit.digest()?;
        tx.execute("INSERT INTO public.zone_player_drop_abort_audit(operation_key,recovery_id,recovery_owner_id,recovery_epoch,audited_receipt,audit_digest) VALUES($1,$2,$3,$4,$5,$6)",
            &[&binding.operation_key(),&recovery_id,&fence.owner_id(),&(fence.fencing_token() as i64),&encoded,&digest]).map_err(|e|e.to_string())?;
        let changed=tx.execute("UPDATE public.zone_player_drop_operations SET status='Aborted',aborted_at_ms=$2,abort_audit=$3,abort_digest=$4 WHERE source_namespace=current_schema() AND operation_key=$1 AND status='Prepared'",
            &[&binding.operation_key(),&(audit.aborted_at_ms as i64),&encoded,&digest]).map_err(|e|e.to_string())?;
        if changed != 1 {
            return Err("player drop abort lost its exact Prepared operation".into());
        }
        let record = decode_record(
            tx.query_one(
                &format!("{RECORD_QUERY} WHERE op.operation_key=$1"),
                &[&binding.operation_key()],
            )
            .map_err(|e| e.to_string())?,
            connection_binding_digest(&self.database_url),
        )?;
        tx.commit().map_err(|_| permit.commit_outcome_unknown())?;
        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_drop_abort_records_newer_canonical_without_replaying_old_before_or_after() {
        let operation = super::super::tests::fixture();
        let (legs, _) = source_delta(&operation).unwrap();
        let event = source_envelope(&operation, legs)
            .event_id_inner(true)
            .unwrap();
        assert_eq!(
            canonical_relation(&operation, operation.before(), &event).unwrap(),
            "exactBefore"
        );
        let mut newer = operation.before().clone();
        newer.revision += 3;
        newer.gold += 7;
        assert_eq!(
            canonical_relation(&operation, &newer, &event).unwrap(),
            "newerCanonicalPreserved"
        );
        assert!(canonical_relation(&operation, operation.checkpoint(), &event).is_err());
        let mut conflict = operation.before().clone();
        conflict.gold += 1;
        assert!(canonical_relation(&operation, &conflict, &event).is_err());
        let mut marker = newer;
        let mut stage5 = mir2_simulation::Stage5SystemsState::default();
        stage5.economy_projection_event_ids.insert(event.clone());
        marker.stage5_systems_json = Some(serde_json::to_string(&stage5).unwrap());
        assert!(canonical_relation(&operation, &marker, &event).is_err());
    }

    #[test]
    fn player_drop_abort_audit_binds_original_operation_successor_and_canonical_evidence() {
        let operation = super::super::tests::fixture();
        let canonical = serde_json::to_value(operation.before()).unwrap();
        let (legs, _) = source_delta(&operation).unwrap();
        let event = source_envelope(&operation, legs)
            .event_id_inner(true)
            .unwrap();
        let audit = PlayerDropAbortAudit {
            version: 1,
            recovery_id: "recovery-1".into(),
            reason: "old writer fenced before publication".into(),
            operation_digest: operation.operation_digest().into(),
            recovery_owner_id: "owner-b".into(),
            recovery_epoch: 8,
            aborted_at_ms: 1000,
            unpublished_event_id: event,
            canonical_source_relation: "exactBefore".into(),
            canonical_checkpoint_digest: canonical_digest(
                b"mir2.player-drop-abort-canonical-checkpoint.v1\0",
                &canonical,
            )
            .unwrap(),
            canonical_checkpoint: canonical,
            canonical_account_digest: "a".repeat(64),
            account_store_version: 1,
            character_save_version: 1,
        };
        audit.validate_operation(&operation).unwrap();
        let original = audit.digest().unwrap();
        let mut old = audit.clone();
        old.recovery_epoch = 7;
        assert!(old.validate_operation(&operation).is_err());
        let mut changed = audit.clone();
        changed.canonical_checkpoint["gold"] = serde_json::json!(0);
        assert!(changed.validate_operation(&operation).is_err());
        let mut changed = audit;
        changed.recovery_id = "recovery-2".into();
        assert_ne!(changed.digest().unwrap(), original);
    }
}
