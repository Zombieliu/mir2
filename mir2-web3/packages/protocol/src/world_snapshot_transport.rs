//! Negotiated, lossless worldSnapshot compression. Catalogs retain their own
//! magic and validator; identities, receipts and arbitrary messages are excluded.
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::catalog_transport::{
    decode_catalog_batch, encode_catalog_batch, CatalogTransportError, CATALOG_GZIP_MAGIC,
    MAX_CATALOG_DECODED_BYTES, MAX_CATALOG_WIRE_BYTES,
};

pub const WORLD_SNAPSHOT_GZIP_CAPABILITY: &str = "serverWorldSnapshotGzipV1";
pub const WORLD_SNAPSHOT_GZIP_MAGIC: [u8; 8] = *b"M2WSPGZ1";
pub const MAX_WORLD_SNAPSHOT_TEXT_BYTES: usize = MAX_CATALOG_DECODED_BYTES - 4;
pub const MAX_WORLD_SNAPSHOT_WIRE_BYTES: usize = MAX_CATALOG_WIRE_BYTES;

#[derive(Deserialize)]
enum SnapshotKind {
    #[serde(rename = "worldSnapshot")]
    WorldSnapshot,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotEnvelope {
    #[serde(rename = "type")]
    _kind: SnapshotKind,
    #[serde(rename = "payload")]
    _payload: Map<String, Value>,
}

fn validate(text: &str) -> Result<(), CatalogTransportError> {
    if text.len() > MAX_WORLD_SNAPSHOT_TEXT_BYTES {
        return Err(CatalogTransportError::DecodedTooLarge);
    }
    // Parse only to validate. Return the original bytes, preserving numeric
    // spelling, whitespace, field order and all authorized snapshot fields.
    serde_json::from_str::<SnapshotEnvelope>(text)
        .map(|_| ())
        .map_err(|_| CatalogTransportError::InvalidEntryLength)
}

pub fn encode_world_snapshot(text: &str) -> Result<Vec<u8>, CatalogTransportError> {
    validate(text)?;
    let mut bytes = encode_catalog_batch(&[text])?;
    bytes[..8].copy_from_slice(&WORLD_SNAPSHOT_GZIP_MAGIC);
    Ok(bytes)
}

pub fn decode_world_snapshot(bytes: &[u8]) -> Result<String, CatalogTransportError> {
    if bytes.len() > MAX_WORLD_SNAPSHOT_WIRE_BYTES {
        return Err(CatalogTransportError::WireTooLarge);
    }
    if bytes.len() < 16 {
        return Err(CatalogTransportError::InvalidHeader);
    }
    if bytes[..8] != WORLD_SNAPSHOT_GZIP_MAGIC {
        return Err(CatalogTransportError::InvalidMagic);
    }
    if u32::from_le_bytes(bytes[12..16].try_into().unwrap()) != 1 {
        return Err(CatalogTransportError::InvalidCount);
    }
    let mut framed = bytes.to_vec();
    framed[..8].copy_from_slice(&CATALOG_GZIP_MAGIC);
    let mut texts = decode_catalog_batch(&framed)?;
    let text = texts.pop().ok_or(CatalogTransportError::InvalidCount)?;
    validate(&text)?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_snapshot_text_and_catalog_domain_remain_separate() {
        let text = " \n{\"type\":\"worldSnapshot\",\"payload\":{\"future\":18446744073709551615,\"name\":\"阿洛\",\"hp\":18}}\t";
        let wire = encode_world_snapshot(text).unwrap();
        assert_eq!(decode_world_snapshot(&wire).unwrap(), text);
        assert_eq!(
            decode_catalog_batch(&wire),
            Err(CatalogTransportError::InvalidMagic)
        );
        assert_eq!(
            decode_world_snapshot(&encode_catalog_batch(&[text]).unwrap()),
            Err(CatalogTransportError::InvalidMagic)
        );
    }

    #[test]
    fn arbitrary_identity_receipt_and_nonobject_payload_cannot_enter_snapshot_domain() {
        for text in [
            r#"{"type":"identitySession","token":"private"}"#,
            r#"{"type":"npcPurchaseOwner","payload":{}}"#,
            r#"{"type":"worldSnapshot","payload":[]}"#,
            r#"{"type":"worldSnapshot","payload":{},"token":"private"}"#,
            r#"{"type":"worldSnapshot","type":"worldSnapshot","payload":{}}"#,
        ] {
            assert!(encode_world_snapshot(text).is_err());
            let mut forged = encode_catalog_batch(&[text]).unwrap();
            forged[..8].copy_from_slice(&WORLD_SNAPSHOT_GZIP_MAGIC);
            assert!(decode_world_snapshot(&forged).is_err());
        }
    }

    #[test]
    fn truncation_crc_trailing_members_and_count_are_checked_before_delivery() {
        let good = encode_world_snapshot(r#"{"type":"worldSnapshot","payload":{}}"#).unwrap();
        let mut crc = good.clone();
        let n = crc.len();
        crc[n - 8] ^= 1;
        let mut count = good.clone();
        count[12..16].copy_from_slice(&2u32.to_le_bytes());
        let mut trailing = good.clone();
        trailing.push(0);
        for wire in [
            good[..good.len() - 1].to_vec(),
            crc,
            count,
            trailing,
            vec![0; MAX_WORLD_SNAPSHOT_WIRE_BYTES + 1],
        ] {
            assert!(decode_world_snapshot(&wire).is_err());
        }
    }

    #[test]
    fn snapshot_size_limits_and_large_lossless_compression_do_not_drop_fields() {
        let text = serde_json::json!({"type":"worldSnapshot","payload":{"entities":
            (0..400).map(|i| serde_json::json!({"objectId":i,"name":"比奇守卫","hp":100,"x":288,"y":616})).collect::<Vec<_>>()}}).to_string();
        let wire = encode_world_snapshot(&text).unwrap();
        assert!(wire.len() < text.len() / 4);
        assert_eq!(decode_world_snapshot(&wire).unwrap(), text);
        let too_big = serde_json::json!({"type":"worldSnapshot","payload":{"text":"x".repeat(MAX_WORLD_SNAPSHOT_TEXT_BYTES)}}).to_string();
        assert_eq!(
            encode_world_snapshot(&too_big),
            Err(CatalogTransportError::DecodedTooLarge)
        );
    }
}
