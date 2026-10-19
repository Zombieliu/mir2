//! Durable default daily-metric producer, called by the game server's worker.
//!
//! One transaction writes an incomplete new-wallet diagnostic revision (or
//! preserves an existing revision), records the schema/event receipt, then ACKs
//! the outbox. Restart and uncertain COMMIT retry by that durable receipt. This
//! producer never claims legacy assets, escrow, market, player or income coverage.

use crate::{lock_realm_and_read_database_clock, EconomyError, EconomyResult, SettlementReceipt};
use mir2_economy_policy::{business_day_key, Policy};
use postgres::{Client, Row, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const DAILY_METRIC_CONSUMER_VERSION: &str = "hunting-daily-new-wallet-diagnostics-v1";
pub const METRIC_EXPORT_MIGRATION_SQL: &str =
    include_str!("../../../infra/postgres/migrations/0019_hunting_economy_metric_exports.sql");
pub const OBSERVATION_MIGRATION_SQL: &str =
    include_str!("../../../infra/postgres/migrations/0017_hunting_economy_observability.sql");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricExportDisposition {
    InsertedPartial,
    ReusedExisting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyMetricExportReceipt {
    pub realm_id: String,
    pub event_id: String,
    pub consumer_version: String,
    pub business_day: i64,
    pub metric_revision: i64,
    pub source_payload_digest: String,
    pub disposition: MetricExportDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DailyMetricExportBatch {
    pub receipts: Vec<DailyMetricExportReceipt>,
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricExportTestFault {
    AfterMetricWrite,
    BeforeCommit,
    CommitAcknowledgementLost,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fault {
    AfterMetricWrite,
    BeforeCommit,
    CommitAcknowledgementLost,
}

fn db(error: postgres::Error) -> EconomyError {
    // Deliberately never expose SQL, DETAIL, identities or connection strings.
    EconomyError::Database {
        sqlstate: error.code().map(|code| code.code().to_owned()),
    }
}

fn commit_error(sqlstate: Option<&str>) -> EconomyError {
    // Same classification as the authoritative store: these responses do not
    // prove rollback. The caller must poll/retry the original consumer/event.
    match sqlstate {
        None | Some("40003") => EconomyError::OutcomeUnknown,
        Some(code) if code.starts_with("08") => EconomyError::OutcomeUnknown,
        Some(code) => EconomyError::Database {
            sqlstate: Some(code.into()),
        },
    }
}

fn fingerprint(value: &Value) -> EconomyResult<String> {
    fn canonical(value: Value) -> Value {
        match value {
            Value::Object(fields) => {
                let mut entries = fields.into_iter().collect::<Vec<_>>();
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                Value::Object(
                    entries
                        .into_iter()
                        .map(|(k, v)| (k, canonical(v)))
                        .collect(),
                )
            }
            Value::Array(values) => Value::Array(values.into_iter().map(canonical).collect()),
            other => other,
        }
    }
    let bytes = serde_json::to_vec(&canonical(value.clone()))
        .map_err(|_| EconomyError::CorruptHistory("metric_digest"))?;
    Ok(format!(
        "sha256:{}",
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    ))
}

fn unsigned_decimal(value: &str) -> bool {
    // PostgreSQL SUM(BIGINT)::text may exceed BIGINT/u64. Validate the exact
    // decimal representation without narrowing or converting to floating point.
    value == "0"
        || (!value.is_empty()
            && !value.starts_with('0')
            && value.bytes().all(|b| b.is_ascii_digit()))
}

fn amount(row: &Row, name: &str) -> EconomyResult<u64> {
    row.get::<_, String>(name)
        .parse()
        .map_err(|_| EconomyError::CorruptHistory("metric_aggregate_overflow"))
}

fn infer_remainder(
    output: u64,
    next: u64,
    basis: u64,
    numerator: u64,
    denominator: u64,
) -> EconomyResult<u64> {
    if denominator == 0 || next >= denominator {
        return Err(EconomyError::CorruptHistory("metric_plan_remainder"));
    }
    let recovered = u128::from(output)
        .checked_mul(u128::from(denominator))
        .and_then(|v| v.checked_add(u128::from(next)))
        .and_then(|v| v.checked_sub(u128::from(basis) * u128::from(numerator)))
        .ok_or(EconomyError::CorruptHistory("metric_plan_remainder"))?;
    if recovered >= u128::from(denominator) {
        return Err(EconomyError::CorruptHistory("metric_plan_remainder"));
    }
    Ok(recovered as u64)
}

fn validate_plan(receipt: &SettlementReceipt, policy: &Policy) -> EconomyResult<(u64, u64)> {
    policy
        .validate()
        .map_err(|_| EconomyError::CorruptHistory("metric_policy"))?;
    let plan = &receipt.plan;
    let cap = u128::from(policy.baseline_daily_mint_atoms)
        * u128::from(policy.daily_mint_cap_multiplier.numerator)
        / u128::from(policy.daily_mint_cap_multiplier.denominator);
    let fee = receipt
        .gross_conversion_gold
        .checked_sub(receipt.net_unbound_gold_inflow)
        .ok_or(EconomyError::CorruptHistory("metric_conversion_net"))?;
    if !unsigned_decimal(&receipt.bound_gold_stock)
        || !unsigned_decimal(&receipt.unbound_gold_stock)
        || receipt.consumed_atoms != receipt.gross_conversion_gold
        || fee > receipt.burns.daily
        || receipt.burns.total()? != plan.destruction_gold_equivalent
        || cap != u128::from(plan.daily_mint_cap_atoms)
        || plan.minted_atoms != plan.raw_mint_atoms.min(plan.daily_mint_cap_atoms)
        || plan.raw_mint_atoms.checked_sub(plan.minted_atoms) != Some(plan.capped_atoms)
        || plan.opening_stock_atoms.checked_add(plan.minted_atoms)
            != Some(plan.pre_decay_stock_atoms)
        || plan.pre_decay_stock_atoms.checked_sub(plan.decayed_atoms)
            != Some(plan.next_pool.stock_atoms)
    {
        return Err(EconomyError::CorruptHistory("metric_settlement_snapshot"));
    }
    Ok((
        infer_remainder(
            plan.raw_mint_atoms,
            plan.next_pool.mint_remainder,
            plan.destruction_gold_equivalent,
            policy.k.numerator,
            policy.k.denominator,
        )?,
        infer_remainder(
            plan.decayed_atoms,
            plan.next_pool.decay_remainder,
            plan.pre_decay_stock_atoms,
            policy.daily_decay.numerator,
            policy.daily_decay.denominator,
        )?,
    ))
}

fn validate_source(
    tx: &mut Transaction<'_>,
    realm: &str,
    event: &Row,
    now_ms: i64,
) -> EconomyResult<(SettlementReceipt, String)> {
    let payload: Value = event.get("payload_json");
    let receipt: SettlementReceipt = serde_json::from_value(payload.clone())
        .map_err(|_| EconomyError::CorruptHistory("metric_settlement_payload"))?;
    let digest = fingerprint(&payload)?;
    let event_id: String = event.get("event_id");
    let expected_event = fingerprint(&json!([
        "hunting_economy_outbox_v1",
        realm,
        "daily_settlement",
        receipt.business_day.to_string()
    ]))?;
    if receipt.realm_id != realm
        || event_id != expected_event
        || event.get::<_, i64>("created_at_ms") != receipt.committed_at_ms
        || receipt.committed_at_ms > now_ms
    {
        return Err(EconomyError::CorruptHistory("metric_event_identity"));
    }
    let source = tx.query_opt("SELECT policy_version,receipt_json,committed_at_ms FROM hunting_economy_settlements WHERE realm_id=$1 AND business_day=$2", &[&realm,&receipt.business_day])
        .map_err(db)?.ok_or(EconomyError::CorruptHistory("metric_source_missing"))?;
    if fingerprint(&source.get("receipt_json"))? != digest
        || source.get::<_, String>("policy_version") != receipt.policy_version
        || source.get::<_, i64>("committed_at_ms") != receipt.committed_at_ms
        || business_day_key(receipt.committed_at_ms.div_euclid(1000))
            .map_err(|_| EconomyError::CorruptHistory("metric_source_clock"))?
            <= receipt.business_day
    {
        return Err(EconomyError::CorruptHistory("metric_source_receipt"));
    }
    let state = tx.query_opt("SELECT genesis_day,settled_through_day,stock_atoms FROM hunting_economy_pools WHERE realm_id=$1", &[&realm])
        .map_err(db)?.ok_or(EconomyError::RealmMissing)?;
    let genesis: i64 = state.get("genesis_day");
    let through: i64 = state.get("settled_through_day");
    let current_day = business_day_key(now_ms.div_euclid(1000))
        .map_err(|_| EconomyError::CorruptHistory("metric_database_clock"))?;
    if receipt.business_day < genesis || receipt.business_day > through || through >= current_day {
        return Err(EconomyError::CorruptHistory("metric_settlement_cursor"));
    }
    let expected_days = through
        .checked_sub(genesis)
        .and_then(|n| n.checked_add(1))
        .ok_or(EconomyError::CorruptHistory("metric_cursor_range"))?;
    let actual_days:i64=tx.query_one("SELECT count(*)::bigint FROM hunting_economy_settlements WHERE realm_id=$1 AND business_day BETWEEN $2 AND $3", &[&realm,&genesis,&through]).map_err(db)?.get(0);
    if actual_days != expected_days {
        return Err(EconomyError::CorruptHistory("metric_cursor_gap"));
    }
    let latest=tx.query_one("SELECT receipt_json FROM hunting_economy_settlements WHERE realm_id=$1 AND business_day=$2", &[&realm,&through]).map_err(db)?;
    let latest: SettlementReceipt = serde_json::from_value(latest.get("receipt_json"))
        .map_err(|_| EconomyError::CorruptHistory("metric_latest_receipt"))?;
    let post=tx.query_one("SELECT COALESCE(sum(consumed_atoms),0)::text AS consumed FROM hunting_economy_conversions WHERE realm_id=$1 AND business_day>$2", &[&realm,&through]).map_err(db)?;
    let live_stock: u64 = u64::try_from(state.get::<_, i64>("stock_atoms"))
        .map_err(|_| EconomyError::CorruptHistory("metric_live_pool_stock"))?;
    if latest.realm_id != realm
        || latest.business_day != through
        || latest
            .plan
            .next_pool
            .stock_atoms
            .checked_sub(amount(&post, "consumed")?)
            != Some(live_stock)
    {
        return Err(EconomyError::CorruptHistory("metric_live_pool_stock"));
    }
    let policy_row = tx.query_opt("SELECT policy_json,policy_digest FROM hunting_economy_policies WHERE realm_id=$1 AND policy_version=$2", &[&realm,&receipt.policy_version])
        .map_err(db)?.ok_or(EconomyError::CorruptHistory("metric_policy_missing"))?;
    let policy_json: Value = policy_row.get("policy_json");
    if fingerprint(&policy_json)? != policy_row.get::<_, String>("policy_digest") {
        return Err(EconomyError::CorruptHistory("metric_policy_digest"));
    }
    let policy: Policy = serde_json::from_value(policy_json)
        .map_err(|_| EconomyError::CorruptHistory("metric_policy"))?;
    if policy.policy_version != receipt.policy_version {
        return Err(EconomyError::CorruptHistory("metric_policy_version"));
    }
    let opening_remainders = validate_plan(&receipt, &policy)?;
    if receipt.business_day == genesis {
        if receipt.plan.opening_stock_atoms != 0 || opening_remainders != (0, 0) {
            return Err(EconomyError::CorruptHistory("metric_genesis_stock"));
        }
    } else {
        let prior_day = receipt
            .business_day
            .checked_sub(1)
            .ok_or(EconomyError::Overflow("metric_prior_day"))?;
        let prior = tx.query_opt("SELECT receipt_json FROM hunting_economy_settlements WHERE realm_id=$1 AND business_day=$2", &[&realm,&prior_day])
            .map_err(db)?.ok_or(EconomyError::CorruptHistory("metric_prior_settlement"))?;
        let prior: SettlementReceipt = serde_json::from_value(prior.get("receipt_json"))
            .map_err(|_| EconomyError::CorruptHistory("metric_prior_receipt"))?;
        if prior.realm_id != realm
            || prior.business_day != prior_day
            || prior
                .plan
                .next_pool
                .stock_atoms
                .checked_sub(receipt.consumed_atoms)
                != Some(receipt.plan.opening_stock_atoms)
        {
            return Err(EconomyError::CorruptHistory("metric_opening_stock"));
        }
        // Cursor semantics disambiguate adjacent events that share one database
        // millisecond: an activation affecting day D must have both cursors D-1.
        let reset: bool = tx.query_one("SELECT EXISTS(SELECT 1 FROM hunting_economy_outbox WHERE realm_id=$1 AND event_kind='policy_activated' AND created_at_ms >= $2 AND created_at_ms <= $3 AND (payload_json #>> '{before,settled_through_day}')::bigint=$4 AND (payload_json #>> '{after,settled_through_day}')::bigint=$4 AND payload_json->'reset_fractional_remainders'='true'::jsonb AND (payload_json #>> '{after,pool,mint_remainder}')::numeric=0 AND (payload_json #>> '{after,pool,decay_remainder}')::numeric=0)", &[&realm,&prior.committed_at_ms,&receipt.committed_at_ms,&prior_day]).map_err(db)?.get(0);
        let previous = (
            prior.plan.next_pool.mint_remainder,
            prior.plan.next_pool.decay_remainder,
        );
        if opening_remainders != previous && !(reset && opening_remainders == (0, 0)) {
            return Err(EconomyError::CorruptHistory("metric_remainder_chain"));
        }
    }
    let flow = tx.query_one("SELECT COALESCE(sum(gross_gold),0)::text AS gross,COALESCE(sum(net_gold),0)::text AS net,COALESCE(sum(fee_gold),0)::text AS fee,COALESCE(sum(consumed_atoms),0)::text AS consumed FROM hunting_economy_conversions WHERE realm_id=$1 AND business_day=$2", &[&realm,&receipt.business_day]).map_err(db)?;
    if amount(&flow, "gross")? != receipt.gross_conversion_gold
        || amount(&flow, "net")? != receipt.net_unbound_gold_inflow
        || amount(&flow, "consumed")? != receipt.consumed_atoms
        || amount(&flow, "fee")? != receipt.gross_conversion_gold - receipt.net_unbound_gold_inflow
    {
        return Err(EconomyError::CorruptHistory("metric_conversion_aggregate"));
    }
    let mut burns = crate::BurnTotals::default();
    for row in tx.query("SELECT layer,sum(gold_equivalent)::text AS amount FROM hunting_economy_burn_events WHERE realm_id=$1 AND business_day=$2 GROUP BY layer", &[&realm,&receipt.business_day]).map_err(db)? {
        match row.get::<_,String>("layer").as_str() {
            "daily" => burns.daily=amount(&row,"amount")?,
            "light_pvp" => burns.light_pvp=amount(&row,"amount")?,
            "siege" => burns.siege=amount(&row,"amount")?,
            _ => return Err(EconomyError::CorruptHistory("metric_burn_layer")),
        }
    }
    if burns != receipt.burns {
        return Err(EconomyError::CorruptHistory("metric_burn_aggregate"));
    }
    let bad_fee: bool = tx.query_one("SELECT EXISTS(SELECT 1 FROM hunting_economy_conversions c LEFT JOIN hunting_economy_burn_events b ON b.realm_id=c.realm_id AND b.source_kind='conversion_fee' AND b.source_key=c.operation_key WHERE c.realm_id=$1 AND c.business_day=$2 AND (b.event_id IS NULL OR b.layer<>'daily' OR b.business_day<>c.business_day OR b.operation_key<>c.operation_key OR b.gold_equivalent<>c.fee_gold)) OR EXISTS(SELECT 1 FROM hunting_economy_burn_events b LEFT JOIN hunting_economy_conversions c ON c.realm_id=b.realm_id AND c.operation_key=b.source_key WHERE b.realm_id=$1 AND b.business_day=$2 AND b.source_kind='conversion_fee' AND c.operation_key IS NULL)", &[&realm,&receipt.business_day]).map_err(db)?.get(0);
    if bad_fee {
        return Err(EconomyError::CorruptHistory("metric_conversion_fee_burn"));
    }
    let wrong_rate:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM hunting_economy_conversions WHERE realm_id=$1 AND business_day=$2 AND fee_gold::numeric IS DISTINCT FROM ceil(gross_gold::numeric * $3::text::numeric / $4::text::numeric))", &[&realm,&receipt.business_day,&policy.conversion_fee.numerator.to_string(),&policy.conversion_fee.denominator.to_string()]).map_err(db)?.get(0);
    if wrong_rate {
        return Err(EconomyError::CorruptHistory("metric_conversion_fee_rate"));
    }
    Ok((receipt, digest))
}

fn injected() -> EconomyError {
    #[cfg(any(test, feature = "test-support"))]
    {
        EconomyError::InjectedFailure
    }
    #[cfg(not(any(test, feature = "test-support")))]
    {
        EconomyError::CorruptHistory("unreachable_metric_test_fault")
    }
}

fn export_impl(
    client: &mut Client,
    realm: &str,
    max_events: u16,
    timeouts: Option<(u32, u32)>,
    fault: Option<Fault>,
) -> EconomyResult<DailyMetricExportBatch> {
    if max_events == 0 || max_events > 128 {
        return Err(EconomyError::InvalidInput("metric_batch_limit"));
    }
    if let Some((lock, statement)) = timeouts {
        if lock == 0 || statement == 0 || lock > statement || statement > 30_000 {
            return Err(EconomyError::InvalidInput("transaction_timeouts"));
        }
    }
    let mut tx = client.transaction().map_err(db)?;
    if let Some((lock, statement)) = timeouts {
        tx.query_one(
            "SELECT set_config('lock_timeout',$1,true),set_config('statement_timeout',$2,true)",
            &[&format!("{lock}ms"), &format!("{statement}ms")],
        )
        .map_err(db)?;
    }
    let clock = lock_realm_and_read_database_clock(&mut tx, realm)?;
    if !tx
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM hunting_economy_pools WHERE realm_id=$1)",
            &[&realm],
        )
        .map_err(db)?
        .get::<_, bool>(0)
    {
        return Err(EconomyError::RealmMissing);
    }
    let events = tx.query("SELECT e.event_id,e.payload_json,e.created_at_ms FROM hunting_economy_outbox e LEFT JOIN hunting_economy_metric_exports x ON x.realm_id=e.realm_id AND x.event_id=e.event_id AND x.consumer_version=$2 WHERE e.realm_id=$1 AND e.event_kind='daily_settlement' AND x.event_id IS NULL ORDER BY e.created_at_ms,e.event_id LIMIT $3 FOR UPDATE OF e SKIP LOCKED", &[&realm,&DAILY_METRIC_CONSUMER_VERSION,&i64::from(max_events)]).map_err(db)?;
    let mut batch = DailyMetricExportBatch::default();
    for event in events {
        let (receipt, digest) = validate_source(&mut tx, realm, &event, clock.database_time_ms)?;
        let event_id: String = event.get("event_id");
        // A trusted richer revision may have arrived first. Its FK already binds
        // the exact immutable settlement; do not append a partial replacement.
        let existing = tx.query_opt("SELECT metric_revision FROM hunting_economy_daily_metric_versions WHERE realm_id=$1 AND business_day=$2 ORDER BY metric_revision DESC LIMIT 1", &[&realm,&receipt.business_day]).map_err(db)?;
        let (metric_revision, disposition) = if let Some(row) = existing {
            (
                row.get::<_, i64>("metric_revision"),
                MetricExportDisposition::ReusedExisting,
            )
        } else {
            let evidence = json!({"classification":"new_wallet_diagnostics_only","source_event_id":event_id,"source_payload_digest":digest,"consumer_version":DAILY_METRIC_CONSUMER_VERSION,"new_wallet_bound_gold_stock":receipt.bound_gold_stock,"new_wallet_unbound_gold_stock":receipt.unbound_gold_stock,"unavailable_sources":["legacy_assets","escrow","market","player_portraits","income"]});
            let inserted=tx.execute("INSERT INTO hunting_economy_daily_metric_versions(realm_id,business_day,metric_revision,metric_source,metric_evidence,created_at_ms,financial_period_closed,source_period_end,source_ledger_watermark,coverage_complete,coverage_scope) VALUES($1,$2::bigint,1,$3,$4,$5,true,((date '1970-01-01' + ($2::bigint)::integer + 1) + time '04:00') AT TIME ZONE 'Asia/Shanghai',$6,false,'new_wallets_only') ON CONFLICT(realm_id,business_day,metric_revision) DO NOTHING", &[&realm,&receipt.business_day,&DAILY_METRIC_CONSUMER_VERSION,&evidence,&clock.database_time_ms,&format!("settlement:{}:{}:{}",realm,receipt.business_day,event_id)]).map_err(db)?;
            let latest=tx.query_one("SELECT metric_revision FROM hunting_economy_daily_metric_versions WHERE realm_id=$1 AND business_day=$2 ORDER BY metric_revision DESC LIMIT 1", &[&realm,&receipt.business_day]).map_err(db)?.get::<_,i64>("metric_revision");
            (
                latest,
                if inserted == 1 {
                    MetricExportDisposition::InsertedPartial
                } else {
                    MetricExportDisposition::ReusedExisting
                },
            )
        };
        if fault == Some(Fault::AfterMetricWrite) {
            return Err(injected());
        }
        let disposition_key = match disposition {
            MetricExportDisposition::InsertedPartial => "inserted_partial",
            MetricExportDisposition::ReusedExisting => "reused_existing",
        };
        tx.execute("INSERT INTO hunting_economy_metric_exports(realm_id,event_id,consumer_version,business_day,metric_revision,source_payload_digest,disposition,exported_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8)", &[&realm,&event_id,&DAILY_METRIC_CONSUMER_VERSION,&receipt.business_day,&metric_revision,&digest,&disposition_key,&clock.database_time_ms]).map_err(db)?;
        // ACK comes after a durable target revision and export receipt have been
        // staged in this SAME transaction; COMMIT publishes all three together.
        if tx.execute("UPDATE hunting_economy_outbox SET delivered_at_ms=COALESCE(delivered_at_ms,$3) WHERE realm_id=$1 AND event_id=$2", &[&realm,&event_id,&clock.database_time_ms]).map_err(db)?!=1 {
            return Err(EconomyError::CorruptHistory("metric_outbox_ack_missing"));
        }
        batch.receipts.push(DailyMetricExportReceipt {
            realm_id: realm.into(),
            event_id,
            consumer_version: DAILY_METRIC_CONSUMER_VERSION.into(),
            business_day: receipt.business_day,
            metric_revision,
            source_payload_digest: digest,
            disposition,
        });
    }
    if fault == Some(Fault::BeforeCommit) {
        return Err(injected());
    }
    tx.commit()
        .map_err(|e| commit_error(e.code().map(|c| c.code())))?;
    if fault == Some(Fault::CommitAcknowledgementLost) {
        return Err(EconomyError::OutcomeUnknown);
    }
    Ok(batch)
}

/// Write a bounded batch of incomplete new-wallet diagnostics to the durable
/// PostgreSQL daily time series. Repeated calls/restarts are idempotent. Does not
/// settle days or mutate game assets. Requires migrations 0016,0017,0019.
pub fn export_daily_metrics(
    client: &mut Client,
    realm: &str,
    max_events: u16,
) -> EconomyResult<DailyMetricExportBatch> {
    export_impl(client, realm, max_events, None, None)
}

/// Production worker entry point. Timeouts are transaction-local, 1..=30,000ms,
/// lock<=statement, matching the authoritative store's bounded maintenance API.
pub fn export_daily_metrics_with_timeouts(
    client: &mut Client,
    realm: &str,
    max_events: u16,
    lock_timeout_ms: u32,
    statement_timeout_ms: u32,
) -> EconomyResult<DailyMetricExportBatch> {
    export_impl(
        client,
        realm,
        max_events,
        Some((lock_timeout_ms, statement_timeout_ms)),
        None,
    )
}

#[cfg(any(test, feature = "test-support"))]
pub fn export_daily_metrics_with_test_fault(
    client: &mut Client,
    realm: &str,
    max_events: u16,
    fault: MetricExportTestFault,
) -> EconomyResult<DailyMetricExportBatch> {
    let fault = match fault {
        MetricExportTestFault::AfterMetricWrite => Fault::AfterMetricWrite,
        MetricExportTestFault::BeforeCommit => Fault::BeforeCommit,
        MetricExportTestFault::CommitAcknowledgementLost => Fault::CommitAcknowledgementLost,
    };
    export_impl(client, realm, max_events, None, Some(fault))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_snapshot_decimals_allow_totals_above_bigint_without_float() {
        for good in ["0", "1", "184467440737095516140000000000"] {
            assert!(unsigned_decimal(good));
        }
        for bad in ["", "01", "+1", "-1", "1.0", "1e6", " 1", "١"] {
            assert!(!unsigned_decimal(bad));
        }
    }
    #[test]
    fn metric_commit_unknown_matches_authority_recovery_contract() {
        for state in [None, Some("08006"), Some("08007"), Some("40003")] {
            assert_eq!(commit_error(state), EconomyError::OutcomeUnknown);
        }
        assert_eq!(
            commit_error(Some("23503")),
            EconomyError::Database {
                sqlstate: Some("23503".into())
            }
        );
    }
    #[test]
    fn decimal_remainder_recovery_rejects_impossible_mint_or_decay() {
        assert_eq!(infer_remainder(10, 3, 100, 1, 10).unwrap(), 3);
        assert!(infer_remainder(11, 3, 100, 1, 10).is_err());
        assert!(infer_remainder(0, 0, 100, 1, 10).is_err());
    }
    #[test]
    fn plan_validation_rejects_stock_consumption_fee_and_decimal_corruption() {
        let policy:Policy=serde_json::from_value(json!({"schema_version":1,"policy_version":"unit-v1","enabled":false,
            "k":{"numerator":1,"denominator":10},"conversion_fee":{"numerator":1,"denominator":50},
            "daily_decay":{"numerator":1,"denominator":150},"baseline_daily_mint_atoms":720000,
            "daily_mint_cap_multiplier":{"numerator":3,"denominator":2},"business_timezone":"Asia/Shanghai","settlement_hour":4})).unwrap();
        let valid = SettlementReceipt {
            realm_id: "unit".into(),
            business_day: 1,
            policy_version: "unit-v1".into(),
            committed_at_ms: 1,
            burns: crate::BurnTotals::default(),
            plan: mir2_economy_policy::plan_settlement(&policy, Default::default(), 0).unwrap(),
            consumed_atoms: 0,
            gross_conversion_gold: 0,
            net_unbound_gold_inflow: 0,
            bound_gold_stock: "27670116110564327421".into(),
            unbound_gold_stock: "0".into(),
        };
        assert_eq!(validate_plan(&valid, &policy).unwrap(), (0, 0));
        let mut bad = valid.clone();
        bad.consumed_atoms = 1;
        assert!(validate_plan(&bad, &policy).is_err());
        let mut bad = valid.clone();
        bad.plan.next_pool.stock_atoms = 1;
        assert!(validate_plan(&bad, &policy).is_err());
        let mut bad = valid.clone();
        bad.bound_gold_stock = "-1".into();
        assert!(validate_plan(&bad, &policy).is_err());
        let mut bad = valid;
        bad.net_unbound_gold_inflow = 1;
        assert!(validate_plan(&bad, &policy).is_err());
    }
}
