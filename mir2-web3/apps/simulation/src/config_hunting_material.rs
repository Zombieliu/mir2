//! Trusted material quote, complete character CAS, permits and unbound fee.
//! Client UID/quantity are intent; every source capability is server constructed.
use super::hunting_source::{
    canonical_hash, database_binding, lock_and_load, source_db, validate_token, LockedImage,
};
use super::*;
use mir2_economy_service::{
    self as economy, EconomyError, Idempotent, MaterialConversionQuoteReceipt,
    MaterialConversionQuoteSource, MaterialConversionReceipt, MaterialConversionSourceResult,
    SourceContext, WalletOwner,
};
use serde_json::json;
use std::{cell::RefCell, sync::atomic::Ordering};

#[derive(Debug, Clone)]
pub(crate) struct HuntingMaterialCommit {
    /// Immutable original result. It must never replace a newer live inventory.
    pub checkpoint: CharacterSaveRecord,
    pub current_revision: Option<u64>,
    pub receipt: Idempotent<MaterialConversionReceipt>,
    publication: HuntingMaterialPublication,
}
#[derive(Clone)]
struct HuntingMaterialPublication {
    binding: SharedAccountStore,
    fence: HuntingMaterialFence,
    receipt_digest: String,
}
impl fmt::Debug for HuntingMaterialPublication {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HuntingMaterialPublication(<private>)")
    }
}
#[derive(Debug)]
pub(crate) enum HuntingMaterialSourceError {
    Rejected(String),
    EconomyRejected(EconomyError),
    OutcomeUnknown(String),
    KnownCommitted {
        commit: Box<HuntingMaterialCommit>,
        detail: String,
    },
}
impl From<String> for HuntingMaterialSourceError {
    fn from(detail: String) -> Self {
        Self::Rejected(detail)
    }
}
impl From<&str> for HuntingMaterialSourceError {
    fn from(detail: &str) -> Self {
        Self::Rejected(detail.into())
    }
}
impl fmt::Display for HuntingMaterialSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(detail) | Self::OutcomeUnknown(detail) => f.write_str(detail),
            Self::EconomyRejected(error) => write!(f, "{error}"),
            Self::KnownCommitted { detail, .. } => {
                write!(f, "material conversion committed; {detail}")
            }
        }
    }
}
impl std::error::Error for HuntingMaterialSourceError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HuntingMaterialFence {
    realm: String,
    database_binding: String,
    owner: WalletOwner,
    quote: MaterialConversionQuoteReceipt,
    guilds: Vec<String>,
    reason: String,
    epoch: u64,
}

pub(crate) fn material_item_digest(value: &Value) -> Result<String, String> {
    canonical_hash(value)
}
fn valid_quote_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && id.bytes().any(|b| b != b'0')
}
fn validate_actor(account: &str, incarnation: &str) -> Result<(), String> {
    validate_token(account, 256)?;
    validate_token(incarnation, 256)?;
    if !incarnation.starts_with("pg-character-v1:") {
        return Err("material source requires the original StartGame incarnation".into());
    }
    Ok(())
}
fn lock_guilds(tx: &mut Transaction<'_>, guilds: &[String]) -> Result<(), EconomyError> {
    for id in guilds {
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,735891))",
            &[id],
        )
        .map_err(source_db)?;
        tx.query_opt(
            "SELECT guild_id FROM shared_guilds WHERE guild_id=$1 FOR UPDATE",
            &[id],
        )
        .map_err(source_db)?;
    }
    Ok(())
}
fn verify_current(tx: &mut Transaction<'_>, image: &LockedImage) -> Result<(), EconomyError> {
    let save = image
        .account
        .saves
        .get(&image.owner.character_index)
        .ok_or(EconomyError::CharacterNotFound)?;
    let row=tx.query_opt("SELECT s.snapshot_json,c.character_name FROM character_saves s JOIN characters c USING(account_id,character_index) WHERE s.account_id=$1 AND s.character_index=$2",
        &[&image.owner.account_id,&image.owner.character_index]).map_err(source_db)?.ok_or(EconomyError::CharacterNotFound)?;
    if row.get::<_, Value>("snapshot_json")
        != to_json(save).map_err(|_| EconomyError::CorruptHistory("material_current_checkpoint"))?
        || row.get::<_, String>("character_name") != save.character.name
    {
        return Err(EconomyError::CorruptHistory("material_current_checkpoint"));
    }
    Ok(())
}
fn read_quote(
    tx: &mut Transaction<'_>,
    realm: &str,
    id: &str,
) -> Result<Option<MaterialConversionQuoteReceipt>, EconomyError> {
    let Some(row)=tx.query_opt("SELECT quote_json,quote_digest FROM hunting_material_conversion_quotes WHERE realm_id=$1 AND quote_id=$2",&[&realm,&id]).map_err(source_db)? else {return Ok(None)};
    let data: Value = row.get("quote_json");
    if canonical_hash(&data).map_err(|_| EconomyError::CorruptHistory("material_quote"))?
        != row.get::<_, String>("quote_digest")
    {
        return Err(EconomyError::CorruptHistory("material_quote_digest"));
    }
    let quote: MaterialConversionQuoteReceipt =
        serde_json::from_value(data).map_err(|_| EconomyError::CorruptHistory("material_quote"))?;
    if quote.realm_id != realm || quote.quote_id != id || !valid_quote_id(id) {
        return Err(EconomyError::CorruptHistory("material_quote_identity"));
    }
    // Service Quote/Confirm/Query subsequently cross-check every SQL column in
    // this same transaction. This read is the private original-fence witness.
    Ok(Some(quote))
}
fn validate_receipt(
    realm: &str,
    quote: &MaterialConversionQuoteReceipt,
    receipt: &MaterialConversionReceipt,
) -> Result<CharacterSaveRecord, EconomyError> {
    let key = economy::material_conversion_operation_key(realm, &quote.owner, &quote.quote_id)?;
    if receipt.realm_id != realm
        || receipt.owner != quote.owner
        || receipt.quote != *quote
        || receipt.operation_key != key
        || receipt.policy_version != quote.policy_version
        || receipt.source.before_revision != quote.input_save_revision
        || Some(receipt.source.after_revision) != quote.input_save_revision.checked_add(1)
        || receipt.source.input_unique_id != quote.input_unique_id
        || receipt.source.output_unique_id == 0
        || (quote.quantity == quote.input_count)
            != (receipt.source.output_unique_id == quote.input_unique_id)
    {
        return Err(EconomyError::CorruptHistory("material_source_receipt"));
    }
    let checkpoint: CharacterSaveRecord = serde_json::from_value(receipt.source.checkpoint.clone())
        .map_err(|_| EconomyError::CorruptHistory("material_source_checkpoint"))?;
    if checkpoint.character.index != quote.owner.character_index
        || checkpoint.revision != receipt.source.after_revision
        || to_json(&checkpoint)
            .map_err(|_| EconomyError::CorruptHistory("material_source_checkpoint"))?
            != receipt.source.checkpoint
    {
        return Err(EconomyError::CorruptHistory("material_source_checkpoint"));
    }
    Ok(checkpoint)
}
fn economic_rejection(error: EconomyError) -> HuntingMaterialSourceError {
    match error {
        EconomyError::OutcomeUnknown => HuntingMaterialSourceError::OutcomeUnknown(
            "material transaction outcome unknown; query the original quote".into(),
        ),
        EconomyError::InsufficientPermits
        | EconomyError::InsufficientUnboundGold
        | EconomyError::MaterialValuationUnavailable
        | EconomyError::MaterialQuoteExpired
        | EconomyError::MaterialQuoteChanged
        | EconomyError::Disabled
        | EconomyError::SettlementRequired { .. } => {
            HuntingMaterialSourceError::EconomyRejected(error)
        }
        _ => HuntingMaterialSourceError::Rejected(
            "material source rejected without committing".into(),
        ),
    }
}

fn same_input(
    quote: &MaterialConversionQuoteReceipt,
    input: &MaterialConversionQuoteSource,
) -> bool {
    quote.item_index == input.item_index
        && quote.input_unique_id == input.input_unique_id
        && quote.input_count == input.input_count
        && quote.quantity == input.quantity
        && quote.input_save_revision == input.input_save_revision
        && quote.input_item_digest == input.input_item_digest
        && quote.source_binding_digest == input.source_binding_digest
}
fn input_from_quote(quote: &MaterialConversionQuoteReceipt) -> MaterialConversionQuoteSource {
    MaterialConversionQuoteSource {
        item_index: quote.item_index,
        input_unique_id: quote.input_unique_id,
        input_count: quote.input_count,
        quantity: quote.quantity,
        input_save_revision: quote.input_save_revision,
        input_item_digest: quote.input_item_digest.clone(),
        source_binding_digest: quote.source_binding_digest.clone(),
    }
}
fn decode_items(values: &[String]) -> Result<Vec<Value>, String> {
    values
        .iter()
        .map(|value| {
            serde_json::from_str(value).map_err(|_| "invalid complete material inventory".into())
        })
        .collect()
}
fn selected_item(before: &CharacterSaveRecord, uid: u64) -> Result<Value, String> {
    let items = decode_items(&before.inventory_items_json)?;
    let mut selected = items
        .into_iter()
        .filter(|item| item.get("unique_id").and_then(Value::as_u64) == Some(uid));
    let item = selected.next().ok_or("material source UID is absent")?;
    if selected.next().is_some() {
        return Err("material source UID is ambiguous".into());
    }
    Ok(item)
}
fn verify_input(
    realm: &str,
    account: &str,
    incarnation: &str,
    index: i32,
    before: &CharacterSaveRecord,
    input: &MaterialConversionQuoteSource,
) -> Result<(), String> {
    crate::runtime::validate_hunting_material_quote_source(
        realm,
        account,
        incarnation,
        index,
        before,
        input,
    )?;
    let item = selected_item(before, input.input_unique_id)?;
    if before.character.index != index
        || before.revision != input.input_save_revision
        || item.get("quantity").and_then(Value::as_u64) != Some(input.input_count)
        || material_item_digest(&item)? != input.input_item_digest
    {
        return Err("material full checkpoint and quoted input disagree".into());
    }
    Ok(())
}
fn sorted_items(items: Vec<Value>) -> Result<Vec<String>, String> {
    let mut values = items
        .iter()
        .map(|item| {
            serde_json::to_string(item).map_err(|_| "material inventory encoding failed".into())
        })
        .collect::<Result<Vec<_>, String>>()?;
    values.sort();
    Ok(values)
}
fn root_uid(item: &Value) -> Option<u64> {
    item.get("unique_id").and_then(Value::as_u64)
}
fn before_contains_uid(before: &CharacterSaveRecord, uid: u64) -> Result<bool, String> {
    for encoded in before
        .inventory_items_json
        .iter()
        .chain(&before.belt_items_json)
        .chain(&before.storage_items_json)
        .chain(&before.equipment_items_json)
        .chain(&before.hero_inventory_items_json)
        .chain(&before.hero_equipment_items_json)
    {
        let value: Value =
            serde_json::from_str(encoded).map_err(|_| "invalid existing material custody")?;
        let mut pending = vec![&value];
        let mut nodes = 0_usize;
        while let Some(value) = pending.pop() {
            nodes += 1;
            if nodes > 32768 {
                return Err("material existing custody exceeds its node budget".into());
            }
            match value {
                Value::Object(fields) => {
                    if fields
                        .get("unique_id")
                        .or_else(|| fields.get("user_item_unique_id"))
                        .and_then(Value::as_u64)
                        == Some(uid)
                    {
                        return Ok(true);
                    }
                    pending.extend(fields.values());
                }
                Value::Array(values) => pending.extend(values),
                _ => {}
            }
        }
    }
    Ok(false)
}
fn validate_transition(
    before: &CharacterSaveRecord,
    after: &CharacterSaveRecord,
    quote: &MaterialConversionQuoteReceipt,
    output_uid: u64,
) -> Result<(), String> {
    if Some(after.revision) != before.revision.checked_add(1)
        || output_uid == 0
        || (quote.quantity == quote.input_count) != (output_uid == quote.input_unique_id)
    {
        return Err("material conversion changed its revision or output identity".into());
    }
    let mut unrelated = after.clone();
    unrelated.revision = before.revision;
    unrelated.inventory_items_json = before.inventory_items_json.clone();
    if to_json(&unrelated)? != to_json(before)? {
        return Err("material conversion changed unrelated character state or legacy gold".into());
    }
    let source = selected_item(before, quote.input_unique_id)?;
    let mut original = decode_items(&before.inventory_items_json)?;
    original.extend(decode_items(&before.belt_items_json)?);
    if original
        .iter()
        .filter(|item| root_uid(item) == Some(quote.input_unique_id))
        .count()
        != 1
    {
        return Err("material source UID is ambiguous across carried collections".into());
    }
    original.retain(|item| root_uid(item) != Some(quote.input_unique_id));
    let mut changed = decode_items(&after.inventory_items_json)?;
    changed.extend(decode_items(&after.belt_items_json)?);
    let outputs: Vec<_> = changed
        .iter()
        .filter(|item| root_uid(item) == Some(output_uid))
        .collect();
    if outputs.len() != 1 {
        return Err("material converted output is absent or ambiguous".into());
    }
    let output = outputs[0];
    let conversion = output
        .get("economic_conversion")
        .filter(|value| value.is_object())
        .ok_or("material output lost explicit conversion provenance")?;
    let raw_valuation = quote
        .valuation_digest
        .strip_prefix("sha256:")
        .filter(|digest| valid_quote_id(digest))
        .ok_or("material valuation digest is invalid")?;
    let lot = conversion
        .get("lotId")
        .and_then(Value::as_str)
        .filter(|id| valid_quote_id(id))
        .ok_or("material output lot is invalid")?;
    let expected_conversion = json!({"version":1,"realm":quote.realm_id,"operationId":economy::material_conversion_operation_key(&quote.realm_id,&quote.owner,&quote.quote_id).map_err(|_|"material operation identity unavailable")?,
        "quoteId":quote.quote_id,"valuationVersion":quote.valuation_version,"valuationDigest":raw_valuation,
        "sourceBindingDigest":quote.source_binding_digest,"inputUniqueId":quote.input_unique_id,"lotId":lot});
    if conversion != &expected_conversion {
        return Err("material output conversion lineage changed".into());
    }
    let mut expected_output = source.clone();
    expected_output
        .as_object_mut()
        .ok_or("material source is not an object")?
        .remove("economic_binding");
    expected_output["economic_conversion"] = expected_conversion;
    expected_output["unique_id"] = json!(output_uid);
    expected_output["quantity"] = json!(quote.quantity);
    if quote.quantity < quote.input_count {
        // The source API is private, but still constrain a future planner to a
        // new canonical empty bag cell. Belt contents remain byte-for-byte.
        let container: ItemContainer = serde_json::from_value(output["container"].clone())
            .map_err(|_| "material output bag is invalid")?;
        let slot = output["slot"]
            .as_u64()
            .and_then(|slot| u8::try_from(slot).ok())
            .filter(|slot| *slot < 40)
            .ok_or("material output slot is invalid")?;
        let logical = match container {
            ItemContainer::Bag1 => slot,
            ItemContainer::Bag2 => 40 + slot,
            _ => return Err("material output must remain in the personal bag".into()),
        };
        if u16::from(logical) >= crystal_bag_slot_capacity(before.inventory_capacity)
            || decode_items(&before.inventory_items_json)?
                .iter()
                .any(|item| {
                    let Some(slot) = item["slot"].as_u64() else {
                        return false;
                    };
                    let base = if item["container"] == json!(ItemContainer::Bag1) {
                        0
                    } else if item["container"] == json!(ItemContainer::Bag2) {
                        40
                    } else {
                        return false;
                    };
                    slot.checked_add(base) == Some(u64::from(logical))
                })
        {
            return Err("material output bag cell is occupied or out of capacity".into());
        }
        for field in ["container", "slot"] {
            expected_output[field] = output[field].clone();
        }
    }
    if &expected_output != output {
        return Err("material output changed non-conversion instance state".into());
    }
    changed.retain(|item| root_uid(item) != Some(output_uid));
    if quote.quantity < quote.input_count {
        if before_contains_uid(before, output_uid)? {
            return Err("material partial output reused an existing UID".into());
        }
        let remainder: Vec<_> = changed
            .iter()
            .filter(|item| root_uid(item) == Some(quote.input_unique_id))
            .collect();
        let mut expected = source;
        expected["quantity"] = json!(quote.input_count - quote.quantity);
        if remainder.len() != 1 || remainder[0] != &expected {
            return Err("material bound remainder changed".into());
        }
        changed.retain(|item| root_uid(item) != Some(quote.input_unique_id));
    }
    if sorted_items(original)? != sorted_items(changed)? {
        return Err("material conversion changed another inventory item".into());
    }
    Ok(())
}

struct Prepared {
    quote: MaterialConversionQuoteReceipt,
    before: CharacterSaveRecord,
    output_uid: u64,
    original: AccountStore,
    staged: AccountStore,
    plan: AccountStoreMutationPlan,
}
impl Prepared {
    fn after(&self) -> &CharacterSaveRecord {
        &self.staged.accounts[&self.quote.owner.account_id].saves[&self.quote.owner.character_index]
    }
}
fn prepare<F>(
    original: AccountStore,
    quote: MaterialConversionQuoteReceipt,
    before: CharacterSaveRecord,
    output_uid: u64,
    stage: F,
) -> Result<Prepared, String>
where
    F: FnOnce(&mut AccountStore) -> Result<(), String>,
{
    let account = &quote.owner.account_id;
    let index = quote.owner.character_index;
    verify_input(
        &quote.realm_id,
        account,
        &quote.owner.incarnation,
        index,
        &before,
        &input_from_quote(&quote),
    )?;
    let durable = original
        .accounts
        .get(account)
        .and_then(|account| account.saves.get(&index))
        .ok_or("material durable checkpoint missing")?;
    if durable.revision != before.revision || durable.character.name != before.character.name {
        return Err("material durable revision changed".into());
    }
    let mut staged = original.clone();
    stage(&mut staged)?;
    let accounts = [account.clone()];
    validate_account_store_transaction_scope(
        &original,
        &staged,
        AccountStoreMutationScope::Accounts(&accounts),
    )
    .map_err(|error| error.to_string())?;
    let after = staged
        .accounts
        .get(account)
        .and_then(|account| account.saves.get(&index))
        .ok_or("material staged checkpoint missing")?;
    validate_transition(&before, after, &quote, output_uid)?;
    let guilds =
        guild_experience::settle_authorized_sources(&original, &mut staged, &accounts, None)?
            .into_iter()
            .collect::<Vec<_>>();
    let scope = AccountStoreMutationScope::AccountsWithGuilds {
        account_ids: &accounts,
        guild_ids: &guilds,
    };
    validate_account_store_transaction_scope(&original, &staged, scope)
        .map_err(|error| error.to_string())?;
    let after = &staged.accounts[account].saves[&index];
    if after.character.index != index
        || after.character.name != before.character.name
        || after.gold != before.gold
        || i64::try_from(after.revision).is_err()
    {
        return Err("material complete checkpoint identity changed".into());
    }
    let mut expected = original.accounts[account].clone();
    expected.saves.insert(index, after.clone());
    if let Some(character) = expected
        .characters
        .iter_mut()
        .find(|character| character.index == index)
    {
        *character = after.character.clone()
    }
    if to_json(&expected)? != to_json(&staged.accounts[account])? {
        return Err("material conversion changed unrelated account state".into());
    }
    let plan = build_account_store_mutation_plan(&original, &staged, scope, false);
    if plan.global_stock.is_some()
        || plan.clock.is_some()
        || !plan.heroes.is_empty()
        || plan.hero_allocator.is_some()
    {
        return Err("material conversion changed unrelated shared state".into());
    }
    serde_json::to_vec(&staged).map_err(|_| "material complete checkpoint encoding failed")?;
    Ok(Prepared {
        quote,
        before,
        output_uid,
        original,
        staged,
        plan,
    })
}

impl SimulationConfig {
    fn material_commit(
        &self,
        realm: &str,
        url: &str,
        quote: &MaterialConversionQuoteReceipt,
        guilds: Vec<String>,
        receipt: Idempotent<MaterialConversionReceipt>,
        current_revision: Option<u64>,
    ) -> Result<HuntingMaterialCommit, EconomyError> {
        let checkpoint = validate_receipt(realm, quote, &receipt.receipt)?;
        let hash = canonical_hash(
            &to_json(&receipt.receipt)
                .map_err(|_| EconomyError::CorruptHistory("material_publication"))?,
        )
        .map_err(|_| EconomyError::CorruptHistory("material_publication"))?;
        Ok(HuntingMaterialCommit {
            checkpoint,
            current_revision,
            receipt,
            publication: HuntingMaterialPublication {
                binding: Arc::clone(&self.account_store),
                receipt_digest: hash,
                fence: HuntingMaterialFence {
                    realm: realm.into(),
                    database_binding: database_binding(url),
                    owner: quote.owner.clone(),
                    quote: quote.clone(),
                    guilds,
                    reason: String::new(),
                    epoch: 0,
                },
            },
        })
    }
    fn fence_hunting_material(
        &self,
        realm: &str,
        url: &str,
        quote: &MaterialConversionQuoteReceipt,
        guilds: Vec<String>,
        detail: String,
    ) -> String {
        // Material authority is PG-only. Linearize the private fence with the
        // write state; repeating an existing freeze must not advance its epoch.
        let mut state = match self.account_store_write_state.lock() {
            Ok(state) => state,
            Err(_) => return "material write-state unavailable; writes remain fail-closed".into(),
        };
        self.fence_hunting_material_locked(&mut state, realm, url, quote, guilds, detail)
    }
    fn fence_hunting_material_locked(
        &self,
        state: &mut AccountStoreWriteState,
        realm: &str,
        url: &str,
        quote: &MaterialConversionQuoteReceipt,
        guilds: Vec<String>,
        detail: String,
    ) -> String {
        if let AccountStoreWriteState::Frozen { reason } = state {
            return reason.clone();
        }
        let reason = format!("{detail}; live AccountStore was not published");
        let epoch = self
            .account_store_freeze_epoch
            .fetch_add(1, Ordering::AcqRel)
            + 1;
        *state = AccountStoreWriteState::Frozen {
            reason: reason.clone(),
        };
        if let Ok(mut slot) = self.hunting_material_fence.lock() {
            *slot = Some(HuntingMaterialFence {
                realm: realm.into(),
                database_binding: database_binding(url),
                owner: quote.owner.clone(),
                quote: quote.clone(),
                guilds,
                reason: reason.clone(),
                epoch,
            });
        }
        reason
    }
    pub(crate) fn fence_committed_hunting_material(
        &self,
        commit: &HuntingMaterialCommit,
        detail: String,
    ) -> Result<String, String> {
        let seal = &commit.publication;
        let expected = &seal.fence;
        let (realm, url) = self.hunting_source_binding()?;
        if !Arc::ptr_eq(&seal.binding, &self.account_store)
            || realm != expected.realm
            || database_binding(url) != expected.database_binding
            || canonical_hash(&to_json(&commit.receipt.receipt)?)? != seal.receipt_digest
            || to_json(&commit.checkpoint)? != commit.receipt.receipt.source.checkpoint
        {
            return Err("material publication seal does not belong to this original source".into());
        }
        validate_receipt(realm, &expected.quote, &commit.receipt.receipt)
            .map_err(|_| "material publication receipt is invalid")?;
        Ok(self.fence_hunting_material(
            realm,
            url,
            &expected.quote,
            expected.guilds.clone(),
            detail,
        ))
    }
    pub(crate) fn register_hunting_material_quote(
        &self,
        account: &str,
        incarnation: &str,
        index: i32,
        request_id: &str,
        before: CharacterSaveRecord,
        input: MaterialConversionQuoteSource,
    ) -> Result<MaterialConversionQuoteReceipt, HuntingMaterialSourceError> {
        validate_actor(account, incarnation)?;
        validate_token(request_id, 256)?;
        let _persist = self
            .account_store_persist_lock
            .lock()
            .map_err(|_| "material quote persistence lock poisoned")?;
        let write_state = self
            .account_store_write_state
            .lock()
            .map_err(|_| "material quote write state unavailable")?;
        if !matches!(&*write_state, AccountStoreWriteState::Writable) {
            return Err("material quote is fenced by another source".into());
        }
        let (realm, url) = self.hunting_source_binding()?;
        verify_input(realm, account, incarnation, index, &before, &input)?;
        // Snapshot under the state guard, then release live before entering PG.
        let original = self
            .account_store
            .lock()
            .map_err(|_| "material quote mirror unavailable")?
            .accounts
            .get(account)
            .cloned()
            .ok_or("material quote account unavailable")?;
        if original.saves.get(&index).is_none_or(|save| {
            save.revision != before.revision || save.character.name != before.character.name
        }) {
            return Err("material quote save revision changed".into());
        }
        let result = with_account_store_postgres_client(url, |client| {
            let result = economy::quote_material_conversion_authorized_with_source_with_timeouts(
                client,
                realm,
                request_id,
                input.input_unique_id,
                input.quantity,
                2000,
                10000,
                |tx, _| {
                    let image = lock_and_load(
                        tx,
                        account,
                        index,
                        Some(&before.character),
                        std::iter::empty(),
                        false,
                    )?
                    .ok_or(EconomyError::CharacterNotFound)?;
                    if image.owner.incarnation != incarnation
                        || to_json(&image.account).map_err(|_| EconomyError::SourceRejected)?
                            != to_json(&original).map_err(|_| EconomyError::SourceRejected)?
                    {
                        return Err(EconomyError::SourceRejected);
                    }
                    verify_current(tx, &image)?;
                    Ok(image.owner)
                },
                |_, _, owner| {
                    if owner.account_id != account
                        || owner.character_index != index
                        || owner.incarnation != incarnation
                    {
                        return Err(EconomyError::SourceRejected);
                    }
                    Ok(input.clone())
                },
            );
            Ok(result)
        })
        .map_err(|_| {
            HuntingMaterialSourceError::OutcomeUnknown(
                "material quote repository outcome unknown; retry its original server request"
                    .into(),
            )
        })?;
        let result = result.map_err(economic_rejection)?;
        if result.receipt.owner.account_id != account
            || result.receipt.owner.character_index != index
            || result.receipt.owner.incarnation != incarnation
            || result.receipt.realm_id != realm
            || result.receipt.request_id != request_id
            || !same_input(&result.receipt, &input)
        {
            return Err("material quote no longer describes this complete original source".into());
        }
        Ok(result.receipt)
    }
    pub(crate) fn publish_hunting_material_source<F>(
        &self,
        account: &str,
        incarnation: &str,
        quote_id: &str,
        before: CharacterSaveRecord,
        output_unique_id: u64,
        stage: F,
    ) -> Result<HuntingMaterialCommit, HuntingMaterialSourceError>
    where
        F: FnOnce(&mut AccountStore) -> Result<(), String>,
    {
        self.publish_hunting_material_source_inner(
            account,
            incarnation,
            quote_id,
            before,
            output_unique_id,
            stage,
            0,
        )
    }
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn publish_hunting_material_source_with_test_fault<F>(
        &self,
        account: &str,
        incarnation: &str,
        quote_id: &str,
        before: CharacterSaveRecord,
        output_unique_id: u64,
        stage: F,
        fault: u8,
    ) -> Result<HuntingMaterialCommit, HuntingMaterialSourceError>
    where
        F: FnOnce(&mut AccountStore) -> Result<(), String>,
    {
        self.publish_hunting_material_source_inner(
            account,
            incarnation,
            quote_id,
            before,
            output_unique_id,
            stage,
            fault,
        )
    }
    fn publish_hunting_material_source_inner<F>(
        &self,
        account: &str,
        incarnation: &str,
        quote_id: &str,
        before: CharacterSaveRecord,
        output_unique_id: u64,
        stage: F,
        test_fault: u8,
    ) -> Result<HuntingMaterialCommit, HuntingMaterialSourceError>
    where
        F: FnOnce(&mut AccountStore) -> Result<(), String>,
    {
        validate_actor(account, incarnation)?;
        if !valid_quote_id(quote_id) {
            return Err("invalid material quote identity".into());
        }
        if let Some(commit) = self.lookup_hunting_material_source(
            account,
            incarnation,
            before.character.index,
            quote_id,
        )? {
            return Ok(commit);
        }
        let _persist = self
            .account_store_persist_lock
            .lock()
            .map_err(|_| "material persistence lock poisoned")?;
        let mut write_state = self
            .account_store_write_state
            .lock()
            .map_err(|_| "material write state unavailable")?;
        if !matches!(&*write_state, AccountStoreWriteState::Writable) {
            return Err("material source is fenced by another operation".into());
        }
        let (realm, url) = self.hunting_source_binding()?;
        let quote = with_account_store_postgres_client(url, |client| {
            economy::read_material_conversion_quote_authorized_with_timeouts(
                client,
                realm,
                quote_id,
                2000,
                10000,
                |tx, _| {
                    let image = lock_and_load(
                        tx,
                        account,
                        before.character.index,
                        Some(&before.character),
                        std::iter::empty(),
                        false,
                    )?
                    .ok_or(EconomyError::CharacterNotFound)?;
                    if image.owner.incarnation != incarnation {
                        return Err(EconomyError::CharacterNotFound);
                    }
                    verify_current(tx, &image)?;
                    Ok(image.owner)
                },
            )
            .map_err(|_| "material registered quote unavailable".into())
        })
        .map_err(|_| "material quote repository unavailable")?
        .ok_or("material quote was not registered")?;
        let original = self
            .account_store
            .lock()
            .map_err(|_| "material account image unavailable")?
            .clone();
        let prepared = prepare(original, quote, before, output_unique_id, stage)?;
        let quote = &prepared.quote;
        let guilds = prepared.plan.guilds.keys().cloned().collect::<Vec<_>>();
        let locked = RefCell::new(None::<LockedImage>);
        let written = RefCell::new(None::<AccountStoreSourceVersions>);
        let delivered = with_account_store_postgres_client(url, |client| {
            let authorize = |tx: &mut Transaction<'_>, _: SourceContext| {
                prepared
                    .plan
                    .lock_guild_plan(tx)
                    .map_err(|_| EconomyError::SourceRejected)?;
                let image = lock_and_load(
                    tx,
                    account,
                    quote.owner.character_index,
                    Some(&prepared.before.character),
                    prepared.plan.guilds.keys().cloned(),
                    false,
                )?
                .ok_or(EconomyError::CharacterNotFound)?;
                if image.owner != quote.owner {
                    return Err(EconomyError::CharacterNotFound);
                }
                verify_current(tx, &image)?;
                if read_quote(tx, realm, quote_id)?.as_ref() != Some(quote) {
                    return Err(EconomyError::RequestConflict);
                }
                let owner = image.owner.clone();
                *locked.borrow_mut() = Some(image);
                Ok(owner)
            };
            let source = |tx: &mut Transaction<'_>,
                          _: SourceContext,
                          current: &MaterialConversionQuoteReceipt,
                          _: &str| {
                if current != quote {
                    return Err(EconomyError::RequestConflict);
                }
                {
                    let image = locked.borrow();
                    let image = image.as_ref().ok_or(EconomyError::SourceRejected)?;
                    if to_json(&image.account).map_err(|_| EconomyError::SourceRejected)?
                        != to_json(&prepared.original.accounts[account])
                            .map_err(|_| EconomyError::SourceRejected)?
                    {
                        return Err(EconomyError::SourceRejected);
                    }
                    for id in prepared.plan.guilds.keys() {
                        if to_json(&image.guilds.get(id).cloned().flatten())
                            .map_err(|_| EconomyError::SourceRejected)?
                            != to_json(&prepared.original.shared_guilds.get(id))
                                .map_err(|_| EconomyError::SourceRejected)?
                        {
                            return Err(EconomyError::SourceRejected);
                        }
                    }
                }
                let versions = write_account_store_mutation_plan_in_transaction(
                    tx,
                    &prepared.plan,
                    AccountStoreDatabaseMode::SourceOfTruth,
                )
                .map_err(|_| EconomyError::SourceRejected)?;
                *written.borrow_mut() = Some(versions);
                #[cfg(any(test, feature = "test-support"))]
                if test_fault == 1 || test_fault == 4 {
                    return Err(EconomyError::SourceRejected);
                }
                Ok(MaterialConversionSourceResult {
                    before_revision: prepared.before.revision,
                    after_revision: prepared.after().revision,
                    input_unique_id: quote.input_unique_id,
                    output_unique_id: prepared.output_uid,
                    checkpoint: to_json(prepared.after())
                        .map_err(|_| EconomyError::SourceRejected)?,
                })
            };
            Ok(
                economy::confirm_material_conversion_authorized_with_source_with_timeouts(
                    client, realm, quote_id, 2000, 10000, authorize, source,
                ),
            )
        });
        let outcome = delivered.unwrap_or(Err(EconomyError::OutcomeUnknown));
        #[cfg(any(test, feature = "test-support"))]
        let outcome =
            if ((test_fault == 2 || test_fault == 5) && outcome.is_ok()) || test_fault == 4 {
                Err(EconomyError::OutcomeUnknown)
            } else {
                outcome
            };
        #[cfg(not(any(test, feature = "test-support")))]
        let _ = test_fault;
        let receipt = match outcome {
            Ok(receipt) => receipt,
            Err(EconomyError::OutcomeUnknown) | Err(EconomyError::CorruptHistory(_)) => {
                let reason = self.fence_hunting_material_locked(
                    &mut write_state,
                    realm,
                    url,
                    quote,
                    guilds,
                    "material COMMIT outcome unknown; query the original quote".into(),
                );
                return Err(HuntingMaterialSourceError::OutcomeUnknown(reason));
            }
            Err(error) => return Err(economic_rejection(error)),
        };
        let current_revision = if receipt.replayed {
            locked.borrow().as_ref().and_then(|image| {
                image
                    .account
                    .saves
                    .get(&quote.owner.character_index)
                    .map(|save| save.revision)
            })
        } else {
            Some(receipt.receipt.source.after_revision)
        };
        let commit = match self.material_commit(
            realm,
            url,
            quote,
            guilds.clone(),
            receipt,
            current_revision,
        ) {
            Ok(commit) => commit,
            Err(_) => {
                let reason = self.fence_hunting_material_locked(
                    &mut write_state,
                    realm,
                    url,
                    quote,
                    guilds,
                    "material committed result failed complete evidence validation".into(),
                );
                return Err(HuntingMaterialSourceError::OutcomeUnknown(reason));
            }
        };
        #[cfg(any(test, feature = "test-support"))]
        if test_fault == 3 {
            let detail = self.fence_hunting_material_locked(
                &mut write_state,
                realm,
                url,
                quote,
                guilds,
                "injected material publication failure after known COMMIT".into(),
            );
            return Err(HuntingMaterialSourceError::KnownCommitted {
                commit: Box::new(commit),
                detail,
            });
        }
        let Some(mut image) = locked.into_inner() else {
            let detail = self.fence_hunting_material_locked(
                &mut write_state,
                realm,
                url,
                quote,
                guilds,
                "material committed current source image unavailable".into(),
            );
            return Err(HuntingMaterialSourceError::KnownCommitted {
                commit: Box::new(commit),
                detail,
            });
        };
        let versions = if commit.receipt.replayed {
            image.versions
        } else {
            let Some(versions) = written.into_inner() else {
                let detail = self.fence_hunting_material_locked(
                    &mut write_state,
                    realm,
                    url,
                    quote,
                    guilds,
                    "material committed source versions unavailable".into(),
                );
                return Err(HuntingMaterialSourceError::KnownCommitted {
                    commit: Box::new(commit),
                    detail,
                });
            };
            image.account = prepared.staged.accounts[account].clone();
            image.guilds = prepared
                .plan
                .guilds
                .keys()
                .map(|id| (id.clone(), prepared.staged.shared_guilds.get(id).cloned()))
                .collect();
            versions
        };
        let mut live = match self.account_store.lock() {
            Ok(live) => live,
            Err(_) => {
                let detail = self.fence_hunting_material_locked(
                    &mut write_state,
                    realm,
                    url,
                    quote,
                    guilds,
                    "material mirror unavailable after known COMMIT".into(),
                );
                return Err(HuntingMaterialSourceError::KnownCommitted {
                    commit: Box::new(commit),
                    detail,
                });
            }
        };
        // No live mutex crosses PG. Even an unexpected local writer cannot be
        // overwritten by the old prepared image after the commit is known.
        let account_unchanged = match (
            to_json(&live.accounts.get(account)),
            to_json(&prepared.original.accounts.get(account)),
        ) {
            (Ok(actual), Ok(expected)) => actual == expected,
            _ => false,
        };
        let guilds_unchanged = prepared.plan.guilds.keys().all(|id| {
            match (
                to_json(&live.shared_guilds.get(id)),
                to_json(&prepared.original.shared_guilds.get(id)),
            ) {
                (Ok(actual), Ok(expected)) => actual == expected,
                _ => false,
            }
        });
        if !account_unchanged || !guilds_unchanged {
            let detail = self.fence_hunting_material_locked(
                &mut write_state,
                realm,
                url,
                quote,
                guilds,
                "material live mirror changed after known COMMIT".into(),
            );
            return Err(HuntingMaterialSourceError::KnownCommitted {
                commit: Box::new(commit),
                detail,
            });
        }
        live.accounts.insert(account.into(), image.account);
        for (id, guild) in image.guilds {
            if let Some(guild) = guild {
                live.shared_guilds.insert(id, guild);
            } else {
                live.shared_guilds.remove(&id);
            }
        }
        apply_account_store_mutation_source_versions(&mut live, &prepared.plan, versions);
        self.guild_clock_updates.fetch_add(1, Ordering::AcqRel);
        Ok(commit)
    }
}

#[path = "config_hunting_material_recovery.rs"]
mod recovery;
