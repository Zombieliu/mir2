use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{ZoneReplayEngine, ZoneReplayReport};

const REPLICA_CHECKPOINT_VERSION: u32 = 1;
const REPLICA_CHECKSUM_DOMAIN: &[u8] = b"obelisk.mir2.zone-replica.v1\0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneReplicaCheckpoint {
    pub version: u32,
    pub source_owner_id: String,
    pub fencing_token: u64,
    pub report: ZoneReplayReport,
    pub checkpoint_bytes: Vec<u8>,
    pub checksum: String,
}

impl ZoneReplicaCheckpoint {
    pub fn capture(
        engine: &ZoneReplayEngine,
        source_owner_id: impl Into<String>,
        fencing_token: u64,
    ) -> Result<Self, String> {
        let source_owner_id = source_owner_id.into();
        validate_owner_id(&source_owner_id)?;
        if fencing_token == 0 {
            return Err("zone replica fencing token must be positive".to_string());
        }
        let report = engine.report();
        if report.epoch != fencing_token {
            return Err(format!(
                "zone replica epoch {} does not match fencing token {fencing_token}",
                report.epoch
            ));
        }
        let checkpoint_bytes = engine.checkpoint_bytes()?;
        let checksum = checkpoint_checksum(
            REPLICA_CHECKPOINT_VERSION,
            &source_owner_id,
            fencing_token,
            &report,
            &checkpoint_bytes,
        )?;
        Ok(Self {
            version: REPLICA_CHECKPOINT_VERSION,
            source_owner_id,
            fencing_token,
            report,
            checkpoint_bytes,
            checksum,
        })
    }

    pub fn verify(&self) -> Result<ZoneReplayEngine, String> {
        if self.version != REPLICA_CHECKPOINT_VERSION {
            return Err(format!(
                "unsupported zone replica checkpoint version {}, expected {}",
                self.version, REPLICA_CHECKPOINT_VERSION
            ));
        }
        validate_owner_id(&self.source_owner_id)?;
        if self.fencing_token == 0 {
            return Err("zone replica fencing token must be positive".to_string());
        }
        if self.report.epoch != self.fencing_token {
            return Err("zone replica report epoch does not match fencing token".to_string());
        }
        let expected = checkpoint_checksum(
            self.version,
            &self.source_owner_id,
            self.fencing_token,
            &self.report,
            &self.checkpoint_bytes,
        )?;
        if !constant_time_equal(expected.as_bytes(), self.checksum.as_bytes()) {
            return Err("zone replica checkpoint checksum mismatch".to_string());
        }
        let engine = ZoneReplayEngine::restore(&self.checkpoint_bytes)?;
        if engine.report() != self.report {
            return Err("zone replica restored report mismatch".to_string());
        }
        Ok(engine)
    }
}

pub struct ZoneStandbyReplica {
    zone_id: String,
    accepted: Option<ZoneReplicaCheckpoint>,
}

impl ZoneStandbyReplica {
    pub fn new(zone_id: impl Into<String>) -> Result<Self, String> {
        let zone_id = zone_id.into();
        if zone_id.trim().is_empty() {
            return Err("standby zone id must not be empty".to_string());
        }
        Ok(Self {
            zone_id,
            accepted: None,
        })
    }

    pub fn accept(&mut self, checkpoint: ZoneReplicaCheckpoint) -> Result<bool, String> {
        let _verified = checkpoint.verify()?;
        if checkpoint.report.zone_id != self.zone_id {
            return Err(format!(
                "standby zone id mismatch: expected {}, got {}",
                self.zone_id, checkpoint.report.zone_id
            ));
        }
        if let Some(current) = &self.accepted {
            if checkpoint.fencing_token < current.fencing_token {
                return Err(format!(
                    "stale zone replica fencing token {} is below {}",
                    checkpoint.fencing_token, current.fencing_token
                ));
            }
            if checkpoint.fencing_token == current.fencing_token {
                let next_sequence = checkpoint.report.final_sequence;
                let current_sequence = current.report.final_sequence;
                if next_sequence < current_sequence {
                    return Err(format!(
                        "stale zone replica sequence {next_sequence:?} is below {current_sequence:?}"
                    ));
                }
                if checkpoint.checksum == current.checksum {
                    return Ok(false);
                }
                if next_sequence == current_sequence {
                    return Err(
                        "conflicting zone replica checkpoint at the same fencing token and sequence"
                            .to_string(),
                    );
                }
            }
        }
        self.accepted = Some(checkpoint);
        Ok(true)
    }

    pub fn report(&self) -> Option<&ZoneReplayReport> {
        self.accepted.as_ref().map(|checkpoint| &checkpoint.report)
    }

    pub fn promote(self, new_fencing_token: u64) -> Result<ZoneReplayEngine, String> {
        let checkpoint = self
            .accepted
            .ok_or_else(|| "standby has no verified checkpoint to promote".to_string())?;
        if new_fencing_token <= checkpoint.fencing_token {
            return Err(format!(
                "promotion fencing token {new_fencing_token} must exceed replicated token {}",
                checkpoint.fencing_token
            ));
        }
        checkpoint.verify()?.rebase_epoch(new_fencing_token)
    }
}

fn checkpoint_checksum(
    version: u32,
    source_owner_id: &str,
    fencing_token: u64,
    report: &ZoneReplayReport,
    checkpoint_bytes: &[u8],
) -> Result<String, String> {
    let report_bytes = serde_json::to_vec(report)
        .map_err(|error| format!("failed to serialize zone replica report: {error}"))?;
    let mut hasher = Sha256::new();
    hasher.update(REPLICA_CHECKSUM_DOMAIN);
    hasher.update(version.to_be_bytes());
    hasher.update((source_owner_id.len() as u64).to_be_bytes());
    hasher.update(source_owner_id.as_bytes());
    hasher.update(fencing_token.to_be_bytes());
    hasher.update((report_bytes.len() as u64).to_be_bytes());
    hasher.update(report_bytes);
    hasher.update((checkpoint_bytes.len() as u64).to_be_bytes());
    hasher.update(checkpoint_bytes);
    Ok(hex_lower(&hasher.finalize()))
}

fn validate_owner_id(owner_id: &str) -> Result<(), String> {
    if owner_id.trim().is_empty() {
        return Err("zone replica source owner id must not be empty".to_string());
    }
    if owner_id.len() > 160 || owner_id.chars().any(char::is_control) {
        return Err("zone replica source owner id is invalid".to_string());
    }
    Ok(())
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        difference |= usize::from(
            left.get(index).copied().unwrap_or_default()
                ^ right.get(index).copied().unwrap_or_default(),
        );
    }
    difference == 0
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
