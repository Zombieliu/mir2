//! Durable server-only save capabilities. A checkpoint is frozen before Prepare;
//! its receipt is published by the account-store transaction, never a second tx.
use super::zone_save_fence::{self, ZoneSaveFence};
use super::{
    AccountStoreDatabaseMode, AccountStoreMutationPlan, AccountStoreSourceVersions,
    CharacterSaveRecord, SimulationConfig,
};
use postgres::{Client, Row, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const REJECTED: &str = "ZONE_SAVE_OPERATION_REJECTED";
pub const UNAVAILABLE: &str = "ZONE_SAVE_OPERATION_UNAVAILABLE";
/// A successful old receipt must be queried, not republished into live CAS state.
pub const ALREADY_COMMITTED: &str = "ZONE_SAVE_OPERATION_ALREADY_COMMITTED";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationStatus {
    Prepared,
    Committed,
    Released,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneSaveOperationBinding {
    operation_id: String,
    rpc_session_id: String,
    fence: ZoneSaveFence,
}
impl ZoneSaveOperationBinding {
    pub fn new(
        operation_id: String,
        rpc_session_id: String,
        fence: ZoneSaveFence,
    ) -> Result<Self, String> {
        if !valid_id(&operation_id) || !valid_id(&rpc_session_id) {
            return Err(REJECTED.into());
        }
        Ok(Self {
            operation_id,
            rpc_session_id,
            fence,
        })
    }
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }
    pub fn rpc_session_id(&self) -> &str {
        &self.rpc_session_id
    }
    pub fn fence(&self) -> &ZoneSaveFence {
        &self.fence
    }
    pub fn to_json(&self) -> Result<Value, String> {
        serde_json::to_value(WireBinding::from_binding(self)).map_err(|_| REJECTED.into())
    }
    pub fn from_json_checked(value: Value) -> Result<Self, String> {
        serde_json::from_value::<WireBinding>(value)
            .map_err(|_| REJECTED)?
            .into_binding()
    }
}

// These private wire descriptions are read-only RPC data, never account-store
// write permits. In particular ZoneSaveFence itself has no serde capability.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireBinding {
    operation_id: String,
    rpc_session_id: String,
    zone_id: String,
    owner_id: String,
    fencing_token: u64,
}
impl WireBinding {
    fn from_binding(binding: &ZoneSaveOperationBinding) -> Self {
        Self {
            operation_id: binding.operation_id.clone(),
            rpc_session_id: binding.rpc_session_id.clone(),
            zone_id: binding.fence.zone_id().into(),
            owner_id: binding.fence.owner_id().into(),
            fencing_token: binding.fence.fencing_token(),
        }
    }
    fn into_binding(self) -> Result<ZoneSaveOperationBinding, String> {
        ZoneSaveOperationBinding::new(
            self.operation_id,
            self.rpc_session_id,
            ZoneSaveFence::new(self.zone_id, self.owner_id, self.fencing_token)
                .map_err(|_| REJECTED)?,
        )
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireOperation {
    binding: WireBinding,
    account_id: String,
    frozen_checkpoint: Value,
    expected_saved_checkpoint: Value,
    checkpoint_digest: String,
    expected_digest: String,
    last_access_binary_datetime: Option<i64>,
}
impl WireOperation {
    fn from_operation(op: &ZoneSaveOperation) -> Self {
        Self {
            binding: WireBinding::from_binding(op.binding()),
            account_id: op.account_id.clone(),
            frozen_checkpoint: op.frozen_json.clone(),
            expected_saved_checkpoint: op.expected_json.clone(),
            checkpoint_digest: op.checkpoint_digest.clone(),
            expected_digest: op.expected_digest.clone(),
            last_access_binary_datetime: op.last_access_binary_datetime,
        }
    }
    fn into_operation(self) -> Result<ZoneSaveOperation, String> {
        if digest(&self.frozen_checkpoint)? != self.checkpoint_digest
            || digest(&self.expected_saved_checkpoint)? != self.expected_digest
        {
            return Err(REJECTED.into());
        }
        let op = ZoneSaveOperation::new(
            self.binding.into_binding()?,
            self.account_id,
            serde_json::from_value(self.frozen_checkpoint.clone()).map_err(|_| REJECTED)?,
            serde_json::from_value(self.expected_saved_checkpoint.clone()).map_err(|_| REJECTED)?,
            self.last_access_binary_datetime,
        )?;
        // Reject ignored/unknown save properties and schema shape changes too.
        if op.frozen_json != self.frozen_checkpoint
            || op.expected_json != self.expected_saved_checkpoint
        {
            return Err(REJECTED.into());
        }
        Ok(op)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireReceipt {
    operation: WireOperation,
    status: String,
    saved_checkpoint: Option<Value>,
    account_store_version: Option<i64>,
    character_save_version: Option<i64>,
}

/// Constructed only by authenticated authoritative server code. Deliberately
/// neither Serialize nor Deserialize: this is not a normal-client capability.
#[derive(Debug, Clone)]
pub struct ZoneSaveOperation {
    binding: ZoneSaveOperationBinding,
    account_id: String,
    frozen_checkpoint: CharacterSaveRecord,
    frozen_json: Value,
    checkpoint_digest: String,
    expected_saved_checkpoint: CharacterSaveRecord,
    expected_json: Value,
    expected_digest: String,
    last_access_binary_datetime: Option<i64>,
}
impl PartialEq for ZoneSaveOperation {
    fn eq(&self, other: &Self) -> bool {
        self.validate_same_operation(other).is_ok()
    }
}
impl Eq for ZoneSaveOperation {}

impl ZoneSaveOperation {
    /// The authoritative runtime derives `expected_saved_checkpoint` on a cloned
    /// store using the existing full-save/mail/guild logic, without publication.
    /// The actual plan must match that full result, including every attribute.
    pub fn new(
        binding: ZoneSaveOperationBinding,
        account_id: String,
        frozen_checkpoint: CharacterSaveRecord,
        expected_saved_checkpoint: CharacterSaveRecord,
        last_access_binary_datetime: Option<i64>,
    ) -> Result<Self, String> {
        if [&account_id, &frozen_checkpoint.character.name]
            .into_iter()
            .any(|v| !valid_id(v))
            || frozen_checkpoint.character.index < 0
            || frozen_checkpoint.revision.checked_add(1) != Some(expected_saved_checkpoint.revision)
            || frozen_checkpoint.character != expected_saved_checkpoint.character
        {
            return Err(REJECTED.into());
        }
        let frozen_json = serde_json::to_value(&frozen_checkpoint).map_err(|_| REJECTED)?;
        let checkpoint_digest = digest(&frozen_json)?;
        let expected_json =
            serde_json::to_value(&expected_saved_checkpoint).map_err(|_| REJECTED)?;
        let expected_digest = digest(&expected_json)?;
        Ok(Self {
            binding,
            account_id,
            frozen_checkpoint,
            frozen_json,
            checkpoint_digest,
            expected_saved_checkpoint,
            expected_json,
            expected_digest,
            last_access_binary_datetime,
        })
    }
    pub fn binding(&self) -> &ZoneSaveOperationBinding {
        &self.binding
    }
    pub fn operation_id(&self) -> &str {
        self.binding.operation_id()
    }
    pub fn rpc_session_id(&self) -> &str {
        self.binding.rpc_session_id()
    }
    pub fn fence(&self) -> &ZoneSaveFence {
        self.binding.fence()
    }
    pub fn account_id(&self) -> &str {
        &self.account_id
    }
    pub fn character_index(&self) -> i32 {
        self.frozen_checkpoint.character.index
    }
    pub fn frozen_checkpoint(&self) -> &CharacterSaveRecord {
        &self.frozen_checkpoint
    }
    pub fn expected_saved_checkpoint(&self) -> &CharacterSaveRecord {
        &self.expected_saved_checkpoint
    }
    pub fn checkpoint_digest(&self) -> &str {
        &self.checkpoint_digest
    }
    pub fn last_access_binary_datetime(&self) -> Option<i64> {
        self.last_access_binary_datetime
    }

    fn validate_plan(
        &self,
        plan: &AccountStoreMutationPlan,
        mode: AccountStoreDatabaseMode,
    ) -> Result<(), String> {
        if mode != AccountStoreDatabaseMode::SourceOfTruth
            || plan.zone_save_fence.as_ref() != Some(self.fence())
            || plan.accounts.len() != 1
            || plan.global_stock.is_some()
            || plan.clock.is_some()
            || plan.stones.is_some()
            || !plan.conquests.is_empty()
            || !plan.heroes.is_empty()
            || plan.hero_allocator.is_some()
        {
            return Err(REJECTED.into());
        }
        let mutation = plan.accounts.get(&self.account_id).ok_or(REJECTED)?;
        let before = mutation.original_account.as_ref().ok_or(REJECTED)?;
        let after = mutation.desired_account.as_ref().ok_or(REJECTED)?;
        let index = self.character_index();
        let old_save = before.saves.get(&index).ok_or(REJECTED)?;
        let saved = after.saves.get(&index).ok_or(REJECTED)?;
        if old_save.revision != self.frozen_checkpoint.revision
            || old_save.character.index != index
            || old_save.character.name != self.frozen_checkpoint.character.name
            || serde_json::to_value(saved).map_err(|_| REJECTED)? != self.expected_json
            || mutation.expected_version.is_none()
            || mutation
                .saves
                .get(&index)
                .is_none_or(|m| m.expected_version.is_none())
            || before.saves.len() != after.saves.len()
        {
            return Err(REJECTED.into());
        }
        for (other, save) in &before.saves {
            if *other != index
                && after
                    .saves
                    .get(other)
                    .map(serde_json::to_value)
                    .transpose()
                    .map_err(|_| REJECTED)?
                    != Some(serde_json::to_value(save).map_err(|_| REJECTED)?)
            {
                return Err(REJECTED.into());
            }
        }
        let expected_access = self.last_access_binary_datetime.or_else(|| {
            before
                .character_last_access_binary_datetimes
                .get(&index)
                .copied()
        });
        if after
            .character_last_access_binary_datetimes
            .get(&index)
            .copied()
            != expected_access
            || !after.characters.iter().any(|c| c == &saved.character)
        {
            return Err(REJECTED.into());
        }
        Ok(())
    }

    pub(super) fn lock_prepared_for_commit(
        &self,
        tx: &mut Transaction<'_>,
        plan: &AccountStoreMutationPlan,
        mode: AccountStoreDatabaseMode,
    ) -> Result<(), String> {
        self.validate_plan(plan, mode)?;
        let receipt = read_receipt(tx, self.binding(), true)?.ok_or(REJECTED)?;
        self.validate_same_operation(&receipt.operation)?;
        match receipt.status {
            OperationStatus::Prepared => Ok(()),
            OperationStatus::Committed | OperationStatus::Released => Err(ALREADY_COMMITTED.into()),
        }
    }

    pub(super) fn write_committed_receipt(
        &self,
        tx: &mut Transaction<'_>,
        versions: &AccountStoreSourceVersions,
    ) -> Result<(), String> {
        // Read the actual source rows after all normal save/projection writes.
        // Evidence and the snapshot remain atomic with those writes on COMMIT.
        let row = tx
            .query_one(
                "SELECT a.store_version,s.save_version,s.snapshot_json \
            FROM accounts a JOIN character_saves s USING(account_id) \
            WHERE a.account_id=$1 AND s.character_index=$2",
                &[&self.account_id, &self.character_index()],
            )
            .map_err(|_| UNAVAILABLE)?;
        let account_version: i64 = row.try_get(0).map_err(|_| UNAVAILABLE)?;
        let save_version: i64 = row.try_get(1).map_err(|_| UNAVAILABLE)?;
        let saved: Value = row.try_get(2).map_err(|_| UNAVAILABLE)?;
        if saved != self.expected_json
            || versions.accounts.get(&self.account_id) != Some(&account_version)
            || versions
                .saves
                .get(&self.account_id)
                .and_then(|s| s.get(&self.character_index()))
                != Some(&save_version)
            || account_version <= 0
            || save_version <= 0
        {
            return Err(REJECTED.into());
        }
        let changed = tx
            .execute(
                "UPDATE public.zone_character_save_operations \
            SET status='Committed',saved_checkpoint=$2,account_store_version=$3, \
            character_save_version=$4,committed_at_ms=\
            floor(extract(epoch FROM clock_timestamp())*1000)::bigint \
            WHERE operation_id=$1 AND status='Prepared'",
                &[
                    &self.operation_id(),
                    &saved,
                    &account_version,
                    &save_version,
                ],
            )
            .map_err(|_| UNAVAILABLE)?;
        if changed != 1 {
            return Err(REJECTED.into());
        }
        Ok(())
    }

    fn validate_same_operation(&self, other: &Self) -> Result<(), String> {
        if self.binding != other.binding
            || self.account_id != other.account_id
            || self.frozen_json != other.frozen_json
            || self.checkpoint_digest != other.checkpoint_digest
            || self.expected_json != other.expected_json
            || self.expected_digest != other.expected_digest
            || self.last_access_binary_datetime != other.last_access_binary_datetime
        {
            return Err(REJECTED.into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ZoneSaveOperationReceipt {
    operation: ZoneSaveOperation,
    status: OperationStatus,
    saved_checkpoint: Option<CharacterSaveRecord>,
    account_store_version: Option<i64>,
    character_save_version: Option<i64>,
}
impl ZoneSaveOperationReceipt {
    pub fn operation(&self) -> &ZoneSaveOperation {
        &self.operation
    }
    pub fn status(&self) -> OperationStatus {
        self.status
    }
    pub fn saved_checkpoint(&self) -> Option<&CharacterSaveRecord> {
        self.saved_checkpoint.as_ref()
    }
    pub fn account_store_version(&self) -> Option<i64> {
        self.account_store_version
    }
    pub fn character_save_version(&self) -> Option<i64> {
        self.character_save_version
    }
    pub fn to_json(&self) -> Result<Value, String> {
        let status = match self.status {
            OperationStatus::Prepared => "Prepared",
            OperationStatus::Committed => "Committed",
            OperationStatus::Released => "Released",
        };
        serde_json::to_value(WireReceipt {
            operation: WireOperation::from_operation(&self.operation),
            status: status.into(),
            saved_checkpoint: self
                .saved_checkpoint
                .as_ref()
                .map(serde_json::to_value)
                .transpose()
                .map_err(|_| REJECTED)?,
            account_store_version: self.account_store_version,
            character_save_version: self.character_save_version,
        })
        .map_err(|_| REJECTED.into())
    }
    /// Checked read-only response decoding. Persist requests must requery the
    /// ledger by binding; a decoded receipt never authorizes a write.
    pub fn from_json_checked(value: Value) -> Result<Self, String> {
        let wire: WireReceipt = serde_json::from_value(value).map_err(|_| REJECTED)?;
        let operation = wire.operation.into_operation()?;
        let status = match wire.status.as_str() {
            "Prepared" => OperationStatus::Prepared,
            "Committed" => OperationStatus::Committed,
            "Released" => OperationStatus::Released,
            _ => return Err(REJECTED.into()),
        };
        if status == OperationStatus::Prepared {
            if wire.saved_checkpoint.is_some()
                || wire.account_store_version.is_some()
                || wire.character_save_version.is_some()
            {
                return Err(REJECTED.into());
            }
        } else if wire.saved_checkpoint.as_ref() != Some(&operation.expected_json)
            || wire.account_store_version.is_none_or(|v| v <= 0)
            || wire.character_save_version.is_none_or(|v| v <= 0)
        {
            return Err(REJECTED.into());
        }
        Ok(Self {
            operation,
            status,
            saved_checkpoint: wire
                .saved_checkpoint
                .map(serde_json::from_value)
                .transpose()
                .map_err(|_| REJECTED)?,
            account_store_version: wire.account_store_version,
            character_save_version: wire.character_save_version,
        })
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 1024
        && value.trim() == value
        && !value.chars().any(char::is_control)
}
fn digest(value: &Value) -> Result<String, String> {
    let bytes = serde_json::to_vec(value).map_err(|_| REJECTED)?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(REJECTED.into());
    }
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
fn read_receipt(
    tx: &mut Transaction<'_>,
    binding: &ZoneSaveOperationBinding,
    lock: bool,
) -> Result<Option<ZoneSaveOperationReceipt>, String> {
    let sql = if lock {
        "SELECT * FROM public.zone_character_save_operations WHERE operation_id=$1 FOR UPDATE"
    } else {
        "SELECT * FROM public.zone_character_save_operations WHERE operation_id=$1"
    };
    tx.query_opt(sql, &[&binding.operation_id])
        .map_err(|_| UNAVAILABLE)?
        .map(|row| decode_receipt(&row, binding))
        .transpose()
}
fn decode_receipt(
    row: &Row,
    binding: &ZoneSaveOperationBinding,
) -> Result<ZoneSaveOperationReceipt, String> {
    let frozen: Value = row.try_get("frozen_checkpoint").map_err(|_| UNAVAILABLE)?;
    let expected: Value = row
        .try_get("expected_checkpoint")
        .map_err(|_| UNAVAILABLE)?;
    let matches = row
        .try_get::<_, String>("operation_id")
        .map_err(|_| UNAVAILABLE)?
        == binding.operation_id
        && row
            .try_get::<_, String>("rpc_session_id")
            .map_err(|_| UNAVAILABLE)?
            == binding.rpc_session_id
        && row
            .try_get::<_, String>("zone_id")
            .map_err(|_| UNAVAILABLE)?
            == binding.fence.zone_id()
        && row
            .try_get::<_, String>("owner_id")
            .map_err(|_| UNAVAILABLE)?
            == binding.fence.owner_id()
        && row
            .try_get::<_, i64>("fencing_token")
            .map_err(|_| UNAVAILABLE)?
            == binding.fence.fencing_token() as i64;
    if !matches {
        return Err(REJECTED.into());
    }
    let operation = ZoneSaveOperation::new(
        binding.clone(),
        row.try_get("account_id").map_err(|_| UNAVAILABLE)?,
        serde_json::from_value(frozen.clone()).map_err(|_| REJECTED)?,
        serde_json::from_value(expected.clone()).map_err(|_| REJECTED)?,
        row.try_get("last_access_binary_datetime")
            .map_err(|_| UNAVAILABLE)?,
    )?;
    if row
        .try_get::<_, i32>("character_index")
        .map_err(|_| UNAVAILABLE)?
        != operation.character_index()
        || row
            .try_get::<_, String>("checkpoint_digest")
            .map_err(|_| UNAVAILABLE)?
            != operation.checkpoint_digest
        || row
            .try_get::<_, String>("expected_digest")
            .map_err(|_| UNAVAILABLE)?
            != operation.expected_digest
        || frozen != operation.frozen_json
        || expected != operation.expected_json
    {
        return Err(REJECTED.into());
    }
    let status = match row
        .try_get::<_, String>("status")
        .map_err(|_| UNAVAILABLE)?
        .as_str()
    {
        "Prepared" => OperationStatus::Prepared,
        "Committed" => OperationStatus::Committed,
        "Released" => OperationStatus::Released,
        _ => return Err(REJECTED.into()),
    };
    let saved: Option<Value> = row.try_get("saved_checkpoint").map_err(|_| UNAVAILABLE)?;
    let account_store_version: Option<i64> = row
        .try_get("account_store_version")
        .map_err(|_| UNAVAILABLE)?;
    let character_save_version: Option<i64> = row
        .try_get("character_save_version")
        .map_err(|_| UNAVAILABLE)?;
    let committed: Option<i64> = row.try_get("committed_at_ms").map_err(|_| UNAVAILABLE)?;
    let released: Option<i64> = row.try_get("released_at_ms").map_err(|_| UNAVAILABLE)?;
    if status == OperationStatus::Prepared {
        if saved.is_some()
            || account_store_version.is_some()
            || character_save_version.is_some()
            || committed.is_some()
            || released.is_some()
        {
            return Err(REJECTED.into());
        }
    } else if saved.as_ref() != Some(&operation.expected_json)
        || account_store_version.is_none_or(|v| v <= 0)
        || character_save_version.is_none_or(|v| v <= 0)
        || committed.is_none_or(|v| v <= 0)
        || released.is_some_and(|v| v <= 0)
        || (status == OperationStatus::Released) != released.is_some()
    {
        return Err(REJECTED.into());
    }
    Ok(ZoneSaveOperationReceipt {
        operation,
        status,
        saved_checkpoint: saved
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| REJECTED)?,
        account_store_version,
        character_save_version,
    })
}

impl SimulationConfig {
    #[cfg(test)]
    pub(crate) fn freeze_zone_save_operation_writes_for_test(&self) {
        self.freeze_account_store_writes(zone_save_fence::commit_outcome_unknown(
            "bounded operation freeze fixture",
        ));
    }

    pub fn with_zone_save_operation(
        mut self,
        operation: ZoneSaveOperation,
    ) -> Result<Self, String> {
        self.validate_zone_save_fence_binding()?;
        if self.zone_save_fence.as_ref() != Some(operation.fence()) {
            return Err(REJECTED.into());
        }
        self.zone_save_operation = Some(operation);
        Ok(self)
    }
    pub fn zone_save_operation(&self) -> Option<&ZoneSaveOperation> {
        self.zone_save_operation.as_ref()
    }
    pub(super) fn validate_zone_save_operation_binding(&self) -> Result<(), String> {
        if let Some(op) = &self.zone_save_operation {
            self.validate_zone_save_fence_binding()?;
            if self.zone_save_fence.as_ref() != Some(op.fence()) {
                return Err(REJECTED.into());
            }
        }
        Ok(())
    }

    /// Idempotently persist the complete immutable checkpoint under its current
    /// lease. No schema migration is allowed on this mutation worker.
    pub fn prepare_zone_save_operation(
        &self,
        operation: &ZoneSaveOperation,
    ) -> Result<ZoneSaveOperationReceipt, String> {
        self.ensure_account_store_writable()?;
        self.validate_zone_save_fence_binding()?;
        if self.zone_save_fence.as_ref() != Some(operation.fence()) {
            return Err(REJECTED.into());
        }
        let op = operation.clone();
        self.zone_operation_repository(true, move |client| {
            let mut tx = client.transaction().map_err(|_| UNAVAILABLE)?;
            op.fence().lock_and_validate(&mut tx, AccountStoreDatabaseMode::SourceOfTruth)?;
            let token = op.fence().fencing_token() as i64;
            tx.execute("INSERT INTO public.zone_character_save_operations \
                (operation_id,rpc_session_id,zone_id,owner_id,fencing_token,account_id, \
                 character_index,checkpoint_digest,frozen_checkpoint,expected_digest,expected_checkpoint, \
                 last_access_binary_datetime,status,prepared_at_ms) \
                VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,'Prepared', \
                 floor(extract(epoch FROM clock_timestamp())*1000)::bigint) \
                ON CONFLICT(operation_id) DO NOTHING",
                &[&op.operation_id(),&op.rpc_session_id(),&op.fence().zone_id(),&op.fence().owner_id(),
                  &token,&op.account_id,&op.character_index(),&op.checkpoint_digest,&op.frozen_json,
                  &op.expected_digest,&op.expected_json,&op.last_access_binary_datetime]).map_err(|_| UNAVAILABLE)?;
            let receipt = read_receipt(&mut tx, op.binding(), true)?.ok_or(REJECTED)?;
            op.validate_same_operation(receipt.operation())?;
            tx.commit().map_err(zone_save_fence::classify_commit_error)?;
            Ok(receipt)
        })
    }

    /// Exact authenticated server binding is sufficient for read-only receipt
    /// reconciliation, even after expiry/takeover or shared-config Unknown freeze.
    pub fn query_zone_save_operation(
        &self,
        binding: &ZoneSaveOperationBinding,
    ) -> Result<Option<ZoneSaveOperationReceipt>, String> {
        self.validate_zone_save_fence_binding()?;
        let binding = binding.clone();
        self.zone_operation_repository(false, move |client| {
            let mut tx = client
                .build_transaction()
                .read_only(true)
                .start()
                .map_err(|_| UNAVAILABLE)?;
            tx.batch_execute("SET LOCAL statement_timeout='2000ms'")
                .map_err(|_| UNAVAILABLE)?;
            read_receipt(&mut tx, &binding, false)
        })
    }

    /// Only the current owner of this Zone may release a durably evidenced
    /// operation. Querying an old receipt does not confer any write capability.
    pub fn release_zone_save_operation(
        &self,
        binding: &ZoneSaveOperationBinding,
    ) -> Result<ZoneSaveOperationReceipt, String> {
        self.ensure_account_store_writable()?;
        self.validate_zone_save_fence_binding()?;
        let fence = self.zone_save_fence.clone().ok_or(REJECTED)?;
        if fence.zone_id() != binding.fence.zone_id() {
            return Err(REJECTED.into());
        }
        let binding = binding.clone();
        self.zone_operation_repository(true, move |client| {
            let mut tx = client.transaction().map_err(|_| UNAVAILABLE)?;
            fence.lock_and_validate(&mut tx, AccountStoreDatabaseMode::SourceOfTruth)?;
            let before = read_receipt(&mut tx, &binding, true)?.ok_or(REJECTED)?;
            if before.status == OperationStatus::Prepared {
                return Err(REJECTED.into());
            }
            if before.status == OperationStatus::Committed {
                tx.execute(
                    "UPDATE public.zone_character_save_operations SET status='Released', \
                    released_at_ms=floor(extract(epoch FROM clock_timestamp())*1000)::bigint \
                    WHERE operation_id=$1 AND status='Committed'",
                    &[&binding.operation_id],
                )
                .map_err(|_| UNAVAILABLE)?;
            }
            let receipt = read_receipt(&mut tx, &binding, true)?.ok_or(REJECTED)?;
            tx.commit()
                .map_err(zone_save_fence::classify_commit_error)?;
            Ok(receipt)
        })
    }

    fn zone_operation_repository<T: Send + 'static>(
        &self,
        mutation: bool,
        action: impl FnOnce(&mut Client) -> Result<T, String> + Send + 'static,
    ) -> Result<T, String> {
        let url = self.account_store_database_url.as_deref().ok_or(REJECTED)?;
        let pool = super::postgres_account_store_pool(url);
        let result = std::thread::spawn(move || {
            let mut client = pool.connection().map_err(|_| UNAVAILABLE)?;
            action(&mut client)
        })
        .join()
        .unwrap_or_else(|_| {
            Err(if mutation {
                zone_save_fence::commit_outcome_unknown("operation worker panicked")
            } else {
                UNAVAILABLE.into()
            })
        });
        result.map_err(|error| {
            if error.starts_with(zone_save_fence::COMMIT_OUTCOME_UNKNOWN) {
                self.freeze_account_store_writes(error)
            } else {
                error
            }
        })
    }
}

#[cfg(test)]
#[path = "zone_save_operation_tests.rs"]
mod tests;
