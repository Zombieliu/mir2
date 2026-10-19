use mir2_economy_policy::{
    business_day_key, plan_settlement, quote_conversion, settlement_window, Policy, PoolState,
    GOLD_PER_LICENSE, LICENSE_ATOMS_PER_UNIT,
};
use serde_json::Value;
use std::{fs, path::PathBuf};

fn source_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    [manifest.clone(), manifest.join("../..")]
        .into_iter()
        .find(|path| path.join("config/hunting-economy.json").is_file())
        .expect("repository policy must exist beside the standalone source or above packages/")
}

fn inputs() -> (Policy, Value) {
    let root = source_root();
    let policy = serde_json::from_str(
        &fs::read_to_string(root.join("config/hunting-economy.json")).unwrap(),
    )
    .unwrap();
    let vectors: Value = serde_json::from_str(
        &fs::read_to_string(root.join("fixtures/hunting-economy-vectors.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(vectors["schema_version"], 1);
    (policy, vectors)
}

fn policy_for(policy: &Policy, case: &Value) -> Policy {
    let mut raw = serde_json::to_value(policy).unwrap();
    if let Some(patch) = case.get("policy_override") {
        for (key, value) in patch.as_object().unwrap() {
            raw[key] = value.clone();
        }
    }
    serde_json::from_value(raw).unwrap()
}

fn pool(value: &Value) -> PoolState {
    serde_json::from_value(value.clone()).unwrap()
}

fn assert_expected(actual: Value, expected: &Value, name: &Value) {
    for (field, value) in expected.as_object().unwrap() {
        assert_eq!(&actual[field], value, "case {name}, field {field}");
    }
}

#[test]
fn disabled_default_and_units_are_explicit() {
    let (policy, _) = inputs();
    assert!(!policy.enabled);
    assert_eq!(LICENSE_ATOMS_PER_UNIT, 1_000_000);
    assert_eq!(GOLD_PER_LICENSE, 1_000_000);
    assert_eq!(policy.baseline_daily_mint_atoms, 720_000);
    assert_eq!(policy.daily_mint_cap_atoms().unwrap(), 1_080_000);
}

#[test]
fn recalibrated_daily_capacity_matches_the_fixed_destruction_scenario() {
    let (policy, _) = inputs();
    let mut pool = PoolState::default();
    let mut gross = 0;
    let mut net = 0;
    for _ in 0..30 {
        let plan = plan_settlement(&policy, pool, 7_200_000).unwrap();
        assert_eq!(plan.minted_atoms, 720_000);
        assert_eq!(plan.decayed_atoms, 4_800);
        let quote = quote_conversion(&policy, plan.next_pool.stock_atoms).unwrap();
        assert_eq!(quote.bound_gold_debit, 715_200);
        assert_eq!(quote.fee_unbound_gold, 14_304);
        assert_eq!(quote.net_unbound_gold_credit, 700_896);
        gross += quote.bound_gold_debit;
        net += quote.net_unbound_gold_credit;
        pool = plan.next_pool;
        pool.stock_atoms -= quote.license_atoms_debit;
    }
    assert_eq!(gross, 21_456_000);
    assert_eq!(net, 21_026_880);
    assert_eq!(pool, PoolState::default());
}

#[test]
fn shared_settlement_vectors_and_pure_repeated_planning() {
    let (policy, vectors) = inputs();
    for case in vectors["settlements"].as_array().unwrap() {
        let effective = policy_for(&policy, case);
        let initial = pool(&case["pool"]);
        let destruction = case["destruction_gold_equivalent"].as_u64().unwrap();
        let result = plan_settlement(&effective, initial, destruction).unwrap();
        assert_expected(
            serde_json::to_value(&result).unwrap(),
            &case["expected"],
            &case["name"],
        );
        // Planning is intentionally repeatable; the durable server must prevent
        // duplicate application with a unique realm/day key and a transaction.
        assert_eq!(
            result,
            plan_settlement(&effective, initial, destruction).unwrap()
        );
    }
}

#[test]
fn shared_settlement_sequences_drop_capped_atoms_and_preserve_fractions() {
    let (policy, vectors) = inputs();
    for case in vectors["settlement_sequences"].as_array().unwrap() {
        let effective = policy_for(&policy, case);
        let mut current = pool(&case["initial_pool"]);
        for step in case["steps"].as_array().unwrap() {
            let result = plan_settlement(
                &effective,
                current,
                step["destruction_gold_equivalent"].as_u64().unwrap(),
            )
            .unwrap();
            assert_expected(
                serde_json::to_value(&result).unwrap(),
                &step["expected"],
                &case["name"],
            );
            current = result.next_pool;
        }
    }
}

#[test]
fn shared_conversion_vectors_cover_fractional_permits_fee_rounding_and_u64_max() {
    let (policy, vectors) = inputs();
    for case in vectors["conversions"].as_array().unwrap() {
        let quote = quote_conversion(
            &policy_for(&policy, case),
            case["gross_gold"].as_u64().unwrap(),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(quote).unwrap(),
            case["expected"],
            "case {}",
            case["name"]
        );
    }
}

#[test]
fn shared_invalid_policy_vectors_reject_bad_rates_and_cap_overflow() {
    let (policy, vectors) = inputs();
    for case in vectors["invalid_policies"].as_array().unwrap() {
        let error = policy_for(&policy, case).validate().unwrap_err();
        assert_eq!(error.code(), case["expected_error"].as_str().unwrap());
    }
}

#[test]
fn shared_invalid_settlements_reject_remainder_and_balance_overflow() {
    let (policy, vectors) = inputs();
    for case in vectors["invalid_settlements"].as_array().unwrap() {
        let error = plan_settlement(
            &policy_for(&policy, case),
            pool(&case["pool"]),
            case["destruction_gold_equivalent"].as_u64().unwrap(),
        )
        .unwrap_err();
        assert_eq!(error.code(), case["expected_error"].as_str().unwrap());
    }
}

#[test]
fn shared_invalid_conversions_reject_zero() {
    let (policy, vectors) = inputs();
    for case in vectors["invalid_conversions"].as_array().unwrap() {
        let error = quote_conversion(
            &policy_for(&policy, case),
            case["gross_gold"].as_u64().unwrap(),
        )
        .unwrap_err();
        assert_eq!(error.code(), case["expected_error"].as_str().unwrap());
    }
}

#[test]
fn shared_business_days_use_four_am_china_time_and_euclidean_division() {
    let (_, vectors) = inputs();
    for case in vectors["business_days"].as_array().unwrap() {
        assert_eq!(
            business_day_key(case["unix_seconds"].as_i64().unwrap()).unwrap(),
            case["expected_day_key"].as_i64().unwrap(),
            "case {}",
            case["name"]
        );
    }
    for case in vectors["invalid_business_days"].as_array().unwrap() {
        assert_eq!(
            business_day_key(case["unix_seconds"].as_i64().unwrap())
                .unwrap_err()
                .code(),
            case["expected_error"].as_str().unwrap()
        );
    }
}

#[test]
fn shared_windows_are_half_open_and_reject_overflow() {
    let (_, vectors) = inputs();
    for case in vectors["settlement_windows"].as_array().unwrap() {
        assert_eq!(
            settlement_window(case["day_key"].as_i64().unwrap()).unwrap(),
            (
                case["expected_start"].as_i64().unwrap(),
                case["expected_end"].as_i64().unwrap()
            )
        );
    }
    for case in vectors["invalid_settlement_windows"].as_array().unwrap() {
        assert_eq!(
            settlement_window(case["day_key"].as_i64().unwrap())
                .unwrap_err()
                .code(),
            case["expected_error"].as_str().unwrap()
        );
    }
}

#[test]
fn strict_policy_json_rejects_unknown_fields_and_noninteger_amounts() {
    let (policy, _) = inputs();
    let mut raw = serde_json::to_value(policy).unwrap();
    raw["unexpected"] = Value::Bool(true);
    assert!(serde_json::from_value::<Policy>(raw.clone()).is_err());
    raw.as_object_mut().unwrap().remove("unexpected");
    for invalid in [
        serde_json::json!(-1),
        serde_json::json!(0.5),
        Value::Bool(true),
    ] {
        raw["baseline_daily_mint_atoms"] = invalid;
        assert!(serde_json::from_value::<Policy>(raw.clone()).is_err());
    }
}

#[test]
fn fee_cannot_be_reduced_by_splitting_a_conversion() {
    let (policy, _) = inputs();
    let combined = quote_conversion(&policy, 102).unwrap().fee_unbound_gold;
    let split = 2 * quote_conversion(&policy, 51).unwrap().fee_unbound_gold;
    assert!(split >= combined);
}
