//! The map coordinator's ordered business-random stream. Entropy is admitted
//! once, before the map starts; subsequent draws are replayable authoritative
//! state. Online identity/nonces continue using their separate OS randomness.
use hmac::{Hmac, KeyInit, Mac};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

const VERSION: u32 = 1;
const DOMAIN: &[u8] = b"mir2.zone.combat-random.hmac-sha256.v1\0";

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RandomWire", into = "RandomWire")]
pub(super) struct AuthoritativeCombatRandom {
    seed: [u8; 32],
    draws: u64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RandomWire {
    version: u32,
    seed: [u8; 32],
    draws: u64,
}

impl std::fmt::Debug for AuthoritativeCombatRandom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthoritativeCombatRandom")
            .field("version", &VERSION)
            .field("draws", &self.draws)
            .finish_non_exhaustive()
    }
}

impl AuthoritativeCombatRandom {
    pub(super) fn fresh() -> Self {
        let mut seed = [0; 32];
        OsRng.fill_bytes(&mut seed);
        Self { seed, draws: 0 }
    }

    /// Only used while authenticating a pre-v8 checkpoint. No command/tick may
    /// run on that temporary runtime. Its old root excludes this field, and a
    /// fresh stream replaces it only after the legacy root has been verified.
    pub(super) fn legacy_verification_placeholder() -> Self {
        Self {
            seed: [0; 32],
            draws: 0,
        }
    }

    /// Preserve the original u32/modulo probability contracts at call sites.
    /// The counter never wraps: exhaustion fails closed without another draw.
    pub(super) fn next_u32(&mut self) -> Option<u32> {
        let next = self.draws.checked_add(1)?;
        let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(&self.seed)
            .expect("fixed-size HMAC key is valid");
        mac.update(DOMAIN);
        mac.update(&self.draws.to_le_bytes());
        let bytes = mac.finalize().into_bytes();
        let result = u32::from_le_bytes(bytes[..4].try_into().expect("SHA256 output length"));
        self.draws = next;
        Some(result)
    }

    pub(super) fn from_seed(seed: [u8; 32]) -> Self {
        Self { seed, draws: 0 }
    }

    #[cfg(test)]
    pub(super) fn draws(&self) -> u64 {
        self.draws
    }
}

impl TryFrom<RandomWire> for AuthoritativeCombatRandom {
    type Error = String;

    fn try_from(value: RandomWire) -> Result<Self, Self::Error> {
        if value.version != VERSION {
            return Err(format!(
                "unsupported combat random stream version {}",
                value.version
            ));
        }
        Ok(Self {
            seed: value.seed,
            draws: value.draws,
        })
    }
}

impl From<AuthoritativeCombatRandom> for RandomWire {
    fn from(value: AuthoritativeCombatRandom) -> Self {
        Self {
            version: VERSION,
            seed: value.seed,
            draws: value.draws,
        }
    }
}

#[cfg(test)]
#[path = "combat_random_tests.rs"]
mod tests;
