//! Synchronous transport policy; no renderer or transaction-owner ABI.
//! Validate the complete compressed batch before returning original text frames.

use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
pub fn gateway_catalog_abi_version() -> u32 {
    1
}

#[wasm_bindgen]
pub fn decode_gateway_catalog(bytes: &[u8]) -> Result<String, String> {
    // Keep parser/decompressor diagnostics private. They must never quote a
    // remote envelope, identity or credential into the browser's error log.
    let invalid = || "Invalid bounded gateway catalog batch".to_owned();
    let texts = mir2_protocol::catalog_envelope::decode_validated_catalog_batch(bytes)
        .map_err(|_| invalid())?;
    serde_json::to_string(&texts).map_err(|_| invalid())
}

#[wasm_bindgen]
pub fn gateway_world_snapshot_abi_version() -> u32 { 1 }

#[wasm_bindgen]
pub fn decode_gateway_world_snapshot(bytes: &[u8]) -> Result<String, String> {
    mir2_protocol::world_snapshot_transport::decode_world_snapshot(bytes)
        .map_err(|_| "Invalid bounded gateway world snapshot".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_policy_has_its_own_abi_and_redacts_invalid_remote_data() {
        assert_eq!(gateway_catalog_abi_version(), 1);
        assert_eq!(
            decode_gateway_catalog(b"private-invalid-wire").unwrap_err(),
            "Invalid bounded gateway catalog batch"
        );
    }
}
