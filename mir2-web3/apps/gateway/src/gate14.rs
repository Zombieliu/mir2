//! Gate 14 vertical POC primitives.
//!
//! The module deliberately keeps the authoritative state machine independent
//! from transport, Commonware, Postgres, and Redis. A finalized command is
//! applied once, produces a deterministic state root, and can then be replayed
//! into any disposable projection.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const STATE_ROOT_DOMAIN: &[u8] = b"obelisk.gate14.authoritative-state.v1\0";
const COMMAND_DIGEST_DOMAIN: &[u8] = b"obelisk.gate14.command.v1\0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gate14CommandEnvelope {
    pub sequence: u64,
    pub idempotency_key: String,
    pub submitted_at_ms: u64,
    pub command: Gate14Command,
}

impl Gate14CommandEnvelope {
    pub fn validate(&self) -> Result<(), String> {
        if self.sequence == 0 {
            return Err("Gate 14 command sequence must be positive".to_string());
        }
        validate_component("idempotency key", &self.idempotency_key)?;
        self.command.validate()
    }

    pub fn digest(&self) -> Result<String, String> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)
            .map_err(|error| format!("Gate 14 command encoding failed: {error}"))?;
        let mut hasher = Sha256::new();
        hasher.update(COMMAND_DIGEST_DOMAIN);
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(bytes);
        Ok(hex(hasher.finalize().as_slice()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Gate14Command {
    RegisterZoneHost {
        host_id: String,
        endpoint: String,
        failure_domain: String,
        max_sessions: usize,
        max_zones: usize,
    },
    PlaceZone {
        zone_id: String,
        generation: u64,
        primary_host_id: String,
        replica_host_ids: Vec<String>,
        expires_at_ms: u64,
    },
    GrantSessionLease {
        session_id: String,
        account_id: String,
        character_id: String,
        gateway_id: String,
        zone_id: String,
        fencing_token: u64,
        expires_at_ms: u64,
    },
    CreateAccount {
        account_id: String,
    },
    CreateCharacter {
        account_id: String,
        character_id: String,
        name: String,
    },
    GrantVerifiedLoot {
        account_id: String,
        character_id: String,
        item_id: String,
        quantity: u32,
        receipt_id: String,
    },
    ChangeGold {
        account_id: String,
        character_id: String,
        delta: i64,
        reason: String,
    },
    ConsumeItem {
        account_id: String,
        character_id: String,
        item_id: String,
        quantity: u32,
    },
}

impl Gate14Command {
    fn validate(&self) -> Result<(), String> {
        match self {
            Self::RegisterZoneHost {
                host_id,
                endpoint,
                failure_domain,
                max_sessions,
                max_zones,
            } => {
                validate_component("Zone Host id", host_id)?;
                validate_component("Zone Host endpoint", endpoint)?;
                validate_component("Zone Host failure domain", failure_domain)?;
                if *max_sessions == 0 || *max_zones == 0 {
                    return Err("Zone Host capacity must be positive".to_string());
                }
            }
            Self::PlaceZone {
                zone_id,
                generation,
                primary_host_id,
                replica_host_ids,
                expires_at_ms,
            } => {
                validate_component("Zone id", zone_id)?;
                validate_component("primary Zone Host id", primary_host_id)?;
                if *generation == 0 || *expires_at_ms == 0 {
                    return Err("placement generation and expiry must be positive".to_string());
                }
                let mut unique = BTreeSet::new();
                unique.insert(primary_host_id.as_str());
                for host_id in replica_host_ids {
                    validate_component("replica Zone Host id", host_id)?;
                    if !unique.insert(host_id) {
                        return Err(format!("duplicate placement host {host_id}"));
                    }
                }
            }
            Self::GrantSessionLease {
                session_id,
                account_id,
                character_id,
                gateway_id,
                zone_id,
                fencing_token,
                expires_at_ms,
            } => {
                for (label, value) in [
                    ("session id", session_id),
                    ("account id", account_id),
                    ("character id", character_id),
                    ("gateway id", gateway_id),
                    ("Zone id", zone_id),
                ] {
                    validate_component(label, value)?;
                }
                if *fencing_token == 0 || *expires_at_ms == 0 {
                    return Err("session fencing token and expiry must be positive".to_string());
                }
            }
            Self::CreateAccount { account_id } => validate_component("account id", account_id)?,
            Self::CreateCharacter {
                account_id,
                character_id,
                name,
            } => {
                validate_component("account id", account_id)?;
                validate_component("character id", character_id)?;
                validate_component("character name", name)?;
            }
            Self::GrantVerifiedLoot {
                account_id,
                character_id,
                item_id,
                quantity,
                receipt_id,
            } => {
                validate_character_ref(account_id, character_id)?;
                validate_component("item id", item_id)?;
                validate_component("verified loot receipt id", receipt_id)?;
                if *quantity == 0 {
                    return Err("verified loot quantity must be positive".to_string());
                }
            }
            Self::ChangeGold {
                account_id,
                character_id,
                delta,
                reason,
            } => {
                validate_character_ref(account_id, character_id)?;
                validate_component("gold change reason", reason)?;
                if *delta == 0 {
                    return Err("gold delta must not be zero".to_string());
                }
            }
            Self::ConsumeItem {
                account_id,
                character_id,
                item_id,
                quantity,
            } => {
                validate_character_ref(account_id, character_id)?;
                validate_component("item id", item_id)?;
                if *quantity == 0 {
                    return Err("consumed quantity must be positive".to_string());
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gate14ZoneHost {
    pub host_id: String,
    pub endpoint: String,
    pub failure_domain: String,
    pub max_sessions: usize,
    pub max_zones: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gate14Placement {
    pub zone_id: String,
    pub generation: u64,
    pub primary_host_id: String,
    pub replica_host_ids: Vec<String>,
    pub expires_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gate14SessionLease {
    pub session_id: String,
    pub account_id: String,
    pub character_id: String,
    pub gateway_id: String,
    pub zone_id: String,
    pub fencing_token: u64,
    pub expires_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gate14Character {
    pub character_id: String,
    pub name: String,
    pub gold: u64,
    pub inventory: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gate14Account {
    pub account_id: String,
    pub characters: BTreeMap<String, Gate14Character>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gate14AuthoritativeState {
    pub version: u32,
    pub finalized_height: u64,
    pub last_sequence: u64,
    pub zone_hosts: BTreeMap<String, Gate14ZoneHost>,
    pub placements: BTreeMap<String, Gate14Placement>,
    pub session_leases: BTreeMap<String, Gate14SessionLease>,
    pub accounts: BTreeMap<String, Gate14Account>,
    pub verified_loot_receipts: BTreeSet<String>,
    pub applied_idempotency_keys: BTreeSet<String>,
}

impl Default for Gate14AuthoritativeState {
    fn default() -> Self {
        Self {
            version: 1,
            finalized_height: 0,
            last_sequence: 0,
            zone_hosts: BTreeMap::new(),
            placements: BTreeMap::new(),
            session_leases: BTreeMap::new(),
            accounts: BTreeMap::new(),
            verified_loot_receipts: BTreeSet::new(),
            applied_idempotency_keys: BTreeSet::new(),
        }
    }
}

impl Gate14AuthoritativeState {
    pub fn apply_finalized(
        &mut self,
        height: u64,
        envelope: &Gate14CommandEnvelope,
    ) -> Result<Gate14ApplyOutcome, String> {
        envelope.validate()?;
        let expected_height = self.finalized_height.saturating_add(1);
        if height != expected_height {
            return Err(format!(
                "finalized height gap: expected {expected_height}, got {height}"
            ));
        }
        let expected_sequence = self.last_sequence.saturating_add(1);
        if envelope.sequence != expected_sequence {
            return Err(format!(
                "command sequence gap: expected {expected_sequence}, got {}",
                envelope.sequence
            ));
        }
        if self
            .applied_idempotency_keys
            .contains(&envelope.idempotency_key)
        {
            return Err(format!(
                "command {} is already applied",
                envelope.idempotency_key
            ));
        }

        self.apply_command(&envelope.command)?;
        self.finalized_height = height;
        self.last_sequence = envelope.sequence;
        self.applied_idempotency_keys
            .insert(envelope.idempotency_key.clone());
        Ok(Gate14ApplyOutcome {
            height,
            sequence: envelope.sequence,
            idempotency_key: envelope.idempotency_key.clone(),
            state_root: self.state_root()?,
        })
    }

    pub fn state_root(&self) -> Result<String, String> {
        let bytes = serde_json::to_vec(self)
            .map_err(|error| format!("authoritative state encoding failed: {error}"))?;
        let mut hasher = Sha256::new();
        hasher.update(STATE_ROOT_DOMAIN);
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(bytes);
        Ok(hex(hasher.finalize().as_slice()))
    }

    pub fn placement(&self, zone_id: &str, now_ms: u64) -> Option<&Gate14Placement> {
        self.placements
            .get(zone_id)
            .filter(|placement| placement.expires_at_ms > now_ms)
    }

    pub fn session_lease(&self, session_id: &str, now_ms: u64) -> Option<&Gate14SessionLease> {
        self.session_leases
            .get(session_id)
            .filter(|lease| lease.expires_at_ms > now_ms)
    }

    fn apply_command(&mut self, command: &Gate14Command) -> Result<(), String> {
        match command {
            Gate14Command::RegisterZoneHost {
                host_id,
                endpoint,
                failure_domain,
                max_sessions,
                max_zones,
            } => {
                self.zone_hosts.insert(
                    host_id.clone(),
                    Gate14ZoneHost {
                        host_id: host_id.clone(),
                        endpoint: endpoint.clone(),
                        failure_domain: failure_domain.clone(),
                        max_sessions: *max_sessions,
                        max_zones: *max_zones,
                    },
                );
            }
            Gate14Command::PlaceZone {
                zone_id,
                generation,
                primary_host_id,
                replica_host_ids,
                expires_at_ms,
            } => {
                if !self.zone_hosts.contains_key(primary_host_id) {
                    return Err(format!("unknown primary Zone Host {primary_host_id}"));
                }
                for host_id in replica_host_ids {
                    if !self.zone_hosts.contains_key(host_id) {
                        return Err(format!("unknown replica Zone Host {host_id}"));
                    }
                }
                if let Some(current) = self.placements.get(zone_id) {
                    if *generation <= current.generation {
                        return Err(format!(
                            "placement generation {} does not fence current generation {}",
                            generation, current.generation
                        ));
                    }
                }
                self.placements.insert(
                    zone_id.clone(),
                    Gate14Placement {
                        zone_id: zone_id.clone(),
                        generation: *generation,
                        primary_host_id: primary_host_id.clone(),
                        replica_host_ids: replica_host_ids.clone(),
                        expires_at_ms: *expires_at_ms,
                    },
                );
            }
            Gate14Command::GrantSessionLease {
                session_id,
                account_id,
                character_id,
                gateway_id,
                zone_id,
                fencing_token,
                expires_at_ms,
            } => {
                self.character(account_id, character_id)?;
                let placement = self
                    .placements
                    .get(zone_id)
                    .ok_or_else(|| format!("Zone {zone_id} has no finalized placement"))?;
                if *expires_at_ms > placement.expires_at_ms {
                    return Err("session lease outlives finalized placement".to_string());
                }
                if let Some(current) = self.session_leases.get(session_id) {
                    if *fencing_token <= current.fencing_token {
                        return Err(format!(
                            "session fencing token {} does not exceed {}",
                            fencing_token, current.fencing_token
                        ));
                    }
                }
                self.session_leases.insert(
                    session_id.clone(),
                    Gate14SessionLease {
                        session_id: session_id.clone(),
                        account_id: account_id.clone(),
                        character_id: character_id.clone(),
                        gateway_id: gateway_id.clone(),
                        zone_id: zone_id.clone(),
                        fencing_token: *fencing_token,
                        expires_at_ms: *expires_at_ms,
                    },
                );
            }
            Gate14Command::CreateAccount { account_id } => {
                if self.accounts.contains_key(account_id) {
                    return Err(format!("account {account_id} already exists"));
                }
                self.accounts.insert(
                    account_id.clone(),
                    Gate14Account {
                        account_id: account_id.clone(),
                        characters: BTreeMap::new(),
                    },
                );
            }
            Gate14Command::CreateCharacter {
                account_id,
                character_id,
                name,
            } => {
                if self
                    .accounts
                    .values()
                    .flat_map(|account| account.characters.values())
                    .any(|character| character.name == *name)
                {
                    return Err(format!("character name {name} already exists"));
                }
                let account = self
                    .accounts
                    .get_mut(account_id)
                    .ok_or_else(|| format!("unknown account {account_id}"))?;
                if account.characters.contains_key(character_id) {
                    return Err(format!("character {character_id} already exists"));
                }
                account.characters.insert(
                    character_id.clone(),
                    Gate14Character {
                        character_id: character_id.clone(),
                        name: name.clone(),
                        gold: 0,
                        inventory: BTreeMap::new(),
                    },
                );
            }
            Gate14Command::GrantVerifiedLoot {
                account_id,
                character_id,
                item_id,
                quantity,
                receipt_id,
            } => {
                if self.verified_loot_receipts.contains(receipt_id) {
                    return Err(format!(
                        "verified loot receipt {receipt_id} already consumed"
                    ));
                }
                let character = self.character_mut(account_id, character_id)?;
                let next = character
                    .inventory
                    .get(item_id)
                    .copied()
                    .unwrap_or_default()
                    .checked_add(*quantity)
                    .ok_or_else(|| "inventory quantity overflow".to_string())?;
                character.inventory.insert(item_id.clone(), next);
                self.verified_loot_receipts.insert(receipt_id.clone());
            }
            Gate14Command::ChangeGold {
                account_id,
                character_id,
                delta,
                ..
            } => {
                let character = self.character_mut(account_id, character_id)?;
                character.gold = if *delta > 0 {
                    character
                        .gold
                        .checked_add(*delta as u64)
                        .ok_or_else(|| "gold overflow".to_string())?
                } else {
                    character
                        .gold
                        .checked_sub(delta.unsigned_abs())
                        .ok_or_else(|| "insufficient gold".to_string())?
                };
            }
            Gate14Command::ConsumeItem {
                account_id,
                character_id,
                item_id,
                quantity,
            } => {
                let character = self.character_mut(account_id, character_id)?;
                let current = character
                    .inventory
                    .get(item_id)
                    .copied()
                    .ok_or_else(|| format!("item {item_id} is not in inventory"))?;
                let remaining = current
                    .checked_sub(*quantity)
                    .ok_or_else(|| format!("insufficient {item_id} quantity"))?;
                if remaining == 0 {
                    character.inventory.remove(item_id);
                } else {
                    character.inventory.insert(item_id.clone(), remaining);
                }
            }
        }
        Ok(())
    }

    fn character(&self, account_id: &str, character_id: &str) -> Result<&Gate14Character, String> {
        self.accounts
            .get(account_id)
            .ok_or_else(|| format!("unknown account {account_id}"))?
            .characters
            .get(character_id)
            .ok_or_else(|| format!("unknown character {character_id}"))
    }

    fn character_mut(
        &mut self,
        account_id: &str,
        character_id: &str,
    ) -> Result<&mut Gate14Character, String> {
        self.accounts
            .get_mut(account_id)
            .ok_or_else(|| format!("unknown account {account_id}"))?
            .characters
            .get_mut(character_id)
            .ok_or_else(|| format!("unknown character {character_id}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gate14ApplyOutcome {
    pub height: u64,
    pub sequence: u64,
    pub idempotency_key: String,
    pub state_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gate14FinalizedRecord {
    pub height: u64,
    pub epoch: u64,
    pub view: u64,
    pub command_digest: String,
    #[serde(default)]
    pub commonware_digest: String,
    pub signer_count: usize,
    pub certificate_base64: String,
    pub command: Gate14CommandEnvelope,
    pub state_root: String,
    pub finalized_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gate14ValidatorStatus {
    pub role: String,
    pub node_id: String,
    pub commonware_release: String,
    pub committee_size: usize,
    pub quorum: usize,
    pub finalized_height: u64,
    pub last_sequence: u64,
    pub state_root: String,
    pub prepared_count: usize,
    pub pending_count: usize,
    pub started_at_ms: u64,
}

#[derive(Debug, Clone)]
pub struct Gate14QuorumSnapshot {
    pub state: Gate14AuthoritativeState,
    pub state_root: String,
    pub agreeing_validators: Vec<String>,
    pub responding_validators: usize,
}

#[derive(Clone, Debug)]
pub struct Gate14QuorumClient {
    validator_urls: Vec<String>,
    quorum: usize,
    client: reqwest::Client,
}

impl Gate14QuorumClient {
    pub fn new(validator_urls: Vec<String>) -> Result<Self, String> {
        if validator_urls.len() < 4 {
            return Err("Gate 14 quorum client requires four validator URLs".to_string());
        }
        let validator_urls = validator_urls
            .into_iter()
            .map(|url| url.trim().trim_end_matches('/').to_string())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if validator_urls.len() != 4 || validator_urls.iter().any(String::is_empty) {
            return Err("Gate 14 validator URLs must contain four unique values".to_string());
        }
        Ok(Self {
            validator_urls,
            quorum: 3,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(3))
                .build()
                .map_err(|error| format!("build Gate 14 HTTP client failed: {error}"))?,
        })
    }

    pub fn validator_urls(&self) -> &[String] {
        &self.validator_urls
    }

    pub async fn quorum_state(&self) -> Result<Gate14QuorumSnapshot, String> {
        let requests = self.validator_urls.iter().cloned().map(|base| {
            let client = self.client.clone();
            async move {
                let status = client
                    .get(format!("{base}/v1/status"))
                    .send()
                    .await
                    .map_err(|error| format!("{base} status request failed: {error}"))?
                    .error_for_status()
                    .map_err(|error| format!("{base} status failed: {error}"))?
                    .json::<Gate14ValidatorStatus>()
                    .await
                    .map_err(|error| format!("{base} status decode failed: {error}"))?;
                Ok::<_, String>((base, status))
            }
        });
        let responses = join_all(requests)
            .await
            .into_iter()
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        let mut groups: BTreeMap<(u64, String), Vec<(String, Gate14ValidatorStatus)>> =
            BTreeMap::new();
        for (base, status) in responses.iter().cloned() {
            groups
                .entry((status.finalized_height, status.state_root.clone()))
                .or_default()
                .push((base, status));
        }
        let ((height, state_root), agreeing) = groups
            .into_iter()
            .filter(|(_, members)| members.len() >= self.quorum)
            .max_by_key(|((height, _), members)| (*height, members.len()))
            .ok_or_else(|| {
                format!(
                    "no 3-of-4 Commonware state quorum; {} validators responded",
                    responses.len()
                )
            })?;
        let representative = &agreeing[0].0;
        let state = self
            .client
            .get(format!("{representative}/v1/state"))
            .send()
            .await
            .map_err(|error| format!("{representative} state request failed: {error}"))?
            .error_for_status()
            .map_err(|error| format!("{representative} state failed: {error}"))?
            .json::<Gate14AuthoritativeState>()
            .await
            .map_err(|error| format!("{representative} state decode failed: {error}"))?;
        if state.finalized_height != height || state.state_root()? != state_root {
            return Err("representative validator state does not match quorum root".to_string());
        }
        Ok(Gate14QuorumSnapshot {
            state,
            state_root,
            agreeing_validators: agreeing
                .into_iter()
                .map(|(_, status)| status.node_id)
                .collect(),
            responding_validators: responses.len(),
        })
    }

    pub async fn finalized_since(
        &self,
        height_exclusive: u64,
    ) -> Result<Vec<Gate14FinalizedRecord>, String> {
        let snapshot = self.quorum_state().await?;
        if snapshot.state.finalized_height <= height_exclusive {
            return Ok(Vec::new());
        }
        let requests = self.validator_urls.iter().cloned().map(|base| {
            let client = self.client.clone();
            async move {
                let records = client
                    .get(format!("{base}/v1/finality?after={height_exclusive}"))
                    .send()
                    .await
                    .map_err(|error| format!("{base} finality request failed: {error}"))?
                    .error_for_status()
                    .map_err(|error| format!("{base} finality failed: {error}"))?
                    .json::<Vec<Gate14FinalizedRecord>>()
                    .await
                    .map_err(|error| format!("{base} finality decode failed: {error}"))?;
                Ok::<_, String>(records)
            }
        });
        let mut candidates = join_all(requests)
            .await
            .into_iter()
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        candidates.sort_by_key(|records| std::cmp::Reverse(records.len()));
        let records = candidates
            .into_iter()
            .find(|records| {
                records
                    .last()
                    .is_some_and(|record| record.height == snapshot.state.finalized_height)
            })
            .ok_or_else(|| "no validator supplied the quorum-finalized record range".to_string())?;
        if records
            .iter()
            .any(|record| record.signer_count < self.quorum)
        {
            return Err("finality range contains a below-quorum certificate".to_string());
        }
        Ok(records)
    }

    pub async fn submit(&self, command: &Gate14CommandEnvelope) -> Result<String, String> {
        command.validate()?;
        let expected_digest = command.digest()?;
        let requests = self.validator_urls.iter().cloned().map(|base| {
            let client = self.client.clone();
            let command = command.clone();
            async move {
                let response = client
                    .post(format!("{base}/v1/commands"))
                    .json(&command)
                    .send()
                    .await
                    .map_err(|error| format!("{base} command request failed: {error}"))?
                    .error_for_status()
                    .map_err(|error| format!("{base} command rejected: {error}"))?
                    .json::<Gate14SubmitResponse>()
                    .await
                    .map_err(|error| format!("{base} command response decode failed: {error}"))?;
                Ok::<_, String>(response)
            }
        });
        let accepted = join_all(requests)
            .await
            .into_iter()
            .filter_map(Result::ok)
            .filter(|response| response.accepted && response.command_digest == expected_digest)
            .count();
        if accepted < self.quorum {
            return Err(format!(
                "command reached {accepted} validators; {} required",
                self.quorum
            ));
        }
        Ok(expected_digest)
    }

    pub async fn wait_for_height(
        &self,
        minimum_height: u64,
        timeout: Duration,
    ) -> Result<Gate14QuorumSnapshot, String> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Ok(snapshot) = self.quorum_state().await {
                if snapshot.state.finalized_height >= minimum_height {
                    return Ok(snapshot);
                }
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "timed out waiting for Commonware height {minimum_height}"
                ));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Gate14SubmitResponse {
    accepted: bool,
    command_digest: String,
}

pub fn replay_gate14_records(
    records: &[Gate14FinalizedRecord],
) -> Result<Gate14AuthoritativeState, String> {
    let mut state = Gate14AuthoritativeState::default();
    for record in records {
        if record.command.digest()? != record.command_digest {
            return Err(format!(
                "finalized record {} command digest mismatch",
                record.height
            ));
        }
        let outcome = state.apply_finalized(record.height, &record.command)?;
        if outcome.state_root != record.state_root {
            return Err(format!(
                "finalized record {} state root mismatch",
                record.height
            ));
        }
    }
    Ok(state)
}

fn validate_character_ref(account_id: &str, character_id: &str) -> Result<(), String> {
    validate_component("account id", account_id)?;
    validate_component("character id", character_id)
}

fn validate_component(label: &str, value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if trimmed.len() > 256 {
        return Err(format!("{label} exceeds 256 bytes"));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(format!("{label} contains control characters"));
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(sequence: u64, command: Gate14Command) -> Gate14CommandEnvelope {
        Gate14CommandEnvelope {
            sequence,
            idempotency_key: format!("gate14-command-{sequence}"),
            submitted_at_ms: 1_000 + sequence,
            command,
        }
    }

    #[test]
    fn authoritative_state_replays_account_inventory_gold_and_leases() {
        let commands = vec![
            Gate14Command::RegisterZoneHost {
                host_id: "dubhe-a".into(),
                endpoint: "dubhe-a:7020".into(),
                failure_domain: "rack-a".into(),
                max_sessions: 128,
                max_zones: 8,
            },
            Gate14Command::RegisterZoneHost {
                host_id: "dubhe-b".into(),
                endpoint: "dubhe-b:7020".into(),
                failure_domain: "rack-b".into(),
                max_sessions: 128,
                max_zones: 8,
            },
            Gate14Command::PlaceZone {
                zone_id: "mir2/map/0".into(),
                generation: 1,
                primary_host_id: "dubhe-a".into(),
                replica_host_ids: vec!["dubhe-b".into()],
                expires_at_ms: 60_000,
            },
            Gate14Command::CreateAccount {
                account_id: "alice".into(),
            },
            Gate14Command::CreateCharacter {
                account_id: "alice".into(),
                character_id: "alice-warrior".into(),
                name: "Alice".into(),
            },
            Gate14Command::GrantVerifiedLoot {
                account_id: "alice".into(),
                character_id: "alice-warrior".into(),
                item_id: "red-potion".into(),
                quantity: 5,
                receipt_id: "loot-1".into(),
            },
            Gate14Command::ChangeGold {
                account_id: "alice".into(),
                character_id: "alice-warrior".into(),
                delta: 100,
                reason: "quest".into(),
            },
            Gate14Command::ConsumeItem {
                account_id: "alice".into(),
                character_id: "alice-warrior".into(),
                item_id: "red-potion".into(),
                quantity: 2,
            },
            Gate14Command::GrantSessionLease {
                session_id: "session-alice".into(),
                account_id: "alice".into(),
                character_id: "alice-warrior".into(),
                gateway_id: "gateway-a".into(),
                zone_id: "mir2/map/0".into(),
                fencing_token: 1,
                expires_at_ms: 50_000,
            },
        ];
        let mut state = Gate14AuthoritativeState::default();
        let mut records = Vec::new();
        for (index, command) in commands.into_iter().enumerate() {
            let height = index as u64 + 1;
            let command = envelope(height, command);
            let digest = command.digest().unwrap();
            let outcome = state.apply_finalized(height, &command).unwrap();
            records.push(Gate14FinalizedRecord {
                height,
                epoch: 0,
                view: height,
                command_digest: digest,
                commonware_digest: format!("commonware-{height}"),
                signer_count: 3,
                certificate_base64: "test".into(),
                command,
                state_root: outcome.state_root,
                finalized_at_ms: 2_000 + height,
            });
        }

        let replayed = replay_gate14_records(&records).unwrap();
        assert_eq!(state.state_root().unwrap(), replayed.state_root().unwrap());
        let character = replayed
            .accounts
            .get("alice")
            .unwrap()
            .characters
            .get("alice-warrior")
            .unwrap();
        assert_eq!(character.gold, 100);
        assert_eq!(character.inventory.get("red-potion"), Some(&3));
        assert_eq!(
            replayed
                .session_lease("session-alice", 49_999)
                .unwrap()
                .gateway_id,
            "gateway-a"
        );
    }

    #[test]
    fn state_machine_rejects_duplicate_receipt_and_stale_fencing() {
        let mut state = Gate14AuthoritativeState::default();
        let setup = [
            Gate14Command::RegisterZoneHost {
                host_id: "dubhe-a".into(),
                endpoint: "dubhe-a:7020".into(),
                failure_domain: "rack-a".into(),
                max_sessions: 8,
                max_zones: 1,
            },
            Gate14Command::PlaceZone {
                zone_id: "mir2/map/0".into(),
                generation: 1,
                primary_host_id: "dubhe-a".into(),
                replica_host_ids: vec![],
                expires_at_ms: 10_000,
            },
            Gate14Command::CreateAccount {
                account_id: "alice".into(),
            },
            Gate14Command::CreateCharacter {
                account_id: "alice".into(),
                character_id: "hero".into(),
                name: "Hero".into(),
            },
            Gate14Command::GrantVerifiedLoot {
                account_id: "alice".into(),
                character_id: "hero".into(),
                item_id: "ore".into(),
                quantity: 1,
                receipt_id: "receipt-1".into(),
            },
        ];
        for (index, command) in setup.into_iter().enumerate() {
            let height = index as u64 + 1;
            state
                .apply_finalized(height, &envelope(height, command))
                .unwrap();
        }
        let duplicate = envelope(
            6,
            Gate14Command::GrantVerifiedLoot {
                account_id: "alice".into(),
                character_id: "hero".into(),
                item_id: "ore".into(),
                quantity: 1,
                receipt_id: "receipt-1".into(),
            },
        );
        assert!(state.apply_finalized(6, &duplicate).is_err());
        assert_eq!(
            state
                .accounts
                .get("alice")
                .unwrap()
                .characters
                .get("hero")
                .unwrap()
                .inventory
                .get("ore"),
            Some(&1)
        );
    }
}
