//! Gate 15 production-path integration for the Gate 14 Commonware control state.
//!
//! Gate 14 proved the replicated state machine with purpose-built POC
//! processes. Gate 15 installs the same finalized state in the real Gateway and
//! Zone Host processes:
//!
//! - real player StartGame acquires a quorum-finalized session lease;
//! - Zone RPC endpoints are resolved from a quorum-finalized placement;
//! - the placement primary and generation become the Zone owner fencing lease;
//! - a background observer keeps long-lived sessions on the latest generation.

use std::env;
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mir2_protocol::ServerPacket;
use mir2_simulation::{ActiveSessionIdentity, WorldCommandExecution, WorldSnapshot};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::gate14::{
    Gate14AuthoritativeState, Gate14Command, Gate14CommandEnvelope, Gate14QuorumClient,
    Gate14QuorumSnapshot, Gate14SessionLease,
};
use crate::routing::{
    SharedZoneOwnerLeaseAuthority, SharedZoneOwnerRpcTransport, ZoneId,
    ZoneLiveOutboundRegistration, ZoneOwnerCommandRequest, ZoneOwnerLease, ZoneOwnerLeaseAuthority,
    ZoneOwnerRpcTransport,
};
use crate::zone_rpc::{TcpZoneOwnerRpcTransport, ZoneRpcLimits};
use crate::{ZonePlacementEndpoint, ZonePlacementLease};

const DEFAULT_SESSION_LEASE_TTL_MS: u64 = 60_000;
const DEFAULT_OBSERVER_INTERVAL_MS: u64 = 200;
const INITIAL_QUORUM_TIMEOUT: Duration = Duration::from_secs(15);
const COMMAND_FINALITY_TIMEOUT: Duration = Duration::from_secs(15);

static GATE15_CONTROL_PLANE: OnceLock<Arc<Gate15ControlPlane>> = OnceLock::new();

#[derive(Debug, Clone)]
struct Gate15ObservedState {
    state: Gate14AuthoritativeState,
    state_root: String,
    agreeing_validators: Vec<String>,
    responding_validators: usize,
    observed_at_ms: u64,
    last_error: Option<String>,
}

impl Default for Gate15ObservedState {
    fn default() -> Self {
        Self {
            state: Gate14AuthoritativeState::default(),
            state_root: String::new(),
            agreeing_validators: Vec::new(),
            responding_validators: 0,
            observed_at_ms: 0,
            last_error: Some("waiting for Commonware quorum".to_string()),
        }
    }
}

#[derive(Debug)]
struct Gate15ControlPlane {
    gateway_id: String,
    quorum: Gate14QuorumClient,
    observed: RwLock<Gate15ObservedState>,
    submit_lock: tokio::sync::Mutex<()>,
    session_lease_ttl_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gate15Health {
    pub enabled: bool,
    pub healthy: bool,
    pub gateway_id: String,
    pub finalized_height: u64,
    pub state_root: String,
    pub agreeing_validators: Vec<String>,
    pub responding_validators: usize,
    pub placement_count: usize,
    pub session_lease_count: usize,
    pub observed_at_ms: u64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gate15PlayerLease {
    pub finalized_height: u64,
    pub state_root: String,
    pub lease: Gate14SessionLease,
    pub placement: ZonePlacementLease,
}

/// Start the Commonware observer when Gate 15 is configured.
///
/// This is synchronous by design: both the Tokio Gateway binary and the
/// synchronous Zone Host binary call it before accepting traffic. Network work
/// lives on a dedicated observer thread, so no nested runtime is created on a
/// request path.
pub fn initialize_from_env() -> Result<bool, String> {
    if GATE15_CONTROL_PLANE.get().is_some() {
        return Ok(true);
    }
    let Some(validator_urls) = validator_urls_from_env()? else {
        return Ok(false);
    };
    let gateway_id = env::var("MIR2_GATE15_GATEWAY_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            env::var("MIR2_GATEWAY_INSTANCE_ID")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| format!("gateway-{}", std::process::id()));
    let session_lease_ttl_ms = env::var("MIR2_GATE15_SESSION_LEASE_TTL_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_SESSION_LEASE_TTL_MS)
        .clamp(5_000, 300_000);
    let observer_interval_ms = env::var("MIR2_GATE15_OBSERVER_INTERVAL_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_OBSERVER_INTERVAL_MS)
        .clamp(50, 10_000);
    let control = Arc::new(Gate15ControlPlane {
        gateway_id,
        quorum: Gate14QuorumClient::new(validator_urls)?,
        observed: RwLock::new(Gate15ObservedState::default()),
        submit_lock: tokio::sync::Mutex::new(()),
        session_lease_ttl_ms,
    });
    let observer = Arc::clone(&control);
    let (initial_tx, initial_rx) = std::sync::mpsc::sync_channel(1);
    thread::Builder::new()
        .name("mir2-gate15-commonware-observer".to_string())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = initial_tx.send(Err(format!(
                        "build Gate 15 observer runtime failed: {error}"
                    )));
                    return;
                }
            };
            runtime.block_on(async move {
                let first = observer.refresh().await;
                let _ = initial_tx.send(first.clone().map(|_| ()));
                if first.is_err() {
                    return;
                }
                loop {
                    tokio::time::sleep(Duration::from_millis(observer_interval_ms)).await;
                    let _ = observer.refresh().await;
                }
            });
        })
        .map_err(|error| format!("spawn Gate 15 observer failed: {error}"))?;
    initial_rx
        .recv_timeout(INITIAL_QUORUM_TIMEOUT)
        .map_err(|error| format!("Gate 15 initial quorum timed out: {error}"))??;
    GATE15_CONTROL_PLANE
        .set(control)
        .map_err(|_| "Gate 15 control plane was initialized concurrently".to_string())?;
    let health = health().expect("Gate 15 health exists after initialization");
    eprintln!(
        "Gate 15 Commonware control enabled: gateway={} height={} quorum={}",
        health.gateway_id,
        health.finalized_height,
        health.agreeing_validators.len()
    );
    Ok(true)
}

pub fn health() -> Option<Gate15Health> {
    let control = GATE15_CONTROL_PLANE.get()?;
    let observed = control
        .observed
        .read()
        .expect("Gate 15 observed state lock should not be poisoned");
    Some(Gate15Health {
        enabled: true,
        healthy: observed.last_error.is_none() && observed.agreeing_validators.len() >= 3,
        gateway_id: control.gateway_id.clone(),
        finalized_height: observed.state.finalized_height,
        state_root: observed.state_root.clone(),
        agreeing_validators: observed.agreeing_validators.clone(),
        responding_validators: observed.responding_validators,
        placement_count: observed.state.placements.len(),
        session_lease_count: observed.state.session_leases.len(),
        observed_at_ms: observed.observed_at_ms,
        last_error: observed.last_error.clone(),
    })
}

pub fn zone_owner_lease_authority() -> Option<SharedZoneOwnerLeaseAuthority> {
    let control = Arc::clone(GATE15_CONTROL_PLANE.get()?);
    let local_host_id = env::var("MIR2_GATE15_LOCAL_ZONE_HOST_ID")
        .ok()
        .filter(|value| !value.trim().is_empty());
    Some(Arc::new(Gate15ZoneOwnerLeaseAuthority {
        control,
        local_host_id,
    }) as SharedZoneOwnerLeaseAuthority)
}

pub fn zone_owner_rpc_transport(zone_id: ZoneId) -> Option<SharedZoneOwnerRpcTransport> {
    let control = Arc::clone(GATE15_CONTROL_PLANE.get()?);
    let auth_token = env::var("MIR2_ZONE_HOST_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty());
    Some(Arc::new(Gate15ZoneOwnerRpcTransport {
        control,
        zone_id,
        rpc_session_id: next_gate15_rpc_session_id(),
        auth_token,
        limits: ZoneRpcLimits::from_env(),
        active: Mutex::new(None),
    }) as SharedZoneOwnerRpcTransport)
}

pub async fn acquire_player_session(
    account_id: &str,
    character_index: i32,
    zone_id: &ZoneId,
) -> Result<Option<Gate15PlayerLease>, String> {
    let Some(control) = GATE15_CONTROL_PLANE.get() else {
        return Ok(None);
    };
    control
        .acquire_player_session(account_id, character_index, zone_id)
        .await
        .map(Some)
}

/// Ensure identities accepted by the authoritative account repository are also
/// quorum-finalized before the player reaches StartGame.
///
/// Gate 15 was originally activated after a one-time legacy identity migration.
/// Without this write-through path, accounts created after that cutover can
/// authenticate and create characters but can never acquire a player lease.
pub async fn finalize_player_identities(
    account_id: &str,
    characters: &[(i32, String)],
) -> Result<Option<u64>, String> {
    let Some(control) = GATE15_CONTROL_PLANE.get() else {
        return Ok(None);
    };
    control
        .finalize_player_identities(account_id, characters)
        .await
        .map(Some)
}

pub fn inspect_player_session(
    account_id: &str,
    character_index: i32,
    zone_id: &ZoneId,
) -> Result<Option<Gate15PlayerLease>, String> {
    let Some(control) = GATE15_CONTROL_PLANE.get() else {
        return Ok(None);
    };
    let observed = control
        .observed
        .read()
        .map_err(|_| "Gate 15 observed state lock poisoned".to_string())?;
    let session_id = player_session_id(account_id, character_index);
    let Some(lease) = observed.state.session_lease(&session_id, now_ms()).cloned() else {
        return Ok(None);
    };
    let placement = placement_from_state(&observed.state, zone_id, now_ms())?;
    Ok(Some(Gate15PlayerLease {
        finalized_height: observed.state.finalized_height,
        state_root: observed.state_root.clone(),
        lease,
        placement,
    }))
}

impl Gate15ControlPlane {
    async fn refresh(&self) -> Result<(), String> {
        match self.quorum.quorum_state().await {
            Ok(snapshot) => {
                self.install_snapshot(snapshot);
                Ok(())
            }
            Err(error) => {
                let mut observed = self
                    .observed
                    .write()
                    .expect("Gate 15 observed state lock should not be poisoned");
                observed.observed_at_ms = now_ms();
                observed.last_error = Some(error.clone());
                Err(error)
            }
        }
    }

    fn install_snapshot(&self, snapshot: Gate14QuorumSnapshot) {
        *self
            .observed
            .write()
            .expect("Gate 15 observed state lock should not be poisoned") = Gate15ObservedState {
            state: snapshot.state,
            state_root: snapshot.state_root,
            agreeing_validators: snapshot.agreeing_validators,
            responding_validators: snapshot.responding_validators,
            observed_at_ms: now_ms(),
            last_error: None,
        };
    }

    fn placement(&self, zone_id: &ZoneId) -> Result<ZonePlacementLease, String> {
        let observed = self
            .observed
            .read()
            .map_err(|_| "Gate 15 observed state lock poisoned".to_string())?;
        placement_from_state(&observed.state, zone_id, now_ms())
    }

    fn owner_lease(&self, zone_id: &ZoneId) -> Result<ZoneOwnerLease, String> {
        let placement = self.placement(zone_id)?;
        Ok(ZoneOwnerLease::new(
            zone_id.clone(),
            placement.primary.host_id,
            placement.generation,
        ))
    }

    async fn acquire_player_session(
        &self,
        account_id: &str,
        character_index: i32,
        zone_id: &ZoneId,
    ) -> Result<Gate15PlayerLease, String> {
        if account_id.trim().is_empty() {
            return Err("Gate 15 account id is required".to_string());
        }
        if character_index < 0 {
            return Err("Gate 15 character index must be non-negative".to_string());
        }
        let _guard = self.submit_lock.lock().await;
        for attempt in 0..3 {
            let snapshot = self.quorum.quorum_state().await?;
            self.install_snapshot(snapshot.clone());
            let placement =
                placement_from_state(&snapshot.state, zone_id, now_ms()).map_err(|error| {
                    format!(
                        "Gate 15 StartGame placement rejected for {account_id}/{character_index}: {error}"
                    )
                })?;
            let character_id =
                character_id_for_index(&snapshot.state, account_id, character_index)?;
            let session_id = player_session_id(account_id, character_index);
            let fencing_token = snapshot
                .state
                .session_leases
                .get(&session_id)
                .map(|lease| lease.fencing_token)
                .unwrap_or(0)
                .saturating_add(1)
                .max(1);
            let expires_at_ms = now_ms()
                .saturating_add(self.session_lease_ttl_ms)
                .min(placement.expires_at_ms);
            if expires_at_ms <= now_ms() {
                return Err(format!(
                    "Gate 15 placement {} expires before a player lease can be granted",
                    zone_id
                ));
            }
            let sequence = snapshot.state.last_sequence.saturating_add(1);
            let command = Gate14CommandEnvelope {
                sequence,
                idempotency_key: format!(
                    "gate15-session:{session_id}:{fencing_token}:{}",
                    self.gateway_id
                ),
                submitted_at_ms: now_ms(),
                command: Gate14Command::GrantSessionLease {
                    session_id: session_id.clone(),
                    account_id: account_id.to_string(),
                    character_id,
                    gateway_id: self.gateway_id.clone(),
                    zone_id: zone_id.as_str().to_string(),
                    fencing_token,
                    expires_at_ms,
                },
            };
            match self.quorum.submit(&command).await {
                Ok(_) => {
                    let finalized = self
                        .quorum
                        .wait_for_height(sequence, COMMAND_FINALITY_TIMEOUT)
                        .await?;
                    let lease = finalized
                        .state
                        .session_leases
                        .get(&session_id)
                        .cloned()
                        .ok_or_else(|| {
                            format!("Gate 15 finalized height {sequence} has no lease {session_id}")
                        })?;
                    let placement = placement_from_state(&finalized.state, zone_id, now_ms())?;
                    let result = Gate15PlayerLease {
                        finalized_height: finalized.state.finalized_height,
                        state_root: finalized.state_root.clone(),
                        lease,
                        placement,
                    };
                    self.install_snapshot(finalized);
                    return Ok(result);
                }
                Err(error) if attempt < 2 => {
                    eprintln!(
                        "Gate 15 session command sequence {sequence} raced, retrying: {error}"
                    );
                }
                Err(error) => return Err(error),
            }
        }
        Err("Gate 15 session lease retries exhausted".to_string())
    }

    async fn finalize_player_identities(
        &self,
        account_id: &str,
        characters: &[(i32, String)],
    ) -> Result<u64, String> {
        let account_id = account_id.trim();
        if account_id.is_empty() {
            return Err("Gate 15 account id is required".to_string());
        }
        if let Some((index, _)) = characters.iter().find(|(index, _)| *index < 0) {
            return Err(format!(
                "Gate 15 character index must be non-negative, got {index}"
            ));
        }
        if characters.iter().any(|(_, name)| name.trim().is_empty()) {
            return Err("Gate 15 character name is required".to_string());
        }

        let _guard = self.submit_lock.lock().await;
        let mut finalized_height = self.ensure_account_finalized(account_id).await?;
        for (character_index, name) in characters {
            finalized_height = self
                .ensure_character_finalized(account_id, *character_index, name)
                .await?;
        }
        Ok(finalized_height)
    }

    async fn ensure_account_finalized(&self, account_id: &str) -> Result<u64, String> {
        for attempt in 0..3 {
            let snapshot = self.quorum.quorum_state().await?;
            self.install_snapshot(snapshot.clone());
            if snapshot.state.accounts.contains_key(account_id) {
                return Ok(snapshot.state.finalized_height);
            }
            let sequence = snapshot.state.last_sequence.saturating_add(1);
            let command = Gate14CommandEnvelope {
                sequence,
                idempotency_key: format!(
                    "gate15-identity-account:{account_id}:{sequence}:{}",
                    self.gateway_id
                ),
                submitted_at_ms: now_ms(),
                command: Gate14Command::CreateAccount {
                    account_id: account_id.to_string(),
                },
            };
            match self.quorum.submit(&command).await {
                Ok(_) => {
                    let finalized = self
                        .quorum
                        .wait_for_height(sequence, COMMAND_FINALITY_TIMEOUT)
                        .await?;
                    if !finalized.state.accounts.contains_key(account_id) {
                        return Err(format!(
                            "Gate 15 finalized height {sequence} has no account {account_id}"
                        ));
                    }
                    let height = finalized.state.finalized_height;
                    self.install_snapshot(finalized);
                    return Ok(height);
                }
                Err(error) if attempt < 2 => {
                    eprintln!(
                        "Gate 15 account finalization sequence {sequence} raced, retrying: {error}"
                    );
                }
                Err(error) => return Err(error),
            }
        }
        Err(format!(
            "Gate 15 account finalization retries exhausted for {account_id}"
        ))
    }

    async fn ensure_character_finalized(
        &self,
        account_id: &str,
        character_index: i32,
        name: &str,
    ) -> Result<u64, String> {
        let character_id = format!("{account_id}:{character_index}");
        for attempt in 0..3 {
            let snapshot = self.quorum.quorum_state().await?;
            self.install_snapshot(snapshot.clone());
            let Some(control_name) =
                character_finalization_plan(&snapshot.state, account_id, character_index, name)?
            else {
                return Ok(snapshot.state.finalized_height);
            };
            let sequence = snapshot.state.last_sequence.saturating_add(1);
            let command = Gate14CommandEnvelope {
                sequence,
                idempotency_key: format!(
                    "gate15-identity-character:{character_id}:{sequence}:{}",
                    self.gateway_id
                ),
                submitted_at_ms: now_ms(),
                command: Gate14Command::CreateCharacter {
                    account_id: account_id.to_string(),
                    character_id: character_id.clone(),
                    name: control_name.clone(),
                },
            };
            match self.quorum.submit(&command).await {
                Ok(_) => {
                    let finalized = self
                        .quorum
                        .wait_for_height(sequence, COMMAND_FINALITY_TIMEOUT)
                        .await?;
                    let finalized_name = finalized
                        .state
                        .accounts
                        .get(account_id)
                        .and_then(|account| account.characters.get(&character_id))
                        .map(|character| character.name.as_str());
                    if finalized_name != Some(control_name.as_str()) {
                        return Err(format!(
                            "Gate 15 finalized height {sequence} has no matching character {character_id}"
                        ));
                    }
                    let height = finalized.state.finalized_height;
                    self.install_snapshot(finalized);
                    return Ok(height);
                }
                Err(error) if attempt < 2 => {
                    eprintln!(
                        "Gate 15 character finalization sequence {sequence} raced, retrying: {error}"
                    );
                }
                Err(error) => return Err(error),
            }
        }
        Err(format!(
            "Gate 15 character finalization retries exhausted for {character_id}"
        ))
    }
}

/// Plan the control-plane identity for an account-store character slot.
///
/// The immutable identity is `account_id:character_index`. `name` is control
/// metadata and may differ from the player-facing display name because the
/// legacy migration deterministically disambiguated duplicate names.
fn character_finalization_plan(
    state: &Gate14AuthoritativeState,
    account_id: &str,
    character_index: i32,
    display_name: &str,
) -> Result<Option<String>, String> {
    let account = state
        .accounts
        .get(account_id)
        .ok_or_else(|| format!("Gate 15 finalized account {account_id} disappeared"))?;
    let character_id = format!("{account_id}:{character_index}");
    if account.characters.contains_key(&character_id) {
        return Ok(None);
    }

    let name_is_available = !state
        .accounts
        .values()
        .flat_map(|account| account.characters.values())
        .any(|character| character.name == display_name);
    if name_is_available {
        return Ok(Some(display_name.to_string()));
    }

    let digest = Sha256::digest(character_id.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    for digest_length in [8_usize, 16, 32, 64] {
        let suffix = format!("~{}", &digest[..digest_length]);
        let maximum_prefix_bytes = 256 - suffix.len();
        let mut prefix_end = display_name.len().min(maximum_prefix_bytes);
        while !display_name.is_char_boundary(prefix_end) {
            prefix_end -= 1;
        }
        let candidate = format!("{}{}", &display_name[..prefix_end], suffix);
        let candidate_is_available = !state
            .accounts
            .values()
            .flat_map(|account| account.characters.values())
            .any(|character| character.name == candidate);
        if candidate_is_available {
            return Ok(Some(candidate));
        }
    }

    Err(format!(
        "Gate 15 could not derive a unique control name for character {character_id}"
    ))
}

#[derive(Debug)]
struct Gate15ZoneOwnerLeaseAuthority {
    control: Arc<Gate15ControlPlane>,
    local_host_id: Option<String>,
}

impl ZoneOwnerLeaseAuthority for Gate15ZoneOwnerLeaseAuthority {
    fn owner_lease(&self, zone_id: &ZoneId) -> ZoneOwnerLease {
        self.control
            .owner_lease(zone_id)
            .unwrap_or_else(|_| ZoneOwnerLease::new(zone_id.clone(), "gate15-unplaced", u64::MAX))
    }

    fn validate_owner_lease(&self, lease: &ZoneOwnerLease) -> Result<(), String> {
        let current = self.control.owner_lease(lease.zone_id())?;
        if let Some(local_host_id) = self.local_host_id.as_deref() {
            if current.owner_id() != local_host_id {
                return Err(format!(
                    "Zone {} is finalized on {}, local host {} is a standby",
                    lease.zone_id(),
                    current.owner_id(),
                    local_host_id
                ));
            }
        }
        if current != *lease {
            return Err(format!(
                "stale Commonware Zone placement for {}: finalized owner {} generation {}, got owner {} generation {}",
                lease.zone_id(),
                current.owner_id(),
                current.fencing_token(),
                lease.owner_id(),
                lease.fencing_token()
            ));
        }
        Ok(())
    }

    fn renew_owner_lease(&self, lease: &ZoneOwnerLease) -> Result<ZoneOwnerLease, String> {
        self.renew_owner_lease_at(lease, now_ms())
    }

    fn renew_owner_lease_at(
        &self,
        lease: &ZoneOwnerLease,
        _now_ms: u64,
    ) -> Result<ZoneOwnerLease, String> {
        let current = self.control.owner_lease(lease.zone_id())?;
        if self.local_host_id.is_some() {
            self.validate_owner_lease(lease)?;
            return Ok(current);
        }
        // A real Gateway adopts a newly finalized placement generation instead
        // of dropping the player. The next Zone RPC is fenced by this token.
        Ok(current)
    }
}

struct ActivePlacementTransport {
    generation: u64,
    endpoint_signature: String,
    transport: TcpZoneOwnerRpcTransport,
}

struct Gate15ZoneOwnerRpcTransport {
    control: Arc<Gate15ControlPlane>,
    zone_id: ZoneId,
    rpc_session_id: String,
    auth_token: Option<String>,
    limits: ZoneRpcLimits,
    active: Mutex<Option<ActivePlacementTransport>>,
}

impl fmt::Debug for Gate15ZoneOwnerRpcTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gate15ZoneOwnerRpcTransport")
            .field("zone_id", &self.zone_id)
            .field("rpc_session_id", &self.rpc_session_id)
            .finish_non_exhaustive()
    }
}

impl Gate15ZoneOwnerRpcTransport {
    fn with_transport<T>(
        &self,
        operation: impl FnOnce(&TcpZoneOwnerRpcTransport) -> Result<T, String>,
    ) -> Result<T, String> {
        let placement = self.control.placement(&self.zone_id)?;
        let endpoint_signature = placement.endpoints().join("\0");
        let mut active = self
            .active
            .lock()
            .map_err(|_| "Gate 15 Zone RPC transport lock poisoned".to_string())?;
        let replace = active.as_ref().is_none_or(|current| {
            current.generation != placement.generation
                || current.endpoint_signature != endpoint_signature
        });
        if replace {
            let transport = TcpZoneOwnerRpcTransport::with_placement(
                &placement,
                self.rpc_session_id.clone(),
                self.auth_token.clone(),
                self.limits.clone(),
            )?;
            *active = Some(ActivePlacementTransport {
                generation: placement.generation,
                endpoint_signature,
                transport,
            });
        }
        operation(
            &active
                .as_ref()
                .expect("Gate 15 active placement transport exists")
                .transport,
        )
    }
}

impl ZoneOwnerRpcTransport for Gate15ZoneOwnerRpcTransport {
    fn on_connect(&self) -> Result<Vec<ServerPacket>, String> {
        self.with_transport(ZoneOwnerRpcTransport::on_connect)
    }

    fn execute(&self, request: ZoneOwnerCommandRequest) -> Result<WorldCommandExecution, String> {
        self.with_transport(|transport| transport.execute(request))
    }

    fn world_snapshot(&self) -> Result<WorldSnapshot, String> {
        self.with_transport(ZoneOwnerRpcTransport::world_snapshot)
    }

    fn active_identity(&self) -> Result<Option<ActiveSessionIdentity>, String> {
        self.with_transport(ZoneOwnerRpcTransport::active_identity)
    }

    fn save_active_character(&self) -> Result<(), String> {
        self.with_transport(ZoneOwnerRpcTransport::save_active_character)
    }

    fn refresh_active_external_mail(&self) -> Result<bool, String> {
        self.with_transport(ZoneOwnerRpcTransport::refresh_active_external_mail)
    }

    fn close_session(&self, owner_lease: &ZoneOwnerLease) -> Result<(), String> {
        self.with_transport(|transport| transport.close_session(owner_lease))
    }

    fn register_live_outbound(
        &self,
        sender: crate::routing::SharedZoneLiveOutboundSender,
    ) -> Result<Option<Box<dyn ZoneLiveOutboundRegistration>>, String> {
        self.with_transport(|transport| transport.register_live_outbound(sender))
    }
}

fn validator_urls_from_env() -> Result<Option<Vec<String>>, String> {
    let Some(raw) = env::var("MIR2_GATE15_VALIDATOR_URLS")
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };
    let urls = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if urls.len() != 4 {
        return Err("MIR2_GATE15_VALIDATOR_URLS must contain four URLs".to_string());
    }
    Ok(Some(urls))
}

fn placement_from_state(
    state: &Gate14AuthoritativeState,
    zone_id: &ZoneId,
    now_ms: u64,
) -> Result<ZonePlacementLease, String> {
    let placement = state
        .placement(zone_id.as_str(), now_ms)
        .ok_or_else(|| format!("Zone {zone_id} has no live quorum-finalized placement"))?;
    let primary = state
        .zone_hosts
        .get(&placement.primary_host_id)
        .ok_or_else(|| {
            format!(
                "finalized primary Zone Host {} is missing",
                placement.primary_host_id
            )
        })?;
    let replicas = placement
        .replica_host_ids
        .iter()
        .map(|host_id| {
            state
                .zone_hosts
                .get(host_id)
                .map(|host| ZonePlacementEndpoint {
                    host_id: host.host_id.clone(),
                    endpoint: host.endpoint.clone(),
                    failure_domain: host.failure_domain.clone(),
                })
                .ok_or_else(|| format!("finalized replica Zone Host {host_id} is missing"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ZonePlacementLease {
        zone_id: zone_id.clone(),
        generation: placement.generation,
        primary: ZonePlacementEndpoint {
            host_id: primary.host_id.clone(),
            endpoint: primary.endpoint.clone(),
            failure_domain: primary.failure_domain.clone(),
        },
        replicas,
        expires_at_ms: placement.expires_at_ms,
    })
}

fn character_id_for_index(
    state: &Gate14AuthoritativeState,
    account_id: &str,
    character_index: i32,
) -> Result<String, String> {
    let account = state
        .accounts
        .get(account_id)
        .ok_or_else(|| format!("account {account_id} is not finalized in Commonware state"))?;
    let canonical = format!("{account_id}:{character_index}");
    if account.characters.contains_key(&canonical) {
        return Ok(canonical);
    }
    account
        .characters
        .keys()
        .nth(character_index as usize)
        .cloned()
        .ok_or_else(|| {
            format!(
                "character index {character_index} for account {account_id} is not finalized in Commonware state"
            )
        })
}

fn player_session_id(account_id: &str, character_index: i32) -> String {
    format!("player:{account_id}:{character_index}")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn next_gate15_rpc_session_id() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("gate15-rpc-{}-{}-{sequence}", std::process::id(), now_ms())
}

#[cfg(test)]
mod tests {
    use super::{
        character_finalization_plan, character_id_for_index, placement_from_state,
        player_session_id,
    };
    use crate::gate14::{
        Gate14Account, Gate14AuthoritativeState, Gate14Character, Gate14Placement, Gate14ZoneHost,
    };
    use crate::ZoneId;
    use std::collections::BTreeMap;

    fn finalized_state() -> Gate14AuthoritativeState {
        let mut state = Gate14AuthoritativeState::default();
        state.zone_hosts.insert(
            "dubhe-a".to_string(),
            Gate14ZoneHost {
                host_id: "dubhe-a".to_string(),
                endpoint: "dubhe-a:7020".to_string(),
                failure_domain: "rack-a".to_string(),
                max_sessions: 128,
                max_zones: 8,
            },
        );
        state.zone_hosts.insert(
            "dubhe-b".to_string(),
            Gate14ZoneHost {
                host_id: "dubhe-b".to_string(),
                endpoint: "dubhe-b:7020".to_string(),
                failure_domain: "rack-b".to_string(),
                max_sessions: 128,
                max_zones: 8,
            },
        );
        state.placements.insert(
            "mir2-map-0".to_string(),
            Gate14Placement {
                zone_id: "mir2-map-0".to_string(),
                generation: 7,
                primary_host_id: "dubhe-a".to_string(),
                replica_host_ids: vec!["dubhe-b".to_string()],
                expires_at_ms: 10_000,
            },
        );
        state.accounts.insert(
            "demo".to_string(),
            Gate14Account {
                account_id: "demo".to_string(),
                characters: BTreeMap::from([(
                    "demo-hero".to_string(),
                    Gate14Character {
                        character_id: "demo-hero".to_string(),
                        name: "Scout".to_string(),
                        gold: 0,
                        inventory: BTreeMap::new(),
                    },
                )]),
            },
        );
        state
    }

    #[test]
    fn finalized_placement_becomes_primary_first_rpc_route() {
        let placement = placement_from_state(&finalized_state(), &ZoneId::new("mir2-map-0"), 5_000)
            .expect("placement should resolve");
        assert_eq!(placement.generation, 7);
        assert_eq!(
            placement.endpoints(),
            vec!["dubhe-a:7020".to_string(), "dubhe-b:7020".to_string()]
        );
    }

    #[test]
    fn real_character_index_resolves_to_finalized_identity() {
        let state = finalized_state();
        assert_eq!(
            character_id_for_index(&state, "demo", 0).expect("character should resolve"),
            "demo-hero"
        );
        assert_eq!(player_session_id("demo", 0), "player:demo:0");
    }

    #[test]
    fn migrated_control_name_alias_does_not_reject_the_same_character_slot() {
        let mut state = finalized_state();
        state.accounts.get_mut("demo").unwrap().characters = BTreeMap::from([(
            "demo:0".to_string(),
            Gate14Character {
                character_id: "demo:0".to_string(),
                name: "Scout~b98ded00".to_string(),
                gold: 0,
                inventory: BTreeMap::new(),
            },
        )]);

        assert_eq!(
            character_finalization_plan(&state, "demo", 0, "Scout")
                .expect("canonical migrated slot should be accepted"),
            None
        );
    }

    #[test]
    fn duplicate_display_name_gets_a_deterministic_control_alias() {
        let mut state = finalized_state();
        state.accounts.insert(
            "new-account".to_string(),
            Gate14Account {
                account_id: "new-account".to_string(),
                characters: BTreeMap::new(),
            },
        );

        let first = character_finalization_plan(&state, "new-account", 0, "Scout")
            .expect("duplicate name should be disambiguated")
            .expect("new slot should require finalization");
        let second = character_finalization_plan(&state, "new-account", 0, "Scout")
            .expect("duplicate name should be disambiguated")
            .expect("new slot should require finalization");

        assert_eq!(first, second);
        assert!(first.starts_with("Scout~"));
        assert_ne!(first, "Scout");
        assert!(first.len() <= 256);
    }
}
