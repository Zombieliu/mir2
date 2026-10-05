//! Opt-in lossless framing for public catalog text envelopes. JSON semantics
//! belong to the caller; an entire decoded batch must be validated before use.

use std::io::{Read, Write};

use flate2::{bufread::GzDecoder, write::GzEncoder, Compression};

pub const CATALOG_GZIP_CAPABILITY: &str = "serverCatalogGzipV1";
pub const CATALOG_GZIP_MAGIC: [u8; 8] = *b"M2CATGZ1";
pub const MAX_CATALOG_DECODED_BYTES: usize = 128 * 1024;
pub const MAX_CATALOG_ENVELOPES: usize = 64;
pub const MAX_CATALOG_WIRE_BYTES: usize = 132 * 1024;
const HEADER_BYTES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogTransportError {
    WireTooLarge,
    InvalidHeader,
    InvalidMagic,
    InvalidCount,
    DecodedTooLarge,
    EmptyEnvelope,
    InvalidGzip,
    TrailingGzipData,
    DecodedLengthMismatch,
    InvalidEntryLength,
    InvalidUtf8,
    TrailingDecodedData,
}

impl std::fmt::Display for CatalogTransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::WireTooLarge => "catalog wire limit exceeded",
            Self::InvalidHeader => "invalid catalog header",
            Self::InvalidMagic => "invalid catalog magic",
            Self::InvalidCount => "invalid catalog envelope count",
            Self::DecodedTooLarge => "catalog decoded limit exceeded",
            Self::EmptyEnvelope => "empty catalog envelope",
            Self::InvalidGzip => "invalid catalog gzip member",
            Self::TrailingGzipData => "trailing catalog gzip data",
            Self::DecodedLengthMismatch => "catalog decoded length mismatch",
            Self::InvalidEntryLength => "invalid catalog entry length",
            Self::InvalidUtf8 => "invalid catalog UTF-8",
            Self::TrailingDecodedData => "trailing catalog decoded data",
        })
    }
}

impl std::error::Error for CatalogTransportError {}

pub fn encode_catalog_batch(texts: &[&str]) -> Result<Vec<u8>, CatalogTransportError> {
    if texts.is_empty() || texts.len() > MAX_CATALOG_ENVELOPES {
        return Err(CatalogTransportError::InvalidCount);
    }
    let mut decoded_len = 0_usize;
    for text in texts {
        if text.is_empty() {
            return Err(CatalogTransportError::EmptyEnvelope);
        }
        decoded_len = decoded_len
            .checked_add(4)
            .and_then(|size| size.checked_add(text.len()))
            .filter(|size| *size <= MAX_CATALOG_DECODED_BYTES)
            .ok_or(CatalogTransportError::DecodedTooLarge)?;
    }
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    for text in texts {
        encoder
            .write_all(&(text.len() as u32).to_le_bytes())
            .and_then(|()| encoder.write_all(text.as_bytes()))
            .map_err(|_| CatalogTransportError::InvalidGzip)?;
    }
    let compressed = encoder
        .finish()
        .map_err(|_| CatalogTransportError::InvalidGzip)?;
    if HEADER_BYTES + compressed.len() > MAX_CATALOG_WIRE_BYTES {
        return Err(CatalogTransportError::WireTooLarge);
    }
    let mut result = Vec::with_capacity(HEADER_BYTES + compressed.len());
    result.extend_from_slice(&CATALOG_GZIP_MAGIC);
    result.extend_from_slice(&(decoded_len as u32).to_le_bytes());
    result.extend_from_slice(&(texts.len() as u32).to_le_bytes());
    result.extend_from_slice(&compressed);
    Ok(result)
}

pub fn decode_catalog_batch(bytes: &[u8]) -> Result<Vec<String>, CatalogTransportError> {
    if bytes.len() > MAX_CATALOG_WIRE_BYTES {
        return Err(CatalogTransportError::WireTooLarge);
    }
    if bytes.len() < HEADER_BYTES {
        return Err(CatalogTransportError::InvalidHeader);
    }
    if bytes[..8] != CATALOG_GZIP_MAGIC {
        return Err(CatalogTransportError::InvalidMagic);
    }
    let decoded_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let count = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if count == 0 || count > MAX_CATALOG_ENVELOPES {
        return Err(CatalogTransportError::InvalidCount);
    }
    if decoded_len == 0 || decoded_len > MAX_CATALOG_DECODED_BYTES {
        return Err(CatalogTransportError::DecodedTooLarge);
    }
    // bufread deliberately leaves the bytes after the first gzip member in
    // the underlying slice. read::GzDecoder may consume them into a buffer.
    let mut decoder = GzDecoder::new(&bytes[HEADER_BYTES..]);
    let mut decoded = Vec::with_capacity(decoded_len.min(4096));
    decoder
        .by_ref()
        .take(decoded_len as u64 + 1)
        .read_to_end(&mut decoded)
        .map_err(|_| CatalogTransportError::InvalidGzip)?;
    if decoded.len() != decoded_len {
        return Err(CatalogTransportError::DecodedLengthMismatch);
    }
    if !decoder.get_ref().is_empty() {
        return Err(CatalogTransportError::TrailingGzipData);
    }
    let mut cursor = 0_usize;
    let mut texts = Vec::with_capacity(count);
    for _ in 0..count {
        let length_bytes = decoded
            .get(cursor..cursor.saturating_add(4))
            .ok_or(CatalogTransportError::InvalidEntryLength)?;
        let length = u32::from_le_bytes(length_bytes.try_into().unwrap()) as usize;
        cursor += 4;
        if length == 0 {
            return Err(CatalogTransportError::EmptyEnvelope);
        }
        let end = cursor
            .checked_add(length)
            .ok_or(CatalogTransportError::InvalidEntryLength)?;
        let text = decoded
            .get(cursor..end)
            .ok_or(CatalogTransportError::InvalidEntryLength)?;
        texts.push(
            std::str::from_utf8(text)
                .map_err(|_| CatalogTransportError::InvalidUtf8)?
                .to_owned(),
        );
        cursor = end;
    }
    if cursor != decoded.len() {
        return Err(CatalogTransportError::TrailingDecodedData);
    }
    Ok(texts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_frame(body: &[u8], count: u32, declared: u32) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(body).unwrap();
        let mut frame = CATALOG_GZIP_MAGIC.to_vec();
        frame.extend_from_slice(&declared.to_le_bytes());
        frame.extend_from_slice(&count.to_le_bytes());
        frame.extend_from_slice(&encoder.finish().unwrap());
        frame
    }

    #[test]
    fn catalog_round_trip_preserves_exact_utf8_order_and_boundaries() {
        let texts = [
            " {\"packet\":\"NewItemInfo\"}\n",
            "骷髅试炼 — 药品",
            "ASCII\0UTF8",
        ];
        assert_eq!(
            decode_catalog_batch(&encode_catalog_batch(&texts).unwrap()).unwrap(),
            texts
        );
        let boundary = "x".repeat(MAX_CATALOG_DECODED_BYTES - 4);
        assert_eq!(
            decode_catalog_batch(&encode_catalog_batch(&[&boundary]).unwrap()).unwrap(),
            [boundary]
        );
        let max_count = vec!["x"; MAX_CATALOG_ENVELOPES];
        assert_eq!(
            decode_catalog_batch(&encode_catalog_batch(&max_count).unwrap()).unwrap(),
            max_count
        );
    }

    #[test]
    fn catalog_encoder_rejects_empty_count_entries_and_decoded_overflow() {
        assert_eq!(
            encode_catalog_batch(&[]),
            Err(CatalogTransportError::InvalidCount)
        );
        assert_eq!(
            encode_catalog_batch(&[""]),
            Err(CatalogTransportError::EmptyEnvelope)
        );
        assert_eq!(
            encode_catalog_batch(&vec!["x"; 65]),
            Err(CatalogTransportError::InvalidCount)
        );
        assert_eq!(
            encode_catalog_batch(&[&"x".repeat(MAX_CATALOG_DECODED_BYTES - 3)]),
            Err(CatalogTransportError::DecodedTooLarge)
        );
    }

    #[test]
    fn catalog_node_generated_utf8_fixture_and_rust_reencoding() {
        let hex = "4d32434154475a319c000000020000001f8b080000000000000a0b656060a8562aa92c4855b2522a484cce4e2d51d28131ac94fc52cb3d4b52733df3d2f2c1c29539f989294a56d54a997929a9154a56e63a4a7989b920bd2f7ad73f9ddca810905f92999fa7545b6b4fd0e0c0d2d4e2129c261bd7d602007d6d409c9c000000";
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
            .collect();
        let texts = [
            r#"{"type":"packet","packet":"NewItemInfo","payload":{"index":7,"name":"药品 Potion"}}"#,
            r#"{"type":"packet","packet":"NewQuestInfo","payload":{"index":3}}"#,
        ];
        assert_eq!(decode_catalog_batch(&bytes).unwrap(), texts);
        let rust = encode_catalog_batch(&texts).unwrap();
        assert_eq!(decode_catalog_batch(&rust).unwrap(), texts);
        let rust_hex: String = rust.iter().map(|byte| format!("{byte:02x}")).collect();
        eprintln!("catalog_rust_fixture_hex={rust_hex}");
    }

    #[test]
    fn catalog_decoder_rejects_all_truncations_crc_multi_member_and_trailing_bytes() {
        let encoded = encode_catalog_batch(&["ASCII", "中文"]).unwrap();
        for cut in 0..encoded.len() {
            assert!(decode_catalog_batch(&encoded[..cut]).is_err(), "cut {cut}");
        }
        let mut crc = encoded.clone();
        let crc_at = crc.len() - 8;
        crc[crc_at] ^= 1;
        assert_eq!(
            decode_catalog_batch(&crc),
            Err(CatalogTransportError::InvalidGzip)
        );
        let mut appended = encoded.clone();
        appended.push(0);
        assert_eq!(
            decode_catalog_batch(&appended),
            Err(CatalogTransportError::TrailingGzipData)
        );
        let mut concatenated = encoded.clone();
        concatenated.extend_from_slice(&encoded[HEADER_BYTES..]);
        assert_eq!(
            decode_catalog_batch(&concatenated),
            Err(CatalogTransportError::TrailingGzipData)
        );
    }

    #[test]
    fn catalog_decoder_rejects_forged_headers_invalid_utf8_and_body_lengths() {
        assert_eq!(
            decode_catalog_batch(&vec![0; MAX_CATALOG_WIRE_BYTES + 1]),
            Err(CatalogTransportError::WireTooLarge)
        );
        let mut frame = encode_catalog_batch(&["x"]).unwrap();
        frame[0] ^= 1;
        assert_eq!(
            decode_catalog_batch(&frame),
            Err(CatalogTransportError::InvalidMagic)
        );
        frame[0] ^= 1;
        for count in [0_u32, 65, u32::MAX] {
            frame[12..16].copy_from_slice(&count.to_le_bytes());
            assert_eq!(
                decode_catalog_batch(&frame),
                Err(CatalogTransportError::InvalidCount)
            );
        }
        for (body, count, expected) in [
            (vec![0, 0, 0, 0], 1, CatalogTransportError::EmptyEnvelope),
            (vec![1, 0, 0, 0, 255], 1, CatalogTransportError::InvalidUtf8),
            (
                vec![2, 0, 0, 0, b'a'],
                1,
                CatalogTransportError::InvalidEntryLength,
            ),
            (
                vec![1, 0, 0, 0, b'a'],
                2,
                CatalogTransportError::InvalidEntryLength,
            ),
            (
                vec![1, 0, 0, 0, b'a', 0],
                1,
                CatalogTransportError::TrailingDecodedData,
            ),
        ] {
            assert_eq!(
                decode_catalog_batch(&raw_frame(&body, count, body.len() as u32)),
                Err(expected)
            );
        }
        let huge = vec![0; MAX_CATALOG_DECODED_BYTES * 8];
        assert_eq!(
            decode_catalog_batch(&raw_frame(&huge, 1, 5)),
            Err(CatalogTransportError::DecodedLengthMismatch)
        );
        assert_eq!(
            decode_catalog_batch(&raw_frame(&[0; 5], 1, u32::MAX)),
            Err(CatalogTransportError::DecodedTooLarge)
        );
    }
}
