use super::*;
use serde_json::{json, Value};

fn vectors() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/hunting-material-vectors.json"
    ))
    .unwrap()
}

fn policy(case: &Value) -> Policy {
    let mut value: Value =
        serde_json::from_str(include_str!("../../../config/hunting-economy.json")).unwrap();
    if let Some(overrides) = case.get("policy_override").and_then(Value::as_object) {
        for (key, override_value) in overrides {
            value[key] = override_value.clone();
        }
    }
    serde_json::from_value(value).unwrap()
}

#[test]
fn material_shared_valuation_vectors_are_strict_and_canonical() {
    let values = vectors();
    for case in values["valid_valuations"].as_array().unwrap() {
        let catalog: MaterialValuationSet = serde_json::from_value(case["input"].clone()).unwrap();
        assert_eq!(
            serde_json::to_value(&catalog).unwrap(),
            case["expected"],
            "{}",
            case["name"]
        );
        assert_eq!(catalog.schema_version(), 1);
        assert_eq!(
            catalog.valuation_version(),
            case["input"]["valuation_version"].as_str().unwrap()
        );
        assert!(catalog
            .entries()
            .windows(2)
            .all(|entries| entries[0].item_index < entries[1].item_index));
    }
    for case in values["invalid_valuations"].as_array().unwrap() {
        assert!(
            serde_json::from_value::<MaterialValuationSet>(case["input"].clone()).is_err(),
            "{}",
            case["name"]
        );
    }
    for case in values["invalid_json"].as_array().unwrap() {
        assert!(
            serde_json::from_str::<MaterialValuationSet>(case["input"].as_str().unwrap()).is_err(),
            "{}",
            case["name"]
        );
    }
}

#[test]
fn material_shared_quote_vectors_have_no_gold_credit_or_bound_debit() {
    for case in vectors()["quotes"].as_array().unwrap() {
        let policy = policy(case);
        let unit_gold = case["unit_gold"].as_u64().unwrap();
        let quantity = case["quantity"].as_u64().unwrap();
        let quote = quote_material_conversion(&policy, unit_gold, quantity).unwrap();
        assert_eq!(
            serde_json::to_value(quote).unwrap(),
            case["expected"],
            "{}",
            case["name"]
        );
        let catalog = MaterialValuationSet::new(
            "fixture-v1",
            vec![MaterialValuationEntry {
                item_index: 1001,
                unit_gold,
            }],
        )
        .unwrap();
        assert_eq!(
            quote_material_conversion_for_item(&policy, &catalog, 1001, quantity).unwrap(),
            quote
        );
        assert_eq!(quote.license_atoms_debit, quote.gold_equivalent);
    }
}

#[test]
fn material_shared_invalid_quote_vectors_refuse_pg_overflow() {
    for case in vectors()["invalid_quotes"].as_array().unwrap() {
        let error = quote_material_conversion(
            &policy(case),
            case["unit_gold"].as_u64().unwrap(),
            case["quantity"].as_u64().unwrap(),
        )
        .unwrap_err();
        assert_eq!(
            error.code(),
            case["expected_error"].as_str().unwrap(),
            "{}",
            case["name"]
        );
    }
}

#[test]
fn material_shared_missing_price_vectors_never_fall_back() {
    for case in vectors()["unconfigured_quotes"].as_array().unwrap() {
        let catalog: MaterialValuationSet =
            serde_json::from_value(case["valuations"].clone()).unwrap();
        let error = quote_material_conversion_for_item(
            &policy(case),
            &catalog,
            case["item_index"].as_i64().unwrap() as i32,
            case["quantity"].as_u64().unwrap(),
        )
        .unwrap_err();
        assert_eq!(
            error.code(),
            case["expected_error"].as_str().unwrap(),
            "{}",
            case["name"]
        );
    }
}

#[test]
fn material_shared_split_vectors_preserve_quota_and_cannot_reduce_fee() {
    for case in vectors()["split_batches"].as_array().unwrap() {
        let policy = policy(case);
        let unit_gold = case["unit_gold"].as_u64().unwrap();
        let quantities: Vec<_> = case["quantities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|quantity| quantity.as_u64().unwrap())
            .collect();
        let combined =
            quote_material_conversion(&policy, unit_gold, quantities.iter().sum()).unwrap();
        let split: Vec<_> = quantities
            .into_iter()
            .map(|quantity| quote_material_conversion(&policy, unit_gold, quantity).unwrap())
            .collect();
        let split_fee: u64 = split.iter().map(|quote| quote.fee_unbound_gold).sum();
        assert_eq!(
            combined.fee_unbound_gold,
            case["expected_combined_fee"].as_u64().unwrap()
        );
        assert_eq!(split_fee, case["expected_split_fee"].as_u64().unwrap());
        assert!(split_fee >= combined.fee_unbound_gold);
        assert_eq!(
            split
                .iter()
                .map(|quote| quote.license_atoms_debit)
                .sum::<u64>(),
            combined.license_atoms_debit
        );
    }
}

#[test]
fn material_constructor_rejects_invalid_entries_and_version_bytes() {
    for version in [
        "".to_string(),
        " invalid".to_string(),
        "x\u{85}y".to_string(),
        "界".repeat(43),
    ] {
        assert!(MaterialValuationSet::new(version, vec![]).is_err());
    }
    for entries in [
        vec![MaterialValuationEntry {
            item_index: 0,
            unit_gold: 1,
        }],
        vec![MaterialValuationEntry {
            item_index: 1,
            unit_gold: 0,
        }],
        vec![MaterialValuationEntry {
            item_index: 1,
            unit_gold: MAX_MATERIAL_GOLD_EQUIVALENT + 1,
        }],
        vec![
            MaterialValuationEntry {
                item_index: 1,
                unit_gold: 1,
            },
            MaterialValuationEntry {
                item_index: 1,
                unit_gold: 1,
            },
        ],
    ] {
        assert!(MaterialValuationSet::new("fixture-v1", entries).is_err());
    }
    let catalog = MaterialValuationSet::new("fixture-v1", vec![]).unwrap();
    assert_eq!(
        catalog.unit_gold(0).unwrap_err().code(),
        "invalid_valuation"
    );
    let mut policy = policy(&json!({}));
    policy.conversion_fee.denominator = 0;
    assert_eq!(
        quote_material_conversion(&policy, 100, 1)
            .unwrap_err()
            .code(),
        "invalid_policy"
    );
}

#[test]
fn material_production_valuation_is_empty_and_disabled_policy_only_plans() {
    let catalog: MaterialValuationSet = serde_json::from_str(include_str!(
        "../../../config/hunting-material-valuations.json"
    ))
    .unwrap();
    assert!(catalog.entries().is_empty());
    let policy = policy(&json!({}));
    assert!(!policy.enabled);
    assert_eq!(
        quote_material_conversion_for_item(&policy, &catalog, 1001, 1)
            .unwrap_err()
            .code(),
        "unconfigured_material"
    );
    // Pure quotes are available for offline checks; this does not authorize an
    // enabled source, debit a wallet, or provision a missing permit stock.
    assert_eq!(
        quote_material_conversion(&policy, 1, 1)
            .unwrap()
            .fee_unbound_gold,
        1
    );
}
