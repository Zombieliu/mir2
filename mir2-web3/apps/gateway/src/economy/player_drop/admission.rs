//! Durable intent before bootstrap or UID reservation. Data records are never
//! continuation authority; only an acknowledged fresh INSERT issues that proof.
use super::*;
use mir2_protocol::{Point, UserItem};
use mir2_simulation::{PlayerDropRequest, ZoneKey};
use postgres::GenericClient;
use std::sync::{Mutex, OnceLock};

fn digest(domain: &[u8], value: &impl Serialize) -> Result<String, String> {
    fn canonical(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(fields) => serde_json::Value::Object(
                fields
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
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(
        serde_json::to_vec(&canonical(
            serde_json::to_value(value).map_err(|e| e.to_string())?,
        ))
        .map_err(|e| e.to_string())?,
    );
    Ok(hex_lower(&hash.finalize()))
}
fn action_id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.trim() != value
        || value.len() > 1024
        || value.chars().any(char::is_control)
    {
        return Err("invalid player drop admission action identity".into());
    }
    Ok(())
}
fn item_ids(item: &UserItem, ids: &mut Vec<u64>) {
    ids.push(item.unique_id);
    for child in item.slots.iter().flatten() {
        item_ids(child, ids);
    }
}
fn clear_ids(item: &mut UserItem) {
    item.unique_id = 0;
    for child in item.slots.iter_mut().flatten() {
        clear_ids(child);
    }
}
fn inspect_item(
    item: &UserItem,
    assigned: bool,
    depth: usize,
    seen: &mut BTreeSet<u64>,
    count: &mut usize,
) -> Result<(), String> {
    *count = count.checked_add(1).ok_or("admission item tree overflow")?;
    if depth > 8
        || *count > 256
        || item.slots.len() > 64
        || item.added_stats.len() > 256
        || item.awake_values.len() > 64
    {
        return Err("admission item carrier exceeds its bounded shape".into());
    }
    static AMBIGUOUS: OnceLock<BTreeSet<i32>> = OnceLock::new();
    let ambiguous = AMBIGUOUS.get_or_init(|| {
        let mut seen = BTreeSet::new();
        mir2_game_data::crystal_item_manifest()
            .items
            .into_iter()
            .filter_map(|item| (!seen.insert(item.item_index)).then_some(item.item_index))
            .collect()
    });
    let template = mir2_game_data::crystal_item_by_index(item.item_index)
        .ok_or("admission item has unknown catalogue identity")?;
    if ambiguous.contains(&item.item_index)
        || item.count == 0
        || item.count > template.stack_size.max(1)
        || (assigned && (item.unique_id == 0 || !seen.insert(item.unique_id)))
        || (!assigned && item.unique_id != 0)
    {
        return Err("admission item carrier has invalid quantity or UID shape".into());
    }
    for child in item.slots.iter().flatten() {
        inspect_item(child, assigned, depth + 1, seen, count)?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrozenPlayerDropAdmission {
    version: u8,
    binding: PlayerDropOperationBinding,
    zone: ZoneKey,
    position: Point,
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    request: PlayerDropRequest,
    preview: PlayerDropOutcome,
    expected_uid_count: usize,
    admission_digest: String,
}
impl FrozenPlayerDropAdmission {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        binding: PlayerDropOperationBinding,
        zone: ZoneKey,
        position: Point,
        created_at_unix_ms: u64,
        expires_at_unix_ms: u64,
        request: PlayerDropRequest,
        preview: PlayerDropOutcome,
    ) -> Result<Self, String> {
        let mut frozen = Self {
            version: 1,
            binding,
            zone,
            position,
            created_at_unix_ms,
            expires_at_unix_ms,
            request,
            preview,
            expected_uid_count: 0,
            admission_digest: String::new(),
        };
        frozen.expected_uid_count = frozen.validate_payload()?;
        frozen.admission_digest = frozen.compute_digest()?;
        Ok(frozen)
    }
    fn validate_payload(&self) -> Result<usize, String> {
        self.binding.validate_integrity()?;
        if self.version != 1
            || self.zone.shard_id.trim().is_empty()
            || self.zone.map_file_name.trim().is_empty()
            || self.created_at_unix_ms == 0
            || self.expires_at_unix_ms <= self.created_at_unix_ms
            || self.expires_at_unix_ms > i64::MAX as u64
            || self.created_at_unix_ms.checked_add(30 * 60 * 1000) != Some(self.expires_at_unix_ms)
        {
            return Err("invalid frozen player drop admission".into());
        }
        let (unique_id, count) = match &self.request {
            PlayerDropRequest::Gold { amount } => {
                return if *amount > 0
                    && self.preview == (PlayerDropOutcome::Gold { amount: *amount })
                {
                    Ok(0)
                } else {
                    Err("admission request differs from preview".into())
                };
            }
            PlayerDropRequest::Item {
                unique_id,
                count,
                hero_inventory,
            } if *unique_id > 0 && *count > 0 && !*hero_inventory => (*unique_id, *count),
            _ => return Err("invalid admission item request".into()),
        };
        let (quantity, exact, split, key) = match &self.preview {
            PlayerDropOutcome::Item {
                quantity,
                loot:
                    GroundDropLootSnapshot::InventoryItem {
                        key,
                        exact_item: Some(exact),
                        ..
                    },
            } if !key.is_empty() => (*quantity, exact, !exact.uid_assigned, key.as_str()),
            PlayerDropOutcome::DestroyedItem {
                unique_id: original,
                quantity,
                item_key,
                exact_item,
            } if *original == unique_id && !item_key.is_empty() => {
                (*quantity, exact_item, false, item_key.as_str())
            }
            _ => return Err("admission item request has no exact preview".into()),
        };
        if quantity != u32::from(count)
            || exact.item.count != count
            || (exact.uid_assigned && exact.item.unique_id != unique_id)
            || mir2_simulation::player_drop_item_template_index(key) != Some(exact.item.item_index)
        {
            return Err("admission source selection differs from exact preview".into());
        }
        let mut nodes = 0;
        inspect_item(
            &exact.item,
            exact.uid_assigned,
            0,
            &mut BTreeSet::new(),
            &mut nodes,
        )?;
        if exact.uid_assigned {
            self.preview.validate_integrity()?;
        }
        Ok(if split { nodes } else { 0 })
    }
    fn compute_digest(&self) -> Result<String, String> {
        digest(
            b"mir2.player-drop-admission.v1\0",
            &(
                self.version,
                &self.binding,
                &self.zone,
                &self.position,
                self.created_at_unix_ms,
                self.expires_at_unix_ms,
                &self.request,
                &self.preview,
                self.expected_uid_count,
            ),
        )
    }
    pub fn validate_integrity(&self) -> Result<(), String> {
        if self.validate_payload()? != self.expected_uid_count
            || self.compute_digest()? != self.admission_digest
        {
            return Err("player drop admission digest conflict".into());
        }
        Ok(())
    }
    pub fn binding(&self) -> &PlayerDropOperationBinding {
        &self.binding
    }
    pub fn zone(&self) -> &ZoneKey {
        &self.zone
    }
    pub fn position(&self) -> &Point {
        &self.position
    }
    pub fn created_at_unix_ms(&self) -> u64 {
        self.created_at_unix_ms
    }
    pub fn expires_at_unix_ms(&self) -> u64 {
        self.expires_at_unix_ms
    }
    pub fn request(&self) -> &PlayerDropRequest {
        &self.request
    }
    pub fn preview(&self) -> &PlayerDropOutcome {
        &self.preview
    }
    pub fn expected_uid_count(&self) -> usize {
        self.expected_uid_count
    }
    pub fn admission_digest(&self) -> &str {
        &self.admission_digest
    }
    pub fn validate_operation(
        &self,
        operation: &PlayerDropPreparedOperation,
    ) -> Result<(), String> {
        self.validate_integrity()?;
        operation.validate_integrity()?;
        self.request.validate_operation(operation)?;
        let intent = operation.intent();
        if operation.binding() != &self.binding
            || intent.zone() != &self.zone
            || intent.position() != &self.position
            || intent.created_at_unix_ms() != self.created_at_unix_ms
            || intent.expires_at_unix_ms() != self.expires_at_unix_ms
        {
            return Err(
                "prepared player drop differs from frozen admission placement or binding".into(),
            );
        }
        let mut actual = intent.outcome().clone();
        if self.expected_uid_count > 0 {
            let PlayerDropOutcome::Item {
                loot:
                    GroundDropLootSnapshot::InventoryItem {
                        exact_item: Some(exact),
                        ..
                    },
                ..
            } = &mut actual
            else {
                return Err("admission split outcome missing".into());
            };
            clear_ids(&mut exact.item);
            exact.uid_assigned = false;
        }
        if actual != self.preview {
            return Err("prepared player drop differs from complete frozen preview".into());
        }
        Ok(())
    }
    fn validate_reserved_operation(
        &self,
        operation: &PlayerDropPreparedOperation,
        ids: Option<&[u64]>,
    ) -> Result<(), String> {
        self.validate_operation(operation)?;
        if self.expected_uid_count == 0 {
            if ids.is_some() {
                return Err("unexpected admission UID reservation".into());
            }
            return Ok(());
        }
        let PlayerDropOutcome::Item {
            loot:
                GroundDropLootSnapshot::InventoryItem {
                    exact_item: Some(exact),
                    ..
                },
            ..
        } = operation.intent().outcome()
        else {
            return Err("admission split carrier missing".into());
        };
        let mut actual = Vec::new();
        item_ids(&exact.item, &mut actual);
        if ids != Some(actual.as_slice()) || actual.len() != self.expected_uid_count {
            return Err("prepared player drop does not use its exact reserved UID tree".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerDropAdmissionStatus {
    Admitted,
    Linked,
    Aborted,
}
impl PlayerDropAdmissionStatus {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "Admitted" => Ok(Self::Admitted),
            "Linked" => Ok(Self::Linked),
            "Aborted" => Ok(Self::Aborted),
            _ => Err("invalid player drop admission status".into()),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerDropAdmissionAbortAudit {
    version: u8,
    admission_digest: String,
    resolution_kind: String,
    recovery_id: String,
    reason: String,
    recovery_owner_id: String,
    recovery_epoch: u64,
    aborted_at_ms: u64,
    bootstrap_witness: Option<EconomyBootstrapReceipt>,
    uid_reservations: Option<Vec<u64>>,
}
impl PlayerDropAdmissionAbortAudit {
    pub fn recovery_id(&self) -> &str {
        &self.recovery_id
    }
    pub fn reason(&self) -> &str {
        &self.reason
    }
    pub fn resolution_kind(&self) -> &str {
        &self.resolution_kind
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
    pub fn bootstrap_witness(&self) -> Option<&EconomyBootstrapReceipt> {
        self.bootstrap_witness.as_ref()
    }
    pub fn uid_reservations(&self) -> Option<&[u64]> {
        self.uid_reservations.as_deref()
    }
    pub fn digest(&self) -> Result<String, String> {
        digest(b"mir2.player-drop-admission-abort.v1\0", self)
    }
    fn validate(&self, frozen: &FrozenPlayerDropAdmission) -> Result<(), String> {
        action_id(&self.recovery_id)?;
        action_id(&self.reason)?;
        action_id(&self.recovery_owner_id)?;
        let binding = frozen.binding();
        let valid_epoch = match self.resolution_kind.as_str() {
            "ownerRejected" => {
                self.recovery_epoch == binding.owner_epoch()
                    && self.recovery_owner_id == binding.owner_id()
            }
            "successorAborted" => self.recovery_epoch > binding.owner_epoch(),
            _ => false,
        };
        if self.version != 1
            || self.admission_digest != frozen.admission_digest()
            || !valid_epoch
            || self.recovery_epoch > i64::MAX as u64
            || self.aborted_at_ms == 0
            || self.aborted_at_ms > i64::MAX as u64
        {
            return Err("invalid admission abort epoch or evidence".into());
        }
        if let Some(bootstrap) = &self.bootstrap_witness {
            if bootstrap.account_id != binding.account_id()
                || bootstrap.character_index != binding.character_index()
            {
                return Err("admission bootstrap witness identity conflict".into());
            }
        }
        if let Some(ids) = &self.uid_reservations {
            if ids.len() != frozen.expected_uid_count()
                || ids.is_empty()
                || ids.iter().any(|id| *id == 0)
                || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
            {
                return Err("admission abort UID witness conflict".into());
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct PlayerDropAdmissionRecord {
    namespace: String,
    connection_binding_digest: String,
    frozen: FrozenPlayerDropAdmission,
    status: PlayerDropAdmissionStatus,
    linked_operation_digest: Option<String>,
    abort_audit: Option<PlayerDropAdmissionAbortAudit>,
    abort_audit_digest: Option<String>,
}
impl PlayerDropAdmissionRecord {
    pub fn account_namespace(&self) -> &str {
        &self.namespace
    }
    pub fn frozen(&self) -> &FrozenPlayerDropAdmission {
        &self.frozen
    }
    pub fn status(&self) -> PlayerDropAdmissionStatus {
        self.status
    }
    pub fn operation_digest(&self) -> Option<&str> {
        self.linked_operation_digest.as_deref()
    }
    pub fn linked_operation_digest(&self) -> Option<&str> {
        self.operation_digest()
    }
    pub fn abort_audit(&self) -> Option<&PlayerDropAdmissionAbortAudit> {
        self.abort_audit.as_ref()
    }
    pub fn abort_audit_digest(&self) -> Option<&str> {
        self.abort_audit_digest.as_deref()
    }
    pub fn validate_operation(
        &self,
        operation: &PlayerDropPreparedOperation,
    ) -> Result<(), String> {
        if self.status != PlayerDropAdmissionStatus::Linked
            || self.operation_digest() != Some(operation.operation_digest())
        {
            return Err("admission has not linked this exact source operation".into());
        }
        self.frozen.validate_operation(operation)
    }
    pub fn recovery_evidence(&self) -> Result<serde_json::Value, String> {
        serde_json::to_value((
            &self.namespace,
            &self.frozen,
            self.status,
            &self.linked_operation_digest,
            &self.abort_audit,
            &self.abort_audit_digest,
        ))
        .map_err(|e| e.to_string())
    }
}
/// No Clone/serde/public constructor: receipt lookup cannot resume an admission.
pub struct PlayerDropAdmissionCapability {
    frozen: FrozenPlayerDropAdmission,
    namespace: String,
    connection_binding_digest: String,
    resolved: Mutex<bool>,
}
impl std::fmt::Debug for PlayerDropAdmissionCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlayerDropAdmissionCapability")
            .field("operation_key", &self.frozen.binding().operation_key())
            .finish_non_exhaustive()
    }
}
impl PlayerDropAdmissionCapability {
    pub fn frozen(&self) -> &FrozenPlayerDropAdmission {
        &self.frozen
    }
    pub fn validate(
        &self,
        permit: &ZoneEconomyWritePermit,
        database_url: &str,
        frozen: &FrozenPlayerDropAdmission,
    ) -> Result<(), String> {
        frozen.validate_integrity()?;
        permit.validate_target_database(database_url)?;
        let binding = frozen.binding();
        permit.validate_context(binding.zone_id(), binding.owner_epoch())?;
        if self.connection_binding_digest != connection_binding_digest(database_url)
            || self.frozen != *frozen
            || permit.fence().owner_id() != binding.owner_id()
            || *self
                .resolved
                .lock()
                .map_err(|_| "admission continuation poisoned")?
        {
            return Err("player drop admission continuation is resolved or mismatched".into());
        }
        Ok(())
    }
}
#[derive(Debug)]
pub enum PlayerDropAdmissionPublication {
    Fresh(PlayerDropAdmissionCapability),
    Existing(PlayerDropAdmissionRecord),
}

const ADMISSION_QUERY:&str="SELECT admission.*,op.operation AS linked_operation,op.operation_digest AS stored_operation_digest,op.operation_key AS stored_operation_key,audit.audit AS audited_abort,audit.audit_digest AS audited_abort_digest,batch.binding AS uid_binding,batch.item_count AS uid_count,ARRAY(SELECT reservation.ordinal FROM public.zone_player_drop_item_uid_reservations reservation WHERE reservation.source_namespace=admission.source_namespace AND reservation.operation_key=admission.operation_key ORDER BY reservation.ordinal) AS uid_ordinals,ARRAY(SELECT reservation.item_uid::text FROM public.zone_player_drop_item_uid_reservations reservation WHERE reservation.source_namespace=admission.source_namespace AND reservation.operation_key=admission.operation_key ORDER BY reservation.ordinal) AS uid_values,EXISTS(SELECT 1 FROM game_economy_transactions tx WHERE tx.idempotency_key=admission.operation_key) AS source_ledger_exists,EXISTS(SELECT 1 FROM game_economy_outbox outbox WHERE outbox.idempotency_key=admission.operation_key) AS source_outbox_exists FROM (SELECT * FROM public.zone_player_drop_admissions WHERE source_namespace=current_schema()) admission LEFT JOIN public.zone_player_drop_operations op ON op.source_namespace=admission.source_namespace AND op.operation_key=admission.operation_key LEFT JOIN public.zone_player_drop_admission_abort_audit audit ON audit.source_namespace=admission.source_namespace AND audit.operation_key=admission.operation_key LEFT JOIN public.zone_player_drop_item_uid_batches batch ON batch.source_namespace=admission.source_namespace AND batch.operation_key=admission.operation_key";
fn row_uids(
    row: &postgres::Row,
    frozen: &FrozenPlayerDropAdmission,
) -> Result<Option<Vec<u64>>, String> {
    let values: Vec<String> = row.get("uid_values");
    let ordinals: Vec<i32> = row.get("uid_ordinals");
    let Some(binding) = row.get::<_, Option<serde_json::Value>>("uid_binding") else {
        if !values.is_empty() || !ordinals.is_empty() {
            return Err("orphan admission UID reservation".into());
        }
        return Ok(None);
    };
    let count = row
        .get::<_, Option<i32>>("uid_count")
        .ok_or("admission UID count missing")?;
    if binding != serde_json::to_value(frozen.binding()).map_err(|e| e.to_string())?
        || count <= 0
        || count as usize != frozen.expected_uid_count()
        || values.len() != count as usize
        || ordinals
            .iter()
            .enumerate()
            .any(|(index, ordinal)| *ordinal != index as i32)
    {
        return Err("admission UID batch identity or ordinal conflict".into());
    }
    let ids = values
        .into_iter()
        .map(|id| {
            id.parse::<u64>()
                .map_err(|_| "invalid admission reserved UID".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if ids.iter().any(|id| *id == 0) || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
        return Err("invalid admission reserved UID set".into());
    }
    Ok(Some(ids))
}
fn decode_admission(
    row: postgres::Row,
    database_url: &str,
) -> Result<PlayerDropAdmissionRecord, String> {
    let frozen: FrozenPlayerDropAdmission =
        serde_json::from_value(row.get("frozen")).map_err(|e| e.to_string())?;
    frozen.validate_integrity()?;
    let binding = frozen.binding();
    if row.get::<_, String>("operation_key") != binding.operation_key()
        || row.get::<_, String>("world_effect_id") != binding.world_effect_id()
        || row.get::<_, String>("zone_id") != binding.zone_id()
        || row.get::<_, String>("owner_id") != binding.owner_id()
        || row.get::<_, i64>("owner_epoch") as u64 != binding.owner_epoch()
        || row.get::<_, String>("admission_digest") != frozen.admission_digest()
        || row.get::<_, i32>("expected_uid_count") as usize != frozen.expected_uid_count()
    {
        return Err("admission immutable payload differs from row binding".into());
    }
    let status = PlayerDropAdmissionStatus::parse(&row.get::<_, String>("status"))?;
    let ids = row_uids(&row, &frozen)?;
    let linked = row.get::<_, Option<String>>("linked_operation_digest");
    let operation: Option<PlayerDropPreparedOperation> = row
        .get::<_, Option<serde_json::Value>>("linked_operation")
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| e.to_string())?;
    if status == PlayerDropAdmissionStatus::Linked {
        let operation = operation
            .as_ref()
            .ok_or("linked admission source operation missing")?;
        frozen.validate_reserved_operation(operation, ids.as_deref())?;
        if linked.as_deref() != Some(operation.operation_digest())
            || row
                .get::<_, Option<String>>("stored_operation_digest")
                .as_deref()
                != linked.as_deref()
            || row
                .get::<_, Option<String>>("linked_operation_key")
                .as_deref()
                != Some(binding.operation_key().as_str())
        {
            return Err("admission linked operation digest conflict".into());
        }
    } else if operation.is_some()
        || linked.is_some()
        || row.get::<_, bool>("source_ledger_exists")
        || row.get::<_, bool>("source_outbox_exists")
    {
        return Err("unlinked admission has source publication evidence".into());
    }
    let abort_digest = row.get::<_, Option<String>>("abort_audit_digest");
    let abort_audit = if status == PlayerDropAdmissionStatus::Aborted {
        let value = row
            .get::<_, Option<serde_json::Value>>("abort_audit")
            .ok_or("admission abort evidence missing")?;
        let audit: PlayerDropAdmissionAbortAudit =
            serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
        audit.validate(&frozen)?;
        if audit.digest()?.as_str()
            != abort_digest
                .as_deref()
                .ok_or("admission abort digest missing")?
            || row.get::<_, Option<serde_json::Value>>("audited_abort") != Some(value)
            || row.get::<_, Option<String>>("audited_abort_digest") != abort_digest
            || row.get::<_, Option<i64>>("aborted_at_ms") != Some(audit.aborted_at_ms() as i64)
            || audit.uid_reservations() != ids.as_deref()
        {
            return Err("admission abort immutable audit conflict".into());
        }
        Some(audit)
    } else {
        None
    };
    Ok(PlayerDropAdmissionRecord {
        namespace: row.get("source_namespace"),
        connection_binding_digest: connection_binding_digest(database_url),
        frozen,
        status,
        linked_operation_digest: linked,
        abort_audit,
        abort_audit_digest: abort_digest,
    })
}
pub(super) fn read_admission_page(
    client: &mut impl GenericClient,
    database_url: &str,
    zone: &str,
    after: Option<&str>,
    limit: usize,
    lock: bool,
) -> Result<Vec<PlayerDropAdmissionRecord>, String> {
    action_id(zone)?;
    if !(1..=256).contains(&limit) {
        return Err("invalid bounded admission enumeration".into());
    }
    let locking = if lock { " FOR UPDATE OF admission" } else { "" };
    client.query(&format!("{ADMISSION_QUERY} WHERE admission.zone_id=$1 AND admission.operation_key>$2 ORDER BY admission.operation_key LIMIT $3{locking}"),&[&zone,&after.unwrap_or(""),&(limit as i64)]).map_err(|e|e.to_string())?
        .into_iter().map(|row|decode_admission(row,database_url)).collect()
}
fn load_admission(
    client: &mut impl GenericClient,
    database_url: &str,
    binding: &PlayerDropOperationBinding,
    lock: bool,
) -> Result<Option<PlayerDropAdmissionRecord>, String> {
    let locking = if lock { " FOR UPDATE OF admission" } else { "" };
    let record = client
        .query_opt(
            &format!("{ADMISSION_QUERY} WHERE admission.operation_key=$1{locking}"),
            &[&binding.operation_key()],
        )
        .map_err(|e| e.to_string())?
        .map(|row| decode_admission(row, database_url))
        .transpose()?;
    if record
        .as_ref()
        .is_some_and(|record| record.frozen.binding() != binding)
    {
        return Err("admission lookup binding conflict".into());
    }
    Ok(record)
}
fn lock_original_operation(
    tx: &mut Transaction<'_>,
    binding: &PlayerDropOperationBinding,
) -> Result<(), String> {
    zone_fence::bound_transaction(tx)?;
    tx.query_one(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
        &[&binding.operation_key()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

impl PostgresEconomyStore {
    pub fn lookup_player_drop_admission(
        &self,
        binding: &PlayerDropOperationBinding,
    ) -> Result<Option<PlayerDropAdmissionRecord>, String> {
        binding.validate_integrity()?;
        load_admission(
            &mut self.player_drop_connect()?,
            &self.database_url,
            binding,
            false,
        )
    }
    pub fn enumerate_zone_player_drop_admissions(
        &self,
        zone: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<PlayerDropAdmissionRecord>, String> {
        read_admission_page(
            &mut self.player_drop_connect()?,
            &self.database_url,
            zone,
            after,
            limit,
            false,
        )
    }
    pub fn admit_player_drop(
        &self,
        permit: &ZoneEconomyWritePermit,
        frozen: &FrozenPlayerDropAdmission,
    ) -> Result<PlayerDropAdmissionPublication, String> {
        self.admit_inner(permit, frozen, false)
    }
    #[cfg(test)]
    pub(crate) fn admit_player_drop_losing_acknowledgement(
        &self,
        permit: &ZoneEconomyWritePermit,
        frozen: &FrozenPlayerDropAdmission,
    ) -> Result<PlayerDropAdmissionPublication, String> {
        self.admit_inner(permit, frozen, true)
    }
    fn admit_inner(
        &self,
        permit: &ZoneEconomyWritePermit,
        frozen: &FrozenPlayerDropAdmission,
        lose_ack: bool,
    ) -> Result<PlayerDropAdmissionPublication, String> {
        frozen.validate_integrity()?;
        permit.validate_target_database(&self.database_url)?;
        let binding = frozen.binding();
        permit.validate_context(binding.zone_id(), binding.owner_epoch())?;
        if permit.fence().owner_id() != binding.owner_id() {
            return Err("admission original owner mismatch".into());
        }
        let mut client = self.player_drop_connect()?;
        let mut tx = client.transaction().map_err(|e| e.to_string())?;
        lock_original_operation(&mut tx, binding)?;
        permit.lock_and_validate(
            &mut tx,
            &self.database_url,
            binding.zone_id(),
            binding.owner_epoch(),
        )?;
        let encoded = serde_json::to_value(frozen).map_err(|e| e.to_string())?;
        let inserted=tx.execute("INSERT INTO public.zone_player_drop_admissions(operation_key,world_effect_id,zone_id,owner_id,owner_epoch,frozen,admission_digest,expected_uid_count,status,admitted_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,'Admitted',floor(extract(epoch FROM clock_timestamp())*1000)::bigint) ON CONFLICT(source_namespace,operation_key) DO NOTHING",
            &[&binding.operation_key(),&binding.world_effect_id(),&binding.zone_id(),&binding.owner_id(),&(binding.owner_epoch() as i64),&encoded,&frozen.admission_digest(),&(frozen.expected_uid_count() as i32)]).map_err(|e|e.to_string())?==1;
        let record = load_admission(&mut tx, &self.database_url, binding, false)?
            .ok_or("admission row missing after insert")?;
        if record.frozen() != frozen {
            return Err("admission identity already freezes a different request".into());
        }
        if !inserted {
            return Ok(PlayerDropAdmissionPublication::Existing(record));
        }
        tx.commit().map_err(|_| permit.commit_outcome_unknown())?;
        if lose_ack {
            return Err(permit.commit_outcome_unknown());
        }
        Ok(PlayerDropAdmissionPublication::Fresh(
            PlayerDropAdmissionCapability {
                frozen: frozen.clone(),
                namespace: record.namespace,
                connection_binding_digest: connection_binding_digest(&self.database_url),
                resolved: Mutex::new(false),
            },
        ))
    }
    pub fn prepare_admitted_player_drop_operation(
        &self,
        permit: &ZoneEconomyWritePermit,
        cap: &PlayerDropAdmissionCapability,
        source: &PreparedPlayerDropAccountSource,
    ) -> Result<(), String> {
        cap.validate(permit, &self.database_url, &cap.frozen)?;
        if source.database_url() != self.database_url {
            return Err("admission source PostgreSQL binding mismatch".into());
        }
        cap.frozen.validate_operation(source.operation())?;
        let binding = cap.frozen.binding();
        let mut client = self.player_drop_connect()?;
        let mut tx = client.transaction().map_err(|e| e.to_string())?;
        lock_original_operation(&mut tx, binding)?;
        permit.lock_and_validate(
            &mut tx,
            &self.database_url,
            binding.zone_id(),
            binding.owner_epoch(),
        )?;
        let record = load_admission(&mut tx, &self.database_url, binding, true)?
            .ok_or("admitted source row missing")?;
        if record.namespace != cap.namespace
            || record.connection_binding_digest != cap.connection_binding_digest
            || record.frozen() != &cap.frozen
            || record.status() != PlayerDropAdmissionStatus::Admitted
        {
            return Err("source preparation requires its live Admitted continuation".into());
        }
        let ids = if cap.frozen.expected_uid_count() > 0 {
            item_uid::read_reservation(&mut tx, binding, cap.frozen.expected_uid_count())?
        } else {
            None
        };
        cap.frozen
            .validate_reserved_operation(source.operation(), ids.as_deref())?;
        insert_prepared_operation_in_transaction(&mut tx, source.operation())?;
        let changed=tx.execute("UPDATE public.zone_player_drop_admissions SET status='Linked',linked_operation_key=operation_key,linked_operation_digest=$2,linked_at_ms=floor(extract(epoch FROM clock_timestamp())*1000)::bigint WHERE source_namespace=current_schema() AND operation_key=$1 AND status='Admitted'",&[&binding.operation_key(),&source.operation().operation_digest()]).map_err(|e|e.to_string())?;
        if changed != 1 {
            return Err("admission lost its Prepared link transition".into());
        }
        tx.commit().map_err(|_| permit.commit_outcome_unknown())?;
        *cap.resolved
            .lock()
            .map_err(|_| permit.commit_outcome_unknown())? = true;
        Ok(())
    }
    pub fn abort_admitted_player_drop(
        &self,
        permit: &ZoneEconomyWritePermit,
        binding: &PlayerDropOperationBinding,
        recovery_id: &str,
        reason: &str,
    ) -> Result<PlayerDropAdmissionRecord, String> {
        if permit.fence().fencing_token() <= binding.owner_epoch() {
            return Err("admission abort requires a newer successor epoch".into());
        }
        self.resolve_admission(
            permit,
            binding,
            None,
            "successorAborted",
            recovery_id,
            reason,
        )
    }
    pub fn reject_admitted_player_drop(
        &self,
        permit: &ZoneEconomyWritePermit,
        cap: &PlayerDropAdmissionCapability,
        reason: &str,
    ) -> Result<(), String> {
        cap.validate(permit, &self.database_url, &cap.frozen)?;
        let recovery_id = format!("owner-rejected:{}", cap.frozen.admission_digest());
        self.resolve_admission(
            permit,
            cap.frozen.binding(),
            Some(cap),
            "ownerRejected",
            &recovery_id,
            reason,
        )?;
        *cap.resolved
            .lock()
            .map_err(|_| permit.commit_outcome_unknown())? = true;
        Ok(())
    }
    fn resolve_admission(
        &self,
        permit: &ZoneEconomyWritePermit,
        binding: &PlayerDropOperationBinding,
        cap: Option<&PlayerDropAdmissionCapability>,
        kind: &str,
        recovery_id: &str,
        reason: &str,
    ) -> Result<PlayerDropAdmissionRecord, String> {
        binding.validate_integrity()?;
        action_id(recovery_id)?;
        action_id(reason)?;
        permit.validate_target_database(&self.database_url)?;
        if permit.fence().zone_id() != binding.zone_id() {
            return Err("admission resolution Zone mismatch".into());
        }
        let mut client = self.player_drop_connect()?;
        let mut tx = client.transaction().map_err(|e| e.to_string())?;
        lock_original_operation(&mut tx, binding)?;
        permit.lock_and_validate(
            &mut tx,
            &self.database_url,
            binding.zone_id(),
            permit.fence().fencing_token(),
        )?;
        let record = load_admission(&mut tx, &self.database_url, binding, true)?
            .ok_or("original admission missing")?;
        if let Some(cap) = cap {
            if record.namespace != cap.namespace || record.frozen() != &cap.frozen {
                return Err("admission rejection continuation mismatch".into());
            }
        }
        if record.status() == PlayerDropAdmissionStatus::Aborted {
            let audit = record
                .abort_audit()
                .ok_or("aborted admission audit missing")?;
            if audit.resolution_kind() != kind
                || audit.recovery_id() != recovery_id
                || audit.reason() != reason
                || audit.recovery_owner_id() != permit.fence().owner_id()
                || audit.recovery_epoch() != permit.fence().fencing_token()
            {
                return Err(
                    "admission resolution identity already has different immutable evidence".into(),
                );
            }
            return Ok(record);
        }
        if record.status() != PlayerDropAdmissionStatus::Admitted {
            return Err(
                "linked admission requires source-operation recovery, not admission rejection"
                    .into(),
            );
        }
        lock_economy_characters(
            &mut tx,
            &[(binding.account_id().to_owned(), binding.character_index())]
                .into_iter()
                .collect(),
        )?;
        let identity = ActiveSessionIdentity {
            account_id: binding.account_id().into(),
            character_index: binding.character_index(),
            character_name: String::new(),
        };
        let bootstrap = zone_fence::read_bootstrap_receipt(&mut tx, &identity)?;
        let uids = if record.frozen.expected_uid_count() > 0 {
            item_uid::read_reservation(&mut tx, binding, record.frozen.expected_uid_count())?
        } else {
            None
        };
        let now: i64 = tx
            .query_one(
                "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
                &[],
            )
            .map_err(|e| e.to_string())?
            .get(0);
        let audit = PlayerDropAdmissionAbortAudit {
            version: 1,
            admission_digest: record.frozen.admission_digest().into(),
            resolution_kind: kind.into(),
            recovery_id: recovery_id.into(),
            reason: reason.into(),
            recovery_owner_id: permit.fence().owner_id().into(),
            recovery_epoch: permit.fence().fencing_token(),
            aborted_at_ms: u64::try_from(now).map_err(|_| "admission database clock invalid")?,
            bootstrap_witness: bootstrap,
            uid_reservations: uids,
        };
        audit.validate(&record.frozen)?;
        let encoded = serde_json::to_value(&audit).map_err(|e| e.to_string())?;
        let audit_digest = audit.digest()?;
        tx.execute("INSERT INTO public.zone_player_drop_admission_abort_audit(operation_key,audit,audit_digest) VALUES($1,$2,$3)",&[&binding.operation_key(),&encoded,&audit_digest]).map_err(|e|e.to_string())?;
        tx.execute("UPDATE public.zone_player_drop_admissions SET status='Aborted',aborted_at_ms=$2,abort_audit=$3,abort_audit_digest=$4 WHERE source_namespace=current_schema() AND operation_key=$1 AND status='Admitted'",&[&binding.operation_key(),&(audit.aborted_at_ms as i64),&encoded,&audit_digest]).map_err(|e|e.to_string())?;
        let result = load_admission(&mut tx, &self.database_url, binding, false)?
            .ok_or("resolved admission missing")?;
        tx.commit().map_err(|_| permit.commit_outcome_unknown())?;
        Ok(result)
    }
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;
