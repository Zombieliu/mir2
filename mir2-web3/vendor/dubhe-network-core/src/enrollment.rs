use std::collections::BTreeSet;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::{
    CapacityChallengeResponse, HomeTunnelPlacement, NodeCapacityCertificate, NodeSigningIdentity,
    node_id_from_public_key, validate_ed25519_public_key, verify_ed25519_signature,
};

pub const HOME_ENROLLMENT_CHALLENGE_SCHEMA: &str = "obelisk.home-enrollment-challenge.v1";
pub const HOME_ENROLLMENT_BUNDLE_SCHEMA: &str = "obelisk.home-enrollment-bundle.v1";
pub const HOME_ENROLLMENT_SIGNATURE_ALGORITHM: &str = "ed25519-zip215";

const CHALLENGE_DOMAIN: &[u8] = b"obelisk.home-enrollment.challenge.v1\0";
const REQUEST_DOMAIN: &[u8] = b"obelisk.home-enrollment.request.v1\0";
const BUNDLE_DOMAIN: &[u8] = b"obelisk.home-enrollment.bundle.v1\0";
const MAX_CHALLENGE_TTL_MS: u64 = 10 * 60 * 1_000;
const MAX_BUNDLE_TTL_MS: u64 = 30 * 24 * 60 * 60 * 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeEnrollmentChallengePayload {
    pub schema: String,
    pub challenge_id: String,
    pub node_id: String,
    pub public_key: String,
    pub nonce: String,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedHomeEnrollmentChallenge {
    pub payload: HomeEnrollmentChallengePayload,
    pub issuer_public_key: String,
    pub signature_algorithm: String,
    pub signature: String,
}

impl SignedHomeEnrollmentChallenge {
    pub fn issue(
        challenge_id: impl Into<String>,
        nonce: impl Into<String>,
        node_id: impl Into<String>,
        public_key: impl Into<String>,
        issued_at_ms: u64,
        ttl_ms: u64,
        issuer: &NodeSigningIdentity,
    ) -> Result<Self, String> {
        if ttl_ms == 0 || ttl_ms > MAX_CHALLENGE_TTL_MS {
            return Err(format!(
                "Home enrollment challenge TTL must be within 1..={MAX_CHALLENGE_TTL_MS}ms"
            ));
        }
        let payload = HomeEnrollmentChallengePayload {
            schema: HOME_ENROLLMENT_CHALLENGE_SCHEMA.to_string(),
            challenge_id: challenge_id.into(),
            node_id: node_id.into(),
            public_key: public_key.into(),
            nonce: nonce.into(),
            issued_at_ms,
            expires_at_ms: issued_at_ms.saturating_add(ttl_ms),
        };
        validate_challenge_payload(&payload, issued_at_ms)?;
        let signature = issuer.sign(&domain_json(CHALLENGE_DOMAIN, &payload)?);
        Ok(Self {
            payload,
            issuer_public_key: issuer.public_key().to_string(),
            signature_algorithm: HOME_ENROLLMENT_SIGNATURE_ALGORITHM.to_string(),
            signature,
        })
    }

    pub fn verify(&self, trusted_issuer_public_key: &str, now_ms: u64) -> Result<(), String> {
        if self.issuer_public_key != trusted_issuer_public_key {
            return Err("Home enrollment challenge issuer is not trusted".to_string());
        }
        if self.signature_algorithm != HOME_ENROLLMENT_SIGNATURE_ALGORITHM {
            return Err("unsupported Home enrollment challenge signature algorithm".to_string());
        }
        validate_challenge_payload(&self.payload, now_ms)?;
        verify_ed25519_signature(
            trusted_issuer_public_key,
            &domain_json(CHALLENGE_DOMAIN, &self.payload)?,
            &self.signature,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeEnrollmentRequest {
    pub challenge: SignedHomeEnrollmentChallenge,
    pub node_signature_algorithm: String,
    pub node_signature: String,
}

impl HomeEnrollmentRequest {
    pub fn sign(
        challenge: SignedHomeEnrollmentChallenge,
        node: &NodeSigningIdentity,
        trusted_issuer_public_key: &str,
        now_ms: u64,
    ) -> Result<Self, String> {
        challenge.verify(trusted_issuer_public_key, now_ms)?;
        if challenge.payload.node_id != node.node_id()
            || challenge.payload.public_key != node.public_key()
        {
            return Err("Home enrollment challenge does not target this node identity".to_string());
        }
        let node_signature = node.sign(&request_signing_bytes(&challenge)?);
        Ok(Self {
            challenge,
            node_signature_algorithm: HOME_ENROLLMENT_SIGNATURE_ALGORITHM.to_string(),
            node_signature,
        })
    }

    pub fn verify(&self, trusted_issuer_public_key: &str, now_ms: u64) -> Result<(), String> {
        self.challenge.verify(trusted_issuer_public_key, now_ms)?;
        if self.node_signature_algorithm != HOME_ENROLLMENT_SIGNATURE_ALGORITHM {
            return Err("unsupported Home enrollment request signature algorithm".to_string());
        }
        verify_ed25519_signature(
            &self.challenge.payload.public_key,
            &request_signing_bytes(&self.challenge)?,
            &self.node_signature,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeEnrollmentRelayConfig {
    pub relay_id: String,
    pub address: String,
    pub server_name: String,
    pub relay_public_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeEnrollmentResourcePolicy {
    pub max_sessions: usize,
    pub max_sessions_per_zone: usize,
    pub max_zones: usize,
    pub cpu_limit_percent: u8,
    pub reserved_memory_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeEnrollmentRelayCredential {
    pub ca_certificate_der: String,
    pub certificate_chain_der: Vec<String>,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeCapacityCertificationRequest {
    pub enrollment: SignedHomeEnrollmentBundle,
    pub response: CapacityChallengeResponse,
    pub certificate_signing_request_der: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeEnrollmentBundlePayload {
    pub schema: String,
    pub enrollment_id: String,
    pub node_id: String,
    pub public_key: String,
    pub key_generation: u64,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub relay: HomeEnrollmentRelayConfig,
    pub control_issuer_public_key: String,
    pub telemetry_url: String,
    pub resource_policy: HomeEnrollmentResourcePolicy,
    pub allowed_games: Vec<String>,
    pub allowed_zones: Vec<String>,
    #[serde(default)]
    pub capacity_issuer_public_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity_certificate: Option<NodeCapacityCertificate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<HomeTunnelPlacement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_credential: Option<HomeEnrollmentRelayCredential>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedHomeEnrollmentBundle {
    pub payload: HomeEnrollmentBundlePayload,
    pub issuer_public_key: String,
    pub signature_algorithm: String,
    pub signature: String,
}

impl SignedHomeEnrollmentBundle {
    pub fn issue(
        mut payload: HomeEnrollmentBundlePayload,
        issuer: &NodeSigningIdentity,
    ) -> Result<Self, String> {
        payload.schema = HOME_ENROLLMENT_BUNDLE_SCHEMA.to_string();
        if payload.capacity_issuer_public_key.trim().is_empty() {
            payload.capacity_issuer_public_key = issuer.public_key().to_string();
        }
        validate_bundle_payload(&payload, payload.issued_at_ms)?;
        let signature = issuer.sign(&domain_json(BUNDLE_DOMAIN, &payload)?);
        Ok(Self {
            payload,
            issuer_public_key: issuer.public_key().to_string(),
            signature_algorithm: HOME_ENROLLMENT_SIGNATURE_ALGORITHM.to_string(),
            signature,
        })
    }

    pub fn verify(
        &self,
        trusted_issuer_public_key: &str,
        expected_node_id: &str,
        expected_public_key: &str,
        now_ms: u64,
    ) -> Result<(), String> {
        if self.issuer_public_key != trusted_issuer_public_key {
            return Err("Home enrollment bundle issuer is not trusted".to_string());
        }
        if self.signature_algorithm != HOME_ENROLLMENT_SIGNATURE_ALGORITHM {
            return Err("unsupported Home enrollment bundle signature algorithm".to_string());
        }
        validate_bundle_payload(&self.payload, now_ms)?;
        if self.payload.node_id != expected_node_id
            || self.payload.public_key != expected_public_key
        {
            return Err("Home enrollment bundle targets another node identity".to_string());
        }
        verify_ed25519_signature(
            trusted_issuer_public_key,
            &domain_json(BUNDLE_DOMAIN, &self.payload)?,
            &self.signature,
        )?;
        if let Some(certificate) = &self.payload.capacity_certificate {
            certificate.verify(&self.payload.capacity_issuer_public_key, now_ms)?;
            if certificate.node_id != self.payload.node_id
                || certificate.public_key != self.payload.public_key
                || certificate.key_generation != self.payload.key_generation
            {
                return Err(
                    "Home enrollment capacity certificate does not match the bundle identity"
                        .to_string(),
                );
            }
        }
        Ok(())
    }

    pub fn capacity_ready(&self) -> bool {
        self.payload.capacity_certificate.is_some()
    }

    pub fn relay_ready(&self) -> bool {
        self.payload.capacity_certificate.is_some()
            && self.payload.placement.is_some()
            && self.payload.relay_credential.is_some()
    }
}

fn validate_challenge_payload(
    payload: &HomeEnrollmentChallengePayload,
    now_ms: u64,
) -> Result<(), String> {
    if payload.schema != HOME_ENROLLMENT_CHALLENGE_SCHEMA {
        return Err("unsupported Home enrollment challenge schema".to_string());
    }
    if payload.challenge_id.trim().is_empty() || payload.nonce.len() < 32 {
        return Err("Home enrollment challenge ID and nonce are required".to_string());
    }
    if node_id_from_public_key(&payload.public_key)? != payload.node_id {
        return Err("Home enrollment challenge Node ID does not match its public key".to_string());
    }
    if payload.expires_at_ms <= payload.issued_at_ms
        || payload.expires_at_ms.saturating_sub(payload.issued_at_ms) > MAX_CHALLENGE_TTL_MS
    {
        return Err("Home enrollment challenge lifetime is invalid".to_string());
    }
    if now_ms < payload.issued_at_ms || now_ms > payload.expires_at_ms {
        return Err("Home enrollment challenge is not currently valid".to_string());
    }
    Ok(())
}

fn validate_bundle_payload(
    payload: &HomeEnrollmentBundlePayload,
    now_ms: u64,
) -> Result<(), String> {
    if payload.schema != HOME_ENROLLMENT_BUNDLE_SCHEMA {
        return Err("unsupported Home enrollment bundle schema".to_string());
    }
    if payload.enrollment_id.trim().is_empty() {
        return Err("Home enrollment ID is required".to_string());
    }
    if node_id_from_public_key(&payload.public_key)? != payload.node_id {
        return Err("Home enrollment bundle Node ID does not match its public key".to_string());
    }
    if payload.key_generation == 0 {
        return Err("Home enrollment key generation must be positive".to_string());
    }
    if payload.expires_at_ms <= payload.issued_at_ms
        || payload.expires_at_ms.saturating_sub(payload.issued_at_ms) > MAX_BUNDLE_TTL_MS
    {
        return Err("Home enrollment bundle lifetime is invalid".to_string());
    }
    if now_ms < payload.issued_at_ms || now_ms > payload.expires_at_ms {
        return Err("Home enrollment bundle is not currently valid".to_string());
    }
    if payload.relay.relay_id.trim().is_empty()
        || payload.relay.address.trim().is_empty()
        || payload.relay.server_name.trim().is_empty()
    {
        return Err("Home enrollment Relay configuration is incomplete".to_string());
    }
    validate_ed25519_public_key(&payload.relay.relay_public_key)
        .map_err(|error| format!("invalid Home Relay public key: {error}"))?;
    validate_ed25519_public_key(&payload.control_issuer_public_key)
        .map_err(|error| format!("invalid Home control issuer public key: {error}"))?;
    validate_ed25519_public_key(&payload.capacity_issuer_public_key)
        .map_err(|error| format!("invalid Home capacity issuer public key: {error}"))?;
    let telemetry = Url::parse(&payload.telemetry_url)
        .map_err(|error| format!("invalid Home telemetry URL: {error}"))?;
    let local_development = telemetry.scheme() == "http"
        && matches!(
            telemetry.host_str(),
            Some("127.0.0.1" | "::1" | "localhost")
        );
    if telemetry.scheme() != "https" && !local_development {
        return Err("Home telemetry URL must use HTTPS outside loopback development".to_string());
    }
    let policy = &payload.resource_policy;
    if policy.max_sessions == 0
        || policy.max_sessions_per_zone == 0
        || policy.max_sessions_per_zone > policy.max_sessions
        || policy.max_zones == 0
        || !(10..=95).contains(&policy.cpu_limit_percent)
        || policy.reserved_memory_bytes < 512 * 1024 * 1024
    {
        return Err("Home enrollment resource policy is invalid".to_string());
    }
    let games = payload
        .allowed_games
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>();
    if games.is_empty() || games.len() != payload.allowed_games.len() {
        return Err("Home enrollment allowed games must be non-empty and unique".to_string());
    }
    match (
        &payload.capacity_certificate,
        &payload.placement,
        &payload.relay_credential,
    ) {
        (None, None, None) => {}
        (Some(certificate), Some(placement), Some(credential)) => {
            certificate.verify(&payload.capacity_issuer_public_key, now_ms)?;
            if certificate.node_id != payload.node_id
                || certificate.public_key != payload.public_key
                || certificate.key_generation != payload.key_generation
            {
                return Err(
                    "Home enrollment capacity certificate does not match the bundle identity"
                        .to_string(),
                );
            }
            placement.verify(
                &payload.control_issuer_public_key,
                &payload.relay.relay_id,
                certificate,
                now_ms,
            )?;
            validate_relay_credential(credential, now_ms)?;
        }
        _ => {
            return Err(
                "Home enrollment production admission requires certificate, placement, and Relay credential together"
                    .to_string(),
            );
        }
    }
    Ok(())
}

fn validate_relay_credential(
    credential: &HomeEnrollmentRelayCredential,
    now_ms: u64,
) -> Result<(), String> {
    if credential.expires_at_ms <= credential.issued_at_ms
        || now_ms < credential.issued_at_ms
        || now_ms > credential.expires_at_ms
    {
        return Err("Home Relay credential is outside its active window".to_string());
    }
    let ca = URL_SAFE_NO_PAD
        .decode(&credential.ca_certificate_der)
        .map_err(|_| "Home Relay CA certificate must be URL-safe base64".to_string())?;
    if ca.is_empty() || ca.len() > 1024 * 1024 {
        return Err("Home Relay CA certificate size is invalid".to_string());
    }
    if credential.certificate_chain_der.is_empty() || credential.certificate_chain_der.len() > 8 {
        return Err("Home Relay certificate chain length is invalid".to_string());
    }
    for certificate in &credential.certificate_chain_der {
        let certificate = URL_SAFE_NO_PAD
            .decode(certificate)
            .map_err(|_| "Home Relay certificate must be URL-safe base64".to_string())?;
        if certificate.is_empty() || certificate.len() > 1024 * 1024 {
            return Err("Home Relay certificate size is invalid".to_string());
        }
    }
    Ok(())
}

fn request_signing_bytes(challenge: &SignedHomeEnrollmentChallenge) -> Result<Vec<u8>, String> {
    domain_json(REQUEST_DOMAIN, challenge)
}

fn domain_json<T: Serialize>(domain: &[u8], value: &T) -> Result<Vec<u8>, String> {
    let mut bytes = domain.to_vec();
    bytes.extend(
        serde_json::to_vec(value)
            .map_err(|error| format!("encode Home enrollment signing payload: {error}"))?,
    );
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(node: &NodeSigningIdentity, now_ms: u64) -> HomeEnrollmentBundlePayload {
        HomeEnrollmentBundlePayload {
            schema: String::new(),
            enrollment_id: "enrollment-1".to_string(),
            node_id: node.node_id().to_string(),
            public_key: node.public_key().to_string(),
            key_generation: 1,
            issued_at_ms: now_ms,
            expires_at_ms: now_ms + 60_000,
            relay: HomeEnrollmentRelayConfig {
                relay_id: "relay-hk-1".to_string(),
                address: "relay.example.com:443".to_string(),
                server_name: "relay.example.com".to_string(),
                relay_public_key: NodeSigningIdentity::from_seed([3; 32])
                    .public_key()
                    .to_string(),
            },
            control_issuer_public_key: NodeSigningIdentity::from_seed([4; 32])
                .public_key()
                .to_string(),
            telemetry_url: "https://telemetry.example.com/v1/home-nodes/report".to_string(),
            resource_policy: HomeEnrollmentResourcePolicy {
                max_sessions: 32,
                max_sessions_per_zone: 16,
                max_zones: 2,
                cpu_limit_percent: 60,
                reserved_memory_bytes: 2 * 1024 * 1024 * 1024,
            },
            allowed_games: vec!["mir2".to_string()],
            allowed_zones: Vec::new(),
            capacity_issuer_public_key: String::new(),
            capacity_certificate: None,
            placement: None,
            relay_credential: None,
        }
    }

    #[test]
    fn enrollment_challenge_request_and_bundle_round_trip() {
        let issuer = NodeSigningIdentity::from_seed([1; 32]);
        let node = NodeSigningIdentity::from_seed([2; 32]);
        let challenge = SignedHomeEnrollmentChallenge::issue(
            "challenge-1",
            "ab".repeat(32),
            node.node_id(),
            node.public_key(),
            1_000,
            60_000,
            &issuer,
        )
        .unwrap();
        challenge.verify(issuer.public_key(), 2_000).unwrap();
        let request =
            HomeEnrollmentRequest::sign(challenge, &node, issuer.public_key(), 2_000).unwrap();
        request.verify(issuer.public_key(), 2_000).unwrap();
        let bundle = SignedHomeEnrollmentBundle::issue(payload(&node, 2_000), &issuer).unwrap();
        bundle
            .verify(
                issuer.public_key(),
                node.node_id(),
                node.public_key(),
                3_000,
            )
            .unwrap();
        assert!(!bundle.capacity_ready());
    }

    #[test]
    fn enrollment_rejects_retargeting_and_untrusted_issuers() {
        let issuer = NodeSigningIdentity::from_seed([11; 32]);
        let other_issuer = NodeSigningIdentity::from_seed([12; 32]);
        let node = NodeSigningIdentity::from_seed([13; 32]);
        let other_node = NodeSigningIdentity::from_seed([14; 32]);
        let challenge = SignedHomeEnrollmentChallenge::issue(
            "challenge-2",
            "cd".repeat(32),
            node.node_id(),
            node.public_key(),
            1_000,
            60_000,
            &issuer,
        )
        .unwrap();
        assert!(challenge.verify(other_issuer.public_key(), 2_000).is_err());
        assert!(
            HomeEnrollmentRequest::sign(challenge, &other_node, issuer.public_key(), 2_000)
                .is_err()
        );
    }

    #[test]
    fn enrollment_rejects_expired_and_insecure_remote_configuration() {
        let issuer = NodeSigningIdentity::from_seed([21; 32]);
        let node = NodeSigningIdentity::from_seed([22; 32]);
        let challenge = SignedHomeEnrollmentChallenge::issue(
            "challenge-3",
            "ef".repeat(32),
            node.node_id(),
            node.public_key(),
            1_000,
            1_000,
            &issuer,
        )
        .unwrap();
        assert!(challenge.verify(issuer.public_key(), 2_001).is_err());

        let mut insecure = payload(&node, 2_000);
        insecure.telemetry_url = "http://telemetry.example.com/report".to_string();
        assert!(SignedHomeEnrollmentBundle::issue(insecure, &issuer).is_err());
    }
}
