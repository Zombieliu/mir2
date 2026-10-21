use crate::*;
use mir2_economy_policy::{business_day_key, plan_settlement, quote_conversion, Policy, PoolState};
use postgres::{Client, Row, Transaction};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const LOCK_SEED: i64 = 0x4855_4e54_4543_4f4e;

#[path = "debit.rs"]
mod debit;
pub use debit::*;
#[path = "zero.rs"]
mod zero;
pub use zero::*;
#[path = "material.rs"]
mod material;
pub use material::*;

fn db(error: postgres::Error) -> EconomyError {
    // Never print connection strings, SQL parameters, or PostgreSQL DETAIL fields.
    EconomyError::Database {
        sqlstate: error.code().map(|code| code.code().to_owned()),
    }
}
fn commit(tx: Transaction<'_>) -> EconomyResult<()> {
    tx.commit()
        .map_err(|error| classify_commit_sqlstate(error.code().map(|code| code.code())))
}

fn classify_commit_sqlstate(sqlstate: Option<&str>) -> EconomyError {
    match sqlstate {
        // An ErrorResponse does not make these outcomes certain. Recover by
        // the original operation key, just as after a dropped COMMIT reply.
        None | Some("40003") => EconomyError::OutcomeUnknown,
        Some(code) if code.starts_with("08") => EconomyError::OutcomeUnknown,
        Some(code) => EconomyError::Database {
            sqlstate: Some(code.to_owned()),
        },
    }
}
fn token(value: &str, field: &'static str, max: usize) -> EconomyResult<()> {
    if value.is_empty()
        || value.trim() != value
        || value.len() > max
        || value.chars().any(char::is_control)
    {
        return Err(EconomyError::InvalidInput(field));
    }
    Ok(())
}
fn validate_owner(owner: &WalletOwner) -> EconomyResult<()> {
    token(&owner.account_id, "account_id", 256)?;
    token(&owner.incarnation, "character_incarnation", 256)?;
    if owner.character_index < 0 {
        return Err(EconomyError::InvalidInput("character_index"));
    }
    Ok(())
}
fn bigint(n: u64, field: &'static str) -> EconomyResult<i64> {
    i64::try_from(n).map_err(|_| EconomyError::Overflow(field))
}
fn unsigned(n: i64, field: &'static str) -> EconomyResult<u64> {
    u64::try_from(n).map_err(|_| EconomyError::CorruptHistory(field))
}
fn encode<T: Serialize>(value: &T) -> EconomyResult<Value> {
    serde_json::to_value(value).map_err(|_| EconomyError::CorruptHistory("json_encode"))
}
fn digest<T: Serialize>(value: &T) -> EconomyResult<String> {
    // JSONB reorders object keys. Sort recursively even when a workspace enables
    // serde_json/preserve_order through feature unification.
    fn canonical(value: Value) -> Value {
        match value {
            Value::Object(fields) => {
                let mut entries = fields.into_iter().collect::<Vec<_>>();
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                Value::Object(
                    entries
                        .into_iter()
                        .map(|(key, value)| (key, canonical(value)))
                        .collect(),
                )
            }
            Value::Array(values) => Value::Array(values.into_iter().map(canonical).collect()),
            other => other,
        }
    }
    let value = canonical(encode(value)?);
    let bytes =
        serde_json::to_vec(&value).map_err(|_| EconomyError::CorruptHistory("digest_encode"))?;
    let hex = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(format!("sha256:{hex}"))
}
// Preserve the original positive-debit key domain. A source quoted at zero and
// the same source quoted at a positive price MUST compete for this one slot.
fn bound_source_operation_key(
    realm: &str,
    owner: &WalletOwner,
    source_kind: &str,
    source_key: &str,
) -> EconomyResult<String> {
    token(realm, "realm_id", 128)?;
    validate_owner(owner)?;
    token(source_kind, "source_kind", 128)?;
    token(source_key, "source_key", 256)?;
    digest(&("bound_debit_burn_v1", realm, owner, source_kind, source_key))
}
fn decode<T: DeserializeOwned>(value: Value) -> EconomyResult<T> {
    serde_json::from_value(value).map_err(|_| EconomyError::CorruptHistory("json_decode"))
}
fn checked_add(a: u64, b: u64, field: &'static str) -> EconomyResult<u64> {
    let value = a.checked_add(b).ok_or(EconomyError::Overflow(field))?;
    bigint(value, field)?;
    Ok(value)
}

/// This lock is also available to trusted inventory destruction adapters. Every
/// participant must take it before source row locks, and before reading its clock.
pub fn lock_realm_and_read_database_clock(
    tx: &mut Transaction<'_>,
    realm: &str,
) -> EconomyResult<SourceContext> {
    token(realm, "realm_id", 128)?;
    tx.query_one(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,$2))",
        &[&realm, &LOCK_SEED],
    )
    .map_err(db)?;
    let ms: i64 = tx
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint AS ms",
            &[],
        )
        .map_err(db)?
        .get("ms");
    if ms < 0 {
        return Err(EconomyError::InvalidInput("database_clock"));
    }
    let day = business_day_key(ms.div_euclid(1000))
        .map_err(|_| EconomyError::Overflow("database_day"))?;
    Ok(SourceContext {
        business_day: day,
        database_time_ms: ms,
    })
}

fn transaction<T>(
    client: &mut Client,
    realm: &str,
    action: impl FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<T>,
) -> EconomyResult<T> {
    transaction_with_timeouts(client, realm, None, action)
}
fn validate_timeouts(timeouts: Option<(u32, u32)>) -> EconomyResult<()> {
    if let Some((lock_ms, statement_ms)) = timeouts {
        if lock_ms == 0
            || statement_ms == 0
            || lock_ms > 30_000
            || statement_ms > 30_000
            || lock_ms > statement_ms
        {
            return Err(EconomyError::InvalidInput("transaction_timeouts"));
        }
    }
    Ok(())
}
fn configure_timeouts(tx: &mut Transaction<'_>, timeouts: Option<(u32, u32)>) -> EconomyResult<()> {
    if let Some((lock_ms, statement_ms)) = timeouts {
        tx.query_one(
            "SELECT set_config('lock_timeout',$1,true),set_config('statement_timeout',$2,true)",
            &[&format!("{lock_ms}ms"), &format!("{statement_ms}ms")],
        )
        .map_err(db)?;
    }
    Ok(())
}
fn transaction_with_timeouts<T>(
    client: &mut Client,
    realm: &str,
    timeouts: Option<(u32, u32)>,
    action: impl FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<T>,
) -> EconomyResult<T> {
    validate_timeouts(timeouts)?;
    let mut tx = client.transaction().map_err(db)?;
    configure_timeouts(&mut tx, timeouts)?;
    let clock = lock_realm_and_read_database_clock(&mut tx, realm)?;
    let result = action(&mut tx, clock)?;
    commit(tx)?;
    Ok(result)
}

fn validate_policy(policy: &Policy) -> EconomyResult<()> {
    policy.validate().map_err(|_| EconomyError::InvalidPolicy)?;
    token(&policy.policy_version, "policy_version", 128)?;
    bigint(
        policy
            .daily_mint_cap_atoms()
            .map_err(|_| EconomyError::InvalidPolicy)?,
        "daily_mint_cap",
    )?;
    bigint(policy.k.denominator, "k_denominator")?;
    bigint(policy.daily_decay.denominator, "decay_denominator")?;
    Ok(())
}
fn insert_policy(
    tx: &mut Transaction<'_>,
    realm: &str,
    policy: &Policy,
    now: i64,
) -> EconomyResult<()> {
    validate_policy(policy)?;
    let data = encode(policy)?;
    let hash = digest(&data)?;
    if let Some(row) = tx.query_opt("SELECT policy_digest,policy_json FROM hunting_economy_policies WHERE realm_id=$1 AND policy_version=$2",
        &[&realm, &policy.policy_version]).map_err(db)? {
        if row.get::<_,String>("policy_digest") != hash || row.get::<_,Value>("policy_json") != data {
            return Err(EconomyError::PolicyConflict);
        }
        return Ok(());
    }
    tx.execute("INSERT INTO hunting_economy_policies(realm_id,policy_version,policy_digest,policy_json,created_at_ms) VALUES($1,$2,$3,$4,$5)",
        &[&realm, &policy.policy_version, &hash, &data, &now]).map_err(db)?;
    Ok(())
}

/// Create a realm at the current DB-derived business day. There is no initial
/// permit injection, historical backdating, or legacy-wallet import argument.
pub fn initialize_realm(
    client: &mut Client,
    realm: &str,
    policy: &Policy,
) -> EconomyResult<RealmState> {
    transaction(client, realm, |tx, clock| {
        insert_policy(tx, realm, policy, clock.database_time_ms)?;
        if tx
            .query_opt(
                "SELECT realm_id FROM hunting_economy_pools WHERE realm_id=$1",
                &[&realm],
            )
            .map_err(db)?
            .is_none()
        {
            let cursor = clock
                .business_day
                .checked_sub(1)
                .ok_or(EconomyError::Overflow("genesis_day"))?;
            tx.execute("INSERT INTO hunting_economy_pools(realm_id,policy_version,genesis_day,settled_through_day,updated_at_ms) VALUES($1,$2,$3,$4,$5)",
                &[&realm, &policy.policy_version, &clock.business_day, &cursor, &clock.database_time_ms]).map_err(db)?;
            outbox(
                tx,
                realm,
                "realm_initialized",
                &format!("genesis:{}", clock.business_day),
                &json!({"genesis_day": clock.business_day, "policy_version": policy.policy_version, "stock_atoms": 0}),
                clock.database_time_ms,
            )?;
        }
        let (state, _) = load_pool(tx, realm)?;
        Ok(state)
    })
}

/// Register an immutable configuration revision; it has no player-asset effect.
pub fn register_policy(client: &mut Client, realm: &str, policy: &Policy) -> EconomyResult<()> {
    transaction(client, realm, |tx, clock| {
        insert_policy(tx, realm, policy, clock.database_time_ms)
    })
}

/// Trusted deployment/config operation. Catch up old days before changing the
/// policy. Fractional remainders may only be reset by explicit operator intent;
/// whole permits are preserved. No Grafana query should invoke this function.
/// Changed economic parameters can take effect only before the first asset
/// operation of the open business day. A version that only changes the player
/// admission flag may take effect immediately without repricing past destruction.
pub fn activate_policy(
    client: &mut Client,
    realm: &str,
    version: &str,
    reset_fractional_remainders: bool,
) -> EconomyResult<RealmState> {
    token(version, "policy_version", 128)?;
    transaction(client, realm, |tx, clock| {
        let (mut state, mut current_policy) = load_pool(tx, realm)?;
        require_caught_up(&state, clock)?;
        if state.policy_version == version {
            return Ok(state);
        }
        let has_daily_activity: bool = tx
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM hunting_economy_operations WHERE realm_id=$1 AND business_day=$2)",
                &[&realm, &clock.business_day],
            )
            .map_err(db)?
            .get(0);
        let next_policy = load_policy(tx, realm, version)?;
        current_policy.policy_version = next_policy.policy_version.clone();
        current_policy.enabled = next_policy.enabled;
        if has_daily_activity && current_policy != next_policy {
            return Err(EconomyError::InvalidInput(
                "policy_change_after_daily_activity",
            ));
        }
        if !reset_fractional_remainders
            && (state.pool.mint_remainder != 0 || state.pool.decay_remainder != 0)
        {
            return Err(EconomyError::InvalidInput(
                "explicit_remainder_reset_required",
            ));
        }
        let old = state.clone();
        state.policy_version = version.into();
        state.pool.mint_remainder = 0;
        state.pool.decay_remainder = 0;
        write_pool(tx, realm, &mut state, clock.database_time_ms)?;
        outbox(
            tx,
            realm,
            "policy_activated",
            &format!("policy:{}", state.revision),
            &json!({"before": old, "after": state, "reset_fractional_remainders": reset_fractional_remainders}),
            clock.database_time_ms,
        )?;
        Ok(state)
    })
}

fn load_policy(tx: &mut Transaction<'_>, realm: &str, version: &str) -> EconomyResult<Policy> {
    let row = tx.query_opt("SELECT policy_json,policy_digest FROM hunting_economy_policies WHERE realm_id=$1 AND policy_version=$2", &[&realm, &version])
        .map_err(db)?.ok_or(EconomyError::CorruptHistory("missing_policy"))?;
    let value: Value = row.get("policy_json");
    if digest(&value)? != row.get::<_, String>("policy_digest") {
        return Err(EconomyError::CorruptHistory("policy_digest"));
    }
    let policy: Policy = decode(value)?;
    validate_policy(&policy)?;
    if policy.policy_version != version {
        return Err(EconomyError::CorruptHistory("policy_version"));
    }
    Ok(policy)
}
fn state_from_row(row: &Row) -> EconomyResult<RealmState> {
    Ok(RealmState {
        policy_version: row.get("policy_version"),
        pool: PoolState {
            stock_atoms: unsigned(row.get("stock_atoms"), "stock_atoms")?,
            mint_remainder: unsigned(row.get("mint_remainder"), "mint_remainder")?,
            decay_remainder: unsigned(row.get("decay_remainder"), "decay_remainder")?,
        },
        genesis_day: row.get("genesis_day"),
        settled_through_day: row.get("settled_through_day"),
        revision: unsigned(row.get("revision"), "pool_revision")?,
    })
}
fn load_pool(tx: &mut Transaction<'_>, realm: &str) -> EconomyResult<(RealmState, Policy)> {
    let row = tx.query_opt("SELECT policy_version,stock_atoms,mint_remainder,decay_remainder,genesis_day,settled_through_day,revision FROM hunting_economy_pools WHERE realm_id=$1 FOR UPDATE", &[&realm])
        .map_err(db)?.ok_or(EconomyError::RealmMissing)?;
    let state = state_from_row(&row)?;
    let policy = load_policy(tx, realm, &state.policy_version)?;
    if state.pool.mint_remainder >= policy.k.denominator
        || state.pool.decay_remainder >= policy.daily_decay.denominator
    {
        return Err(EconomyError::CorruptHistory("pool_remainder"));
    }
    Ok((state, policy))
}
fn write_pool(
    tx: &mut Transaction<'_>,
    realm: &str,
    state: &mut RealmState,
    now: i64,
) -> EconomyResult<()> {
    state.revision = checked_add(state.revision, 1, "pool_revision")?;
    tx.execute("UPDATE hunting_economy_pools SET policy_version=$2,stock_atoms=$3,mint_remainder=$4,decay_remainder=$5,settled_through_day=$6,revision=$7,updated_at_ms=$8 WHERE realm_id=$1",
        &[&realm, &state.policy_version, &bigint(state.pool.stock_atoms,"stock_atoms")?, &bigint(state.pool.mint_remainder,"mint_remainder")?,
            &bigint(state.pool.decay_remainder,"decay_remainder")?, &state.settled_through_day, &bigint(state.revision,"pool_revision")?, &now]).map_err(db)?;
    Ok(())
}
fn require_enabled(policy: &Policy) -> EconomyResult<()> {
    if !policy.enabled {
        Err(EconomyError::Disabled)
    } else {
        Ok(())
    }
}
fn require_caught_up(state: &RealmState, clock: SourceContext) -> EconomyResult<()> {
    let next = state
        .settled_through_day
        .checked_add(1)
        .ok_or(EconomyError::Overflow("settlement_cursor"))?;
    if next > clock.business_day || state.genesis_day > clock.business_day {
        return Err(EconomyError::CorruptHistory("future_settlement_cursor"));
    }
    if next < clock.business_day {
        return Err(EconomyError::SettlementRequired {
            next_day: next,
            current_day: clock.business_day,
        });
    }
    Ok(())
}

fn wallet(
    tx: &mut Transaction<'_>,
    realm: &str,
    owner: &WalletOwner,
    now: i64,
) -> EconomyResult<WalletBalance> {
    validate_owner(owner)?;
    tx.execute("INSERT INTO hunting_economy_wallets(realm_id,account_id,character_index,character_incarnation,updated_at_ms) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING",
        &[&realm, &owner.account_id, &owner.character_index, &owner.incarnation, &now]).map_err(db)?;
    let row = tx.query_one("SELECT bound_gold,unbound_gold,revision FROM hunting_economy_wallets WHERE realm_id=$1 AND account_id=$2 AND character_index=$3 AND character_incarnation=$4 FOR UPDATE",
        &[&realm,&owner.account_id,&owner.character_index,&owner.incarnation]).map_err(db)?;
    wallet_from_row(&row)
}
fn wallet_from_row(row: &Row) -> EconomyResult<WalletBalance> {
    Ok(WalletBalance {
        bound_gold: unsigned(row.get("bound_gold"), "bound_gold")?,
        unbound_gold: unsigned(row.get("unbound_gold"), "unbound_gold")?,
        revision: unsigned(row.get("revision"), "wallet_revision")?,
    })
}
fn write_wallet(
    tx: &mut Transaction<'_>,
    realm: &str,
    owner: &WalletOwner,
    balance: &mut WalletBalance,
    now: i64,
) -> EconomyResult<()> {
    balance.revision = checked_add(balance.revision, 1, "wallet_revision")?;
    tx.execute("UPDATE hunting_economy_wallets SET bound_gold=$5,unbound_gold=$6,revision=$7,updated_at_ms=$8 WHERE realm_id=$1 AND account_id=$2 AND character_index=$3 AND character_incarnation=$4",
        &[&realm,&owner.account_id,&owner.character_index,&owner.incarnation,&bigint(balance.bound_gold,"bound_gold")?,
            &bigint(balance.unbound_gold,"unbound_gold")?,&bigint(balance.revision,"wallet_revision")?,&now]).map_err(db)?;
    Ok(())
}

/// Provision only zero balances; ordinary logout has no write path to this table.
pub fn initialize_wallet(
    client: &mut Client,
    realm: &str,
    owner: &WalletOwner,
) -> EconomyResult<WalletBalance> {
    transaction(client, realm, |tx, clock| {
        load_pool(tx, realm)?;
        wallet(tx, realm, owner, clock.database_time_ms)
    })
}
pub fn read_wallet(
    client: &mut Client,
    realm: &str,
    owner: &WalletOwner,
) -> EconomyResult<Option<WalletBalance>> {
    token(realm, "realm_id", 128)?;
    validate_owner(owner)?;
    client.query_opt("SELECT bound_gold,unbound_gold,revision FROM hunting_economy_wallets WHERE realm_id=$1 AND account_id=$2 AND character_index=$3 AND character_incarnation=$4",
        &[&realm,&owner.account_id,&owner.character_index,&owner.incarnation]).map_err(db)?.as_ref().map(wallet_from_row).transpose()
}
pub fn read_realm(client: &mut Client, realm: &str) -> EconomyResult<RealmState> {
    transaction(client, realm, |tx, _| {
        load_pool(tx, realm).map(|(state, _)| state)
    })
}

/// The server verifies its signed identity first, then this callback locks and
/// rechecks the revocable authorization and character incarnation in the SAME
/// transaction as the read. The callback must acquire account/session/character
/// source locks in the documented game-server order and retain them until return.
pub fn read_wallet_authorized<F>(
    client: &mut Client,
    realm: &str,
    authorize: F,
) -> EconomyResult<AuthorizedWalletSnapshot>
where
    F: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
{
    read_wallet_authorized_impl(client, realm, None, authorize)
}
pub fn read_wallet_authorized_with_timeouts<F>(
    client: &mut Client,
    realm: &str,
    lock_timeout_ms: u32,
    statement_timeout_ms: u32,
    authorize: F,
) -> EconomyResult<AuthorizedWalletSnapshot>
where
    F: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
{
    read_wallet_authorized_impl(
        client,
        realm,
        Some((lock_timeout_ms, statement_timeout_ms)),
        authorize,
    )
}
fn read_wallet_authorized_impl<F>(
    client: &mut Client,
    realm: &str,
    timeouts: Option<(u32, u32)>,
    authorize: F,
) -> EconomyResult<AuthorizedWalletSnapshot>
where
    F: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
{
    transaction_with_timeouts(client, realm, timeouts, |tx, clock| {
        let owner = authorize(tx, clock)?;
        validate_owner(&owner)?;
        let (state, policy) = load_pool(tx, realm)?;
        let balance = tx.query_opt("SELECT bound_gold,unbound_gold,revision FROM hunting_economy_wallets WHERE realm_id=$1 AND account_id=$2 AND character_index=$3 AND character_incarnation=$4",
            &[&realm,&owner.account_id,&owner.character_index,&owner.incarnation]).map_err(db)?
            .as_ref().map(wallet_from_row).transpose()?;
        Ok(AuthorizedWalletSnapshot {
            owner,
            wallet: balance,
            realm: state,
            enabled: policy.enabled,
        })
    })
}

fn existing<T: DeserializeOwned>(
    tx: &mut Transaction<'_>,
    realm: &str,
    key: &str,
    kind: &str,
    request_digest: &str,
) -> EconomyResult<Option<T>> {
    let Some(row) = tx.query_opt("SELECT operation_kind,request_digest,receipt_json,receipt_digest FROM hunting_economy_operations WHERE realm_id=$1 AND operation_key=$2", &[&realm,&key]).map_err(db)? else { return Ok(None); };
    if row.get::<_, String>("operation_kind") != kind
        || row.get::<_, String>("request_digest") != request_digest
    {
        return Err(EconomyError::RequestConflict);
    }
    let data: Value = row.get("receipt_json");
    if digest(&data)? != row.get::<_, String>("receipt_digest") {
        return Err(EconomyError::CorruptHistory("receipt_digest"));
    }
    decode(data).map(Some)
}
fn operation<T: Serialize>(
    tx: &mut Transaction<'_>,
    realm: &str,
    key: &str,
    kind: &str,
    request_digest: &str,
    receipt: &T,
    clock: SourceContext,
) -> EconomyResult<()> {
    let data = encode(receipt)?;
    tx.execute("INSERT INTO hunting_economy_operations(realm_id,operation_key,operation_kind,request_digest,receipt_json,receipt_digest,business_day,committed_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
        &[&realm,&key,&kind,&request_digest,&data,&digest(&data)?,&clock.business_day,&clock.database_time_ms]).map_err(db)?;
    outbox(tx, realm, kind, key, &data, clock.database_time_ms)
}
fn outbox(
    tx: &mut Transaction<'_>,
    realm: &str,
    kind: &str,
    key: &str,
    payload: &Value,
    now: i64,
) -> EconomyResult<()> {
    let id = digest(&("hunting_economy_outbox_v1", realm, kind, key))?;
    tx.execute("INSERT INTO hunting_economy_outbox(realm_id,event_id,event_kind,payload_json,created_at_ms) VALUES($1,$2,$3,$4,$5)", &[&realm,&id,&kind,&payload,&now]).map_err(db)?;
    Ok(())
}

/// Trusted game-source credit. The callback consumes/marks the real production
/// or quest source using this transaction. Retries skip the callback entirely.
/// `source_digest` binds the exact original source payload, not a network claim.
pub fn credit_bound_with_source<F>(
    client: &mut Client,
    realm: &str,
    owner: &WalletOwner,
    source_key: &str,
    source_digest: &str,
    amount: u64,
    source: F,
) -> EconomyResult<Idempotent<BoundCreditReceipt>>
where
    F: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<()>,
{
    credit_bound_authorized_with_source_impl(
        client,
        realm,
        source_key,
        source_digest,
        amount,
        None,
        |_, _| Ok(owner.clone()),
        source,
    )
}

/// Stable server-side identity for a trusted credit. Source participants may
/// use this key for a deferred foreign key in the SAME transaction.
pub fn bound_credit_operation_identity(
    realm: &str,
    owner: &WalletOwner,
    source_key: &str,
    source_digest: &str,
    amount: u64,
) -> EconomyResult<(String, String)> {
    token(realm, "realm_id", 128)?;
    validate_owner(owner)?;
    token(source_key, "source_key", 256)?;
    token(source_digest, "source_digest", 256)?;
    if amount == 0 {
        return Err(EconomyError::InvalidInput("credit_amount"));
    }
    bigint(amount, "credit_amount")?;
    let key = digest(&("bound_credit_v1", realm, owner, source_key))?;
    let hash = digest(&(
        "bound_credit_v1",
        realm,
        owner,
        source_key,
        source_digest,
        amount,
    ))?;
    Ok((key, hash))
}

/// Authorization runs under the realm lock before receipt lookup, including
/// retries. It must lock the real source owner (and any preceding Guild locks)
/// before this library can create or lock its wallet. The source callback writes
/// the complete gameplay CAS using this same transaction; it is skipped on replay.
pub fn credit_bound_authorized_with_source<A, S>(
    client: &mut Client,
    realm: &str,
    source_key: &str,
    source_digest: &str,
    amount: u64,
    authorize: A,
    source: S,
) -> EconomyResult<Idempotent<BoundCreditReceipt>>
where
    A: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
    S: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<()>,
{
    credit_bound_authorized_with_source_impl(
        client,
        realm,
        source_key,
        source_digest,
        amount,
        None,
        authorize,
        source,
    )
}

pub fn credit_bound_authorized_with_source_with_timeouts<A, S>(
    client: &mut Client,
    realm: &str,
    source_key: &str,
    source_digest: &str,
    amount: u64,
    lock_timeout_ms: u32,
    statement_timeout_ms: u32,
    authorize: A,
    source: S,
) -> EconomyResult<Idempotent<BoundCreditReceipt>>
where
    A: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
    S: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<()>,
{
    credit_bound_authorized_with_source_impl(
        client,
        realm,
        source_key,
        source_digest,
        amount,
        Some((lock_timeout_ms, statement_timeout_ms)),
        authorize,
        source,
    )
}

fn credit_bound_authorized_with_source_impl<A, S>(
    client: &mut Client,
    realm: &str,
    source_key: &str,
    source_digest: &str,
    amount: u64,
    timeouts: Option<(u32, u32)>,
    authorize: A,
    source: S,
) -> EconomyResult<Idempotent<BoundCreditReceipt>>
where
    A: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
    S: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<()>,
{
    transaction_with_timeouts(client, realm, timeouts, |tx, clock| {
        let owner = authorize(tx, clock)?;
        let (key, hash) =
            bound_credit_operation_identity(realm, &owner, source_key, source_digest, amount)?;
        if let Some(receipt) = existing(tx, realm, &key, "bound_credit", &hash)? {
            return Ok(Idempotent {
                receipt,
                replayed: true,
            });
        }
        let (state, policy) = load_pool(tx, realm)?;
        require_enabled(&policy)?;
        require_caught_up(&state, clock)?;
        source(tx, clock)?;
        let mut balance = wallet(tx, realm, &owner, clock.database_time_ms)?;
        balance.bound_gold = checked_add(balance.bound_gold, amount, "bound_gold")?;
        write_wallet(tx, realm, &owner, &mut balance, clock.database_time_ms)?;
        let receipt = BoundCreditReceipt {
            realm_id: realm.into(),
            operation_key: key.clone(),
            request_digest: hash.clone(),
            owner: owner.clone(),
            source_key: source_key.into(),
            bound_gold_credit: amount,
            business_day: clock.business_day,
            committed_at_ms: clock.database_time_ms,
            wallet_after: balance,
        };
        operation(tx, realm, &key, "bound_credit", &hash, &receipt, clock)?;
        Ok(Idempotent {
            receipt,
            replayed: false,
        })
    })
}

/// Read-only original-operation recovery. No policy check, wallet creation,
/// source callback or new credit occurs. The trusted caller supplies locking
/// authorization; normal clients must never select a historical owner.
pub fn lookup_bound_credit_authorized_with_timeouts<A>(
    client: &mut Client,
    realm: &str,
    source_key: &str,
    source_digest: &str,
    amount: u64,
    lock_timeout_ms: u32,
    statement_timeout_ms: u32,
    authorize: A,
) -> EconomyResult<Option<BoundCreditReceipt>>
where
    A: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
{
    transaction_with_timeouts(
        client,
        realm,
        Some((lock_timeout_ms, statement_timeout_ms)),
        |tx, clock| {
            let owner = authorize(tx, clock)?;
            let (key, hash) =
                bound_credit_operation_identity(realm, &owner, source_key, source_digest, amount)?;
            existing(tx, realm, &key, "bound_credit", &hash)
        },
    )
}

fn insert_burn(
    tx: &mut Transaction<'_>,
    realm: &str,
    key: &str,
    burn: &TrustedBurn,
    policy: &str,
    clock: SourceContext,
) -> EconomyResult<String> {
    let event = digest(&("burn_event_v1", realm, &burn.source_kind, &burn.source_key))?;
    tx.execute("INSERT INTO hunting_economy_burn_events(realm_id,event_id,operation_key,layer,source_kind,source_key,gold_equivalent,valuation_version,policy_version,business_day,committed_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
        &[&realm,&event,&key,&burn.layer.as_db(),&burn.source_kind,&burn.source_key,&bigint(burn.gold_equivalent,"burn_amount")?,
            &burn.valuation_version,&policy,&clock.business_day,&clock.database_time_ms]).map_err(db)?;
    Ok(event)
}

/// Record permanent destruction ONLY together with its actual trusted source
/// mutation. Theft/transfers and repairable damage are not permanent destruction.
pub fn burn_with_source<F>(
    client: &mut Client,
    realm: &str,
    burn: &TrustedBurn,
    source_digest: &str,
    source: F,
) -> EconomyResult<Idempotent<BurnReceipt>>
where
    F: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<()>,
{
    token(&burn.source_kind, "source_kind", 128)?;
    token(&burn.source_key, "source_key", 256)?;
    token(&burn.valuation_version, "valuation_version", 128)?;
    token(source_digest, "source_digest", 256)?;
    if matches!(burn.source_kind.as_str(), "conversion_fee" | "material_conversion_fee") || burn.gold_equivalent == 0 {
        return Err(EconomyError::InvalidInput("burn_source"));
    }
    bigint(burn.gold_equivalent, "burn_amount")?;
    let key = digest(&("burn_v1", realm, &burn.source_kind, &burn.source_key))?;
    let hash = digest(&(
        "burn_v1",
        realm,
        &burn.source_kind,
        &burn.source_key,
        burn.layer,
        burn.gold_equivalent,
        &burn.valuation_version,
        source_digest,
    ))?;
    transaction(client, realm, |tx, clock| {
        if let Some(receipt) = existing(tx, realm, &key, "burn", &hash)? {
            return Ok(Idempotent {
                receipt,
                replayed: true,
            });
        }
        let (state, policy) = load_pool(tx, realm)?;
        require_enabled(&policy)?;
        require_caught_up(&state, clock)?;
        source(tx, clock)?;
        let event_id = insert_burn(tx, realm, &key, burn, &policy.policy_version, clock)?;
        let receipt = BurnReceipt {
            realm_id: realm.into(),
            operation_key: key.clone(),
            request_digest: hash.clone(),
            event_id,
            gold_equivalent: burn.gold_equivalent,
            layer: burn.layer,
            business_day: clock.business_day,
            committed_at_ms: clock.database_time_ms,
        };
        operation(tx, realm, &key, "burn", &hash, &receipt, clock)?;
        Ok(Idempotent {
            receipt,
            replayed: false,
        })
    })
}

/// Convert only NEW bound gold. The 2% fee is retained from gross output and
/// permanently destroyed; the owner receives net output. This does not emit an
/// old GainedGold packet or touch the legacy unified-gold wallet.
pub fn convert(
    client: &mut Client,
    realm: &str,
    owner: &WalletOwner,
    request_id: &str,
    gross_gold: u64,
) -> EconomyResult<Idempotent<ConversionReceipt>> {
    convert_impl(client, realm, owner, request_id, gross_gold, None)
}

#[derive(Clone, Copy)]
#[cfg_attr(not(any(test, feature = "test-support")), allow(dead_code))]
enum Fault {
    BeforeCommit,
    CommitAcknowledgementLost,
}

fn convert_impl(
    client: &mut Client,
    realm: &str,
    owner: &WalletOwner,
    request_id: &str,
    gross_gold: u64,
    fault: Option<Fault>,
) -> EconomyResult<Idempotent<ConversionReceipt>> {
    validate_owner(owner)?;
    token(request_id, "request_id", 256)?;
    bigint(gross_gold, "conversion_amount")?;
    let mut tx = client.transaction().map_err(db)?;
    let clock = lock_realm_and_read_database_clock(&mut tx, realm)?;
    let result = convert_locked(&mut tx, realm, owner, request_id, gross_gold, clock)?;
    if !result.replayed && matches!(fault, Some(Fault::BeforeCommit)) {
        return Err(injected_failure());
    }
    commit(tx)?;
    if !result.replayed && matches!(fault, Some(Fault::CommitAcknowledgementLost)) {
        return Err(EconomyError::OutcomeUnknown);
    }
    Ok(result)
}

/// Authentication is checked even when an old receipt exists. The server's
/// callback derives the owner from verified identity and locks revocation, ban,
/// and character rows until this conversion commits. No client owner is trusted.
pub fn convert_authorized<F>(
    client: &mut Client,
    realm: &str,
    request_id: &str,
    gross_gold: u64,
    authorize: F,
) -> EconomyResult<Idempotent<ConversionReceipt>>
where
    F: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
{
    convert_authorized_impl(client, realm, request_id, gross_gold, None, authorize)
}
pub fn convert_authorized_with_timeouts<F>(
    client: &mut Client,
    realm: &str,
    request_id: &str,
    gross_gold: u64,
    lock_timeout_ms: u32,
    statement_timeout_ms: u32,
    authorize: F,
) -> EconomyResult<Idempotent<ConversionReceipt>>
where
    F: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
{
    convert_authorized_impl(
        client,
        realm,
        request_id,
        gross_gold,
        Some((lock_timeout_ms, statement_timeout_ms)),
        authorize,
    )
}
fn convert_authorized_impl<F>(
    client: &mut Client,
    realm: &str,
    request_id: &str,
    gross_gold: u64,
    timeouts: Option<(u32, u32)>,
    authorize: F,
) -> EconomyResult<Idempotent<ConversionReceipt>>
where
    F: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
{
    token(request_id, "request_id", 256)?;
    bigint(gross_gold, "conversion_amount")?;
    transaction_with_timeouts(client, realm, timeouts, |tx, clock| {
        let owner = authorize(tx, clock)?;
        validate_owner(&owner)?;
        convert_locked(tx, realm, &owner, request_id, gross_gold, clock)
    })
}

fn convert_locked(
    tx: &mut Transaction<'_>,
    realm: &str,
    owner: &WalletOwner,
    request_id: &str,
    gross_gold: u64,
    clock: SourceContext,
) -> EconomyResult<Idempotent<ConversionReceipt>> {
    let key = digest(&("conversion_v1", realm, owner, request_id))?;
    let hash = digest(&("conversion_v1", realm, owner, request_id, gross_gold))?;
    // Recovery remains available across policy disablement, boundary, and outage.
    if let Some(receipt) = existing(tx, realm, &key, "conversion", &hash)? {
        return Ok(Idempotent {
            receipt,
            replayed: true,
        });
    }
    let (mut state, policy) = load_pool(tx, realm)?;
    require_enabled(&policy)?;
    require_caught_up(&state, clock)?;
    let quote = quote_conversion(&policy, gross_gold)
        .map_err(|_| EconomyError::InvalidInput("conversion_amount"))?;
    let mut balance = wallet(tx, realm, owner, clock.database_time_ms)?;
    if state.pool.stock_atoms < quote.license_atoms_debit {
        return Err(EconomyError::InsufficientPermits);
    }
    if balance.bound_gold < quote.bound_gold_debit {
        return Err(EconomyError::InsufficientBoundGold);
    }
    balance.bound_gold -= quote.bound_gold_debit;
    balance.unbound_gold = checked_add(
        balance.unbound_gold,
        quote.net_unbound_gold_credit,
        "unbound_gold",
    )?;
    state.pool.stock_atoms -= quote.license_atoms_debit;
    write_wallet(tx, realm, owner, &mut balance, clock.database_time_ms)?;
    write_pool(tx, realm, &mut state, clock.database_time_ms)?;
    let receipt = ConversionReceipt {
        realm_id: realm.into(),
        operation_key: key.clone(),
        request_digest: hash.clone(),
        owner: owner.clone(),
        request_id: request_id.into(),
        policy_version: policy.policy_version.clone(),
        business_day: clock.business_day,
        committed_at_ms: clock.database_time_ms,
        quote,
        wallet_after: balance,
        pool_after_atoms: state.pool.stock_atoms,
    };
    operation(tx, realm, &key, "conversion", &hash, &receipt, clock)?;
    let fee = TrustedBurn {
        source_kind: "conversion_fee".into(),
        source_key: key.clone(),
        layer: BurnLayer::Daily,
        gold_equivalent: quote.fee_unbound_gold,
        valuation_version: "gold-face-value-v1".into(),
    };
    insert_burn(tx, realm, &key, &fee, &policy.policy_version, clock)?;
    tx.execute("INSERT INTO hunting_economy_conversions(realm_id,operation_key,account_id,character_index,character_incarnation,gross_gold,fee_gold,net_gold,consumed_atoms,business_day,committed_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
        &[&realm,&key,&owner.account_id,&owner.character_index,&owner.incarnation,&bigint(quote.bound_gold_debit,"gross_gold")?,
            &bigint(quote.fee_unbound_gold,"fee_gold")?,&bigint(quote.net_unbound_gold_credit,"net_gold")?,&bigint(quote.license_atoms_debit,"consumed_atoms")?,
            &clock.business_day,&clock.database_time_ms]).map_err(db)?;
    Ok(Idempotent {
        receipt,
        replayed: false,
    })
}

fn injected_failure() -> EconomyError {
    #[cfg(any(test, feature = "test-support"))]
    {
        EconomyError::InjectedFailure
    }
    #[cfg(not(any(test, feature = "test-support")))]
    {
        EconomyError::SourceRejected
    }
}
#[cfg(any(test, feature = "test-support"))]
pub fn convert_with_test_fault(
    client: &mut Client,
    realm: &str,
    owner: &WalletOwner,
    request_id: &str,
    gross_gold: u64,
    fault: TestFault,
) -> EconomyResult<Idempotent<ConversionReceipt>> {
    let fault = match fault {
        TestFault::BeforeCommit => Fault::BeforeCommit,
        TestFault::CommitAcknowledgementLost => Fault::CommitAcknowledgementLost,
    };
    convert_impl(client, realm, owner, request_id, gross_gold, Some(fault))
}

fn sum_u64(row: &Row, column: &str) -> EconomyResult<u64> {
    row.get::<_, String>(column)
        .parse::<u64>()
        .map_err(|_| EconomyError::Overflow("daily_aggregate"))
}
fn burn_totals(tx: &mut Transaction<'_>, realm: &str, day: i64) -> EconomyResult<BurnTotals> {
    let mut totals = BurnTotals::default();
    for row in tx.query("SELECT layer,sum(gold_equivalent)::text AS total FROM hunting_economy_burn_events WHERE realm_id=$1 AND business_day=$2 GROUP BY layer", &[&realm,&day]).map_err(db)? {
        let amount = sum_u64(&row,"total")?;
        match row.get::<_,String>("layer").as_str() {
            "daily" => totals.daily=amount, "light_pvp" => totals.light_pvp=amount, "siege" => totals.siege=amount,
            _ => return Err(EconomyError::CorruptHistory("burn_layer")),
        }
    }
    Ok(totals)
}
fn settle_impl(
    client: &mut Client,
    realm: &str,
    max_days: u16,
    fault: Option<Fault>,
    timeouts: Option<(u32, u32)>,
) -> EconomyResult<Vec<SettlementReceipt>> {
    if max_days == 0 || max_days > 366 {
        return Err(EconomyError::InvalidInput("max_days"));
    }
    validate_timeouts(timeouts)?;
    let mut tx = client.transaction().map_err(db)?;
    // Local settings precede the realm lock and reset even on timeout rollback.
    configure_timeouts(&mut tx, timeouts)?;
    let clock = lock_realm_and_read_database_clock(&mut tx, realm)?;
    let (mut state, policy) = load_pool(&mut tx, realm)?;
    if state.genesis_day > clock.business_day || state.settled_through_day >= clock.business_day {
        return Err(EconomyError::CorruptHistory("future_settlement_cursor"));
    }
    let mut receipts = Vec::new();
    for _ in 0..max_days {
        let day = state
            .settled_through_day
            .checked_add(1)
            .ok_or(EconomyError::Overflow("settlement_day"))?;
        if day >= clock.business_day {
            break;
        }
        if day < state.genesis_day {
            return Err(EconomyError::CorruptHistory("pre_genesis_cursor"));
        }
        // A row ahead of the locked cursor is corruption, not permission to mint
        // twice or silently skip a lost pool mutation.
        if tx.query_opt("SELECT business_day FROM hunting_economy_settlements WHERE realm_id=$1 AND business_day=$2", &[&realm,&day]).map_err(db)?.is_some() {
            return Err(EconomyError::CorruptHistory("settlement_ahead_of_cursor"));
        }
        let burns = burn_totals(&mut tx, realm, day)?;
        let plan = plan_settlement(&policy, state.pool, burns.total()?)
            .map_err(|_| EconomyError::Overflow("settlement_plan"))?;
        bigint(plan.next_pool.stock_atoms, "stock_atoms")?;
        let flows=tx.query_one("SELECT COALESCE(sum(consumed_atoms),0)::text AS consumed,COALESCE(sum(gross_gold),0)::text AS gross,COALESCE(sum(net_gold),0)::text AS net FROM hunting_economy_conversions WHERE realm_id=$1 AND business_day=$2", &[&realm,&day]).map_err(db)?;
        let materials=tx.query_one("SELECT COALESCE(sum(consumed_atoms),0)::text AS consumed,COALESCE(sum(gold_equivalent),0)::text AS equivalent,COALESCE(sum(fee_gold),0)::text AS fee FROM hunting_economy_material_conversions WHERE realm_id=$1 AND business_day=$2", &[&realm,&day]).map_err(db)?;
        let stocks=tx.query_one("SELECT COALESCE(sum(bound_gold),0)::text AS bound,COALESCE(sum(unbound_gold),0)::text AS unbound FROM hunting_economy_wallets WHERE realm_id=$1", &[&realm]).map_err(db)?;
        let receipt = SettlementReceipt {
            realm_id: realm.into(),
            business_day: day,
            policy_version: policy.policy_version.clone(),
            committed_at_ms: clock.database_time_ms,
            burns,
            plan: plan.clone(),
            consumed_atoms: sum_u64(&flows, "consumed")?.checked_add(sum_u64(&materials, "consumed")?)
                .ok_or(EconomyError::Overflow("daily_consumed_atoms"))?,
            gross_conversion_gold: sum_u64(&flows, "gross")?,
            net_unbound_gold_inflow: sum_u64(&flows, "net")?,
            material_conversion_gold_equivalent: sum_u64(&materials, "equivalent")?,
            material_conversion_fee_unbound_gold: sum_u64(&materials, "fee")?,
            bound_gold_stock: stocks.get("bound"),
            unbound_gold_stock: stocks.get("unbound"),
        };
        tx.execute("INSERT INTO hunting_economy_settlements(realm_id,business_day,policy_version,receipt_json,committed_at_ms) VALUES($1,$2,$3,$4,$5)",
            &[&realm,&day,&policy.policy_version,&encode(&receipt)?,&clock.database_time_ms]).map_err(db)?;
        state.pool = plan.next_pool;
        state.settled_through_day = day;
        write_pool(&mut tx, realm, &mut state, clock.database_time_ms)?;
        outbox(
            &mut tx,
            realm,
            "daily_settlement",
            &day.to_string(),
            &encode(&receipt)?,
            clock.database_time_ms,
        )?;
        receipts.push(receipt);
    }
    if matches!(fault, Some(Fault::BeforeCommit)) {
        return Err(injected_failure());
    }
    commit(tx)?;
    if matches!(fault, Some(Fault::CommitAcknowledgementLost)) {
        return Err(EconomyError::OutcomeUnknown);
    }
    Ok(receipts)
}

/// Settle up to `max_days` CLOSED days, strictly in order from the durable
/// genesis/cursor. Multi-instance calls are safe. An already caught-up call is
/// an empty success. Repeat bounded batches after outages until caught up.
/// Maintenance advances days even when player mutations are policy-disabled;
/// this allows a disabled realm to be caught up and safely re-enabled later.
pub fn settle_due(
    client: &mut Client,
    realm: &str,
    max_days: u16,
) -> EconomyResult<Vec<SettlementReceipt>> {
    settle_impl(client, realm, max_days, None, None)
}

/// Bounded maintenance for a process-lifetime worker or HTTP catch-up. Timeouts
/// are 1..=30,000 ms, lock <= statement, and scoped to this transaction only.
pub fn settle_due_with_timeouts(
    client: &mut Client,
    realm: &str,
    max_days: u16,
    lock_timeout_ms: u32,
    statement_timeout_ms: u32,
) -> EconomyResult<Vec<SettlementReceipt>> {
    settle_impl(
        client,
        realm,
        max_days,
        None,
        Some((lock_timeout_ms, statement_timeout_ms)),
    )
}
#[cfg(any(test, feature = "test-support"))]
pub fn settle_due_with_test_fault(
    client: &mut Client,
    realm: &str,
    max_days: u16,
    fault: TestFault,
) -> EconomyResult<Vec<SettlementReceipt>> {
    let fault = match fault {
        TestFault::BeforeCommit => Fault::BeforeCommit,
        TestFault::CommitAcknowledgementLost => Fault::CommitAcknowledgementLost,
    };
    settle_impl(client, realm, max_days, Some(fault), None)
}

pub fn read_settlement(
    client: &mut Client,
    realm: &str,
    day: i64,
) -> EconomyResult<Option<SettlementReceipt>> {
    token(realm, "realm_id", 128)?;
    client.query_opt("SELECT receipt_json FROM hunting_economy_settlements WHERE realm_id=$1 AND business_day=$2", &[&realm,&day])
        .map_err(db)?.map(|row|decode(row.get("receipt_json"))).transpose()
}

/// Confirm an outbox export only after the downstream consumer durably accepts
/// the event ID. Re-delivery is expected and consumers deduplicate by event ID.
pub fn acknowledge_outbox(client: &mut Client, realm: &str, event_id: &str) -> EconomyResult<bool> {
    token(event_id, "event_id", 128)?;
    transaction(client, realm, |tx, clock| {
        Ok(tx.execute("UPDATE hunting_economy_outbox SET delivered_at_ms=COALESCE(delivered_at_ms,$3) WHERE realm_id=$1 AND event_id=$2", &[&realm,&event_id,&clock.database_time_ms]).map_err(db)? == 1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commit_unknown_sqlstates_preserve_original_request_recovery() {
        for code in [None, Some("08007"), Some("08006"), Some("40003")] {
            assert_eq!(classify_commit_sqlstate(code), EconomyError::OutcomeUnknown);
        }
        for code in ["40001", "23503", "25P02"] {
            assert_eq!(
                classify_commit_sqlstate(Some(code)),
                EconomyError::Database {
                    sqlstate: Some(code.into())
                }
            );
        }
    }
    #[test]
    fn malformed_identity_and_bigint_overflow_are_rejected_without_a_database() {
        let good = WalletOwner {
            account_id: "player".into(),
            character_index: 0,
            incarnation: "creation-1".into(),
        };
        assert!(validate_owner(&good).is_ok());
        let mut bad = good.clone();
        bad.incarnation = " ".into();
        assert!(validate_owner(&bad).is_err());
        bad = good;
        bad.character_index = -1;
        assert!(validate_owner(&bad).is_err());
        assert!(bigint(i64::MAX as u64, "amount").is_ok());
        assert!(matches!(
            bigint(i64::MAX as u64 + 1, "amount"),
            Err(EconomyError::Overflow(_))
        ));
    }
    #[test]
    fn idempotency_namespace_binds_character_incarnation_and_not_the_amount() {
        let owner = WalletOwner {
            account_id: "same".into(),
            character_index: 2,
            incarnation: "first".into(),
        };
        let mut recreated = owner.clone();
        recreated.incarnation = "second".into();
        let key = digest(&("conversion_v1", "realm", &owner, "request")).unwrap();
        assert_ne!(
            key,
            digest(&("conversion_v1", "realm", &recreated, "request")).unwrap()
        );
        assert_ne!(
            digest(&("conversion_v1", "realm", &owner, "request", 1u64)).unwrap(),
            digest(&("conversion_v1", "realm", &owner, "request", 2u64)).unwrap()
        );
    }
    #[test]
    fn jsonb_key_reordering_does_not_change_digest() {
        let first: Value =
            serde_json::from_str(r#"{"z":{"b":1,"a":2},"a":[{"y":3,"x":4}]}"#).unwrap();
        let second: Value =
            serde_json::from_str(r#"{"a":[{"x":4,"y":3}],"z":{"a":2,"b":1}}"#).unwrap();
        assert_eq!(digest(&first).unwrap(), digest(&second).unwrap());
    }
    #[test]
    fn transaction_timeout_ranges_are_validated_without_touching_a_connection() {
        assert!(validate_timeouts(None).is_ok());
        assert!(validate_timeouts(Some((1, 1))).is_ok());
        assert!(validate_timeouts(Some((2000, 10000))).is_ok());
        assert!(validate_timeouts(Some((30000, 30000))).is_ok());
        for pair in [(0, 100), (100, 0), (101, 100), (30001, 30001), (1, 30001)] {
            assert!(matches!(
                validate_timeouts(Some(pair)),
                Err(EconomyError::InvalidInput(_))
            ));
        }
    }
}
