//! Import authoritative account-funded monthly entitlements into promotion accounting.
use std::time::Duration;

use postgres::{Client, NoTls, Transaction};
use serde_json::{json, Value};

use super::store::{audit, db, hash, positive, refresh_account_qualifications, signed};
use super::PromotionStore;
use crate::billing_provenance::monthly_revenue_sources;
use crate::{with_account_store_postgres_client, AccountRecord};

fn hold(
    tx: &mut Transaction<'_>,
    env: &str,
    campaign: &str,
    account: &str,
    subscription: &str,
    allocation: Option<&str>,
    active: bool,
    reason: &str,
    source_hash: &str,
    target: u64,
    occurred: u64,
    now: u64,
) -> Result<(), String> {
    db(tx.execute("INSERT INTO promotion_revenue_source_holds(environment,campaign_id,account_id,subscription_id,allocation_id,active,reason_code,source_payload_sha256,target_reversal_micro_usd,reversal_occurred_at_ms,updated_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) ON CONFLICT(environment,campaign_id,account_id,subscription_id) DO UPDATE SET allocation_id=COALESCE(EXCLUDED.allocation_id,promotion_revenue_source_holds.allocation_id),active=EXCLUDED.active,reason_code=EXCLUDED.reason_code,source_payload_sha256=EXCLUDED.source_payload_sha256,target_reversal_micro_usd=GREATEST(promotion_revenue_source_holds.target_reversal_micro_usd,EXCLUDED.target_reversal_micro_usd),reversal_occurred_at_ms=CASE WHEN EXCLUDED.target_reversal_micro_usd>promotion_revenue_source_holds.target_reversal_micro_usd THEN EXCLUDED.reversal_occurred_at_ms ELSE COALESCE(promotion_revenue_source_holds.reversal_occurred_at_ms,EXCLUDED.reversal_occurred_at_ms) END,updated_at_ms=EXCLUDED.updated_at_ms",
        &[&env,&campaign,&account,&subscription,&allocation,&active,&reason,&source_hash,&signed(target)?,&signed(occurred)?,&signed(now)?]))?;
    Ok(())
}

fn observe(
    tx: &mut Transaction<'_>,
    env: &str,
    campaign: &str,
    account: &str,
    subscription: &str,
    source: &Value,
    status: &str,
    reason: &str,
    now: u64,
) -> Result<String, String> {
    let source_hash =
        hash(&json!({"accountId":account,"subscriptionId":subscription,"source":source}))?;
    let inserted=db(tx.execute("INSERT INTO promotion_monthly_source_observations(environment,account_id,subscription_id,source_payload_sha256,funding_status,reason_code,source_json,observed_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT DO NOTHING",&[&env,&account,&subscription,&source_hash,&status,&reason,&source,&signed(now)?]))?;
    if inserted > 0 {
        audit(
            tx,
            env,
            "live_payment_adapter",
            "source.revenue_observation",
            &source_hash,
            "account_cas_credit_provenance",
            json!({"campaignId":campaign,"accountId":account,"subscriptionId":subscription,"status":status,"reasonCode":reason}),
            now,
        )?;
    }
    Ok(source_hash)
}

fn allocation_id(env: &str, account: &str, uid: u64) -> Result<String, String> {
    Ok(format!(
        "live_{}",
        hash(&json!(["monthly-credit-fifo-v1", env, account, uid]))?
    ))
}

/// A separate connection owns the worker session lock. It never holds a pool
/// connection while calling ingest_reversal, so pool size 1 also works.
fn worker_lock(database_url: &str, env: &str) -> Result<Option<Client>, String> {
    let mut config: postgres::Config = database_url
        .parse()
        .map_err(|_| "promotionConfigurationInvalid")?;
    config.connect_timeout(Duration::from_secs(3));
    let mut client = config
        .connect(NoTls)
        .map_err(|_| "promotionStorageUnavailable")?;
    let acquired:bool=db(client.query_one("SELECT pg_try_advisory_lock(hashtextextended('promotion.live-revenue.v1:'||current_database()||':'||current_schema()||':'||$1,0))",&[&env]))?.get(0);
    Ok(if acquired { Some(client) } else { None })
}

impl PromotionStore {
    /// Real input is read from the authoritative persisted game account. No
    /// caller supplies amounts, wallet ownership, payment status or tax/FX.
    pub fn reconcile_live_monthly_revenue(&self, now_ms: u64) -> Result<Value, String> {
        let pairs = with_account_store_postgres_client(&self.database_url, |client| {
            db(client.query("SELECT a.campaign_id,a.account_id FROM promotion_attributions a JOIN promotion_campaigns c USING(environment,campaign_id) WHERE a.environment=$1 ORDER BY a.campaign_id,a.account_id",&[&self.environment])).map(|rows|rows.iter().map(|r|(r.get::<_,String>(0),r.get::<_,String>(1))).collect::<Vec<_>>())
        })?;
        let Some(_worker_guard) = worker_lock(&self.database_url, &self.environment)? else {
            return Ok(json!({"status":"busy","accountsScanned":0,"environment":self.environment}));
        };
        let mut imported = 0u64;
        let mut pending = 0u64;
        let mut excluded = 0u64;
        let mut observations = 0u64;
        for (campaign_id, account_id) in &pairs {
            let counts = with_account_store_postgres_client(&self.database_url, |client| {
                let mut tx = db(client.transaction())?;
                let campaign=db(tx.query_one("SELECT starts_at_ms,ends_at_ms FROM promotion_campaigns WHERE environment=$1 AND campaign_id=$2 FOR UPDATE",&[&self.environment,&campaign_id]))?;
                let attribution=db(tx.query_one("SELECT confirmed_at_ms FROM promotion_attributions WHERE environment=$1 AND campaign_id=$2 AND account_id=$3 FOR UPDATE",&[&self.environment,&campaign_id,&account_id]))?;
                let confirmed = positive(attribution.get(0));
                let raw: Value = db(tx.query_one(
                    "SELECT raw_json FROM accounts WHERE account_id=$1 FOR SHARE",
                    &[&account_id],
                ))?
                .get(0);
                let sources = serde_json::from_value::<AccountRecord>(raw)
                    .map_err(|_| "promotionAccountUnavailable".to_owned())
                    .and_then(|account| {
                        monthly_revenue_sources(
                            &account,
                            account_id,
                            self.environment == "production",
                        )
                    });
                let sources = match sources {
                    Ok(sources) => sources,
                    Err(error) => {
                        let value =
                            json!({"reasonCode":"payment_source_invalid","sourceError":error});
                        let source_hash = observe(
                            &mut tx,
                            &self.environment,
                            campaign_id,
                            account_id,
                            "account-source-invalid",
                            &value,
                            "pending",
                            "payment_source_invalid",
                            now_ms,
                        )?;
                        let existing=db(tx.query("SELECT subscription_id,allocation_id FROM promotion_revenue_allocations WHERE environment=$1 AND campaign_id=$2 AND account_id=$3",&[&self.environment,&campaign_id,&account_id]))?;
                        for row in existing {
                            hold(
                                &mut tx,
                                &self.environment,
                                campaign_id,
                                account_id,
                                &row.get::<_, String>(0),
                                Some(&row.get::<_, String>(1)),
                                true,
                                "payment_source_invalid",
                                &source_hash,
                                0,
                                now_ms,
                                now_ms,
                            )?;
                        }
                        refresh_account_qualifications(
                            &mut tx,
                            &self.environment,
                            account_id,
                            now_ms,
                        )?;
                        db(tx.commit())?;
                        return Ok((0, 1, 0, 1));
                    }
                };
                let mut counts = (0, 0, 0, 0);
                for source in sources {
                    let subscription = format!("monthly-item:{}", source.unique_id);
                    let existing=db(tx.query_opt("SELECT allocation_id,campaign_id,account_id,eligible_micro_usd,paid_at_ms,entitlement_at_ms,service_start_ms,service_end_ms,reason_code FROM promotion_revenue_allocations WHERE environment=$1 AND subscription_id=$2 FOR UPDATE",&[&self.environment,&subscription]))?;
                    // One immutable monthly UID is accounted once across all seasons.
                    if existing.as_ref().is_some_and(|r| {
                        r.get::<_, String>(1) != *campaign_id
                            || r.get::<_, String>(2) != *account_id
                    }) {
                        continue;
                    }
                    if existing.is_none()
                        && (source.service_end_ms <= positive(campaign.get(0))
                            || source.service_start_ms >= positive(campaign.get(1)))
                    {
                        continue;
                    }
                    let mut early_payment = false;
                    for part in &source.parts {
                        if part.paid_at_ms > 0 && part.paid_at_ms < confirmed &&
                            part.reason_code != "payment_environment_mismatch" &&
                            !super::attribution_migration::permits_preconfirmation_payment(
                                &mut tx,&self.environment,campaign_id,account_id,
                                &part.source_order_id,part.paid_at_ms,confirmed)? {
                            early_payment = true;
                            break;
                        }
                    }
                    let future = source.entitlement_at_ms > now_ms
                        || source.paid_at_ms > now_ms
                        || source.source_observed_at_ms > now_ms;
                    let (status, reason) = if early_payment {
                        ("pending", "payment_precedes_attribution")
                    } else if future {
                        ("pending", "payment_source_time_pending")
                    } else {
                        (source.funding_status.as_str(), source.reason_code.as_str())
                    };
                    let value =
                        serde_json::to_value(&source).map_err(|_| "promotionSnapshotInvalid")?;
                    let source_hash = observe(
                        &mut tx,
                        &self.environment,
                        campaign_id,
                        account_id,
                        &subscription,
                        &value,
                        status,
                        reason,
                        now_ms,
                    )?;
                    counts.3 += 1;
                    if early_payment {
                        db(tx.execute("UPDATE promotion_attributions SET status='review',reason_code='payment_precedes_attribution' WHERE environment=$1 AND campaign_id=$2 AND account_id=$3 AND status<>'excluded'",&[&self.environment,&campaign_id,&account_id]))?;
                    }
                    let existing_id = existing.as_ref().map(|r| r.get::<_, String>(0));
                    if let Some(original) = &existing {
                        let consistent = original.get::<_, String>(8)
                            == "verified_paid_credit_fifo"
                            && positive(original.get(3)) == source.eligible_micro_usd
                            && positive(original.get(4)) == source.paid_at_ms
                            && positive(original.get(5)) == source.entitlement_at_ms
                            && positive(original.get(6)) == source.service_start_ms
                            && positive(original.get(7)) == source.service_end_ms;
                        if !consistent {
                            hold(
                                &mut tx,
                                &self.environment,
                                campaign_id,
                                account_id,
                                &subscription,
                                existing_id.as_deref(),
                                true,
                                "payment_source_conflict",
                                &source_hash,
                                source.reversed_micro_usd,
                                source.source_observed_at_ms,
                                now_ms,
                            )?;
                            counts.1 += 1;
                            continue;
                        }
                    }
                    if status != "verified_paid" {
                        if existing_id.is_some() || early_payment {
                            hold(
                                &mut tx,
                                &self.environment,
                                campaign_id,
                                account_id,
                                &subscription,
                                existing_id.as_deref(),
                                true,
                                reason,
                                &source_hash,
                                source.reversed_micro_usd,
                                source.source_observed_at_ms,
                                now_ms,
                            )?;
                        }
                        if status == "excluded" {
                            counts.2 += 1;
                        } else {
                            counts.1 += 1;
                        }
                        continue;
                    }
                    let id = existing_id.unwrap_or(allocation_id(
                        &self.environment,
                        account_id,
                        source.unique_id,
                    )?);
                    if existing.is_none() {
                        if source.eligible_micro_usd == 0 {
                            counts.2 += 1;
                            continue;
                        }
                        let first = source
                            .parts
                            .iter()
                            .find(|p| p.funding_status == "verified_paid")
                            .ok_or("promotionPaymentEvidenceMissing")?;
                        let original_amount = source.parts.iter().try_fold(0u64, |sum, p| {
                            sum.checked_add(p.original_amount_minor)
                                .ok_or("promotionAmountOverflow")
                        })?;
                        db(tx.execute("INSERT INTO promotion_revenue_allocations(environment,allocation_id,campaign_id,account_id,source_order_id,subscription_id,paid_at_ms,entitlement_at_ms,service_start_ms,service_end_ms,eligible_micro_usd,original_currency,original_amount_minor,fx_snapshot_ref,source_evidence_ref,source_payload_hash,funding_status,reason_code,is_card_payment) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,'usd',$12,$13,$14,$15,'verified_paid','verified_paid_credit_fifo',TRUE)",&[&self.environment,&id,&campaign_id,&account_id,&first.source_order_id,&subscription,&signed(source.paid_at_ms)?,&signed(source.entitlement_at_ms)?,&signed(source.service_start_ms)?,&signed(source.service_end_ms)?,&signed(source.eligible_micro_usd)?,&signed(original_amount)?,&first.fx_snapshot_ref,&source.source_evidence_ref,&source_hash]))?;
                        for part in source
                            .parts
                            .iter()
                            .filter(|p| p.funding_status == "verified_paid")
                        {
                            db(tx.execute("INSERT INTO promotion_credit_lot_allocations(environment,allocation_id,account_id,recharge_order_id,paid_credits,net_micro_usd,source_evidence_ref,lot_id,credits_offset) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)",&[&self.environment,&id,&account_id,&part.source_order_id,&i64::from(part.credits),&signed(part.eligible_micro_usd)?,&part.evidence_ref,&part.lot_id,&i64::from(part.credits_offset)]))?;
                        }
                        audit(
                            &mut tx,
                            &self.environment,
                            "live_payment_adapter",
                            "source.revenue",
                            &id,
                            &source.source_evidence_ref,
                            json!({"campaignId":campaign_id,"accountId":account_id,"subscriptionId":subscription,"eligibleMicroUsd":source.eligible_micro_usd.to_string(),"source":"authoritative_credit_fifo","sandbox":self.environment!="production","livemode":self.environment=="production"}),
                            now_ms,
                        )?;
                        counts.0 += 1;
                    }
                    let reversed:i64=db(tx.query_one("SELECT COALESCE(sum(reversal_micro_usd),0)::bigint FROM promotion_revenue_adjustments WHERE environment=$1 AND allocation_id=$2",&[&self.environment,&id]))?.get(0);
                    let current = positive(reversed);
                    if source.reversed_micro_usd < current {
                        hold(
                            &mut tx,
                            &self.environment,
                            campaign_id,
                            account_id,
                            &subscription,
                            Some(&id),
                            true,
                            "payment_reversal_source_decreased",
                            &source_hash,
                            current,
                            source.source_observed_at_ms,
                            now_ms,
                        )?;
                        counts.1 += 1;
                        continue;
                    }
                    let waiting = source.reversed_micro_usd > current;
                    hold(
                        &mut tx,
                        &self.environment,
                        campaign_id,
                        account_id,
                        &subscription,
                        Some(&id),
                        waiting,
                        if waiting {
                            "source_reversal_pending"
                        } else {
                            "payment_source_verified"
                        },
                        &source_hash,
                        source.reversed_micro_usd,
                        source.source_observed_at_ms,
                        now_ms,
                    )?;
                }
                refresh_account_qualifications(&mut tx, &self.environment, account_id, now_ms)?;
                db(tx.commit())?;
                Ok(counts)
            })?;
            imported += counts.0;
            pending += counts.1;
            excluded += counts.2;
            observations += counts.3;
        }
        // These durable holds survive a crash between source observation and
        // frozen-rate reversal posting. The session lock serializes target/delta.
        let jobs = with_account_store_postgres_client(&self.database_url, |client| {
            db(client.query("SELECT h.allocation_id,max(h.target_reversal_micro_usd),min(h.reversal_occurred_at_ms) FROM promotion_revenue_source_holds h WHERE h.environment=$1 AND h.allocation_id IS NOT NULL AND h.target_reversal_micro_usd>0 GROUP BY h.allocation_id ORDER BY h.allocation_id",&[&self.environment])).map(|rows|rows.iter().map(|r|(r.get::<_,String>(0),positive(r.get(1)),r.get::<_,Option<i64>>(2).map(positive).unwrap_or(now_ms))).collect::<Vec<_>>())
        })?;
        let mut reversals = 0u64;
        for (id, target, occurred) in jobs {
            let current = with_account_store_postgres_client(&self.database_url, |client| {
                db(client.query_one("SELECT COALESCE(sum(reversal_micro_usd),0)::bigint FROM promotion_revenue_adjustments WHERE environment=$1 AND allocation_id=$2",&[&self.environment,&id])).map(|r|positive(r.get(0)))
            })?;
            if target > current {
                let event = format!(
                    "fifo-reversal:{}:{target}",
                    id.strip_prefix("live_").unwrap_or(&id)
                );
                self.ingest_reversal(
                    &id,
                    &event,
                    target - current,
                    "authoritative_credit_fifo_refund",
                    occurred.min(now_ms),
                    now_ms,
                )?;
                reversals += 1;
            }
            with_account_store_postgres_client(&self.database_url, |client| {
                let mut tx = db(client.transaction())?;
                let owners=db(tx.query("SELECT campaign_id,account_id FROM promotion_revenue_source_holds WHERE environment=$1 AND allocation_id=$2 ORDER BY campaign_id,account_id",&[&self.environment,&id]))?;
                for owner in owners {
                    let campaign: String = owner.get(0);
                    let account: String = owner.get(1);
                    db(tx.query_one("SELECT campaign_id FROM promotion_campaigns WHERE environment=$1 AND campaign_id=$2 FOR UPDATE",&[&self.environment,&campaign]))?;
                    db(tx.execute("UPDATE promotion_revenue_source_holds SET active=FALSE,reason_code='payment_source_verified',updated_at_ms=$3 WHERE environment=$1 AND allocation_id=$2 AND reason_code='source_reversal_pending' AND target_reversal_micro_usd<=(SELECT COALESCE(sum(reversal_micro_usd),0)::bigint FROM promotion_revenue_adjustments WHERE environment=$1 AND allocation_id=$2)",&[&self.environment,&id,&signed(now_ms)?]))?;
                    refresh_account_qualifications(&mut tx, &self.environment, &account, now_ms)?;
                }
                db(tx.commit())?;
                Ok(())
            })?;
        }
        let holds = with_account_store_postgres_client(&self.database_url, |client| {
            db(client.query_one("SELECT count(*) FROM promotion_revenue_source_holds WHERE environment=$1 AND active",&[&self.environment])).map(|r|r.get::<_,i64>(0))
        })?;
        Ok(
            json!({"status":"reconciled","environment":self.environment,"accountsScanned":pairs.len(),"sourcesObserved":observations,"allocationsImported":imported,"pendingSources":pending,"excludedSources":excluded,"reversalsApplied":reversals,"activeHolds":holds,"authority":"persisted_game_account_credit_fifo","externalStripeAcceptance":false}),
        )
    }
}
