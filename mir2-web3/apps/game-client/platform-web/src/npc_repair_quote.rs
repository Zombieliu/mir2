//! Thin read-only ABI for the shared Native repair quote rules.
use mir2_client_core::npc_repair_quote::{repair_is_affordable, repair_quote, NpcRepairSource};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn npc_repair_quote_abi_version() -> u32 { 1 }

/// The host must validate a complete raw item source and current live fields
/// before calling. Omitted source fields must not be replaced by zero defaults.
#[wasm_bindgen]
pub fn npc_repair_quote(
    live_unique_id: u64, source_unique_id: u64, template_item_index: i32,
    source_item_index: i32, template_price: u32, template_durability: u16,
    live_count: u32, live_current_dura: u16, live_max_dura: u16,
    added_stat_values: &[i32], rental: bool, rate: f32, special: bool, gold: u32,
) -> String {
    let source = NpcRepairSource { live_unique_id: Some(live_unique_id), source_unique_id,
        template_item_index, source_item_index, template_price, template_durability,
        live_count, live_current_dura: Some(live_current_dura), live_max_dura: Some(live_max_dura),
        added_stat_values, rental };
    let Some(quote) = repair_quote(Some(source), rate, special) else {
        return "{\"version\":1,\"known\":false,\"quote\":null}".into();
    };
    // f64::from keeps the exact f32 value in the JS JSON number; affordability
    // is already evaluated by the shared f32 Native gate, never JS truncation.
    // Use the existing JSON float encoder, rather than linking a second float
    // formatting implementation into the size-bounded browser Core.
    let Ok(displayed_total) = serde_json::to_string(&f64::from(quote.displayed_total)) else {
        return "{\"version\":1,\"known\":false,\"quote\":null}".into();
    };
    let displayed_total = displayed_total.strip_suffix(".0").unwrap_or(&displayed_total);
    format!("{{\"version\":1,\"known\":true,\"quote\":{{\"repairPrice\":{},\"displayedTotal\":{},\"totalPrice\":{},\"affordable\":{}}}}}",
        quote.repair_price, displayed_total, quote.total_price,
        repair_is_affordable(quote, gold))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abi_exact_quote_and_float_affordability_are_shared() {
        assert_eq!(npc_repair_quote_abi_version(), 1);
        assert_eq!(npc_repair_quote(41, 41, 77, 77, 1000, 1000, 1, 500, 1000, &[5], false, 1.0, false, 188),
            "{\"version\":1,\"known\":true,\"quote\":{\"repairPrice\":188,\"displayedTotal\":188,\"totalPrice\":188,\"affordable\":true}}");
        let below = npc_repair_quote(41, 41, 77, 77, 1000, 1000, 1, 500, 1000, &[5], false, 0.1, false, 18);
        assert!(below.contains("\"totalPrice\":18"));
        assert!(below.contains("\"affordable\":false"));
        assert!(npc_repair_quote(41, 41, 77, 77, 1000, 1000, 1, 500, 1000, &[5], false, 0.1, false, 19)
            .contains("\"affordable\":true"));
    }
    #[test]
    fn unknown_metadata_or_identity_has_no_price_or_affordance() {
        let unknown = "{\"version\":1,\"known\":false,\"quote\":null}";
        assert_eq!(npc_repair_quote(41, 42, 77, 77, 1000, 1000, 1, 500, 1000, &[], false, 1.0, false, 999), unknown);
        assert_eq!(npc_repair_quote(41, 41, 77, 78, 1000, 1000, 1, 500, 1000, &[], false, 1.0, false, 999), unknown);
        assert_eq!(npc_repair_quote(41, 41, 77, 77, 1000, 0, 1, 0, 0, &[], false, 1.0, false, 999), unknown);
    }
    #[test]
    fn json_float_encoding_keeps_the_exact_shared_f32_quote() {
        for rate in [0.0, 0.00001, 0.1, 1.0, 3.1415927, 10000.0] {
            let quote = repair_quote(Some(NpcRepairSource {
                live_unique_id: Some(41), source_unique_id: 41,
                template_item_index: 77, source_item_index: 77,
                template_price: 1000, template_durability: 1000,
                live_count: 1, live_current_dura: Some(500), live_max_dura: Some(1000),
                added_stat_values: &[5], rental: false,
            }), rate, false).unwrap();
            let raw = npc_repair_quote(41, 41, 77, 77, 1000, 1000, 1, 500, 1000,
                &[5], false, rate, false, 19);
            let document: serde_json::Value = serde_json::from_str(&raw).unwrap();
            assert_eq!(document["quote"]["displayedTotal"].as_f64().unwrap().to_bits(),
                f64::from(quote.displayed_total).to_bits());
            assert_eq!(document["quote"]["repairPrice"].as_u64(), Some(u64::from(quote.repair_price)));
            assert_eq!(document["quote"]["totalPrice"].as_u64(), Some(u64::from(quote.total_price)));
            assert_eq!(document["quote"]["affordable"].as_bool(), Some(repair_is_affordable(quote, 19)));
        }
    }
}
