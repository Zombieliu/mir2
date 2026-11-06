use std::collections::{HashMap, VecDeque};
use std::fmt;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer};
use sha2::{Digest, Sha256};

pub(crate) const NATIVE_RESUME_PROTOCOL: &str = "nativeResumeV1";
pub(crate) const RESUME_CREDENTIAL_TTL_MS: u64 = 30_000;
pub(crate) const RESUME_CREDENTIAL_ROTATION_MS: u64 = 10_000;
const MAX_FAMILY_GENERATIONS: usize = 2;
const CREDENTIAL_BYTES: usize = 32;
const ENCODED_CREDENTIAL_BYTES: usize = 43;
const NONCE_BYTES: usize = 32;
const FAMILY_ID_BYTES: usize = 16;

type CredentialHash = [u8; 32];

/// Process-local authorization revisions captured when a resume credential is
/// issued. They are not client-controlled and let the gateway invalidate an
/// already-issued credential atomically when an identity revocation crosses
/// the resume commit fence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ResumeAuthRevision {
    pub(crate) account: u64,
    pub(crate) identity_session: u64,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct ResumeFamilyId([u8; FAMILY_ID_BYTES]);

impl fmt::Debug for ResumeFamilyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ResumeFamilyId(<redacted>)")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ResumeConnectionNonce([u8; NONCE_BYTES]);

impl ResumeConnectionNonce {
    pub(crate) fn generate() -> Self {
        let mut bytes = [0_u8; NONCE_BYTES];
        OsRng.fill_bytes(&mut bytes);
        Self(bytes)
    }
}

impl fmt::Debug for ResumeConnectionNonce {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ResumeConnectionNonce(<redacted>)")
    }
}

pub(crate) struct ResumeCredential(String);

impl ResumeCredential {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    fn parse(value: &str) -> Result<Self, &'static str> {
        if value.len() != ENCODED_CREDENTIAL_BYTES {
            return Err("resume credential must contain exactly 43 ASCII characters");
        }
        if !value.is_ascii()
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err("resume credential must be unpadded base64url ASCII");
        }
        let decoded = URL_SAFE_NO_PAD
            .decode(value.as_bytes())
            .map_err(|_| "resume credential must be valid unpadded base64url")?;
        if decoded.len() != CREDENTIAL_BYTES {
            return Err("resume credential must decode to exactly 32 bytes");
        }
        Ok(Self(value.to_string()))
    }
}

impl<'de> Deserialize<'de> for ResumeCredential {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ResumeCredentialVisitor;

        impl Visitor<'_> for ResumeCredentialVisitor {
            type Value = ResumeCredential;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a 43-character unpadded base64url resume credential")
            }

            fn visit_borrowed_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                ResumeCredential::parse(value).map_err(E::custom)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                ResumeCredential::parse(value).map_err(E::custom)
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                ResumeCredential::parse(&value).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(ResumeCredentialVisitor)
    }
}

impl fmt::Debug for ResumeCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ResumeCredential(<redacted>)")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResumeBinding {
    pub(crate) family_id: ResumeFamilyId,
    pub(crate) account_id: String,
    pub(crate) character_index: i32,
    pub(crate) gateway_session_id: String,
    pub(crate) identity_session_id: String,
    pub(crate) identity_expires_at_ms: u64,
    pub(crate) auth_revision: ResumeAuthRevision,
    pub(crate) source_connection_nonce: ResumeConnectionNonce,
    pub(crate) generation: u64,
    pub(crate) issued_at_ms: u64,
    pub(crate) expires_at_ms: u64,
}

pub(crate) struct IssuedResumeCredential {
    pub(crate) credential: ResumeCredential,
    pub(crate) binding: ResumeBinding,
}

impl fmt::Debug for IssuedResumeCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IssuedResumeCredential")
            .field("credential", &"<redacted>")
            .field("binding", &self.binding)
            .finish()
    }
}

#[derive(Clone)]
pub(crate) struct ResumeIssueContext<'a> {
    pub(crate) account_id: &'a str,
    pub(crate) character_index: i32,
    pub(crate) gateway_session_id: &'a str,
    pub(crate) identity_session_id: &'a str,
    pub(crate) identity_expires_at_ms: u64,
    pub(crate) source_connection_nonce: &'a ResumeConnectionNonce,
}

#[derive(Default)]
pub(crate) struct ResumeCredentialRegistry {
    records: HashMap<CredentialHash, ResumeBinding>,
    families: HashMap<ResumeFamilyId, VecDeque<CredentialHash>>,
}

impl fmt::Debug for ResumeCredentialRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResumeCredentialRegistry")
            .field("record_count", &self.records.len())
            .field("family_count", &self.families.len())
            .finish()
    }
}

impl ResumeCredentialRegistry {
    pub(crate) fn issue(
        &mut self,
        current_family: Option<&ResumeFamilyId>,
        context: ResumeIssueContext<'_>,
        now_ms: u64,
        minimum_generation: u64,
        auth_revision: ResumeAuthRevision,
    ) -> IssuedResumeCredential {
        self.purge_expired(now_ms);
        let (family_id, generation) = current_family
            .and_then(|family_id| {
                self.latest_binding(family_id).and_then(|binding| {
                    (binding_matches_issue_context(binding, &context)
                        && binding.auth_revision == auth_revision)
                        .then(|| {
                            (
                                family_id.clone(),
                                binding.generation.saturating_add(1).max(minimum_generation),
                            )
                        })
                })
            })
            .unwrap_or_else(|| (generate_family_id(), minimum_generation.max(1)));

        let credential = generate_credential();
        let hash = credential_hash(credential.as_str())
            .expect("a generated resume credential must always be well formed");
        let binding = ResumeBinding {
            family_id: family_id.clone(),
            account_id: context.account_id.to_string(),
            character_index: context.character_index,
            gateway_session_id: context.gateway_session_id.to_string(),
            identity_session_id: context.identity_session_id.to_string(),
            identity_expires_at_ms: context.identity_expires_at_ms,
            auth_revision,
            source_connection_nonce: context.source_connection_nonce.clone(),
            generation,
            issued_at_ms: now_ms,
            expires_at_ms: now_ms.saturating_add(RESUME_CREDENTIAL_TTL_MS),
        };
        self.records.insert(hash, binding.clone());
        let family = self.families.entry(family_id).or_default();
        family.push_back(hash);
        while family.len() > MAX_FAMILY_GENERATIONS {
            if let Some(stale_hash) = family.pop_front() {
                self.records.remove(&stale_hash);
            }
        }
        IssuedResumeCredential {
            credential,
            binding,
        }
    }

    pub(crate) fn binding_for_credential(
        &mut self,
        credential: &str,
        now_ms: u64,
    ) -> Option<ResumeBinding> {
        self.purge_expired(now_ms);
        let hash = credential_hash(credential)?;
        self.records.get(&hash).cloned()
    }

    pub(crate) fn consume_matching(
        &mut self,
        credential: &str,
        expected: &ResumeBinding,
        now_ms: u64,
    ) -> Option<ResumeBinding> {
        self.purge_expired(now_ms);
        let hash = credential_hash(credential)?;
        let binding = self.records.get(&hash)?.clone();
        if &binding != expected || binding.expires_at_ms <= now_ms {
            return None;
        }
        self.revoke_family(&binding.family_id);
        Some(binding)
    }

    pub(crate) fn contains_binding(&mut self, expected: &ResumeBinding, now_ms: u64) -> bool {
        self.purge_expired(now_ms);
        self.records.values().any(|binding| binding == expected)
    }

    pub(crate) fn revoke_family(&mut self, family_id: &ResumeFamilyId) {
        let Some(hashes) = self.families.remove(family_id) else {
            return;
        };
        for hash in hashes {
            self.records.remove(&hash);
        }
    }

    pub(crate) fn purge_expired(&mut self, now_ms: u64) {
        let expired_families = self
            .families
            .iter()
            .filter_map(|(family_id, hashes)| {
                let has_live_record = hashes.iter().any(|hash| {
                    self.records
                        .get(hash)
                        .is_some_and(|binding| binding.expires_at_ms > now_ms)
                });
                (!has_live_record).then(|| family_id.clone())
            })
            .collect::<Vec<_>>();
        for family_id in expired_families {
            self.revoke_family(&family_id);
        }
        for hashes in self.families.values_mut() {
            hashes.retain(|hash| {
                self.records
                    .get(hash)
                    .is_some_and(|binding| binding.expires_at_ms > now_ms)
            });
        }
        self.records
            .retain(|_, binding| binding.expires_at_ms > now_ms);
    }

    fn latest_binding(&self, family_id: &ResumeFamilyId) -> Option<&ResumeBinding> {
        self.families
            .get(family_id)?
            .back()
            .and_then(|hash| self.records.get(hash))
    }

    #[cfg(test)]
    pub(crate) fn record_count(&self) -> usize {
        self.records.len()
    }

    #[cfg(test)]
    pub(crate) fn family_generation_count(&self, family_id: &ResumeFamilyId) -> usize {
        self.families.get(family_id).map_or(0, VecDeque::len)
    }
}

fn binding_matches_issue_context(
    binding: &ResumeBinding,
    context: &ResumeIssueContext<'_>,
) -> bool {
    binding.account_id == context.account_id
        && binding.character_index == context.character_index
        && binding.gateway_session_id == context.gateway_session_id
        && binding.identity_session_id == context.identity_session_id
        && binding.identity_expires_at_ms == context.identity_expires_at_ms
        && &binding.source_connection_nonce == context.source_connection_nonce
}

fn generate_credential() -> ResumeCredential {
    let mut bytes = [0_u8; CREDENTIAL_BYTES];
    OsRng.fill_bytes(&mut bytes);
    ResumeCredential::parse(&URL_SAFE_NO_PAD.encode(bytes))
        .expect("a generated resume credential must always be well formed")
}

fn generate_family_id() -> ResumeFamilyId {
    let mut bytes = [0_u8; FAMILY_ID_BYTES];
    OsRng.fill_bytes(&mut bytes);
    ResumeFamilyId(bytes)
}

fn credential_hash(credential: &str) -> Option<CredentialHash> {
    let credential = ResumeCredential::parse(credential).ok()?;
    let decoded = URL_SAFE_NO_PAD.decode(credential.as_str()).ok()?;
    Some(Sha256::digest(decoded).into())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::thread;

    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;

    use super::{
        ResumeConnectionNonce, ResumeCredentialRegistry, ResumeIssueContext,
        RESUME_CREDENTIAL_TTL_MS,
    };

    fn issue_context<'a>(nonce: &'a ResumeConnectionNonce) -> ResumeIssueContext<'a> {
        ResumeIssueContext {
            account_id: "account-a",
            character_index: 7,
            gateway_session_id: "gateway-session-a",
            identity_session_id: "identity-session-a",
            identity_expires_at_ms: 90_000,
            source_connection_nonce: nonce,
        }
    }

    #[test]
    fn generated_credential_is_32_random_bytes_and_registry_is_hash_only() {
        let nonce = ResumeConnectionNonce::generate();
        let mut registry = ResumeCredentialRegistry::default();
        let issued = registry.issue(
            None,
            issue_context(&nonce),
            1_000,
            1,
            super::ResumeAuthRevision::default(),
        );
        let plaintext = issued.credential.as_str().to_string();
        assert_eq!(URL_SAFE_NO_PAD.decode(&plaintext).unwrap().len(), 32);
        assert_eq!(registry.record_count(), 1);
        assert!(!format!("{registry:?}").contains(&plaintext));
        assert!(!format!("{issued:?}").contains(&plaintext));
    }

    #[test]
    fn binding_and_rotation_retain_only_latest_two_generations() {
        let nonce = ResumeConnectionNonce::generate();
        let mut registry = ResumeCredentialRegistry::default();
        let first = registry.issue(
            None,
            issue_context(&nonce),
            1_000,
            1,
            super::ResumeAuthRevision::default(),
        );
        let family = first.binding.family_id.clone();
        let second = registry.issue(
            Some(&family),
            issue_context(&nonce),
            11_000,
            1,
            super::ResumeAuthRevision::default(),
        );
        let third = registry.issue(
            Some(&family),
            issue_context(&nonce),
            21_000,
            1,
            super::ResumeAuthRevision::default(),
        );
        assert_eq!(second.binding.generation, 2);
        assert_eq!(third.binding.generation, 3);
        assert_eq!(registry.family_generation_count(&family), 2);
        assert!(registry
            .binding_for_credential(first.credential.as_str(), 21_000)
            .is_none());
        let bound = registry
            .binding_for_credential(third.credential.as_str(), 21_000)
            .expect("latest credential should remain live");
        assert_eq!(bound.account_id, "account-a");
        assert_eq!(bound.character_index, 7);
        assert_eq!(bound.gateway_session_id, "gateway-session-a");
        assert_eq!(bound.identity_session_id, "identity-session-a");
        assert_eq!(bound.source_connection_nonce, nonce);
    }

    #[test]
    fn malformed_and_expired_credentials_fail_closed() {
        let nonce = ResumeConnectionNonce::generate();
        let mut registry = ResumeCredentialRegistry::default();
        let issued = registry.issue(
            None,
            issue_context(&nonce),
            1_000,
            1,
            super::ResumeAuthRevision::default(),
        );
        assert!(registry
            .binding_for_credential("not-base64!", 1_000)
            .is_none());
        assert!(registry
            .binding_for_credential(issued.credential.as_str(), 1_000 + RESUME_CREDENTIAL_TTL_MS)
            .is_none());
        assert_eq!(registry.record_count(), 0);
    }

    #[test]
    fn deserialize_rejects_wrong_length_alphabet_padding_and_decoded_size() {
        for malformed in [
            "A".repeat(42),
            "A".repeat(44),
            format!("{}=", "A".repeat(42)),
            format!("{}+", "A".repeat(42)),
            format!("{}é", "A".repeat(42)),
        ] {
            assert!(
                serde_json::from_value::<super::ResumeCredential>(serde_json::json!(malformed))
                    .is_err()
            );
        }

        let valid = URL_SAFE_NO_PAD.encode([7_u8; 32]);
        let credential =
            serde_json::from_value::<super::ResumeCredential>(serde_json::json!(valid))
                .expect("exact 32-byte unpadded base64url credential should deserialize");
        assert_eq!(
            URL_SAFE_NO_PAD.decode(credential.as_str()).unwrap().len(),
            32
        );
    }

    #[test]
    fn consuming_one_generation_revokes_siblings_and_replay() {
        let nonce = ResumeConnectionNonce::generate();
        let mut registry = ResumeCredentialRegistry::default();
        let first = registry.issue(
            None,
            issue_context(&nonce),
            1_000,
            1,
            super::ResumeAuthRevision::default(),
        );
        let family = first.binding.family_id.clone();
        let second = registry.issue(
            Some(&family),
            issue_context(&nonce),
            11_000,
            1,
            super::ResumeAuthRevision::default(),
        );
        let expected = registry
            .binding_for_credential(first.credential.as_str(), 11_000)
            .unwrap();
        assert!(registry
            .consume_matching(first.credential.as_str(), &expected, 11_000)
            .is_some());
        assert!(registry
            .binding_for_credential(second.credential.as_str(), 11_000)
            .is_none());
        assert!(registry
            .consume_matching(first.credential.as_str(), &expected, 11_000)
            .is_none());
        assert_eq!(registry.record_count(), 0);
    }

    #[test]
    fn concurrent_consumers_have_exactly_one_success() {
        let nonce = ResumeConnectionNonce::generate();
        let mut registry = ResumeCredentialRegistry::default();
        let issued = registry.issue(
            None,
            issue_context(&nonce),
            1_000,
            1,
            super::ResumeAuthRevision::default(),
        );
        let credential = issued.credential.as_str().to_string();
        let expected = issued.binding;
        let registry = Arc::new(Mutex::new(registry));
        let successes = (0..8)
            .map(|_| {
                let registry = Arc::clone(&registry);
                let credential = credential.clone();
                let expected = expected.clone();
                thread::spawn(move || {
                    registry
                        .lock()
                        .unwrap()
                        .consume_matching(&credential, &expected, 1_000)
                        .is_some()
                })
            })
            .map(|thread| thread.join().unwrap() as usize)
            .sum::<usize>();
        assert_eq!(successes, 1);
    }
}
