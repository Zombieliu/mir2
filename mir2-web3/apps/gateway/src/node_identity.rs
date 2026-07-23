use std::collections::BTreeMap;
use std::env;
use std::fmt;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ed25519_consensus::{Signature, SigningKey, VerificationKey};
use sha2::{Digest, Sha256};

const NODE_ID_DOMAIN: &[u8] = b"obelisk.guild-node.ed25519.v1\0";

#[derive(Clone)]
pub struct NodeSigningIdentity {
    signing_key: SigningKey,
    public_key: String,
    node_id: String,
}

impl fmt::Debug for NodeSigningIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NodeSigningIdentity")
            .field("node_id", &self.node_id)
            .field("public_key", &self.public_key)
            .field("signing_key", &"[REDACTED]")
            .finish()
    }
}

impl NodeSigningIdentity {
    pub fn from_seed(seed: [u8; 32]) -> Self {
        let signing_key = SigningKey::from(seed);
        let public_key =
            URL_SAFE_NO_PAD.encode(signing_key.verification_key().to_bytes().as_slice());
        let node_id = node_id_from_public_key(&public_key)
            .expect("derived Ed25519 public key must produce a valid node id");
        Self {
            signing_key,
            public_key,
            node_id,
        }
    }

    pub fn from_base64_seed(value: &str) -> Result<Self, String> {
        let decoded = URL_SAFE_NO_PAD
            .decode(value.trim())
            .map_err(|_| "node signing key must be URL-safe base64 without padding".to_string())?;
        let seed: [u8; 32] = decoded
            .try_into()
            .map_err(|_| "node signing key must decode to exactly 32 bytes".to_string())?;
        Ok(Self::from_seed(seed))
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let value = fs::read_to_string(path).map_err(|error| {
            format!(
                "failed to read node signing key {}: {error}",
                path.display()
            )
        })?;
        Self::from_base64_seed(&value)
    }

    pub fn from_env() -> Result<Option<Self>, String> {
        let inline = env::var("MIR2_ZONE_HOST_SIGNING_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let file = env::var("MIR2_ZONE_HOST_SIGNING_KEY_FILE")
            .ok()
            .filter(|value| !value.trim().is_empty());
        match (inline, file) {
            (Some(_), Some(_)) => Err(
                "configure only one of MIR2_ZONE_HOST_SIGNING_KEY or MIR2_ZONE_HOST_SIGNING_KEY_FILE"
                    .to_string(),
            ),
            (Some(value), None) => Self::from_base64_seed(&value).map(Some),
            (None, Some(path)) => Self::from_file(path).map(Some),
            (None, None) => Ok(None),
        }
    }

    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    pub fn public_key(&self) -> &str {
        &self.public_key
    }

    pub fn sign(&self, message: &[u8]) -> String {
        URL_SAFE_NO_PAD.encode(self.signing_key.sign(message).to_bytes())
    }
}

pub fn node_id_from_public_key(public_key: &str) -> Result<String, String> {
    let decoded = decode_public_key(public_key)?;
    let mut digest = Sha256::new();
    digest.update(NODE_ID_DOMAIN);
    digest.update(decoded);
    Ok(format!("ed25519:{}", hex_digest(&digest.finalize())))
}

pub fn validate_ed25519_public_key(public_key: &str) -> Result<(), String> {
    let public_key = decode_public_key(public_key)?;
    VerificationKey::try_from(public_key)
        .map(|_| ())
        .map_err(|_| "invalid Ed25519 public key".to_string())
}

pub fn verify_ed25519_signature(
    public_key: &str,
    message: &[u8],
    signature: &str,
) -> Result<(), String> {
    let public_key = decode_public_key(public_key)?;
    let verification_key = VerificationKey::try_from(public_key)
        .map_err(|_| "invalid Ed25519 public key".to_string())?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| "invalid Ed25519 signature encoding".to_string())?;
    let signature = Signature::try_from(signature.as_slice())
        .map_err(|_| "invalid Ed25519 signature length".to_string())?;
    verification_key
        .verify(&signature, message)
        .map_err(|_| "invalid Ed25519 signature".to_string())
}

fn decode_public_key(public_key: &str) -> Result<[u8; 32], String> {
    let decoded = URL_SAFE_NO_PAD
        .decode(public_key)
        .map_err(|_| "invalid Ed25519 public key encoding".to_string())?;
    decoded
        .try_into()
        .map_err(|_| "Ed25519 public key must decode to exactly 32 bytes".to_string())
}

#[derive(Debug, Default)]
pub struct NodeHeartbeatReplayGuard {
    sequences: Mutex<BTreeMap<(String, u64, u32), u64>>,
}

impl NodeHeartbeatReplayGuard {
    pub fn accept(
        &self,
        node_id: &str,
        key_generation: u64,
        process_id: u32,
        sequence: u64,
    ) -> Result<(), String> {
        if sequence == 0 {
            return Err("heartbeat sequence must be positive".to_string());
        }
        let key = (node_id.to_string(), key_generation, process_id);
        let mut sequences = self
            .sequences
            .lock()
            .map_err(|_| "heartbeat replay guard mutex poisoned".to_string())?;
        if sequences
            .get(&key)
            .is_some_and(|previous| sequence <= *previous)
        {
            return Err(format!(
                "replayed heartbeat sequence {sequence} for {node_id} generation {key_generation}"
            ));
        }
        sequences.insert(key, sequence);
        Ok(())
    }

    pub fn clear_node(&self, node_id: &str) {
        if let Ok(mut sequences) = self.sequences.lock() {
            sequences.retain(|(registered, _, _), _| registered != node_id);
        }
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_signatures_bind_to_the_public_key_and_node_id() {
        let identity = NodeSigningIdentity::from_seed([7; 32]);
        let message = b"gate13-heartbeat";
        let signature = identity.sign(message);
        verify_ed25519_signature(identity.public_key(), message, &signature).unwrap();
        assert_eq!(
            node_id_from_public_key(identity.public_key()).unwrap(),
            identity.node_id()
        );
        assert!(verify_ed25519_signature(identity.public_key(), b"tampered", &signature).is_err());
    }

    #[test]
    fn signing_identity_debug_never_exposes_seed() {
        let identity = NodeSigningIdentity::from_seed([7; 32]);
        let debug = format!("{identity:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("[7, 7, 7"));
    }

    #[test]
    fn replay_guard_is_scoped_to_key_generation_and_process() {
        let guard = NodeHeartbeatReplayGuard::default();
        guard.accept("node-a", 1, 10, 1).unwrap();
        assert!(guard.accept("node-a", 1, 10, 1).is_err());
        assert!(guard.accept("node-a", 1, 10, 0).is_err());
        guard.accept("node-a", 1, 10, 2).unwrap();
        guard.accept("node-a", 2, 10, 1).unwrap();
        guard.accept("node-a", 2, 11, 1).unwrap();
    }
}
