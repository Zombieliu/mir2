//! Read-only ABI for the shared cash-shop preview policy. No gameplay commands.
use mir2_client_core::cash_preview::{preview_layers, preview_turn, CashPreviewInput};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn cash_preview_abi_version() -> u32 { 1 }

#[wasm_bindgen]
pub fn cash_preview_layers(
    item_type: u8, shape: i32, required_gender: u8, armour_shape: i32,
    female: bool, direction: u8, elapsed_ms: u64,
) -> String {
    if !(1..=8).contains(&direction) || !(0..=32767).contains(&shape) || !(-32768..=32767).contains(&armour_shape)
        || !matches!(item_type, 1 | 2 | 19 | 37)
    {
        return format!("{{\"version\":1,\"known\":false,\"direction\":{direction},\"layers\":[]}}");
    }
    let layers = preview_layers(CashPreviewInput {
        item_type, shape, required_gender, armour_shape, female, direction, elapsed_ms,
    });
    let mut json = format!("{{\"version\":1,\"known\":true,\"direction\":{direction},\"layers\":[");
    for (index, layer) in layers.iter().enumerate() {
        if index > 0 { json.push(','); }
        // Library names are produced exclusively by the bounded shared policy.
        json.push_str(&format!("{{\"library\":\"{}\",\"frame\":{}}}", layer.library, layer.frame));
    }
    json.push_str("]}");
    json
}

#[wasm_bindgen]
pub fn cash_preview_turn(direction: u8, right: bool) -> u8 {
    if !(1..=8).contains(&direction) { return 0; }
    preview_turn(direction, right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_is_exact_shared_layers_and_local_turn_only() {
        assert_eq!(cash_preview_abi_version(), 1);
        assert_eq!(cash_preview_layers(1, 9, 1, 0, false, 2, 150),
            "{\"version\":1,\"known\":true,\"direction\":2,\"layers\":[{\"library\":\"CArmour/00\",\"frame\":39},{\"library\":\"CWeapon/09\",\"frame\":39}]}");
        assert_eq!(cash_preview_turn(8, true), 1);
        assert_eq!(cash_preview_turn(1, false), 8);
    }

    #[test]
    fn unknown_or_invalid_inputs_do_not_create_preview_layers() {
        assert_eq!(cash_preview_layers(1, -1, 1, 0, false, 6, 0),
            "{\"version\":1,\"known\":false,\"direction\":6,\"layers\":[]}");
        assert!(cash_preview_layers(0, 0, 0, 0, false, 1, 0).contains("\"known\":false"));
        assert!(cash_preview_layers(1, 32768, 0, 0, false, 1, 0).contains("\"known\":false"));
        assert!(cash_preview_layers(1, 65536, 0, 0, false, 1, 0).contains("\"known\":false"));
        assert!(cash_preview_layers(1, 9, 0, 32768, false, 1, 0).contains("\"known\":false"));
        assert_eq!(cash_preview_turn(0, true), 0);
        assert_eq!(cash_preview_turn(9, false), 0);
    }
}
