//! Material-specific authority. Authorization runs on every request; the source
//! callback runs only on the first successful confirmation, in this transaction.
//! No function here mints coins or accepts a client price/owner/item snapshot.
use super::*;
use mir2_economy_policy::{
    quote_material_conversion_for_item, MaterialConversionQuote, MaterialValuationSet,
};
use serde::Deserialize;

pub const MATERIAL_QUOTE_TTL_MS: i64 = 30_000;

/// A main-server callback reads and validates this complete canonical source.
/// Deliberately not Deserialize: a packet cannot become source authorization.
#[derive(Debug, Clone)]
pub struct MaterialConversionQuoteSource {
    pub item_index: i32,
    pub input_unique_id: u64,
    pub input_count: u64,
    pub quantity: u64,
    pub input_save_revision: u64,
    pub input_item_digest: String,
    pub source_binding_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialConversionQuoteReceipt {
    pub realm_id: String,
    pub quote_id: String,
    pub request_digest: String,
    pub owner: WalletOwner,
    pub request_id: String,
    pub item_index: i32,
    pub input_unique_id: u64,
    pub input_count: u64,
    pub quantity: u64,
    pub input_save_revision: u64,
    pub input_item_digest: String,
    pub source_binding_digest: String,
    pub valuation_version: String,
    pub valuation_digest: String,
    pub valuation_revision: u64,
    pub policy_version: String,
    pub unit_gold: u64,
    pub quote: MaterialConversionQuote,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
}

/// Full durable source outcome, not a request body. The caller must write this
/// same checkpoint with its normal character CAS, not merely return JSON here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialConversionSourceResult {
    pub before_revision: u64,
    pub after_revision: u64,
    pub input_unique_id: u64,
    pub output_unique_id: u64,
    pub checkpoint: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialConversionReceipt {
    pub realm_id: String,
    pub operation_key: String,
    pub request_digest: String,
    pub owner: WalletOwner,
    pub quote: MaterialConversionQuoteReceipt,
    pub source: MaterialConversionSourceResult,
    pub policy_version: String,
    pub business_day: i64,
    pub committed_at_ms: i64,
    pub wallet_after: WalletBalance,
    pub pool_after_atoms: u64,
    pub burn_event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveMaterialValuation {
    pub valuation_version: String,
    pub valuation_digest: String,
    pub revision: u64,
}

fn raw_hash<T: Serialize>(value: &T) -> EconomyResult<String> {
    Ok(digest(value)?
        .strip_prefix("sha256:")
        .ok_or(EconomyError::CorruptHistory("hash_domain"))?
        .into())
}
fn hex_id(value: &str, field: &'static str) -> EconomyResult<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || value.bytes().all(|b| b == b'0')
    {
        return Err(EconomyError::InvalidInput(field));
    }
    Ok(())
}
pub fn material_conversion_operation_key(
    realm: &str,
    owner: &WalletOwner,
    quote_id: &str,
) -> EconomyResult<String> {
    token(realm, "realm_id", 128)?;
    validate_owner(owner)?;
    hex_id(quote_id, "material_quote_id")?;
    raw_hash(&("hunting_material_conversion_v1", realm, owner, quote_id))
}

/// Registration never activates a price or touches a player asset. An empty
/// official configuration is valid and quotes every item as unconfigured.
pub fn register_material_valuation(
    client: &mut Client,
    realm: &str,
    set: &MaterialValuationSet,
) -> EconomyResult<String> {
    transaction(client, realm, |tx, clock| {
        load_pool(tx, realm)?;
        let data = encode(set)?;
        let hash = digest(&("hunting_material_valuation_v1", &data))?;
        if let Some(row) = tx.query_opt("SELECT valuation_digest,config_json FROM hunting_material_valuations WHERE realm_id=$1 AND valuation_version=$2", &[&realm,&set.valuation_version()]).map_err(db)? {
            if row.get::<_,String>("valuation_digest") != hash || row.get::<_,Value>("config_json") != data {
                return Err(EconomyError::PolicyConflict);
            }
            return Ok(hash);
        }
        tx.execute("INSERT INTO hunting_material_valuations(realm_id,valuation_version,valuation_digest,config_json,created_at_ms) VALUES($1,$2,$3,$4,$5)", &[&realm,&set.valuation_version(),&hash,&data,&clock.database_time_ms]).map_err(db)?;
        Ok(hash)
    })
}

fn load_valuation(
    tx: &mut Transaction<'_>,
    realm: &str,
    version: &str,
) -> EconomyResult<(MaterialValuationSet, String)> {
    let row = tx.query_opt("SELECT valuation_digest,config_json FROM hunting_material_valuations WHERE realm_id=$1 AND valuation_version=$2", &[&realm,&version]).map_err(db)?.ok_or(EconomyError::MaterialValuationUnavailable)?;
    let data: Value = row.get("config_json");
    let hash: String = row.get("valuation_digest");
    let set: MaterialValuationSet = decode(data.clone())?;
    if set.valuation_version() != version
        || digest(&("hunting_material_valuation_v1", &data))? != hash
    {
        return Err(EconomyError::CorruptHistory("material_valuation_digest"));
    }
    Ok((set, hash))
}
fn active_valuation(
    tx: &mut Transaction<'_>,
    realm: &str,
) -> EconomyResult<(MaterialValuationSet, ActiveMaterialValuation)> {
    let row = tx.query_opt("SELECT valuation_version,valuation_digest,revision FROM hunting_material_active_valuations WHERE realm_id=$1 FOR UPDATE", &[&realm]).map_err(db)?.ok_or(EconomyError::MaterialValuationUnavailable)?;
    let version: String = row.get("valuation_version");
    let (set, hash) = load_valuation(tx, realm, &version)?;
    if hash != row.get::<_, String>("valuation_digest") {
        return Err(EconomyError::CorruptHistory("material_active_valuation"));
    }
    Ok((
        set,
        ActiveMaterialValuation {
            valuation_version: version,
            valuation_digest: hash,
            revision: unsigned(row.get("revision"), "material_valuation_revision")?,
        },
    ))
}
pub fn activate_material_valuation(
    client: &mut Client,
    realm: &str,
    version: &str,
) -> EconomyResult<ActiveMaterialValuation> {
    token(version, "valuation_version", 128)?;
    transaction(client, realm, |tx, clock| {
        load_pool(tx, realm)?;
        let (_, hash) = load_valuation(tx, realm, version)?;
        let old = tx.query_opt("SELECT valuation_version,valuation_digest,revision FROM hunting_material_active_valuations WHERE realm_id=$1 FOR UPDATE", &[&realm]).map_err(db)?;
        let revision = if let Some(row) = old {
            let revision = unsigned(row.get("revision"), "material_valuation_revision")?;
            if row.get::<_, String>("valuation_version") == version
                && row.get::<_, String>("valuation_digest") == hash
            {
                return Ok(ActiveMaterialValuation {
                    valuation_version: version.into(),
                    valuation_digest: hash,
                    revision,
                });
            }
            checked_add(revision, 1, "material_valuation_revision")?
        } else {
            1
        };
        tx.execute("INSERT INTO hunting_material_active_valuations(realm_id,valuation_version,valuation_digest,revision,activated_at_ms) VALUES($1,$2,$3,$4,$5) ON CONFLICT(realm_id) DO UPDATE SET valuation_version=EXCLUDED.valuation_version,valuation_digest=EXCLUDED.valuation_digest,revision=EXCLUDED.revision,activated_at_ms=EXCLUDED.activated_at_ms", &[&realm,&version,&hash,&bigint(revision,"material_valuation_revision")?,&clock.database_time_ms]).map_err(db)?;
        let active = ActiveMaterialValuation {
            valuation_version: version.into(),
            valuation_digest: hash,
            revision,
        };
        outbox(
            tx,
            realm,
            "material_valuation_activated",
            &revision.to_string(),
            &encode(&active)?,
            clock.database_time_ms,
        )?;
        Ok(active)
    })
}

fn load_quote(
    tx: &mut Transaction<'_>,
    realm: &str,
    id: &str,
) -> EconomyResult<Option<MaterialConversionQuoteReceipt>> {
    let Some(row) = tx.query_opt("SELECT *,input_unique_id::text AS input_uid_text FROM hunting_material_conversion_quotes WHERE realm_id=$1 AND quote_id=$2", &[&realm,&id]).map_err(db)? else { return Ok(None) };
    let data: Value = row.get("quote_json");
    if digest(&data)? != row.get::<_, String>("quote_digest") {
        return Err(EconomyError::CorruptHistory("material_quote_digest"));
    }
    let quote: MaterialConversionQuoteReceipt = decode(data)?;
    let uid = row
        .get::<_, String>("input_uid_text")
        .parse::<u64>()
        .map_err(|_| EconomyError::CorruptHistory("material_input_uid"))?;
    if quote.realm_id != realm
        || quote.quote_id != id
        || quote.request_digest != row.get::<_, String>("request_digest")
        || quote.owner.account_id != row.get::<_, String>("account_id")
        || quote.owner.character_index != row.get::<_, i32>("character_index")
        || quote.owner.incarnation != row.get::<_, String>("character_incarnation")
        || quote.item_index != row.get::<_, i32>("item_index")
        || quote.input_unique_id != uid
        || quote.quantity != unsigned(row.get("quantity"), "material_quantity")?
        || quote.input_count != unsigned(row.get("input_count"), "material_input_count")?
        || quote.input_save_revision
            != unsigned(row.get("input_save_revision"), "material_source_revision")?
        || quote.input_item_digest != row.get::<_, String>("input_item_digest")
        || quote.source_binding_digest != row.get::<_, String>("source_binding_digest")
        || quote.policy_version != row.get::<_, String>("policy_version")
        || quote.valuation_version != row.get::<_, String>("valuation_version")
        || quote.valuation_digest != row.get::<_, String>("valuation_digest")
        || quote.valuation_revision
            != unsigned(row.get("valuation_revision"), "material_valuation_revision")?
        || quote.unit_gold != unsigned(row.get("unit_gold"), "material_unit_gold")?
        || quote.quote.gold_equivalent
            != unsigned(row.get("gold_equivalent"), "material_equivalent")?
        || quote.quote.fee_unbound_gold != unsigned(row.get("fee_gold"), "material_fee")?
        || quote.quote.license_atoms_debit != unsigned(row.get("consumed_atoms"), "material_atoms")?
        || quote.created_at_ms != row.get::<_, i64>("created_at_ms")
        || quote.expires_at_ms != row.get::<_, i64>("expires_at_ms")
    {
        return Err(EconomyError::CorruptHistory("material_quote_columns"));
    }
    Ok(Some(quote))
}

fn material_request_digest(quote: &MaterialConversionQuoteReceipt) -> EconomyResult<String> {
    digest(&("hunting_material_confirmation_v1", quote))
}

fn material_existing(
    tx: &mut Transaction<'_>,
    realm: &str,
    quote: &MaterialConversionQuoteReceipt,
) -> EconomyResult<Option<MaterialConversionReceipt>> {
    let key = material_conversion_operation_key(realm, &quote.owner, &quote.quote_id)?;
    let hash = material_request_digest(quote)?;
    let receipt: Option<MaterialConversionReceipt> =
        existing(tx, realm, &key, "material_conversion", &hash)?;
    let result=tx.query_opt("SELECT operation_key,receipt_json,receipt_digest,before_revision,after_revision,committed_at_ms FROM hunting_material_conversion_results WHERE realm_id=$1 AND quote_id=$2", &[&realm,&quote.quote_id]).map_err(db)?;
    match (receipt, result) {
        (None, None) => Ok(None),
        (Some(receipt), Some(row)) => {
            let data: Value = row.get("receipt_json");
            if receipt.realm_id != realm
                || receipt.owner != quote.owner
                || receipt.quote != *quote
                || receipt.operation_key != key
                || receipt.request_digest != hash
                || row.get::<_, String>("operation_key") != key
                || digest(&data)? != row.get::<_, String>("receipt_digest")
                || data != encode(&receipt)?
                || unsigned(row.get("before_revision"), "material_before_revision")?
                    != receipt.source.before_revision
                || unsigned(row.get("after_revision"), "material_after_revision")?
                    != receipt.source.after_revision
                || row.get::<_, i64>("committed_at_ms") != receipt.committed_at_ms
            {
                return Err(EconomyError::CorruptHistory("material_result_evidence"));
            }
            let ledger=tx.query_opt("SELECT quote_id,gold_equivalent,fee_gold,consumed_atoms FROM hunting_economy_material_conversions WHERE realm_id=$1 AND operation_key=$2", &[&realm,&key]).map_err(db)?
                .ok_or(EconomyError::CorruptHistory("material_result_ledger"))?;
            if ledger.get::<_, String>("quote_id") != quote.quote_id
                || unsigned(ledger.get("gold_equivalent"), "material_equivalent")?
                    != quote.quote.gold_equivalent
                || unsigned(ledger.get("fee_gold"), "material_fee")? != quote.quote.fee_unbound_gold
                || unsigned(ledger.get("consumed_atoms"), "material_atoms")?
                    != quote.quote.license_atoms_debit
            {
                return Err(EconomyError::CorruptHistory("material_result_ledger"));
            }
            Ok(Some(receipt))
        }
        _ => Err(EconomyError::CorruptHistory("material_result_evidence")),
    }
}

/// Recovery authenticates the original owner but does not check current prices,
/// policy enabled, quote TTL or settlement cursor. It never executes a source.
pub fn query_material_conversion_authorized_with_timeouts<A>(
    client: &mut Client,
    realm: &str,
    quote_id: &str,
    lock_timeout_ms: u32,
    statement_timeout_ms: u32,
    authorize: A,
) -> EconomyResult<Option<MaterialConversionReceipt>>
where
    A: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
{
    hex_id(quote_id, "material_quote_id")?;
    transaction_with_timeouts(
        client,
        realm,
        Some((lock_timeout_ms, statement_timeout_ms)),
        |tx, clock| {
            let owner = authorize(tx, clock)?;
            validate_owner(&owner)?;
            let Some(quote) = load_quote(tx, realm, quote_id)? else {
                let key = material_conversion_operation_key(realm, &owner, quote_id)?;
                if tx.query_opt("SELECT operation_key FROM hunting_economy_operations WHERE realm_id=$1 AND operation_key=$2", &[&realm,&key]).map_err(db)?.is_some() {
                return Err(EconomyError::CorruptHistory("material_quote_missing"));
            }
                return Ok(None);
            };
            if quote.owner != owner {
                return Err(EconomyError::CharacterNotFound);
            }
            material_existing(tx, realm, &quote)
        },
    )
}

pub fn read_material_conversion_quote_authorized_with_timeouts<A>(
    client: &mut Client,
    realm: &str,
    quote_id: &str,
    lock_timeout_ms: u32,
    statement_timeout_ms: u32,
    authorize: A,
) -> EconomyResult<Option<MaterialConversionQuoteReceipt>>
where
    A: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
{
    hex_id(quote_id, "material_quote_id")?;
    transaction_with_timeouts(
        client,
        realm,
        Some((lock_timeout_ms, statement_timeout_ms)),
        |tx, clock| {
            let owner = authorize(tx, clock)?;
            validate_owner(&owner)?;
            let quote = load_quote(tx, realm, quote_id)?;
            if quote.as_ref().is_some_and(|quote| quote.owner != owner) {
                return Err(EconomyError::CharacterNotFound);
            }
            Ok(quote)
        },
    )
}

/// Source S MUST persist the complete canonical item/character mutation with its
/// original revision CAS in this transaction. It must not modify live ECS,
/// wallets, pools or any economy history, nor perform external I/O. A replay
/// returns the immutable outcome without calling S, even when now disabled.
pub fn confirm_material_conversion_authorized_with_source_with_timeouts<A, S>(
    client: &mut Client,
    realm: &str,
    quote_id: &str,
    lock_timeout_ms: u32,
    statement_timeout_ms: u32,
    authorize: A,
    source: S,
) -> EconomyResult<Idempotent<MaterialConversionReceipt>>
where
    A: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
    S: FnOnce(
        &mut Transaction<'_>,
        SourceContext,
        &MaterialConversionQuoteReceipt,
        &str,
    ) -> EconomyResult<MaterialConversionSourceResult>,
{
    confirm_material_impl(
        client,
        realm,
        quote_id,
        lock_timeout_ms,
        statement_timeout_ms,
        None,
        authorize,
        source,
    )
}

fn confirm_material_impl<A, S>(
    client: &mut Client,
    realm: &str,
    quote_id: &str,
    lock_timeout_ms: u32,
    statement_timeout_ms: u32,
    fault: Option<Fault>,
    authorize: A,
    source: S,
) -> EconomyResult<Idempotent<MaterialConversionReceipt>>
where
    A: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
    S: FnOnce(
        &mut Transaction<'_>,
        SourceContext,
        &MaterialConversionQuoteReceipt,
        &str,
    ) -> EconomyResult<MaterialConversionSourceResult>,
{
    hex_id(quote_id, "material_quote_id")?;
    let timeouts = Some((lock_timeout_ms, statement_timeout_ms));
    validate_timeouts(timeouts)?;
    let mut tx = client.transaction().map_err(db)?;
    configure_timeouts(&mut tx, timeouts)?;
    let clock = lock_realm_and_read_database_clock(&mut tx, realm)?;
    let owner = authorize(&mut tx, clock)?;
    validate_owner(&owner)?;
    let quoted = load_quote(&mut tx, realm, quote_id)?.ok_or(EconomyError::SourceRejected)?;
    if quoted.owner != owner {
        return Err(EconomyError::CharacterNotFound);
    }
    if let Some(receipt) = material_existing(&mut tx, realm, &quoted)? {
        commit(tx)?;
        return Ok(Idempotent {
            receipt,
            replayed: true,
        });
    }
    let (mut state, policy) = load_pool(&mut tx, realm)?;
    require_enabled(&policy)?;
    require_caught_up(&state, clock)?;
    if clock.database_time_ms >= quoted.expires_at_ms {
        return Err(EconomyError::MaterialQuoteExpired);
    }
    let (valuations, active) = active_valuation(&mut tx, realm)?;
    if active.valuation_version != quoted.valuation_version
        || active.valuation_digest != quoted.valuation_digest
        || active.revision != quoted.valuation_revision
        || policy.policy_version != quoted.policy_version
    {
        return Err(EconomyError::MaterialQuoteChanged);
    }
    let computed = quote_material_conversion_for_item(
        &policy,
        &valuations,
        quoted.item_index,
        quoted.quantity,
    )
    .map_err(|_| EconomyError::CorruptHistory("material_quote_arithmetic"))?;
    if computed != quoted.quote {
        return Err(EconomyError::CorruptHistory("material_quote_arithmetic"));
    }
    if state.pool.stock_atoms < computed.license_atoms_debit {
        return Err(EconomyError::InsufficientPermits);
    }
    let mut balance = wallet(&mut tx, realm, &owner, clock.database_time_ms)?;
    if balance.unbound_gold < computed.fee_unbound_gold {
        return Err(EconomyError::InsufficientUnboundGold);
    }
    let key = material_conversion_operation_key(realm, &owner, quote_id)?;
    let hash = material_request_digest(&quoted)?;
    let changed = source(&mut tx, clock, &quoted, &key)?;
    if changed.before_revision != quoted.input_save_revision
        || changed.before_revision.checked_add(1) != Some(changed.after_revision)
        || changed.input_unique_id != quoted.input_unique_id
        || changed.output_unique_id == 0
        || !changed.checkpoint.is_object()
        || (quoted.quantity == quoted.input_count)
            != (changed.output_unique_id == changed.input_unique_id)
    {
        return Err(EconomyError::SourceRejected);
    }
    bigint(changed.after_revision, "material_after_revision")?;
    balance.unbound_gold -= computed.fee_unbound_gold;
    state.pool.stock_atoms -= computed.license_atoms_debit;
    write_wallet(&mut tx, realm, &owner, &mut balance, clock.database_time_ms)?;
    write_pool(&mut tx, realm, &mut state, clock.database_time_ms)?;
    let burn_event_id = insert_burn(
        &mut tx,
        realm,
        &key,
        &TrustedBurn {
            source_kind: "material_conversion_fee".into(),
            source_key: key.clone(),
            layer: BurnLayer::Daily,
            gold_equivalent: computed.fee_unbound_gold,
            valuation_version: "gold-face-value-v1".into(),
        },
        &policy.policy_version,
        clock,
    )?;
    let receipt = MaterialConversionReceipt {
        realm_id: realm.into(),
        operation_key: key.clone(),
        request_digest: hash.clone(),
        owner: owner.clone(),
        quote: quoted.clone(),
        source: changed,
        policy_version: policy.policy_version.clone(),
        business_day: clock.business_day,
        committed_at_ms: clock.database_time_ms,
        wallet_after: balance,
        pool_after_atoms: state.pool.stock_atoms,
        burn_event_id,
    };
    operation(
        &mut tx,
        realm,
        &key,
        "material_conversion",
        &hash,
        &receipt,
        clock,
    )?;
    tx.execute("INSERT INTO hunting_economy_material_conversions(realm_id,operation_key,quote_id,account_id,character_index,character_incarnation,item_index,quantity,valuation_version,valuation_digest,gold_equivalent,fee_gold,consumed_atoms,policy_version,business_day,committed_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)",
        &[&realm,&key,&quote_id,&owner.account_id,&owner.character_index,&owner.incarnation,&quoted.item_index,&bigint(quoted.quantity,"material_quantity")?,
          &quoted.valuation_version,&quoted.valuation_digest,&bigint(computed.gold_equivalent,"material_equivalent")?,&bigint(computed.fee_unbound_gold,"material_fee")?,
          &bigint(computed.license_atoms_debit,"material_atoms")?,&policy.policy_version,&clock.business_day,&clock.database_time_ms]).map_err(db)?;
    tx.execute("INSERT INTO hunting_material_conversion_results(realm_id,quote_id,operation_key,receipt_json,receipt_digest,before_revision,after_revision,committed_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
        &[&realm,&quote_id,&key,&encode(&receipt)?,&digest(&receipt)?,&bigint(receipt.source.before_revision,"material_before_revision")?,
          &bigint(receipt.source.after_revision,"material_after_revision")?,&clock.database_time_ms]).map_err(db)?;
    if matches!(fault, Some(Fault::BeforeCommit)) {
        return Err(injected_failure());
    }
    commit(tx)?;
    if matches!(fault, Some(Fault::CommitAcknowledgementLost)) {
        return Err(EconomyError::OutcomeUnknown);
    }
    Ok(Idempotent {
        receipt,
        replayed: false,
    })
}

#[cfg(any(test, feature = "test-support"))]
pub fn confirm_material_conversion_authorized_with_source_with_test_fault<A, S>(
    client: &mut Client,
    realm: &str,
    quote_id: &str,
    lock_timeout_ms: u32,
    statement_timeout_ms: u32,
    fault: TestFault,
    authorize: A,
    source: S,
) -> EconomyResult<Idempotent<MaterialConversionReceipt>>
where
    A: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
    S: FnOnce(
        &mut Transaction<'_>,
        SourceContext,
        &MaterialConversionQuoteReceipt,
        &str,
    ) -> EconomyResult<MaterialConversionSourceResult>,
{
    let fault = match fault {
        TestFault::BeforeCommit => Fault::BeforeCommit,
        TestFault::CommitAcknowledgementLost => Fault::CommitAcknowledgementLost,
    };
    confirm_material_impl(
        client,
        realm,
        quote_id,
        lock_timeout_ms,
        statement_timeout_ms,
        Some(fault),
        authorize,
        source,
    )
}

/// The UID and quantity are intent only. The trusted callback locks the real
/// character and validates a plain, current-owned Bound source and full revision.
/// It is read-only: it must not alter the character, live ECS, assets or history.
/// request_id must include a trusted session/request namespace. A resetting
/// client counter alone would alias an old immutable quote after reconnect.
pub fn quote_material_conversion_authorized_with_source_with_timeouts<A, S>(
    client: &mut Client,
    realm: &str,
    request_id: &str,
    input_unique_id: u64,
    quantity: u64,
    lock_timeout_ms: u32,
    statement_timeout_ms: u32,
    authorize: A,
    source: S,
) -> EconomyResult<Idempotent<MaterialConversionQuoteReceipt>>
where
    A: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
    S: FnOnce(
        &mut Transaction<'_>,
        SourceContext,
        &WalletOwner,
    ) -> EconomyResult<MaterialConversionQuoteSource>,
{
    token(request_id, "material_request_id", 256)?;
    if input_unique_id == 0 || quantity == 0 {
        return Err(EconomyError::InvalidInput("material_source"));
    }
    bigint(quantity, "material_quantity")?;
    transaction_with_timeouts(
        client,
        realm,
        Some((lock_timeout_ms, statement_timeout_ms)),
        |tx, clock| {
            let owner = authorize(tx, clock)?;
            validate_owner(&owner)?;
            let id = raw_hash(&("hunting_material_quote_v1", realm, &owner, request_id))?;
            let request_digest = digest(&(
                "hunting_material_quote_request_v1",
                realm,
                &owner,
                request_id,
                input_unique_id,
                quantity,
            ))?;
            if let Some(quote) = load_quote(tx, realm, &id)? {
                if quote.request_digest != request_digest {
                    return Err(EconomyError::RequestConflict);
                }
                return Ok(Idempotent {
                    receipt: quote,
                    replayed: true,
                });
            }
            let (state, policy) = load_pool(tx, realm)?;
            require_enabled(&policy)?;
            require_caught_up(&state, clock)?;
            let (valuations, active) = active_valuation(tx, realm)?;
            let input = source(tx, clock, &owner)?;
            if input.item_index <= 0
                || input.input_unique_id != input_unique_id
                || input.quantity != quantity
                || input.input_count < quantity
            {
                return Err(EconomyError::SourceRejected);
            }
            bigint(input.input_count, "material_input_count")?;
            bigint(input.input_save_revision, "material_source_revision")?;
            token(&input.input_item_digest, "material_item_digest", 128)?;
            hex_id(&input.source_binding_digest, "material_binding_digest")?;
            let quote = quote_material_conversion_for_item(
                &policy,
                &valuations,
                input.item_index,
                quantity,
            )
            .map_err(|_| EconomyError::MaterialValuationUnavailable)?;
            let unit_gold = valuations
                .unit_gold(input.item_index)
                .map_err(|_| EconomyError::MaterialValuationUnavailable)?;
            let receipt = MaterialConversionQuoteReceipt {
                realm_id: realm.into(),
                quote_id: id.clone(),
                request_digest,
                owner,
                request_id: request_id.into(),
                item_index: input.item_index,
                input_unique_id,
                input_count: input.input_count,
                quantity,
                input_save_revision: input.input_save_revision,
                input_item_digest: input.input_item_digest,
                source_binding_digest: input.source_binding_digest,
                valuation_version: active.valuation_version,
                valuation_digest: active.valuation_digest,
                valuation_revision: active.revision,
                policy_version: policy.policy_version,
                unit_gold,
                quote,
                created_at_ms: clock.database_time_ms,
                expires_at_ms: clock
                    .database_time_ms
                    .checked_add(MATERIAL_QUOTE_TTL_MS)
                    .ok_or(EconomyError::Overflow("material_quote_expiry"))?,
            };
            tx.execute("INSERT INTO hunting_material_conversion_quotes(realm_id,quote_id,request_digest,account_id,character_index,character_incarnation,item_index,input_unique_id,input_count,quantity,input_save_revision,input_item_digest,source_binding_digest,valuation_version,valuation_digest,valuation_revision,policy_version,unit_gold,gold_equivalent,fee_gold,consumed_atoms,created_at_ms,expires_at_ms,quote_json,quote_digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8::text::numeric,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25)",
            &[&realm,&id,&receipt.request_digest,&receipt.owner.account_id,&receipt.owner.character_index,&receipt.owner.incarnation,&receipt.item_index,&input_unique_id.to_string(),
              &bigint(receipt.input_count,"material_input_count")?,&bigint(quantity,"material_quantity")?,&bigint(receipt.input_save_revision,"material_source_revision")?,
              &receipt.input_item_digest,&receipt.source_binding_digest,&receipt.valuation_version,&receipt.valuation_digest,&bigint(receipt.valuation_revision,"material_valuation_revision")?,
              &receipt.policy_version,&bigint(unit_gold,"material_unit_gold")?,&bigint(quote.gold_equivalent,"material_equivalent")?,&bigint(quote.fee_unbound_gold,"material_fee")?,
              &bigint(quote.license_atoms_debit,"material_atoms")?,&receipt.created_at_ms,&receipt.expires_at_ms,&encode(&receipt)?,&digest(&receipt)?]).map_err(db)?;
            Ok(Idempotent {
                receipt,
                replayed: false,
            })
        },
    )
}
