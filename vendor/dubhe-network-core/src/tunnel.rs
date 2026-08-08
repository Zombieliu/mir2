use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};

use crate::{
    NodeCapacityCertificate, NodeSigningIdentity, node_id_from_public_key, verify_ed25519_signature,
};

const CHALLENGE_DOMAIN: &[u8] = b"obelisk.home-tunnel.challenge.v1\0";
const REGISTRATION_DOMAIN: &[u8] = b"obelisk.home-tunnel.registration.v1\0";
const PLACEMENT_DOMAIN: &[u8] = b"obelisk.home-tunnel.placement.v1\0";
const STREAM_OPEN_DOMAIN: &[u8] = b"obelisk.home-tunnel.stream-open.v1\0";
const SIGNATURE_ALGORITHM: &str = "ed25519-zip215";

pub const HOME_TUNNEL_PROTOCOL_VERSION: u16 = 1;
pub const HOME_TUNNEL_MIN_NONCE_BYTES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeTunnelChallenge {
    pub protocol_version: u16,
    pub relay_id: String,
    pub challenge_id: String,
    pub nonce: String,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub issuer_public_key: String,
    pub signature_algorithm: String,
    pub signature: String,
}

impl HomeTunnelChallenge {
    pub fn issue(
        relay_id: impl Into<String>,
        challenge_id: impl Into<String>,
        nonce: impl Into<String>,
        issued_at_ms: u64,
        expires_at_ms: u64,
        relay_identity: &NodeSigningIdentity,
    ) -> Result<Self, String> {
        let mut challenge = Self {
            protocol_version: HOME_TUNNEL_PROTOCOL_VERSION,
            relay_id: relay_id.into(),
            challenge_id: challenge_id.into(),
            nonce: nonce.into(),
            issued_at_ms,
            expires_at_ms,
            issuer_public_key: relay_identity.public_key().to_string(),
            signature_algorithm: SIGNATURE_ALGORITHM.to_string(),
            signature: String::new(),
        };
        challenge.validate_fields(issued_at_ms)?;
        challenge.signature = relay_identity.sign(&challenge.signing_bytes()?);
        Ok(challenge)
    }

    pub fn verify(&self, trusted_relay_issuer: &str, now_ms: u64) -> Result<(), String> {
        self.validate_fields(now_ms)?;
        if self.issuer_public_key != trusted_relay_issuer {
            return Err("home tunnel challenge issuer is not trusted".to_string());
        }
        verify_ed25519_signature(
            &self.issuer_public_key,
            &self.signing_bytes()?,
            &self.signature,
        )
    }

    fn validate_fields(&self, now_ms: u64) -> Result<(), String> {
        if self.protocol_version != HOME_TUNNEL_PROTOCOL_VERSION {
            return Err(format!(
                "unsupported home tunnel protocol version {}",
                self.protocol_version
            ));
        }
        validate_component("home tunnel relay id", &self.relay_id)?;
        validate_component("home tunnel challenge id", &self.challenge_id)?;
        validate_nonce("home tunnel challenge nonce", &self.nonce)?;
        if self.issued_at_ms == 0
            || self.expires_at_ms <= self.issued_at_ms
            || now_ms < self.issued_at_ms
            || now_ms > self.expires_at_ms
        {
            return Err("home tunnel challenge is outside its active window".to_string());
        }
        if self.signature_algorithm != SIGNATURE_ALGORITHM {
            return Err("home tunnel challenge must use Ed25519".to_string());
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Result<Vec<u8>, String> {
        let mut unsigned = self.clone();
        unsigned.signature.clear();
        domain_json(CHALLENGE_DOMAIN, &unsigned)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeTunnelRegistration {
    pub protocol_version: u16,
    pub challenge: HomeTunnelChallenge,
    pub node_id: String,
    pub public_key: String,
    pub key_generation: u64,
    pub agent_instance_id: String,
    pub process_id: u32,
    pub sequence: u64,
    pub started_at_ms: u64,
    pub tls_certificate_sha256: String,
    pub capacity_certificate: NodeCapacityCertificate,
    pub signature_algorithm: String,
    pub signature: String,
}

impl HomeTunnelRegistration {
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        challenge: HomeTunnelChallenge,
        identity: &NodeSigningIdentity,
        key_generation: u64,
        agent_instance_id: impl Into<String>,
        process_id: u32,
        sequence: u64,
        started_at_ms: u64,
        tls_certificate_sha256: impl Into<String>,
        capacity_certificate: NodeCapacityCertificate,
    ) -> Result<Self, String> {
        let mut registration = Self {
            protocol_version: HOME_TUNNEL_PROTOCOL_VERSION,
            challenge,
            node_id: identity.node_id().to_string(),
            public_key: identity.public_key().to_string(),
            key_generation,
            agent_instance_id: agent_instance_id.into(),
            process_id,
            sequence,
            started_at_ms,
            tls_certificate_sha256: tls_certificate_sha256.into(),
            capacity_certificate,
            signature_algorithm: SIGNATURE_ALGORITHM.to_string(),
            signature: String::new(),
        };
        registration.signature = identity.sign(&registration.signing_bytes()?);
        Ok(registration)
    }

    pub fn verify(
        &self,
        trusted_relay_issuer: &str,
        trusted_capacity_issuer: &str,
        expected_relay_id: &str,
        now_ms: u64,
    ) -> Result<(), String> {
        if self.protocol_version != HOME_TUNNEL_PROTOCOL_VERSION {
            return Err("home tunnel registration protocol version mismatch".to_string());
        }
        self.challenge.verify(trusted_relay_issuer, now_ms)?;
        if self.challenge.relay_id != expected_relay_id {
            return Err("home tunnel registration targets a different relay".to_string());
        }
        validate_component("home tunnel node id", &self.node_id)?;
        validate_component("home tunnel agent instance id", &self.agent_instance_id)?;
        if self.key_generation == 0 || self.process_id == 0 || self.sequence == 0 {
            return Err(
                "home tunnel registration generation, process and sequence must be positive"
                    .to_string(),
            );
        }
        if self.started_at_ms == 0 || self.started_at_ms > now_ms {
            return Err("home tunnel agent start time is invalid".to_string());
        }
        validate_sha256(
            "home tunnel TLS certificate fingerprint",
            &self.tls_certificate_sha256,
        )?;
        if node_id_from_public_key(&self.public_key)? != self.node_id {
            return Err("home tunnel registration node id does not match public key".to_string());
        }
        self.capacity_certificate
            .verify(trusted_capacity_issuer, now_ms)?;
        if self.capacity_certificate.node_id != self.node_id
            || self.capacity_certificate.public_key != self.public_key
            || self.capacity_certificate.key_generation != self.key_generation
        {
            return Err(
                "home tunnel registration does not match its capacity certificate".to_string(),
            );
        }
        if self.signature_algorithm != SIGNATURE_ALGORITHM {
            return Err("home tunnel registration must use Ed25519".to_string());
        }
        verify_ed25519_signature(&self.public_key, &self.signing_bytes()?, &self.signature)
    }

    fn signing_bytes(&self) -> Result<Vec<u8>, String> {
        let mut unsigned = self.clone();
        unsigned.signature.clear();
        domain_json(REGISTRATION_DOMAIN, &unsigned)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeTunnelPlacement {
    pub protocol_version: u16,
    pub placement_id: String,
    pub relay_id: String,
    pub zone_id: String,
    pub node_id: String,
    pub key_generation: u64,
    pub generation: u64,
    pub max_streams: usize,
    pub finalized_control_height: u64,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub issuer_public_key: String,
    pub signature_algorithm: String,
    pub signature: String,
}

impl HomeTunnelPlacement {
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        placement_id: impl Into<String>,
        relay_id: impl Into<String>,
        zone_id: impl Into<String>,
        node_id: impl Into<String>,
        key_generation: u64,
        generation: u64,
        max_streams: usize,
        finalized_control_height: u64,
        issued_at_ms: u64,
        expires_at_ms: u64,
        control_identity: &NodeSigningIdentity,
    ) -> Result<Self, String> {
        let mut placement = Self {
            protocol_version: HOME_TUNNEL_PROTOCOL_VERSION,
            placement_id: placement_id.into(),
            relay_id: relay_id.into(),
            zone_id: zone_id.into(),
            node_id: node_id.into(),
            key_generation,
            generation,
            max_streams,
            finalized_control_height,
            issued_at_ms,
            expires_at_ms,
            issuer_public_key: control_identity.public_key().to_string(),
            signature_algorithm: SIGNATURE_ALGORITHM.to_string(),
            signature: String::new(),
        };
        placement.validate_fields(issued_at_ms)?;
        placement.signature = control_identity.sign(&placement.signing_bytes()?);
        Ok(placement)
    }

    pub fn verify(
        &self,
        trusted_control_issuer: &str,
        expected_relay_id: &str,
        certificate: &NodeCapacityCertificate,
        now_ms: u64,
    ) -> Result<(), String> {
        self.validate_fields(now_ms)?;
        if self.issuer_public_key != trusted_control_issuer {
            return Err("home tunnel placement issuer is not trusted".to_string());
        }
        if self.relay_id != expected_relay_id {
            return Err("home tunnel placement targets a different relay".to_string());
        }
        if self.node_id != certificate.node_id || self.key_generation != certificate.key_generation
        {
            return Err("home tunnel placement identity does not match certificate".to_string());
        }
        if self.max_streams > certificate.max_sessions_per_zone {
            return Err("home tunnel placement exceeds certified per-Zone capacity".to_string());
        }
        verify_ed25519_signature(
            &self.issuer_public_key,
            &self.signing_bytes()?,
            &self.signature,
        )
    }

    fn validate_fields(&self, now_ms: u64) -> Result<(), String> {
        if self.protocol_version != HOME_TUNNEL_PROTOCOL_VERSION {
            return Err("home tunnel placement protocol version mismatch".to_string());
        }
        validate_component("home tunnel placement id", &self.placement_id)?;
        validate_component("home tunnel placement relay id", &self.relay_id)?;
        validate_component("home tunnel placement Zone id", &self.zone_id)?;
        validate_component("home tunnel placement node id", &self.node_id)?;
        if self.key_generation == 0
            || self.generation == 0
            || self.max_streams == 0
            || self.finalized_control_height == 0
        {
            return Err("home tunnel placement contains an invalid zero field".to_string());
        }
        if self.issued_at_ms == 0
            || self.expires_at_ms <= self.issued_at_ms
            || now_ms < self.issued_at_ms
            || now_ms > self.expires_at_ms
        {
            return Err("home tunnel placement is outside its active window".to_string());
        }
        if self.signature_algorithm != SIGNATURE_ALGORITHM {
            return Err("home tunnel placement must use Ed25519".to_string());
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Result<Vec<u8>, String> {
        let mut unsigned = self.clone();
        unsigned.signature.clear();
        domain_json(PLACEMENT_DOMAIN, &unsigned)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeTunnelStreamOpen {
    pub protocol_version: u16,
    pub placement_id: String,
    pub relay_id: String,
    pub zone_id: String,
    pub node_id: String,
    pub key_generation: u64,
    pub placement_generation: u64,
    pub session_id: String,
    pub stream_sequence: u64,
    pub nonce: String,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub relay_public_key: String,
    pub signature_algorithm: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeTunnelStreamEnvelope {
    pub placement: HomeTunnelPlacement,
    pub open: HomeTunnelStreamOpen,
}

impl HomeTunnelStreamEnvelope {
    pub fn verify(
        &self,
        trusted_control_issuer: &str,
        trusted_relay_issuer: &str,
        expected_relay_id: &str,
        capacity_certificate: &NodeCapacityCertificate,
        now_ms: u64,
    ) -> Result<(), String> {
        self.placement.verify(
            trusted_control_issuer,
            expected_relay_id,
            capacity_certificate,
            now_ms,
        )?;
        self.open
            .verify(trusted_relay_issuer, &self.placement, now_ms)
    }
}

impl HomeTunnelStreamOpen {
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        placement: &HomeTunnelPlacement,
        session_id: impl Into<String>,
        stream_sequence: u64,
        nonce: impl Into<String>,
        issued_at_ms: u64,
        expires_at_ms: u64,
        relay_identity: &NodeSigningIdentity,
    ) -> Result<Self, String> {
        let mut open = Self {
            protocol_version: HOME_TUNNEL_PROTOCOL_VERSION,
            placement_id: placement.placement_id.clone(),
            relay_id: placement.relay_id.clone(),
            zone_id: placement.zone_id.clone(),
            node_id: placement.node_id.clone(),
            key_generation: placement.key_generation,
            placement_generation: placement.generation,
            session_id: session_id.into(),
            stream_sequence,
            nonce: nonce.into(),
            issued_at_ms,
            expires_at_ms,
            relay_public_key: relay_identity.public_key().to_string(),
            signature_algorithm: SIGNATURE_ALGORITHM.to_string(),
            signature: String::new(),
        };
        open.validate_fields(issued_at_ms)?;
        open.signature = relay_identity.sign(&open.signing_bytes()?);
        Ok(open)
    }

    pub fn verify(
        &self,
        trusted_relay_issuer: &str,
        placement: &HomeTunnelPlacement,
        now_ms: u64,
    ) -> Result<(), String> {
        self.validate_fields(now_ms)?;
        if self.relay_public_key != trusted_relay_issuer {
            return Err("home tunnel stream issuer is not trusted".to_string());
        }
        if self.placement_id != placement.placement_id
            || self.relay_id != placement.relay_id
            || self.zone_id != placement.zone_id
            || self.node_id != placement.node_id
            || self.key_generation != placement.key_generation
            || self.placement_generation != placement.generation
        {
            return Err("home tunnel stream does not match finalized placement".to_string());
        }
        if self.expires_at_ms > placement.expires_at_ms {
            return Err("home tunnel stream outlives its placement".to_string());
        }
        verify_ed25519_signature(
            &self.relay_public_key,
            &self.signing_bytes()?,
            &self.signature,
        )
    }

    fn validate_fields(&self, now_ms: u64) -> Result<(), String> {
        if self.protocol_version != HOME_TUNNEL_PROTOCOL_VERSION {
            return Err("home tunnel stream protocol version mismatch".to_string());
        }
        validate_component("home tunnel stream placement id", &self.placement_id)?;
        validate_component("home tunnel stream relay id", &self.relay_id)?;
        validate_component("home tunnel stream Zone id", &self.zone_id)?;
        validate_component("home tunnel stream node id", &self.node_id)?;
        validate_component("home tunnel stream Session id", &self.session_id)?;
        validate_nonce("home tunnel stream nonce", &self.nonce)?;
        if self.key_generation == 0 || self.placement_generation == 0 || self.stream_sequence == 0 {
            return Err("home tunnel stream contains an invalid zero field".to_string());
        }
        if self.issued_at_ms == 0
            || self.expires_at_ms <= self.issued_at_ms
            || now_ms < self.issued_at_ms
            || now_ms > self.expires_at_ms
        {
            return Err("home tunnel stream is outside its active window".to_string());
        }
        if self.signature_algorithm != SIGNATURE_ALGORITHM {
            return Err("home tunnel stream must use Ed25519".to_string());
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Result<Vec<u8>, String> {
        let mut unsigned = self.clone();
        unsigned.signature.clear();
        domain_json(STREAM_OPEN_DOMAIN, &unsigned)
    }
}

#[derive(Debug, Default)]
pub struct HomeTunnelReplayGuard {
    state: Mutex<HomeTunnelReplayState>,
}

#[derive(Debug, Default)]
struct HomeTunnelReplayState {
    challenge_ids: BTreeSet<String>,
    registration_sequences: BTreeMap<(String, u64, String, u32), u64>,
    stream_sequences: BTreeMap<(String, u64), u64>,
    active_sessions: BTreeMap<String, BTreeSet<String>>,
    stream_nonces: BTreeMap<(String, String), u64>,
}

impl HomeTunnelReplayGuard {
    pub fn accept_registration(
        &self,
        registration: &HomeTunnelRegistration,
        trusted_relay_issuer: &str,
        trusted_capacity_issuer: &str,
        expected_relay_id: &str,
        now_ms: u64,
    ) -> Result<(), String> {
        registration.verify(
            trusted_relay_issuer,
            trusted_capacity_issuer,
            expected_relay_id,
            now_ms,
        )?;
        let key = (
            registration.node_id.clone(),
            registration.key_generation,
            registration.agent_instance_id.clone(),
            registration.process_id,
        );
        let mut state = self
            .state
            .lock()
            .map_err(|_| "home tunnel replay guard mutex poisoned".to_string())?;
        if state
            .challenge_ids
            .contains(&registration.challenge.challenge_id)
        {
            return Err("home tunnel challenge was already consumed".to_string());
        }
        if state
            .registration_sequences
            .get(&key)
            .is_some_and(|previous| registration.sequence <= *previous)
        {
            return Err("home tunnel registration sequence was replayed".to_string());
        }
        state
            .challenge_ids
            .insert(registration.challenge.challenge_id.clone());
        state
            .registration_sequences
            .insert(key, registration.sequence);
        Ok(())
    }

    pub fn accept_stream(
        &self,
        open: &HomeTunnelStreamOpen,
        placement: &HomeTunnelPlacement,
        trusted_relay_issuer: &str,
        now_ms: u64,
    ) -> Result<(), String> {
        open.verify(trusted_relay_issuer, placement, now_ms)?;
        let stream_key = (open.placement_id.clone(), open.stream_sequence);
        let nonce_key = (open.placement_id.clone(), open.nonce.clone());
        let mut state = self
            .state
            .lock()
            .map_err(|_| "home tunnel replay guard mutex poisoned".to_string())?;
        state
            .stream_sequences
            .retain(|_, expires_at_ms| now_ms <= *expires_at_ms);
        state
            .stream_nonces
            .retain(|_, expires_at_ms| now_ms <= *expires_at_ms);
        if state.stream_nonces.contains_key(&nonce_key) {
            return Err("home tunnel stream nonce was replayed".to_string());
        }
        if state.stream_sequences.contains_key(&stream_key) {
            return Err("home tunnel stream sequence was replayed".to_string());
        }
        let active = state
            .active_sessions
            .entry(open.placement_id.clone())
            .or_default();
        let replaces_existing = active.contains(&open.session_id);
        if !replaces_existing && active.len() >= placement.max_streams {
            return Err("home tunnel placement stream capacity exhausted".to_string());
        }
        active.insert(open.session_id.clone());
        state
            .stream_sequences
            .insert(stream_key, open.expires_at_ms);
        state.stream_nonces.insert(nonce_key, open.expires_at_ms);
        Ok(())
    }

    pub fn close_stream(&self, placement_id: &str, session_id: &str) {
        if let Ok(mut state) = self.state.lock()
            && let Some(active) = state.active_sessions.get_mut(placement_id)
        {
            active.remove(session_id);
            if active.is_empty() {
                state.active_sessions.remove(placement_id);
            }
        }
    }

    pub fn active_streams(&self, placement_id: &str) -> usize {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.active_sessions.get(placement_id).map(BTreeSet::len))
            .unwrap_or_default()
    }
}

fn domain_json(domain: &[u8], value: &impl Serialize) -> Result<Vec<u8>, String> {
    let payload = serde_json::to_vec(value)
        .map_err(|error| format!("home tunnel canonical serialization failed: {error}"))?;
    let mut bytes = Vec::with_capacity(domain.len() + payload.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

fn validate_component(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        return Err(format!("invalid {label}"));
    }
    Ok(())
}

fn validate_nonce(label: &str, value: &str) -> Result<(), String> {
    let decoded = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| format!("{label} must be URL-safe base64 without padding"))?;
    if decoded.len() < HOME_TUNNEL_MIN_NONCE_BYTES {
        return Err(format!("{label} must contain at least 128 bits"));
    }
    Ok(())
}

fn validate_sha256(label: &str, value: &str) -> Result<(), String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{label} must be a SHA-256 hex digest"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CapacityChallenge, CapacityChallengeResponse, CapacityWorkload,
        FinalizedGuildNodeRegistration, GuildNodeStatus, SuiFinalityProof,
    };

    fn capacity_certificate(
        node: &NodeSigningIdentity,
        issuer: &NodeSigningIdentity,
    ) -> NodeCapacityCertificate {
        let registration = FinalizedGuildNodeRegistration {
            node_id: node.node_id().to_string(),
            operator_sui_address: format!("0x{}", "11".repeat(32)),
            public_key: node.public_key().to_string(),
            endpoint: "outbound-only".to_string(),
            failure_domain: "home-isp-a".to_string(),
            stake_mist: 1_000_000,
            max_sessions: 8,
            max_zones: 2,
            key_generation: 1,
            status: GuildNodeStatus::Active,
            finality: SuiFinalityProof {
                network: "testnet".to_string(),
                package_id: format!("0x{}", "22".repeat(32)),
                transaction_digest: "home-tunnel-test".to_string(),
                event_sequence: 0,
                checkpoint: 42,
            },
        };
        let challenge = CapacityChallenge {
            challenge_id: "capacity-home-a".to_string(),
            node_id: node.node_id().to_string(),
            nonce: URL_SAFE_NO_PAD.encode([3; 32]),
            issued_at_ms: 1_000,
            expires_at_ms: 5_000,
            workload: CapacityWorkload {
                concurrent_sessions: 8,
                max_sessions_per_zone: 4,
                zone_count: 2,
                command_count: 100,
                maximum_p95_latency_ms: 200,
                minimum_success_bps: 9_900,
            },
        };
        let response =
            CapacityChallengeResponse::sign(challenge, node, 1, 100, 0, 40, "ab".repeat(32), 2_000)
                .unwrap();
        NodeCapacityCertificate::issue(&response, &registration, issuer, 2_500, 60_000, 7).unwrap()
    }

    struct Fixture {
        node: NodeSigningIdentity,
        relay: NodeSigningIdentity,
        control: NodeSigningIdentity,
        capacity_issuer: NodeSigningIdentity,
        certificate: NodeCapacityCertificate,
        challenge: HomeTunnelChallenge,
    }

    fn fixture() -> Fixture {
        let node = NodeSigningIdentity::from_seed([1; 32]);
        let relay = NodeSigningIdentity::from_seed([2; 32]);
        let control = NodeSigningIdentity::from_seed([3; 32]);
        let capacity_issuer = NodeSigningIdentity::from_seed([4; 32]);
        let certificate = capacity_certificate(&node, &capacity_issuer);
        let challenge = HomeTunnelChallenge::issue(
            "relay-hk-a",
            "challenge-a",
            URL_SAFE_NO_PAD.encode([5; 32]),
            3_000,
            4_000,
            &relay,
        )
        .unwrap();
        Fixture {
            node,
            relay,
            control,
            capacity_issuer,
            certificate,
            challenge,
        }
    }

    fn placement(fixture: &Fixture, max_streams: usize) -> HomeTunnelPlacement {
        HomeTunnelPlacement::issue(
            "placement-map-0",
            "relay-hk-a",
            "map:0",
            fixture.node.node_id(),
            1,
            9,
            max_streams,
            99,
            3_100,
            10_000,
            &fixture.control,
        )
        .unwrap()
    }

    #[test]
    fn signed_registration_binds_relay_node_generation_and_capacity() {
        let fixture = fixture();
        let registration = HomeTunnelRegistration::sign(
            fixture.challenge.clone(),
            &fixture.node,
            1,
            "agent-a",
            42,
            1,
            2_900,
            "aa".repeat(32),
            fixture.certificate.clone(),
        )
        .unwrap();
        registration
            .verify(
                fixture.relay.public_key(),
                fixture.capacity_issuer.public_key(),
                "relay-hk-a",
                3_500,
            )
            .unwrap();

        let mut tampered = registration;
        tampered.key_generation = 2;
        assert!(
            tampered
                .verify(
                    fixture.relay.public_key(),
                    fixture.capacity_issuer.public_key(),
                    "relay-hk-a",
                    3_500,
                )
                .is_err()
        );
    }

    #[test]
    fn registration_challenge_and_sequence_are_single_use() {
        let fixture = fixture();
        let registration = HomeTunnelRegistration::sign(
            fixture.challenge,
            &fixture.node,
            1,
            "agent-a",
            42,
            1,
            2_900,
            "aa".repeat(32),
            fixture.certificate,
        )
        .unwrap();
        let guard = HomeTunnelReplayGuard::default();
        guard
            .accept_registration(
                &registration,
                fixture.relay.public_key(),
                fixture.capacity_issuer.public_key(),
                "relay-hk-a",
                3_500,
            )
            .unwrap();
        assert!(
            guard
                .accept_registration(
                    &registration,
                    fixture.relay.public_key(),
                    fixture.capacity_issuer.public_key(),
                    "relay-hk-a",
                    3_500,
                )
                .is_err()
        );
    }

    #[test]
    fn placement_rejects_wrong_generation_and_certified_capacity_overflow() {
        let fixture = fixture();
        let valid = placement(&fixture, 4);
        valid
            .verify(
                fixture.control.public_key(),
                "relay-hk-a",
                &fixture.certificate,
                3_500,
            )
            .unwrap();

        let overflow = placement(&fixture, 5);
        assert!(
            overflow
                .verify(
                    fixture.control.public_key(),
                    "relay-hk-a",
                    &fixture.certificate,
                    3_500,
                )
                .is_err()
        );
        let mut stale = valid;
        stale.generation = 8;
        assert!(
            stale
                .verify(
                    fixture.control.public_key(),
                    "relay-hk-a",
                    &fixture.certificate,
                    3_500,
                )
                .is_err()
        );
    }

    #[test]
    fn stream_guard_enforces_replay_reconnect_and_capacity() {
        let fixture = fixture();
        let placement = placement(&fixture, 1);
        let first = HomeTunnelStreamOpen::sign(
            &placement,
            "session-a",
            1,
            URL_SAFE_NO_PAD.encode([7; 32]),
            3_200,
            3_900,
            &fixture.relay,
        )
        .unwrap();
        let guard = HomeTunnelReplayGuard::default();
        guard
            .accept_stream(&first, &placement, fixture.relay.public_key(), 3_500)
            .unwrap();
        assert_eq!(guard.active_streams(&placement.placement_id), 1);
        assert!(
            guard
                .accept_stream(&first, &placement, fixture.relay.public_key(), 3_500)
                .is_err()
        );

        let overflow = HomeTunnelStreamOpen::sign(
            &placement,
            "session-b",
            1,
            URL_SAFE_NO_PAD.encode([8; 32]),
            3_200,
            3_900,
            &fixture.relay,
        )
        .unwrap();
        assert!(
            guard
                .accept_stream(&overflow, &placement, fixture.relay.public_key(), 3_500)
                .is_err()
        );

        let reconnect = HomeTunnelStreamOpen::sign(
            &placement,
            "session-a",
            2,
            URL_SAFE_NO_PAD.encode([9; 32]),
            3_200,
            3_900,
            &fixture.relay,
        )
        .unwrap();
        guard
            .accept_stream(&reconnect, &placement, fixture.relay.public_key(), 3_500)
            .unwrap();
        guard.close_stream(&placement.placement_id, "session-a");
        assert_eq!(guard.active_streams(&placement.placement_id), 0);

        let ahead = HomeTunnelStreamOpen::sign(
            &placement,
            "session-a",
            4,
            URL_SAFE_NO_PAD.encode([10; 32]),
            3_200,
            3_900,
            &fixture.relay,
        )
        .unwrap();
        guard
            .accept_stream(&ahead, &placement, fixture.relay.public_key(), 3_500)
            .unwrap();
        guard.close_stream(&placement.placement_id, "session-a");
        let delayed = HomeTunnelStreamOpen::sign(
            &placement,
            "session-a",
            3,
            URL_SAFE_NO_PAD.encode([11; 32]),
            3_200,
            3_900,
            &fixture.relay,
        )
        .unwrap();
        guard
            .accept_stream(&delayed, &placement, fixture.relay.public_key(), 3_500)
            .expect("unique signed QUIC streams may be handled out of order");
        guard.close_stream(&placement.placement_id, "session-a");
        assert!(
            guard
                .accept_stream(&ahead, &placement, fixture.relay.public_key(), 3_500)
                .is_err()
        );
    }
}
