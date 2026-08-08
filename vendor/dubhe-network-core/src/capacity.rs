use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    NodeSigningIdentity, node_id_from_public_key, validate_ed25519_public_key,
    verify_ed25519_signature,
};

const CHALLENGE_RESPONSE_DOMAIN: &[u8] = b"obelisk.capacity-response.v2\0";
const CAPACITY_CERTIFICATE_DOMAIN: &[u8] = b"obelisk.capacity-certificate.v2\0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuiFinalityProof {
    pub network: String,
    pub package_id: String,
    pub transaction_digest: String,
    pub event_sequence: u64,
    pub checkpoint: u64,
}

impl SuiFinalityProof {
    pub fn validate(&self) -> Result<(), String> {
        if self.network != "testnet" && self.network != "mainnet" {
            return Err("guild node registration must come from testnet or mainnet".to_string());
        }
        validate_sui_object_id("Sui package id", &self.package_id)?;
        validate_component("Sui transaction digest", &self.transaction_digest)?;
        if self.checkpoint == 0 {
            return Err("Sui finality checkpoint must be positive".to_string());
        }
        Ok(())
    }

    pub fn idempotency_key(&self) -> String {
        format!(
            "sui:{}:{}:{}",
            self.network, self.transaction_digest, self.event_sequence
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GuildNodeStatus {
    Active,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalizedGuildNodeRegistration {
    pub node_id: String,
    pub operator_sui_address: String,
    pub public_key: String,
    pub endpoint: String,
    pub failure_domain: String,
    pub stake_mist: u64,
    pub max_sessions: usize,
    pub max_zones: usize,
    pub key_generation: u64,
    pub status: GuildNodeStatus,
    pub finality: SuiFinalityProof,
}

impl FinalizedGuildNodeRegistration {
    pub fn validate(&self) -> Result<(), String> {
        validate_component("guild node id", &self.node_id)?;
        validate_sui_object_id("guild operator Sui address", &self.operator_sui_address)?;
        validate_ed25519_public_key(&self.public_key)?;
        validate_component("guild node endpoint", &self.endpoint)?;
        validate_component("guild node failure domain", &self.failure_domain)?;
        if self.key_generation == 0 {
            return Err("guild node key generation must be positive".to_string());
        }
        if self.stake_mist == 0 {
            return Err("guild node registration stake must be positive".to_string());
        }
        if self.max_sessions == 0 || self.max_zones == 0 {
            return Err("guild node registered capacity must be positive".to_string());
        }
        if self.key_generation == 1 {
            let derived = node_id_from_public_key(&self.public_key)?;
            if derived != self.node_id {
                return Err(format!(
                    "initial guild node id {} does not match public key identity {derived}",
                    self.node_id
                ));
            }
        }
        self.finality.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityWorkload {
    pub concurrent_sessions: usize,
    pub max_sessions_per_zone: usize,
    pub zone_count: usize,
    pub command_count: u64,
    pub maximum_p95_latency_ms: u64,
    pub minimum_success_bps: u16,
}

impl CapacityWorkload {
    pub fn validate(&self) -> Result<(), String> {
        if self.concurrent_sessions == 0
            || self.max_sessions_per_zone == 0
            || self.zone_count == 0
            || self.command_count == 0
        {
            return Err("capacity workload dimensions must be positive".to_string());
        }
        if self.max_sessions_per_zone > self.concurrent_sessions {
            return Err(
                "capacity workload per-Zone sessions cannot exceed total sessions".to_string(),
            );
        }
        if self.concurrent_sessions > self.max_sessions_per_zone.saturating_mul(self.zone_count) {
            return Err(
                "capacity workload total sessions do not fit the declared per-Zone bound"
                    .to_string(),
            );
        }
        if self.maximum_p95_latency_ms == 0 {
            return Err("capacity workload latency bound must be positive".to_string());
        }
        if self.minimum_success_bps == 0 || self.minimum_success_bps > 10_000 {
            return Err(
                "capacity workload success target must be within 1..=10000 bps".to_string(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityChallenge {
    pub challenge_id: String,
    pub node_id: String,
    pub nonce: String,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub workload: CapacityWorkload,
}

impl CapacityChallenge {
    pub fn validate(&self, now_ms: u64) -> Result<(), String> {
        validate_component("capacity challenge id", &self.challenge_id)?;
        validate_component("capacity challenge node id", &self.node_id)?;
        let nonce = URL_SAFE_NO_PAD
            .decode(&self.nonce)
            .map_err(|_| "capacity challenge nonce must be URL-safe base64".to_string())?;
        if nonce.len() < 16 {
            return Err("capacity challenge nonce must contain at least 128 bits".to_string());
        }
        if self.expires_at_ms <= self.issued_at_ms
            || now_ms < self.issued_at_ms
            || now_ms > self.expires_at_ms
        {
            return Err(
                "capacity challenge is not active, expired, or has an invalid window".to_string(),
            );
        }
        self.workload.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityChallengeResponse {
    pub challenge: CapacityChallenge,
    pub public_key: String,
    pub key_generation: u64,
    pub completed_commands: u64,
    pub failed_commands: u64,
    pub p95_latency_ms: u64,
    pub transcript_commitment: String,
    pub observed_at_ms: u64,
    pub signature_algorithm: String,
    pub signature: String,
}

impl CapacityChallengeResponse {
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        challenge: CapacityChallenge,
        identity: &NodeSigningIdentity,
        key_generation: u64,
        completed_commands: u64,
        failed_commands: u64,
        p95_latency_ms: u64,
        transcript_commitment: String,
        observed_at_ms: u64,
    ) -> Result<Self, String> {
        let mut response = Self {
            challenge,
            public_key: identity.public_key().to_string(),
            key_generation,
            completed_commands,
            failed_commands,
            p95_latency_ms,
            transcript_commitment,
            observed_at_ms,
            signature_algorithm: "ed25519-zip215".to_string(),
            signature: String::new(),
        };
        response.signature = identity.sign(&response.signing_bytes()?);
        Ok(response)
    }

    pub fn verify(
        &self,
        registration: &FinalizedGuildNodeRegistration,
        now_ms: u64,
    ) -> Result<(), String> {
        self.verify_node_claim(now_ms)?;
        registration.validate()?;
        if registration.status != GuildNodeStatus::Active {
            return Err("capacity response node is not active on Sui".to_string());
        }
        if self.challenge.node_id != registration.node_id
            || self.public_key != registration.public_key
            || self.key_generation != registration.key_generation
        {
            return Err(
                "capacity response identity does not match finalized registration".to_string(),
            );
        }
        Ok(())
    }

    pub fn verify_node_claim(&self, now_ms: u64) -> Result<(), String> {
        self.challenge.validate(now_ms)?;
        if self.key_generation == 0 {
            return Err("capacity response key generation must be positive".to_string());
        }
        if node_id_from_public_key(&self.public_key)? != self.challenge.node_id {
            return Err("capacity response public key does not match challenged node".to_string());
        }
        if self.signature_algorithm != "ed25519-zip215" {
            return Err("capacity response must use Ed25519".to_string());
        }
        if self.observed_at_ms < self.challenge.issued_at_ms
            || self.observed_at_ms > self.challenge.expires_at_ms
        {
            return Err(
                "capacity response observation is outside the challenge window".to_string(),
            );
        }
        if self.completed_commands.saturating_add(self.failed_commands)
            != self.challenge.workload.command_count
        {
            return Err(
                "capacity response command accounting does not match challenge".to_string(),
            );
        }
        let success_bps =
            self.completed_commands.saturating_mul(10_000) / self.challenge.workload.command_count;
        if success_bps < u64::from(self.challenge.workload.minimum_success_bps) {
            return Err("capacity response is below the success target".to_string());
        }
        if self.p95_latency_ms > self.challenge.workload.maximum_p95_latency_ms {
            return Err("capacity response exceeds the p95 latency target".to_string());
        }
        validate_sha256(
            "capacity transcript commitment",
            &self.transcript_commitment,
        )?;
        verify_ed25519_signature(&self.public_key, &self.signing_bytes()?, &self.signature)
    }

    fn signing_bytes(&self) -> Result<Vec<u8>, String> {
        let mut unsigned = self.clone();
        unsigned.signature.clear();
        domain_json(CHALLENGE_RESPONSE_DOMAIN, &unsigned)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeCapacityCertificate {
    pub certificate_id: String,
    pub node_id: String,
    pub public_key: String,
    pub key_generation: u64,
    pub challenge_id: String,
    pub max_sessions: usize,
    pub max_sessions_per_zone: usize,
    pub max_zones: usize,
    pub measured_p95_latency_ms: u64,
    pub success_bps: u16,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub finalized_control_height: u64,
    pub issuer_public_key: String,
    pub signature_algorithm: String,
    pub signature: String,
}

impl NodeCapacityCertificate {
    pub fn issue(
        response: &CapacityChallengeResponse,
        registration: &FinalizedGuildNodeRegistration,
        issuer: &NodeSigningIdentity,
        issued_at_ms: u64,
        validity_ms: u64,
        finalized_control_height: u64,
    ) -> Result<Self, String> {
        response.verify(registration, issued_at_ms)?;
        if validity_ms == 0 || finalized_control_height == 0 {
            return Err(
                "capacity certificate validity and control height must be positive".to_string(),
            );
        }
        let success_bps = response.completed_commands.saturating_mul(10_000)
            / response.challenge.workload.command_count;
        let mut certificate = Self {
            certificate_id: String::new(),
            node_id: registration.node_id.clone(),
            public_key: registration.public_key.clone(),
            key_generation: registration.key_generation,
            challenge_id: response.challenge.challenge_id.clone(),
            max_sessions: response
                .challenge
                .workload
                .concurrent_sessions
                .min(registration.max_sessions),
            max_sessions_per_zone: response
                .challenge
                .workload
                .max_sessions_per_zone
                .min(registration.max_sessions),
            max_zones: response
                .challenge
                .workload
                .zone_count
                .min(registration.max_zones),
            measured_p95_latency_ms: response.p95_latency_ms,
            success_bps: success_bps as u16,
            issued_at_ms,
            expires_at_ms: issued_at_ms.saturating_add(validity_ms),
            finalized_control_height,
            issuer_public_key: issuer.public_key().to_string(),
            signature_algorithm: "ed25519-zip215".to_string(),
            signature: String::new(),
        };
        certificate.certificate_id = certificate.compute_id()?;
        certificate.signature = issuer.sign(&certificate.signing_bytes()?);
        Ok(certificate)
    }

    pub fn verify(&self, trusted_issuer: &str, now_ms: u64) -> Result<(), String> {
        if self.signature_algorithm != "ed25519-zip215" {
            return Err("capacity certificate must use Ed25519".to_string());
        }
        if self.issuer_public_key != trusted_issuer {
            return Err("capacity certificate issuer is not trusted".to_string());
        }
        if self.key_generation == 0
            || self.max_sessions == 0
            || self.max_sessions_per_zone == 0
            || self.max_zones == 0
            || self.finalized_control_height == 0
        {
            return Err("capacity certificate contains an invalid zero field".to_string());
        }
        if self.max_sessions_per_zone > self.max_sessions {
            return Err("capacity certificate per-Zone sessions exceed total sessions".to_string());
        }
        if self.expires_at_ms <= self.issued_at_ms || now_ms > self.expires_at_ms {
            return Err("capacity certificate is expired or has an invalid window".to_string());
        }
        if self.success_bps == 0 || self.success_bps > 10_000 {
            return Err("capacity certificate success rate is invalid".to_string());
        }
        if self.compute_id()? != self.certificate_id {
            return Err("capacity certificate id mismatch".to_string());
        }
        verify_ed25519_signature(
            &self.issuer_public_key,
            &self.signing_bytes()?,
            &self.signature,
        )
    }

    fn compute_id(&self) -> Result<String, String> {
        let mut unsigned = self.clone();
        unsigned.certificate_id.clear();
        unsigned.signature.clear();
        let bytes = domain_json(CAPACITY_CERTIFICATE_DOMAIN, &unsigned)?;
        let mut hash = Sha256::new();
        hash.update(bytes);
        Ok(hex_digest(&hash.finalize()))
    }

    fn signing_bytes(&self) -> Result<Vec<u8>, String> {
        let mut unsigned = self.clone();
        unsigned.signature.clear();
        domain_json(CAPACITY_CERTIFICATE_DOMAIN, &unsigned)
    }
}

fn domain_json(domain: &[u8], value: &impl Serialize) -> Result<Vec<u8>, String> {
    let payload = serde_json::to_vec(value)
        .map_err(|error| format!("guild node canonical serialization failed: {error}"))?;
    let mut bytes = Vec::with_capacity(domain.len() + payload.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

fn validate_sui_object_id(label: &str, value: &str) -> Result<(), String> {
    let Some(hex) = value.strip_prefix("0x") else {
        return Err(format!("{label} must start with 0x"));
    };
    if hex.is_empty() || hex.len() > 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("invalid {label}"));
    }
    Ok(())
}

fn validate_component(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        return Err(format!("invalid {label}"));
    }
    Ok(())
}

fn validate_sha256(label: &str, value: &str) -> Result<(), String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{label} must be a SHA-256 hex digest"));
    }
    Ok(())
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registration(identity: &NodeSigningIdentity) -> FinalizedGuildNodeRegistration {
        FinalizedGuildNodeRegistration {
            node_id: identity.node_id().to_string(),
            operator_sui_address: format!("0x{}", "11".repeat(32)),
            public_key: identity.public_key().to_string(),
            endpoint: "node-a:7020".to_string(),
            failure_domain: "test-az-a".to_string(),
            stake_mist: 1_000_000,
            max_sessions: 256,
            max_zones: 16,
            key_generation: 1,
            status: GuildNodeStatus::Active,
            finality: SuiFinalityProof {
                network: "testnet".to_string(),
                package_id: format!("0x{}", "22".repeat(32)),
                transaction_digest: "testnet-transaction".to_string(),
                event_sequence: 0,
                checkpoint: 42,
            },
        }
    }

    fn challenge(node_id: &str) -> CapacityChallenge {
        CapacityChallenge {
            challenge_id: "challenge-1".to_string(),
            node_id: node_id.to_string(),
            nonce: URL_SAFE_NO_PAD.encode([3_u8; 32]),
            issued_at_ms: 1_000,
            expires_at_ms: 10_000,
            workload: CapacityWorkload {
                concurrent_sessions: 128,
                max_sessions_per_zone: 16,
                zone_count: 8,
                command_count: 1_000,
                maximum_p95_latency_ms: 50,
                minimum_success_bps: 9_900,
            },
        }
    }

    #[test]
    fn signed_challenge_produces_a_verifiable_expiring_certificate() {
        let node = NodeSigningIdentity::from_seed([7; 32]);
        let issuer = NodeSigningIdentity::from_seed([9; 32]);
        let registration = registration(&node);
        let response = CapacityChallengeResponse::sign(
            challenge(node.node_id()),
            &node,
            1,
            995,
            5,
            41,
            "ab".repeat(32),
            2_000,
        )
        .unwrap();
        response.verify(&registration, 2_500).unwrap();
        let certificate =
            NodeCapacityCertificate::issue(&response, &registration, &issuer, 2_500, 60_000, 7)
                .unwrap();
        certificate.verify(issuer.public_key(), 3_000).unwrap();
        assert_eq!(certificate.max_sessions, 128);
        assert_eq!(certificate.max_sessions_per_zone, 16);
        assert_eq!(certificate.success_bps, 9_950);
        assert!(certificate.verify(issuer.public_key(), 70_000).is_err());
    }

    #[test]
    fn capacity_response_rejects_tampering() {
        let node = NodeSigningIdentity::from_seed([7; 32]);
        let registration = registration(&node);
        let mut response = CapacityChallengeResponse::sign(
            challenge(node.node_id()),
            &node,
            1,
            1_000,
            0,
            40,
            "cd".repeat(32),
            2_000,
        )
        .unwrap();
        response.p95_latency_ms = 60;
        assert!(response.verify(&registration, 2_500).is_err());
    }

    #[test]
    fn capacity_challenge_rejects_use_before_issuance() {
        let node = NodeSigningIdentity::from_seed([7; 32]);
        assert!(challenge(node.node_id()).validate(999).is_err());
        challenge(node.node_id()).validate(1_000).unwrap();
    }

    #[test]
    fn capacity_workload_rejects_an_impossible_per_zone_distribution() {
        let workload = CapacityWorkload {
            concurrent_sessions: 100,
            max_sessions_per_zone: 10,
            zone_count: 8,
            command_count: 1_000,
            maximum_p95_latency_ms: 50,
            minimum_success_bps: 9_900,
        };
        assert!(workload.validate().is_err());

        CapacityWorkload {
            zone_count: 10,
            ..workload
        }
        .validate()
        .expect("ten Zones can contain one hundred sessions at ten per Zone");
    }
}
