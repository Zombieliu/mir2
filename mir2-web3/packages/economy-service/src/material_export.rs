//! Material diagnostics validate the authoritative ledger, never inventory or
//! market prices. A material's fixed valuation consumes permits, not coins;
//! only the fee is a coin burn.

use super::{amount, db, fingerprint};
use crate::{EconomyError, EconomyResult, SettlementReceipt};
use mir2_economy_policy::Policy;
use postgres::Transaction;
use serde_json::Value;

pub(super) fn combined_conversion_totals(receipt: &SettlementReceipt) -> EconomyResult<(u64, u64)> {
    let gold_fee = receipt
        .gross_conversion_gold
        .checked_sub(receipt.net_unbound_gold_inflow)
        .ok_or(EconomyError::CorruptHistory("metric_conversion_net"))?;
    let consumed = receipt
        .gross_conversion_gold
        .checked_add(receipt.material_conversion_gold_equivalent)
        .ok_or(EconomyError::CorruptHistory("metric_conversion_totals"))?;
    let fee = gold_fee
        .checked_add(receipt.material_conversion_fee_unbound_gold)
        .ok_or(EconomyError::CorruptHistory("metric_conversion_totals"))?;
    if (receipt.material_conversion_gold_equivalent == 0)
        != (receipt.material_conversion_fee_unbound_gold == 0)
        || receipt.material_conversion_fee_unbound_gold
            > receipt.material_conversion_gold_equivalent
    {
        return Err(EconomyError::CorruptHistory("metric_material_aggregate"));
    }
    Ok((consumed, fee))
}

pub(super) fn post_settlement_consumption(
    tx: &mut Transaction<'_>,
    realm: &str,
    through: i64,
) -> EconomyResult<u64> {
    let row = tx
        .query_one(
            "SELECT COALESCE(sum(consumed_atoms),0)::text AS consumed FROM (
                SELECT consumed_atoms FROM hunting_economy_conversions
                    WHERE realm_id=$1 AND business_day>$2
                UNION ALL
                SELECT consumed_atoms FROM hunting_economy_material_conversions
                    WHERE realm_id=$1 AND business_day>$2
            ) conversions",
            &[&realm, &through],
        )
        .map_err(db)?;
    amount(&row, "consumed")
}

fn validate_material_totals(
    receipt: &SettlementReceipt,
    equivalent: u64,
    fee: u64,
    consumed: u64,
) -> EconomyResult<()> {
    if equivalent != receipt.material_conversion_gold_equivalent
        || fee != receipt.material_conversion_fee_unbound_gold
        || consumed != equivalent
    {
        return Err(EconomyError::CorruptHistory("metric_material_aggregate"));
    }
    Ok(())
}

fn validate_original_policy(value: Value, digest: &str, version: &str) -> EconomyResult<()> {
    if fingerprint(&value)? != digest {
        return Err(EconomyError::CorruptHistory(
            "metric_material_policy_digest",
        ));
    }
    let policy: Policy = serde_json::from_value(value)
        .map_err(|_| EconomyError::CorruptHistory("metric_material_policy"))?;
    policy
        .validate()
        .map_err(|_| EconomyError::CorruptHistory("metric_material_policy"))?;
    if policy.policy_version != version {
        return Err(EconomyError::CorruptHistory(
            "metric_material_policy_version",
        ));
    }
    Ok(())
}

pub(super) fn validate_material_flow(
    tx: &mut Transaction<'_>,
    realm: &str,
    receipt: &SettlementReceipt,
) -> EconomyResult<()> {
    let day = receipt.business_day;
    let flow = tx
        .query_one(
            "SELECT COALESCE(sum(gold_equivalent),0)::text AS equivalent,
            COALESCE(sum(fee_gold),0)::text AS fee,
            COALESCE(sum(consumed_atoms),0)::text AS consumed
         FROM hunting_economy_material_conversions WHERE realm_id=$1 AND business_day=$2",
            &[&realm, &day],
        )
        .map_err(db)?;
    validate_material_totals(
        receipt,
        amount(&flow, "equivalent")?,
        amount(&flow, "fee")?,
        amount(&flow, "consumed")?,
    )?;

    // The quote's composite FKs bind its immutable policy, valuation, owner and
    // amounts. Check the operation identity too, without interpreting inventory
    // checkpoint JSON as an alternative source of conversion authority.
    let bad_operation: bool = tx
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM hunting_economy_material_conversions c
         LEFT JOIN hunting_economy_operations o
            ON o.realm_id=c.realm_id AND o.operation_key=c.operation_key
         WHERE c.realm_id=$1 AND c.business_day=$2 AND
            (o.operation_key IS NULL OR o.operation_kind<>'material_conversion'
             OR o.business_day<>c.business_day OR o.committed_at_ms<>c.committed_at_ms))",
            &[&realm, &day],
        )
        .map_err(db)?
        .get(0);
    if bad_operation {
        return Err(EconomyError::CorruptHistory("metric_material_operation"));
    }

    let bad_fee: bool = tx
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM hunting_economy_material_conversions c
         LEFT JOIN hunting_economy_burn_events b
            ON b.realm_id=c.realm_id AND b.source_kind='material_conversion_fee'
            AND b.source_key=c.operation_key
         WHERE c.realm_id=$1 AND c.business_day=$2 AND
            (b.event_id IS NULL OR b.layer<>'daily' OR b.business_day<>c.business_day
             OR b.operation_key<>c.operation_key OR b.gold_equivalent<>c.fee_gold
             OR b.policy_version<>c.policy_version
             OR b.valuation_version<>'gold-face-value-v1'
             OR b.committed_at_ms<>c.committed_at_ms))
         OR EXISTS(SELECT 1 FROM hunting_economy_burn_events b
         LEFT JOIN hunting_economy_material_conversions c
            ON c.realm_id=b.realm_id AND c.operation_key=b.source_key
         WHERE b.realm_id=$1 AND b.business_day=$2
            AND b.source_kind='material_conversion_fee' AND
            (c.operation_key IS NULL OR c.business_day<>b.business_day))",
            &[&realm, &day],
        )
        .map_err(db)?
        .get(0);
    if bad_fee {
        return Err(EconomyError::CorruptHistory("metric_material_fee_burn"));
    }

    // A quote may retain an earlier policy than the settlement. Validate every
    // referenced policy's typed shape/digest before SQL numeric division, then
    // calculate each fee with that quote's own policy, never today's rate.
    // Use integer div(n+d-1,d): ordinary numeric / may round away a positive
    // remainder before ceil, even for a three-gold quote with a large ratio.
    for row in tx
        .query(
            "SELECT DISTINCT c.policy_version,p.policy_json,p.policy_digest
         FROM hunting_economy_material_conversions c
         LEFT JOIN hunting_economy_policies p
            ON p.realm_id=c.realm_id AND p.policy_version=c.policy_version
         WHERE c.realm_id=$1 AND c.business_day=$2",
            &[&realm, &day],
        )
        .map_err(db)?
    {
        let value =
            row.get::<_, Option<Value>>("policy_json")
                .ok_or(EconomyError::CorruptHistory(
                    "metric_material_policy_missing",
                ))?;
        let digest =
            row.get::<_, Option<String>>("policy_digest")
                .ok_or(EconomyError::CorruptHistory(
                    "metric_material_policy_missing",
                ))?;
        validate_original_policy(value, &digest, &row.get::<_, String>("policy_version"))?;
    }
    let wrong_rate: bool = tx
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM hunting_economy_material_conversions c
         JOIN hunting_economy_policies p
            ON p.realm_id=c.realm_id AND p.policy_version=c.policy_version
         WHERE c.realm_id=$1 AND c.business_day=$2 AND c.fee_gold::numeric
            IS DISTINCT FROM div(c.gold_equivalent::numeric
                * (p.policy_json #>> '{conversion_fee,numerator}')::numeric
                + (p.policy_json #>> '{conversion_fee,denominator}')::numeric - 1,
                (p.policy_json #>> '{conversion_fee,denominator}')::numeric))",
            &[&realm, &day],
        )
        .map_err(db)?
        .get(0);
    if wrong_rate {
        return Err(EconomyError::CorruptHistory("metric_material_fee_rate"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn policy() -> Policy {
        serde_json::from_value(json!({
            "schema_version":1,"policy_version":"original-v1","enabled":false,
            "k":{"numerator":1,"denominator":10},
            "conversion_fee":{"numerator":1,"denominator":50},
            "daily_decay":{"numerator":1,"denominator":150},
            "baseline_daily_mint_atoms":720000,
            "daily_mint_cap_multiplier":{"numerator":3,"denominator":2},
            "business_timezone":"Asia/Shanghai","settlement_hour":4
        }))
        .unwrap()
    }

    fn receipt() -> SettlementReceipt {
        SettlementReceipt {
            realm_id: "unit".into(),
            business_day: 1,
            policy_version: "original-v1".into(),
            committed_at_ms: 1,
            burns: crate::BurnTotals::default(),
            plan: mir2_economy_policy::plan_settlement(&policy(), Default::default(), 0).unwrap(),
            consumed_atoms: 0,
            gross_conversion_gold: 0,
            net_unbound_gold_inflow: 0,
            material_conversion_gold_equivalent: 0,
            material_conversion_fee_unbound_gold: 0,
            bound_gold_stock: "0".into(),
            unbound_gold_stock: "0".into(),
        }
    }

    #[test]
    fn material_allowance_and_fee_do_not_become_coin_inflow() {
        let mut receipt = receipt();
        receipt.gross_conversion_gold = 1000;
        receipt.net_unbound_gold_inflow = 980;
        receipt.material_conversion_gold_equivalent = 250;
        receipt.material_conversion_fee_unbound_gold = 5;
        receipt.consumed_atoms = 1250;
        assert_eq!(combined_conversion_totals(&receipt).unwrap(), (1250, 25));
        assert_eq!(receipt.net_unbound_gold_inflow, 980);
        assert!(validate_material_totals(&receipt, 250, 5, 250).is_ok());
        assert!(validate_material_totals(&receipt, 250, 5, 1250).is_err());
        assert!(validate_material_totals(&receipt, 250, 6, 250).is_err());
        assert!(validate_material_totals(&receipt, 249, 5, 249).is_err());
        receipt.burns.daily = 25;
        receipt.plan =
            mir2_economy_policy::plan_settlement(&policy(), Default::default(), 25).unwrap();
        assert!(super::super::validate_plan(&receipt, &policy()).is_ok());
        receipt.burns.daily = 24;
        receipt.plan =
            mir2_economy_policy::plan_settlement(&policy(), Default::default(), 24).unwrap();
        assert!(super::super::validate_plan(&receipt, &policy()).is_err());
    }

    #[test]
    fn material_snapshot_cannot_hide_fees_or_overflow_combined_totals() {
        let mut receipt = receipt();
        receipt.material_conversion_fee_unbound_gold = 1;
        assert!(combined_conversion_totals(&receipt).is_err());
        receipt.material_conversion_gold_equivalent = 1;
        receipt.gross_conversion_gold = u64::MAX;
        receipt.net_unbound_gold_inflow = u64::MAX;
        assert!(combined_conversion_totals(&receipt).is_err());
        receipt.gross_conversion_gold = 0;
        receipt.net_unbound_gold_inflow = 0;
        receipt.material_conversion_fee_unbound_gold = 2;
        assert!(combined_conversion_totals(&receipt).is_err());
    }

    #[test]
    fn original_material_policy_is_digest_and_version_bound_even_when_disabled() {
        let value = serde_json::to_value(policy()).unwrap();
        let digest = fingerprint(&value).unwrap();
        assert!(validate_original_policy(value.clone(), &digest, "original-v1").is_ok());
        assert!(validate_original_policy(value.clone(), &digest, "settlement-v2").is_err());
        let mut changed = value;
        changed["conversion_fee"]["numerator"] = json!(2);
        assert!(validate_original_policy(changed.clone(), &digest, "original-v1").is_err());
        changed["conversion_fee"]["denominator"] = json!(0);
        let digest = fingerprint(&changed).unwrap();
        assert!(validate_original_policy(changed, &digest, "original-v1").is_err());
    }

    #[test]
    fn legacy_settlement_keeps_original_json_and_payload_digest() {
        // Fixed pre-material shape and independently calculated SHA-256: do not
        // create this fixture with today's serializer and silently move the bar.
        let legacy = json!({
            "realm_id":"legacy","business_day":1,"policy_version":"original-v1",
            "committed_at_ms":1,"burns":{"daily":0,"light_pvp":0,"siege":0},
            "plan":{"destruction_gold_equivalent":0,"opening_stock_atoms":0,
                "raw_mint_atoms":0,"daily_mint_cap_atoms":1080000,"minted_atoms":0,
                "capped_atoms":0,"pre_decay_stock_atoms":0,"decayed_atoms":0,
                "next_pool":{"stock_atoms":0,"mint_remainder":0,"decay_remainder":0}},
            "consumed_atoms":0,"gross_conversion_gold":0,"net_unbound_gold_inflow":0,
            "bound_gold_stock":"0","unbound_gold_stock":"0"
        });
        assert!(!legacy
            .as_object()
            .unwrap()
            .contains_key("material_conversion_gold_equivalent"));
        assert!(!legacy
            .as_object()
            .unwrap()
            .contains_key("material_conversion_fee_unbound_gold"));
        let digest = fingerprint(&legacy).unwrap();
        assert_eq!(
            digest,
            "sha256:32e5925bc36590c075f78feae42c594ee27eba7b81500fa8a55a691f0ba960fc"
        );
        let recovered: SettlementReceipt = serde_json::from_value(legacy.clone()).unwrap();
        assert_eq!(recovered.material_conversion_gold_equivalent, 0);
        assert_eq!(recovered.material_conversion_fee_unbound_gold, 0);
        let recovered = serde_json::to_value(recovered).unwrap();
        assert_eq!(
            serde_json::to_vec(&legacy).unwrap(),
            serde_json::to_vec(&recovered).unwrap()
        );
        assert_eq!(fingerprint(&recovered).unwrap(), digest);
        let mut material: SettlementReceipt = serde_json::from_value(legacy).unwrap();
        material.material_conversion_gold_equivalent = 1;
        material.material_conversion_fee_unbound_gold = 1;
        assert_ne!(
            fingerprint(&serde_json::to_value(material).unwrap()).unwrap(),
            digest
        );
    }
}
