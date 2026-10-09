//! Thin scalar ABI for the shared fixed-Windows Pearl admission rules.
//! Raw source and boolean custody remain the host's responsibility. Numeric
//! arguments use f64 so invalid ranges cannot wrap through a wasm i32 argument.

use mir2_client_core::npc_pearl_buy::{npc_pearl_catalog_unit_price, plan_npc_pearl_buy, NpcPearlBuyFacts, NpcPearlGood};
use wasm_bindgen::prelude::*;

const INVALID: &str = "{\"version\":1,\"ok\":false}";
const MAX_SAFE_JS_INTEGER: f64 = 9_007_199_254_740_991.0;

#[wasm_bindgen]
pub fn npc_pearl_buy_abi_version() -> u32 { 2 }

fn integer_in_range(value: f64, min: f64, max: f64) -> bool {
    value.is_finite() && value >= min && value <= max && value == value.trunc()
}

/// ABI2 inputs verify the actual catalogue unit price. Version1 output has fixed fields: version, ok, maxQuantity, quote,
/// admittedCount and denial (0 admitted; shared enum codes 1..7 rejected).
/// ok means scalar input is valid, not that a purchase was sent or committed.
/// Signed CreaturePearls is normalized with max(0) only when wallet_known.
/// Missing getter/capability or incomplete current host facts must fail closed.
#[wasm_bindgen]
pub fn npc_pearl_buy_plan(
    allows_buy: bool, selected: bool, use_pearls: bool,
    unique_id: f64, price: f64, stock: f64, quantity: f64,
    wallet_known: bool, pearls: f64, occupied: f64, info_price: f64, rate: f64,
) -> String {
    let ranges = [
        (unique_id, 0.0, MAX_SAFE_JS_INTEGER),
        (price, 0.0, f64::from(u32::MAX)),
        (stock, f64::from(i32::MIN), f64::from(i32::MAX)),
        (quantity, 0.0, f64::from(u16::MAX)),
        (pearls, f64::from(i32::MIN), f64::from(i32::MAX)),
        (occupied, 0.0, f64::from(u32::MAX)),
        (info_price, 0.0, f64::from(u32::MAX)),
    ];
    if ranges.into_iter().any(|(value, min, max)| !integer_in_range(value, min, max)) {
        return INVALID.to_owned();
    }
    if !rate.is_finite() || rate < 0.0 || npc_pearl_catalog_unit_price(info_price as u32, rate as f32) != Some(price as u32) {
        return INVALID.to_owned();
    }
    let plan = plan_npc_pearl_buy(NpcPearlBuyFacts {
        allows_buy,
        selected: selected.then_some(NpcPearlGood {
            unique_id: unique_id as u64, use_pearls, unit_price: price as u32, stock: stock as i32,
        }),
        quantity: quantity as u16,
        balance: wallet_known.then_some((pearls as i32).max(0) as u32),
        occupied_bag_entries: occupied as usize,
    });
    let quote = plan.quote.map_or_else(|| "null".to_owned(), |value| value.to_string());
    let count = plan.admitted_count.map_or_else(|| "null".to_owned(), |value| value.to_string());
    let denial = plan.denial.map_or(0, |reason| reason as u8);
    format!("{{\"version\":1,\"ok\":true,\"maxQuantity\":{},\"quote\":{},\"admittedCount\":{},\"denial\":{}}}",
        plan.max_quantity, quote, count, denial)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn plan(price: f64, stock: f64, quantity: f64, known: bool, pearls: f64, occupied: f64) -> Value {
        serde_json::from_str(&npc_pearl_buy_plan(true, true, true, 0.0, price, stock, quantity,
            known, pearls, occupied, price, 1.0)).unwrap()
    }

    #[test]
    fn pearl_abi_quote_and_admitted_count_follow_the_shared_windows_plan() {
        assert_eq!(npc_pearl_buy_abi_version(), 2);
        assert_eq!(plan(10.0, -1.0, 0.0, true, 10.0, 45.0), json!({
            "version":1,"ok":true,"maxQuantity":99,"quote":10,"admittedCount":1,"denial":0,
        }));
        assert_eq!(plan(10.0, -2.0, 65535.0, true, 990.0, 0.0)["admittedCount"], 99);
        let saturated = plan(f64::from(u32::MAX), -1.0, 99.0, true, f64::from(i32::MAX), 0.0);
        assert_eq!(saturated["quote"], json!(u32::MAX));
        assert_eq!((saturated["admittedCount"].clone(), saturated["denial"].clone()), (Value::Null, json!(6)));
    }

    #[test]
    fn pearl_abi_signed_wallet_max_zero_and_unknown_free_wallet_are_distinct() {
        for pearls in [f64::from(i32::MIN), -1.0, 0.0] {
            assert_eq!(plan(0.0, -1.0, 1.0, true, pearls, 0.0)["denial"], 0);
            assert_eq!(plan(1.0, -1.0, 1.0, true, pearls, 0.0)["denial"], 6);
        }
        assert_eq!(plan(0.0, -1.0, 1.0, false, 0.0, 0.0)["denial"], 4);
        assert_eq!(plan(2_147_483_520.0, -1.0, 1.0, true, f64::from(i32::MAX), 0.0)["denial"], 0);
    }

    #[test]
    fn pearl_abi_rejects_coercible_fractional_nonfinite_and_out_of_range_numbers() {
        let valid = [0.0, 10.0, -1.0, 1.0, 10.0, 0.0];
        for (index, malformed) in [
            (0, -1.0), (0, MAX_SAFE_JS_INTEGER + 1.0),
            (1, -1.0), (1, f64::from(u32::MAX) + 1.0),
            (2, f64::from(i32::MIN) - 1.0), (2, f64::from(i32::MAX) + 1.0),
            (3, -1.0), (3, f64::from(u16::MAX) + 1.0),
            (4, f64::from(i32::MIN) - 1.0), (4, f64::from(i32::MAX) + 1.0),
            (5, -1.0), (5, f64::from(u32::MAX) + 1.0),
        ] {
            let mut fields = valid; fields[index] = malformed;
            assert_eq!(npc_pearl_buy_plan(true, true, true, fields[0], fields[1], fields[2], fields[3],
                true, fields[4], fields[5], 10.0, 1.0), INVALID);
        }
        for index in 0..valid.len() {
            for malformed in [0.5, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                let mut fields = valid; fields[index] = malformed;
                assert_eq!(npc_pearl_buy_plan(true, true, true, fields[0], fields[1], fields[2], fields[3],
                    true, fields[4], fields[5], 10.0, 1.0), INVALID);
            }
        }
    }

    #[test]
    fn pearl_abi_service_selection_currency_stock_and_bag_denials_are_fixed() {
        for (allows, selected, pearls, reason) in [
            (false, true, true, 1), (true, false, true, 2), (true, true, false, 3),
        ] {
            let output: Value = serde_json::from_str(&npc_pearl_buy_plan(allows, selected, pearls,
                0.0, 10.0, -1.0, 1.0, true, 10.0, 0.0, 10.0, 1.0)).unwrap();
            assert_eq!(output["denial"], reason);
            assert_eq!(output["admittedCount"], Value::Null);
        }
        assert_eq!(plan(10.0, 0.0, 1.0, true, 10.0, 0.0)["denial"], 5);
        assert_eq!(plan(10.0, 1.0, 1.0, true, 10.0, 46.0)["denial"], 7);
        assert_eq!(plan(10.0, 1.0, 1.0, true, 10.0, 45.0)["denial"], 0);
    }

    #[test]
    fn pearl_abi_raw_uid_zero_and_js_safe_boundary_keep_identical_admission() {
        for uid in [0.0, MAX_SAFE_JS_INTEGER] {
            let output: Value = serde_json::from_str(&npc_pearl_buy_plan(true, true, true,
                uid, 10.0, -1.0, 2.0, true, 20.0, 0.0, 10.0, 1.0)).unwrap();
            assert_eq!(output["admittedCount"], 2);
            assert_eq!(output["denial"], 0);
            assert_eq!(output.as_object().unwrap().len(), 6);
        }
    }
    #[test]
    fn pearl_abi_mismatched_partial_free_projection_and_invalid_rate_fail_before_admission() {
        for (price, base, rate) in [(0.0, 100.0, 1.0), (127.0, 101.0, 1.25), (1.0, -1.0, 1.0),
            (1.0, 1.5, 1.0), (1.0, 4_294_967_296.0, 1.0), (1.0, 1.0, -1.0), (0.0, 100.0, -1e-300),
            (1.0, 1.0, f64::INFINITY), (1.0, 1.0, f64::NAN), (1.0, 1.0, f64::MAX)] {
            assert_eq!(npc_pearl_buy_plan(true, true, true, 0.0, price, -1.0, 1.0,
                true, 1000.0, 0.0, base, rate), INVALID);
        }
        for (price, base, rate) in [(126.0, 101.0, 1.25), (0.0, 0.0, 1.0), (0.0, 100.0, 0.0),
            (16_777_216.0, 16_777_217.0, 1.0)] {
            let output: Value = serde_json::from_str(&npc_pearl_buy_plan(true, true, true, 0.0, price,
                -1.0, 1.0, false, 0.0, 0.0, base, rate)).unwrap();
            assert_eq!(output["ok"], true);
            assert_eq!(output["denial"], 4);
            assert_eq!(output["quote"], price as u32);
        }
    }

}
