use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const DEFAULT_MODEL: &str = "gpt-6-luna";
pub const MAX_KEY_BYTES: usize = 2048;
pub const MAX_CONFIGURATION_BYTES: usize = 8192;
pub const FIXED_ERROR: &str = "Local OpenAI settings are unavailable.";
pub const TARGET_PREFIX: &str = "NumeronAlo/OpenAI/v1/";

pub type Result<T> = std::result::Result<T, &'static str>;

// Deliberately has no Debug or Serialize implementation.
pub struct Secret(pub Vec<u8>);
impl Drop for Secret {
    fn drop(&mut self) {
        for byte in &mut self.0 {
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
    }
}

fn deserialize_secret<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Secret, D::Error> {
    struct SecretVisitor;
    impl<'de> serde::de::Visitor<'de> for SecretVisitor {
        type Value = Secret;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a credential string")
        }
        fn visit_str<E: serde::de::Error>(
            self,
            value: &str,
        ) -> std::result::Result<Self::Value, E> {
            Ok(Secret(value.as_bytes().to_vec()))
        }
        fn visit_string<E: serde::de::Error>(
            self,
            value: String,
        ) -> std::result::Result<Self::Value, E> {
            Ok(Secret(value.into_bytes()))
        }
    }
    deserializer.deserialize_string(SecretVisitor)
}

// Intentionally Deserialize-only. Neither this request nor its secret may be
// logged, formatted with Debug, or serialized as a response.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigureInput {
    pub model: String,
    #[serde(deserialize_with = "deserialize_secret")]
    pub api_key: Secret,
}

pub fn read_configuration(mut reader: impl Read) -> Result<ConfigureInput> {
    // One fixed allocation avoids reallocating/copying partial raw credentials.
    // Keep the full buffer length until Drop so even unused capacity is wiped.
    let mut raw = Secret(vec![0u8; MAX_CONFIGURATION_BYTES + 1]);
    let mut length = 0;
    while length < raw.0.len() {
        match reader.read(&mut raw.0[length..]) {
            Ok(0) => break,
            Ok(count) => length += count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(FIXED_ERROR),
        }
    }
    if length > MAX_CONFIGURATION_BYTES {
        return Err(FIXED_ERROR);
    }
    let input: ConfigureInput =
        serde_json::from_slice(&raw.0[..length]).map_err(|_| FIXED_ERROR)?;
    if !valid_model(&input.model) || !valid_key(&input.api_key.0) {
        return Err(FIXED_ERROR);
    }
    Ok(input)
}

pub fn valid_key(value: &[u8]) -> bool {
    (16..=MAX_KEY_BYTES).contains(&value.len())
        && value.starts_with(b"sk-")
        && value
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

pub fn valid_model(value: &str) -> bool {
    (1..=100).contains(&value.len())
        && !value.to_ascii_lowercase().starts_with("sk-")
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

pub fn normalized_bundle_path(value: &str) -> String {
    let value = value.replace('/', "\\");
    let value = if value.starts_with("\\\\?\\UNC\\") {
        format!("\\\\{}", &value[8..])
    } else {
        value.trim_start_matches("\\\\?\\").to_owned()
    };
    value.trim_end_matches('\\').to_lowercase()
}

pub fn credential_target(root: &Path) -> Result<String> {
    let canonical = root.canonicalize().map_err(|_| FIXED_ERROR)?;
    let value = canonical.to_str().ok_or(FIXED_ERROR)?;
    let digest = Sha256::digest(normalized_bundle_path(value).as_bytes());
    Ok(format!("{TARGET_PREFIX}{digest:x}"))
}

pub fn metadata_path(root: &Path) -> PathBuf {
    root.join("apps/ai-player/.data/local-playtest/openai.local.json")
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Metadata {
    pub schema_version: u32,
    pub provider: String,
    pub enabled: bool,
    pub model: String,
    pub credential_target: String,
    pub max_calls_per_hour: u32,
}
impl Metadata {
    pub fn new(model: String, target: String) -> Result<Self> {
        if !valid_model(&model) {
            return Err(FIXED_ERROR);
        }
        Ok(Self {
            schema_version: 1,
            provider: "openai".into(),
            enabled: true,
            model,
            credential_target: target,
            max_calls_per_hour: 20,
        })
    }
    pub fn validate(&self, target: &str) -> Result<()> {
        if self.schema_version != 1
            || self.provider != "openai"
            || !self.enabled
            || !valid_model(&self.model)
            || self.credential_target != target
            || self.max_calls_per_hour != 20
        {
            return Err(FIXED_ERROR);
        }
        Ok(())
    }
}

pub fn read_metadata(root: &Path, target: &str) -> Result<Metadata> {
    let file = std::fs::File::open(metadata_path(root)).map_err(|_| FIXED_ERROR)?;
    let mut bytes = Vec::new();
    file.take(8193)
        .read_to_end(&mut bytes)
        .map_err(|_| FIXED_ERROR)?;
    if bytes.len() > 8192 {
        return Err(FIXED_ERROR);
    }
    let metadata: Metadata = serde_json::from_slice(&bytes).map_err(|_| FIXED_ERROR)?;
    metadata.validate(target)?;
    Ok(metadata)
}

// A complete temporary file is prepared before touching credentials. A failed
// rename keeps the previous metadata; the Windows caller restores its old key.
pub fn prepare_metadata(root: &Path, metadata: &Metadata) -> Result<PathBuf> {
    let destination = metadata_path(root);
    let parent = destination.parent().ok_or(FIXED_ERROR)?;
    std::fs::create_dir_all(parent).map_err(|_| FIXED_ERROR)?;
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| FIXED_ERROR)?
        .as_nanos();
    let temporary = parent.join(format!(
        "openai.local.json.{}.{unique}.tmp",
        std::process::id()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|_| FIXED_ERROR)?;
    let result = (|| {
        let encoded = serde_json::to_vec_pretty(metadata).map_err(|_| FIXED_ERROR)?;
        file.write_all(&encoded).map_err(|_| FIXED_ERROR)?;
        file.sync_all().map_err(|_| FIXED_ERROR)
    })();
    drop(file);
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result.map(|_| temporary)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_and_key_validation_never_accepts_tokens_or_payloads() {
        assert!(valid_model(DEFAULT_MODEL));
        assert!(valid_key(b"sk-synthetic-test-only-123456"));
        for value in [
            "",
            "gpt x",
            "https://example",
            "gpt\nother",
            "sk-synthetic-test-only-123456",
        ] {
            assert!(!valid_model(value));
        }
        for value in [
            "",
            "vck_synthetic_123456789",
            "eyJhbGciOi.fake.token",
            "sk-short",
            "sk-synthetic\n123456",
        ] {
            assert!(!valid_key(value.as_bytes()));
        }
    }
    #[test]
    fn mixed_path_case_and_extended_prefix_share_one_target_input() {
        assert_eq!(
            normalized_bundle_path("F:/Bundle/game/.."),
            "f:\\bundle\\game\\.."
        );
        assert_eq!(
            normalized_bundle_path("F:/Bundle/"),
            normalized_bundle_path("\\\\?\\f:\\BUNDLE")
        );
        assert_eq!(
            normalized_bundle_path("\\\\?\\UNC\\host\\share\\bundle"),
            normalized_bundle_path("\\\\host\\share\\bundle")
        );
        assert_ne!(
            normalized_bundle_path("F:/one"),
            normalized_bundle_path("F:/two")
        );
    }
    #[test]
    fn metadata_is_strict_and_has_no_secret_field() {
        let meta = Metadata::new(DEFAULT_MODEL.into(), "bound-target".into()).unwrap();
        meta.validate("bound-target").unwrap();
        assert!(meta.validate("another-target").is_err());
        let encoded = serde_json::to_string(&meta).unwrap();
        assert!(!encoded.contains("apiKey"));
        let extra = encoded.replacen('{', "{\"apiKey\":\"synthetic\",", 1);
        assert!(serde_json::from_str::<Metadata>(&extra).is_err());
    }

    #[test]
    fn private_configuration_parser_accepts_only_the_two_valid_fields() {
        let encoded =
            br#"{"model":"gpt-6-luna","apiKey":"sk-synthetic-test-only-never-an-api-key"}"#;
        let input = read_configuration(&encoded[..]).unwrap();
        assert_eq!(input.model, DEFAULT_MODEL);
        assert!(input.api_key.0 == b"sk-synthetic-test-only-never-an-api-key");
        let escaped =
            br#"{"model":"gpt-6-luna","apiKey":"\u0073k-synthetic-test-only-never-an-api-key"}"#;
        let input = read_configuration(&escaped[..]).unwrap();
        assert!(valid_key(&input.api_key.0));
    }

    #[test]
    fn private_configuration_parser_rejects_overrides_duplicates_missing_fields_and_bad_values() {
        for encoded in [
            "",
            "{}",
            "[]",
            "null",
            "not-json",
            r#"{"model":"gpt-6-luna"}"#,
            r#"{"apiKey":"sk-synthetic-test-only-123456"}"#,
            r#"{"model":"gpt-6-luna","apiKey":""}"#,
            r#"{"model":"gpt-6-luna","apiKey":null}"#,
            r#"{"model":"gpt-6-luna\n","apiKey":"sk-synthetic-test-only-123456"}"#,
            r#"{"model":"sk-synthetic-test-only-123456","apiKey":"sk-synthetic-test-only-123456"}"#,
            r#"{"model":"gpt-6-luna","apiKey":"sk-synthetic\ntest-only-123456"}"#,
            r#"{"model":"gpt-6-luna","apiKey":"sk-synthetic-test-only-123456","target":"other"}"#,
            r#"{"model":"gpt-6-luna","apiKey":"sk-synthetic-test-only-123456","root":"other"}"#,
            r#"{"model":"gpt-6-luna","apiKey":"sk-synthetic-test-only-123456","path":"other"}"#,
            r#"{"model":"gpt-6-luna","model":"other","apiKey":"sk-synthetic-test-only-123456"}"#,
            r#"{"model":"gpt-6-luna","apiKey":"sk-synthetic-test-only-123456"}{}"#,
        ] {
            assert!(read_configuration(encoded.as_bytes()).is_err());
        }
        assert!(read_configuration(&[0xffu8][..]).is_err());
    }

    #[test]
    fn private_configuration_parser_bounds_reads_and_accepts_exactly_the_limit() {
        let mut encoded =
            br#"{"model":"gpt-6-luna","apiKey":"sk-synthetic-test-only-123456"}"#.to_vec();
        encoded.resize(MAX_CONFIGURATION_BYTES, b' ');
        assert!(read_configuration(encoded.as_slice()).is_ok());
        encoded.push(b' ');
        assert!(read_configuration(encoded.as_slice()).is_err());
        struct Infinite {
            read: usize,
        }
        impl Read for Infinite {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                buffer.fill(b'x');
                self.read += buffer.len();
                Ok(buffer.len())
            }
        }
        let mut infinite = Infinite { read: 0 };
        assert!(read_configuration(&mut infinite).is_err());
        assert_eq!(infinite.read, MAX_CONFIGURATION_BYTES + 1);
    }

    #[test]
    fn private_configuration_read_errors_are_fixed_and_never_include_input() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other(
                    "synthetic-private-input-never-report",
                ))
            }
        }
        assert_eq!(read_configuration(Broken).err(), Some(FIXED_ERROR));
    }
}
